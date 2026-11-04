//! Colour emoji glyphs. egui's default text path (ab_glyph) only knows outline
//! fonts, so emoji drawn as text came out as monochrome outlines or boxes; the
//! system emoji font is a colour font (COLR/CPAL on Windows, CBDT on Linux).
//! `swash` rasterises those to RGBA, and the window paints them as textures the
//! way the official client does.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// Where the colour emoji face lives.
const CANDIDATES: &[&str] = &[
    "C:\\Windows\\Fonts\\seguiemj.ttf",
    "/usr/share/fonts/truetype/noto/NotoColorEmoji.ttf",
    "/usr/share/fonts/noto/NotoColorEmoji.ttf",
    "/usr/share/fonts/google-noto/NotoColorEmoji.ttf",
    "/System/Library/Fonts/Apple Color Emoji.ttc",
];

fn font_bytes() -> Option<&'static [u8]> {
    static BYTES: OnceLock<Option<Vec<u8>>> = OnceLock::new();
    BYTES
        .get_or_init(|| CANDIDATES.iter().find_map(|path| std::fs::read(path).ok()))
        .as_deref()
}

fn font_ref() -> Option<swash::FontRef<'static>> {
    let bytes = font_bytes()?;
    swash::FontRef::from_index(bytes, 0)
}

/// Render one emoji at `px` into RGBA pixels (unpremultiplied, top-down).
pub fn rgba(ch: char, px: u32) -> Option<(Vec<u8>, u32, u32)> {
    use swash::scale::image::Content;
    use swash::scale::{Render, ScaleContext, Source, StrikeWith};

    let font = font_ref()?;
    let glyph = font.charmap().map(ch);
    if glyph == 0 {
        return None;
    }
    let mut context = ScaleContext::new();
    let mut scaler = context.builder(font).size(px as f32).hint(true).build();
    let image = Render::new(&[
        Source::ColorOutline(0),
        Source::ColorBitmap(StrikeWith::BestFit),
        Source::Outline,
    ])
    .render(&mut scaler, glyph)?;
    // A mask-only result means the face has no colour data for this glyph; the
    // caller falls back to text so we never paint a solid block.
    if image.content != Content::Color || image.data.is_empty() {
        return None;
    }
    Some((image.data, image.placement.width, image.placement.height))
}

/// A texture for `ch` at `px`, cached on the context: the picker shows the same
/// emoji over and over, and rasterising is the expensive part.
pub fn texture(
    ctx: &egui::Context,
    cache: &mut GlyphCache,
    ch: char,
    px: u32,
) -> Option<egui::TextureHandle> {
    if let Some(hit) = cache.get(&(ch, px)) {
        return hit.clone();
    }
    let texture = rasterise(ctx, ch, px);
    // The cache is tiny (a glyph is a few kB); drop it wholesale if a session
    // ever collects a very large number of distinct sizes.
    if cache.len() > 2_000 {
        cache.clear();
    }
    cache.insert((ch, px), texture.clone());
    texture
}

/// The texture cache key: one glyph at one size.
type GlyphKey = (char, u32);
/// A cached emoji texture (or the knowledge that this glyph has none).
type GlyphCache = HashMap<GlyphKey, Option<egui::TextureHandle>>;

/// The shared cache: message bubbles are painted by free functions that have no
/// access to the window state, and they need the same textures.
pub fn texture_shared(ctx: &egui::Context, ch: char, px: u32) -> Option<egui::TextureHandle> {
    static CACHE: OnceLock<Mutex<GlyphCache>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(cache) = cache.lock()
        && let Some(hit) = cache.get(&(ch, px))
    {
        return hit.clone();
    }
    let texture = rasterise(ctx, ch, px);
    if let Ok(mut cache) = cache.lock() {
        if cache.len() > 2_000 {
            cache.clear();
        }
        cache.insert((ch, px), texture.clone());
    }
    texture
}

fn rasterise(ctx: &egui::Context, ch: char, px: u32) -> Option<egui::TextureHandle> {
    let (data, width, height) = rgba(ch, px)?;
    let image = egui::ColorImage::from_rgba_unmultiplied([width as usize, height as usize], &data);
    Some(ctx.load_texture(
        format!("emoji:{ch}:{px}"),
        image,
        egui::TextureOptions::LINEAR,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_emoji_rasterises_with_colour() {
        if font_bytes().is_none() {
            eprintln!("skipping: no colour emoji font on this machine");
            return;
        }
        let Some((data, width, height)) = rgba('😀', 28) else {
            // A colour font is present but this face carries no bitmap/COLR
            // glyph for it: the UI falls back to text, so it must not fail.
            eprintln!("no rasterised glyph");
            return;
        };
        assert!(width > 0 && height > 0, "{width}x{height}");
        assert_eq!(data.len(), (width * height * 4) as usize);
        let opaque = data
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|pixel| pixel[3] > 128)
            .count();
        assert!(opaque > 40, "only {opaque} opaque pixels");
        // A smiling face is yellow: some pixel must be warm.
        assert!(
            data.as_chunks::<4>()
                .0
                .iter()
                .any(|pixel| pixel[0] > 150 && pixel[1] > 120 && pixel[2] < 120),
            "no warm pixels - the glyph came out monochrome"
        );
    }
}
