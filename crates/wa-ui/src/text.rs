//! Text logic ported from the frontend's `chat-utils.js` and `core.js`:
//! previews, phone formatting, WhatsApp inline markup, mentions, links and
//! sender colours. Pure functions with unit tests.

use std::collections::HashMap;
use std::sync::OnceLock;

use regex::Regex;
use serde::Deserialize;

use crate::html::{escape, icon};

/* ------------------------------------------------------------------ jids */

/// The user part of a JID, without the device suffix:
/// `"6285..:53@s.whatsapp.net"` -> `"6285.."`.
pub fn phone_from_jid(jid: &str) -> String {
    if jid.is_empty() {
        return String::new();
    }
    let user = jid.split('@').next().unwrap_or("");
    user.split(':').next().unwrap_or("").to_string()
}

/// A chat whose display name is still the bare JID user part.
pub fn is_placeholder_name(jid: &str, name: &str) -> bool {
    name == jid.split('@').next().unwrap_or("")
}

/// The account's own conversation.
pub fn is_self_jid(jid: &str, me_jid: &str, me_lid: &str) -> bool {
    if jid.is_empty() {
        return false;
    }
    let user = phone_from_jid(jid);
    [me_jid, me_lid]
        .iter()
        .filter(|value| !value.is_empty())
        .any(|value| phone_from_jid(value) == user)
}

/* ------------------------------------------------------------- phone text */

/// Two-digit country codes; `1` and `7` are one digit and everything else
/// falls back to three (`+254`, `+998`, ...).
const CC2: [&str; 38] = [
    "20", "27", "30", "31", "32", "33", "34", "36", "39", "40", "41", "43", "44", "45", "46", "47",
    "48", "49", "51", "52", "53", "54", "55", "56", "57", "58", "60", "61", "62", "63", "64", "65",
    "66", "81", "82", "84", "86", "90",
];

fn cc2(code: &str) -> bool {
    CC2.contains(&code)
}

fn group_phone(rest: &str) -> String {
    let chars: Vec<char> = rest.chars().collect();
    let mut groups = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        let end = (index + 4).min(chars.len());
        groups.push(chars[index..end].iter().collect::<String>());
        index += 4;
    }
    groups.join("-")
}

/// Phone-number user part -> `+62 896-3384-66984`.
pub fn format_phone(phone: &str) -> String {
    let digits: String = phone.chars().filter(|ch| ch.is_ascii_digit()).collect();
    if digits.is_empty() {
        return String::new();
    }
    let mut cc_length = 3;
    if digits.starts_with('1') || digits.starts_with('7') {
        cc_length = 1;
    } else if cc2(&digits.chars().take(2).collect::<String>()) {
        cc_length = 2;
    }
    let chars: Vec<char> = digits.chars().collect();
    let cc: String = chars[..cc_length.min(chars.len())].iter().collect();
    let rest: String = chars[cc_length.min(chars.len())..].iter().collect();
    if rest.is_empty() {
        return format!("+{cc}");
    }
    if cc == "1" {
        let part = |a: usize, b: usize| -> String {
            let slice: Vec<char> = rest.chars().collect();
            let end = b.min(slice.len());
            if a >= end {
                String::new()
            } else {
                slice[a..end].iter().collect()
            }
        };
        return format!(
            "+1 {}-{}-{}",
            part(0, 3),
            part(3, 6),
            part(6, rest.chars().count())
        );
    }
    if cc == "62" || cc == "86" {
        let slice: Vec<char> = rest.chars().collect();
        let part = |a: usize, b: usize| -> String {
            let end = b.min(slice.len());
            if a >= end {
                String::new()
            } else {
                slice[a..end].iter().collect()
            }
        };
        let joined: Vec<String> = [part(0, 3), part(3, 7), part(7, slice.len())]
            .into_iter()
            .filter(|value| !value.is_empty())
            .collect();
        return format!("+{cc} {}", joined.join("-"));
    }
    format!("+{cc} {}", group_phone(&rest))
}

/// Chat header subtitle: `Message yourself`, `Group chat`, or the phone number.
pub fn chat_subtitle(jid: &str, is_group: bool, me_jid: &str, me_lid: &str, phone: &str) -> String {
    if is_group {
        return "Group chat".to_string();
    }
    if !jid.is_empty() && (jid == me_jid || jid == me_lid) {
        return "Message yourself".to_string();
    }
    let mut raw = phone.to_string();
    if raw.is_empty() && !jid.is_empty() && !jid.ends_with("@lid") {
        raw = phone_from_jid(jid);
    }
    if raw.is_empty() {
        String::new()
    } else {
        format_phone(&raw)
    }
}

/* ------------------------------------------------------------------ colour */

/// The official `--WDS-components-profile-photo-content-*` palette.
const SENDER_COLORS: [&str; 11] = [
    "#DBA685", "#53A6FD", "#BDBDBD", "#25D366", "#FC9775", "#FF72A1", "#A791FF", "#FB5061",
    "#53BDEB", "#42C7B8", "#FFD279",
];

/// WhatsApp derives a group participant's colour from their JID.
pub fn sender_color(jid: &str) -> &'static str {
    let mut hash: i32 = 0;
    for ch in jid.chars() {
        hash = hash.wrapping_mul(31).wrapping_add(ch as i32);
    }
    let index = (hash as i64).unsigned_abs() as usize % SENDER_COLORS.len();
    SENDER_COLORS[index]
}

/* ------------------------------------------------------------------- links */

fn find_scheme(text: &str) -> Option<usize> {
    let lower = text.to_ascii_lowercase();
    lower.find("http://").or_else(|| lower.find("https://"))
}

/// First web link in a message body, without trailing punctuation.
pub fn first_url(text: &str) -> Option<String> {
    let start = find_scheme(text)?;
    let rest: Vec<char> = text[start..].chars().collect();
    let mut end = 0;
    while end < rest.len() {
        let ch = rest[end];
        if ch.is_whitespace() || matches!(ch, '<' | '>' | '"' | '\'') {
            break;
        }
        end += 1;
    }
    let url: String = rest[..end].iter().collect();
    let trimmed = url.trim_end_matches([',', ')', '.', ';', '!', '?']);
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// `"https://www.instagram.com/p/x"` -> `"www.instagram.com"`.
pub fn site_of(url: &str) -> String {
    let Some(scheme) = url.find("://") else {
        return String::new();
    };
    let rest = &url[scheme + 3..];
    let authority: String = rest
        .chars()
        .take_while(|ch| !matches!(ch, '/' | '?' | '#'))
        .collect();
    let host = authority.rsplit('@').next().unwrap_or("");
    let host = host.split(':').next().unwrap_or("");
    host.to_ascii_lowercase()
}

/* -------------------------------------------------------------------- misc */

/// `"/ai explain this"` -> `Some("explain this")`; anything else -> `None`.
pub fn ai_prompt(text: &str) -> Option<String> {
    if text.is_empty() {
        return None;
    }
    let trimmed = text.trim();
    let lower = trimmed.to_lowercase();
    if lower != "/ai" && !lower.starts_with("/ai ") {
        return None;
    }
    let rest = trimmed
        .chars()
        .skip(3)
        .collect::<String>()
        .trim()
        .to_string();
    if rest.is_empty() { None } else { Some(rest) }
}

/// `1048576` -> `"1.0 MB"`.
pub fn file_size_label(bytes: u64) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{} KB", (bytes as f64 / 1024.0).round() as u64)
    } else {
        format!("{bytes} B")
    }
}

/// `"@ada_lovelace"` -> `"Ada Lovelace"`; the self chat's `"You"` is dropped.
pub fn display_name_from_handle(name: &str) -> String {
    let clean = name
        .trim_start_matches('@')
        .replace('_', " ")
        .trim()
        .to_string();
    if clean.is_empty()
        || ["you", "me", "unknown", "whatsapp"]
            .iter()
            .any(|word| clean.eq_ignore_ascii_case(word))
    {
        return String::new();
    }
    clean
        .split(' ')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/* ---------------------------------------------------------------- messages */

/// A message reduced to what list/preview rendering needs. Unknown JSON fields
/// are ignored, so the webview can send its full message objects.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct MessageRow {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub text: String,
    /// `"text"`, `"image"`, `"video"`, `"audio"`, `"document"`, `"sticker"`,
    /// `"view_once"`, ...
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub from_me: bool,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub sender: String,
    #[serde(default)]
    pub sender_name: String,
    #[serde(default)]
    pub sender_phone: String,
    #[serde(default)]
    pub deleted: bool,
    #[serde(default)]
    pub deleted_for_me: bool,
    #[serde(default)]
    pub cleared: bool,
    #[serde(default)]
    pub timestamp: i64,
    #[serde(default)]
    pub document: Option<DocumentRow>,
    #[serde(default)]
    pub reactions: Vec<ReactionRow>,
    #[serde(default)]
    pub link: Option<LinkRow>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct DocumentRow {
    #[serde(default)]
    pub name: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ReactionRow {
    #[serde(default)]
    pub emoji: String,
    #[serde(default)]
    pub from_me: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct LinkRow {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub thumb: Option<String>,
}

impl MessageRow {
    fn kind_label(&self) -> Option<&'static str> {
        match self.kind.as_str() {
            "image" => Some("Photo"),
            "video" => Some("Video"),
            "audio" => Some("Audio"),
            "document" => Some("Document"),
            "sticker" => Some("Sticker"),
            "view_once" => Some("View once message"),
            _ => None,
        }
    }

    /// Cleared history and "delete for me" messages are kept but hidden until
    /// the eye toggle is on.
    pub fn is_visible(&self, show_deleted: bool) -> bool {
        if show_deleted {
            return true;
        }
        !(self.cleared || self.deleted_for_me)
    }
}

/// `None` when the message was not deleted, `Some("")` when it is hidden by the
/// eye toggle, or the original text when revealed.
pub fn deleted_message_text(message: &MessageRow, show_deleted: bool) -> Option<String> {
    if !message.deleted {
        return None;
    }
    Some(if show_deleted {
        message.text.clone()
    } else {
        String::new()
    })
}

/// The sidebar preview line for one message.
///
/// `contact_name` resolves a saved contact for a sender JID, which is what the
/// group preview uses for unsaved members' push names.
pub fn preview_of(
    message: &MessageRow,
    is_group: bool,
    show_deleted: bool,
    contact_name: &dyn Fn(&str) -> String,
) -> String {
    let text = if message.deleted {
        let revealed = show_deleted && !message.text.is_empty();
        if revealed {
            message.text.clone()
        } else {
            "This message was deleted".to_string()
        }
    } else if message.kind == "document" {
        message
            .document
            .as_ref()
            .map(|doc| doc.name.clone())
            .filter(|name| !name.is_empty())
            .or_else(|| Some(message.text.clone()).filter(|text| !text.is_empty()))
            .unwrap_or_else(|| "Document".to_string())
    } else if message.kind != "text" && message.kind_label().is_some() {
        let label = message.kind_label().unwrap_or_default();
        if message.text.is_empty() {
            label.to_string()
        } else {
            format!("{label}: {}", message.text)
        }
    } else if !message.text.is_empty() {
        message.text.clone()
    } else {
        "Unsupported message".to_string()
    };

    let who = if message.from_me {
        if is_group {
            "You: ".to_string()
        } else {
            String::new()
        }
    } else if is_group {
        let saved = contact_name(&message.sender);
        let push = if message.sender_name.is_empty() {
            String::new()
        } else {
            format!("~ {}", message.sender_name)
        };
        let label = if !saved.is_empty() {
            saved
        } else if !push.is_empty() {
            push
        } else {
            let phone = format_phone(&message.sender_phone);
            if phone.is_empty() {
                "Unknown".to_string()
            } else {
                phone
            }
        };
        format!("{label}: ")
    } else {
        String::new()
    };
    format!("{who}{text}")
}

/// The sidebar delivery marker for one of our own direct-chat messages: grey
/// single tick, grey double tick once delivered, blue double tick when read.
pub fn preview_tick(message: &MessageRow, is_group: bool) -> String {
    if !message.from_me || is_group {
        return String::new();
    }
    let read = message.status == "read" || message.status == "played";
    let delivered = message.status == "delivered";
    let glyph = icon(if read || delivered {
        "done_all"
    } else {
        "check"
    });
    format!(
        r#"<span class="tick preview-tick{}">{glyph}</span>"#,
        if read { " read" } else { "" }
    )
}

/* ---------------------------------------------------------------- markup */

fn formatting_patterns() -> &'static [(Regex, &'static str, &'static str)] {
    static PATTERNS: OnceLock<Vec<(Regex, &'static str, &'static str)>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        vec![
            (Regex::new(r"```([^`\n]+)```").unwrap(), "<code>", "</code>"),
            (Regex::new(r"\*([^*\n]+)\*").unwrap(), "<b>", "</b>"),
            (Regex::new(r"_([^_\n]+)_").unwrap(), "<i>", "</i>"),
            (Regex::new(r"~([^~\n]+)~").unwrap(), "<s>", "</s>"),
        ]
    })
}

/// WhatsApp inline markup: *bold*, _italic_, ~strike~, ```monospace```.
pub fn apply_whatsapp_formatting(escaped: &str) -> String {
    let mut html = escaped.to_string();
    for (pattern, open, close) in formatting_patterns() {
        let replacement = format!("{open}${{1}}{close}");
        html = pattern
            .replace_all(&html, replacement.as_str())
            .into_owned();
    }
    html
}

fn url_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"https?://\S+").unwrap())
}

fn mention_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"@(\d{5,})(?::\d+)?").unwrap())
}

/// A message body with links, mentions and inline formatting, ready for
/// `innerHTML`. `mentions` is the protocol's `mentionedJid` list and
/// `mention_name` maps a JID user part to a display name.
pub fn body_html(text: &str, mentions: &[String], mention_name: &dyn Fn(&str) -> String) -> String {
    let escaped = escape(text);

    // URLs are masked through the formatting/mention passes so a query string
    // cannot be split by `_italic_`, and restored as one anchor each.
    let mut urls: Vec<String> = Vec::new();
    let masked = url_pattern()
        .replace_all(&escaped, |caps: &regex::Captures<'_>| {
            let url = caps[0].to_string();
            let trimmed = url.trim_end_matches("&quot;").to_string();
            urls.push(trimmed.clone());
            let leftover = "&quot;".repeat((url.len() - trimmed.len()) / 6);
            format!("\u{0}{}\u{0}{leftover}", urls.len() - 1)
        })
        .into_owned();

    let formatted = apply_whatsapp_formatting(&masked);

    let jid_for_digits = |digits: &str| -> String {
        mentions
            .iter()
            .find(|jid| phone_from_jid(jid) == digits)
            .cloned()
            .unwrap_or_default()
    };

    let mut html = mention_pattern()
        .replace_all(&formatted, |caps: &regex::Captures<'_>| {
            let digits = &caps[1];
            let name = mention_name(digits);
            let jid = jid_for_digits(digits);
            let shown = if name.is_empty() { digits } else { &name };
            let attr = if jid.is_empty() {
                String::new()
            } else {
                format!(r#" data-mention-jid="{}""#, escape(&jid))
            };
            format!(r#"<span class="mention"{attr}>@{}</span>"#, escape(shown))
        })
        .into_owned();

    // Forwarded posts carry the push name as plain text plus the mention list.
    for jid in mentions {
        let user = phone_from_jid(jid);
        let name = mention_name(&user);
        if name.is_empty() {
            continue;
        }
        let token = format!("@{}", escape(&name));
        if token.len() <= 1 || !html.contains(&token) {
            continue;
        }
        html = html.replace(
            &token,
            &format!(
                r#"<span class="mention" data-mention-jid="{}">{token}</span>"#,
                escape(jid)
            ),
        );
    }

    // Restore the masked links.
    let restored = url_pattern_marker().replace_all(&html, |caps: &regex::Captures<'_>| {
        let index: usize = caps[1].parse().unwrap_or(0);
        match urls.get(index) {
            Some(url) => {
                format!(r#"<a href="{url}" target="_blank" rel="noreferrer noopener">{url}</a>"#)
            }
            None => caps[0].to_string(),
        }
    });
    restored.into_owned()
}

fn url_pattern_marker() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new("\u{0}(\\d+)\u{0}").unwrap())
}

/// Escape text and highlight every case-insensitive match in the accent green.
pub fn highlight_match(text: &str, needle: &str) -> String {
    if needle.is_empty() {
        return escape(text);
    }
    let lower = text.to_lowercase();
    let needle = needle.to_lowercase();
    let mut html = String::new();
    let mut from = 0usize;
    // Offsets from `to_lowercase` can drift for some scripts; fall back to the
    // whole escaped string when they do not line up.
    if lower.len() != text.len() {
        return escape(text);
    }
    while from < text.len() {
        let Some(at) = lower[from..].find(&needle).map(|at| at + from) else {
            break;
        };
        html.push_str(&escape(&text[from..at]));
        html.push_str(&format!(
            r#"<mark class="search-hit">{}</mark>"#,
            escape(&text[at..at + needle.len()])
        ));
        from = at + needle.len();
    }
    html.push_str(&escape(&text[from..]));
    html
}

/// The mention name the UI resolves today: saved contact, resolved phone, or
/// the last push name seen in the chat.
pub struct MentionResolver<'a> {
    pub me: Option<String>,
    pub me_jid: Option<String>,
    pub me_lid: Option<String>,
    pub contacts: &'a HashMap<String, String>,
    pub phones: &'a HashMap<String, String>,
    pub sender_names: &'a HashMap<String, String>,
    pub self_handle: Option<String>,
}

impl MentionResolver<'_> {
    pub fn resolve(&self, user: &str) -> String {
        let self_user = |jid: &Option<String>| {
            jid.as_ref()
                .map(|value| phone_from_jid(value))
                .unwrap_or_default()
        };
        if self_user(&self.me_jid) == user || self_user(&self.me_lid) == user {
            if let Some(me) = self.me.as_ref().filter(|me| !me.is_empty()) {
                return me.clone();
            }
            return self
                .self_handle
                .as_deref()
                .map(display_name_from_handle)
                .unwrap_or_default();
        }
        let lid = self.phones.get(&format!("{user}@lid"));
        let pn = self.phones.get(&format!("{user}@s.whatsapp.net"));
        let resolved = [lid, pn]
            .into_iter()
            .flatten()
            .find(|value| !value.is_empty() && *value != "missing" && *value != "loading")
            .cloned()
            .unwrap_or_else(|| user.to_string());
        if let Some(name) = self.contacts.get(&resolved) {
            return name.clone();
        }
        if let Some(name) = self.contacts.get(user) {
            return name.clone();
        }
        self.sender_names.get(user).cloned().unwrap_or_default()
    }

    /// The sidebar shows the mention's display name instead of its user id.
    pub fn rewrite_preview_mentions(&self, text: &str) -> String {
        mention_pattern()
            .replace_all(text, |caps: &regex::Captures<'_>| {
                let digits = &caps[1];
                let name = self.resolve(digits);
                if name.is_empty() {
                    caps[0].to_string()
                } else {
                    format!("@{name}")
                }
            })
            .into_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jid_user_part() {
        assert_eq!(phone_from_jid("6285:53@s.whatsapp.net"), "6285");
        assert_eq!(phone_from_jid(""), "");
        assert!(is_placeholder_name("6285@s.whatsapp.net", "6285"));
        assert!(is_self_jid(
            "6285:12@s.whatsapp.net",
            "6285@s.whatsapp.net",
            ""
        ));
    }

    #[test]
    fn phones_format_like_whatsapp() {
        assert_eq!(format_phone("6289633846698"), "+62 896-3384-6698");
        assert_eq!(format_phone("14155552671"), "+1 415-555-2671");
        assert_eq!(format_phone("254712345678"), "+254 7123-4567-8");
        assert_eq!(format_phone("+62 896-3384-6698"), "+62 896-3384-6698");
        assert_eq!(format_phone(""), "");
    }

    #[test]
    fn subtitles() {
        assert_eq!(chat_subtitle("1@g.us", true, "", "", ""), "Group chat");
        assert_eq!(
            chat_subtitle("62@x", false, "62@x", "", ""),
            "Message yourself"
        );
        assert_eq!(
            chat_subtitle("62896@s.whatsapp.net", false, "", "", ""),
            "+62 896"
        );
        assert_eq!(chat_subtitle("1@lid", false, "", "", ""), "");
    }

    #[test]
    fn sender_colors_are_stable() {
        assert_eq!(
            sender_color("6285@s.whatsapp.net"),
            sender_color("6285@s.whatsapp.net")
        );
        assert!(sender_color("x").starts_with('#'));
    }

    #[test]
    fn links() {
        assert_eq!(
            first_url("look https://a.test/x_(y), ok"),
            Some("https://a.test/x_(y".to_string())
        );
        assert_eq!(first_url("nothing here"), None);
        assert_eq!(
            site_of("https://www.instagram.com/p/1"),
            "www.instagram.com"
        );
        assert_eq!(site_of("nope"), "");
    }

    #[test]
    fn misc_helpers() {
        assert_eq!(
            ai_prompt("  /AI explain this "),
            Some("explain this".to_string())
        );
        assert_eq!(ai_prompt("/ai"), None);
        assert_eq!(ai_prompt("hello"), None);
        assert_eq!(file_size_label(2_500_000), "2.4 MB");
        assert_eq!(file_size_label(2048), "2 KB");
        assert_eq!(file_size_label(12), "12 B");
        assert_eq!(display_name_from_handle("@ada_lovelace"), "Ada Lovelace");
        assert_eq!(display_name_from_handle("@you"), "");
    }

    fn msg(kind: &str, text: &str) -> MessageRow {
        MessageRow {
            kind: kind.to_string(),
            text: text.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn previews_match_the_official_rules() {
        let none = |_: &str| String::new();
        assert_eq!(preview_of(&msg("text", "hi"), false, false, &none), "hi");
        assert_eq!(preview_of(&msg("image", ""), false, false, &none), "Photo");
        assert_eq!(
            preview_of(&msg("image", "sunset"), false, false, &none),
            "Photo: sunset"
        );
        let mut own = msg("text", "yo");
        own.from_me = true;
        assert_eq!(preview_of(&own, false, false, &none), "yo");
        assert_eq!(preview_of(&own, true, false, &none), "You: yo");
        let mut member = msg("text", "hey");
        member.sender = "6285@s.whatsapp.net".to_string();
        member.sender_name = "Budi".to_string();
        assert_eq!(preview_of(&member, true, false, &none), "~ Budi: hey");
        let saved = |_: &str| "Budi".to_string();
        assert_eq!(preview_of(&member, true, false, &saved), "Budi: hey");
        let mut deleted = msg("text", "gone");
        deleted.deleted = true;
        assert_eq!(
            preview_of(&deleted, false, false, &none),
            "This message was deleted"
        );
        assert_eq!(preview_of(&deleted, false, true, &none), "gone");
        let mut doc = msg("document", "");
        doc.document = Some(DocumentRow {
            name: "a.pdf".to_string(),
        });
        assert_eq!(preview_of(&doc, false, false, &none), "a.pdf");
    }

    #[test]
    fn ticks_follow_receipts() {
        let mut own = msg("text", "x");
        own.from_me = true;
        assert!(preview_tick(&own, false).contains("preview-tick"));
        assert!(!preview_tick(&own, false).contains(" read"));
        own.status = "read".to_string();
        assert!(preview_tick(&own, false).contains("read"));
        assert_eq!(preview_tick(&own, true), "");
        let mut other = msg("text", "x");
        other.from_me = false;
        assert_eq!(preview_tick(&other, false), "");
    }

    #[test]
    fn body_renders_markup_mentions_and_links() {
        let names = |user: &str| {
            if user == "62896" {
                "Budi".to_string()
            } else {
                String::new()
            }
        };
        let html = body_html(
            "*hi* _there_ @62896",
            &["62896@s.whatsapp.net".to_string()],
            &names,
        );
        assert!(html.contains("<b>hi</b>"), "{html}");
        assert!(html.contains("<i>there</i>"), "{html}");
        assert!(
            html.contains(
                r#"<span class="mention" data-mention-jid="62896@s.whatsapp.net">@Budi</span>"#
            ),
            "{html}"
        );

        let link = body_html("see https://a.test/x_y_z now", &[], &|_| String::new());
        assert_eq!(
            link,
            r#"see <a href="https://a.test/x_y_z" target="_blank" rel="noreferrer noopener">https://a.test/x_y_z</a> now"#
        );

        let quoted = body_html("https://a.test/?q=1\"", &[], &|_| String::new());
        assert!(quoted.contains(r#"href="https://a.test/?q=1""#), "{quoted}");
    }

    #[test]
    fn highlight_is_case_insensitive_and_escaped() {
        assert_eq!(
            highlight_match("Ada & Bob", "bob"),
            r#"Ada &amp; <mark class="search-hit">Bob</mark>"#
        );
        assert_eq!(highlight_match("<x>", ""), "&lt;x&gt;");
    }

    #[test]
    fn mention_resolver_prefers_saved_names() {
        let mut contacts = HashMap::new();
        contacts.insert("62899".to_string(), "Siti".to_string());
        let mut phones = HashMap::new();
        phones.insert("62899@lid".to_string(), "62899".to_string());
        let resolver = MentionResolver {
            me: Some("Me".to_string()),
            me_jid: Some("62111@s.whatsapp.net".to_string()),
            me_lid: None,
            contacts: &contacts,
            phones: &phones,
            sender_names: &HashMap::new(),
            self_handle: None,
        };
        assert_eq!(resolver.resolve("62111"), "Me");
        assert_eq!(resolver.resolve("62899"), "Siti");
        assert_eq!(resolver.rewrite_preview_mentions("hi @62899"), "hi @Siti");
    }
}
