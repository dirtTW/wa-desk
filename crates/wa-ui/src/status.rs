//! Status/stories markup: the status rail list, the story viewer shell, the
//! media body and the "who viewed" list.
//!
//! The webview keeps the timers, the media download and the event wiring; the
//! markup and the list/expiry rules live here.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::html::{avatar, escape, icon};
use crate::labels::{clock_label, day_of};

/// Stories expire after 24 hours.
pub const STATUS_TTL_MS: i64 = 24 * 60 * 60 * 1000;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ViewerRow {
    #[serde(default)]
    pub jid: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub timestamp: i64,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct StatusUpdateView {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub sender: String,
    #[serde(default)]
    pub name: String,
    /// `text`, `image` or `video`.
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub background: String,
    #[serde(default)]
    pub timestamp: i64,
    #[serde(default)]
    pub viewed: bool,
    #[serde(default)]
    pub from_me: bool,
    #[serde(default)]
    pub viewers: Vec<ViewerRow>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct StatusEntryView {
    #[serde(default)]
    pub jid: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub updates: Vec<StatusUpdateView>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StatusRequest {
    #[serde(default)]
    pub entries: Vec<StatusEntryView>,
    #[serde(default)]
    pub photos: HashMap<String, String>,
    /// Locally cached story media by update id (data or asset URL).
    #[serde(default)]
    pub local_media: HashMap<String, String>,
    #[serde(default)]
    pub me: String,
    #[serde(default)]
    pub me_jid: String,
    #[serde(default)]
    pub me_lid: String,
    #[serde(default)]
    pub now: i64,
}

#[derive(Debug, Serialize)]
pub struct StatusListOutput {
    pub recent_html: String,
    pub viewed_html: String,
    pub unseen: usize,
    /// JIDs whose profile photo is worth fetching.
    pub queue: Vec<String>,
    pub mine_has_thumb: bool,
    pub mine_label: String,
    pub mine_thumb_html: String,
    pub mine_thumb_style: String,
    pub mine_thumb_has_image: bool,
}

fn is_mine(entry: &StatusEntryView, req: &StatusRequest) -> bool {
    entry.updates.iter().any(|update| update.from_me)
        || entry.jid == req.me_jid
        || entry.jid == req.me_lid
}

fn latest(entry: &StatusEntryView) -> i64 {
    entry
        .updates
        .iter()
        .map(|update| update.timestamp)
        .max()
        .unwrap_or(0)
}

/// Entries that still have a live story, newest first.
fn live_entries(req: &StatusRequest) -> Vec<StatusEntryView> {
    let mut entries: Vec<StatusEntryView> = req
        .entries
        .iter()
        .map(|entry| StatusEntryView {
            jid: entry.jid.clone(),
            name: entry.name.clone(),
            updates: entry
                .updates
                .iter()
                .filter(|update| req.now - update.timestamp < STATUS_TTL_MS)
                .cloned()
                .collect(),
        })
        .filter(|entry| !entry.updates.is_empty())
        .collect();
    entries.sort_by_key(|entry| std::cmp::Reverse(latest(entry)));
    entries
}

fn item_html(entry: &StatusEntryView, req: &StatusRequest) -> String {
    let created = latest(entry);
    let viewed = entry.updates.iter().all(|update| update.viewed);
    let count = entry.updates.len();
    let suffix = if count > 1 {
        format!(" \u{2022} {count} updates")
    } else {
        String::new()
    };
    format!(
        r#"
    <div class="status-item{}" data-status-jid="{}">
      <div class="status-ring{}">{}</div>
      <div class="status-meta">
        <strong>{}</strong>
        <span>{} at {}{suffix}</span>
      </div>
    </div>"#,
        if viewed { " viewed" } else { "" },
        escape(&entry.jid),
        if viewed { " viewed" } else { "" },
        avatar(&entry.name, &entry.jid, "avatar", &req.photos),
        escape(&entry.name),
        escape(&day_of(created, req.now)),
        escape(&clock_label(created)),
    )
}

fn valid_colour(value: &str) -> Option<&str> {
    let bytes = value.as_bytes();
    if bytes.len() == 7 && bytes[0] == b'#' && bytes[1..].iter().all(|b| b.is_ascii_hexdigit()) {
        Some(value)
    } else {
        None
    }
}

/// The rail list: my own row plus the recent/viewed sections.
pub fn list(req: &StatusRequest) -> StatusListOutput {
    let entries = live_entries(req);
    let mut mine_updates: Vec<&StatusUpdateView> = entries
        .iter()
        .filter(|entry| is_mine(entry, req))
        .flat_map(|entry| entry.updates.iter())
        .collect();
    mine_updates.sort_by_key(|update| std::cmp::Reverse(update.timestamp));
    let latest_mine = mine_updates.first().copied();

    let others: Vec<&StatusEntryView> = entries
        .iter()
        .filter(|entry| !is_mine(entry, req))
        .collect();
    let recent: Vec<&&StatusEntryView> = others
        .iter()
        .filter(|entry| entry.updates.iter().any(|update| !update.viewed))
        .collect();
    let viewed: Vec<&&StatusEntryView> = others
        .iter()
        .filter(|entry| entry.updates.iter().all(|update| update.viewed))
        .collect();

    let recent_html = if recent.is_empty() {
        r#"<p class="empty">No recent status updates.</p>"#.to_string()
    } else {
        recent
            .iter()
            .map(|entry| item_html(entry, req))
            .collect::<String>()
    };
    let viewed_html = if viewed.is_empty() {
        r#"<p class="empty">No viewed status updates.</p>"#.to_string()
    } else {
        viewed
            .iter()
            .map(|entry| item_html(entry, req))
            .collect::<String>()
    };

    let mut queue: Vec<String> = entries.iter().map(|entry| entry.jid.clone()).collect();

    let (mine_label, thumb_html, thumb_style, thumb_has_image) = match latest_mine {
        None => (
            "Click to add status update".to_string(),
            String::new(),
            String::new(),
            false,
        ),
        Some(update) => {
            let label = format!(
                "{} at {}",
                day_of(update.timestamp, req.now),
                clock_label(update.timestamp)
            );
            if update.kind == "text" {
                let colour = valid_colour(&update.background).unwrap_or("#243640");
                let text: String = update.text.chars().take(48).collect();
                (label, escape(&text), format!("background: {colour}"), false)
            } else {
                let url = req.local_media.get(&update.id).cloned().unwrap_or_default();
                let has_image = !url.is_empty();
                let style = if has_image {
                    format!("background: center / cover no-repeat url(\"{url}\")")
                } else {
                    "background: var(--panel-3)".to_string()
                };
                let html = if update.kind == "video" {
                    icon("play_arrow")
                } else {
                    String::new()
                };
                (label, html, style, has_image)
            }
        }
    };

    // Keep the request's own order for photo requests; dedupe.
    let mut seen = std::collections::HashSet::new();
    queue.retain(|jid| seen.insert(jid.clone()));

    StatusListOutput {
        recent_html,
        viewed_html,
        unseen: recent.len(),
        queue,
        mine_has_thumb: latest_mine.is_some(),
        mine_label,
        mine_thumb_html: thumb_html,
        mine_thumb_style: thumb_style,
        mine_thumb_has_image: thumb_has_image,
    }
}

/* ------------------------------------------------------------------- viewer */

#[derive(Debug, Clone, Deserialize)]
pub struct StatusFrameRequest {
    /// The flattened viewer queue (every group's live updates, oldest first).
    #[serde(default)]
    pub queue: Vec<StatusUpdateView>,
    #[serde(default)]
    pub index: usize,
    #[serde(default)]
    pub group_name: String,
    #[serde(default)]
    pub group_jid: String,
    #[serde(default)]
    pub photos: HashMap<String, String>,
    #[serde(default)]
    pub local_media: HashMap<String, String>,
    #[serde(default)]
    pub now: i64,
}

#[derive(Debug, Serialize)]
pub struct StatusFrameOutput {
    pub progress_html: String,
    pub name: String,
    pub time_label: String,
    pub avatar_name: String,
    pub avatar_jid: String,
    /// `Some(colour)` paints the whole stage (text stories).
    pub background: Option<String>,
    pub body_html: String,
    pub kind: String,
    pub from_me: bool,
    pub viewers: usize,
    /// The media still has to be fetched before the body can be painted.
    pub needs_media: bool,
}

/// One progress segment per update; everything before the current one is done.
pub fn progress_html(count: usize, index: usize) -> String {
    (0..count)
        .map(|i| {
            format!(
                r#"<span class="{}"><i></i></span>"#,
                if i < index { "done" } else { "" }
            )
        })
        .collect()
}

/// The viewer shell for one frame.
pub fn frame(req: &StatusFrameRequest) -> Option<StatusFrameOutput> {
    let update = req.queue.get(req.index)?;
    let name = if update.from_me {
        "You".to_string()
    } else if !req.group_name.is_empty() {
        req.group_name.clone()
    } else if !update.name.is_empty() {
        update.name.clone()
    } else {
        "Status".to_string()
    };
    let avatar_name = if !req.group_name.is_empty() {
        req.group_name.clone()
    } else if !update.name.is_empty() {
        update.name.clone()
    } else {
        "Status".to_string()
    };
    let avatar_jid = if !req.group_jid.is_empty() {
        req.group_jid.clone()
    } else {
        update.sender.clone()
    };
    let time_label = format!(
        "{} at {}",
        day_of(update.timestamp, req.now),
        clock_label(update.timestamp)
    );

    let (background, body_html, needs_media) = if update.kind == "text" {
        (
            Some(
                valid_colour(&update.background)
                    .unwrap_or("#243640")
                    .to_string(),
            ),
            format!(r#"<div class="text-status">{}</div>"#, escape(&update.text)),
            false,
        )
    } else if let Some(url) = req
        .local_media
        .get(&update.id)
        .filter(|url| !url.is_empty())
    {
        (None, media_html(url, &update.kind, &update.name), false)
    } else {
        (
            None,
            r#"<div class="media-box">Loading status...</div>"#.to_string(),
            true,
        )
    };

    Some(StatusFrameOutput {
        progress_html: progress_html(req.queue.len(), req.index),
        name,
        time_label,
        avatar_name,
        avatar_jid,
        background,
        body_html,
        kind: update.kind.clone(),
        from_me: update.from_me,
        viewers: update.viewers.len(),
        needs_media,
    })
}

/// The story body once the media URL is known (or from the local cache).
pub fn media_html(url: &str, kind: &str, name: &str) -> String {
    if kind == "video" {
        format!(
            r#"<video src="{}" autoplay playsinline></video>"#,
            escape(url)
        )
    } else {
        format!(r#"<img src="{}" alt="{}" />"#, escape(url), escape(name))
    }
}

/// The body shown when the server no longer has the story media.
pub fn media_error_html() -> String {
    r#"<div class="text-status">Status media is no longer available.</div>"#.to_string()
}

/* -------------------------------------------------------------------- views */

#[derive(Debug, Serialize)]
pub struct StatusViewsOutput {
    pub title: String,
    pub html: String,
    pub queue: Vec<String>,
}

/// Who viewed my story, newest first.
pub fn views(viewers: &[ViewerRow], req: &StatusRequest) -> StatusViewsOutput {
    let mut viewers: Vec<&ViewerRow> = viewers.iter().collect();
    viewers.sort_by_key(|viewer| std::cmp::Reverse(viewer.timestamp));
    let title = format!("Viewed by {}", viewers.len());
    let html = if viewers.is_empty() {
        r#"<p class="empty">No views recorded yet. WhatsApp only reports views that arrive while WA Desk is running.</p>"#.to_string()
    } else {
        viewers
            .iter()
            .map(|viewer| {
                format!(
                    r#"
        <div class="status-view-row">
          {}
          <div class="status-view-meta">
            <strong>{}</strong>
            <span>{} at {}</span>
          </div>
        </div>"#,
                    avatar(&viewer.name, &viewer.jid, "avatar", &req.photos),
                    escape(&viewer.name),
                    escape(&day_of(viewer.timestamp, req.now)),
                    escape(&clock_label(viewer.timestamp)),
                )
            })
            .collect()
    };
    StatusViewsOutput {
        title,
        html,
        queue: viewers
            .iter()
            .take(20)
            .map(|viewer| viewer.jid.clone())
            .collect(),
    }
}

/// Text stories get 10 s (plus a little for long ones); photos 8 s.
pub const STATUS_TEXT_MS: i64 = 10_000;
pub const STATUS_IMAGE_MS: i64 = 8_000;
/// A video whose metadata has not arrived gets this placeholder duration.
pub const STATUS_VIDEO_FALLBACK_MS: i64 = 20_000;
/// The server no longer has the media — still show a moving bar.
pub const STATUS_MEDIA_FAILED_MS: i64 = 3_000;
/// Thumbnail/poster capture limits.
pub const STATUS_THUMB_MAX_PX: u32 = 320;
pub const STATUS_POSTER_TIMEOUT_MS: i64 = 4_000;

/// How long one story frame stays up. A video returns 0 until its real
/// duration is known; long text stories get up to 4 s extra.
pub fn frame_duration(kind: &str, text: &str) -> i64 {
    match kind {
        "video" => 0,
        "text" => {
            let len = text.chars().count() as i64;
            let extra = ((len - 80).max(0) * 40).min(4_000);
            STATUS_TEXT_MS + extra
        }
        _ => STATUS_IMAGE_MS,
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct FrameAnchor {
    pub total: i64,
    pub remaining: i64,
    /// `performance.now()` coordinate when the frame ends.
    pub ends_at: f64,
}

/// Anchor a frame: total duration, remaining time and the end instant.
pub fn anchor(total: i64, remaining: i64, now: f64) -> FrameAnchor {
    let total = total.max(1);
    let remaining = remaining.max(0);
    FrameAnchor {
        total,
        remaining,
        ends_at: now + remaining as f64,
    }
}

/// How far into the frame we are, from the same clock the advance timer uses.
pub fn elapsed(total: i64, remaining: i64, ends_at: f64, now: f64) -> i64 {
    let left = if ends_at > 0.0 {
        (ends_at - now).max(0.0)
    } else {
        remaining as f64
    };
    (total as f64 - left).max(0.0).round() as i64
}

/// Freezing the clock freezes the bar: keep the remaining time, drop the end.
pub fn pause_remaining(remaining: i64, ends_at: f64, now: f64) -> i64 {
    if ends_at > 0.0 {
        (ends_at - now).max(0.0).round() as i64
    } else {
        remaining.max(0)
    }
}

/// Resuming re-anchors with at least one millisecond left.
pub fn resume_totals(total: i64, remaining: i64) -> (i64, i64) {
    let remaining = remaining.max(1).max(total.min(remaining.max(1)));
    let remaining = remaining.max(1);
    (total.max(remaining), remaining)
}

/// The bar fill (0..1) for a frame.
pub fn progress_ratio(total: i64, remaining: i64) -> f64 {
    if total <= 0 {
        return 0.0;
    }
    ((total - remaining) as f64 / total as f64).clamp(0.0, 1.0)
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct ThumbSize {
    pub width: u32,
    pub height: u32,
}

/// Fit a picture into the capture box, never enlarging it.
pub fn thumb_scale(width: u32, height: u32, max: u32) -> ThumbSize {
    if width == 0 || height == 0 {
        return ThumbSize {
            width: 1,
            height: 1,
        };
    }
    let scale = (max as f64 / width as f64)
        .min(max as f64 / height as f64)
        .min(1.0);
    ThumbSize {
        width: ((width as f64 * scale).round() as u32).max(1),
        height: ((height as f64 * scale).round() as u32).max(1),
    }
}

/// Where to seek for a video poster (a frame just after the start).
pub fn poster_seek_seconds(duration: f64) -> f64 {
    let duration = if duration.is_finite() && duration > 0.0 {
        duration
    } else {
        1.0
    };
    (duration / 2.0).min(0.2)
}

/// The status composer's capture limits (thumbnails and video posters).
#[derive(Debug, Clone, Copy, Serialize)]
pub struct CaptureLimits {
    pub thumb_max_px: u32,
    pub jpeg_quality: f64,
}

pub fn capture_limits() -> CaptureLimits {
    CaptureLimits {
        thumb_max_px: STATUS_THUMB_MAX_PX,
        jpeg_quality: 0.72,
    }
}

/// The duration a posted video story reports: at least one second, rounded.
pub fn video_duration_seconds(duration: f64) -> i64 {
    let value = if duration.is_finite() && duration > 0.0 {
        duration
    } else {
        1.0
    };
    value.round().max(1.0) as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_limits_and_video_duration() {
        let limits = capture_limits();
        assert_eq!(limits.thumb_max_px, 320);
        assert_eq!(limits.jpeg_quality, 0.72);
        assert_eq!(video_duration_seconds(4.4), 4);
        assert_eq!(video_duration_seconds(4.6), 5);
        assert_eq!(video_duration_seconds(0.0), 1);
        assert_eq!(video_duration_seconds(f64::NAN), 1);
    }

    #[test]
    fn durations_follow_the_kind() {
        assert_eq!(frame_duration("video", "anything"), 0);
        assert_eq!(frame_duration("image", ""), STATUS_IMAGE_MS);
        assert_eq!(frame_duration("text", "short"), STATUS_TEXT_MS);
        // 180 chars => 100 chars over the threshold => 4000 ms extra (capped).
        assert_eq!(frame_duration("text", &"x".repeat(180)), 14_000);
        assert_eq!(frame_duration("text", &"x".repeat(500)), 14_000);
        assert_eq!(frame_duration("text", &"x".repeat(100)), 10_800);
    }

    #[test]
    fn anchors_and_elapsed_time() {
        let a = anchor(8_000, 8_000, 1_000.0);
        assert_eq!(a.total, 8_000);
        assert_eq!(a.ends_at, 9_000.0);
        assert_eq!(elapsed(a.total, a.remaining, a.ends_at, 1_000.0), 0);
        assert_eq!(elapsed(a.total, a.remaining, a.ends_at, 5_000.0), 4_000);
        assert_eq!(elapsed(a.total, a.remaining, a.ends_at, 20_000.0), 8_000);
        // Total is clamped to at least 1 ms.
        assert_eq!(anchor(0, 0, 0.0).total, 1);
    }

    #[test]
    fn pause_and_resume_keep_the_clock() {
        let a = anchor(8_000, 8_000, 1_000.0);
        let remaining = pause_remaining(a.remaining, a.ends_at, 3_500.0);
        assert_eq!(remaining, 5_500);
        // Paused again with no end instant keeps the stored remaining.
        assert_eq!(pause_remaining(remaining, 0.0, 9_999.0), 5_500);

        let (total, remaining) = resume_totals(8_000, 0);
        assert_eq!(total, 8_000);
        assert_eq!(remaining, 1, "never resumes with zero time");
        let (total, remaining) = resume_totals(8_000, 5_500);
        assert_eq!((total, remaining), (8_000, 5_500));
    }

    #[test]
    fn progress_ratio_is_clamped() {
        assert_eq!(progress_ratio(8_000, 8_000), 0.0);
        assert_eq!(progress_ratio(8_000, 4_000), 0.5);
        assert_eq!(progress_ratio(8_000, 0), 1.0);
        assert_eq!(progress_ratio(0, 0), 0.0);
        assert_eq!(progress_ratio(8_000, 12_000), 0.0);
    }

    #[test]
    fn thumbnails_fit_and_never_enlarge() {
        assert_eq!(thumb_scale(640, 480, 320).width, 320);
        assert_eq!(thumb_scale(640, 480, 320).height, 240);
        assert_eq!(thumb_scale(100, 50, 320).width, 100, "no upscaling");
        assert_eq!(thumb_scale(0, 0, 320).width, 1);
        assert_eq!(thumb_scale(1080, 1920, 320).height, 320);
    }

    #[test]
    fn poster_seek() {
        assert_eq!(poster_seek_seconds(10.0), 0.2);
        assert_eq!(poster_seek_seconds(0.2), 0.1);
        assert_eq!(poster_seek_seconds(0.0), 0.2);
        assert_eq!(poster_seek_seconds(f64::NAN), 0.2);
        assert_eq!(STATUS_POSTER_TIMEOUT_MS, 4_000);
        assert_eq!(STATUS_VIDEO_FALLBACK_MS, 20_000);
        assert_eq!(STATUS_MEDIA_FAILED_MS, 3_000);
    }

    fn update(id: &str, kind: &str, ts: i64, viewed: bool) -> StatusUpdateView {
        StatusUpdateView {
            id: id.into(),
            sender: "6285@s.whatsapp.net".into(),
            name: "Ada".into(),
            kind: kind.into(),
            text: "hello".into(),
            timestamp: ts,
            viewed,
            ..Default::default()
        }
    }

    fn request(entries: Vec<StatusEntryView>) -> StatusRequest {
        StatusRequest {
            entries,
            photos: HashMap::new(),
            local_media: HashMap::new(),
            me: "Me".into(),
            me_jid: "62111@s.whatsapp.net".into(),
            me_lid: String::new(),
            now: 1_770_000_000_000,
        }
    }

    #[test]
    fn list_splits_recent_viewed_and_expires() {
        let fresh = 1_770_000_000_000 - 60_000;
        let stale = fresh - STATUS_TTL_MS - 1;
        let entries = vec![
            StatusEntryView {
                jid: "1@s.whatsapp.net".into(),
                name: "Ada".into(),
                updates: vec![update("a", "text", fresh, false)],
            },
            StatusEntryView {
                jid: "2@s.whatsapp.net".into(),
                name: "Budi".into(),
                updates: vec![update("b", "image", fresh, true)],
            },
            StatusEntryView {
                jid: "3@s.whatsapp.net".into(),
                name: "Old".into(),
                updates: vec![update("c", "text", stale, false)],
            },
        ];
        let out = list(&request(entries));
        assert!(
            out.recent_html
                .contains("data-status-jid=\"1@s.whatsapp.net\"")
        );
        assert!(
            out.viewed_html
                .contains("data-status-jid=\"2@s.whatsapp.net\"")
        );
        assert!(!out.recent_html.contains("3@s.whatsapp.net"));
        assert!(!out.viewed_html.contains("3@s.whatsapp.net"));
        assert_eq!(out.unseen, 1);
        assert!(!out.mine_has_thumb);
        assert_eq!(out.mine_label, "Click to add status update");
        assert!(out.recent_html.contains("at "));
    }

    #[test]
    fn my_story_thumbnail_follows_the_kind() {
        let mut entries = vec![StatusEntryView {
            jid: "62111@s.whatsapp.net".into(),
            name: "Me".into(),
            updates: vec![StatusUpdateView {
                id: "mine".into(),
                kind: "text".into(),
                text: "a long story text that should be cut".into(),
                background: "#123456".into(),
                timestamp: 1_770_000_000_000 - 1000,
                from_me: true,
                ..Default::default()
            }],
        }];
        let out = list(&request(entries.clone()));
        assert!(out.mine_has_thumb);
        assert!(out.mine_thumb_style.contains("#123456"));
        assert!(!out.mine_thumb_has_image);
        assert!(out.mine_thumb_html.starts_with("a long story"));

        entries[0].updates[0].kind = "video".into();
        let mut req = request(entries);
        req.local_media
            .insert("mine".into(), "asset://localhost/x.mp4".into());
        let out = list(&req);
        assert!(out.mine_thumb_has_image);
        assert!(out.mine_thumb_style.contains("asset://localhost/x.mp4"));
        assert!(out.mine_thumb_html.contains("<svg"));
        assert_eq!(out.queue, vec!["62111@s.whatsapp.net".to_string()]);
    }

    #[test]
    fn viewer_shell_paints_text_and_waits_for_media() {
        let queue = vec![
            update("a", "text", 1_770_000_000_000, false),
            update("b", "image", 1_770_000_000_000, false),
        ];
        let mut req = StatusFrameRequest {
            queue,
            index: 1,
            group_name: "Ada".into(),
            group_jid: "6285@s.whatsapp.net".into(),
            photos: HashMap::new(),
            local_media: HashMap::new(),
            now: 1_770_000_000_000,
        };
        let out = frame(&req).expect("frame");
        assert_eq!(out.progress_html.matches("<span").count(), 2);
        assert_eq!(out.progress_html.matches("done").count(), 1);
        assert!(out.needs_media);
        assert!(out.body_html.contains("Loading status..."));
        assert_eq!(out.name, "Ada");
        assert!(out.time_label.contains(" at "));

        req.local_media
            .insert("b".into(), "asset://localhost/b.jpg".into());
        let out = frame(&req).unwrap();
        assert!(!out.needs_media);
        assert!(
            out.body_html
                .contains(r#"<img src="asset://localhost/b.jpg""#)
        );

        req.index = 0;
        let out = frame(&req).unwrap();
        assert_eq!(out.kind, "text");
        assert!(out.body_html.contains("text-status"));
        assert_eq!(out.background.as_deref(), Some("#243640"));
    }

    #[test]
    fn media_markup_and_errors() {
        assert!(media_html("/a.mp4", "video", "Ada").starts_with("<video"));
        assert!(media_html("/a.jpg", "image", "Ada").contains(r#"alt="Ada""#));
        assert!(media_error_html().contains("no longer available"));
    }

    #[test]
    fn views_are_newest_first() {
        let viewers = vec![
            ViewerRow {
                jid: "1@s.whatsapp.net".into(),
                name: "Ada".into(),
                timestamp: 1000,
            },
            ViewerRow {
                jid: "2@s.whatsapp.net".into(),
                name: "Budi".into(),
                timestamp: 9000,
            },
        ];
        let out = views(&viewers, &request(vec![]));
        assert_eq!(out.title, "Viewed by 2");
        assert!(out.html.find("Budi").unwrap() < out.html.find("Ada").unwrap());
        assert_eq!(
            out.queue,
            vec![
                "2@s.whatsapp.net".to_string(),
                "1@s.whatsapp.net".to_string()
            ]
        );

        let empty = views(&[], &request(vec![]));
        assert_eq!(empty.title, "Viewed by 0");
        assert!(empty.html.contains("No views recorded yet"));
    }
}
