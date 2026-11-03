//! HTML primitives shared by every renderer: escaping, Material icons and
//! avatars. Everything here is pure, so the webview only ever receives strings
//! it can assign to `innerHTML`.

use std::collections::HashMap;

use crate::icons_data::ICONS;

/// Escape text for HTML. Mirrors the frontend's `escapeHtml` byte for byte so
/// rendered snapshots stay comparable while the renderer moves to Rust.
pub fn escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

/// The `<path>` body for a Material Symbols icon, if it is in the set.
pub fn icon_path(name: &str) -> Option<&'static str> {
    ICONS
        .iter()
        .find(|(icon, _)| *icon == name)
        .map(|(_, path)| *path)
}

/// One inline SVG on the Material Symbols 960-unit grid, coloured by
/// `currentColor`. Matches `waIcon(name)` in the UI.
pub fn icon(name: &str) -> String {
    icon_with_class(name, "")
}

pub fn icon_with_class(name: &str, class: &str) -> String {
    let body = icon_path(name).unwrap_or("");
    let class = if class.is_empty() {
        "wa-icon".to_string()
    } else {
        format!("wa-icon {class}")
    };
    format!(r#"<svg class="{class}" viewBox="0 -960 960 960" aria-hidden="true">{body}</svg>"#)
}

/// The official two-person glyph WhatsApp Web draws for a photo-less group.
pub const GROUP_AVATAR_ICON: &str = r#"<svg class="avatar-icon" viewBox="0 -960 960 960" aria-hidden="true"><path d="M40-160v-112q0-34 17.5-62.5T104-378q62-31 126-46.5T360-440q66 0 130 15.5T616-378q29 15 46.5 43.5T680-272v112H40Zm720 0v-120q0-44-24.5-84.5T666-434q51 6 96 20.5t84 35.5q36 20 55 44.5t19 53.5v120H760ZM247-527q-47-47-47-113t47-113q47-47 113-47t113 47q47 47 47 113t-47 113q-47 47-113 47t-113-47Zm466 0q-47 47-113 47-11 0-28-2.5t-28-5.5q27-32 41.5-71t14.5-81q0-42-14.5-81T544-792q14-5 28-6.5t28-1.5q66 0 113 47t47 113q0 66-47 113ZM120-240h480v-32q0-11-5.5-20T580-306q-54-27-109-40.5T360-360q-56 0-111 13.5T140-306q-9 5-14.5 14t-5.5 20v32Zm296.5-343.5Q440-607 440-640t-23.5-56.5Q393-720 360-720t-56.5 23.5Q280-673 280-640t23.5 56.5Q327-560 360-560t56.5-23.5ZM360-240Zm0-400Z"/></svg>"#;

/// The official `--WDS-components-profile-photo-surface-*` /
/// `-content-*` pairs: (circle tint, letter colour).
const AVATAR_COLORS: [(&str, &str); 11] = [
    ("#35271E", "#DBA685"), // brown
    ("#092642", "#53A6FD"), // cobalt
    ("#242626", "#BDBDBD"), // gray
    ("#103529", "#25D366"), // green
    ("#35221E", "#FC9775"), // orange
    ("#36192A", "#FF72A1"), // pink
    ("#242447", "#A791FF"), // purple
    ("#321622", "#FB5061"), // red
    ("#092C3D", "#53BDEB"), // sky blue
    ("#092D2F", "#42C7B8"), // teal
    ("#362C1F", "#FFD279"), // yellow
];

/// The official default-avatar tint pairs, as (background, foreground).
pub fn avatar_palette() -> Vec<(String, String)> {
    AVATAR_COLORS
        .iter()
        .map(|(bg, fg)| ((*bg).to_string(), (*fg).to_string()))
        .collect()
}

/// Deterministic avatar tint for a display name (the official default-avatar
/// palette).
pub fn avatar_style(name: &str) -> String {
    let mut hash: u32 = 0;
    for ch in if name.is_empty() { "?" } else { name }.chars() {
        hash = hash.wrapping_mul(31).wrapping_add(ch as u32);
    }
    let (bg, fg) = AVATAR_COLORS[(hash as usize) % AVATAR_COLORS.len()];
    format!("background:{bg};color:{fg}")
}

/// First letters of a display name, up to two parts.
pub fn initials(name: &str) -> String {
    let clean = if name.trim().is_empty() {
        "?"
    } else {
        name.trim()
    };
    let parts: Vec<&str> = clean
        .split([' ', '.', '_', '-'])
        .filter(|part| !part.is_empty())
        .collect();
    let letters: String = parts
        .iter()
        .take(2)
        .filter_map(|part| part.chars().next())
        .collect();
    let text = if letters.is_empty() {
        clean.chars().next().map(String::from).unwrap_or_default()
    } else {
        letters
    };
    if text.is_empty() {
        "?".to_string()
    } else {
        text.to_uppercase()
    }
}

pub fn is_group_jid(jid: &str) -> bool {
    jid.ends_with("@g.us")
}

/// A chat/person avatar: the cached profile photo when there is one, the
/// group glyph for a photo-less group, otherwise tinted initials.
pub fn avatar(
    name: &str,
    jid: &str,
    extra_class: &str,
    photos: &HashMap<String, String>,
) -> String {
    let url = photos.get(jid);
    let letters = escape(&initials(name));
    let class = if extra_class.is_empty() {
        "avatar"
    } else {
        extra_class
    };
    if let Some(url) = url {
        return format!(
            r#"<div class="{class}" data-jid="{}" data-initials="{letters}"><img class="avatar-img" src="{}" alt="" loading="lazy" onerror="window.__waAvatarError(this)"></div>"#,
            escape(jid),
            escape(url)
        );
    }
    if is_group_jid(jid) {
        return format!(
            r#"<div class="{class} group-avatar" data-jid="{}" data-initials="{letters}">{GROUP_AVATAR_ICON}</div>"#,
            escape(jid)
        );
    }
    format!(
        r#"<div class="{class}" style="{}">{letters}</div>"#,
        avatar_style(name)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_markup() {
        assert_eq!(
            escape(r#"<a href="x">&</a>"#),
            "&lt;a href=&quot;x&quot;&gt;&amp;&lt;/a&gt;"
        );
    }

    #[test]
    fn icons_render_on_the_material_grid() {
        let html = icon("check");
        assert!(html.starts_with(r#"<svg class="wa-icon" viewBox="0 -960 960 960""#));
        assert!(html.contains("M382-240"));
        assert_eq!(
            icon("nope"),
            r#"<svg class="wa-icon" viewBox="0 -960 960 960" aria-hidden="true"></svg>"#
        );
    }

    #[test]
    fn initials_take_two_words() {
        assert_eq!(initials("ada lovelace"), "AL");
        assert_eq!(initials("budi"), "B");
        assert_eq!(initials("  "), "?");
    }

    #[test]
    fn avatar_kinds() {
        let mut photos = HashMap::new();
        photos.insert(
            "1@s.whatsapp.net".to_string(),
            "https://x/y.jpg".to_string(),
        );
        assert!(avatar("Ada", "1@s.whatsapp.net", "avatar", &photos).contains("avatar-img"));
        assert!(avatar("Team", "2@g.us", "avatar", &photos).contains("group-avatar"));
        assert!(avatar("Ada", "3@s.whatsapp.net", "avatar", &photos).contains("background:#"));
    }

    #[test]
    fn avatar_style_is_stable() {
        assert_eq!(avatar_style("Ada"), avatar_style("Ada"));
        assert!(avatar_style("Ada").starts_with("background:#"));
    }
}
