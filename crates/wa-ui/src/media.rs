//! Media-hydration decisions: whether a rendered tile should be observed,
//! tapped, repainted or left alone, and whether a download may start.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Deserialize)]
pub struct HydrateInput {
    /// `loading`, `ready`, `error` or empty when nothing is cached yet.
    #[serde(default)]
    pub status: String,
    /// The tile is painted as `data-failed` and waits for an explicit tap.
    #[serde(default)]
    pub failed: bool,
    /// `image`, `video`, `audio`, `document` or `sticker`.
    #[serde(default)]
    pub kind: String,
    /// The open chat is a channel (its posts load without a tap).
    #[serde(default)]
    pub channel: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HydrateAction {
    /// Failed tiles wait for an explicit tap; never re-fetch on render.
    Skip,
    /// Cached and ready: the renderer paints it, so this DOM is stale.
    Repaint,
    /// Start the download as soon as the tile scrolls into view.
    Observe,
    /// Chat videos stay tap-to-play so a long clip never downloads by accident.
    TapToLoad,
    /// A failed tile: tap clears the error and retries.
    RetryTap,
}

/// What `hydrateMedia` does with one rendered tile.
pub fn hydrate_action(input: &HydrateInput) -> HydrateAction {
    if input.failed {
        return HydrateAction::Skip;
    }
    if input.status == "ready" {
        return HydrateAction::Repaint;
    }
    if input.status == "error" {
        return HydrateAction::RetryTap;
    }
    if input.kind == "video" && !input.channel {
        return HydrateAction::TapToLoad;
    }
    HydrateAction::Observe
}

/// One action per tile, in the order the tiles were sent.
pub fn hydrate_plan(nodes: &[HydrateInput]) -> Vec<HydrateAction> {
    nodes.iter().map(hydrate_action).collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DownloadAction {
    /// Already downloading or already downloaded.
    Skip,
    Start,
}

/// A download only starts when nothing is in flight or already cached.
pub fn download_action(status: &str) -> DownloadAction {
    if status == "loading" || status == "ready" {
        DownloadAction::Skip
    } else {
        DownloadAction::Start
    }
}

/// A download failure turned into something a person can act on.
pub fn friendly_error(text: &str) -> String {
    let lower = text.to_lowercase();
    let gone = ["403", "404", "410"]
        .iter()
        .any(|code| lower.contains(code));
    if gone {
        return "WhatsApp no longer serves this file. Ask the sender to resend it.".to_string();
    }
    if lower.contains("timed out") || lower.contains("too long") || lower.contains("timeout") {
        return "The download timed out. Check your connection and try again.".to_string();
    }
    text.to_string()
}

/// How many entries an LRU-style map must drop to get back under `max`.
pub fn cache_overflow(len: usize, max: usize) -> usize {
    len.saturating_sub(max)
}

/// The ids to evict from an insertion-ordered map, oldest first.
pub fn evictions(keys: &[String], max: usize) -> Vec<String> {
    let overflow = cache_overflow(keys.len(), max);
    keys.iter().take(overflow).cloned().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(status: &str, kind: &str) -> HydrateInput {
        HydrateInput {
            status: status.into(),
            kind: kind.into(),
            ..Default::default()
        }
    }

    #[test]
    fn cached_images_repaint_and_fresh_ones_observe() {
        assert_eq!(
            hydrate_action(&node("ready", "image")),
            HydrateAction::Repaint
        );
        assert_eq!(hydrate_action(&node("", "image")), HydrateAction::Observe);
        assert_eq!(
            hydrate_action(&node("loading", "image")),
            HydrateAction::Observe
        );
        assert_eq!(
            hydrate_action(&node("error", "image")),
            HydrateAction::RetryTap
        );
    }

    #[test]
    fn chat_videos_wait_for_a_tap_but_channels_autoload() {
        assert_eq!(hydrate_action(&node("", "video")), HydrateAction::TapToLoad);
        let channel = HydrateInput {
            status: String::new(),
            kind: "video".into(),
            channel: true,
            ..Default::default()
        };
        assert_eq!(hydrate_action(&channel), HydrateAction::Observe);
        // A cached channel video still repaints.
        let ready = HydrateInput {
            status: "ready".into(),
            kind: "video".into(),
            channel: true,
            ..Default::default()
        };
        assert_eq!(hydrate_action(&ready), HydrateAction::Repaint);
    }

    #[test]
    fn failed_tiles_are_never_refetched_on_render() {
        let failed = HydrateInput {
            failed: true,
            kind: "image".into(),
            ..Default::default()
        };
        assert_eq!(hydrate_action(&failed), HydrateAction::Skip);
        // Even a "ready" tile marked failed is left alone until it is tapped.
        let ready_failed = HydrateInput {
            failed: true,
            status: "ready".into(),
            kind: "image".into(),
            ..Default::default()
        };
        assert_eq!(hydrate_action(&ready_failed), HydrateAction::Skip);
    }

    #[test]
    fn the_plan_keeps_the_order() {
        let plan = hydrate_plan(&[
            node("", "image"),
            node("", "video"),
            node("ready", "image"),
            node("error", "image"),
        ]);
        assert_eq!(
            plan,
            vec![
                HydrateAction::Observe,
                HydrateAction::TapToLoad,
                HydrateAction::Repaint,
                HydrateAction::RetryTap
            ]
        );
    }

    #[test]
    fn downloads_skip_in_flight_and_cached_media() {
        assert_eq!(download_action("loading"), DownloadAction::Skip);
        assert_eq!(download_action("ready"), DownloadAction::Skip);
        assert_eq!(download_action("error"), DownloadAction::Start);
        assert_eq!(download_action(""), DownloadAction::Start);
    }

    #[test]
    fn download_errors_are_explained() {
        assert!(friendly_error("HTTP 404").contains("no longer serves"));
        assert!(friendly_error("status 410").contains("no longer serves"));
        assert!(friendly_error("the download timed out").contains("timed out"));
        assert!(friendly_error("taking too long").contains("timed out"));
        assert_eq!(friendly_error("boom"), "boom");
    }

    #[test]
    fn cache_eviction_counts_the_overflow() {
        assert_eq!(cache_overflow(250, 240), 10);
        assert_eq!(cache_overflow(240, 240), 0);
        assert_eq!(cache_overflow(10, 240), 0);

        let keys: Vec<String> = (0..5).map(|i| format!("k{i}")).collect();
        assert_eq!(
            evictions(&keys, 3),
            vec!["k0".to_string(), "k1".to_string()],
            "oldest insertions go first"
        );
        assert!(evictions(&keys, 5).is_empty());
        assert!(evictions(&[], 10).is_empty());
    }
}
