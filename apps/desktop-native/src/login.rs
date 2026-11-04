//! The login gate: QR code, pairing code, and the connection line.
//!
//! `wa-core` hands the UI an already-rendered QR **SVG**, so this module only
//! has to rasterise it for `egui` — no QR library, and no WebView to draw it.

/// What the window shows before the account is linked.
#[derive(Debug, Default, Clone, PartialEq)]
pub enum LoginState {
    /// Waiting for the first QR code.
    #[default]
    Waiting,
    /// A scannable code, kept as the raw SVG `wa-core` emitted.
    Qr(String),
    /// The rotation budget is spent: offer a restart.
    Expired,
    /// The phone asked for a pairing code instead of a scan.
    PairCode(String),
    /// Linked and syncing.
    Linked,
}

impl LoginState {
    pub fn is_linked(&self) -> bool {
        matches!(self, LoginState::Linked)
    }

    /// The line under the title (the tests pin the wording).
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn hint(&self) -> &'static str {
        match self {
            LoginState::Waiting => "Connecting…",
            LoginState::Qr(_) => "Scan this code with WhatsApp → Linked devices",
            LoginState::Expired => "The code expired — request a new one",
            LoginState::PairCode(_) => "Enter this code on your phone",
            LoginState::Linked => "",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum LoginEvent {
    Qr(String),
    Expired,
    PairCode(String),
    Paired,
    Connected,
    LoggedOut,
}

/// Fold one event into the gate state.
pub fn fold(_state: &LoginState, event: &LoginEvent) -> LoginState {
    match event {
        LoginEvent::Qr(svg) => LoginState::Qr(svg.clone()),
        LoginEvent::Expired => LoginState::Expired,
        LoginEvent::PairCode(code) => LoginState::PairCode(code.clone()),
        // Linking, then the transport coming up, both leave the gate.
        LoginEvent::Paired | LoginEvent::Connected => LoginState::Linked,
        LoginEvent::LoggedOut => LoginState::Waiting,
    }
}

/// Whether the gate should offer the "new code" action.
pub fn can_restart(state: &LoginState) -> bool {
    matches!(state, LoginState::Expired | LoginState::Waiting)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_qr_replaces_the_waiting_state() {
        let state = fold(&LoginState::Waiting, &LoginEvent::Qr("<svg/>".into()));
        assert_eq!(state, LoginState::Qr("<svg/>".into()));
        assert!(!state.is_linked());
        assert!(state.hint().contains("Scan"));
        assert!(!can_restart(&state));
    }

    #[test]
    fn an_expired_code_can_be_restarted() {
        let state = fold(&LoginState::Qr("<svg/>".into()), &LoginEvent::Expired);
        assert_eq!(state, LoginState::Expired);
        assert!(can_restart(&state));
        assert!(state.hint().contains("expired"));
    }

    #[test]
    fn linking_or_connecting_leaves_the_gate() {
        assert!(fold(&LoginState::Qr("x".into()), &LoginEvent::Paired).is_linked());
        assert!(fold(&LoginState::Waiting, &LoginEvent::Connected).is_linked());
        // A pairing code is its own screen, then linking leaves it too.
        let code = fold(
            &LoginState::Waiting,
            &LoginEvent::PairCode("ABCD-1234".into()),
        );
        assert_eq!(code, LoginState::PairCode("ABCD-1234".into()));
        assert!(code.hint().contains("Enter this code"));
        assert!(fold(&code, &LoginEvent::Paired).is_linked());
    }

    #[test]
    fn logging_out_returns_to_the_gate() {
        let state = fold(&LoginState::Linked, &LoginEvent::LoggedOut);
        assert_eq!(state, LoginState::Waiting);
        assert!(can_restart(&state), "the user needs a way back in");
    }
}
