//! Animated WebP decoding, without libwebp and without `ffmpeg`.
//!
//! WhatsApp stickers are WebP animations (a RIFF `ANIM` chunk plus `ANMF`
//! frames). This machine's `ffmpeg` has no `webp_anim` demuxer, so the previous
//! "decode it like a GIF" path silently produced nothing and the sticker stayed
//! a placeholder. Instead the container is parsed here and each frame's nested
//! `VP8`/`VP8L` bitstream is decoded with the `image` crate (which already ships
//! a pure-Rust webp decoder), then composited onto a canvas using the
//! specification's blend and disposal rules.

use std::path::Path;

/// One fully composited frame, already scaled for the window.
pub struct Frame {
    pub image: egui::ColorImage,
    /// How long this frame stays on screen.
    pub duration_ms: u32,
}

/// Read a 24-bit little-endian integer.
fn u24(bytes: &[u8]) -> u32 {
    u32::from(bytes[0]) | (u32::from(bytes[1]) << 8) | (u32::from(bytes[2]) << 16)
}

/// Walk the RIFF chunks from `start` and return `(fourcc, payload)` for each.
fn chunks_from(data: &[u8], start: usize) -> Vec<(&[u8; 4], &[u8])> {
    let mut out = Vec::new();
    let mut index = start;
    while index + 8 <= data.len() {
        let fourcc: &[u8; 4] = data[index..index + 4].try_into().expect("four bytes");
        let size = u32::from_le_bytes(
            data[index + 4..index + 8]
                .try_into()
                .expect("four bytes for the size"),
        ) as usize;
        let start = index + 8;
        let end = (start + size).min(data.len());
        out.push((fourcc, &data[start..end]));
        // Chunks are word aligned.
        index = start + size + (size & 1);
        if out.len() > 4_000 {
            break;
        }
    }
    out
}

/// The chunks of a whole `.webp` file (after `RIFF<size>WEBP`).
fn chunks(data: &[u8]) -> Vec<(&[u8; 4], &[u8])> {
    chunks_from(data, 12)
}

/// Rebuild a single still image out of one frame's nested chunks, so the
/// `image` crate's webp decoder can read it.
fn still(frame_chunks: &[(&[u8; 4], &[u8])]) -> Option<Vec<u8>> {
    let mut body = Vec::new();
    for (fourcc, payload) in frame_chunks {
        if !matches!(*fourcc, b"ALPH" | b"VP8 " | b"VP8L") {
            continue;
        }
        body.extend_from_slice(*fourcc);
        body.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        body.extend_from_slice(payload);
        if payload.len() & 1 == 1 {
            body.push(0);
        }
    }
    if body.is_empty() {
        return None;
    }
    let mut file = Vec::with_capacity(body.len() + 12);
    file.extend_from_slice(b"RIFF");
    file.extend_from_slice(&((body.len() + 4) as u32).to_le_bytes());
    file.extend_from_slice(b"WEBP");
    file.extend_from_slice(&body);
    Some(file)
}

/// Is this file an animated WebP? (A still WebP has no `ANIM` chunk.)
pub fn is_animated(path: &Path) -> bool {
    let Ok(data) = std::fs::read(path) else {
        return false;
    };
    data.len() > 16
        && &data[0..4] == b"RIFF"
        && &data[8..12] == b"WEBP"
        && chunks(&data).iter().any(|(fourcc, _)| *fourcc == b"ANIM")
}

/// Decode every frame of an animated WebP, composited and scaled to `width`.
pub fn frames(path: &Path, width: u32) -> Option<Vec<Frame>> {
    let data = std::fs::read(path).ok()?;
    if data.len() < 16 || &data[0..4] != b"RIFF" || &data[8..12] != b"WEBP" {
        return None;
    }
    let all = chunks(&data);
    // Canvas size comes from the `VP8X` header: width-1, height-1 as u24.
    let mut canvas_size = None;
    let mut background = image::Rgba([0, 0, 0, 0]);
    for (fourcc, payload) in &all {
        if *fourcc == b"VP8X" && payload.len() >= 10 {
            let w = u24(&payload[4..7]) + 1;
            let h = u24(&payload[7..10]) + 1;
            canvas_size = Some((w, h));
        }
        if *fourcc == b"ANIM" && payload.len() >= 6 {
            // BGRA order in the specification.
            background = image::Rgba([payload[2], payload[1], payload[0], 255]);
        }
    }

    // The chunk list already gives each ANMF payload; re-walk it to build the
    // per-frame nested chunk lists.
    let mut out: Vec<Frame> = Vec::new();
    let (mut canvas_w, mut canvas_h) = canvas_size.unwrap_or((512, 512));
    if canvas_size.is_none() {
        // Older files without VP8X: take the first frame's own size.
        canvas_w = 512;
        canvas_h = 512;
    }
    let mut canvas = image::RgbaImage::from_pixel(canvas_w, canvas_h, background);

    for (fourcc, payload) in &all {
        if *fourcc != b"ANMF" || payload.len() < 16 {
            continue;
        }
        let left = u24(&payload[0..3]) * 2;
        let top = u24(&payload[3..6]) * 2;
        let width_px = u24(&payload[6..9]) + 1;
        let height_px = u24(&payload[9..12]) + 1;
        let duration = u24(&payload[12..15]);
        let flags = payload[15];
        let dispose_to_background = flags & 0b1 != 0;
        // Bit 1: 0 = alpha blend, 1 = overwrite the rectangle.
        let blend = flags & 0b10 == 0;

        // ANMF payloads are a 16-byte frame header followed by the nested
        // chunks (there is no RIFF header inside a frame).
        let nested = chunks_from(payload, 16);
        let Some(file) = still(&nested) else {
            continue;
        };
        let Ok(decoded) = image::load_from_memory(&file) else {
            continue;
        };
        let decoded = decoded.to_rgba8();
        if canvas_size.is_none() {
            canvas_w = decoded.width();
            canvas_h = decoded.height();
            canvas = image::RgbaImage::from_pixel(canvas_w, canvas_h, background);
        }
        // Place the frame on the canvas.
        for y in 0..height_px.min(decoded.height()) {
            for x in 0..width_px.min(decoded.width()) {
                let source = decoded.get_pixel(x, y);
                let canvas_x = left + x;
                let canvas_y = top + y;
                if canvas_x >= canvas.width() || canvas_y >= canvas.height() {
                    continue;
                }
                if blend && source[3] > 0 {
                    // Source-over, the specified blend method.
                    let destination = canvas.get_pixel(canvas_x, canvas_y);
                    let sa = f32::from(source[3]) / 255.0;
                    let da = f32::from(destination[3]) / 255.0;
                    let out_a = sa + da * (1.0 - sa);
                    let channel = |s: u8, d: u8| -> u8 {
                        if out_a <= 0.0 {
                            0
                        } else {
                            ((f32::from(s) * sa + f32::from(d) * da * (1.0 - sa)) / out_a)
                                .round()
                                .clamp(0.0, 255.0) as u8
                        }
                    };
                    canvas.put_pixel(
                        canvas_x,
                        canvas_y,
                        image::Rgba([
                            channel(source[0], destination[0]),
                            channel(source[1], destination[1]),
                            channel(source[2], destination[2]),
                            (out_a * 255.0).round() as u8,
                        ]),
                    );
                } else {
                    canvas.put_pixel(canvas_x, canvas_y, *source);
                }
            }
        }
        // Scale down for the window once, so playback only uploads pixels.
        let scale = (width as f32 / canvas.width() as f32).min(1.0);
        let scaled = if scale < 1.0 {
            image::imageops::resize(
                &canvas,
                (canvas.width() as f32 * scale) as u32,
                (canvas.height() as f32 * scale) as u32,
                image::imageops::FilterType::Triangle,
            )
        } else {
            canvas.clone()
        };
        out.push(Frame {
            image: egui::ColorImage::from_rgba_unmultiplied(
                [scaled.width() as usize, scaled.height() as usize],
                scaled.as_raw(),
            ),
            duration_ms: duration.max(20),
        });
        if dispose_to_background {
            for y in top..(top + height_px).min(canvas.height()) {
                for x in left..(left + width_px).min(canvas.width()) {
                    canvas.put_pixel(x, y, image::Rgba([0, 0, 0, 0]));
                }
            }
        }
    }
    if out.is_empty() { None } else { Some(out) }
}

/* ------------------------------------------------------------ playback */

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// One decoded animation, shared with the paint thread.
type Shared = std::sync::Arc<Vec<Frame>>;
/// Decoded animations, keyed by sticker/message id (`None` while decoding).
type Decoded = HashMap<String, Option<Shared>>;

fn decoded() -> &'static Mutex<Decoded> {
    static DECODED: OnceLock<Mutex<Decoded>> = OnceLock::new();
    DECODED.get_or_init(|| Mutex::new(HashMap::new()))
}

/// When each animation started, so it loops from the first frame.
fn starts() -> &'static Mutex<HashMap<String, std::time::Instant>> {
    static STARTS: OnceLock<Mutex<HashMap<String, std::time::Instant>>> = OnceLock::new();
    STARTS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Is any animated sticker decoded and therefore running? The window only needs
/// the fast repaint when this is true.
pub fn any_ready() -> bool {
    decoded()
        .lock()
        .map(|all| all.values().any(|entry| entry.is_some()))
        .unwrap_or(false)
}

/// The shortest frame of any decoded animation, so the window can repaint at
/// the animation's own rate instead of a fixed 10 fps.
pub fn shortest_frame_ms() -> Option<u32> {
    let all = decoded().lock().ok()?;
    all.values()
        .flatten()
        .flat_map(|frames| frames.iter().map(|frame| frame.duration_ms))
        .min()
}

/// Forget every decoded animation (another chat was opened).
pub fn clear() {
    if let Ok(mut all) = decoded().lock() {
        all.clear();
    }
    if let Ok(mut starts) = starts().lock() {
        starts.clear();
    }
}

/// The frame to paint for the animated sticker `id`, decoding on a worker the
/// first time it is seen. `None` until the first frame is ready, which keeps
/// the paint thread free (this is the same rule as the video prefetch).
pub fn frame(
    ctx: &egui::Context,
    textures: &mut HashMap<String, Option<egui::TextureHandle>>,
    id: &str,
    path: &str,
) -> Option<egui::TextureHandle> {
    let animation = {
        let mut all = decoded().lock().ok()?;
        match all.get(id) {
            Some(entry) => entry.clone(),
            None => {
                all.insert(id.to_string(), None);
                let id = id.to_string();
                let path = std::path::PathBuf::from(path);
                std::thread::spawn(move || {
                    let frames = frames(&path, 192).map(std::sync::Arc::new);
                    if let Ok(mut all) = decoded().lock() {
                        all.insert(id.clone(), frames);
                    }
                });
                return None;
            }
        }
    }?;
    if animation.is_empty() {
        return None;
    }
    let started = {
        let mut starts = starts().lock().ok()?;
        *starts
            .entry(id.to_string())
            .or_insert_with(std::time::Instant::now)
    };
    let index = frame_at(&animation, started.elapsed().as_millis() as u64);
    let key = format!("anim:{id}#{index}");
    if let Some(Some(texture)) = textures.get(&key) {
        return Some(texture.clone());
    }
    let texture = ctx.load_texture(
        key.clone(),
        animation[index].image.clone(),
        egui::TextureOptions::LINEAR,
    );
    if textures.len() > 400 {
        textures.clear();
    }
    textures.insert(key, Some(texture.clone()));
    Some(texture)
}

/// The frame that is showing `elapsed_ms` into the animation.
pub fn frame_at(frames: &[Frame], elapsed_ms: u64) -> usize {
    if frames.is_empty() {
        return 0;
    }
    let total: u64 = frames
        .iter()
        .map(|frame| u64::from(frame.duration_ms))
        .sum();
    if total == 0 {
        return 0;
    }
    let mut position = elapsed_ms % total;
    for (index, frame) in frames.iter().enumerate() {
        let duration = u64::from(frame.duration_ms);
        if position < duration {
            return index;
        }
        position -= duration;
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_chunk_walker_reads_the_animation_and_frames() {
        let mut file = Vec::new();
        file.extend_from_slice(b"RIFF");
        file.extend_from_slice(&0u32.to_le_bytes());
        file.extend_from_slice(b"WEBP");
        // ANIM with a background colour and a loop count.
        file.extend_from_slice(b"ANIM");
        file.extend_from_slice(&6u32.to_le_bytes());
        file.extend_from_slice(&[255, 255, 255, 0, 0, 0]);
        // One ANMF frame header: x=0, y=0, 1x1, 100 ms, blend.
        file.extend_from_slice(b"ANMF");
        file.extend_from_slice(&16u32.to_le_bytes());
        file.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 100, 0, 0, 2]);
        let all = chunks(&file);
        assert!(all.iter().any(|(fourcc, _)| *fourcc == b"ANIM"));
        let frames: Vec<_> = all
            .iter()
            .filter(|(fourcc, _)| *fourcc == b"ANMF")
            .collect();
        assert_eq!(frames.len(), 1);
        let payload = frames[0].1;
        assert_eq!(u24(&payload[0..3]) * 2, 0, "left");
        assert_eq!(u24(&payload[6..9]) + 1, 1, "width");
        assert_eq!(u24(&payload[12..15]), 100, "duration");
    }

    #[test]
    fn frame_timing_wraps_around_the_loop() {
        let frames = vec![
            Frame {
                image: egui::ColorImage::filled([2, 2], egui::Color32::RED),
                duration_ms: 100,
            },
            Frame {
                image: egui::ColorImage::filled([2, 2], egui::Color32::BLUE),
                duration_ms: 300,
            },
        ];
        assert_eq!(frame_at(&frames, 0), 0);
        assert_eq!(frame_at(&frames, 120), 1);
        assert_eq!(frame_at(&frames, 399), 1);
        // 400 ms is one full loop.
        assert_eq!(frame_at(&frames, 400), 0);
        assert_eq!(frame_at(&frames, 450), 0);
    }

    #[test]
    fn a_still_webp_is_not_animated() {
        let dir = std::env::temp_dir().join("wa-desk-webp-anim");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dir");
        let still = dir.join("still.webp");
        std::fs::write(&still, b"RIFF\0\0\0\0WEBPVP8 \0\0\0\0").expect("write");
        assert!(!is_animated(&still));
        let missing = dir.join("missing.webp");
        assert!(!is_animated(&missing));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
