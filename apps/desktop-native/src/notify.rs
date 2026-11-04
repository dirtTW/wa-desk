//! Desktop notifications: WinRT toasts on Windows, the session bus elsewhere.
//! The app calls `send` for incoming messages while the window is unfocused (the
//! same rule the official client uses), and for `--test-notification`.

/// Show a toast. Returns an error string when the platform refused it, so the
/// settings screen can say so instead of failing silently.
pub fn send(title: &str, body: &str) -> Result<(), String> {
    if title.trim().is_empty() {
        return Err("empty notification title".to_string());
    }
    platform_send(title, body)
}

#[cfg(windows)]
fn platform_send(title: &str, body: &str) -> Result<(), String> {
    use tauri_winrt_notification::{Duration, Toast};
    Toast::new(APP_ID)
        .title(title)
        .text1(body)
        .duration(Duration::Short)
        .sound(None)
        .show()
        .map_err(|err| format!("toast: {err}"))
}

#[cfg(not(windows))]
fn platform_send(title: &str, body: &str) -> Result<(), String> {
    notify_rust::Notification::new()
        .appname("WA Desk")
        .summary(title)
        .body(body)
        .icon("io.github.dirttw.wadesk")
        .timeout(notify_rust::Timeout::Milliseconds(6_000))
        .show()
        .map(|_| ())
        .map_err(|err| format!("notify: {err}"))
}

/// The Windows AppUserModelID: keeps every toast grouped under our own name.
#[cfg(windows)]
pub const APP_ID: &str = "io.github.dirttw.wadesk";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_title_is_rejected_before_hitting_the_platform() {
        assert!(send("  ", "body").is_err());
    }

    #[cfg(windows)]
    #[test]
    fn the_app_id_is_set() {
        assert!(APP_ID.contains("wadesk"));
    }
}
