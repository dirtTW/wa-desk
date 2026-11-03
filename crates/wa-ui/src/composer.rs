//! The status text composer: its background palette, fonts, the ARGB value the
//! protobuf carries and the status emoji panel.

use serde::Serialize;

use crate::html::escape;

/// The status text composer's send button: it enables once the text has content.
pub fn can_post_text(text: &str) -> bool {
    !text.trim().is_empty()
}

/// Where an Esc/back press in the status composer goes: the emoji panel closes
/// first, then the text screen, and only then the media composer.
pub fn wizard_back(text_screen_open: bool, emoji_open: bool) -> &'static str {
    if emoji_open {
        "emoji"
    } else if text_screen_open {
        "text_screen"
    } else {
        "composer"
    }
}

/// The official composer's 20 backgrounds (read from the shipped client).
pub const BACKGROUNDS: [&str; 20] = [
    "#A62C71", "#90A841", "#C1A03F", "#792138", "#AE8774", "#F0B330", "#B6B327", "#C69FCC",
    "#8B6990", "#FF8A8C", "#54C265", "#FF7B6B", "#26C4DC", "#57C9FF", "#74676A", "#7E90A3",
    "#5696FF", "#6E257E", "#7ACBA5", "#243640",
];

/// The four fonts the "Aa" button cycles, as CSS font stacks.
pub const FONTS: [&str; 4] = [
    "inherit",
    "Georgia, 'Times New Roman', serif",
    "'Segoe Script', 'Comic Sans MS', cursive",
    "Consolas, 'Courier New', monospace",
];

/// Wire names `post_status_text` maps onto the protobuf font enum.
pub const FONT_NAMES: [&str; 4] = ["system", "serif", "script", "bold"];

/// `#rrggbb` -> the opaque `0xAARRGGBB` integer the status protobuf carries.
pub fn argb(hex: &str) -> u32 {
    let clean = hex.strip_prefix('#').unwrap_or(hex);
    if clean.len() != 6 || !clean.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return 0xff00_a884;
    }
    match u32::from_str_radix(clean, 16) {
        Ok(value) => 0xff00_0000 | value,
        Err(_) => 0xff00_a884,
    }
}

/// Wrap an index into `len`, like the composer's cycling buttons.
pub fn wrap(index: i64, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    (((index % len as i64) + len as i64) % len as i64) as usize
}

#[derive(Debug, Serialize)]
pub struct StatusPalette {
    pub backgrounds: Vec<String>,
    pub fonts: Vec<String>,
    pub font_names: Vec<String>,
}

pub fn palette() -> StatusPalette {
    StatusPalette {
        backgrounds: BACKGROUNDS.iter().map(|value| value.to_string()).collect(),
        fonts: FONTS.iter().map(|value| value.to_string()).collect(),
        font_names: FONT_NAMES.iter().map(|value| value.to_string()).collect(),
    }
}

#[derive(Debug, Serialize)]
pub struct EmojiPanel {
    pub title: String,
    pub html: String,
    pub count: usize,
}

fn item(char: &str, keywords: &str) -> String {
    format!(
        r#"<button type="button" class="emoji" data-status-emoji="{}" title="{}">{}</button>"#,
        escape(char),
        escape(keywords),
        escape(char)
    )
}

/// The status composer's category strip (its own data attribute).
pub fn emoji_categories_html() -> String {
    crate::emoji_data::EMOJI_GROUPS
        .iter()
        .enumerate()
        .map(|(index, group)| {
            format!(
                r#"<button type="button" class="emoji-cat{}" data-status-emoji-group="{index}" title="{}">{}</button>"#,
                if index == 0 { " active" } else { "" },
                escape(group.label),
                escape(group.icon)
            )
        })
        .collect()
}

/// One status emoji category, or the search results across every category.
pub fn emoji_group_html(index: usize, query: &str) -> EmojiPanel {
    let needle = query.trim().to_lowercase();
    if !needle.is_empty() {
        let items: Vec<(&str, &str)> = crate::emoji_data::EMOJI_GROUPS
            .iter()
            .flat_map(|group| group.items.iter().copied())
            .filter(|(_, keywords)| keywords.to_lowercase().contains(&needle))
            .collect();
        let title = if items.is_empty() {
            format!("No emoji match \"{query}\"")
        } else {
            "Search results".to_string()
        };
        return EmojiPanel {
            title,
            html: items
                .iter()
                .map(|(char, keywords)| item(char, keywords))
                .collect(),
            count: items.len(),
        };
    }

    let group = crate::emoji_data::EMOJI_GROUPS
        .get(index)
        .unwrap_or(&crate::emoji_data::EMOJI_GROUPS[0]);
    let title = if index == 0 {
        "Smileys & People".to_string()
    } else {
        group.label.to_string()
    };
    EmojiPanel {
        title,
        html: group
            .items
            .iter()
            .map(|(char, keywords)| item(char, keywords))
            .collect(),
        count: group.items.len(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_send_button_and_the_back_stack() {
        assert!(!can_post_text("   "));
        assert!(can_post_text(" hi "));

        assert_eq!(
            wizard_back(true, true),
            "emoji",
            "the emoji panel closes first"
        );
        assert_eq!(wizard_back(true, false), "text_screen");
        assert_eq!(wizard_back(false, false), "composer");
    }

    #[test]
    fn palette_matches_the_official_sets() {
        let p = palette();
        assert_eq!(p.backgrounds.len(), 20);
        assert_eq!(p.backgrounds[0], "#A62C71");
        assert_eq!(p.backgrounds[19], "#243640");
        assert_eq!(p.fonts.len(), 4);
        assert_eq!(p.font_names, vec!["system", "serif", "script", "bold"]);
    }

    #[test]
    fn argb_is_opaque() {
        assert_eq!(argb("#243640"), 0xff24_3640);
        assert_eq!(argb("243640"), 0xff24_3640);
        assert_eq!(argb("#zzzzzz"), 0xff00_a884);
        assert_eq!(argb(""), 0xff00_a884);
        assert_eq!(argb("#fff"), 0xff00_a884);
    }

    #[test]
    fn cycling_wraps_both_ways() {
        assert_eq!(wrap(0, 20), 0);
        assert_eq!(wrap(-1, 20), 19);
        assert_eq!(wrap(21, 20), 1);
        assert_eq!(wrap(3, 0), 0);
    }

    #[test]
    fn status_emoji_categories_and_groups() {
        let cats = emoji_categories_html();
        assert_eq!(cats.matches("emoji-cat").count(), 9);
        assert!(cats.contains(r#"class="emoji-cat active" data-status-emoji-group="0""#));

        let first = emoji_group_html(0, "");
        assert_eq!(first.title, "Smileys & People");
        assert!(first.count > 100);
        assert!(first.html.contains("data-status-emoji="));

        let animals = emoji_group_html(2, "");
        assert_eq!(animals.title, "Animals");
    }

    #[test]
    fn status_emoji_search_and_no_match() {
        let hits = emoji_group_html(0, "grinning");
        assert_eq!(hits.title, "Search results");
        assert!(hits.count > 0);
        assert!(hits.html.contains(r#"title="grinning"#));

        let none = emoji_group_html(0, "zzzz-nope");
        assert_eq!(none.title, "No emoji match \"zzzz-nope\"");
        assert_eq!(none.count, 0);
        assert_eq!(none.html, "");
    }
}
