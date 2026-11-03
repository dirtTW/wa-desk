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
    fn search_grid() {
        let html = search_html("grinning", 240);
        assert!(html.starts_with(r#"<div class="emoji-grid search-results">"#));
        assert!(html.contains("data-char="));
    }
}
