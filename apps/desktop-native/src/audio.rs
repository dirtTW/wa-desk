//! Audio playback: WhatsApp voice notes are OGG/Opus, which the usual decoders
//! do not handle, so the packets are decoded with libopus (`audiopus`) and fed
//! to a `rodio` sink. Regular files go through rodio's own decoders.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex};

/// What the window shows for the player bubble.
#[derive(Debug, Default, Clone)]
pub struct PlaybackState {
    /// The message being played, if any.
    pub id: Option<String>,
    pub playing: bool,
    /// Seconds played so far.
    pub elapsed: f32,
    /// Total seconds, when known.
    pub duration: f32,
}

/// The peer's playout level, published by the audio thread as f32 bits so the
/// microphone can duck while the speaker is loud (see `echo`).
static CALL_LEVEL: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
static CALL_LEVEL_AT: std::sync::Mutex<Option<std::time::Instant>> = std::sync::Mutex::new(None);

/// The recent playout samples, for the echo canceller.
static CALL_REFERENCE: std::sync::OnceLock<std::sync::Mutex<crate::aec::Reference>> =
    std::sync::OnceLock::new();

fn reference_ring() -> &'static std::sync::Mutex<crate::aec::Reference> {
    CALL_REFERENCE.get_or_init(|| std::sync::Mutex::new(crate::aec::Reference::default()))
}

/// The last `len` playout samples, `crate::aec::DELAY` behind the newest one.
pub fn call_reference(len: usize) -> Vec<f32> {
    match reference_ring().lock() {
        Ok(reference) => reference.take(len, crate::aec::DELAY),
        Err(_) => vec![0.0; len],
    }
}

/// The peer's level right now (RMS, 0.0 when silent or stale).
pub fn call_level() -> f32 {
    if let Ok(at) = CALL_LEVEL_AT.lock()
        && let Some(at) = *at
        && at.elapsed() > std::time::Duration::from_millis(400)
    {
        return 0.0;
    }
    f32::from_bits(CALL_LEVEL.load(std::sync::atomic::Ordering::Relaxed))
}

fn publish_call_level(samples: &[i16]) {
    if samples.is_empty() {
        return;
    }
    if let Ok(mut reference) = reference_ring().lock() {
        for sample in samples {
            reference.push(*sample as f32 / 32_768.0);
        }
    }
    let sum: f64 = samples
        .iter()
        .map(|sample| {
            let value = *sample as f64 / 32_768.0;
            value * value
        })
        .sum();
    let rms = (sum / samples.len() as f64).sqrt() as f32;
    CALL_LEVEL.store(rms.to_bits(), std::sync::atomic::Ordering::Relaxed);
    if let Ok(mut at) = CALL_LEVEL_AT.lock() {
        *at = Some(std::time::Instant::now());
    }
}

enum Command {
    Play {
        id: String,
        path: String,
        duration: f32,
        /// Seconds to skip before starting (the video viewer seeks this way).
        start_at: f32,
    },
    Seek {
        seconds: f32,
    },
    Volume(f32),
    /// One 16 kHz mono playout frame from the peer.
    CallAudio(Vec<i16>),
    /// The call is over: drop the playout queue.
    CallStop,
    Stop,
}

/// Handle used by the window; the sink lives on its own thread.
pub struct Audio {
    tx: Sender<Command>,
    state: Arc<Mutex<PlaybackState>>,
    running: Arc<AtomicBool>,
}

impl Audio {
    /// Start the playback thread (no-op if there is no audio device).
    pub fn new() -> Self {
        let (tx, rx) = channel::<Command>();
        let state = Arc::new(Mutex::new(PlaybackState::default()));
        let worker_state = state.clone();
        let running = Arc::new(AtomicBool::new(true));
        let worker_running = running.clone();
        std::thread::spawn(move || {
            audio_thread(rx, worker_state, worker_running);
        });
        Self { tx, state, running }
    }

    pub fn state(&self) -> PlaybackState {
        self.state
            .lock()
            .map(|state| state.clone())
            .unwrap_or_default()
    }

    /// Play, or stop when the same message is already playing.
    pub fn toggle(&self, id: &str, path: &str, duration: f32) {
        let current = self.state();
        if current.id.as_deref() == Some(id) && current.playing {
            let _ = self.tx.send(Command::Stop);
        } else {
            self.play_from(id, path, duration, 0.0);
        }
    }

    /// Start `id` from the beginning or from `start_at` seconds.
    pub fn play_from(&self, id: &str, path: &str, duration: f32, start_at: f32) {
        let _ = self.tx.send(Command::Play {
            id: id.to_string(),
            path: path.to_string(),
            duration,
            start_at,
        });
    }

    /// Jump inside the clip that is playing.
    pub fn seek(&self, seconds: f32) {
        let _ = self.tx.send(Command::Seek { seconds });
    }

    /// 0.0 mutes, 1.0 is full volume.
    pub fn set_volume(&self, volume: f32) {
        let _ = self.tx.send(Command::Volume(volume));
    }

    /// Play one 16 kHz mono frame from the peer (a call's playout).
    pub fn play_call_audio(&self, samples: Vec<i16>) {
        let _ = self.tx.send(Command::CallAudio(samples));
    }

    /// Drop the playout queue when the call ends.
    pub fn stop_call(&self) {
        let _ = self.tx.send(Command::CallStop);
    }

    pub fn stop(&self) {
        let _ = self.tx.send(Command::Stop);
    }
}

impl Drop for Audio {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        let _ = self.tx.send(Command::Stop);
    }
}

fn audio_thread(
    rx: std::sync::mpsc::Receiver<Command>,
    state: Arc<Mutex<PlaybackState>>,
    running: Arc<AtomicBool>,
) {
    use rodio::OutputStream;
    use rodio::Sink;

    let Ok((_stream, handle)) = OutputStream::try_default() else {
        // No device (or no permission): keep the handle alive, play nothing.
        while running.load(Ordering::Relaxed) {
            let _ = rx.recv_timeout(std::time::Duration::from_millis(250));
        }
        return;
    };
    let Ok(sink) = Sink::try_new(&handle) else {
        return;
    };
    // The call has its own sink: a voice note and a call never share a queue.
    let Ok(call_sink) = Sink::try_new(&handle) else {
        return;
    };
    let mut volume = 1.0f32;

    while running.load(Ordering::Relaxed) {
        match rx.recv_timeout(std::time::Duration::from_millis(120)) {
            Ok(Command::Play {
                id,
                path,
                duration,
                start_at,
            }) => {
                sink.stop();
                match load(&path) {
                    Ok(mut source) => {
                        if start_at > 0.05 {
                            use rodio::Source as _;
                            let _ = source.try_seek(std::time::Duration::from_secs_f32(start_at));
                        }
                        sink.set_volume(volume);
                        sink.append(source);
                        sink.play();
                        if let Ok(mut guard) = state.lock() {
                            guard.id = Some(id);
                            guard.playing = true;
                            guard.elapsed = start_at;
                            guard.duration = duration;
                        }
                    }
                    Err(err) => {
                        crate::app::diag(&format!("audio {path}: {err}"));
                        if let Ok(mut guard) = state.lock() {
                            guard.id = None;
                            guard.playing = false;
                        }
                    }
                }
            }
            Ok(Command::Seek { seconds }) => {
                let _ = sink.try_seek(std::time::Duration::from_secs_f32(seconds.max(0.0)));
                if let Ok(mut guard) = state.lock() {
                    guard.elapsed = seconds.max(0.0);
                }
            }
            Ok(Command::Volume(value)) => {
                volume = value.clamp(0.0, 1.0);
                sink.set_volume(volume);
            }
            Ok(Command::CallAudio(samples)) => {
                if samples.is_empty() {
                    continue;
                }
                publish_call_level(&samples);
                // 16 kHz mono PCM from the peer (`wa_ui::calls::audio_format`).
                let pcm: Vec<f32> = samples
                    .into_iter()
                    .map(|sample| sample as f32 / 32_768.0)
                    .collect();
                // Do not let a stalled device grow the queue without bound.
                if call_sink.len() > 40 {
                    continue;
                }
                call_sink.set_volume(volume);
                call_sink.append(rodio::buffer::SamplesBuffer::new(1, 16_000, pcm));
                call_sink.play();
            }
            Ok(Command::CallStop) => {
                call_sink.stop();
                publish_call_level(&[]);
                CALL_LEVEL.store(0, std::sync::atomic::Ordering::Relaxed);
            }
            Ok(Command::Stop) => {
                sink.stop();
                if let Ok(mut guard) = state.lock() {
                    guard.id = None;
                    guard.playing = false;
                    guard.elapsed = 0.0;
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }

        if let Ok(mut guard) = state.lock()
            && guard.playing
        {
            guard.elapsed = sink.get_pos().as_secs_f32();
            if sink.empty() {
                guard.playing = false;
                guard.id = None;
                guard.elapsed = 0.0;
            }
        }
    }
}

/// Decode a file into a playable source.
fn load(path: &str) -> Result<rodio::buffer::SamplesBuffer<f32>, String> {
    use rodio::Source as _;
    let extension = Path::new(path)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if matches!(extension.as_str(), "ogg" | "opus" | "oga") {
        let bytes = std::fs::read(path).map_err(|err| err.to_string())?;
        if let Ok(samples) = decode_opus(&bytes) {
            return Ok(rodio::buffer::SamplesBuffer::new(2, 48_000, samples));
        }
    }
    // Anything else rodio understands (mp3, wav, vorbis, …).
    let file = std::fs::File::open(path).map_err(|err| err.to_string())?;
    let decoder = rodio::Decoder::new(std::io::BufReader::new(file))
        .map_err(|err| format!("unsupported audio: {err}"))?;
    let channels = decoder.channels();
    let sample_rate = decoder.sample_rate();
    let samples: Vec<f32> = decoder.map(|sample| sample as f32 / 32_768.0).collect();
    Ok(rodio::buffer::SamplesBuffer::new(
        channels,
        sample_rate,
        samples,
    ))
}

/// OGG/Opus (WhatsApp voice notes) via libopus.
fn decode_opus(bytes: &[u8]) -> Result<Vec<f32>, String> {
    use audiopus::{Channels, SampleRate, coder::Decoder};

    let cursor = std::io::Cursor::new(bytes.to_vec());
    let mut reader = ogg::PacketReader::new(cursor);
    let mut decoder = Decoder::new(SampleRate::Hz48000, Channels::Stereo)
        .map_err(|err| format!("opus: {err}"))?;
    let mut samples: Vec<f32> = Vec::new();
    let mut packet = reader.read_packet().map_err(|err| err.to_string())?;
    while let Some(current) = packet {
        if !(current.data.starts_with(b"OpusHead") || current.data.starts_with(b"OpusTags")) {
            let mut pcm = vec![0i16; 5_760 * 2];
            if let Ok(per_channel) = decoder.decode(Some(&current.data), &mut pcm[..], false) {
                for value in pcm[..per_channel * 2].iter() {
                    samples.push(*value as f32 / 32_768.0);
                }
            }
        }
        packet = reader.read_packet().map_err(|err| err.to_string())?;
    }
    if samples.is_empty() {
        return Err("no opus frames".to_string());
    }
    Ok(samples)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unknown_file_is_rejected() {
        let path = std::env::temp_dir().join("wa-desk-not-audio.bin");
        std::fs::write(&path, b"not audio").expect("write");
        assert!(load(path.to_str().unwrap()).is_err());
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn the_player_starts_idle() {
        let state = PlaybackState::default();
        assert!(state.id.is_none());
        assert!(!state.playing);
    }
}
