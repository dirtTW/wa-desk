//! The emoji catalogue and picker markup. The catalogue lives in Rust
//! (`emoji_data.rs`) and the picker's grids are rendered here, so the frontend
//! only assigns the returned HTML and handles clicks.

use serde::Serialize;

use crate::emoji_data::EMOJI_GROUPS;
use crate::html::escape;

#[derive(Serialize, Clone)]
pub struct EmojiItem {
    pub char: String,
    pub keywords: String,
}

#[derive(Serialize, Clone)]
pub struct EmojiGroupView {
    pub id: String,
    pub label: String,
    pub icon: String,
    pub items: Vec<EmojiItem>,
}

/// The whole catalogue, in picker order.
pub fn groups() -> Vec<EmojiGroupView> {
    EMOJI_GROUPS
        .iter()
        .map(|group| EmojiGroupView {
            id: group.id.to_string(),
            label: group.label.to_string(),
            icon: group.icon.to_string(),
            items: group
                .items
                .iter()
                .map(|(char, keywords)| EmojiItem {
                    char: (*char).to_string(),
                    keywords: (*keywords).to_string(),
                })
                .collect(),
        })
        .collect()
}

pub fn count() -> usize {
    EMOJI_GROUPS.iter().map(|group| group.items.len()).sum()
}

/// Case-insensitive keyword search across every group, in catalogue order.
pub fn search(query: &str, limit: usize) -> Vec<EmojiItem> {
    let needle = query.trim().to_lowercase();
    if needle.is_empty() {
        return Vec::new();
    }
    let mut matches = Vec::new();
    for group in EMOJI_GROUPS {
        for (char, keywords) in group.items {
            if keywords.to_lowercase().contains(&needle) {
                matches.push(EmojiItem {
                    char: (*char).to_string(),
                    keywords: (*keywords).to_string(),
                });
                if matches.len() >= limit {
                    return matches;
                }
            }
        }
    }
    matches
}

fn button(char: &str, keywords: &str) -> String {
    format!(
        r#"<button type="button" class="emoji" data-char="{}" data-keywords="{}" title="{}">{}</button>"#,
        escape(char),
        escape(keywords),
        escape(keywords),
        escape(char)
    )
}

/// The picker body: a title and a grid per group.
pub fn picker_html() -> String {
    let mut html = String::new();
    for (index, group) in EMOJI_GROUPS.iter().enumerate() {
        html.push_str(&format!(
            r#"<div class="emoji-title" data-group="{index}">{}</div>"#,
            escape(group.label)
        ));
        html.push_str(&format!(r#"<div class="emoji-grid" data-group="{index}">"#));
        for (char, keywords) in group.items {
            html.push_str(&button(char, keywords));
        }
        html.push_str("</div>");
    }
    html
}

/// The category strip glyphs, first one active.
pub fn categories_html() -> String {
    let mut html = String::new();
    for (index, group) in EMOJI_GROUPS.iter().enumerate() {
        html.push_str(&format!(
            r#"<button type="button" class="emoji-cat{}" data-group="{index}" title="{}">{}</button>"#,
            if index == 0 { " active" } else { "" },
            escape(group.label),
            escape(group.icon)
        ));
    }
    html
}

/// The recent-emoji row above the grids.
pub fn recents_html(chars: &[String]) -> String {
    chars
        .iter()
        .map(|value| {
            format!(
                r#"<button type="button" class="emoji" data-char="{}">{}</button>"#,
                escape(value),
                escape(value)
            )
        })
        .collect()
}

/// Search results as one grid (empty when the query has no matches).
pub fn search_html(query: &str, limit: usize) -> String {
    let matches = search(query, limit);
    let mut html = String::from(r#"<div class="emoji-grid search-results">"#);
    for item in &matches {
        html.push_str(&button(&item.char, &item.keywords));
    }
    html.push_str("</div>");
    html
}

/// Which group the category strip should highlight while the picker scrolls.
/// `offsets` holds `(group_index, offset_top)` in document order; a title counts
/// once it has scrolled to the top (plus `pad`).
pub fn active_group(offsets: &[(usize, f64)], scroll_top: f64, pad: f64) -> usize {
    let top = scroll_top + pad;
    let mut current = offsets.first().map(|(index, _)| *index).unwrap_or(0);
    for (index, offset) in offsets {
        if *offset <= top {
            current = *index;
        }
    }
    current
}

/// How many recent emoji the picker keeps.
pub const RECENTS_CAP: usize = 24;

/// Move `char` to the front of the recents, deduped and capped. Older duplicates
/// in the stored list are cleaned up on the way.
pub fn remember_recent(recents: &[String], char: &str) -> Vec<String> {
    let char = char.trim();
    if char.is_empty() {
        return recents.to_vec();
    }
    let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
    seen.insert(char);
    let mut next: Vec<String> = Vec::with_capacity(recents.len() + 1);
    next.push(char.to_string());
    for value in recents {
        if seen.insert(value.as_str()) {
            next.push(value.clone());
        }
    }
    next.truncate(RECENTS_CAP);
    next
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PickerTab {
    pub emoji_view: bool,
    pub sticker_view: bool,
    pub gif_view: bool,
    /// The element to focus, when the tab takes typing.
    pub focus: Option<&'static str>,
    /// The sticker grid is rebuilt when its tab opens.
    pub reload_stickers: bool,
}

/// Which of the picker's three views a tab shows.
pub fn picker_tab(tab: &str) -> PickerTab {
    PickerTab {
        emoji_view: tab != "stickers" && tab != "gif",
        sticker_view: tab == "stickers",
        gif_view: tab == "gif",
        focus: if tab == "emoji" {
            Some("emoji-search")
        } else {
            None
        },
        reload_stickers: tab == "stickers",
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Insertion {
    pub value: String,
    pub caret: usize,
}

/// Insert text at the composer's selection, replacing it. Out-of-range or
/// reversed selection indices are clamped to the value.
pub fn insert_text(value: &str, start: usize, end: usize, insert: &str) -> Insertion {
    let chars: Vec<char> = value.chars().collect();
    let len = chars.len();
    let (from, to) = if start <= end {
        (start.min(len), end.min(len))
    } else {
        (end.min(len), start.min(len))
    };
    let mut next: String = chars[..from].iter().collect();
    next.push_str(insert);
    next.extend(chars[to..].iter());
    Insertion {
        value: next,
        caret: from + insert.chars().count(),
    }
}

/// The composer's send button is live only with non-blank text.
pub fn composer_has_text(text: &str) -> bool {
    !text.trim().is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalogue_is_complete() {
        let groups = groups();
        assert_eq!(groups.len(), 9);
        assert!(count() > 2000, "count = {}", count());
        assert_eq!(groups[0].id, "smileys");
        assert!(!groups[0].items[0].char.is_empty());
    }

    #[test]
    fn recents_move_to_the_front_and_stop_at_24() {
        let recents = vec!["😀".to_string(), "🔥".to_string()];
        let next = remember_recent(&recents, "😀");
        assert_eq!(
            next,
            vec!["😀".to_string(), "🔥".to_string()],
            "deduped, moved first"
        );

        let added = remember_recent(&recents, "👍");
        assert_eq!(
            added,
            vec!["👍".to_string(), "😀".to_string(), "🔥".to_string()]
        );

        // An older list with duplicates is cleaned up.
        let messy = vec!["😀".to_string(), "🔥".to_string(), "😀".to_string()];
        assert_eq!(
            remember_recent(&messy, "👍").len(),
            3,
            "no duplicate survives"
        );

        // The cap keeps the newest 24.
        let full: Vec<String> = (0..30).map(|i| format!("e{i}")).collect();
        let capped = remember_recent(&full, "new");
        assert_eq!(capped.len(), RECENTS_CAP);
        assert_eq!(capped[0], "new");

        // A blank pick changes nothing.
        assert_eq!(remember_recent(&recents, "  ").len(), 2);
    }

    #[test]
    fn the_picker_tab_picks_one_view() {
        let emoji = picker_tab("emoji");
        assert!(emoji.emoji_view && !emoji.sticker_view && !emoji.gif_view);
        assert_eq!(emoji.focus, Some("emoji-search"));
        assert!(!emoji.reload_stickers);

        let stickers = picker_tab("stickers");
        assert!(stickers.sticker_view && !stickers.emoji_view);
        assert!(stickers.reload_stickers);
        assert!(stickers.focus.is_none(), "the sticker grid needs no typing");

        let gif = picker_tab("gif");
        assert!(gif.gif_view && !gif.emoji_view && !gif.reload_stickers);

        // An unknown tab falls back to the emoji view.
        assert!(picker_tab("nonsense").emoji_view);
    }

    #[test]
    fn inserting_emoji_replaces_the_selection() {
        let at_end = insert_text("hello ", 6, 6, "😀");
        assert_eq!(at_end.value, "hello 😀");
        assert_eq!(at_end.caret, 7);

        // A selection is replaced, not appended.
        let replaced = insert_text("hello world", 6, 11, "🔥");
        assert_eq!(replaced.value, "hello 🔥");
        assert_eq!(replaced.caret, 7);

        // A reversed selection normalises (it replaces "bc").
        let reversed = insert_text("abc", 3, 1, "x");
        assert_eq!(reversed.value, "ax");

        // Out-of-range indices clamp to the value.
        let beyond = insert_text("ab", 99, 99, "z");
        assert_eq!(beyond.value, "abz");
        assert_eq!(beyond.caret, 3);
    }

    #[test]
    fn the_composer_needs_real_text() {
        assert!(!composer_has_text(""));
        assert!(!composer_has_text("   "));
        assert!(composer_has_text(" hi "));
    }

    #[test]
    fn the_scrolling_category_follows_the_titles() {
        // Titles sit at 0, 800, 1600, 2400.
        let offsets = [(0, 0.0), (1, 800.0), (2, 1600.0), (3, 2400.0)];
        assert_eq!(active_group(&offsets, 0.0, 8.0), 0);
        assert_eq!(active_group(&offsets, 10.0, 8.0), 0);
        assert_eq!(active_group(&offsets, 100.0, 8.0), 0);
        assert_eq!(active_group(&offsets, 795.0, 8.0), 1, "the 8 px pad counts");
        assert_eq!(active_group(&offsets, 1592.0, 8.0), 2, "top = 1600");
        assert_eq!(active_group(&offsets, 9000.0, 8.0), 3);
        // No titles yet.
        assert_eq!(active_group(&[], 100.0, 8.0), 0);
    }

    #[test]
    fn search_matches_keywords() {
        let hits = search("grinning", 240);
        assert!(!hits.is_empty());
        assert!(hits.iter().all(|item| item.keywords.contains("grinning")));
        assert_eq!(search("", 240).len(), 0);
        assert!(search("zzzz-no-such-emoji", 240).is_empty());
        assert_eq!(search("face", 5).len(), 5);
    }

    #[test]
    fn picker_markup_is_escaped_and_grouped() {
        let html = picker_html();
        assert!(html.contains(r#"<div class="emoji-title" data-group="0">Smileys</div>"#));
        assert!(html.contains(r#"<div class="emoji-grid" data-group="8">"#));
        assert!(html.contains(r#"data-char="\u{1F600}""#) || html.contains("😀"));
        let cats = categories_html();
        assert!(cats.contains(r#"class="emoji-cat active" data-group="0""#));
        assert!(cats.contains(r#"data-group="8""#));
    }

    #[test]
    fn recent_row() {
        let html = recents_html(&["😀".to_string(), "\"".to_string()]);
        assert!(html.contains(r#"data-char="😀""#));
        assert!(html.contains("&quot;"), "values are escaped: {html}");
    }

    #[test]
    fn search_grid() {
        let html = search_html("grinning", 240);
        assert!(html.starts_with(r#"<div class="emoji-grid search-results">"#));
        assert!(html.contains("data-char="));
    }
}
