//! Call-overlay state: which buttons are visible, the status line and the
//! outgoing-call guards.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Deserialize)]
pub struct CallOverlayInput {
    /// `ringing`, `calling`, `connecting` or `connected`.
    #[serde(default)]
    pub state: String,
    #[serde(default)]
    pub video: bool,
    #[serde(default)]
    pub send_video: bool,
    #[serde(default)]
    pub muted: bool,
    /// Unix ms when the call connected, if it has.
    #[serde(default)]
    pub started_at: Option<i64>,
    #[serde(default)]
    pub now: i64,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub struct CallOverlay {
    pub ringing: bool,
    /// The video stage (remote canvas + self view) is visible.
    pub video_stage: bool,
    pub self_view: bool,
    pub accept: bool,
    pub reject: bool,
    pub mute: bool,
    pub hangup: bool,
    pub mute_pressed: bool,
}

impl CallOverlay {
    /// The status line the overlay shows.
    pub fn status(&self, input: &CallOverlayInput) -> String {
        if self.ringing {
            return if input.video {
                "Incoming video call".to_string()
            } else {
                "Incoming voice call".to_string()
            };
        }
        match input.started_at {
            Some(started) => {
                let elapsed = (input.now - started).max(0) / 1000;
                format!("Connected \u{b7} {:02}:{:02}", elapsed / 60, elapsed % 60)
            }
            None => "Calling...".to_string(),
        }
    }
}

/// Which controls the overlay shows for a call in `state`.
pub fn overlay(input: &CallOverlayInput) -> CallOverlay {
    let ringing = input.state == "ringing";
    CallOverlay {
        ringing,
        video_stage: input.video && !ringing,
        self_view: input.send_video,
        accept: ringing,
        reject: ringing,
        mute: !ringing,
        hangup: !ringing,
        mute_pressed: input.muted,
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CallGuard {
    pub ok: bool,
    pub toast: Option<String>,
}

/// Refuse a new outgoing call for a group or while another call is up.
pub fn call_guard(is_group: bool, has_active_call: bool) -> CallGuard {
    if is_group {
        return CallGuard {
            ok: false,
            toast: Some("Group calls aren't supported yet.".to_string()),
        };
    }
    if has_active_call {
        return CallGuard {
            ok: false,
            toast: Some("A call is already active.".to_string()),
        };
    }
    CallGuard {
        ok: true,
        toast: None,
    }
}

/// The toast when the required media inputs are missing.
pub fn media_unavailable_toast(video: bool) -> &'static str {
    if video {
        "No camera/microphone or H.264 encoder available."
    } else {
        "No microphone available."
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct VideoConfig {
    pub codec: &'static str,
    pub width: u32,
    pub height: u32,
    pub bitrate: u32,
    pub framerate: u32,
    pub avc_format: &'static str,
    /// A keyframe is forced every N frames so a late join can decode.
    pub keyframe_interval: u32,
}

/// The H.264 profile this client asks WebCodecs for, on both directions.
pub const VIDEO_CODEC: &str = "avc1.42E01F";
pub const VIDEO_WIDTH: u32 = 640;
pub const VIDEO_HEIGHT: u32 = 480;
pub const VIDEO_BITRATE: u32 = 700_000;
pub const VIDEO_FRAMERATE: u32 = 15;
pub const VIDEO_KEYFRAME_INTERVAL: u32 = 30;

pub fn video_config() -> VideoConfig {
    VideoConfig {
        codec: VIDEO_CODEC,
        width: VIDEO_WIDTH,
        height: VIDEO_HEIGHT,
        bitrate: VIDEO_BITRATE,
        framerate: VIDEO_FRAMERATE,
        avc_format: "annexb",
        keyframe_interval: VIDEO_KEYFRAME_INTERVAL,
    }
}

/// Whether a frame must be encoded as a keyframe.
pub fn is_keyframe(frame_number: u32, interval: u32) -> bool {
    interval == 0 || frame_number.is_multiple_of(interval)
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct AudioFormat {
    /// Mono PCM at 16 kHz, 60 ms per frame.
    pub sample_rate: u32,
    pub frame_samples: usize,
}

pub const AUDIO_SAMPLE_RATE: u32 = 16_000;
pub const AUDIO_FRAME_SAMPLES: usize = 960;

pub fn audio_format() -> AudioFormat {
    AudioFormat {
        sample_rate: AUDIO_SAMPLE_RATE,
        frame_samples: AUDIO_FRAME_SAMPLES,
    }
}

/// Float sample (-1..1) to signed 16-bit, symmetric like the official client.
pub fn pcm_i16(sample: f32) -> i16 {
    let clamped = sample.clamp(-1.0, 1.0);
    if clamped < 0.0 {
        (clamped * 0x8000 as f32) as i16
    } else {
        (clamped * 0x7fff as f32) as i16
    }
}

/// When to start the next remote audio buffer: a small jitter buffer that resets
/// when the peer falls behind (under 20 ms) or runs far ahead (over 600 ms).
pub fn next_play_time(next_play: f64, now: f64, duration: f64) -> (f64, f64) {
    let start = if next_play == 0.0 || next_play < now + 0.02 || next_play > now + 0.6 {
        now + 0.06
    } else {
        next_play
    };
    (start, start + duration)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(state: &str, video: bool) -> CallOverlayInput {
        CallOverlayInput {
            state: state.into(),
            video,
            send_video: video,
            ..Default::default()
        }
    }

    #[test]
    fn video_and_audio_formats_match_the_wire() {
        let video = video_config();
        assert_eq!(video.codec, "avc1.42E01F");
        assert_eq!((video.width, video.height), (640, 480));
        assert_eq!(video.bitrate, 700_000);
        assert_eq!(video.framerate, 15);
        assert_eq!(video.avc_format, "annexb");
        assert!(is_keyframe(0, video.keyframe_interval));
        assert!(!is_keyframe(29, video.keyframe_interval));
        assert!(is_keyframe(30, video.keyframe_interval));
        assert!(is_keyframe(7, 0), "an interval of 0 means every frame");

        let audio = audio_format();
        assert_eq!(audio.sample_rate, 16_000);
        assert_eq!(audio.frame_samples, 960, "60 ms per frame");
    }

    #[test]
    fn pcm_conversion_is_symmetric() {
        assert_eq!(pcm_i16(0.0), 0);
        assert_eq!(pcm_i16(1.0), 0x7fff);
        assert_eq!(pcm_i16(-1.0), -0x8000);
        assert_eq!(pcm_i16(2.0), 0x7fff, "clamped");
        assert_eq!(pcm_i16(-2.0), -0x8000);
        assert_eq!(pcm_i16(0.5), (0.5 * 0x7fff as f32) as i16);
    }

    #[test]
    fn audio_jitter_buffer_resets_when_out_of_window() {
        let close =
            |a: (f64, f64), b: (f64, f64)| (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9;
        // Nothing scheduled yet: start shortly after now.
        assert!(close(next_play_time(0.0, 10.0, 0.06), (10.06, 10.12)));
        // Far behind (under now + 20 ms): reset.
        assert!(close(next_play_time(10.005, 10.0, 0.06), (10.06, 10.12)));
        // Far ahead (over now + 600 ms): reset instead of queueing seconds.
        assert!(close(next_play_time(10.7, 10.0, 0.06), (10.06, 10.12)));
        // In the window: keep the running schedule.
        assert!(close(next_play_time(10.2, 10.0, 0.06), (10.2, 10.26)));
    }

    #[test]
    fn ringing_offers_accept_and_reject() {
        let overlay = overlay(&input("ringing", true));
        assert!(overlay.ringing && overlay.accept && overlay.reject);
        assert!(!overlay.mute && !overlay.hangup);
        assert!(!overlay.video_stage, "the stage waits for the answer");
        assert_eq!(
            overlay.status(&input("ringing", true)),
            "Incoming video call"
        );
        assert_eq!(
            overlay.status(&input("ringing", false)),
            "Incoming voice call"
        );
    }

    #[test]
    fn answered_calls_show_the_stage_and_timer() {
        let mut call = input("connected", true);
        call.started_at = Some(10_000);
        call.now = 10_000 + 65_000;
        let state = overlay(&call);
        assert!(!state.accept && !state.reject);
        assert!(state.mute && state.hangup);
        assert!(state.video_stage && state.self_view);
        assert_eq!(state.status(&call), "Connected \u{b7} 01:05");

        let mut voice = input("calling", false);
        voice.started_at = None;
        assert_eq!(overlay(&voice).status(&voice), "Calling...");
        assert!(!overlay(&voice).video_stage);
    }

    #[test]
    fn camera_less_video_calls_keep_the_stage_but_hide_the_self_view() {
        let mut call = input("connecting", true);
        call.send_video = false;
        let overlay = overlay(&call);
        assert!(overlay.video_stage, "the peer is still visible");
        assert!(!overlay.self_view, "no black self tile");
    }

    #[test]
    fn muted_state_and_guards() {
        let mut call = input("connected", false);
        call.muted = true;
        assert!(overlay(&call).mute_pressed);

        let group = call_guard(true, false);
        assert!(!group.ok);
        assert_eq!(
            group.toast.as_deref(),
            Some("Group calls aren't supported yet.")
        );
        let busy = call_guard(false, true);
        assert_eq!(busy.toast.as_deref(), Some("A call is already active."));
        assert!(call_guard(false, false).ok);
        assert_eq!(
            media_unavailable_toast(true),
            "No camera/microphone or H.264 encoder available."
        );
        assert_eq!(media_unavailable_toast(false), "No microphone available.");
    }
}
