//! Rich text inside a bubble: WhatsApp's `*bold*`, `_italic_`, `~strike~` and
//! ```` ```mono``` ```` markup, colour emoji, and the painter that draws it.
//! These have no access to the window state, which is why they live outside
//! `app.rs`.

/// How one run of the body is drawn.
#[derive(Clone, Copy, Default, PartialEq)]
pub(crate) struct Style {
    pub bold: bool,
    pub strike: bool,
    pub mono: bool,
}

/// One laid-out piece of a message body: text or a colour emoji.
pub(crate) enum Inline {
    Text(std::sync::Arc<egui::Galley>, Style),
    Emoji(egui::TextureHandle),
}

/// A body laid out with text galleys and colour emoji side by side.
pub(crate) struct BodyLayout {
    placed: Vec<(egui::Pos2, Inline)>,
    pub(crate) size: egui::Vec2,
    pub(crate) emoji_px: f32,
    pub(crate) color: egui::Color32,
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

/// Does the body carry any of WhatsApp's formatting markers?
pub(crate) fn has_markup(text: &str) -> bool {
    text.contains('*') || text.contains('_') || text.contains('~') || text.contains('`')
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

/// Split a body into styled runs, following WhatsApp's marker rules: an opener
/// must be followed by a non-space and a closer must be preceded by one (so
/// `snake_case` stays literal). A run of markers is one delimiter, which is how
/// `**bold**` and ``` ```mono``` ``` are written.
fn spans(text: &str) -> Vec<(String, Style)> {
    let chars: Vec<char> = text.chars().collect();
    let mut out: Vec<(String, Style)> = Vec::new();
    let mut plain = String::new();
    let mut index = 0;
    while index < chars.len() {
        let marker = chars[index];
        // The delimiter's own length, so ``` … ``` closes on a triple too.
        let mut open_len = 1;
        while index + open_len < chars.len() && chars[index + open_len] == marker {
            open_len += 1;
        }
        let opens = matches!(marker, '*' | '_' | '~' | '`')
            && index + open_len < chars.len()
            && !chars[index + open_len].is_whitespace()
            && (index == 0 || chars[index - 1].is_whitespace() || chars[index - 1] == '(');
        let mut close: Option<(usize, usize)> = None;
        if opens {
            let mut probe = index + open_len;
            while probe < chars.len() {
                if chars[probe] != marker {
                    probe += 1;
                    continue;
                }
                let mut len = 1;
                while probe + len < chars.len() && chars[probe + len] == marker {
                    len += 1;
                }
                let after_text = probe > 0 && !chars[probe - 1].is_whitespace();
                if after_text && len >= open_len {
                    close = Some((probe, len));
                    break;
                }
                probe += len;
            }
        }
        match close {
            Some((end, close_len)) => {
                if !plain.is_empty() {
                    out.push((std::mem::take(&mut plain), Style::default()));
                }
                let inner: String = chars[index + open_len..end].iter().collect();
                let style = Style {
                    bold: marker == '*',
                    strike: marker == '~',
                    mono: marker == '`',
                };
                out.push((inner, style));
                index = end + close_len;
            }
            None => {
                plain.push(marker);
                index += 1;
            }
        }
    }
    if !plain.is_empty() {
        out.push((plain, Style::default()));
    }
    out
}

/// Lay out a message body where emoji are colour textures and the rest is text.
/// Words wrap at `max_width`; emoji occupy `emoji_px` square cells.
pub(crate) fn layout_body(
    ui: &egui::Ui,
    text: &str,
    color: egui::Color32,
    max_width: f32,
    emoji_px: u32,
) -> BodyLayout {
    enum Token {
        Word(String, Style),
        Emoji(char),
        Break,
    }

    let font = egui::FontId::proportional(14.0);
    let mono = egui::FontId::monospace(14.0);
    let base_row = ui.fonts_mut(|fonts| fonts.row_height(&font).max(fonts.row_height(&mono)));
    let emoji_size = emoji_px as f32;
    // Every line is at least as tall as an emoji cell: with a shorter row the
    // 19 px cells overlapped the line above, which made mixed text look like it
    // floated off the paragraph.
    let row = base_row.max(emoji_size);
    let space = ui
        .painter()
        .layout_no_wrap(" ".to_string(), font.clone(), color)
        .size()
        .x;
    let space_mono = ui
        .painter()
        .layout_no_wrap(" ".to_string(), mono.clone(), color)
        .size()
        .x;

    // Split every styled run into emoji cells and words.
    let mut tokens: Vec<Token> = Vec::new();
    for (run, style) in spans(text) {
        let mut emoji_run = String::new();
        let mut word_run = String::new();
        let flush_words = |word_run: &mut String, tokens: &mut Vec<Token>| {
            for word in word_run.split_inclusive(char::is_whitespace) {
                if !word.contains('\n') {
                    tokens.push(Token::Word(word.to_string(), style));
                    continue;
                }
                // A newline is a real line break (the numbered lists people
                // paste used to collapse into one paragraph).
                let mut first = true;
                for part in word.split('\n') {
                    if !first {
                        tokens.push(Token::Break);
                    }
                    first = false;
                    if !part.is_empty() {
                        tokens.push(Token::Word(part.to_string(), style));
                    }
                }
            }
            word_run.clear();
        };
        for ch in run.chars() {
            let emoji = is_emoji_char(ch) || ch == '\u{FE0F}' || ch == '\u{200D}';
            if emoji {
                if !word_run.is_empty() {
                    flush_words(&mut word_run, &mut tokens);
                }
                emoji_run.push(ch);
            } else {
                for pending in emoji_run.chars() {
                    if pending != '\u{FE0F}' && pending != '\u{200D}' {
                        tokens.push(Token::Emoji(pending));
                    }
                }
                emoji_run.clear();
                word_run.push(ch);
            }
        }
        for pending in emoji_run.chars() {
            if pending != '\u{FE0F}' && pending != '\u{200D}' {
                tokens.push(Token::Emoji(pending));
            }
        }
        if !word_run.is_empty() {
            flush_words(&mut word_run, &mut tokens);
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
                    y += row;
                }
                let offset = (row - emoji_size) * 0.5;
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
                        let text_offset = (row - galley.size().y) * 0.5;
                        placed.push((
                            egui::pos2(x, y + text_offset),
                            Inline::Text(galley, Style::default()),
                        ));
                        x += width.max(1.0);
                    }
                }
                widest = widest.max(x);
            }
            Token::Break => {
                x = 0.0;
                y += row;
            }
            Token::Word(word, style) => {
                // `layout_no_wrap` drops trailing whitespace from the measured
                // width, so the space has to be added back by hand: without it
                // every word sat tight against the next and the paragraph lost
                // its even gaps.
                let body = word.trim_end();
                let spaces = word.len() - body.len();
                let face = if style.mono {
                    mono.clone()
                } else {
                    font.clone()
                };
                let galley = ui.painter().layout_no_wrap(body.to_string(), face, color);
                let width = galley.size().x;
                if x > 0.0 && x + width > max_width {
                    x = 0.0;
                    y += row;
                }
                let offset = (row - galley.size().y) * 0.5;
                placed.push((egui::pos2(x, y + offset), Inline::Text(galley, style)));
                x += width;
                widest = widest.max(x);
                x += (if style.mono { space_mono } else { space }) * spaces as f32;
            }
        }
    }
    BodyLayout {
        placed,
        size: egui::vec2(widest, y + row),
        emoji_px: emoji_size,
        color,
    }
}

/// Paint a laid-out body at `origin`. Bold is a second pass 0.4 px to the right
/// (the bundled faces have no bold weight), strike is a hairline through the
/// middle of the run.
pub(crate) fn paint_body(painter: &egui::Painter, origin: egui::Pos2, body: &BodyLayout) {
    for (pos, inline) in &body.placed {
        let at = origin + pos.to_vec2();
        match inline {
            Inline::Text(galley, style) => {
                painter.galley(at, galley.clone(), egui::Color32::PLACEHOLDER);
                if style.bold {
                    painter.galley(
                        at + egui::vec2(0.4, 0.0),
                        galley.clone(),
                        egui::Color32::PLACEHOLDER,
                    );
                }
                if style.strike {
                    let mid = at.y + galley.size().y * 0.55;
                    painter.line_segment(
                        [
                            egui::pos2(at.x, mid),
                            egui::pos2(at.x + galley.size().x, mid),
                        ],
                        egui::Stroke::new(1.0_f32, body.color),
                    );
                }
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

#[cfg(test)]
mod markup_tests {
    use super::*;

    fn plain(text: &str) -> Vec<(String, Style)> {
        spans(text)
    }

    #[test]
    fn bold_italic_strike_and_mono_are_styles_not_characters() {
        let runs = plain("hi *there* _now_ ~gone~ ```code```");
        assert_eq!(runs[0].0, "hi ");
        assert!(runs[1].1.bold);
        assert_eq!(runs[1].0, "there");
        assert_eq!(runs[2].0, " ");
        assert_eq!(runs[3].0, "now");
        assert!(runs[5].1.strike);
        assert_eq!(runs[5].0, "gone");
        assert!(runs[7].1.mono);
        assert_eq!(runs[7].0, "code");
    }

    #[test]
    fn a_run_of_markers_is_one_delimiter() {
        let runs = plain("**bold**");
        assert_eq!(runs.len(), 1);
        assert!(runs[0].1.bold);
        assert_eq!(runs[0].0, "bold");
    }

    #[test]
    fn an_unpaired_marker_stays_literal() {
        let runs = plain("snake_case and 2 * 3");
        assert!(runs.iter().all(|(_, style)| *style == Style::default()));
        assert_eq!(runs[0].0, "snake_case and 2 * 3");
    }

    #[test]
    fn markup_is_detected_for_the_layout_switch() {
        assert!(has_markup("*hi*"));
        assert!(has_markup("~gone~"));
        assert!(!has_markup("plain words"));
    }
}
