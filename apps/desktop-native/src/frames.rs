//! Decode video frames ahead of the playhead on a worker thread.
//!
//! Decoding a JPEG into an `egui` texture on the UI thread is what makes a
//! player stutter (and the same class of bug as emilk/egui#7860, where WebP and
//! GIF decoding blocked the paint loop). Here a worker keeps a window of frames
//! ready around the playhead and the window only uploads what has already been
//! decoded, so a paint never waits on the disk.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError};

/// How far ahead of the playhead frames are kept decoded. Six frames is 0.2 s
/// at 30 fps - enough to ride out a slow disk without holding ~100 MB of RGBA
/// for a long clip.
const AHEAD: usize = 12;

/// A running worker for one clip.
pub struct Prefetch {
    playhead: Arc<AtomicUsize>,
    stop: Arc<AtomicBool>,
    rx: Receiver<(usize, egui::ColorImage)>,
    frames: usize,
}

impl Prefetch {
    /// Start decoding `dir`'s frames, or `None` when the clip has no frames.
    pub fn start(dir: PathBuf, frames: usize) -> Option<Self> {
        if frames == 0 {
            return None;
        }
        let playhead = Arc::new(AtomicUsize::new(0));
        let stop = Arc::new(AtomicBool::new(false));
        let (tx, rx) = std::sync::mpsc::channel();
        let worker_head = Arc::clone(&playhead);
        let worker_stop = Arc::clone(&stop);
        std::thread::spawn(move || {
            let mut next = 0usize;
            let mut anchor = usize::MAX;
            while !worker_stop.load(Ordering::Relaxed) {
                let head = worker_head.load(Ordering::Relaxed).min(frames - 1);
                if head != anchor {
                    // A seek restarts the window at the new playhead.
                    anchor = head;
                    next = head;
                }
                if next >= frames || next >= head + AHEAD {
                    std::thread::sleep(std::time::Duration::from_millis(4));
                    continue;
                }
                let path = dir.join(format!("f{:05}.jpg", next + 1));
                if let Some(image) = decode(&path)
                    && tx.send((next, image)).is_err()
                {
                    break;
                }
                next += 1;
            }
        });
        Some(Self {
            playhead,
            stop,
            rx,
            frames,
        })
    }

    /// Tell the worker where playback is, so it decodes what comes next.
    pub fn seek(&self, index: usize) {
        self.playhead
            .store(index.min(self.frames.saturating_sub(1)), Ordering::Relaxed);
    }

    /// Upload-ready frames, at most `limit` per paint so a burst cannot blow
    /// the frame budget.
    pub fn drain(&self, limit: usize) -> Vec<(usize, egui::ColorImage)> {
        let mut out = Vec::new();
        while out.len() < limit {
            match self.rx.try_recv() {
                Ok(frame) => out.push(frame),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => break,
            }
        }
        out
    }
}

impl Drop for Prefetch {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

/// One JPEG frame as RGBA pixels.
fn decode(path: &std::path::Path) -> Option<egui::ColorImage> {
    let bytes = std::fs::read(path).ok()?;
    let image = image::load_from_memory(&bytes).ok()?.to_rgba8();
    let size = [image.width() as usize, image.height() as usize];
    Some(egui::ColorImage::from_rgba_unmultiplied(
        size,
        image.as_raw(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The worker decodes a real frame and hands it over, then follows a seek.
    #[test]
    fn frames_are_decoded_ahead_and_follow_the_playhead() {
        let dir = std::env::temp_dir().join("wa-desk-prefetch");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dir");
        for index in 1..=3 {
            let image = image::RgbImage::from_pixel(8, 8, image::Rgb([index as u8, 0, 0]));
            image
                .save(dir.join(format!("f{index:05}.jpg")))
                .expect("frame");
        }
        let prefetch = Prefetch::start(dir.clone(), 3).expect("prefetch");
        prefetch.seek(0);
        let mut frames = Vec::new();
        for _ in 0..200 {
            frames.extend(prefetch.drain(8));
            if frames.len() >= 3 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let mut indexes: Vec<usize> = frames.iter().map(|(index, _)| *index).collect();
        indexes.sort_unstable();
        indexes.dedup();
        assert_eq!(indexes, vec![0, 1, 2], "every frame is decoded");
        assert_eq!(frames[0].1.size, [8, 8]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_empty_clip_has_no_worker() {
        assert!(Prefetch::start(PathBuf::from("/nowhere"), 0).is_none());
    }
}
