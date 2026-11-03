//! The sticker picker grids: the Create tile, favourite/recent cells, the
//! loading placeholders and the empty states.

use serde::{Deserialize, Serialize};

use crate::html::{escape, icon};

#[derive(Debug, Clone, Default, Deserialize)]
pub struct StickerCell {
    #[serde(default)]
    pub id: String,
    /// The sticker image URL, or `None` while it downloads.
    #[serde(default)]
    pub url: Option<String>,
    /// `favorite` or `recent`.
    #[serde(default)]
    pub kind: String,
    /// Extra text the picker search matches.
    #[serde(default)]
    pub search: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StickerGridRequest {
    #[serde(default)]
    pub cells: Vec<StickerCell>,
    #[serde(default)]
    pub query: String,
    /// Shown in place of the grid when there is nothing to list.
    #[serde(default)]
    pub empty: String,
}

#[derive(Debug, Serialize)]
pub struct StickerGridOutput {
    pub html: String,
    /// Number of sticker cells that actually carry an image.
    pub loaded: usize,
}

/// The "Create" tile that opens the file picker.
pub fn create_cell() -> String {
    format!(
        r#"<button type="button" class="sticker-cell sticker-create" data-sticker-create="1">
    {icon}
    <span>Create</span>
  </button>"#,
        icon = icon("add")
    )
}

/// One sticker cell, loading when it has no URL yet.
fn cell_html(cell: &StickerCell) -> String {
    match cell.url.as_deref().filter(|url| !url.is_empty()) {
        Some(url) => {
            let (attr, alt) = if cell.kind == "favorite" {
                (
                    format!(r#"data-favorite-id="{}""#, escape(&cell.id)),
                    "Favorite sticker",
                )
            } else {
                (
                    format!(r#"data-sticker-id="{}""#, escape(&cell.id)),
                    "Sticker",
                )
            };
            format!(
                r#"<div class="sticker-cell" {attr} data-search="{}"><img src="{}" alt="{alt}" loading="lazy" /></div>"#,
                escape(&cell.search),
                escape(url)
            )
        }
        None => format!(
            r#"<div class="sticker-cell loading" data-sticker-id="{}" data-search="{}"></div>"#,
            escape(&cell.id),
            escape(&cell.search)
        ),
    }
}

/// Render the grid for the current search. `empty` (when set) replaces the
/// grid body if nothing matches, after the Create tile.
pub fn grid(req: &StickerGridRequest) -> StickerGridOutput {
    let needle = req.query.trim().to_lowercase();
    let show_create = needle.is_empty();
    let mut loaded = 0usize;
    let mut cells = String::new();
    for cell in &req.cells {
        if !needle.is_empty() {
            let haystack = if cell.search.is_empty() {
                cell.id.to_lowercase()
            } else {
                cell.search.to_lowercase()
            };
            if !haystack.contains(&needle) {
                continue;
            }
        }
        if cell.url.as_deref().is_some_and(|url| !url.is_empty()) {
            loaded += 1;
        }
        cells.push_str(&cell_html(cell));
    }

    let mut html = String::new();
    if show_create {
        html.push_str(&create_cell());
    }
    if cells.is_empty() {
        if !req.empty.is_empty() && req.cells.is_empty() {
            html.push_str(&format!(r#"<p class="empty">{}</p>"#, escape(&req.empty)));
        }
    } else {
        html.push_str(&cells);
    }

    StickerGridOutput { html, loaded }
}

/// The text of the sticker count badge, `""` when zero.
pub fn count_label(count: usize) -> String {
    if count == 0 {
        String::new()
    } else {
        format!("({count})")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell(id: &str, url: Option<&str>, kind: &str) -> StickerCell {
        StickerCell {
            id: id.into(),
            url: url.map(str::to_string),
            kind: kind.into(),
            search: format!("{kind} sticker"),
        }
    }

    #[test]
    fn grids_list_cells_with_urls() {
        let out = grid(&StickerGridRequest {
            cells: vec![
                cell("f1", Some("/f1.webp"), "favorite"),
                cell("r1", Some("/r1.webp"), "recent"),
            ],
            query: String::new(),
            empty: String::new(),
        });
        assert!(out.html.contains("sticker-create"));
        assert!(out.html.contains(r#"data-favorite-id="f1""#));
        assert!(out.html.contains(r#"data-sticker-id="r1""#));
        assert!(out.html.contains(r#"alt="Favorite sticker""#));
        assert_eq!(out.loaded, 2);
    }

    #[test]
    fn missing_urls_render_loading_cells() {
        let out = grid(&StickerGridRequest {
            cells: vec![cell("r1", None, "recent")],
            query: String::new(),
            empty: String::new(),
        });
        assert!(out.html.contains("sticker-cell loading"));
        assert_eq!(out.loaded, 0);
    }

    #[test]
    fn searching_hides_the_create_tile_and_filters() {
        let out = grid(&StickerGridRequest {
            cells: vec![
                cell("f1", Some("/f1.webp"), "favorite"),
                cell("r1", Some("/r1.webp"), "recent"),
            ],
            query: "favorite".into(),
            empty: String::new(),
        });
        assert!(!out.html.contains("sticker-create"));
        assert!(out.html.contains("f1"));
        assert!(!out.html.contains("r1"));

        let none = grid(&StickerGridRequest {
            cells: vec![cell("r1", Some("/r1.webp"), "recent")],
            query: "zzz".into(),
            empty: String::new(),
        });
        assert_eq!(none.html, "");
    }

    #[test]
    fn empty_state_and_count() {
        let out = grid(&StickerGridRequest {
            cells: Vec::new(),
            query: String::new(),
            empty: "No stickers yet.".into(),
        });
        assert!(out.html.contains("sticker-create"));
        assert!(out.html.contains("No stickers yet."));
        assert_eq!(count_label(0), "");
        assert_eq!(count_label(7), "(7)");
        assert!(create_cell().contains("Create"));
    }
}
