//! The message list renderer: day dividers, bubble classes, sender runs,
//! reply/status quotes, albums, media blocks, documents, link previews,
//! reactions, ticks and channel counters.
//!
//! The webview owns hydration (downloads, thumbnails, players) but not the
//! markup: it sends its state and injects the HTML returned here.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::html::{avatar, escape, icon};
use crate::labels::{clock_label, day_of, format_followers};
use crate::text::{
    MentionResolver, MessageRow, body_html, format_phone, is_self_jid, phone_from_jid, sender_color,
};

/* ------------------------------------------------------------------ request */

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ChatInfo {
    #[serde(default)]
    pub jid: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub is_group: bool,
    #[serde(default)]
    pub is_channel: bool,
    #[serde(default)]
    pub subscribers: i64,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct MediaState {
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChatRequest {
    #[serde(default)]
    pub chat: ChatInfo,
    #[serde(default)]
    pub messages: Vec<MessageRow>,
    #[serde(default)]
    pub media: HashMap<String, MediaState>,
    /// Inline JPEG posters by message id.
    #[serde(default)]
    pub thumbs: HashMap<String, String>,
    /// Frames sampled from downloaded clips by message id.
    #[serde(default)]
    pub posters: HashMap<String, String>,
    /// Voice-note length in seconds by message id.
    #[serde(default)]
    pub durations: HashMap<String, i64>,
    #[serde(default)]
    pub doc_thumbs: HashMap<String, String>,
    #[serde(default)]
    pub photos: HashMap<String, String>,
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
    /// Resolved phone for a LID chat, used by the header subtitle.
    #[serde(default)]
    pub known_phone: String,
    #[serde(default)]
    pub show_deleted: bool,
    #[serde(default)]
    pub selecting: bool,
    #[serde(default)]
    pub selected: Vec<String>,
    #[serde(default)]
    pub starred: Vec<String>,
    #[serde(default)]
    pub now: i64,
}

#[derive(Debug, Serialize)]
pub struct ChatOutput {
    /// Full `<div class="bubble">` list, day dividers included.
    pub html: String,
    pub empty_html: String,
    /// Header subtitle ("Group chat", "+62 …", "Message yourself", followers).
    pub subtitle: String,
    /// Sender JIDs whose profile photo is worth fetching.
    pub queue: Vec<String>,
    /// Number of rendered messages (hidden ones excluded).
    pub count: usize,
}

/* ------------------------------------------------------------------- helpers */

fn banner_forwarded() -> String {
    r#"<span class="forwarded"><svg viewBox="0 -960 960 960" aria-hidden="true"><path d="m640-280-57-56 184-184-184-184 57-56 240 240-240 240ZM80-200v-160q0-83 58.5-141.5T280-560h247L383-704l57-56 240 240-240 240-57-56 144-144H280q-50 0-85 35t-35 85v160H80Z"/></svg>Forwarded</span>"#.to_string()
}

fn view_once_placeholder(open_id: Option<&str>) -> String {
    let class = if open_id.is_some() {
        "body view-once openable"
    } else {
        "body view-once"
    };
    let open = match open_id {
        Some(id) => format!(
            r#" data-view-once-open="{}" title="Tap to view""#,
            escape(id)
        ),
        None => String::new(),
    };
    format!(
        r#"
      <div class="{class}"{open}>
        <span class="view-once-icon" aria-hidden="true">{}</span>
        You received a view once message. For added privacy, you can only open it on your phone.
        <a class="view-once-more" href="https://faq.whatsapp.com/578442220724722/?cms_platform=android" target="_blank" rel="noreferrer">Learn more</a>
      </div>"#,
        icon("schedule")
    )
}

/// WhatsApp renders emoji-only messages much larger.
fn is_emoji_only(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() || trimmed.chars().count() > 12 {
        return false;
    }
    let mut has_pictograph = false;
    for ch in trimmed.chars() {
        if ch.is_alphanumeric() {
            return false;
        }
        let code = ch as u32;
        if code >= 0x1F000 || (0x2600..=0x27BF).contains(&code) {
            has_pictograph = true;
        }
    }
    has_pictograph
}

/// "95k" / "1.2m", the way WhatsApp abbreviates counters.
pub fn compact_count(value: i64) -> String {
    if value >= 1_000_000 {
        let text = format!("{:.1}", value as f64 / 1_000_000.0);
        text.trim_end_matches(".0").to_string() + "m"
    } else if value >= 1_000 {
        let text = format!("{:.1}", value as f64 / 1_000.0);
        text.trim_end_matches(".0").to_string() + "k"
    } else {
        value.to_string()
    }
}

fn tick_html(message: &MessageRow, self_chat: bool) -> String {
    if !message.from_me {
        return String::new();
    }
    if message.pending {
        return format!(r#"<span class="tick">{}</span>"#, icon("hourglass_empty"));
    }
    // The self chat is owned by the phone: our own sends are delivered at once.
    if self_chat {
        return format!(r#"<span class="tick">{}</span>"#, icon("done_all"));
    }
    let status = if message.status.is_empty() {
        "sent"
    } else {
        message.status.as_str()
    };
    if status == "read" || status == "played" {
        return format!(r#"<span class="tick read">{}</span>"#, icon("done_all"));
    }
    if status == "delivered" {
        return format!(r#"<span class="tick">{}</span>"#, icon("done_all"));
    }
    format!(r#"<span class="tick">{}</span>"#, icon("check"))
}
/* --------------------------------------------------------------- context */

struct Ctx<'a> {
    req: &'a ChatRequest,
    self_chat: bool,
    starred: &'a HashSet<String>,
    selected: &'a HashSet<String>,
}

impl Ctx<'_> {
    fn mentions(&self) -> MentionResolver<'_> {
        MentionResolver {
            me: Some(self.req.me.clone()),
            me_jid: Some(self.req.me_jid.clone()),
            me_lid: Some(self.req.me_lid.clone()),
            contacts: &self.req.contacts,
            phones: &self.req.phones,
            sender_names: &self.req.sender_names,
            self_handle: Some(self.req.self_handle.clone()),
        }
    }

    fn body(&self, message: &MessageRow) -> String {
        let resolver = self.mentions();
        body_html(&message.text, &message.mentions, &|user| {
            resolver.resolve(user)
        })
    }

    fn time(&self, ms: i64) -> String {
        clock_label(ms)
    }

    /// The name WhatsApp prints above a group bubble.
    fn sender_display_name(&self, message: &MessageRow) -> String {
        let saved = self
            .req
            .contacts
            .get(&phone_from_jid(&message.sender))
            .cloned()
            .or_else(|| {
                self.req
                    .phones
                    .get(&message.sender)
                    .and_then(|pn| self.req.contacts.get(pn))
                    .cloned()
            })
            .unwrap_or_default();
        if !saved.is_empty() {
            return saved;
        }
        let push = if message.sender_name.is_empty() {
            self.req
                .sender_names
                .get(&phone_from_jid(&message.sender))
                .cloned()
                .unwrap_or_default()
        } else {
            message.sender_name.clone()
        };
        if !push.is_empty() {
            return format!("~ {push}");
        }
        let is_lid = message.sender.ends_with("@lid");
        let source = if !message.sender_phone.is_empty() {
            message.sender_phone.clone()
        } else if !message.sender.is_empty() && !is_lid {
            phone_from_jid(&message.sender)
        } else {
            String::new()
        };
        let phone = format_phone(&source);
        if phone.is_empty() {
            "Unknown".to_string()
        } else {
            phone
        }
    }

    fn sender_html(&self, message: &MessageRow) -> String {
        if !self.req.chat.is_group || message.from_me {
            return String::new();
        }
        let name = self.sender_display_name(message);
        let unresolved = if name == "Unknown" && message.sender.ends_with("@lid") {
            format!(r#" data-phone-jid="{}""#, escape(&message.sender))
        } else {
            String::new()
        };
        let color = sender_color(if message.sender.is_empty() {
            &name
        } else {
            &message.sender
        });
        format!(
            r#"<div class="sender" style="color:{color}"><span class="sender-name"{unresolved}>{}</span></div>"#,
            escape(&name)
        )
    }

    fn avatar_html(&self, message: &MessageRow) -> String {
        if !self.req.chat.is_group || message.from_me || self.req.chat.is_channel {
            return String::new();
        }
        if message.sender.is_empty() {
            return String::new();
        }
        avatar(
            &self.sender_display_name(message),
            &message.sender,
            "avatar msg-avatar",
            &self.req.photos,
        )
    }

    fn reply_quote(&self, reply: &ReplyRow) -> String {
        let name = if !reply.sender_name.is_empty() {
            reply.sender_name.clone()
        } else if !reply.sender.is_empty() {
            phone_from_jid(&reply.sender)
        } else {
            "Message".to_string()
        };
        let kind_label = media_label(&reply.kind);
        let text = if !reply.text.is_empty() {
            let resolver = self.mentions();
            body_html(&reply.text, &[], &|user| resolver.resolve(user))
        } else if let Some(label) = kind_label {
            format!(r#"<span class="quote-kind">{}</span>"#, escape(label))
        } else {
            String::new()
        };
        let thumb = match reply.thumb.as_deref() {
            Some(thumb) => format!(
                r#"<img class="quote-thumb" src="{}" alt="" />"#,
                escape(thumb)
            ),
            None => String::new(),
        };
        let (attr, role, title) = if reply.id.is_empty() {
            (String::new(), "presentation".to_string(), String::new())
        } else {
            (
                format!(r#" data-quote-id="{}""#, escape(&reply.id)),
                "button".to_string(),
                r#" tabindex="0" title="Go to the quoted message""#.to_string(),
            )
        };
        format!(
            r#"<div class="quote"{attr} role="{role}"{title}>{thumb}<div class="quote-main"><div class="quote-sender">{}</div><div class="quote-text">{text}</div></div></div>"#,
            escape(&name)
        )
    }

    fn status_quote(&self, reply: &ReplyRow) -> String {
        let name = if !reply.sender_name.is_empty() {
            reply.sender_name.clone()
        } else if !reply.sender.is_empty() {
            phone_from_jid(&reply.sender)
        } else {
            "Status".to_string()
        };
        let label = media_label(&reply.kind);
        let icon_name = match reply.kind.as_str() {
            "video" => "videocam",
            "audio" => "mic",
            "document" => "description",
            "sticker" => "sticker",
            _ => "image",
        };
        let body = match label {
            Some(label) => format!(
                r#"<div class="status-quote-kind">{}{}</div>"#,
                icon(icon_name),
                escape(label)
            ),
            None if !reply.text.is_empty() => format!(
                r#"<div class="status-quote-text">{}</div>"#,
                self.body_text(&reply.text)
            ),
            None => String::new(),
        };
        let thumb = match reply.thumb.as_deref() {
            Some(thumb) => format!(
                r#"<img class="status-quote-thumb" src="{}" alt="" />"#,
                escape(thumb)
            ),
            None => String::new(),
        };
        let (attr, role, title) = if reply.id.is_empty() {
            (String::new(), "presentation".to_string(), String::new())
        } else {
            (
                format!(r#" data-quote-id="{}""#, escape(&reply.id)),
                "button".to_string(),
                r#" tabindex="0" title="Go to the status""#.to_string(),
            )
        };
        format!(
            r#"<div class="quote status-quote"{attr} role="{role}"{title}>{thumb}<div class="status-quote-name">{}<span> · Status</span></div>{body}</div>"#,
            escape(&name)
        )
    }

    fn body_text(&self, text: &str) -> String {
        let resolver = self.mentions();
        body_html(text, &[], &|user| resolver.resolve(user))
    }

    fn reactions(&self, message: &MessageRow) -> String {
        if message.reactions.is_empty() {
            return String::new();
        }
        let mut counts: Vec<(String, usize)> = Vec::new();
        for reaction in &message.reactions {
            if reaction.emoji.is_empty() {
                continue;
            }
            match counts
                .iter_mut()
                .find(|(emoji, _)| *emoji == reaction.emoji)
            {
                Some((_, count)) => *count += 1,
                None => counts.push((reaction.emoji.clone(), 1)),
            }
        }
        if counts.is_empty() {
            return String::new();
        }
        let pills: String = counts
            .iter()
            .map(|(emoji, count)| {
                format!(
                    r#"<span class="reaction-pill">{}{}</span>"#,
                    escape(emoji),
                    if *count > 1 {
                        format!(" {count}")
                    } else {
                        String::new()
                    }
                )
            })
            .collect();
        format!(r#"<div class="reactions">{pills}</div>"#)
    }

    fn media_block(&self, message: &MessageRow) -> String {
        let cached = self.req.media.get(&message.id);
        if let Some(state) = cached
            && state.status == "ready"
            && !state.url.is_empty()
        {
            return self.media_element(&message.kind, &state.url, message);
        }
        if let Some(state) = cached
            && state.status == "error"
        {
            let preview = self
                .req
                .thumbs
                .get(&message.id)
                .cloned()
                .unwrap_or_default();
            let style = if preview.is_empty() {
                String::new()
            } else {
                format!(r#" style="background-image:url('{preview}')""#)
            };
            let has_thumb = if preview.is_empty() { "" } else { "has-thumb" };
            let fallback = if preview.is_empty() {
                media_icon(&message.kind)
            } else {
                String::new()
            };
            return format!(
                r#"<div class="media media-box error retry {has_thumb}" data-media-id="{}" data-media-kind="{}" data-failed="1"{style}>{fallback}<span class="media-retry-hint">Tap to retry</span></div>"#,
                escape(&message.id),
                escape(&message.kind)
            );
        }

        let is_video = message.kind == "video";
        let is_audio = message.kind == "audio";
        let poster = if is_video {
            self.req
                .thumbs
                .get(&message.id)
                .cloned()
                .unwrap_or_default()
        } else {
            String::new()
        };
        let play = if is_video {
            format!(r#"<span class="media-play">{}</span>"#, icon("play_arrow"))
        } else {
            String::new()
        };
        let hint = if is_video {
            "Tap to load video"
        } else if message.kind == "sticker" {
            "Loading sticker..."
        } else if is_audio {
            "Loading audio..."
        } else {
            "Loading photo..."
        };
        let style = if poster.is_empty() {
            String::new()
        } else {
            format!(r#" style="background-image:url('{poster}')""#)
        };
        format!(
            r#"<div class="media media-box {video} {thumb}" data-media-id="{}" data-media-kind="{}" data-thumb-id="{}"{style}>{play}<span class="media-icon">{}</span><span class="media-hint">{hint}</span></div>"#,
            escape(&message.id),
            escape(&message.kind),
            escape(&message.id),
            media_icon(&message.kind),
            video = if is_video { "video-box" } else { "" },
            thumb = if poster.is_empty() { "" } else { "has-thumb" },
        )
    }

    fn media_element(&self, kind: &str, url: &str, message: &MessageRow) -> String {
        match kind {
            "audio" => self.audio_player(url, message),
            "video" => {
                let id = escape(&message.id);
                let poster = self
                    .req
                    .posters
                    .get(&message.id)
                    .or_else(|| self.req.thumbs.get(&message.id))
                    .cloned()
                    .unwrap_or_default();
                let poster_attr = if poster.is_empty() {
                    String::new()
                } else {
                    format!(r#" poster="{poster}""#)
                };
                let seconds = self.req.durations.get(&message.id).copied().unwrap_or(0);
                let duration = if seconds > 0 {
                    format_duration(seconds)
                } else {
                    String::new()
                };
                format!(
                    r#"<div class="media-video-wrap" data-media-id="{id}" data-media-kind="video" data-thumb-id="{id}"><video class="media-video" data-media-id="{id}" src="{}"{poster_attr} playsinline preload="metadata"></video><span class="media-video-play">{}</span><span class="media-video-duration" data-duration-id="{id}">{duration}</span></div>"#,
                    escape(url),
                    icon("play_arrow")
                )
            }
            _ => format!(
                r#"<a class="media-image" href="{}" target="_blank" rel="noreferrer" data-media-id="{}" data-media-kind="image"><img src="{}" alt="{}" /></a>"#,
                escape(url),
                escape(&message.id),
                escape(url),
                escape(media_label(kind).unwrap_or("Media"))
            ),
        }
    }

    fn audio_player(&self, url: &str, message: &MessageRow) -> String {
        let audio = message.audio.clone().unwrap_or_default();
        let ptt = audio.ptt;
        let seconds = audio.seconds.unwrap_or(0);
        let wave = if ptt {
            build_wave_bars(&audio.waveform)
        } else {
            String::new()
        };
        let dim = if wave.is_empty() {
            String::new()
        } else {
            format!(r#"<div class="audio-wave dim">{wave}</div>"#)
        };
        let lit = if wave.is_empty() {
            String::new()
        } else {
            format!(r#"<div class="audio-wave lit" data-audio-wave>{wave}</div>"#)
        };
        let stamp = format!(
            r#"<span class="meta audio-stamp">{}{}</span>"#,
            self.time(message.timestamp),
            tick_html(message, self.self_chat)
        );
        let duration = if seconds > 0 {
            format_duration(seconds)
        } else {
            String::new()
        };
        let id = escape(&message.id);
        format!(
            r#"<div class="audio-player{ptt_class}" data-audio-id="{id}">
      <div class="audio-row">
        <button type="button" class="audio-play" aria-label="Play audio">
          <svg class="glyph glyph-play" viewBox="0 -960 960 960"><path d="M320-200v-560l440 280-440 280Zm80-280Zm0 134 210-134-210-134v268Z"/></svg>
          <svg class="glyph glyph-pause" viewBox="0 -960 960 960"><path d="M520-200v-560h240v560H520Zm-320 0v-560h240v560H200Zm400-80h80v-400h-80v400Zm-320 0h80v-400h-80v400Zm0-400v400-400Zm320 0v400-400Z"/></svg>
        </button>
        <div class="audio-track" data-audio-track>
          <div class="audio-line"><span class="audio-fill" data-audio-fill></span></div>
          {dim}{lit}
          <span class="audio-dot" data-audio-dot></span>
        </div>
        <span class="audio-badge" aria-hidden="true">{headphones}</span>
      </div>
      <div class="audio-foot">
        <span class="audio-dur" data-audio-dur>{duration}</span>
        {stamp}
      </div>
      <audio class="audio-src" src="{url}" preload="metadata" data-audio-id="{id}"></audio>
    </div>"#,
            ptt_class = if ptt { " ptt" } else { "" },
            headphones = icon("headphones"),
            url = escape(url),
        )
    }

    fn document_card(&self, message: &MessageRow) -> String {
        let info = document_info(message);
        let meta = document_meta(message);
        let preview = message
            .document
            .as_ref()
            .and_then(|doc| doc.thumb.clone())
            .or_else(|| self.req.doc_thumbs.get(&message.id).cloned())
            .unwrap_or_default();
        let thumb = if preview.is_empty() {
            String::new()
        } else {
            format!(
                r#"<div class="doc-preview"><img class="doc-thumb" src="{}" alt="" loading="lazy" /></div>"#,
                escape(&preview)
            )
        };
        let id = escape(&message.id);
        format!(
            r#"{thumb}
    <div class="doc-card" data-doc-message="{id}" data-doc-name="{}">
      <div class="doc-main">
        {}
        <div class="doc-text">
          <div class="doc-name" title="{}">{}</div>
          {meta_line}
        </div>
      </div>
    </div>"#,
            escape(&info.name),
            document_badge(&info.ext, ""),
            escape(&info.name),
            escape(&info.name),
            meta_line = if meta.is_empty() {
                String::new()
            } else {
                format!(r#"<div class="doc-meta">{}</div>"#, escape(&meta))
            },
        )
    }

    fn document_actions(&self, message: &MessageRow) -> String {
        let info = document_info(message);
        let id = escape(&message.id);
        let save = format!(
            r#"<button type="button" data-doc-action="save" data-doc-id="{id}" data-doc-name="{}">"#,
            escape(&info.name)
        );
        if !is_previewable(&info.ext) {
            return format!(r#"<div class="doc-actions">{save}Download</button></div>"#);
        }
        format!(
            r#"<div class="doc-actions">
      <button type="button" data-doc-action="view" data-doc-id="{id}">View</button>
      {save}Save as&hellip;</button>
    </div>"#
        )
    }

    fn link_preview(&self, message: &MessageRow) -> String {
        let embedded = message.link.as_ref();
        let has_embedded = embedded
            .map(|link| {
                !link.title.is_empty() || link.thumb.is_some() || !link.description.is_empty()
            })
            .unwrap_or(false);
        if has_embedded {
            let link = embedded.expect("checked above");
            let wants_big = link.thumb_path.is_some() && self.req.chat.is_channel;
            let attrs = if wants_big {
                format!(r#" data-thumb-id="{}""#, escape(&message.id))
            } else {
                String::new()
            };
            return format!(
                r#"<div class="link-preview ready"{attrs}>{}</div>"#,
                preview_card(
                    &link.url,
                    &link.title,
                    &link.description,
                    &crate::text::site_of(&link.url),
                    link.thumb.as_deref()
                )
            );
        }
        match crate::text::first_url(&message.text) {
            Some(url) => format!(
                r#"<div class="link-preview" data-preview-url="{}"></div>"#,
                escape(&url)
            ),
            None => String::new(),
        }
    }

    fn caption(&self, message: &MessageRow) -> String {
        if message.text.is_empty() {
            return String::new();
        }
        let html = self.body(message);
        if !self.req.chat.is_channel || message.text.chars().count() < 420 {
            return format!(r#"<div class="body">{html}</div>"#);
        }
        format!(
            r#"<div class="body clamp">{html}</div><button class="channel-more" type="button">Read more</button>"#
        )
    }

    fn channel_stats(&self, message: &MessageRow) -> String {
        let Some(stats) = message.stats.as_ref() else {
            return String::new();
        };
        let pills: String = stats
            .reactions
            .iter()
            .take(3)
            .map(|reaction| {
                format!(
                    r#"<span class="channel-reaction">{}<b>{}</b></span>"#,
                    escape(&reaction.code),
                    compact_count(reaction.count)
                )
            })
            .collect();
        let total = if stats.reaction_total > 0 {
            stats.reaction_total
        } else {
            stats.reactions.iter().map(|reaction| reaction.count).sum()
        };
        let reactions = if pills.is_empty() {
            String::new()
        } else {
            let total_html = if total > 0 {
                format!(
                    r#"<span class="channel-reaction-total">{}</span>"#,
                    compact_count(total)
                )
            } else {
                String::new()
            };
            format!(
                r#"<span class="channel-reaction-group" title="Reactions">{pills}{total_html}</span>"#
            )
        };
        let forwards = if stats.forwards > 0 {
            format!(
                r#"<span class="channel-forward" title="Forwarded">{}{}</span>"#,
                r#"<svg viewBox="0 -960 960 960" aria-hidden="true"><path d="m640-280-57-56 184-184-184-184 57-56 240 240-240 240ZM80-200v-160q0-83 58.5-141.5T280-560h247L383-704l57-56 240 240-240 240-57-56 144-144H280q-50 0-85 35t-35 85v160H80Z"/></svg>"#,
                compact_count(stats.forwards)
            )
        } else {
            String::new()
        };
        let views = if stats.views > 0 {
            format!(
                r#"<span class="channel-views" title="Views">{}</span>"#,
                compact_count(stats.views)
            )
        } else {
            String::new()
        };
        if reactions.is_empty() && forwards.is_empty() && views.is_empty() {
            return String::new();
        }
        format!(
            r#"<div class="channel-stats" data-for="{}">{reactions}{forwards}{views}</div>"#,
            escape(&message.id)
        )
    }
}

/* --------------------------------------------------------------- metadata */

fn media_label(kind: &str) -> Option<&'static str> {
    match kind {
        "image" => Some("Photo"),
        "video" => Some("Video"),
        "audio" => Some("Audio"),
        "document" => Some("Document"),
        "sticker" => Some("Sticker"),
        _ => None,
    }
}

fn media_icon(kind: &str) -> String {
    let name = match kind {
        "image" => "image",
        "video" => "videocam",
        "audio" => "mic",
        "sticker" => "sticker",
        _ => "description",
    };
    icon(name)
}

pub fn format_duration(seconds: i64) -> String {
    let seconds = seconds.max(0);
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

fn build_wave_bars(samples: &[u8]) -> String {
    if samples.is_empty() {
        return String::new();
    }
    let count = samples.len();
    let step = (count.div_ceil(64)).max(1);
    let mut bars = Vec::new();
    let mut index = 0;
    while index < count {
        let end = (index + step).min(count);
        let peak = samples[index..end].iter().copied().max().unwrap_or(0);
        let height = (peak as f32 / 255.0).clamp(0.1, 1.0);
        bars.push(format!(
            r#"<i style="--h:{}%"></i>"#,
            (height * 100.0).round() as i64
        ));
        index += step;
    }
    bars.join("")
}

fn extension_of(name: &str) -> String {
    let name = name.trim();
    let Some(dot) = name.rfind('.') else {
        return String::new();
    };
    let ext = &name[dot + 1..];
    if ext.is_empty() || ext.len() > 8 || !ext.chars().all(|ch| ch.is_ascii_alphanumeric()) {
        return String::new();
    }
    ext.to_lowercase()
}

struct DocInfo {
    name: String,
    ext: String,
}

fn document_info(message: &MessageRow) -> DocInfo {
    let doc = message.document.clone().unwrap_or_default();
    let mut name = if !doc.name.is_empty() {
        doc.name.clone()
    } else {
        message.text.clone()
    }
    .trim()
    .to_string();
    let mut ext = extension_of(&name);
    if ext.is_empty() {
        ext = mime_extension(&doc.mime);
        if !ext.is_empty() {
            if name.is_empty() {
                name = "document".to_string();
            }
            name = format!("{name}.{ext}");
        }
    }
    DocInfo {
        name: if name.is_empty() {
            "Document".to_string()
        } else {
            name
        },
        ext,
    }
}

fn mime_extension(mime: &str) -> String {
    let mime = mime.to_lowercase();
    let ext = match mime.as_str() {
        "application/pdf" => "pdf",
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/webp" => "webp",
        "image/gif" => "gif",
        "application/msword" => "doc",
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => "docx",
        "application/vnd.ms-excel" => "xls",
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" => "xlsx",
        "application/vnd.ms-powerpoint" => "ppt",
        "application/vnd.openxmlformats-officedocument.presentationml.presentation" => "pptx",
        "application/zip" => "zip",
        "text/csv" => "csv",
        "text/plain" => "txt",
        _ => "",
    };
    ext.to_string()
}

fn doc_tone(ext: &str) -> &'static str {
    match ext {
        "pdf" => "pdf",
        "doc" | "docx" | "odt" | "rtf" | "pages" => "word",
        "xls" | "xlsx" | "csv" | "ods" => "sheet",
        "ppt" | "pptx" | "odp" => "slides",
        "zip" | "rar" | "7z" | "gz" | "tar" => "archive",
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "heic" => "image",
        "vsd" | "vsdx" | "svg" | "ai" | "dwg" | "dxf" => "vector",
        "mp3" | "wav" | "m4a" | "ogg" => "audio",
        "mp4" | "mov" | "mkv" | "avi" => "video",
        _ => "plain",
    }
}

fn is_previewable(ext: &str) -> bool {
    matches!(
        ext,
        "pdf"
            | "png"
            | "jpg"
            | "jpeg"
            | "gif"
            | "webp"
            | "bmp"
            | "txt"
            | "csv"
            | "log"
            | "md"
            | "json"
            | "xml"
    )
}

fn format_bytes(bytes: u64) -> String {
    if bytes == 0 {
        return String::new();
    }
    let units = ["B", "kB", "MB", "GB"];
    let mut size = bytes as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < units.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    let decimals = if size >= 100.0 || unit == 0 { 0 } else { 1 };
    format!("{size:.decimals$} {}", units[unit])
}

fn document_meta(message: &MessageRow) -> String {
    let doc = message.document.clone().unwrap_or_default();
    let info = document_info(message);
    let mut parts: Vec<String> = Vec::new();
    if doc.pages > 0 {
        parts.push(if doc.pages == 1 {
            "1 page".to_string()
        } else {
            format!("{} pages", doc.pages)
        });
    }
    if !info.ext.is_empty() {
        parts.push(info.ext.to_uppercase());
    }
    let size = format_bytes(doc.size);
    if !size.is_empty() {
        parts.push(size);
    }
    parts.join(" · ")
}

fn document_badge(ext: &str, extra: &str) -> String {
    let tone = doc_tone(ext);
    let label = if ext.is_empty() {
        "FILE".to_string()
    } else {
        ext.to_uppercase().chars().take(5).collect()
    };
    format!(
        r#"<span class="doc-badge doc-{tone} {extra}">{}</span>"#,
        escape(&label)
    )
}

fn preview_card(
    url: &str,
    title: &str,
    description: &str,
    site: &str,
    image: Option<&str>,
) -> String {
    let img = match image {
        Some(image) if !image.is_empty() => format!(
            r#"<img class="preview-img" src="{}" alt="" loading="lazy" referrerpolicy="no-referrer" onerror="this.remove()" />"#,
            escape(image)
        ),
        _ => String::new(),
    };
    let heading = if title.is_empty() { url } else { title };
    let desc = if description.is_empty() {
        String::new()
    } else {
        format!(
            r#"<span class="preview-desc">{}</span>"#,
            escape(description)
        )
    };
    format!(
        r#"<a class="preview-card" href="{}" target="_blank" rel="noreferrer">
      {img}
      <div class="preview-body">
        <strong class="preview-title">{}</strong>
        {desc}
        <span class="preview-site">{}</span>
      </div>
    </a>"#,
        escape(url),
        escape(heading),
        escape(if site.is_empty() { "Link" } else { site })
    )
}

/* ------------------------------------------------------------------ render */

/// Consecutive photos/videos from the same person collapse into one album.
fn album_at(messages: &[&MessageRow], start: usize) -> Option<Vec<MessageRow>> {
    let first = messages.get(start)?;
    if first.deleted || (first.kind != "image" && first.kind != "video") {
        return None;
    }
    let mut items = vec![(*first).clone()];
    let mut index = start + 1;
    while index < messages.len() && items.len() < 9 {
        let next = messages[index];
        if next.deleted || (next.kind != "image" && next.kind != "video") {
            break;
        }
        if next.from_me != first.from_me || next.sender != first.sender {
            break;
        }
        if next.timestamp - items.last().map(|m| m.timestamp).unwrap_or(0) > 4000 {
            break;
        }
        items.push((*next).clone());
        index += 1;
    }
    if items.len() >= 2 { Some(items) } else { None }
}

fn bubble_classes(
    message: &MessageRow,
    ctx: &Ctx<'_>,
    first_of_run: bool,
    extra: &[&str],
) -> String {
    let mut classes = vec!["bubble".to_string()];
    if message.from_me {
        classes.push("out".into());
    }
    if message.forwarded {
        classes.push("is-forwarded".into());
    }
    if ctx.req.selecting && ctx.selected.contains(&message.id) {
        classes.push("selected".into());
    }
    if !message.reactions.is_empty() {
        classes.push("has-reactions".into());
    }
    if first_of_run {
        classes.push("first".into());
    }
    for class in extra {
        classes.push((*class).to_string());
    }
    classes.join(" ")
}

pub fn render(req: &ChatRequest) -> ChatOutput {
    let self_chat = is_self_jid(&req.chat.jid, &req.me_jid, &req.me_lid);
    let starred: HashSet<String> = req.starred.iter().cloned().collect();
    let selected: HashSet<String> = req.selected.iter().cloned().collect();
    let ctx = Ctx {
        req,
        self_chat,
        starred: &starred,
        selected: &selected,
    };

    let subtitle = if req.chat.is_channel {
        format_followers(req.chat.subscribers)
    } else {
        crate::text::chat_subtitle(
            &req.chat.jid,
            req.chat.is_group,
            &req.me_jid,
            &req.me_lid,
            &req.known_phone,
        )
    };

    let visible: Vec<&MessageRow> = req
        .messages
        .iter()
        .filter(|message| message.is_visible(req.show_deleted))
        .collect();

    if visible.is_empty() {
        let empty_html = if req.chat.is_channel {
            r#"<p class="empty">No posts synced from this channel yet. WhatsApp only shares channel posts received while this device is linked — use the import button in the header to load an exported <em>.txt</em> of its posts.</p>"#.to_string()
        } else {
            r#"<p class="empty">No messages loaded for this chat yet. New messages will appear here.</p>"#
                .to_string()
        };
        return ChatOutput {
            html: String::new(),
            empty_html,
            subtitle,
            queue: Vec::new(),
            count: 0,
        };
    }

    let mut html = String::new();
    let mut queue: Vec<String> = Vec::new();
    let mut last_day = String::new();
    let mut previous: Option<MessageRow> = None;
    let mut index = 0usize;

    while index < visible.len() {
        let message = visible[index];
        let day = day_of(message.timestamp, req.now);
        if day != last_day {
            html.push_str(&format!(r#"<div class="day">{}</div>"#, escape(&day)));
            last_day = day;
            previous = None;
        }

        let same_run = match previous.as_ref() {
            Some(previous) => {
                previous.from_me == message.from_me
                    && previous.sender == message.sender
                    && message.timestamp - previous.timestamp < 5 * 60_000
            }
            None => false,
        };
        let first_of_run = !same_run;

        if let Some(album) = album_at(&visible, index) {
            let last = album.last().expect("album is not empty");
            let caption = album
                .iter()
                .rev()
                .find(|item| !item.text.trim().is_empty())
                .cloned();
            let size = album.len().min(4);
            let size_class = format!("album-{size}");
            let extra = vec!["album", size_class.as_str()];
            let classes = bubble_classes(last, &ctx, first_of_run, &extra);

            let cells: String = album
                .iter()
                .take(4)
                .enumerate()
                .map(|(cell, item)| {
                    let more = if cell == 3 && album.len() > 4 {
                        format!(r#"<span class="album-more">+{}</span>"#, album.len() - 4)
                    } else {
                        String::new()
                    };
                    format!(
                        r#"<div class="album-cell">{}{more}</div>"#,
                        ctx.media_block(item)
                    )
                })
                .collect();
            let caption_html = match caption {
                Some(item) => format!(r#"<div class="body">{}</div>"#, ctx.body(&item)),
                None => String::new(),
            };
            let select_tick = if req.selecting {
                r#"<span class="select-tick"></span>"#
            } else {
                ""
            };
            let avatar = if first_of_run {
                ctx.avatar_html(message)
            } else {
                String::new()
            };
            let sender = if first_of_run {
                ctx.sender_html(message)
            } else {
                String::new()
            };
            html.push_str(&format!(
                r#"
        <div class="{classes}" data-id="{}">
          {}
          {select_tick}
          {avatar}
          {sender}
          {}
          <div class="album-grid album-{size}">{cells}</div>
          {caption_html}
          {}
          <span class="meta">{}{}</span>
        </div>"#,
                escape(&message.id),
                if last.forwarded {
                    banner_forwarded()
                } else {
                    String::new()
                },
                last.reply_to
                    .as_ref()
                    .map(|reply| ctx.reply_quote(reply))
                    .unwrap_or_default(),
                ctx.reactions(last),
                ctx.time(last.timestamp),
                tick_html(last, self_chat),
            ));
            previous = Some(last.clone());
            index += album.len();
            continue;
        }

        let mut extra: Vec<&str> = Vec::new();
        let body;
        let mut doc_actions = String::new();
        let mut owns_meta = false;

        if message.deleted {
            extra.push("deleted");
            let revealed = if req.show_deleted && !message.text.is_empty() {
                Some(message.text.clone())
            } else {
                None
            };
            body = match revealed {
                Some(text) => format!(
                    r#"<div class="body">{}</div><span class="deleted-tag">deleted</span>"#,
                    ctx.body_text(&text)
                ),
                None => r#"<div class="body muted">This message was deleted</div>"#.to_string(),
            };
        } else if message.kind == "document" {
            extra.push("has-media");
            extra.push("doc-bubble");
            let info = document_info(message);
            let caption_text = message.text.trim().to_string();
            let caption = if !caption_text.is_empty() && caption_text != info.name {
                format!(
                    r#"<div class="body doc-caption">{}</div>"#,
                    ctx.body(message)
                )
            } else {
                String::new()
            };
            body = format!(
                r#"{}
        <div class="doc-footer">{caption}<span class="meta doc-time">{}{}</span></div>"#,
                ctx.document_card(message),
                ctx.time(message.timestamp),
                tick_html(message, self_chat)
            );
            doc_actions = ctx.document_actions(message);
            owns_meta = true;
        } else if media_label(&message.kind).is_some() {
            let hidden = message.once_recovered && !req.show_deleted;
            if hidden {
                extra.push("view-once-bubble");
            } else {
                extra.push("has-media");
                if message.kind == "sticker" {
                    extra.push("sticker");
                }
            }
            let caption = if message.text.is_empty() {
                String::new()
            } else {
                ctx.caption(message)
            };
            body = if hidden {
                view_once_placeholder(Some(&message.id))
            } else {
                format!("{}{caption}", ctx.media_block(message))
            };
            if message.kind == "audio" {
                extra.push("voice");
                owns_meta = true;
            }
        } else if !message.text.is_empty() {
            if is_emoji_only(&message.text) {
                extra.push("emoji-only");
            }
            body = format!("{}{}", ctx.link_preview(message), ctx.caption(message));
        } else if message.kind == "view_once" {
            extra.push("unsupported");
            extra.push("view-once-bubble");
            body = view_once_placeholder(None);
        } else {
            extra.push("unsupported");
            body = r#"<div class="body muted">Unsupported message</div>"#.to_string();
        }

        if message.status_quote {
            extra.push("status-interaction");
        }
        let quoted = match message.reply_to.as_ref() {
            Some(reply) => {
                if message.status_quote {
                    ctx.status_quote(reply)
                } else {
                    ctx.reply_quote(reply)
                }
            }
            None => String::new(),
        };

        let classes = bubble_classes(message, &ctx, first_of_run, &extra);
        let avatar = if first_of_run {
            ctx.avatar_html(message)
        } else {
            String::new()
        };
        let sender = if first_of_run {
            ctx.sender_html(message)
        } else {
            String::new()
        };
        let meta = if owns_meta {
            String::new()
        } else {
            let edited = if message.edited {
                r#"<span class="edited">edited</span>"#
            } else {
                ""
            };
            let star = if ctx.starred.contains(&message.id) {
                format!(
                    r#"<span class="msg-star" title="Starred">{}</span>"#,
                    icon("star")
                )
            } else {
                String::new()
            };
            format!(
                r#"<span class="meta">{edited}{star}<span>{}</span>{}</span>"#,
                ctx.time(message.timestamp),
                tick_html(message, self_chat)
            )
        };
        let select_tick = if req.selecting {
            r#"<span class="select-tick"></span>"#
        } else {
            ""
        };
        html.push_str(&format!(
            r#"
      <div class="{classes}" data-id="{}">
        {}
        {select_tick}
        {avatar}
        {sender}
        {quoted}
        {body}
        {}
        {meta}
        {doc_actions}
      </div>"#,
            escape(&message.id),
            if message.forwarded {
                banner_forwarded()
            } else {
                String::new()
            },
            ctx.reactions(message),
        ));
        if req.chat.is_channel {
            html.push_str(&ctx.channel_stats(message));
        }

        if req.chat.is_group && !message.from_me && !message.sender.is_empty() {
            queue.push(message.sender.clone());
        }
        previous = Some(message.clone());
        index += 1;
    }

    ChatOutput {
        html,
        empty_html: String::new(),
        subtitle,
        queue,
        count: visible.len(),
    }
}

/* -------------------------------------------------------------- sub-structs */

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ReplyRow {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub sender: String,
    #[serde(default)]
    pub sender_name: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub thumb: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct AudioRow {
    #[serde(default)]
    pub ptt: bool,
    #[serde(default)]
    pub seconds: Option<i64>,
    #[serde(default)]
    pub waveform: Vec<u8>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ChannelStatsRow {
    #[serde(default)]
    pub reactions: Vec<ChannelReactionRow>,
    #[serde(default)]
    pub reaction_total: i64,
    #[serde(default)]
    pub forwards: i64,
    #[serde(default)]
    pub views: i64,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ChannelReactionRow {
    #[serde(default)]
    pub code: String,
    #[serde(default)]
    pub count: i64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn message(id: &str, text: &str, ts: i64) -> MessageRow {
        MessageRow {
            id: id.into(),
            text: text.into(),
            kind: "text".into(),
            timestamp: ts,
            ..Default::default()
        }
    }

    fn make_request(messages: Vec<MessageRow>) -> ChatRequest {
        ChatRequest {
            chat: ChatInfo {
                jid: "6285@s.whatsapp.net".into(),
                name: "Ada".into(),
                ..Default::default()
            },
            messages,
            media: HashMap::new(),
            thumbs: HashMap::new(),
            posters: HashMap::new(),
            durations: HashMap::new(),
            doc_thumbs: HashMap::new(),
            photos: HashMap::new(),
            contacts: HashMap::new(),
            phones: HashMap::new(),
            sender_names: HashMap::new(),
            me: "Me".into(),
            me_jid: "62111@s.whatsapp.net".into(),
            me_lid: String::new(),
            self_handle: "@me".into(),
            known_phone: String::new(),
            show_deleted: false,
            selecting: false,
            selected: Vec::new(),
            starred: Vec::new(),
            now: 1_770_000_000_000,
        }
    }

    #[test]
    fn bubbles_carry_markup_and_ticks() {
        let mut own = message("m1", "*hi* there", 1_770_000_000_000);
        own.from_me = true;
        own.status = "read".into();
        let request = make_request(vec![own]);
        let out = render(&request);
        assert!(out.html.contains(r#"<div class="day">"#));
        assert!(out.html.contains(r#"data-id="m1""#));
        assert!(out.html.contains("<b>hi</b> there"));
        assert!(out.html.contains(r#"class="tick read""#));
        assert!(
            out.html.contains("8:44") || out.html.contains(":"),
            "clock is rendered"
        );
        assert_eq!(out.count, 1);
    }

    #[test]
    fn first_of_run_and_deleted() {
        let mut a = message("a", "one", 1_770_000_000_000);
        let mut b = message("b", "two", 1_770_000_000_100);
        b.from_me = false;
        a.from_me = false;
        let mut request = make_request(vec![a, b]);
        request.chat.is_group = true;
        request.messages[0].sender = "6287@s.whatsapp.net".into();
        request.messages[0].sender_name = "Budi".into();
        request.messages[1].sender = "6287@s.whatsapp.net".into();
        request.messages[1].sender_name = "Budi".into();
        let out = render(&request);
        assert_eq!(out.html.matches("class=\"bubble first").count(), 1);
        assert!(out.html.contains("~ Budi"));
        assert!(out.queue.contains(&"6287@s.whatsapp.net".to_string()));

        let mut gone = message("g", "bye", 1_770_000_000_000);
        gone.deleted = true;
        let request = make_request(vec![gone]);
        let out = render(&request);
        assert!(out.html.contains("This message was deleted"));

        let mut hidden = message("h", "secret", 1_770_000_000_000);
        hidden.cleared = true;
        let out = render(&request_db(hidden));
        assert_eq!(out.count, 0);
    }

    fn request_db(message: MessageRow) -> ChatRequest {
        make_request(vec![message])
    }

    #[test]
    fn media_blocks_track_download_state() {
        let mut photo = message("p", "", 1_770_000_000_000);
        photo.kind = "image".into();
        let mut request = make_request(vec![photo]);
        let loading = render(&request);
        assert!(loading.html.contains(r#"class="media media-box"#));
        assert!(loading.html.contains("Loading photo..."));

        request.media.insert(
            "p".into(),
            MediaState {
                status: "ready".into(),
                url: "/media/p.jpg".into(),
            },
        );
        let ready = render(&request);
        assert!(ready.html.contains(r#"class="media-image""#));
        assert!(ready.html.contains(r#"src="/media/p.jpg""#));
        assert!(ready.html.contains(r#"data-media-kind="image""#));

        request.media.get_mut("p").unwrap().status = "error".into();
        let failed = render(&request);
        assert!(failed.html.contains("data-failed=\"1\""));
        assert!(failed.html.contains("Tap to retry"));
    }

    #[test]
    fn albums_collapse_consecutive_media() {
        let mut a = message("a", "", 1_000);
        a.kind = "image".into();
        let mut b = message("b", "", 2_000);
        b.kind = "video".into();
        let mut c = message("c", "caption", 3_000);
        c.kind = "image".into();
        let mut request = make_request(vec![a, b, c]);
        for id in ["a", "b", "c"] {
            request.media.insert(
                id.into(),
                MediaState {
                    status: "ready".into(),
                    url: format!("/media/{id}"),
                },
            );
        }
        let out = render(&request);
        assert!(out.html.contains("album-grid album-3"));
        assert_eq!(out.html.matches("album-cell").count(), 3);
        assert!(out.html.contains("caption"));
    }

    #[test]
    fn audio_player_and_documents() {
        let mut voice = message("v", "", 1_770_000_000_000);
        voice.kind = "audio".into();
        voice.audio = Some(AudioRow {
            ptt: true,
            seconds: Some(17),
            waveform: vec![10, 200, 90, 255],
        });
        let mut request = make_request(vec![voice]);
        request.media.insert(
            "v".into(),
            MediaState {
                status: "ready".into(),
                url: "/media/v.ogg".into(),
            },
        );
        let out = render(&request);
        assert!(out.html.contains("audio-player ptt"));
        assert!(out.html.contains("data-audio-wave"));
        assert!(out.html.contains("0:17"));
        assert!(
            out.html
                .contains(r#"<audio class="audio-src" src="/media/v.ogg""#)
        );

        let mut doc = message("d", "", 1_770_000_000_000);
        doc.kind = "document".into();
        doc.document = Some(crate::text::DocumentRow {
            name: "report.pdf".into(),
            mime: "application/pdf".into(),
            size: 2_500_000,
            pages: 0,
            thumb: None,
        });
        let request = make_request(vec![doc]);
        let out = render(&request);
        assert!(out.html.contains("doc-badge doc-pdf"));
        assert!(out.html.contains("report.pdf"));
        assert!(out.html.contains("2.4 MB"));
        assert!(out.html.contains("data-doc-action=\"view\""));
    }

    #[test]
    fn quotes_reactions_and_stars() {
        let mut reply = message("r", "answer", 1_770_000_000_000);
        reply.reply_to = Some(ReplyRow {
            id: "q".into(),
            sender: "6287@s.whatsapp.net".into(),
            sender_name: "Budi".into(),
            text: "the question".into(),
            kind: "text".into(),
            thumb: None,
        });
        reply.reactions = vec![
            crate::text::ReactionRow {
                emoji: "👍".into(),
                from_me: true,
            },
            crate::text::ReactionRow {
                emoji: "👍".into(),
                from_me: false,
            },
        ];
        let mut request = make_request(vec![reply]);
        request.starred = vec!["r".into()];
        let out = render(&request);
        assert!(out.html.contains(r#"class="quote""#));
        assert!(out.html.contains("the question"));
        assert!(out.html.contains("reaction-pill"));
        assert!(out.html.contains("👍 2"));
        assert!(out.html.contains("msg-star"));
        assert!(out.html.contains("has-reactions"));
    }

    #[test]
    fn link_previews_and_channel_stats() {
        let mut post = message("p", "see https://a.test", 1_770_000_000_000);
        post.link = Some(crate::text::LinkRow {
            title: "A site".into(),
            description: "about".into(),
            url: "https://a.test".into(),
            thumb: Some("data:x".into()),
            thumb_path: None,
        });
        let mut request = make_request(vec![post]);
        request.chat.is_channel = true;
        request.chat.subscribers = 84_300;
        request.messages[0].stats = Some(ChannelStatsRow {
            reactions: vec![ChannelReactionRow {
                code: "❤".into(),
                count: 1200,
            }],
            reaction_total: 1200,
            forwards: 3,
            views: 40_000,
        });
        let out = render(&request);
        assert!(out.html.contains("preview-card"));
        assert!(out.html.contains("A site"));
        assert_eq!(out.subtitle, "84.3k followers");
        assert!(out.html.contains("channel-stats"));
        assert!(out.html.contains("1.2k"));
        assert!(out.html.contains("40k"));
    }

    #[test]
    fn unsupported_and_view_once() {
        let mut once = message("o", "", 1_770_000_000_000);
        once.kind = "view_once".into();
        let out = render(&make_request(vec![once]));
        assert!(out.html.contains("view-once-bubble"));
        assert!(out.html.contains("You received a view once message"));

        let mut other = message("x", "", 1_770_000_000_000);
        other.kind = "other".into();
        let out = render(&make_request(vec![other]));
        assert!(out.html.contains("Unsupported message"));
    }

    #[test]
    fn counters_and_durations() {
        assert_eq!(compact_count(999), "999");
        assert_eq!(compact_count(1500), "1.5k");
        assert_eq!(compact_count(2_000_000), "2m");
        assert_eq!(format_duration(17), "0:17");
        assert_eq!(format_duration(605), "10:05");
        assert!(is_emoji_only("😀😀"));
        assert!(!is_emoji_only("hi 😀"));
        assert_eq!(extension_of("a.PDF"), "pdf");
        assert_eq!(format_bytes(2_500_000), "2.4 MB");
    }
}
