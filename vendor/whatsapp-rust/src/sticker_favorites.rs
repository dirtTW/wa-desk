//! In-memory capture of `favoriteSticker` app-state mutations.
//!
//! WhatsApp Web stores favorite stickers in the `favorite-stickers` IndexedDB
//! table. The linked session receives the same `favoriteSticker` app-state
//! mutations; this module keeps the decoded CDN metadata so the desktop client
//! can download and show them without a browser.

use std::sync::{Mutex, OnceLock};

#[derive(Clone, Debug, Default)]
pub struct FavoriteStickerRecord {
    pub id: String,
    pub url: Option<String>,
    pub direct_path: Option<String>,
    pub media_key: Option<Vec<u8>>,
    pub file_enc_sha256: Option<Vec<u8>>,
    pub file_sha256: Option<Vec<u8>>,
    pub file_length: Option<u64>,
    pub mimetype: Option<String>,
    pub width: u32,
    pub height: u32,
    pub is_favorite: bool,
}

fn store() -> &'static Mutex<Vec<FavoriteStickerRecord>> {
    static STORE: OnceLock<Mutex<Vec<FavoriteStickerRecord>>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(Vec::new()))
}

pub fn upsert(record: FavoriteStickerRecord) {
    let mut all = store().lock().expect("favorite sticker store");
    if let Some(existing) = all.iter_mut().find(|item| item.id == record.id) {
        *existing = record;
    } else {
        all.push(record);
    }
}

pub fn remove(id: &str) {
    let mut all = store().lock().expect("favorite sticker store");
    all.retain(|item| item.id != id);
}

pub fn snapshot() -> Vec<FavoriteStickerRecord> {
    store().lock().expect("favorite sticker store").clone()
}
