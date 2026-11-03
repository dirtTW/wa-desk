//! The chat scroll box: distance from the bottom, whether the "jump to latest"
//! disc shows, and where the viewport lands when a repaint must not move it.

use serde::{Deserialize, Serialize};

/// A reader this close to the bottom counts as "following" the chat.
pub const FOLLOW_THRESHOLD_PX: f64 = 120.0;
/// The disc hides again once the gap is this small (it would cover the last
/// bubble otherwise).
pub const LATEST_BUTTON_GAP_PX: f64 = 8.0;
/// The disc's resting distance above the composer stack.
pub const LATEST_BUTTON_BOTTOM_GAP_PX: f64 = 19.0;

/// The scroll thresholds, so the webview's synchronous scroll handler can read
/// them without awaiting per event.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct ScrollConstants {
    pub follow_threshold_px: f64,
    pub latest_button_gap_px: f64,
    pub latest_button_bottom_gap_px: f64,
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
        latest_button_bottom_gap_px: LATEST_BUTTON_BOTTOM_GAP_PX,
    }
}

/// Distance from the bottom of a scroll box.
pub fn distance_from_bottom(scroll_height: f64, scroll_top: f64, client_height: f64) -> f64 {
    scroll_height - scroll_top - client_height
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub struct ScrollEventPlan {
    /// The reader moved up since the previous event.
    pub moved_up: bool,
    /// `Some(false)` when the reader detached, `Some(true)` when they came back
    /// to the bottom, `None` when nothing changes.
    pub stick_bottom: Option<bool>,
    /// The unread dot on the disc goes away.
    pub clear_unread: bool,
}

/// One scroll event: moving up detaches the chat from the newest message;
/// reaching the bottom re-attaches it and clears the unread dot.
pub fn scroll_event(last_top: f64, top: f64, distance: f64) -> ScrollEventPlan {
    let moved_up = top < last_top - 1.0;
    let near_bottom = distance <= LATEST_BUTTON_GAP_PX;
    ScrollEventPlan {
        moved_up,
        stick_bottom: if moved_up {
            Some(false)
        } else if near_bottom {
            Some(true)
        } else {
            None
        },
        clear_unread: near_bottom,
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct AnchorNode {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub offset_top: f64,
    #[serde(default)]
    pub offset_height: f64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ScrollAnchor {
    pub id: String,
    /// The bubble's distance below the top of the viewport.
    pub offset: f64,
}

/// The first bubble still visible at the top of the box (1 px of slack so a
/// bubble flush with the edge still counts).
pub fn visible_anchor(nodes: &[AnchorNode], scroll_top: f64) -> Option<ScrollAnchor> {
    let top = scroll_top + 1.0;
    nodes
        .iter()
        .find(|node| node.offset_top + node.offset_height > top)
        .map(|node| ScrollAnchor {
            id: node.id.clone(),
            offset: node.offset_top - scroll_top,
        })
}

/// Where the "jump to latest" disc sits: above every visible card between the
/// messages and the composer, plus its resting gap.
pub fn latest_button_offset(visible_heights: &[f64], gap: f64) -> f64 {
    visible_heights.iter().sum::<f64>() + gap
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
    fn scrolling_up_detaches_and_the_bottom_re_attaches() {
        // Reading older messages: detach, no unread clear.
        let up = scroll_event(1000.0, 900.0, 400.0);
        assert!(up.moved_up);
        assert_eq!(up.stick_bottom, Some(false));
        assert!(!up.clear_unread);

        // Back at the bottom: re-attach and clear the dot.
        let back = scroll_event(900.0, 1000.0, 2.0);
        assert!(!back.moved_up);
        assert_eq!(back.stick_bottom, Some(true));
        assert!(back.clear_unread);

        // Scrolling down but still far from the bottom: nothing changes.
        let down = scroll_event(900.0, 950.0, 300.0);
        assert!(!down.moved_up);
        assert_eq!(down.stick_bottom, None);
        assert!(!down.clear_unread);

        // A 1 px jitter is not "moving up".
        assert!(!scroll_event(1000.0, 999.5, 400.0).moved_up);
    }

    #[test]
    fn the_visible_anchor_is_the_first_bubble_at_the_edge() {
        let nodes = vec![
            AnchorNode {
                id: "a".into(),
                offset_top: 0.0,
                offset_height: 100.0,
            },
            AnchorNode {
                id: "b".into(),
                offset_top: 100.0,
                offset_height: 80.0,
            },
            AnchorNode {
                id: "c".into(),
                offset_top: 180.0,
                offset_height: 80.0,
            },
        ];
        // Scrolled past the first bubble: the second one owns the top edge.
        let anchor = visible_anchor(&nodes, 120.0).unwrap();
        assert_eq!(anchor.id, "b");
        assert_eq!(anchor.offset, -20.0);
        // Flush with the top still counts.
        assert_eq!(visible_anchor(&nodes, 100.0).unwrap().id, "b");
        // At the very top the first bubble wins.
        assert_eq!(visible_anchor(&nodes, 0.0).unwrap().id, "a");
        // Past everything.
        assert!(visible_anchor(&nodes, 5000.0).is_none());
        assert!(visible_anchor(&[], 0.0).is_none());
    }

    #[test]
    fn the_disc_lifts_above_the_visible_cards() {
        assert_eq!(latest_button_offset(&[], LATEST_BUTTON_BOTTOM_GAP_PX), 19.0);
        assert_eq!(
            latest_button_offset(&[56.0, 40.0], LATEST_BUTTON_BOTTOM_GAP_PX),
            115.0
        );
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
