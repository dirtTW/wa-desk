//! Media inputs for the vision model: photos, stickers and short videos.

use std::path::PathBuf;
use std::process::Command;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as B64;
use wa_core::Session;

/// Small data URI for photos, so a vision model can read them.
pub(super) async fn image_data_url(session: &Session, id: &str) -> Option<String> {
    let path = session.fetch_media(id).await.ok()?;
    let bytes = tokio::fs::read(path).await.ok()?;
    if bytes.is_empty() || bytes.len() > 8_000_000 {
        return None;
    }
    let mime = if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        "image/png"
    } else if bytes.starts_with(b"GIF8") {
        "image/gif"
    } else if bytes.len() > 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        "image/webp"
    } else {
        "image/jpeg"
    };
    Some(format!("data:{mime};base64,{}", B64.encode(bytes)))
}

pub(super) async fn video_frames(session: &Session, id: &str) -> Vec<String> {
    let path = match session.fetch_media(id).await {
        Ok(path) => path,
        Err(err) => {
            tracing::debug!(%err, "video frames: media unavailable");
            return Vec::new();
        }
    };
    match tokio::task::spawn_blocking(move || extract_video_frames(&path)).await {
        Ok(frames) => frames,
        Err(err) => {
            tracing::warn!(%err, "video frame task failed");
            Vec::new()
        }
    }
}

pub(super) fn ffmpeg_binary() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("WA_DESK_FFMPEG") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Some(path);
        }
    }

    // A portable build may ship ffmpeg next to the exe.
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        for name in ["ffmpeg.exe", "ffmpeg"] {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    let works = Command::new("ffmpeg")
        .arg("-version")
        .output()
        .is_ok_and(|output| output.status.success());
    works.then(|| PathBuf::from("ffmpeg"))
}

/// Grab up to three one-second frames from a short video/GIF. Without ffmpeg
/// (or when decoding fails) the caller falls back to the embedded thumbnail.
pub(super) fn extract_video_frames(path: &str) -> Vec<String> {
    let Some(ffmpeg) = ffmpeg_binary() else {
        tracing::debug!("ffmpeg not available; using the video thumbnail");
        return Vec::new();
    };

    let safe: String = path
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .take(32)
        .collect();
    let dir = std::env::temp_dir().join(format!("wa-desk-frames-{}-{safe}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    if std::fs::create_dir_all(&dir).is_err() {
        return Vec::new();
    }

    let pattern = dir.join("frame-%02d.jpg");
    let mut command = Command::new(ffmpeg);
    command
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-t",
            "3",
            "-i",
            path,
            "-an",
            "-vf",
            "fps=1,scale=512:-2",
            "-frames:v",
            "3",
            "-q:v",
            "5",
        ])
        .arg(&pattern);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }

    if let Ok(output) = command.output()
        && !output.status.success()
    {
        tracing::debug!(
            stderr = %String::from_utf8_lossy(&output.stderr).trim(),
            "ffmpeg could not extract video frames"
        );
    }

    let mut frames = Vec::new();
    for index in 1..=3 {
        let file = dir.join(format!("frame-{index:02}.jpg"));
        if let Ok(bytes) = std::fs::read(&file)
            && !bytes.is_empty()
        {
            frames.push(format!("data:image/jpeg;base64,{}", B64.encode(bytes)));
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    frames
}
