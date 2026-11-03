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
