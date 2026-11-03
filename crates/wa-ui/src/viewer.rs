//! The full-screen media viewer: header labels, caption, stage content (image
//! or the custom video player) and the filmstrip.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::html::{escape, icon};
use crate::text::{MentionResolver, body_html};

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ViewerItem {
    #[serde(default)]
    pub id: String,
    /// `image` or `video`.
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub timestamp: i64,
    #[serde(default)]
    pub mentions: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ViewerRequest {
    #[serde(default)]
    pub items: Vec<ViewerItem>,
    #[serde(default)]
    pub index: usize,
    #[serde(default)]
    pub chat_name: String,
    /// Media URLs by message id (already downloaded).
    #[serde(default)]
    pub media: HashMap<String, String>,
    #[serde(default)]
    pub thumbs: HashMap<String, String>,
    #[serde(default)]
    pub durations: HashMap<String, i64>,
    #[serde(default)]
    pub contacts: HashMap<String, String>,
    #[serde(default)]
    pub phones: HashMap<String, String>,
    #[serde(default)]
    pub sender_names: HashMap<String, String>,
    #[serde(default)]
    pub me: String,
    #[serde(default)]
    pub me_jid: String,
    #[serde(default)]
    pub me_lid: String,
    #[serde(default)]
    pub self_handle: String,
    #[serde(default)]
    pub now: i64,
}

#[derive(Debug, Serialize)]
pub struct ViewerOutput {
    pub chat_label: String,
    pub time_label: String,
    pub caption_html: String,
    /// `None` when there is no caption.
    pub has_caption: bool,
    pub stage_html: String,
    pub strip_html: String,
    pub prev_disabled: bool,
    pub next_disabled: bool,
    pub kind: String,
    pub item_id: String,
    /// The media still has to be fetched before the stage can be painted.
    pub needs_media: bool,
}

/// The window of filmstrip thumbnails around the current item.
const STRIP_BEFORE: usize = 15;
const STRIP_AFTER: usize = 16;

fn duration_label(seconds: i64) -> String {
    if seconds <= 0 {
        String::new()
    } else {
        format!("{}:{:02}", seconds / 60, seconds % 60)
    }
}

/// One filmstrip thumbnail's inner HTML.
fn strip_thumb_inner(
    item: &ViewerItem,
    media: &HashMap<String, String>,
    thumbs: &HashMap<String, String>,
    durations: &HashMap<String, i64>,
) -> String {
    let id = escape(&item.id);
    if item.kind == "video" {
        let poster = thumbs.get(&item.id).cloned().unwrap_or_default();
        let seconds = durations.get(&item.id).copied().unwrap_or(0);
        let badge = format!(
            r#"<span class="strip-duration" data-duration-id="{id}">{}</span>"#,
            duration_label(seconds)
        );
        let art = if poster.is_empty() {
            format!(
                r#"<span class="strip-play big">{}</span>"#,
                icon("play_arrow")
            )
        } else {
            format!(
                r#"<img src="{poster}" alt="" /><span class="strip-play">{}</span>"#,
                icon("play_arrow")
            )
        };
        return format!("{art}{badge}");
    }
    match media.get(&item.id).filter(|url| !url.is_empty()) {
        Some(url) => format!(r#"<img src="{}" alt="" />"#, escape(url)),
        None => r#"<span class="strip-loading"></span>"#.to_string(),
    }
}

/// The custom video player: play/pause, seek, time, speed, mute, fullscreen.
pub fn video_player_html(url: &str, poster: Option<&str>) -> String {
    let poster_attr = match poster.filter(|value| !value.is_empty()) {
        Some(poster) => format!(r#" poster="{}""#, escape(poster)),
        None => String::new(),
    };
    format!(
        r#"<div class="viewer-video" data-viewer-video>
      <video class="viewer-video-el" src="{}"{poster_attr} playsinline autoplay preload="metadata"></video>
      <button type="button" class="viewer-big-play" data-video-toggle aria-label="Play video">{play}</button>
      <div class="viewer-video-controls">
        <button type="button" class="vv-btn" data-video-toggle aria-label="Play">{play}</button>
        <input class="vv-seek" data-video-seek type="range" min="0" max="1000" step="1" value="0" aria-label="Seek" />
        <span class="vv-time"><span data-video-current>0:00</span> / <span data-video-duration>0:00</span></span>
        <button type="button" class="vv-btn vv-speed" data-video-speed aria-label="Playback speed">1.0x</button>
        <button type="button" class="vv-btn" data-video-mute aria-label="Mute">{volume}</button>
        <button type="button" class="vv-btn" data-video-fullscreen aria-label="Fullscreen">{fullscreen}</button>
      </div>
    </div>"#,
        escape(url),
        play = icon("play_arrow"),
        volume = icon("volume_up"),
        fullscreen = icon("fullscreen"),
    )
}

/// What the stage shows for one item once its URL is known.
pub fn stage_html(url: &str, kind: &str, text: &str, poster: Option<&str>) -> String {
    if kind == "video" {
        return video_player_html(url, poster);
    }
    format!(
        r#"<div class="viewer-zoom-layer"><img src="{}" alt="{}" /></div>"#,
        escape(url),
        escape(if text.is_empty() { "Photo" } else { text })
    )
}

pub fn loading_html() -> &'static str {
    r#"<div class="viewer-loading">Loading media...</div>"#
}

pub fn unavailable_html() -> &'static str {
    r#"<div class="viewer-loading">Media unavailable</div>"#
}

/// Render the viewer chrome for one item.
pub fn render(req: &ViewerRequest) -> Option<ViewerOutput> {
    let item = req.items.get(req.index)?;
    let chat_label = if req.chat_name.is_empty() {
        "Media".to_string()
    } else {
        req.chat_name.clone()
    };
    let time_label = format!(
        "{} at {}",
        crate::labels::day_of(item.timestamp, req.now),
        crate::labels::clock_label(item.timestamp)
    );

    let caption_text = item.text.trim();
    let caption_html = if caption_text.is_empty() {
        String::new()
    } else {
        let resolver = MentionResolver {
            me: Some(req.me.clone()),
            me_jid: Some(req.me_jid.clone()),
            me_lid: Some(req.me_lid.clone()),
            contacts: &req.contacts,
            phones: &req.phones,
            sender_names: &req.sender_names,
            self_handle: Some(req.self_handle.clone()),
        };
        body_html(caption_text, &item.mentions, &|user| resolver.resolve(user))
    };

    let media_url = req.media.get(&item.id).filter(|url| !url.is_empty());
    let stage = match media_url {
        Some(url) => stage_html(
            url,
            &item.kind,
            &item.text,
            req.thumbs.get(&item.id).map(String::as_str),
        ),
        None => loading_html().to_string(),
    };

    let start = req.index.saturating_sub(STRIP_BEFORE);
    let end = (req.index + STRIP_AFTER).min(req.items.len());
    let strip_html = req.items[start..end]
        .iter()
        .enumerate()
        .map(|(offset, entry)| {
            let index = start + offset;
            format!(
                r#"<button type="button" class="media-strip-thumb {}" data-index="{index}" data-media-id="{}" data-thumb-id="{}" title="{}">{}</button>"#,
                if index == req.index { "active" } else { "" },
                escape(&entry.id),
                escape(&entry.id),
                if entry.kind == "video" { "Video" } else { "Photo" },
                strip_thumb_inner(entry, &req.media, &req.thumbs, &req.durations)
            )
        })
        .collect();

    Some(ViewerOutput {
        chat_label,
        time_label,
        has_caption: !caption_html.is_empty(),
        caption_html,
        stage_html: stage,
        strip_html,
        prev_disabled: req.index == 0,
        next_disabled: req.index + 1 >= req.items.len(),
        kind: item.kind.clone(),
        item_id: item.id.clone(),
        needs_media: media_url.is_none(),
    })
}

/// The inner HTML of one strip thumbnail once its media has downloaded.
pub fn strip_thumb_html(
    id: &str,
    kind: &str,
    url: &str,
    poster: Option<&str>,
    duration: i64,
) -> String {
    if kind == "video" {
        let art = match poster.filter(|value| !value.is_empty()) {
            Some(poster) => format!(
                r#"<img src="{}" alt="" /><span class="strip-play">{}</span>"#,
                escape(poster),
                icon("play_arrow")
            ),
            None => format!(
                r#"<span class="strip-play big">{}</span>"#,
                icon("play_arrow")
            ),
        };
        return format!(
            r#"{art}<span class="strip-duration" data-duration-id="{}">{}</span>"#,
            escape(id),
            duration_label(duration)
        );
    }
    format!(r#"<img src="{}" alt="" />"#, escape(url))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, kind: &str, text: &str) -> ViewerItem {
        ViewerItem {
            id: id.into(),
            kind: kind.into(),
            text: text.into(),
            timestamp: 1_770_000_000_000,
            ..Default::default()
        }
    }

    fn request(items: Vec<ViewerItem>, index: usize) -> ViewerRequest {
        ViewerRequest {
            items,
            index,
            chat_name: "Ada".into(),
            media: HashMap::new(),
            thumbs: HashMap::new(),
            durations: HashMap::new(),
            contacts: HashMap::new(),
            phones: HashMap::new(),
            sender_names: HashMap::new(),
            me: "Me".into(),
            me_jid: "62111@s.whatsapp.net".into(),
            me_lid: String::new(),
            self_handle: String::new(),
            now: 1_770_000_000_000,
        }
    }

    #[test]
    fn image_stage_and_caption() {
        let mut req = request(vec![item("a", "image", "look *here*")], 0);
        req.media.insert("a".into(), "/media/a.jpg".into());
        let out = render(&req).unwrap();
        assert_eq!(out.chat_label, "Ada");
        assert!(out.time_label.contains(" at "));
        assert!(out.has_caption);
        assert!(out.caption_html.contains("<b>here</b>"));
        assert!(out.stage_html.contains(r#"class="viewer-zoom-layer""#));
        assert!(out.stage_html.contains(r#"src="/media/a.jpg""#));
        assert!(!out.needs_media);
        assert!(out.prev_disabled);
        assert!(out.next_disabled);
    }

    #[test]
    fn video_uses_the_custom_player() {
        let mut req = request(vec![item("v", "video", "")], 0);
        req.media.insert("v".into(), "/media/v.mp4".into());
        req.thumbs.insert("v".into(), "data:poster".into());
        let out = render(&req).unwrap();
        assert!(out.stage_html.contains("viewer-video"));
        assert!(out.stage_html.contains(r#"poster="data:poster""#));
        assert!(out.stage_html.contains("data-video-seek"));
        assert!(out.stage_html.contains("data-video-speed"));
        assert!(!out.has_caption);
    }

    #[test]
    fn missing_media_waits_for_the_download() {
        let out = render(&request(vec![item("a", "image", "")], 0)).unwrap();
        assert!(out.needs_media);
        assert!(out.stage_html.contains("Loading media..."));
    }

    #[test]
    fn filmstrip_windows_and_marks_the_active_thumb() {
        let items: Vec<ViewerItem> = (0..60)
            .map(|i| {
                item(
                    &format!("m{i}"),
                    if i % 3 == 0 { "video" } else { "image" },
                    "",
                )
            })
            .collect();
        let mut req = request(items, 30);
        req.media.insert("m31".into(), "/m31.jpg".into());
        req.durations.insert("m30".into(), 12);
        let out = render(&req).unwrap();
        let thumbs = out.strip_html.matches("media-strip-thumb").count();
        assert_eq!(thumbs, 31, "15 before + current + 16 after");
        assert_eq!(out.strip_html.matches("active").count(), 1);
        assert!(out.strip_html.contains(r#"data-index="30""#));
        assert!(out.strip_html.contains("strip-duration"));
        assert!(out.strip_html.contains("0:12"));
        assert!(out.strip_html.contains("strip-loading"));
        assert!(!out.prev_disabled && !out.next_disabled);
    }

    #[test]
    fn strip_thumb_refresh() {
        assert!(strip_thumb_html("v", "video", "/x.mp4", Some("/p.jpg"), 5).contains("0:05"));
        assert_eq!(
            strip_thumb_html("i", "image", "/x.jpg", None, 0),
            r#"<img src="/x.jpg" alt="" />"#
        );
    }

    #[test]
    fn placeholders() {
        assert!(loading_html().contains("Loading media"));
        assert!(unavailable_html().contains("unavailable"));
        assert!(video_player_html("/v.mp4", None).contains("viewer-video-el"));
    }
}
