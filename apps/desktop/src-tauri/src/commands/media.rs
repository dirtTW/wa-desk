//! Media download/upload, stickers and contact photos.

use tauri::Manager;
use tauri::State;

use crate::state::{AppState, session};

#[tauri::command]
pub async fn fetch_media(state: State<'_, AppState>, id: String) -> Result<String, String> {
    session(&state)?.fetch_media(&id).await
}

#[tauri::command]
pub fn media_thumbnail(state: State<'_, AppState>, id: String) -> Result<Option<String>, String> {
    Ok(session(&state)?.message_thumbnail(&id))
}

/// Large link-preview image shipped as a CDN path on channel/broadcast link
/// posts; fetched on demand and returned as a data URI.
#[tauri::command]
pub async fn link_thumbnail(state: State<'_, AppState>, id: String) -> Result<Option<String>, String> {
    session(&state)?.link_thumbnail(&id).await
}

#[tauri::command]
pub fn cached_media_path(state: State<'_, AppState>, id: String) -> Result<Option<String>, String> {
    Ok(session(&state)?.cached_media_path(&id))
}

/// Copy a downloaded media file into the user's Downloads folder. The viewer
/// uses this instead of an `<a download>` link: WebView2 ignores `download`
/// for `asset.localhost` URLs and would navigate the whole app to the video.
///
/// `name` is the sender's own file name for documents (so a PDF lands on disk
/// as `invoice.pdf` instead of a message id); photos and videos omit it.
#[tauri::command]
pub async fn download_media(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    id: String,
    name: Option<String>,
) -> Result<String, String> {
    let source = std::path::PathBuf::from(session(&state)?.fetch_media(&id).await?);
    let extension = source
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("bin")
        .to_owned();
    let folder = app
        .path()
        .download_dir()
        .map_err(|err| format!("cannot find the Downloads folder: {err}"))?
        .join("WA Desk");
    tokio::fs::create_dir_all(&folder)
        .await
        .map_err(|err| format!("cannot create the download folder: {err}"))?;

    let file_name = saved_file_name(&id, name.as_deref(), &extension);
    let mut destination = folder.join(&file_name);
    let mut suffix = 2;
    while destination.exists() {
        destination = folder.join(with_suffix(&file_name, suffix, &extension));
        suffix += 1;
    }

    tokio::fs::copy(&source, &destination)
        .await
        .map_err(|err| format!("cannot save the media file: {err}"))?;
    Ok(destination.to_string_lossy().into_owned())
}

/// Retrofit the file card for documents cached before the UI rendered one:
/// name, MIME, size, page count and the embedded thumbnail.
#[tauri::command]
pub fn document_info(state: State<'_, AppState>, id: String) -> Option<wa_core::DocumentView> {
    session(&state).ok()?.message_document(&id)
}

/// Preview of a document already on disk, produced by the Windows shell
/// thumbnail cache. Documents from the phone usually carry an embedded JPEG,
/// but older history entries do not; this fills the gap for local files.
#[tauri::command]
pub async fn file_thumbnail(
    state: State<'_, AppState>,
    id: String,
) -> Result<Option<String>, String> {
    let Some(path) = session(&state)
        .ok()
        .and_then(|session| session.cached_media_path(&id))
    else {
        return Ok(None);
    };
    let bytes = tokio::task::spawn_blocking(move || shell_thumbnail(&path, 128))
        .await
        .map_err(|err| err.to_string())?;
    let Some(bytes) = bytes else {
        return Ok(None);
    };
    use base64::Engine as _;
    Ok(Some(format!(
        "data:image/bmp;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    )))
}

/// Windows shell thumbnail for a local file (PDF covers, Office documents,
/// videos ...), returned as a top-down 32-bit BMP.
#[cfg(windows)]
fn shell_thumbnail(path: &str, side: i32) -> Option<Vec<u8>> {
    use windows::Win32::Foundation::SIZE;
    use windows::Win32::Graphics::Gdi::{
        BI_RGB, BITMAP, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, DIB_RGB_COLORS, DeleteDC,
        DeleteObject, GetDIBits, GetObjectW, HGDIOBJ, SelectObject,
    };
    use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize};
    use windows::Win32::UI::Shell::{
        IShellItemImageFactory, SHCreateItemFromParsingName, SIIGBF_BIGGERSIZEOK,
        SIIGBF_THUMBNAILONLY,
    };
    use windows::core::HSTRING;

    unsafe {
        let init = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let initialized = init.is_ok();
        let result = (|| -> Option<Vec<u8>> {
            let factory: IShellItemImageFactory =
                match SHCreateItemFromParsingName(&HSTRING::from(path), None) {
                    Ok(factory) => factory,
                    Err(err) => {
                        tracing::debug!(%err, "shell thumbnail: no shell item");
                        return None;
                    }
                };
            let bitmap = match factory.GetImage(
                SIZE { cx: side, cy: side },
                SIIGBF_THUMBNAILONLY | SIIGBF_BIGGERSIZEOK,
            ) {
                Ok(bitmap) => bitmap,
                Err(err) => {
                    tracing::debug!(%err, "shell thumbnail: no image");
                    return None;
                }
            };

            let mut info = BITMAP::default();
            let read = GetObjectW(
                HGDIOBJ(bitmap.0),
                std::mem::size_of::<BITMAP>() as i32,
                Some(&mut info as *mut _ as *mut _),
            );
            let width = info.bmWidth;
            let height = info.bmHeight.abs();
            if read == 0 || width <= 0 || height == 0 {
                let _ = DeleteObject(HGDIOBJ(bitmap.0));
                return None;
            }

            let mut header = BITMAPINFO::default();
            header.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
            header.bmiHeader.biWidth = width;
            header.bmiHeader.biHeight = -height;
            header.bmiHeader.biPlanes = 1;
            header.bmiHeader.biBitCount = 32;
            header.bmiHeader.biCompression = BI_RGB.0;

            let mut pixels = vec![0u8; (width * height * 4) as usize];
            let dc = CreateCompatibleDC(None);
            let previous = SelectObject(dc, HGDIOBJ(bitmap.0));
            let lines = GetDIBits(
                dc,
                bitmap,
                0,
                height as u32,
                Some(pixels.as_mut_ptr() as *mut _),
                &mut header,
                DIB_RGB_COLORS,
            );
            SelectObject(dc, previous);
            let _ = DeleteDC(dc);
            let _ = DeleteObject(HGDIOBJ(bitmap.0));
            if lines == 0 {
                return None;
            }

            // The device-independent bits are BGRA: swap to RGBA and force an
            // opaque alpha channel before wrapping them in a BMP container.
            for pixel in pixels.as_chunks_mut::<4>().0 {
                pixel.swap(0, 2);
                pixel[3] = 255;
            }
            Some(bmp_bytes(width as u32, height as u32, &pixels))
        })();
        if initialized {
            CoUninitialize();
        }
        result
    }
}

#[cfg(not(windows))]
fn shell_thumbnail(_path: &str, _side: i32) -> Option<Vec<u8>> {
    None
}

/// Minimal 32-bit top-down BMP (negative height) around raw RGBA pixels.
#[cfg(windows)]
fn bmp_bytes(width: u32, height: u32, pixels: &[u8]) -> Vec<u8> {
    let image_size = (width * height * 4) as usize;
    let mut out = Vec::with_capacity(54 + image_size);
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&((54 + image_size) as u32).to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&54u32.to_le_bytes());
    out.extend_from_slice(&40u32.to_le_bytes());
    out.extend_from_slice(&(width as i32).to_le_bytes());
    out.extend_from_slice(&(-(height as i32)).to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&32u16.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&(image_size as u32).to_le_bytes());
    out.extend_from_slice(&2835i32.to_le_bytes());
    out.extend_from_slice(&2835i32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(pixels);
    out
}

/// Open a downloaded file with the OS default application (a PDF in the
/// browser/reader, an XLSX in Excel, ...).
#[tauri::command]
pub async fn open_media(state: State<'_, AppState>, id: String) -> Result<String, String> {
    let path = session(&state)?.fetch_media(&id).await?;
    open_in_default_app(&path).map_err(|err| format!("cannot open {path}: {err}"))?;
    Ok(path)
}

/// Open an http(s) link in the user's default browser. WebView2 never opens
/// `target="_blank"` anchors itself (new windows are denied), so the UI routes
/// every external link through here. Only http(s) is accepted, so a crafted
/// message cannot make this spawn arbitrary commands.
#[tauri::command]
pub fn open_link(url: String) -> Result<(), String> {
    let trimmed = url.trim();
    if !trimmed.starts_with("http://") && !trimmed.starts_with("https://") {
        return Err("only http(s) links can be opened".to_string());
    }
    open_in_default_app(trimmed).map_err(|err| format!("cannot open the link: {err}"))
}

/// Reveal the cached file in the OS file manager, so it can be dragged out or
/// opened with a different app (the file-manager affordance of a file card).
#[tauri::command]
pub async fn reveal_media(state: State<'_, AppState>, id: String) -> Result<String, String> {
    let path = session(&state)?.fetch_media(&id).await?;
    reveal_in_file_manager(&path).map_err(|err| format!("cannot show {path}: {err}"))?;
    Ok(path)
}

/// Keep the sender's name but make it safe for a Windows file system.
fn saved_file_name(id: &str, requested: Option<&str>, extension: &str) -> String {
    let safe_id: String = id
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect();
    let fallback = || format!("WA-Desk-{safe_id}.{extension}");

    let Some(requested) = requested.map(str::trim).filter(|name| !name.is_empty()) else {
        return fallback();
    };
    // Drop any directory part the sender may have included.
    let base = requested.rsplit(['/', '\\']).next().unwrap_or(requested);
    let cleaned: String = base
        .chars()
        .map(|ch| {
            if ch.is_control() || matches!(ch, ':' | '*' | '?' | '"' | '<' | '>' | '|') {
                '_'
            } else {
                ch
            }
        })
        .collect();
    let mut name = cleaned.trim().trim_end_matches('.').to_owned();
    if name.chars().count() > 120 {
        // Keep the extension while bounding the name.
        let tail: String = name
            .chars()
            .rev()
            .take(16)
            .collect::<String>()
            .chars()
            .rev()
            .collect();
        let stem: String = name.chars().take(100).collect();
        name = if tail.contains('.') {
            format!("{stem}{}", &tail[tail.find('.').unwrap_or(0)..])
        } else {
            stem
        };
    }
    if name.is_empty() {
        return fallback();
    }
    if std::path::Path::new(&name).extension().is_none() {
        name.push('.');
        name.push_str(extension);
    }
    name
}

/// `report.pdf` + 2 -> `report-2.pdf`.
fn with_suffix(file_name: &str, suffix: u32, extension: &str) -> String {
    let actual = std::path::Path::new(file_name)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or(extension);
    let suffix_with_dot = format!(".{actual}");
    let stem = file_name
        .strip_suffix(&suffix_with_dot)
        .unwrap_or(file_name);
    format!("{stem}-{suffix}.{actual}")
}

#[cfg(windows)]
fn open_in_default_app(path: &str) -> std::io::Result<()> {
    use std::os::windows::process::CommandExt;
    // `start` treats the first quoted argument as the window title, so pass an
    // empty one; CREATE_NO_WINDOW keeps a console from flashing.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    std::process::Command::new("cmd")
        .args(["/C", "start", "", path])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()?;
    Ok(())
}

#[cfg(not(windows))]
fn open_in_default_app(path: &str) -> std::io::Result<()> {
    std::process::Command::new("xdg-open").arg(path).spawn()?;
    Ok(())
}

#[cfg(windows)]
fn reveal_in_file_manager(path: &str) -> std::io::Result<()> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    std::process::Command::new("explorer")
        .arg(format!("/select,\"{path}\""))
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()?;
    Ok(())
}

#[cfg(not(windows))]
fn reveal_in_file_manager(path: &str) -> std::io::Result<()> {
    let folder = std::path::Path::new(path)
        .parent()
        .map(|parent| parent.to_path_buf())
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    std::process::Command::new("xdg-open").arg(folder).spawn()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_the_senders_document_name() {
        assert_eq!(
            saved_file_name("ABC123", Some("quarterly report.pdf"), "pdf"),
            "quarterly report.pdf"
        );
        // Directory separators and characters Windows rejects are replaced.
        assert_eq!(
            saved_file_name("ABC123", Some("../../etc/passwd"), "pdf"),
            "passwd.pdf"
        );
        assert_eq!(
            saved_file_name("ABC123", Some("a:b*c?.xlsx"), "xlsx"),
            "a_b_c_.xlsx"
        );
        // No extension in the sender's name: borrow the downloaded one.
        assert_eq!(
            saved_file_name("ABC123", Some("raw material"), "pdf"),
            "raw material.pdf"
        );
        // Photos and videos keep the message-id name.
        assert_eq!(
            saved_file_name("3EB0/ABC", None, "jpg"),
            "WA-Desk-3EB0_ABC.jpg"
        );
        assert_eq!(
            saved_file_name("ABC", Some("   "), "pdf"),
            "WA-Desk-ABC.pdf"
        );
    }

    #[test]
    fn deduplicates_repeated_file_names() {
        assert_eq!(with_suffix("report.pdf", 2, "pdf"), "report-2.pdf");
        assert_eq!(with_suffix("archive.tar.gz", 3, "gz"), "archive.tar-3.gz");
        assert_eq!(with_suffix("no-extension", 2, "bin"), "no-extension-2.bin");
    }

    #[test]
    fn bounds_very_long_names() {
        let long = format!("{}.pdf", "a".repeat(200));
        let saved = saved_file_name("ABC", Some(&long), "pdf");
        assert!(saved.chars().count() <= 120, "name is bounded: {saved}");
        assert!(saved.ends_with(".pdf"), "extension survives: {saved}");
    }
}

#[tauri::command]
pub async fn contact_photo(
    state: State<'_, AppState>,
    chat: String,
) -> Result<Option<String>, String> {
    session(&state)?.contact_photo(&chat).await
}

#[tauri::command]
pub async fn send_attachment(
    state: State<'_, AppState>,
    chat: String,
    name: String,
    mime: String,
    caption: Option<String>,
    data: String,
) -> Result<wa_core::AttachmentResult, String> {
    session(&state)?
        .send_attachment(&chat, &name, &mime, caption.as_deref(), &data)
        .await
}

#[tauri::command]
pub async fn send_sticker(
    state: State<'_, AppState>,
    chat: String,
    data: String,
) -> Result<wa_core::AttachmentResult, String> {
    session(&state)?.send_sticker(&chat, &data).await
}

#[tauri::command]
pub async fn forward_sticker(
    state: State<'_, AppState>,
    chat: String,
    id: String,
) -> Result<(), String> {
    session(&state)?.forward_sticker(&chat, &id).await
}

#[tauri::command]
pub async fn fetch_favorite_stickers(
    state: State<'_, AppState>,
) -> Result<Vec<wa_core::FavoriteStickerView>, String> {
    session(&state)?.fetch_favorite_stickers().await
}

#[tauri::command]
pub async fn send_favorite_sticker(
    state: State<'_, AppState>,
    chat: String,
    id: String,
) -> Result<wa_core::AttachmentResult, String> {
    session(&state)?.send_favorite_sticker(&chat, &id).await
}
