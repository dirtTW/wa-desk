//! Rich text inside a bubble: the layout that mixes ordinary text with colour
//! emoji textures, and the painter that draws it. These have no access to the
//! window state, which is why they live outside `app.rs`.

/// One laid-out piece of a message body: text or a colour emoji.
pub(crate) enum Inline {
    Text(std::sync::Arc<egui::Galley>),
    Emoji(egui::TextureHandle),
}

/// A body laid out with text galleys and colour emoji side by side.
pub(crate) struct BodyLayout {
    placed: Vec<(egui::Pos2, Inline)>,
    pub(crate) size: egui::Vec2,
    pub(crate) emoji_px: f32,
}

/// Is this a character the official client draws as an emoji picture?
pub(crate) fn is_emoji_char(ch: char) -> bool {
    let code = ch as u32;
    code >= 0x1F000
        || (0x2600..=0x27BF).contains(&code)
        || (0x2B00..=0x2BFF).contains(&code)
        || code == 0x00A9
        || code == 0x00AE
}

pub(crate) fn has_emoji(text: &str) -> bool {
    text.chars().any(is_emoji_char)
}

/// WhatsApp paints emoji-only messages much larger.
pub(crate) fn emoji_only(text: &str) -> bool {
    let trimmed = text.trim();
    !trimmed.is_empty()
        && trimmed.chars().count() <= 12
        && trimmed.chars().all(|ch| {
            is_emoji_char(ch) || ch == '\u{FE0F}' || ch == '\u{200D}' || ch.is_whitespace()
        })
}

/// Lay out a message body where emoji are colour textures and the rest is text.
/// Words are wrapped at `max_width`; emoji occupy `emoji_px` square cells.
pub(crate) fn layout_body(
    ui: &egui::Ui,
    text: &str,
    color: egui::Color32,
    max_width: f32,
    emoji_px: u32,
) -> BodyLayout {
    enum Token {
        Word(String),
        Emoji(char),
    }

    let font = egui::FontId::proportional(14.0);
    let row_height = ui.fonts(|fonts| fonts.row_height(&font));
    let emoji_size = emoji_px as f32;

    // Split into runs of emoji and text, then emoji cells and words.
    let mut runs: Vec<(String, bool)> = Vec::new();
    for ch in text.chars() {
        let emoji = is_emoji_char(ch) || ch == '\u{FE0F}' || ch == '\u{200D}';
        match runs.last_mut() {
            Some((buffer, kind)) if *kind == emoji => buffer.push(ch),
            _ => runs.push((ch.to_string(), emoji)),
        }
    }
    let mut tokens: Vec<Token> = Vec::new();
    for (run, emoji) in runs {
        if emoji {
            for ch in run.chars() {
                if ch != '\u{FE0F}' && ch != '\u{200D}' {
                    tokens.push(Token::Emoji(ch));
                }
            }
        } else {
            for word in run.split_inclusive(char::is_whitespace) {
                tokens.push(Token::Word(word.to_string()));
            }
        }
    }

    let mut placed = Vec::new();
    let mut x = 0.0f32;
    let mut y = 0.0f32;
    let mut widest = 0.0f32;
    for token in tokens {
        match token {
            Token::Emoji(ch) => {
                if x > 0.0 && x + emoji_size > max_width {
                    x = 0.0;
                    y += row_height;
                }
                let offset = (row_height - emoji_size) * 0.5;
                match crate::emoji_glyph::texture_shared(ui.ctx(), ch, emoji_px) {
                    Some(texture) => {
                        placed.push((egui::pos2(x, y + offset), Inline::Emoji(texture)));
                        // Advance the cell: two emoji in a row used to paint on
                        // top of each other.
                        x += emoji_size;
                    }
                    None => {
                        // No colour glyph for this codepoint: draw it as text
                        // (the symbol face carries most of these) instead of
                        // leaving a hole where a character should be.
                        let galley =
                            ui.painter()
                                .layout_no_wrap(ch.to_string(), font.clone(), color);
                        let width = galley.size().x;
                        let text_offset = (row_height - galley.size().y) * 0.5;
                        placed.push((egui::pos2(x, y + text_offset), Inline::Text(galley)));
                        x += width.max(1.0);
                    }
                }
                widest = widest.max(x);
            }
            Token::Word(word) => {
                let galley = ui
                    .painter()
                    .layout_no_wrap(word.clone(), font.clone(), color);
                let width = galley.size().x;
                if x > 0.0 && x + width > max_width {
                    x = 0.0;
                    y += row_height;
                }
                let offset = (row_height - galley.size().y) * 0.5;
                placed.push((egui::pos2(x, y + offset), Inline::Text(galley)));
                x += width;
                widest = widest.max(x);
            }
        }
    }
    BodyLayout {
        placed,
        size: egui::vec2(widest, y + row_height),
        emoji_px: emoji_size,
    }
}

/// Paint a laid-out body at `origin`.
pub(crate) fn paint_body(painter: &egui::Painter, origin: egui::Pos2, body: &BodyLayout) {
    for (pos, inline) in &body.placed {
        let at = origin + pos.to_vec2();
        match inline {
            Inline::Text(galley) => {
                painter.galley(at, galley.clone(), egui::Color32::PLACEHOLDER);
            }
            Inline::Emoji(texture) => {
                let size = texture.size_vec2();
                let scale = (body.emoji_px / size.y.max(1.0)).min(body.emoji_px / size.x.max(1.0));
                painter.image(
                    texture.id(),
                    egui::Rect::from_min_size(at, size * scale),
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    egui::Color32::WHITE,
                );
            }
        }
    }
}

/// Drop invisible formatting characters (zero-width space/joiner, word joiner,
/// BOM): WhatsApp passes them through, and the bundled fonts paint them as
/// missing-glyph boxes.
pub(crate) fn clean_text(text: &str) -> String {
    text.chars()
        .filter(|ch| {
            let code = *ch as u32;
            !(0x200B..=0x200F).contains(&code)
                && !(0x2060..=0x2064).contains(&code)
                && code != 0xFEFF
                && code != 0x00AD
        })
        .collect()
}
