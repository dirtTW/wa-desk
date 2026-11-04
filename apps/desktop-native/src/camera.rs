//! Outgoing call video: the default webcam is captured by `ffmpeg`'s DirectShow
//! input, encoded to H.264 by a second `ffmpeg` child, and split into Annex-B
//! access units for `Session::send_call_video`.
//!
//! Doing both stages with ffmpeg avoids linking a camera API: the only platform
//! detail is the DirectShow device name, which `device()` discovers.

use std::io::{Read, Write};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

/// The capture size and rate the encoder expects.
pub const WIDTH: usize = 640;
pub const HEIGHT: usize = 480;
pub const FPS: u32 = 15;
pub const FRAME_BYTES: usize = WIDTH * HEIGHT * 4;

/// The self-view size (a quarter of the capture, for a corner thumbnail).
pub const PREVIEW_WIDTH: usize = 160;
pub const PREVIEW_HEIGHT: usize = 120;

/// The newest preview frame, for the call sheet's self-view.
#[derive(Debug, Default)]
pub struct Preview {
    generation: std::sync::atomic::AtomicU64,
    frame: Mutex<Option<Vec<u8>>>,
}

impl Preview {
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Relaxed)
    }

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

/// Find the webcam: `WA_DESK_CAMERA` wins, otherwise the first DirectShow video
/// device ffmpeg lists.
pub fn device() -> Option<String> {
    if let Ok(name) = std::env::var("WA_DESK_CAMERA")
        && !name.trim().is_empty()
    {
        return Some(name);
    }
    let ffmpeg = crate::video::ffmpeg()?;
    let output = crate::spawn::quiet(ffmpeg)
        .args([
            "-hide_banner",
            "-list_devices",
            "true",
            "-f",
            "dshow",
            "-i",
            "dummy",
        ])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stderr);
    for line in text.lines() {
        let line = line.trim();
        if line.contains("(video)")
            && let Some(start) = line.find('"')
            && let Some(end) = line[start + 1..].find('"')
        {
            return Some(line[start + 1..start + 1 + end].to_string());
        }
    }
    None
}

/// The capture child: DirectShow frames as RGBA rawvideo on stdout.
fn capture_command(device: &str) -> Command {
    let mut command =
        crate::spawn::quiet(crate::video::ffmpeg().unwrap_or_else(|| "ffmpeg".into()));
    command.args([
        "-hide_banner",
        "-loglevel",
        "error",
        "-rtbufsize",
        "16M",
        "-f",
        "dshow",
        "-framerate",
        &FPS.to_string(),
        "-video_size",
        &format!("{WIDTH}x{HEIGHT}"),
        "-i",
        &format!("video={device}"),
        "-pix_fmt",
        "rgba",
        "-f",
        "rawvideo",
        "-",
    ]);
    command
}

/// The encoder child: RGBA frames in, Annex-B H.264 out.
pub fn encoder_command() -> Command {
    let mut command =
        crate::spawn::quiet(crate::video::ffmpeg().unwrap_or_else(|| "ffmpeg".into()));
    command.args([
        "-hide_banner",
        "-loglevel",
        "error",
        "-f",
        "rawvideo",
        "-pix_fmt",
        "rgba",
        "-s",
        &format!("{WIDTH}x{HEIGHT}"),
        "-r",
        &FPS.to_string(),
        "-i",
        "-",
        "-c:v",
        "libx264",
        "-preset",
        "ultrafast",
        "-tune",
        "zerolatency",
        "-profile:v",
        "baseline",
        "-g",
        "30",
        "-pix_fmt",
        "yuv420p",
        "-f",
        "h264",
        "-",
    ]);
    command
}

/// Split an Annex-B byte stream into access units. One H.264 frame is one start
/// code group that ends where the next picture (or a new SPS) begins.
pub fn split_access_units(chunk: &[u8], pending: &mut Vec<u8>) -> Vec<Vec<u8>> {
    pending.extend_from_slice(chunk);
    let mut units = Vec::new();
    loop {
        // Find the second start code: everything before it is one unit.
        let mut starts = Vec::new();
        let mut index = 0;
        while index + 3 <= pending.len() {
            if pending[index..].starts_with(&[0, 0, 1]) {
                starts.push(index);
                index += 3;
            } else if pending[index..].starts_with(&[0, 0, 0, 1]) {
                starts.push(index);
                index += 4;
            } else {
                index += 1;
            }
        }
        if starts.len() < 2 {
            break;
        }
        // A unit may hold several NALs (SPS, PPS, SEI, then the slice); it ends
        // where a picture NAL appears after another picture NAL.
        let mut boundary = None;
        let mut seen_picture = false;
        for (position, start) in starts.iter().enumerate() {
            let header = pending.get(start + 3).copied().unwrap_or(0);
            // Long start codes carry the NAL header one byte later.
            let header = if header & 0x1F == 0 && pending.get(start + 4).is_some() {
                pending[start + 4] & 0x1F
            } else {
                header & 0x1F
            };
            let picture = header == 1 || header == 5;
            if picture && seen_picture {
                boundary = Some(*start);
                break;
            }
            if header == 7 && seen_picture {
                boundary = Some(*start);
                break;
            }
            seen_picture |= picture;
            let _ = position;
        }
        let Some(boundary) = boundary else {
            break;
        };
        units.push(pending[..boundary].to_vec());
        pending.drain(..boundary);
    }
    units
}

/// A running capture + encode; dropping it stops the camera and both children.
pub struct Camera {
    frames: Mutex<Receiver<Vec<u8>>>,
    preview: Arc<Preview>,
    stop: Arc<AtomicBool>,
    children: Arc<Mutex<Vec<Child>>>,
}

impl Drop for Camera {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Ok(mut children) = self.children.lock() {
            for child in children.iter_mut() {
                let _ = child.kill();
                let _ = child.wait();
            }
            children.clear();
        }
    }
}

impl Camera {
    /// Start capturing; `None` when there is no camera or ffmpeg is missing.
    pub fn start() -> Option<Self> {
        let device = device()?;
        let mut capture = capture_command(&device)
            .stdout(Stdio::piped())
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;
        let mut encoder = encoder_command()
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;
        let capture_out = capture.stdout.take()?;
        let encoder_in = encoder.stdin.take()?;
        let encoder_out = encoder.stdout.take()?;

        let (tx, frames) = channel::<Vec<u8>>();
        let preview = Arc::new(Preview::default());
        let stop = Arc::new(AtomicBool::new(false));

        // Capture -> encoder (and a downscaled self-view every third frame).
        let pump_stop = stop.clone();
        let pump_preview = preview.clone();
        std::thread::spawn(move || {
            let mut reader = std::io::BufReader::new(capture_out);
            let mut writer = std::io::BufWriter::new(encoder_in);
            let mut frame = vec![0u8; FRAME_BYTES];
            let mut count = 0usize;
            while !pump_stop.load(Ordering::Relaxed) {
                if reader.read_exact(&mut frame).is_err() {
                    break;
                }
                if writer.write_all(&frame).is_err() {
                    break;
                }
                let _ = writer.flush();
                count += 1;
                if count.is_multiple_of(3) {
                    pump_preview.store(downscale(&frame));
                }
            }
        });

        // Encoder -> access units.
        let read_stop = stop.clone();
        std::thread::spawn(move || {
            let mut reader = std::io::BufReader::new(encoder_out);
            let mut buffer = [0u8; 8192];
            let mut pending: Vec<u8> = Vec::new();
            let mut sent = 0usize;
            while !read_stop.load(Ordering::Relaxed) {
                let Ok(read) = reader.read(&mut buffer) else {
                    break;
                };
                if read == 0 {
                    break;
                }
                for unit in split_access_units(&buffer[..read], &mut pending) {
                    sent += 1;
                    if tx.send(unit).is_err() {
                        return;
                    }
                }
            }
            // Flush whatever the encoder left behind.
            if !pending.is_empty() {
                let _ = tx.send(std::mem::take(&mut pending));
            }
            let _ = sent;
        });

        Some(Self {
            frames: Mutex::new(frames),
            preview,
            stop,
            children: Arc::new(Mutex::new(vec![capture, encoder])),
        })
    }

    /// Wait up to `timeout` for the next encoded access unit.
    pub fn recv_timeout(&self, timeout: std::time::Duration) -> Option<Vec<u8>> {
        let frames = self.frames.lock().ok()?;
        frames.recv_timeout(timeout).ok()
    }

    /// The newest self-view frame (160x120 RGBA).
    pub fn preview(&self) -> &Preview {
        &self.preview
    }
}

/// Nearest-neighbour downscale to the preview size.
fn downscale(frame: &[u8]) -> Vec<u8> {
    let mut preview = Vec::with_capacity(PREVIEW_WIDTH * PREVIEW_HEIGHT * 4);
    let step_x = WIDTH / PREVIEW_WIDTH;
    let step_y = HEIGHT / PREVIEW_HEIGHT;
    for y in (0..HEIGHT).step_by(step_y).take(PREVIEW_HEIGHT) {
        for x in (0..WIDTH).step_by(step_x).take(PREVIEW_WIDTH) {
            let offset = (y * WIDTH + x) * 4;
            preview.extend_from_slice(&frame[offset..offset + 4]);
        }
    }
    preview
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_stream(dir: &std::path::Path) -> Option<Vec<u8>> {
        let ffmpeg = crate::video::ffmpeg()?;
        let path = dir.join("camera.h264");
        let status = crate::spawn::quiet(ffmpeg)
            .args([
                "-y",
                "-loglevel",
                "error",
                "-f",
                "lavfi",
                "-i",
                "testsrc=size=320x240:rate=10:duration=1",
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
    fn the_stream_splits_into_one_unit_per_frame() {
        let dir = std::env::temp_dir().join(format!("wa-camera-split-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dir");
        let Some(stream) = sample_stream(&dir) else {
            eprintln!("skipping: no ffmpeg");
            return;
        };
        let mut pending = Vec::new();
        let mut units = Vec::new();
        // Feed it the way a pipe would, in small chunks.
        for chunk in stream.chunks(97) {
            units.extend(split_access_units(chunk, &mut pending));
        }
        units.push(std::mem::take(&mut pending));
        assert!(
            units.len() >= 10,
            "expected at least a unit per frame, got {}",
            units.len()
        );
        for unit in &units {
            assert!(
                unit.starts_with(&[0, 0, 0, 1]) || unit.starts_with(&[0, 0, 1]),
                "an access unit must start with a start code"
            );
        }
        let joined: Vec<u8> = units.concat();
        assert_eq!(joined, stream, "splitting must not lose or reorder bytes");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn raw_frames_encode_into_access_units() {
        let Some(ffmpeg) = crate::video::ffmpeg() else {
            eprintln!("skipping: no ffmpeg");
            return;
        };
        if crate::spawn::quiet(ffmpeg)
            .arg("-version")
            .output()
            .is_err()
        {
            eprintln!("skipping: no ffmpeg");
            return;
        }
        let mut child = encoder_command()
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("encoder");
        let mut stdin = child.stdin.take().expect("stdin");
        let mut stdout = child.stdout.take().expect("stdout");
        // Two frames of a moving gradient.
        for frame in 0..2u8 {
            let mut raw = vec![0u8; FRAME_BYTES];
            for (index, pixel) in raw.as_chunks_mut::<4>().0.iter_mut().enumerate() {
                pixel[0] = (index % 255) as u8;
                pixel[1] = frame * 100;
                pixel[2] = (index / 7 % 255) as u8;
                pixel[3] = 255;
            }
            stdin.write_all(&raw).expect("write frame");
        }
        stdin.flush().expect("flush");
        drop(stdin);

        let mut stream = Vec::new();
        let mut buffer = [0u8; 8192];
        loop {
            match stdout.read(&mut buffer) {
                Ok(0) | Err(_) => break,
                Ok(read) => stream.extend_from_slice(&buffer[..read]),
            }
        }
        let _ = child.wait();
        assert!(!stream.is_empty(), "no H.264 came out");
        let mut pending = Vec::new();
        let units = split_access_units(&stream, &mut pending);
        assert!(
            !units.is_empty() || !pending.is_empty(),
            "the encoder output did not split into access units"
        );
    }

    #[test]
    fn the_preview_is_a_quarter_of_the_capture() {
        let frame: Vec<u8> = (0..FRAME_BYTES).map(|index| (index % 251) as u8).collect();
        let preview = downscale(&frame);
        assert_eq!(preview.len(), PREVIEW_WIDTH * PREVIEW_HEIGHT * 4);
    }
}
