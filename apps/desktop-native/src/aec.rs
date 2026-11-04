//! Adaptive echo cancellation (NLMS) for calls.
//!
//! The loudspeaker feeds the microphone: what we play comes back, delayed and
//! attenuated. A normalised least-mean-squares filter models that path from the
//! playout reference and subtracts the prediction from the microphone signal.
//! It runs before the `echo::EchoGuard` ducking, which stays as the backstop for
//! the acoustic delay the filter cannot model.

/// Samples kept in the reference ring (256 ms at 16 kHz): more than the taps.
pub const RING: usize = 4096;

/// How far back the reference is taken, in samples (120 ms): speaker + buffer
/// latency, the part of the echo path the filter taps then refine.
pub const DELAY: usize = 1_920;

#[derive(Debug, Clone)]
pub struct Aec {
    weights: Vec<f32>,
    history: Vec<f32>,
    pos: usize,
    step: f32,
    power: f32,
}

impl Aec {
    pub fn new(taps: usize) -> Self {
        let taps = taps.max(16);
        Self {
            weights: vec![0.0; taps],
            history: vec![0.0; taps],
            pos: 0,
            step: 0.5,
            power: 1e-6,
        }
    }

    /// Cancel one sample: `mic` is what the microphone heard, `reference` what
    /// we played. Returns the cleaned signal.
    pub fn process(&mut self, mic: f32, reference: f32) -> f32 {
        let taps = self.weights.len();
        self.power = self.power * 0.999 + reference * reference;
        self.history[self.pos] = reference;

        // y = w · x, newest sample first.
        let mut estimate = 0.0;
        for (index, weight) in self.weights.iter().enumerate() {
            let sample = self.history[(self.pos + taps - index) % taps];
            estimate += weight * sample;
        }
        let error = mic - estimate;
        // NLMS update, normalised by the reference power.
        let gain = self.step * error / (self.power + 1e-6);
        for (index, weight) in self.weights.iter_mut().enumerate() {
            let sample = self.history[(self.pos + taps - index) % taps];
            *weight += gain * sample;
        }
        self.pos = (self.pos + 1) % taps;
        error
    }
}

/// The microphone-side history the filter needs, kept in a ring so the audio
/// thread can publish playout samples and the capture thread can read them.
#[derive(Debug, Default)]
pub struct Reference {
    samples: Vec<f32>,
    position: usize,
    filled: usize,
}

impl Reference {
    pub fn push(&mut self, sample: f32) {
        if self.samples.len() < RING {
            self.samples.push(sample);
            self.position = (self.position + 1) % RING;
            self.filled = self.samples.len();
            return;
        }
        self.samples[self.position] = sample;
        self.position = (self.position + 1) % RING;
        self.filled = RING;
    }

    /// The last `len` samples, `delay` samples behind the newest one.
    pub fn take(&self, len: usize, delay: usize) -> Vec<f32> {
        let mut out = vec![0.0; len];
        if self.filled == 0 {
            return out;
        }
        let count = self.filled.min(RING);
        for (index, slot) in out.iter_mut().enumerate() {
            // +1: the newest sample sits at `position - 1`.
            let back = delay + len - index;
            if back == 0 || back > count {
                continue;
            }
            let at = (self.position + RING * 2 - back) % RING.max(1);
            *slot = self.samples.get(at).copied().unwrap_or(0.0);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn noise(seed: &mut u32) -> f32 {
        // A cheap deterministic generator, no rand dependency in the tests.
        *seed ^= *seed << 13;
        *seed ^= *seed >> 17;
        *seed ^= *seed << 5;
        (*seed as f32 / u32::MAX as f32) * 2.0 - 1.0
    }

    #[test]
    fn the_filter_cancels_a_delayed_echo() {
        let taps = 2048;
        let delay = 200;
        let mut aec = Aec::new(taps);
        let mut seed = 0x1234_5678u32;
        let mut reference = vec![0.0f32; 16_000 * 4];
        for sample in reference.iter_mut() {
            *sample = noise(&mut seed) * 0.3;
        }
        let mut mic_power = 0.0f64;
        let mut out_power = 0.0f64;
        // Adapt for 2 s, then measure over the next second.
        for (index, played) in reference.iter().enumerate() {
            let echo = if index >= delay {
                reference[index - delay] * 0.6
            } else {
                0.0
            };
            let cleaned = aec.process(echo, *played);
            if (32_000..48_000).contains(&index) {
                mic_power += (echo * echo) as f64;
                out_power += (cleaned * cleaned) as f64;
            }
        }
        let erle = 10.0 * (mic_power / out_power.max(1e-12)).log10();
        eprintln!("aec erle: {erle:.1} dB");
        assert!(
            erle > 10.0,
            "echo return loss enhancement was only {erle:.1} dB"
        );
    }

    #[test]
    fn silence_passes_through() {
        let mut aec = Aec::new(256);
        for _ in 0..1_000 {
            let out = aec.process(0.0, 0.0);
            assert_eq!(out, 0.0);
        }
    }

    #[test]
    fn the_reference_ring_returns_the_delayed_tail() {
        let mut reference = Reference::default();
        for index in 0..10_000u32 {
            reference.push(index as f32);
        }
        // The newest sample is 9999; ask for the four samples 3 back.
        let tail = reference.take(4, 3);
        assert_eq!(tail, vec![9993.0, 9994.0, 9995.0, 9996.0]);
        // Asking for more history than the ring holds is padded, not a panic.
        let long = reference.take(RING + 10, 0);
        assert_eq!(long.len(), RING + 10);
        assert_eq!(*long.last().unwrap(), 9999.0);
    }

    #[test]
    fn an_empty_ring_is_silent() {
        let reference = Reference::default();
        assert!(reference.take(8, 0).iter().all(|sample| *sample == 0.0));
    }
}
