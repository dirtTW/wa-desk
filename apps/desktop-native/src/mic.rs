//! Microphone capture for voice/video calls: the device stream is downmixed to
//! mono and resampled to the 16 kHz the call protocol uses, delivered in 60 ms
//! frames (`wa_ui::calls::audio_format`).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;

/// The call's sample rate and frame length.
pub const RATE: u32 = 16_000;
pub const FRAME: usize = 960;

/// A running capture; dropping it stops the microphone.
pub struct Mic {
    stop: Arc<AtomicBool>,
}

impl Drop for Mic {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

impl Mic {
    /// Start capturing into `frames` (16 kHz mono i16, 960 samples each).
    pub fn start(frames: Sender<Vec<i16>>) -> Result<Self, String> {
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
        let worker_stop = stop.clone();

        std::thread::spawn(move || {
            let ratio = input_rate / RATE as f64;
            let mut pending: Vec<i16> = Vec::new();
            let mut carry = 0.0f64;
            let mut guard = crate::echo::EchoGuard::default();
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
                    while pending.len() >= FRAME {
                        let mut frame: Vec<i16> = pending.drain(..FRAME).collect();
                        // Duck while the peer talks, so the speaker does not
                        // feed back into our microphone (see `echo`).
                        let gain = guard.next(crate::audio::call_level());
                        if gain < 0.99 {
                            for sample in &mut frame {
                                *sample = (*sample as f32 * gain) as i16;
                            }
                        }
                        if frames.send(frame).is_err() {
                            return;
                        }
                    }
                },
                |err| crate::app::diag(&format!("mic: {err}")),
                None,
            );
            let Ok(stream) = stream else {
                return;
            };
            if stream.play().is_err() {
                return;
            }
            while !worker_stop.load(Ordering::Relaxed) {
                std::thread::sleep(std::time::Duration::from_millis(120));
            }
            drop(stream);
        });

        Ok(Self { stop })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_call_format_matches_wa_ui() {
        let format = wa_ui::calls::audio_format();
        assert_eq!(format.sample_rate, RATE);
        assert_eq!(format.frame_samples, FRAME);
    }
}
