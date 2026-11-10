//! The chat wallpaper: the official faint doodle sheet, tiled behind the
//! messages and cached as one texture per palette.

/// The chat wallpaper: WhatsApp's faint doodle sheet, drawn once into a tile
/// and repeated behind the messages. Official dark is `#0B141A` with `#182229`
/// doodles; the light theme uses the paper sheet with `#EFE7DD` doodles.
fn wallpaper_textile(ctx: &egui::Context, palette: &crate::theme::Palette) -> egui::TextureHandle {
    let key = format!(
        "wallpaper:{:02x}{:02x}{:02x}",
        palette.wallpaper.r(),
        palette.wallpaper.g(),
        palette.wallpaper.b()
    );
    if let Some(texture) =
        ctx.data(|data| data.get_temp::<egui::TextureHandle>(egui::Id::unique(&key)))
    {
        return texture;
    }
    const SIDE: usize = 220;
    let base = palette.wallpaper;
    let ink = if palette.dark {
        egui::Color32::from_rgb(17, 25, 31)
    } else {
        egui::Color32::from_rgb(240, 233, 224)
    };
    let mut pixels = vec![base; SIDE * SIDE];
    let plot = |pixels: &mut Vec<egui::Color32>, x: i32, y: i32, radius: i32| {
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                let px = x + dx;
                let py = y + dy;
                if px < 0 || py < 0 || px >= SIDE as i32 || py >= SIDE as i32 {
                    continue;
                }
                pixels[py as usize * SIDE + px as usize] = ink;
            }
        }
    };
    // A deterministic scatter of small doodles: rings, blocks and dashes.
    let mut seed = 0x9E37_79B9_7F4A_7C15u64;
    for _ in 0..26 {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        let x = (seed % SIDE as u64) as i32;
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        let y = (seed % SIDE as u64) as i32;
        match seed % 3 {
            0 => {
                for angle in 0..40 {
                    let step = angle as f32 * std::f32::consts::TAU / 40.0;
                    plot(
                        &mut pixels,
                        x + (step.cos() * 7.0) as i32,
                        y + (step.sin() * 7.0) as i32,
                        1,
                    );
                }
            }
            1 => {
                for dy in 0..4 {
                    for dx in 0..7 {
                        plot(&mut pixels, x + dx, y + dy, 0);
                    }
                }
            }
            _ => {
                for dx in 0..11 {
                    plot(&mut pixels, x + dx, y, 0);
                }
            }
        }
    }
    let image = egui::ColorImage {
        size: [SIDE, SIDE],
        source_size: egui::vec2(SIDE as f32, SIDE as f32),
        pixels,
    };
    let texture = ctx.load_texture(key.clone(), image, egui::TextureOptions::LINEAR);
    ctx.data_mut(|data| data.insert_temp(egui::Id::unique(&key), texture.clone()));
    texture
}

/// Paint the wallpaper tile across `area`, like a tiled background image.
pub(crate) fn paint_wallpaper(ui: &egui::Ui, palette: &crate::theme::Palette, area: egui::Rect) {
    let texture = wallpaper_textile(ui.ctx(), palette);
    let side = 220.0;
    let painter = ui.painter().with_clip_rect(area);
    let mut y = area.top();
    while y < area.bottom() {
        let mut x = area.left();
        while x < area.right() {
            painter.image(
                texture.id(),
                egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(side, side)),
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
            x += side;
        }
        y += side;
    }
}
