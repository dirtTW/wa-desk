//! Sticker packs shared in chats.
//!
//! WhatsApp never syncs an account's pack list to linked devices, so a pack
//! becomes available here when someone shares it: `stickerPackMessage` carries
//! the pack name/publisher, the pack's zip (all stickers, with a media key at
//! pack level) and the file name of each sticker inside that zip.

use std::io::Cursor;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::wa;
use whatsapp_rust::prelude::*;

/// One pack as it is kept on disk (enough to download and unpack the zip).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredPack {
    pub id: String,
    pub name: String,
    pub publisher: String,
    /// File names inside the pack zip, in message order.
    pub file_names: Vec<String>,
    pub animated: bool,
    pub file_length: u64,
    pub media_key: Vec<u8>,
    pub file_sha256: Vec<u8>,
    pub file_enc_sha256: Vec<u8>,
    pub direct_path: String,
}

fn store_path(dir: &Path) -> PathBuf {
    dir.join("sticker-packs.json")
}

/// Every pack seen so far, newest first.
pub fn load(dir: &Path) -> Vec<StoredPack> {
    let Ok(text) = std::fs::read_to_string(store_path(dir)) else {
        return Vec::new();
    };
    let mut packs: Vec<StoredPack> = serde_json::from_str(&text).unwrap_or_default();
    packs.reverse();
    packs
}

/// Remember a pack, replacing an older copy with the same id.
pub fn save(dir: &Path, pack: StoredPack) -> Result<(), String> {
    let mut packs: Vec<StoredPack> = std::fs::read_to_string(store_path(dir))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default();
    packs.retain(|existing| existing.id != pack.id);
    packs.push(pack);
    // Bound the file: the ten most recent packs are plenty for the picker.
    if packs.len() > 10 {
        let drop = packs.len() - 10;
        packs.drain(..drop);
    }
    let text = serde_json::to_string(&packs).map_err(|err| err.to_string())?;
    std::fs::write(store_path(dir), text).map_err(|err| err.to_string())
}

/// Where one sticker of a pack lives once unpacked. File names are sanitised,
/// and index-prefixed so two packs cannot collide.
pub fn sticker_path(dir: &Path, pack: &StoredPack, file_name: &str) -> PathBuf {
    let safe: String = file_name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '.' || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .take(96)
        .collect();
    let prefix: String = pack.id.chars().take(8).collect();
    dir.join(format!("pack-{prefix}-{safe}"))
}

/// The view the window paints: cached stickers carry their path.
pub fn view(dir: &Path, pack: &StoredPack) -> wa_types::StickerPackView {
    let stickers = pack
        .file_names
        .iter()
        .map(|name| {
            let path = sticker_path(dir, pack, name);
            wa_types::StickerItemView {
                name: name.clone(),
                animated: name.to_ascii_lowercase().ends_with(".webp") && pack.animated,
                path: if path.is_file() {
                    path.to_string_lossy().into_owned()
                } else {
                    String::new()
                },
            }
        })
        .collect();
    wa_types::StickerPackView {
        id: pack.id.clone(),
        name: pack.name.clone(),
        publisher: pack.publisher.clone(),
        stickers,
    }
}

/// Unpack the downloaded zip into the media folder. Returns how many stickers
/// were written.
pub fn extract(dir: &Path, pack: &StoredPack, zip: &[u8]) -> Result<usize, String> {
    let mut archive =
        zip::ZipArchive::new(Cursor::new(zip)).map_err(|err| format!("bad pack zip: {err}"))?;
    let mut written = 0;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|err| format!("pack entry: {err}"))?;
        if !entry.is_file() {
            continue;
        }
        let name = entry
            .name()
            .map_err(|err| format!("pack entry name: {err}"))?
            .to_string();
        // The message lists the sticker names; skip the cover/icon entries.
        if !pack.file_names.iter().any(|wanted| wanted == &name) {
            continue;
        }
        let mut bytes = Vec::new();
        std::io::Read::read_to_end(&mut entry, &mut bytes).map_err(|err| err.to_string())?;
        let path = sticker_path(dir, pack, &name);
        std::fs::write(&path, bytes).map_err(|err| err.to_string())?;
        written += 1;
    }
    if written == 0 {
        return Err("the pack zip held none of the listed stickers".to_string());
    }
    Ok(written)
}

/// Read a shared pack out of a message, if that is what it carries.
pub fn from_message(message: &wa::Message) -> Option<StoredPack> {
    let pack = message
        .get_base_message()
        .sticker_pack_message
        .as_option()?;
    let id = pack.sticker_pack_id.clone().unwrap_or_default();
    if id.is_empty() {
        return None;
    }
    let file_names: Vec<String> = pack
        .stickers
        .iter()
        .filter_map(|sticker| sticker.file_name.clone())
        .collect();
    if file_names.is_empty() {
        return None;
    }
    Some(StoredPack {
        id,
        name: pack.name.clone().unwrap_or_default(),
        publisher: pack.publisher.clone().unwrap_or_default(),
        animated: pack
            .stickers
            .iter()
            .any(|sticker| sticker.is_animated.unwrap_or(false)),
        file_length: pack.file_length.unwrap_or(0),
        media_key: pack.media_key.clone().unwrap_or_default(),
        file_sha256: pack.file_sha256.clone().unwrap_or_default(),
        file_enc_sha256: pack.file_enc_sha256.clone().unwrap_or_default(),
        direct_path: pack.direct_path.clone().unwrap_or_default(),
        file_names,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_pack() -> StoredPack {
        StoredPack {
            id: "pack-123456789".to_string(),
            name: "Cats".to_string(),
            publisher: "Ada".to_string(),
            file_names: vec!["a.webp".to_string(), "b.webp".to_string()],
            animated: false,
            file_length: 100,
            media_key: vec![1, 2, 3],
            file_sha256: vec![],
            file_enc_sha256: vec![],
            direct_path: "/v/t62".to_string(),
        }
    }

    /// A zip with two sticker files and one extra icon, written into memory.
    fn sample_zip() -> Vec<u8> {
        use std::io::Write as _;
        let cursor = Cursor::new(Vec::new());
        let mut writer = zip::ZipWriter::new(cursor);
        let options = zip::write::SimpleFileOptions::default();
        for name in ["a.webp", "b.webp", "icon.png"] {
            writer.start_file(name, options).expect("start");
            writer.write_all(b"RIFF0000WEBP").expect("write");
        }
        writer.finish().expect("finish").into_inner()
    }

    #[test]
    fn the_store_round_trips_and_keeps_the_newest() {
        let dir = std::env::temp_dir().join(format!("wa-packs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dir");
        assert!(load(&dir).is_empty());
        save(&dir, sample_pack()).expect("save");
        let packs = load(&dir);
        assert_eq!(packs.len(), 1);
        assert_eq!(packs[0].name, "Cats");
        assert_eq!(packs[0].file_names, vec!["a.webp", "b.webp"]);
        // Saving the same id replaces rather than duplicates.
        let mut updated = sample_pack();
        updated.name = "Cats 2".to_string();
        save(&dir, updated).expect("save again");
        let packs = load(&dir);
        assert_eq!(packs.len(), 1);
        assert_eq!(packs[0].name, "Cats 2");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_zip_is_unpacked_into_the_media_folder() {
        let dir = std::env::temp_dir().join(format!("wa-pack-extract-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dir");
        let pack = sample_pack();
        let written = extract(&dir, &pack, &sample_zip()).expect("extract");
        assert_eq!(written, 2, "only the listed stickers are written");
        for name in ["a.webp", "b.webp"] {
            let path = sticker_path(&dir, &pack, name);
            assert!(path.is_file(), "{}", path.display());
            assert!(std::fs::read(&path).unwrap().starts_with(b"RIFF"));
        }
        // The icon is not a sticker, so it is not written.
        assert!(!sticker_path(&dir, &pack, "icon.png").exists());
        let view = view(&dir, &pack);
        assert_eq!(view.stickers.len(), 2);
        assert!(view.stickers.iter().all(|sticker| !sticker.path.is_empty()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_zip_without_the_listed_stickers_is_an_error() {
        let dir = std::env::temp_dir().join(format!("wa-pack-bad-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let mut pack = sample_pack();
        pack.file_names = vec!["missing.webp".to_string()];
        assert!(extract(&dir, &pack, &sample_zip()).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
