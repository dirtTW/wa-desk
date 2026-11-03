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

#[cfg(test)]
mod tests {
    use super::*;

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
