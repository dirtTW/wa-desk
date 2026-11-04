//! Turning a picture into a sticker: the picker's Create tile takes an image,
//! ffmpeg crops it to the 512x512 WebP WhatsApp expects, and the session sends
//! it as a `stickerMessage`.

use std::path::Path;
use std::process::{Command, Stdio};

/// The sticker canvas WhatsApp uses.
pub const SIZE: u32 = 512;

/// Encode `source` into a 512x512 WebP sticker (RGBA, lossy, square crop).
pub fn make_sticker(source: &Path) -> Result<Vec<u8>, String> {
    if !source.is_file() {
        return Err("the selected picture does not exist".to_string());
    }
    let ffmpeg = crate::video::ffmpeg().ok_or_else(|| "ffmpeg is not available".to_string())?;
    let child = Command::new(ffmpeg)
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-i",
            &source.to_string_lossy(),
            "-vf",
            // Cover the square, then crop the overflow: the same framing the
            // official sticker maker uses.
            &format!("scale={SIZE}:{SIZE}:force_original_aspect_ratio=increase,crop={SIZE}:{SIZE}"),
            "-frames:v",
            "1",
            "-c:v",
            "libwebp",
            "-lossless",
            "0",
            "-q:v",
            "80",
            "-f",
            "webp",
            "-",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|err| format!("could not start ffmpeg: {err}"))?;
    let output = child
        .wait_with_output()
        .map_err(|err| format!("ffmpeg failed: {err}"))?;
    if !output.status.success() || output.stdout.is_empty() {
        return Err("ffmpeg could not encode the sticker".to_string());
    }
    if !output.stdout.starts_with(b"RIFF") {
        return Err("the encoder did not produce a WebP".to_string());
    }
    Ok(output.stdout)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Is a usable ffmpeg on this machine? CI images have none, so the tests
    /// that need it skip instead of failing.
    fn ffmpeg_works() -> bool {
        crate::video::ffmpeg()
            .and_then(|ffmpeg| Command::new(ffmpeg).arg("-version").output().ok())
            .is_some_and(|output| output.status.success())
    }

    /// A test picture: ffmpeg's colour source, so no fixture is needed.
    fn sample(dir: &std::path::Path) -> Option<std::path::PathBuf> {
        let ffmpeg = crate::video::ffmpeg()?;
        let path = dir.join("picture.png");
        let status = Command::new(ffmpeg)
            .args([
                "-y",
                "-loglevel",
                "error",
                "-f",
                "lavfi",
                "-i",
                "gradients=size=800x600:duration=1",
                "-frames:v",
                "1",
            ])
            .arg(&path)
            .status()
            .ok()?;
        status.success().then_some(path)
    }

    #[test]
    fn a_picture_becomes_a_square_webp_sticker() {
        if !ffmpeg_works() {
            eprintln!("skipping: no ffmpeg");
            return;
        }
        let dir = std::env::temp_dir().join(format!("wa-sticker-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dir");
        let Some(picture) = sample(&dir) else {
            eprintln!("skipping: could not render the sample");
            return;
        };
        let bytes = make_sticker(&picture).expect("sticker");
        assert!(bytes.starts_with(b"RIFF"), "WebP container");
        assert!(&bytes[8..12] == b"WEBP", "WebP signature");
        let image = image::load_from_memory(&bytes).expect("decode the sticker");
        assert_eq!(image.width(), SIZE);
        assert_eq!(image.height(), SIZE);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_missing_picture_is_an_error() {
        let path = std::env::temp_dir().join("wa-desk-does-not-exist.png");
        let _ = std::fs::remove_file(&path);
        assert!(make_sticker(&path).is_err());
    }

    #[test]
    fn the_sticker_keeps_alpha() {
        // The encoder writes RGBA WebP; `image` decodes it with alpha intact.
        if !ffmpeg_works() {
            eprintln!("skipping: no ffmpeg");
            return;
        }
        let dir = std::env::temp_dir().join(format!("wa-sticker-alpha-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("tiny.png");
        // A 4x4 transparent PNG, written by hand so the test needs no fixture.
        let pixels = image::RgbaImage::from_pixel(4, 4, image::Rgba([255, 0, 0, 128]));
        pixels.save(&path).expect("write png");
        let bytes = make_sticker(&path).expect("sticker");
        let decoded = image::load_from_memory(&bytes).expect("decode");
        assert_eq!(decoded.width(), SIZE);
        assert_eq!(decoded.height(), SIZE);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
