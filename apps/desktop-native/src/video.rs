//! Video playback without a native decoder: `ffmpeg` pre-decodes a clip into a
//! frame sequence (JPEG) plus a WAV track, and the media viewer plays the frames
//! at a fixed rate while `audio.rs` plays the track. Optional: when ffmpeg is
//! missing, the viewer keeps showing the poster.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Frames per second extracted for playback.
pub const FPS: f32 = 10.0;
/// Longest clip that is decoded (the rest plays as the poster).
const MAX_SECONDS: f32 = 120.0;

/// A prepared clip.
#[derive(Debug, Clone)]
pub struct Video {
    pub dir: PathBuf,
    pub frames: usize,
    pub duration: f32,
    /// The extracted audio track, when the clip had one.
    pub audio: Option<PathBuf>,
}

impl Video {
    pub fn frame_path(&self, index: usize) -> Option<PathBuf> {
        if index >= self.frames {
            return None;
        }
        Some(self.dir.join(format!("f{:05}.jpg", index + 1)))
    }
}

/// Locate `ffmpeg`: the configured path, a copy next to the exe, or the PATH.
pub fn ffmpeg() -> Option<PathBuf> {
    if let Ok(configured) = std::env::var("WA_DESK_FFMPEG") {
        let path = PathBuf::from(configured);
        if path.is_file() {
            return Some(path);
        }
    }
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        let candidate = dir.join(if cfg!(windows) {
            "ffmpeg.exe"
        } else {
            "ffmpeg"
        });
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    Some(PathBuf::from("ffmpeg"))
}

/// Decode `source` into frames + audio under a per-message directory.
pub fn prepare(source: &str, id: &str) -> Result<Video, String> {
    prepare_into(source, id, Path::new(&crate::model::media_dir()))
}

/// `prepare`, but with an explicit base directory (tests use a temp folder).
pub fn prepare_into(source: &str, id: &str, base: &Path) -> Result<Video, String> {
    let ffmpeg = ffmpeg().ok_or_else(|| "ffmpeg is not available".to_string())?;
    let safe: String = id
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    let dir = base.join(format!("video-{safe}"));
    prune_cache(base, &dir);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(|err| err.to_string())?;

    let status = Command::new(&ffmpeg)
        .args([
            "-y",
            "-loglevel",
            "error",
            "-i",
            source,
            "-t",
            &MAX_SECONDS.to_string(),
            "-vf",
            &format!("fps={FPS},scale=640:-2"),
            "-q:v",
            "4",
        ])
        .arg(dir.join("f%05d.jpg"))
        .status()
        .map_err(|err| format!("could not run ffmpeg: {err}"))?;
    if !status.success() {
        return Err("ffmpeg could not decode the video".to_string());
    }

    let frames = std::fs::read_dir(&dir)
        .map_err(|err| err.to_string())?
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry
                .path()
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("jpg"))
        })
        .count();
    if frames == 0 {
        return Err("the video has no decodable frames".to_string());
    }

    // The audio track is optional; a silent clip simply skips it.
    let audio_path = dir.join("audio.wav");
    let audio = Command::new(&ffmpeg)
        .args([
            "-y",
            "-loglevel",
            "error",
            "-i",
            source,
            "-t",
            &MAX_SECONDS.to_string(),
            "-vn",
            "-ac",
            "2",
            "-ar",
            "48000",
            "-f",
            "wav",
        ])
        .arg(&audio_path)
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
        .then_some(audio_path);

    Ok(Video {
        dir,
        frames,
        duration: frames as f32 / FPS,
        audio,
    })
}

/// Keep at most this many prepared clips on disk (the cache is a convenience).
const MAX_CACHED: usize = 6;

/// Drop the oldest `video-*` directories so decoding clips cannot fill the disk.
fn prune_cache(base: &Path, keep: &Path) {
    let Ok(entries) = std::fs::read_dir(base) else {
        return;
    };
    let mut clips: Vec<(std::time::SystemTime, PathBuf)> = entries
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_name().to_string_lossy().starts_with("video-"))
        .filter(|entry| entry.path() != keep)
        .map(|entry| {
            let stamp = entry
                .metadata()
                .and_then(|meta| meta.modified())
                .unwrap_or(std::time::UNIX_EPOCH);
            (stamp, entry.path())
        })
        .collect();
    if clips.len() < MAX_CACHED {
        return;
    }
    clips.sort_by_key(|(stamp, _)| *stamp);
    let excess = clips.len() + 1 - MAX_CACHED;
    for (_, dir) in clips.into_iter().take(excess) {
        let _ = std::fs::remove_dir_all(dir);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_paths_are_one_based_and_bounded() {
        let video = Video {
            dir: PathBuf::from("videos"),
            frames: 3,
            duration: 0.3,
            audio: None,
        };
        assert_eq!(
            video.frame_path(0),
            Some(PathBuf::from("videos/f00001.jpg"))
        );
        assert_eq!(
            video.frame_path(2),
            Some(PathBuf::from("videos/f00003.jpg"))
        );
        assert!(video.frame_path(3).is_none());
    }

    #[test]
    fn old_clips_are_pruned_from_the_cache() {
        let base = std::env::temp_dir().join("wa-desk-video-prune");
        let _ = std::fs::remove_dir_all(&base);
        for index in 0..(MAX_CACHED + 2) {
            std::fs::create_dir_all(base.join(format!("video-old{index}"))).expect("dir");
        }
        let keep = base.join("video-keep");
        prune_cache(&base, &keep);
        let left = std::fs::read_dir(&base)
            .expect("read")
            .filter_map(|entry| entry.ok())
            .count();
        assert_eq!(left, MAX_CACHED - 1, "keeps room for the clip being made");
        let _ = std::fs::remove_dir_all(&base);
    }

    /// Is a working `ffmpeg` on this machine? The playback pipeline needs it,
    /// so the end-to-end test is skipped where it is missing (CI images).
    fn ffmpeg_works() -> bool {
        Command::new(ffmpeg().expect("a path"))
            .arg("-version")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    #[test]
    fn a_clip_becomes_frames_and_an_audio_track() {
        if !ffmpeg_works() {
            eprintln!("skipping: no ffmpeg on PATH");
            return;
        }
        let base = std::env::temp_dir().join("wa-desk-video-prepare");
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).expect("temp dir");
        let clip = base.join("clip.avi");
        let encoded = Command::new(ffmpeg().expect("a path"))
            .args([
                "-y",
                "-loglevel",
                "error",
                "-f",
                "lavfi",
                "-i",
                "testsrc=size=320x240:rate=10:duration=2",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:duration=2",
                "-c:v",
                "mpeg4",
                "-c:a",
                "pcm_s16le",
                "-shortest",
            ])
            .arg(&clip)
            .status()
            .expect("run ffmpeg");
        assert!(encoded.success(), "could not make the sample clip");

        let video = prepare_into(clip.to_str().unwrap(), "clip", &base).expect("prepare");
        assert_eq!(video.frames, 20, "10 fps for 2 seconds");
        assert!((video.duration - 2.0).abs() < 0.01);
        assert!(video.frame_path(0).expect("first frame").is_file());
        assert!(
            video.audio.as_ref().is_some_and(|path| path.is_file()),
            "the soundtrack is extracted for playback"
        );
        let _ = std::fs::remove_dir_all(&base);
    }
}
