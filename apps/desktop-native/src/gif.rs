//! Inline GIF playback. WhatsApp sends GIFs as videos with `gifPlayback`, so a
//! bubble can animate them exactly like the official client does: decode the
//! clip once with `video::prepare`, then step its frames on a loop.
//!
//! The prepared clips live in a process-wide registry because bubbles are painted
//! by free functions that only see `&Model`.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use crate::video::Video;

fn clips() -> &'static Mutex<HashMap<String, Option<Video>>> {
    static CLIPS: OnceLock<Mutex<HashMap<String, Option<Video>>>> = OnceLock::new();
    CLIPS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn workers() -> &'static Mutex<HashMap<String, crate::frames::Prefetch>> {
    static WORKERS: OnceLock<Mutex<HashMap<String, crate::frames::Prefetch>>> = OnceLock::new();
    WORKERS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn starts() -> &'static Mutex<HashMap<String, std::time::Instant>> {
    static STARTS: OnceLock<Mutex<HashMap<String, std::time::Instant>>> = OnceLock::new();
    STARTS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Which frame is showing `elapsed` seconds into a clip of `frames` frames: it
/// loops, and an empty clip reports frame 0.
pub fn frame_index(elapsed: f32, frames: usize, fps: f32) -> usize {
    if frames == 0 || fps <= 0.0 {
        return 0;
    }
    let index = (elapsed.max(0.0) * fps) as usize;
    index % frames
}

/// The frame to paint for GIF `id`, preparing the clip on first use.
///
/// Returns `None` while the decode is running (the bubble keeps the poster).
pub fn frame(
    ctx: &egui::Context,
    textures: &mut HashMap<String, Option<egui::TextureHandle>>,
    id: &str,
    source: &str,
) -> Option<egui::TextureHandle> {
    let prepared = {
        let mut all = clips().lock().ok()?;
        match all.get(id) {
            Some(clip) => clip.clone(),
            None => {
                // First sighting: mark it and decode on a worker thread.
                all.insert(id.to_string(), None);
                let id = id.to_string();
                let source = source.to_string();
                std::thread::spawn(move || {
                    let reply = crate::video::prepare(&source, &id);
                    if let Ok(mut all) = clips().lock() {
                        all.insert(id.clone(), reply.ok());
                    }
                    crate::app::diag(&format!("gif {id}: prepared"));
                });
                None
            }
        }
    }?;
    if prepared.frames == 0 {
        return None;
    }
    let started = {
        let mut starts = starts().lock().ok()?;
        *starts
            .entry(id.to_string())
            .or_insert_with(std::time::Instant::now)
    };
    let index = frame_index(
        started.elapsed().as_secs_f32(),
        prepared.frames,
        prepared.fps,
    );
    // Decode ahead on a worker: a frame decoded on the paint thread is exactly
    // the stutter (and the egui#7860 hang) this avoids.
    let dir_key = format!("gif:{id}");
    let mut workers = workers().lock().ok()?;
    if workers.get(&dir_key).is_none()
        && let Some(worker) = crate::frames::Prefetch::start(prepared.dir.clone(), prepared.frames)
    {
        workers.insert(dir_key.clone(), worker);
    }
    let worker = workers.get(&dir_key)?;
    worker.seek(index);
    for (number, image) in worker.drain(4) {
        let key = format!("{dir_key}#{number}");
        let texture = ctx.load_texture(key.clone(), image, egui::TextureOptions::LINEAR);
        textures.insert(key, Some(texture));
    }

    // The frame itself, otherwise the nearest one behind it, so a bubble never
    // blinks to the poster while a frame is in flight.
    for behind in 0..=8 {
        if behind > index {
            break;
        }
        let key = format!("{dir_key}#{}", index - behind);
        if let Some(Some(texture)) = textures.get(&key) {
            return Some(texture.clone());
        }
    }
    None
}

/// Is any GIF decoded and therefore animating right now? The window only needs
/// the fast repaint when this is true - merely *having* a GIF in the chat must
/// not make it relayout ten times a second.
pub fn any_prepared() -> bool {
    clips()
        .lock()
        .map(|all| all.values().any(|clip| clip.is_some()))
        .unwrap_or(false)
}

/// Forget a clip (used when the chat changes, so decodes do not pile up).
pub fn clear() {
    if let Ok(mut all) = clips().lock() {
        all.clear();
    }
    if let Ok(mut starts) = starts().lock() {
        starts.clear();
    }
    if let Ok(mut workers) = workers().lock() {
        workers.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_frame_index_loops() {
        let frames = 30;
        assert_eq!(frame_index(0.0, frames, 10.0), 0);
        assert_eq!(frame_index(0.1, frames, 10.0), 1);
        assert_eq!(frame_index(1.0, frames, 10.0), 10);
        // Past the end it wraps to the start and keeps going.
        assert_eq!(frame_index(3.0, frames, 10.0), 0);
        assert_eq!(frame_index(3.1, frames, 10.0), 1);
    }

    #[test]
    fn an_empty_clip_stays_on_the_first_frame() {
        assert_eq!(frame_index(5.0, 0, 10.0), 0);
        assert_eq!(frame_index(-1.0, 10, 10.0), 0);
    }
}
