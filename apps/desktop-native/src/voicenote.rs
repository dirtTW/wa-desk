//! Voice-note recording: the microphone is captured at 16 kHz mono (the same
//! path the calls use), encoded to Opus and muxed into Ogg - the container
//! WhatsApp expects for a `ptt` (push-to-talk) audio message.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;

/// Opus wants 20 ms frames at 16 kHz.
const FRAME_SAMPLES: usize = 320;
/// Longest note the composer records (WhatsApp caps at 15 minutes).
pub const MAX_SECONDS: u32 = 900;

/// A running recording.
pub struct Recording {
    stop: Arc<AtomicBool>,
    frames: Receiver<Vec<i16>>,
    samples: Vec<i16>,
}

impl Recording {
    /// Start capturing (16 kHz mono). Dropping the value stops the microphone.
    pub fn start() -> Result<Self, String> {
        use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| "no microphone".to_string())?;
        let supported = device
            .default_input_config()
            .map_err(|err| format!("microphone: {err}"))?;
        if supported.sample_format() != cpal::SampleFormat::F32 {
            return Err(format!(
                "unsupported microphone format: {:?}",
                supported.sample_format()
            ));
        }
        let input_rate = supported.sample_rate().0 as f64;
        let channels = supported.channels() as usize;
        let config = supported.config();
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        let (tx, frames) = std::sync::mpsc::channel::<Vec<i16>>();

        std::thread::spawn(move || {
            let ratio = input_rate / crate::mic::RATE as f64;
            let mut pending: Vec<i16> = Vec::new();
            let mut carry = 0.0f64;
            let stream = device.build_input_stream(
                &config,
                move |data: &[f32], _| {
                    let mono: Vec<f32> = data
                        .chunks(channels)
                        .map(|frame| frame.iter().sum::<f32>() / channels as f32)
                        .collect();
                    let mut pos = carry;
                    while pos < mono.len() as f64 {
                        let index = pos as usize;
                        let fraction = (pos - index as f64) as f32;
                        let first = mono.get(index).copied().unwrap_or(0.0);
                        let second = mono.get(index + 1).copied().unwrap_or(first);
                        let value = first + (second - first) * fraction;
                        pending.push((value.clamp(-1.0, 1.0) * 32_767.0) as i16);
                        pos += ratio;
                    }
                    carry = pos - mono.len() as f64;
                    while pending.len() >= FRAME_SAMPLES {
                        let frame: Vec<i16> = pending.drain(..FRAME_SAMPLES).collect();
                        if tx.send(frame).is_err() {
                            return;
                        }
                    }
                },
                move |err| eprintln!("voice note capture: {err}"),
                None,
            );
            let Ok(stream) = stream else {
                return;
            };
            if stream.play().is_err() {
                return;
            }
            while !worker_stop.load(Ordering::Relaxed) {
                std::thread::sleep(std::time::Duration::from_millis(30));
            }
        });

        Ok(Self {
            stop,
            frames,
            samples: Vec::new(),
        })
    }

    /// How many seconds have been recorded so far.
    pub fn seconds(&self) -> f32 {
        self.samples.len() as f32 / crate::mic::RATE as f32
    }

    /// Drain whatever the capture thread has produced.
    pub fn pump(&mut self) {
        while let Ok(frame) = self.frames.try_recv() {
            if self.samples.len() < crate::mic::RATE as usize * MAX_SECONDS as usize {
                self.samples.extend_from_slice(&frame);
            }
        }
    }

    /// Stop the microphone and encode the note.
    pub fn finish(mut self) -> Result<Note, String> {
        self.stop.store(true, Ordering::Relaxed);
        // A last pass picks up the tail of the recording.
        std::thread::sleep(std::time::Duration::from_millis(80));
        self.pump();
        if self.samples.is_empty() {
            return Err("nothing was recorded".to_string());
        }
        encode(&self.samples)
    }
}

/// A finished note, ready for the session.
pub struct Note {
    /// Ogg/Opus bytes.
    pub ogg: Vec<u8>,
    pub seconds: u32,
    /// 64 bars (1-100) for the bubble's waveform.
    pub waveform: Vec<u8>,
}

/// Encode 16 kHz mono PCM into an Ogg/Opus stream with the two headers a
/// WhatsApp voice note carries (`OpusHead`, `OpusTags`).
pub fn encode(samples: &[i16]) -> Result<Note, String> {
    use audiopus::coder::Encoder;
    use audiopus::{Application, Channels, SampleRate};

    let mut encoder = Encoder::new(SampleRate::Hz16000, Channels::Mono, Application::Voip)
        .map_err(|err| format!("opus encoder: {err}"))?;
    encoder
        .set_bitrate(audiopus::Bitrate::BitsPerSecond(24_000))
        .map_err(|err| format!("opus bitrate: {err}"))?;

    let head = opus_head();
    let tags = opus_tags();

    // `ogg` 0.9 writes a whole packet per call and asks how the packet ends the
    // page/stream: the headers end their own pages, audio is continued.
    let mut writer = ogg::PacketWriter::new(Vec::new());
    let serial = 0x5741_4445u32; // "WADE"
    writer
        .write_packet(head, serial, ogg::PacketWriteEndInfo::EndPage, 0)
        .map_err(|err| format!("ogg header: {err}"))?;
    writer
        .write_packet(tags, serial, ogg::PacketWriteEndInfo::EndPage, 0)
        .map_err(|err| format!("ogg tags: {err}"))?;

    let mut buffer = vec![0u8; 512];
    let mut position = 0;
    let mut granule = 0u64;
    while position < samples.len() {
        let end = (position + FRAME_SAMPLES).min(samples.len());
        let mut frame = samples[position..end].to_vec();
        frame.resize(FRAME_SAMPLES, 0);
        let encoded = encoder
            .encode(&frame, &mut buffer)
            .map_err(|err| format!("opus encode: {err}"))?;
        // Copy before the writer borrows it (the encoder holds `buffer` mutably).
        let packet = buffer[..encoded].to_vec();
        // Opus granules are at 48 kHz regardless of the input rate.
        granule += (end - position) as u64 * 3;
        writer
            .write_packet(packet, serial, ogg::PacketWriteEndInfo::EndPage, granule)
            .map_err(|err| format!("ogg packet: {err}"))?;
        position = end;
    }
    let ogg = writer.into_inner();
    if ogg.is_empty() {
        return Err("the encoder produced no audio".to_string());
    }

    Ok(Note {
        ogg,
        seconds: (samples.len() as f32 / crate::mic::RATE as f32).round() as u32,
        waveform: waveform(samples),
    })
}

/// The 19-byte `OpusHead` for 16 kHz mono.
fn opus_head() -> Vec<u8> {
    let mut head = Vec::with_capacity(19);
    head.extend_from_slice(b"OpusHead");
    head.push(1); // version
    head.push(1); // channels
    head.extend_from_slice(&0u16.to_le_bytes()); // pre-skip
    head.extend_from_slice(&16_000u32.to_le_bytes()); // input rate
    head.extend_from_slice(&0i16.to_le_bytes()); // output gain
    head.push(0); // channel mapping
    head
}

/// The (empty) `OpusTags` packet.
fn opus_tags() -> Vec<u8> {
    let mut tags = Vec::new();
    tags.extend_from_slice(b"OpusTags");
    tags.extend_from_slice(&4u32.to_le_bytes());
    tags.extend_from_slice(b"WADe");
    tags.extend_from_slice(&0u32.to_le_bytes());
    tags
}

/// 64 amplitude bars (1-100) for the bubble, like the phone's waveform.
pub fn waveform(samples: &[i16]) -> Vec<u8> {
    const BARS: usize = 64;
    if samples.is_empty() {
        return vec![1; BARS];
    }
    let bucket = (samples.len() / BARS).max(1);
    let peak = samples
        .iter()
        .map(|value| i32::from(*value).abs())
        .max()
        .unwrap_or(1)
        .max(1);
    (0..BARS)
        .map(|bar| {
            let start = bar * bucket;
            let end = (start + bucket).min(samples.len());
            if start >= samples.len() {
                return 1;
            }
            let local = samples[start..end]
                .iter()
                .map(|value| i32::from(*value).abs())
                .max()
                .unwrap_or(0);
            ((local * 100 / peak).clamp(1, 100)) as u8
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A second of a sine wave becomes an Ogg/Opus stream: two header packets
    /// then audio packets, which is what WhatsApp plays back.
    #[test]
    fn a_recording_becomes_ogg_opus() {
        let samples: Vec<i16> = (0..16_000)
            .map(|index| {
                let t = index as f32 / 16_000.0;
                ((t * 440.0 * std::f32::consts::TAU).sin() * 8_000.0) as i16
            })
            .collect();
        let note = encode(&samples).expect("encode");
        assert_eq!(note.seconds, 1);
        assert_eq!(note.waveform.len(), 64);
        assert!(note.waveform.iter().any(|bar| *bar > 50), "the sine peaks");

        // Parse it back: the first two packets are the Opus headers.
        let mut reader = ogg::PacketReader::new(std::io::Cursor::new(note.ogg.clone()));
        let first = reader.read_packet().expect("packet").expect("first");
        assert_eq!(&first.data[..8], b"OpusHead");
        let second = reader.read_packet().expect("packet").expect("second");
        assert_eq!(&second.data[..8], b"OpusTags");
        let mut audio = 0;
        while let Ok(Some(_)) = reader.read_packet() {
            audio += 1;
        }
        assert!(
            audio >= 40,
            "a second at 20 ms frames is ~50 packets, got {audio}"
        );
    }

    #[test]
    fn silence_keeps_a_flat_waveform() {
        let bars = waveform(&[0i16; 16_000]);
        assert_eq!(bars.len(), 64);
        assert!(bars.iter().all(|bar| *bar >= 1));
    }
}
