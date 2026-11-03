//! The chat scroll box: distance from the bottom, whether the "jump to latest"
//! disc shows, and where the viewport lands when a repaint must not move it.

/// A reader this close to the bottom counts as "following" the chat.
pub const FOLLOW_THRESHOLD_PX: f64 = 120.0;
/// The disc hides again once the gap is this small (it would cover the last
/// bubble otherwise).
pub const LATEST_BUTTON_GAP_PX: f64 = 8.0;

use serde::Serialize;

/// The scroll thresholds, so the webview's synchronous scroll handler can read
/// them without awaiting per event.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct ScrollConstants {
    pub follow_threshold_px: f64,
    pub latest_button_gap_px: f64,
}

/// Whether a repaint should stick to the newest message.
pub fn is_following(distance: f64) -> bool {
    distance <= FOLLOW_THRESHOLD_PX
}

/// Whether the floating "jump to latest" disc is worth showing.
pub fn show_latest_button(child_count: usize, distance: f64) -> bool {
    child_count > 0 && distance > LATEST_BUTTON_GAP_PX
}

/// The gap used by the "jump to latest" disc, mirrored into the webview so its
/// synchronous scroll handler never awaits.
pub fn constants() -> ScrollConstants {
    ScrollConstants {
        follow_threshold_px: FOLLOW_THRESHOLD_PX,
        latest_button_gap_px: LATEST_BUTTON_GAP_PX,
    }
}

/// Distance from the bottom of a scroll box.
pub fn distance_from_bottom(scroll_height: f64, scroll_top: f64, client_height: f64) -> f64 {
    scroll_height - scroll_top - client_height
}

/// Where to scroll so the anchored bubble keeps its on-screen offset after a
/// repaint (never negative).
pub fn anchor_scroll_top(node_offset: f64, anchor_offset: f64, fallback: f64) -> f64 {
    if node_offset.is_finite() && anchor_offset.is_finite() {
        (node_offset - anchor_offset).max(0.0)
    } else {
        fallback
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distance_and_follow_threshold() {
        assert_eq!(distance_from_bottom(2000.0, 1400.0, 600.0), 0.0);
        assert!(is_following(0.0));
        assert!(is_following(120.0));
        assert!(!is_following(120.5));
        assert!(!is_following(600.0));
    }

    #[test]
    fn the_latest_disc_needs_content_and_a_gap() {
        assert!(!show_latest_button(0, 500.0), "an empty chat shows nothing");
        assert!(!show_latest_button(3, 8.0), "the gap is too small");
        assert!(show_latest_button(3, 8.1));
        assert!(show_latest_button(1, 400.0));
    }

    #[test]
    fn anchoring_keeps_the_bubble_offset() {
        // The bubble sat 40 px below the viewport top and lives at 900 px.
        assert_eq!(anchor_scroll_top(900.0, 40.0, 0.0), 860.0);
        // Never scroll above the top.
        assert_eq!(anchor_scroll_top(20.0, 40.0, 0.0), 0.0);
        // Missing metrics fall back to the previous scroll position.
        assert_eq!(anchor_scroll_top(f64::NAN, 0.0, 123.0), 123.0);
        assert_eq!(anchor_scroll_top(0.0, f64::INFINITY, 55.0), 55.0);
    }
}
