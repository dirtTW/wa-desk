//! Half-duplex echo suppression for calls. Full AEC needs an adaptive filter
//! (WebRTC's AEC); what this does instead is duck the microphone while the
//! peer's audio is loud, which stops the speaker -> mic feedback loop that
//! otherwise howls on laptop speakers. The gain is computed per 60 ms frame so
//! the transitions are smooth.

/// RMS below which the peer counts as silent.
pub const QUIET_RMS: f32 = 0.02;
/// RMS at which the microphone is ducked the most.
pub const LOUD_RMS: f32 = 0.08;
/// The floor: -12 dB, enough to break the loop without muting us.
pub const MIN_GAIN: f32 = 0.25;

/// The gain the microphone should move towards, given the peer's level.
pub fn target_gain(remote_rms: f32) -> f32 {
    if remote_rms <= QUIET_RMS {
        return 1.0;
    }
    if remote_rms >= LOUD_RMS {
        return MIN_GAIN;
    }
    let span = LOUD_RMS - QUIET_RMS;
    1.0 - (remote_rms - QUIET_RMS) / span * (1.0 - MIN_GAIN)
}

/// The smoothed microphone gain, frame by frame.
#[derive(Debug, Clone)]
pub struct EchoGuard {
    gain: f32,
}

impl Default for EchoGuard {
    fn default() -> Self {
        Self { gain: 1.0 }
    }
}

impl EchoGuard {
    /// Fold one frame and return the gain to apply to it.
    pub fn next(&mut self, remote_rms: f32) -> f32 {
        let target = target_gain(remote_rms);
        // Duck quickly (about one frame) and release slowly (about 300 ms), so
        // the end of a word does not chop the start of ours.
        let rate = if target < self.gain { 0.7 } else { 0.2 };
        self.gain += (target - self.gain) * rate;
        self.gain = self.gain.clamp(MIN_GAIN, 1.0);
        self.gain
    }

    #[cfg(test)]
    pub fn gain(&self) -> f32 {
        self.gain
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silence_keeps_the_microphone_open() {
        assert_eq!(target_gain(0.0), 1.0);
        assert_eq!(target_gain(QUIET_RMS), 1.0);
        let mut guard = EchoGuard::default();
        for _ in 0..20 {
            guard.next(0.0);
        }
        assert!((guard.gain() - 1.0).abs() < 0.01);
    }

    #[test]
    fn loud_playout_ducks_the_microphone() {
        assert_eq!(target_gain(LOUD_RMS), MIN_GAIN);
        let mut guard = EchoGuard::default();
        let mut gain = 1.0;
        for _ in 0..10 {
            gain = guard.next(0.2);
        }
        assert!(gain <= 0.3, "gain {gain} should be ducked");
        assert!(gain >= MIN_GAIN);
    }

    #[test]
    fn the_level_ramps_between_the_thresholds() {
        let mid = target_gain((QUIET_RMS + LOUD_RMS) / 2.0);
        assert!(mid < 1.0 && mid > MIN_GAIN, "mid gain {mid}");
        assert!(target_gain(0.03) > target_gain(0.07));
    }

    #[test]
    fn the_gain_releases_when_the_peer_goes_quiet() {
        let mut guard = EchoGuard::default();
        for _ in 0..10 {
            guard.next(0.2);
        }
        let ducked = guard.gain();
        assert!(ducked < 0.4);
        for _ in 0..20 {
            guard.next(0.0);
        }
        assert!(
            guard.gain() > ducked + 0.3,
            "should recover: {}",
            guard.gain()
        );
        assert!(guard.gain() <= 1.0);
    }
}
