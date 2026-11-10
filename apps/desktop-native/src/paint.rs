//! Painted details that several screens share: the empty filmstrip tile, the
//! circular profile photo, the per-sender name colours and the bounded texture
//! caches.
//!
//! Split out of `app.rs`; `app.rs` re-exports these names so the call sites keep
//! importing them from there.

use crate::app::COLORS;

/// The placeholder fill behind a filmstrip cell that has no thumbnail yet.
pub(crate) fn palette_thumb() -> egui::Color32 {
    egui::Color32::from_rgb(38, 42, 44)
}

/// Decode a profile photo into a circular avatar texture.
pub(crate) fn circular_avatar(
    ctx: &egui::Context,
    key: &str,
    bytes: &[u8],
) -> Option<egui::TextureHandle> {
    let image = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (width, height) = image.dimensions();
    let side = width.min(height).max(1);
    let x0 = (width - side) / 2;
    let y0 = (height - side) / 2;
    let radius = side as f32 / 2.0;
    let mut pixels = Vec::with_capacity((side * side * 4) as usize);
    for y in 0..side {
        for x in 0..side {
            let pixel = image.get_pixel(x0 + x, y0 + y);
            let dx = x as f32 + 0.5 - radius;
            let dy = y as f32 + 0.5 - radius;
            let alpha = if dx * dx + dy * dy <= radius * radius {
                pixel[3]
            } else {
                0
            };
            pixels.extend_from_slice(&[pixel[0], pixel[1], pixel[2], alpha]);
        }
    }
    let color = egui::ColorImage::from_rgba_unmultiplied([side as usize, side as usize], &pixels);
    Some(ctx.load_texture(key, color, egui::TextureOptions::LINEAR))
}

/// Group sender names are coloured per sender, like the official client.
pub(crate) fn sender_color(name: &str, palette: &crate::theme::Palette) -> egui::Color32 {
    let color = hex_color(COLORS[hash(name) % COLORS.len()]);
    if palette.dark {
        color
    } else {
        // Darken for the light theme so the names stay readable on white.
        egui::Color32::from_rgb(color.r() * 3 / 4, color.g() * 3 / 4, color.b() * 4 / 5)
    }
}

fn hex_color(value: u32) -> egui::Color32 {
    egui::Color32::from_rgb((value >> 16) as u8, (value >> 8) as u8, value as u8)
}

fn hash(text: &str) -> usize {
    text.bytes().fold(0u32, |acc, byte| {
        acc.wrapping_mul(31).wrapping_add(byte as u32)
    }) as usize
}

/// Decode the inline thumbnail (a data URI) into a texture, cached under
/// thumb-<id>. Used only while the real file is still downloading, so the
/// bubble is never an empty placeholder.
pub(crate) fn thumb_texture(
    ctx: &egui::Context,
    cache: &mut std::collections::HashMap<String, Option<egui::TextureHandle>>,
    id: &str,
    thumb: Option<&str>,
) -> Option<egui::TextureHandle> {
    use base64::Engine as _;
    let data = thumb?;
    let key = format!("thumb-{id}");
    if let Some(texture) = cache.get(&key) {
        return texture.clone();
    }
    let encoded = data.split_once(',').map(|(_, body)| body).unwrap_or(data);
    let texture = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .ok()
        .and_then(|bytes| image::load_from_memory(&bytes).ok())
        .map(|image| {
            let image = image.to_rgba8();
            let size = [image.width() as usize, image.height() as usize];
            let color = egui::ColorImage::from_rgba_unmultiplied(size, image.as_raw());
            ctx.load_texture(key.clone(), color, egui::TextureOptions::LINEAR)
        });
    cache.insert(key, texture.clone());
    texture
}

/// Keep the texture caches bounded: drop the least-recently-painted entries
/// once a cache grows past `cap`. This is what stops a long session from
/// holding every photo and sticker it ever scrolled past in video memory.
pub(crate) fn prune_textures(
    ctx: &egui::Context,
    cache: &mut std::collections::HashMap<String, Option<egui::TextureHandle>>,
    cap: usize,
) {
    if cache.len() <= cap {
        return;
    }
    let now = ctx.cumulative_frame_nr();
    let mut used: Vec<(u64, String)> = cache
        .keys()
        .map(|key| {
            let stamp = ctx
                .data(|data| data.get_temp::<u64>(egui::Id::unique(("texture-use", key.as_str()))))
                .unwrap_or(0);
            (stamp, key.clone())
        })
        .collect();
    used.sort_unstable();
    // Everything painted this frame stays; the rest goes oldest first.
    let mut live = used.len();
    let kept: Vec<(u64, String)> = used;
    for (stamp, key) in &kept {
        if live <= cap {
            break;
        }
        if *stamp == now {
            continue;
        }
        cache.remove(key);
        live -= 1;
    }
    // A single frame that painted more than `cap` textures is still capped:
    // those extra ones simply re-decode next frame.
    for (_, key) in &kept {
        if cache.len() <= cap {
            break;
        }
        cache.remove(key);
    }
}
