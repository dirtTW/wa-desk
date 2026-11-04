//! Remote call video: the session hands the window Annex-B H.264 access units
//! (`UiEvent::CallVideo`), and this module feeds them to an `ffmpeg` child that
//! decodes to fixed-size RGBA frames the call sheet paints.
//!
//! The decode runs on two threads - one writes access units to ffmpeg's stdin,
//! one reads frames from its stdout - so the UI thread only ever clones the
//! newest frame. Writing and reading on one thread would deadlock as soon as
//! ffmpeg buffers more than a pipe's worth of output.

use std::io::{Read, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex};

/// The decoded frame size the stage expects (16:9, letterboxed by ffmpeg).
pub const WIDTH: usize = 480;
pub const HEIGHT: usize = 270;
pub const FRAME_BYTES: usize = WIDTH * HEIGHT * 4;

/// The newest decoded frame, plus a generation counter so the UI can tell when
/// it has already uploaded that frame as a texture.
#[derive(Debug, Default)]
pub struct Latest {
    generation: AtomicU64,
    frame: Mutex<Option<Vec<u8>>>,
}

impl Latest {
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Relaxed)
    }

    /// The newest frame, if one has been decoded.
    pub fn frame(&self) -> Option<Vec<u8>> {
        self.frame.lock().ok().and_then(|frame| frame.clone())
    }

    fn store(&self, frame: Vec<u8>) {
        if let Ok(mut slot) = self.frame.lock() {
            *slot = Some(frame);
            self.generation.fetch_add(1, Ordering::Relaxed);
        }
    }
}

/// A running decoder; dropping it stops the child and both threads.
pub struct CallVideo {
    tx: Sender<Vec<u8>>,
    latest: Arc<Latest>,
    stop: Arc<AtomicBool>,
    child: Arc<Mutex<Option<Child>>>,
}

impl Drop for CallVideo {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Ok(mut child) = self.child.lock()
            && let Some(mut child) = child.take()
        {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl CallVideo {
    /// Start the decoder. `None` when ffmpeg is not installed.
    pub fn start() -> Option<Self> {
        let ffmpeg = crate::video::ffmpeg()?;
        let mut child = Command::new(ffmpeg)
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                // Streaming H.264: no probing delay, no frame reordering.
                "-probesize",
                "32",
                "-analyzeduration",
                "0",
                "-f",
                "h264",
                "-i",
                "-",
                "-vf",
                // A fixed canvas keeps the raw stream parseable: every frame is
                // exactly FRAME_BYTES. Odd source sizes are letterboxed, not
                // stretched.
                "scale=480:270:force_original_aspect_ratio=decrease,pad=480:270:(ow-iw)/2:(oh-ih)/2",
                "-pix_fmt",
                "rgba",
                "-f",
                "rawvideo",
                "-",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;
        let stdin = child.stdin.take()?;
        let stdout = child.stdout.take()?;

        let (tx, rx) = channel::<Vec<u8>>();
        let latest = Arc::new(Latest::default());
        let stop = Arc::new(AtomicBool::new(false));

        // Access units -> ffmpeg.
        let writer_stop = stop.clone();
        std::thread::spawn(move || {
            let mut stdin: ChildStdin = stdin;
            while let Ok(access_unit) = rx.recv() {
                if writer_stop.load(Ordering::Relaxed) {
                    break;
                }
                if stdin.write_all(&access_unit).is_err() {
                    break;
                }
                let _ = stdin.flush();
            }
        });

        // ffmpeg -> the newest frame.
        let reader_stop = stop.clone();
        let reader_latest = latest.clone();
        std::thread::spawn(move || {
            let mut stdout: ChildStdout = stdout;
            let mut frame = vec![0u8; FRAME_BYTES];
            while !reader_stop.load(Ordering::Relaxed) {
                if stdout.read_exact(&mut frame).is_err() {
                    break;
                }
                reader_latest.store(frame.clone());
            }
        });

        Some(Self {
            tx,
            latest,
            stop,
            child: Arc::new(Mutex::new(Some(child))),
        })
    }

    /// Queue one Annex-B access unit for decoding.
    pub fn push(&self, access_unit: Vec<u8>) {
        // A full queue means the decoder is behind; dropping the frame is the
        // right call for live video.
        let _ = self.tx.send(access_unit);
    }

    pub fn latest(&self) -> &Latest {
        &self.latest
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Encode a short H.264 Annex-B stream with ffmpeg; `None` without ffmpeg.
    fn sample_stream(dir: &std::path::Path) -> Option<Vec<u8>> {
        let ffmpeg = crate::video::ffmpeg()?;
        let path = dir.join("call.h264");
        let status = Command::new(ffmpeg)
            .args([
                "-y",
                "-loglevel",
                "error",
                "-f",
                "lavfi",
                "-i",
                "testsrc=size=480x270:rate=10:duration=4",
                "-c:v",
                "libx264",
                "-preset",
                "ultrafast",
                "-tune",
                "zerolatency",
                "-pix_fmt",
                "yuv420p",
                "-f",
                "h264",
            ])
            .arg(&path)
            .status()
            .ok()?;
        if !status.success() {
            return None;
        }
        std::fs::read(&path).ok()
    }

    #[test]
    fn an_access_unit_stream_decodes_to_a_frame() {
        if Command::new("ffmpeg").arg("-version").output().is_err() {
            eprintln!("skipping: no ffmpeg on PATH");
            return;
        }
        let dir = std::env::temp_dir().join(format!("wa-callvideo-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dir");
        let Some(stream) = sample_stream(&dir) else {
            eprintln!("skipping: could not encode the sample");
            return;
        };
        let Some(decoder) = CallVideo::start() else {
            eprintln!("skipping: ffmpeg could not start the decoder");
            return;
        };
        // Feed the whole stream in pipe-sized chunks, as the session would.
        for chunk in stream.chunks(4096) {
            decoder.push(chunk.to_vec());
        }
        let mut frame = None;
        for _ in 0..100 {
            if let Some(decoded) = decoder.latest().frame()
                && decoded.iter().any(|byte| *byte != 0)
            {
                frame = Some(decoded);
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        let frame = frame.expect("no frame decoded");
        assert_eq!(frame.len(), FRAME_BYTES, "fixed canvas size");
        let first = &frame[..4];
        assert!(
            frame.as_chunks::<4>().0.iter().any(|pixel| pixel != first),
            "the frame is uniform, so nothing was really decoded"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
