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

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ViewerRow {
    #[serde(default)]
    pub jid: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub timestamp: i64,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
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

#[derive(Debug, Clone, Default, Deserialize)]
pub struct OwnUpdateInput {
    /// The server id when the post returned one, otherwise empty.
    #[serde(default)]
    pub server_id: String,
    /// The client-side id (local media is keyed by it) when there is no server
    /// id; empty falls back to `local-<timestamp>`.
    #[serde(default)]
    pub local_id: String,
    #[serde(default)]
    pub sender: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub background: String,
    #[serde(default)]
    pub timestamp: i64,
}

/// The optimistic own story after a successful post: ours, already seen, and
/// keeping the server id when there is one (only that id can be revoked).
pub fn own_update(input: &OwnUpdateInput) -> StatusUpdateView {
    let id = if !input.server_id.is_empty() {
        input.server_id.clone()
    } else if !input.local_id.is_empty() {
        input.local_id.clone()
    } else {
        format!("local-{}", input.timestamp)
    };
    StatusUpdateView {
        id,
        sender: if input.sender.is_empty() {
            "me".to_string()
        } else {
            input.sender.clone()
        },
        name: if input.name.is_empty() {
            "Me".to_string()
        } else {
            input.name.clone()
        },
        kind: if input.kind.is_empty() {
            "text".to_string()
        } else {
            input.kind.clone()
        },
        text: input.text.clone(),
        background: input.background.clone(),
        timestamp: input.timestamp,
        viewed: true,
        from_me: true,
        viewers: Vec::new(),
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DraftPreview {
    /// `video` or `image` (empty when the MIME is neither).
    pub kind: &'static str,
    /// The `<video controls>` / `<img>` preview markup for the draft.
    pub html: String,
}

/// The composer's draft preview for a picked file.
pub fn draft_preview(mime: &str, url: &str) -> DraftPreview {
    let kind = if mime.starts_with("video/") {
        "video"
    } else if mime.starts_with("image/") {
        "image"
    } else {
        ""
    };
    let safe = crate::html::escape(url);
    DraftPreview {
        kind,
        html: match kind {
            "video" => format!("<video src=\"{safe}\" controls></video>"),
            "image" => format!("<img src=\"{safe}\" alt=\"\" />"),
            _ => String::new(),
        },
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ViewerChromeInput {
    #[serde(default)]
    pub own: bool,
    #[serde(default)]
    pub has_frame: bool,
    #[serde(default)]
    pub paused: bool,
    #[serde(default)]
    pub group_index: usize,
    #[serde(default)]
    pub group_count: usize,
    #[serde(default)]
    pub frame_index: usize,
    #[serde(default)]
    pub frame_count: usize,
    #[serde(default)]
    pub viewers: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ViewerChrome {
    /// The "previous" arrow is useless on the very first story.
    pub prev_hidden: bool,
    pub next_hidden: bool,
    /// The story's own frame: no reply bar, grey footer, add badge, view count.
    pub own_story: bool,
    pub reply_hidden: bool,
    pub add_badge: bool,
    pub view_count_hidden: bool,
    pub view_count_label: String,
    pub pause_hidden: bool,
    pub pause_pressed: bool,
    pub pause_title: &'static str,
}

/// The story viewer's chrome for one frame: which arrows, the pause glyph and
/// whether the reply bar / view counter belong to this story.
pub fn viewer_chrome(input: &ViewerChromeInput) -> ViewerChrome {
    let frames_left = input.frame_count.saturating_sub(input.frame_index + 1);
    let groups_left = input.group_count.saturating_sub(input.group_index + 1);
    ViewerChrome {
        prev_hidden: input.group_index == 0 && input.frame_index == 0,
        next_hidden: frames_left == 0 && groups_left == 0,
        own_story: input.own,
        reply_hidden: input.own,
        add_badge: input.own,
        view_count_hidden: !input.own,
        view_count_label: if input.own {
            input.viewers.to_string()
        } else {
            String::new()
        },
        pause_hidden: !input.has_frame,
        pause_pressed: input.paused,
        pause_title: if input.paused { "Play" } else { "Pause" },
    }
}

/// The next position after a step: `(group, frame)` or `None` when it runs off
/// the end of the last group.
/// Where a step lands. `frame: None` means "the last frame of that group" (a
/// backwards step into the previous group cannot know its length up front).
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub struct StepTarget {
    pub group: usize,
    pub frame: Option<usize>,
}

pub fn next_position(
    group: usize,
    frame: usize,
    frame_count: usize,
    group_count: usize,
    forward: bool,
) -> Option<StepTarget> {
    let at = |group: usize, frame: Option<usize>| Some(StepTarget { group, frame });
    if forward {
        if frame + 1 < frame_count {
            return at(group, Some(frame + 1));
        }
        if group + 1 < group_count {
            return at(group + 1, Some(0));
        }
        None
    } else {
        if frame > 0 {
            return at(group, Some(frame - 1));
        }
        if group > 0 {
            return at(group - 1, None);
        }
        None
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ViewerGroup {
    pub jid: String,
    pub name: String,
    /// Live update ids, oldest first (the viewer's play order).
    pub update_ids: Vec<String>,
}

/// The groups the viewer can step through: entries with a live story, newest
/// activity first, each group's updates oldest first.
pub fn viewer_groups(req: &StatusRequest) -> Vec<ViewerGroup> {
    let mut entries = live_entries(req);
    entries.sort_by_key(|entry| std::cmp::Reverse(latest(entry)));
    entries
        .into_iter()
        .map(|entry| {
            let mut updates = entry.updates.clone();
            updates.sort_by_key(|update| update.timestamp);
            ViewerGroup {
                jid: entry.jid.clone(),
                name: entry.name.clone(),
                update_ids: updates.into_iter().map(|update| update.id).collect(),
            }
        })
        .collect()
}

/// Ids of our own stories, so a view receipt can be attributed to one of them.
pub fn own_update_ids(req: &StatusRequest) -> Vec<String> {
    req.entries
        .iter()
        .flat_map(|entry| entry.updates.iter())
        .filter(|update| update.from_me && !update.id.is_empty())
        .map(|update| update.id.clone())
        .collect()
}

/// One viewer, or `None` when the receipt should be ignored.
#[derive(Debug, Clone, Serialize)]
pub struct NoteView {
    pub accept: bool,
    pub viewer: Option<ViewerRow>,
}

/// Add a "view" receipt to a story: the author JID loses its device suffix, the
/// broadcast/newsletter chat itself is never a viewer, and a repeat is dropped.
pub fn note_view(viewers: &[ViewerRow], jid: &str, name: &str, timestamp: i64) -> NoteView {
    let normalized = crate::text::jid_without_device(jid);
    if normalized.is_empty()
        || normalized.ends_with("@broadcast")
        || normalized.ends_with("@newsletter")
    {
        return NoteView {
            accept: false,
            viewer: None,
        };
    }
    if viewers.iter().any(|viewer| viewer.jid == normalized) {
        return NoteView {
            accept: false,
            viewer: None,
        };
    }
    NoteView {
        accept: true,
        viewer: Some(ViewerRow {
            jid: normalized.clone(),
            name: if name.is_empty() {
                normalized
            } else {
                name.to_string()
            },
            timestamp,
        }),
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct HealMessage {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub status_quote: bool,
    #[serde(default)]
    pub reply_to_id: String,
    #[serde(default)]
    pub once_recovered: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct HealChat {
    #[serde(default)]
    pub messages: Vec<HealMessage>,
}

#[derive(Debug, Default, Serialize)]
pub struct HealPlan {
    /// Messages that quote a story and must wear the "· Status" card.
    pub quote_ids: Vec<String>,
    /// Phantom chat bubbles recovered from a status reply, now dropped.
    pub remove_ids: Vec<String>,
}

/// Replies/reactions to a story used to be stored as plain replies with the
/// quoted story recovered as a phantom bubble; heal both once the id is a story
/// we track.
pub fn heal_plan(chats: &[HealChat], update_ids: &[String]) -> Vec<HealPlan> {
    let known: std::collections::HashSet<&str> = update_ids.iter().map(String::as_str).collect();
    chats
        .iter()
        .map(|chat| {
            let mut plan = HealPlan::default();
            for message in &chat.messages {
                if !message.status_quote
                    && !message.reply_to_id.is_empty()
                    && known.contains(message.reply_to_id.as_str())
                {
                    plan.quote_ids.push(message.id.clone());
                }
                if message.once_recovered && known.contains(message.id.as_str()) {
                    plan.remove_ids.push(message.id.clone());
                }
            }
            plan
        })
        .collect()
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
    fn own_updates_keep_the_server_id_and_start_viewed() {
        let text = own_update(&OwnUpdateInput {
            server_id: "3EB0".into(),
            local_id: String::new(),
            sender: "6285@s.whatsapp.net".into(),
            name: "Ada".into(),
            kind: "text".into(),
            text: "hello".into(),
            background: "#243640".into(),
            timestamp: 42,
        });
        assert_eq!(text.id, "3EB0", "the server id is what can be revoked");
        assert!(text.from_me && text.viewed);
        assert_eq!(text.kind, "text");
        assert_eq!(text.background, "#243640");
        assert!(text.viewers.is_empty());

        // No server id: a local id, plus the fallbacks for identity and kind.
        let local = own_update(&OwnUpdateInput {
            timestamp: 99,
            ..Default::default()
        });
        assert_eq!(local.id, "local-99");
        assert_eq!(local.sender, "me");
        assert_eq!(local.name, "Me");
        assert_eq!(local.kind, "text");
        assert!(local.background.is_empty());

        // A client id (local media is keyed by it) wins over the fallback.
        let keyed = own_update(&OwnUpdateInput {
            local_id: "local-status-7".into(),
            timestamp: 99,
            ..Default::default()
        });
        assert_eq!(keyed.id, "local-status-7");
    }

    #[test]
    fn the_draft_preview_picks_a_tag_per_mime() {
        let video = draft_preview("video/mp4", "blob:abc");
        assert_eq!(video.kind, "video");
        assert!(video.html.starts_with("<video src=\"blob:abc\""));
        assert!(video.html.contains("controls"));

        let image = draft_preview("image/png", "blob:def");
        assert_eq!(image.kind, "image");
        assert_eq!(image.html, "<img src=\"blob:def\" alt=\"\" />");

        // Anything else has no preview, and quotes in the URL cannot break out.
        let other = draft_preview("application/pdf", "blob:x");
        assert_eq!(other.kind, "");
        assert!(other.html.is_empty());
        let quoted = draft_preview("image/png", "blob:x\" onerror=\"alert(1)");
        assert!(!quoted.html.contains("\" onerror="));
    }

    #[test]
    fn viewer_chrome_follows_the_story_owner() {
        let theirs = viewer_chrome(&ViewerChromeInput {
            own: false,
            has_frame: true,
            paused: false,
            group_index: 0,
            group_count: 2,
            frame_index: 0,
            frame_count: 3,
            viewers: 0,
        });
        assert!(theirs.prev_hidden, "first frame of the first group");
        assert!(!theirs.next_hidden);
        assert!(!theirs.own_story && !theirs.reply_hidden);
        assert!(!theirs.add_badge && theirs.view_count_hidden);
        assert!(!theirs.pause_hidden && !theirs.pause_pressed);
        assert_eq!(theirs.pause_title, "Pause");

        let mine = viewer_chrome(&ViewerChromeInput {
            own: true,
            has_frame: true,
            paused: true,
            group_index: 1,
            group_count: 2,
            frame_index: 2,
            frame_count: 3,
            viewers: 7,
        });
        assert!(!mine.prev_hidden);
        assert!(mine.next_hidden, "last frame of the last group");
        assert!(mine.own_story && mine.reply_hidden && mine.add_badge);
        assert!(!mine.view_count_hidden);
        assert_eq!(mine.view_count_label, "7");
        assert!(mine.pause_pressed);
        assert_eq!(mine.pause_title, "Play");

        // No frame yet: the pause button hides.
        let empty = viewer_chrome(&ViewerChromeInput::default());
        assert!(empty.pause_hidden);
    }

    #[test]
    fn stepping_walks_frames_then_groups() {
        let at = |group: usize, frame: Option<usize>| Some(StepTarget { group, frame });
        // Two groups of three frames.
        assert_eq!(next_position(0, 0, 3, 2, true), at(0, Some(1)));
        assert_eq!(next_position(0, 2, 3, 2, true), at(1, Some(0)));
        assert_eq!(next_position(1, 2, 3, 2, true), None, "end of the queue");
        assert_eq!(next_position(0, 0, 3, 2, false), None, "start");
        // Backwards into the previous group lands on its last frame.
        assert_eq!(next_position(1, 0, 3, 2, false), at(0, None));
        assert_eq!(next_position(0, 1, 3, 2, false), at(0, Some(0)));
    }

    #[test]
    fn viewer_groups_order_newest_first_oldest_updates_first() {
        let fresh = 1_770_000_000_000;
        let entries = vec![
            StatusEntryView {
                jid: "older@s.whatsapp.net".into(),
                name: "Older".into(),
                updates: vec![
                    update("o2", "text", fresh - 10_000, false),
                    update("o1", "text", fresh - 60_000, false),
                ],
            },
            StatusEntryView {
                jid: "newer@s.whatsapp.net".into(),
                name: "Newer".into(),
                updates: vec![update("n1", "image", fresh - 1_000, false)],
            },
            StatusEntryView {
                jid: "stale@s.whatsapp.net".into(),
                name: "Stale".into(),
                updates: vec![update("s1", "text", fresh - STATUS_TTL_MS - 1, false)],
            },
        ];
        let groups = viewer_groups(&request(entries));
        assert_eq!(groups.len(), 2, "expired entries are dropped");
        assert_eq!(groups[0].jid, "newer@s.whatsapp.net");
        assert_eq!(groups[1].jid, "older@s.whatsapp.net");
        assert_eq!(
            groups[1].update_ids,
            vec!["o1".to_string(), "o2".to_string()]
        );
    }

    #[test]
    fn own_update_ids_only_lists_our_stories() {
        let fresh = 1_770_000_000_000;
        let mut mine = update("mine", "text", fresh, true);
        mine.from_me = true;
        let theirs = update("theirs", "text", fresh, false);
        let ids = own_update_ids(&request(vec![StatusEntryView {
            jid: "me@s.whatsapp.net".into(),
            name: "Me".into(),
            updates: vec![mine, theirs],
        }]));
        assert_eq!(ids, vec!["mine".to_string()]);
    }

    #[test]
    fn view_receipts_normalize_and_dedupe() {
        let viewers = vec![ViewerRow {
            jid: "111@lid".into(),
            name: "Ada".into(),
            timestamp: 1,
        }];
        // A device suffix is stripped before comparing.
        let again = note_view(&viewers, "111:42@lid", "Ada", 5);
        assert!(!again.accept, "already recorded");
        assert!(again.viewer.is_none());

        let fresh = note_view(&viewers, "222:9@s.whatsapp.net", "", 7);
        assert!(fresh.accept);
        let viewer = fresh.viewer.unwrap();
        assert_eq!(viewer.jid, "222@s.whatsapp.net");
        assert_eq!(
            viewer.name, "222@s.whatsapp.net",
            "name falls back to the JID"
        );
        assert_eq!(viewer.timestamp, 7);

        assert!(!note_view(&[], "status@broadcast", "x", 1).accept);
        assert!(!note_view(&[], "1@newsletter", "x", 1).accept);
        assert!(!note_view(&[], "", "x", 1).accept);
    }

    #[test]
    fn healing_marks_status_quotes_and_drops_phantoms() {
        let chats = vec![
            HealChat {
                messages: vec![
                    HealMessage {
                        id: "m1".into(),
                        status_quote: false,
                        reply_to_id: "story-1".into(),
                        once_recovered: false,
                    },
                    HealMessage {
                        id: "m2".into(),
                        status_quote: false,
                        reply_to_id: "chat-msg".into(),
                        once_recovered: false,
                    },
                    HealMessage {
                        id: "m3".into(),
                        status_quote: true,
                        reply_to_id: "story-1".into(),
                        once_recovered: false,
                    },
                ],
            },
            HealChat {
                messages: vec![HealMessage {
                    id: "story-1".into(),
                    status_quote: false,
                    reply_to_id: String::new(),
                    once_recovered: true,
                }],
            },
        ];
        let plans = heal_plan(&chats, &["story-1".to_string()]);
        assert_eq!(plans.len(), 2);
        assert_eq!(plans[0].quote_ids, vec!["m1".to_string()]);
        assert!(plans[0].remove_ids.is_empty());
        assert_eq!(plans[1].remove_ids, vec!["story-1".to_string()]);

        let none = heal_plan(&chats, &[]);
        assert!(none[0].quote_ids.is_empty() && none[0].remove_ids.is_empty());
        assert!(none[1].remove_ids.is_empty());
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
