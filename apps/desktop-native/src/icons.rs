//! Material Symbols for the native window.
//!
//! The icon bodies live in `wa_ui::icons_data` (generated from the official
//! Material Symbols set). egui cannot draw SVG paths, so each one is rasterised
//! with `resvg` into a small texture and cached on the context; the cache key
//! includes the colour, so hover states get their own copy.

use egui::{Color32, ColorImage, TextureHandle, TextureOptions};

/// The texture for `name` at `size` px, tinted `color`.
pub fn texture(
    ctx: &egui::Context,
    name: &str,
    size: u32,
    color: Color32,
) -> Option<TextureHandle> {
    let key = format!(
        "icon:{name}:{size}:{:02x}{:02x}{:02x}",
        color.r(),
        color.g(),
        color.b()
    );
    let id = egui::Id::unique(&key);
    if let Some(texture) = ctx.data(|data| data.get_temp::<TextureHandle>(id)) {
        return Some(texture);
    }
    let image = rasterise(name, size, color)?;
    let texture = ctx.load_texture(key, image, TextureOptions::LINEAR);
    ctx.data_mut(|data| data.insert_temp(id, texture.clone()));
    Some(texture)
}

/// Rasterise one icon body to RGBA pixels (no egui context needed).
pub fn rasterise(name: &str, size: u32, color: Color32) -> Option<ColorImage> {
    let body = wa_ui::icons_data::ICONS
        .iter()
        .find(|(icon, _)| *icon == name)?
        .1;
    let size = size.max(1);
    // The generated entries are `<path d="…"/>`; give the path the tint.
    let body = body.replacen(
        "<path ",
        &format!(
            "<path fill=\"#{:02x}{:02x}{:02x}\" ",
            color.r(),
            color.g(),
            color.b()
        ),
        1,
    );
    // `width`/`height` matter: with only a `viewBox` resvg falls back to its
    // own default size and the 960-unit path becomes a speck.
    let svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{size}\" height=\"{size}\" viewBox=\"0 -960 960 960\">{body}</svg>"
    );

    let tree = resvg::usvg::Tree::from_str(&svg, &resvg::usvg::Options::default()).ok()?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(size, size)?;
    // The `width`/`height` above already scale the viewBox, so render 1:1.
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    );
    Some(ColorImage::from_rgba_premultiplied(
        [size as usize, size as usize],
        pixmap.data(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_the_window_uses_exists() {
        for name in [
            "search",
            "more_vert",
            "donut_large",
            "campaign",
            "settings",
            "mood",
            "sticker",
            "send",
            "reply",
            "shortcut",
            "content_copy",
            "edit",
            "delete",
        ] {
            assert!(
                wa_ui::icons_data::ICONS
                    .iter()
                    .any(|(icon, _)| *icon == name),
                "{name} is missing from icons_data"
            );
        }
    }

    #[test]
    fn material_icons_rasterise_into_visible_pixels() {
        for name in ["send", "search", "delete"] {
            let image = rasterise(name, 24, Color32::WHITE).expect(name);
            let painted = image.pixels.iter().filter(|pixel| pixel.a() > 0).count();
            assert!(
                painted > 10,
                "{name} produced only {painted} painted pixels"
            );
        }
        assert!(rasterise("no_such_icon", 24, Color32::WHITE).is_none());
    }
}
