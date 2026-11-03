//! calls operations for `Session`.

use super::*;

impl Session {
    /// Place a 1:1 call. Audio frames come from the webview (16 kHz mono i16,
    /// 960 samples per 60 ms); decoded peer frames go back over `wa:event`.
    pub async fn start_call(&self, chat: &str, video: bool) -> Result<CallInfo, String> {
        if self.active_call.lock().expect("call lock").is_some() {
            return Err("a call is already active".to_string());
        }
        let peer = self.resolve_chat(chat)?;
        if peer.server == Server::Group {
            return Err("group calls are not supported yet".to_string());
        }

        let (audio_in_tx, audio_in_rx) = async_channel::bounded::<Vec<i16>>(8);
        let (audio_out_tx, audio_out_rx) = async_channel::bounded::<Vec<i16>>(8);
        let (video_in_tx, video_in_rx) = async_channel::bounded::<Vec<u8>>(4);
        let (video_out_tx, video_out_rx) = async_channel::bounded::<VideoFrame>(4);

        let voip = self.client.voip();
        let mut builder = voip.call(&peer).audio(audio_in_rx, audio_out_tx);
        if video {
            builder = builder.video(video_in_rx, video_out_tx);
        }
        let handle = builder
            .start()
            .await
            .map_err(|err| format!("could not start the call: {err}"))?;
        let call_id = handle.call_id().to_string();
        let info = CallInfo {
            call_id: call_id.clone(),
            peer: chat.to_string(),
            video,
            outgoing: true,
        };
        self.active_call
            .lock()
            .expect("call lock")
            .replace(ActiveCall {
                handle: handle.clone(),
                audio_in: audio_in_tx,
                video_in: video.then_some(video_in_tx),
                info: info.clone(),
            });
        self.wire_call_media(call_id.clone(), video, audio_out_rx, video_out_rx);
        self.spawn_call_end(handle.clone(), call_id.clone());
        self.spawn_call_events(handle.clone(), call_id.clone());
        let _ = self.tx.send(UiEvent::CallState {
            call_id,
            state: "calling".to_string(),
        });
        Ok(info)
    }

    /// Answer an incoming offer with the same webview media endpoints.
    ///
    /// A video *offer* is always answered with video, even when this machine
    /// has no camera: the library builds its media engine from the endpoints it
    /// is given, so an audio-only accept would drop the caller's frames too
    /// (the "video calls arrive black" bug). Without a camera we simply never
    /// push frames into the source channel.
    pub async fn accept_call(&self, call_id: &str, video: bool) -> Result<CallInfo, String> {
        let incoming = self
            .incoming_calls
            .lock()
            .expect("call lock")
            .remove(call_id)
            .ok_or_else(|| "that call is no longer ringing".to_string())?;

        let offered_video = matches!(&incoming.action, CallAction::Offer { is_video: true, .. });
        let video = negotiated_video(offered_video, video);

        let (audio_in_tx, audio_in_rx) = async_channel::bounded::<Vec<i16>>(8);
        let (audio_out_tx, audio_out_rx) = async_channel::bounded::<Vec<i16>>(8);
        let (video_in_tx, video_in_rx) = async_channel::bounded::<Vec<u8>>(4);
        let (video_out_tx, video_out_rx) = async_channel::bounded::<VideoFrame>(4);

        let voip = self.client.voip();
        let mut builder = voip.accept(&incoming).audio(audio_in_rx, audio_out_tx);
        if video {
            builder = builder.video(video_in_rx, video_out_tx);
        }
        let handle = builder
            .start()
            .await
            .map_err(|err| format!("could not answer the call: {err}"))?;
        let info = CallInfo {
            call_id: call_id.to_string(),
            peer: incoming.from.to_string(),
            video,
            outgoing: false,
        };
        self.active_call
            .lock()
            .expect("call lock")
            .replace(ActiveCall {
                handle: handle.clone(),
                audio_in: audio_in_tx,
                video_in: video.then_some(video_in_tx),
                info: info.clone(),
            });
        self.wire_call_media(call_id.to_string(), video, audio_out_rx, video_out_rx);
        self.spawn_call_end(handle.clone(), call_id.to_string());
        self.spawn_call_events(handle.clone(), call_id.to_string());
        let _ = self.tx.send(UiEvent::CallState {
            call_id: call_id.to_string(),
            state: "connected".to_string(),
        });
        Ok(info)
    }

    /// Decline a ringing offer.
    pub async fn reject_call(&self, call_id: &str) -> Result<(), String> {
        let incoming = self
            .incoming_calls
            .lock()
            .expect("call lock")
            .remove(call_id)
            .ok_or_else(|| "that call is no longer ringing".to_string())?;
        self.client
            .voip()
            .reject(&incoming)
            .await
            .map_err(|err| format!("could not reject the call: {err}"))
    }

    /// Hang up the active call.
    pub async fn hangup_call(&self) -> Result<(), String> {
        let call = self.active_call.lock().expect("call lock").take();
        if let Some(call) = call {
            call.handle.hangup().await;
        }
        Ok(())
    }

    /// Mute/unmute the microphone on the active call.
    pub fn set_call_muted(&self, muted: bool) -> Result<(), String> {
        let call = self.active_call.lock().expect("call lock");
        let call = call.as_ref().ok_or_else(|| "no active call".to_string())?;
        call.handle.set_muted(muted);
        Ok(())
    }

    /// Push one 16 kHz mono PCM frame from the webview into the call.
    pub fn send_call_audio(&self, samples: Vec<i16>) -> Result<(), String> {
        let call = self.active_call.lock().expect("call lock");
        let call = call.as_ref().ok_or_else(|| "no active call".to_string())?;
        call.audio_in
            .try_send(samples)
            .map_err(|_| "the call audio queue is full".to_string())
    }

    /// Push one Annex-B H.264 access unit from the webview into the call.
    pub fn send_call_video(&self, data: Vec<u8>) -> Result<(), String> {
        let call = self.active_call.lock().expect("call lock");
        let call = call.as_ref().ok_or_else(|| "no active call".to_string())?;
        let video = call
            .video_in
            .as_ref()
            .ok_or_else(|| "this is an audio call".to_string())?;
        video
            .try_send(data)
            .map_err(|_| "the call video queue is full".to_string())
    }

    fn wire_call_media(
        &self,
        call_id: String,
        video: bool,
        audio_out: async_channel::Receiver<Vec<i16>>,
        video_out: async_channel::Receiver<VideoFrame>,
    ) {
        let tx = self.tx.clone();
        let audio_id = call_id.clone();
        tokio::spawn(async move {
            while let Ok(samples) = audio_out.recv().await {
                let _ = tx.send(UiEvent::CallAudio {
                    call_id: audio_id.clone(),
                    samples,
                });
            }
        });
        if video {
            let tx = self.tx.clone();
            let video_id = call_id;
            tokio::spawn(async move {
                use base64::Engine as _;
                while let Ok(frame) = video_out.recv().await {
                    let _ = tx.send(UiEvent::CallVideo {
                        call_id: video_id.clone(),
                        data: base64::engine::general_purpose::STANDARD.encode(frame.data),
                        keyframe: frame.keyframe,
                    });
                }
            });
        }
    }

    fn spawn_call_end(&self, handle: CallHandle, call_id: String) {
        let tx = self.tx.clone();
        let active = self.active_call.clone();
        tokio::spawn(async move {
            handle.wait_ended().await;
            {
                let mut slot = active.lock().expect("call lock");
                if slot
                    .as_ref()
                    .is_some_and(|call| call.info.call_id == call_id)
                {
                    *slot = None;
                }
            }
            let _ = tx.send(UiEvent::CallEnded {
                call_id,
                reason: "call ended".to_string(),
            });
        });
    }

    fn spawn_call_events(&self, handle: CallHandle, call_id: String) {
        let tx = self.tx.clone();
        let events = handle.events();
        tokio::spawn(async move {
            while let Ok(event) = events.recv().await {
                match event {
                    CallEvent::RelayAllocated => {
                        let _ = tx.send(UiEvent::CallState {
                            call_id: call_id.clone(),
                            state: "connected".to_string(),
                        });
                    }
                    CallEvent::AudioFormatMismatch {
                        expected_rate,
                        received_rates,
                    } => {
                        let _ = tx.send(UiEvent::CallEnded {
                            call_id: call_id.clone(),
                            reason: format!(
                                "audio format mismatch: expected {expected_rate} Hz, peer sent {received_rates:?}"
                            ),
                        });
                    }
                    CallEvent::RelayAllocateFailed(code) => {
                        let _ = tx.send(UiEvent::CallEnded {
                            call_id: call_id.clone(),
                            reason: format!("relay rejected the call ({code})"),
                        });
                    }
                    CallEvent::RelayAllocateTimedOut | CallEvent::RelayReconnectTimedOut => {
                        let _ = tx.send(UiEvent::CallEnded {
                            call_id: call_id.clone(),
                            reason: "the call relay timed out".to_string(),
                        });
                    }
                    _ => {}
                }
            }
        });
    }
}

/// Whether an answered call negotiates video media.
///
/// The offer wins: a video call stays a video call for both directions even
/// when the answering side has no camera. Asking for video on a voice offer
/// would be refused by the library (`VideoNotOffered`), so that direction is
/// not upgraded here.
fn negotiated_video(offered: bool, requested: bool) -> bool {
    offered || requested
}

#[cfg(test)]
mod tests {
    use super::negotiated_video;

    #[test]
    fn video_offer_is_always_answered_with_video() {
        // Camera-less accept: still negotiated, so the peer's frames arrive.
        assert!(negotiated_video(true, false));
        assert!(negotiated_video(true, true));
        // A voice offer is never upgraded behind the peer's back.
        assert!(!negotiated_video(false, false));
        assert!(negotiated_video(false, true));
    }
}
