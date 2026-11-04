//! WA Desk, rendered natively in Rust.
//!
//! This binary is the "no WebView" front end: it owns the window, paints with
//! `egui` and talks to the protocol layer directly. Formatting, labels, previews,
//! ticks and every other decision still come from `wa-ui`, the same crate the
//! WebView version uses, so the two front ends cannot drift apart.
//!
//! `--selftest` builds the model headlessly, applies a scripted set of protocol
//! events and prints what the window would show, then exits non-zero if any
//! expectation fails. That is how the UI is verified in CI without a display.

// Release builds are a GUI app: Windows must not open a console window. Debug
// builds keep the console so `--selftest` and the update checks stay readable.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod clipboard;
mod icons;
mod login;
mod model;
mod settings;
mod theme;
mod updates;

use std::sync::Arc;

/// A JID for a contact key: the address book uses bare phone numbers.
fn as_jid(key: &str) -> String {
    if key.contains('@') {
        key.to_string()
    } else {
        format!("{key}@s.whatsapp.net")
    }
}

/// A conservative MIME type from the file extension.
fn mime_for(path: &str) -> &'static str {
    let extension = std::path::Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match extension.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "mp4" => "video/mp4",
        "mov" => "video/quicktime",
        "mp3" => "audio/mpeg",
        "ogg" | "opus" => "audio/ogg",
        "pdf" => "application/pdf",
        "zip" => "application/zip",
        "txt" => "text/plain",
        "csv" => "text/csv",
        "json" => "application/json",
        "doc" => "application/msword",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xls" => "application/vnd.ms-excel",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "ppt" => "application/vnd.ms-powerpoint",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        _ => "application/octet-stream",
    }
}

/// Request the next batch of chats from the seeded address book.
async fn sync_batch(
    session: &wa_core::Session,
    contacts: &std::sync::Arc<std::sync::Mutex<Vec<(String, String)>>>,
    cursor: &std::sync::Arc<std::sync::Mutex<usize>>,
    step: usize,
) {
    let snapshot = contacts.lock().expect("sync contacts").clone();
    if snapshot.is_empty() {
        return;
    }
    let start = {
        let mut cursor = cursor.lock().expect("sync cursor");
        let start = *cursor;
        *cursor = (*cursor + step).min(snapshot.len());
        start
    };
    let requests: Vec<wa_types::HistoryRequest> = snapshot
        .iter()
        .skip(start)
        .take(8)
        .map(|(jid, _)| wa_types::HistoryRequest {
            chat: jid.clone(),
            oldest_id: String::new(),
            oldest_from_me: false,
            oldest_timestamp: 0,
            count: 20,
        })
        .collect();
    if requests.is_empty() {
        return;
    }
    match session.sync_chat_history(&requests).await {
        Ok(sent) => app::diag(&format!("history batch from {start}: sent {sent}")),
        Err(err) => app::diag(&format!("history batch from {start}: {err}")),
    }
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "wa_core=info,wa_desktop_native=info".into()),
        )
        .init();

    if std::env::args().any(|arg| arg == "--selftest") {
        match model::selftest() {
            Ok(report) => {
                println!("{report}");
                return;
            }
            Err(err) => {
                eprintln!("selftest failed: {err}");
                std::process::exit(1);
            }
        }
    }

    // The updater is verified from the command line too: `--check-updates`
    // prints the newest release, `--download-update` proves the asset downloads.
    // `WA_DESK_UPDATE_FROM` overrides the installed version for testing.
    if std::env::args().any(|arg| arg == "--check-updates")
        || std::env::args().any(|arg| arg == "--download-update")
    {
        let current = std::env::var("WA_DESK_UPDATE_FROM")
            .unwrap_or_else(|_| env!("CARGO_PKG_VERSION").to_string());
        match updates::check(&current) {
            Ok(Some(release)) => {
                println!(
                    "{} ({} bytes) {} — {:?}",
                    release.tag, release.size, release.url, release.notes_url
                );
                if std::env::args().any(|arg| arg == "--download-update") {
                    match updates::download(&release, &updates::staging_path()) {
                        Ok(path) => println!(
                            "downloaded {} bytes to {}",
                            std::fs::metadata(&path).map(|meta| meta.len()).unwrap_or(0),
                            path.display()
                        ),
                        Err(err) => {
                            eprintln!("download failed: {err}");
                            std::process::exit(1);
                        }
                    }
                }
            }
            Ok(None) => println!("up to date ({current})"),
            Err(err) => {
                eprintln!("check failed: {err}");
                std::process::exit(1);
            }
        }
        return;
    }

    // Right after an update the old process is still releasing the database
    // and the single-instance mutex; give it a moment before opening the store.
    if std::env::args().any(|arg| arg == "--restarted") {
        std::thread::sleep(std::time::Duration::from_millis(1500));
    }

    // `--preview` renders the UI with sample data and no session (used for
    // screenshots); `--screenshot <path>` saves one settled frame and quits.
    let preview = std::env::args().any(|arg| arg == "--preview");
    let screenshot = std::env::args()
        .collect::<Vec<String>>()
        .windows(2)
        .find(|pair| pair[0] == "--screenshot")
        .map(|pair| std::path::PathBuf::from(&pair[1]));

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");

    let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel::<wa_types::UiEvent>();
    let (command_tx, command_rx) = std::sync::mpsc::channel::<model::Command>();
    let (media_tx, media_rx) = std::sync::mpsc::channel::<(String, Result<String, String>)>();
    let (group_tx, group_rx) =
        std::sync::mpsc::channel::<(String, Result<wa_types::GroupInfoView, String>)>();
    let (fav_tx, fav_rx) =
        std::sync::mpsc::channel::<Result<Vec<wa_types::FavoriteStickerView>, String>>();
    let (avatar_tx, avatar_rx) = std::sync::mpsc::channel::<(String, Option<String>)>();
    let (attach_tx, attach_rx) = std::sync::mpsc::channel::<Result<String, String>>();

    let store = model::store_path();
    let media = model::media_dir();
    tracing::info!(store = %store, media = %media, "starting session");

    let handle = runtime.handle().clone();
    let session_slot: Arc<std::sync::Mutex<Option<Arc<wa_core::Session>>>> =
        Arc::new(std::sync::Mutex::new(None));
    // A clone kept for synthetic events (seeding the chat list from the address
    // book) sent from the command loop.
    let events_out = event_tx.clone();
    if preview {
        drop(event_tx);
    } else {
        let slot = session_slot.clone();
        let startup_events = events_out.clone();
        handle.spawn(async move {
            match wa_core::Session::start(&store, media.into(), event_tx).await {
                Ok(session) => {
                    // Tell the UI who we are, so the sidebar avatar and the
                    // self-chat label have a JID to work with.
                    let jid = session
                        .own_jid()
                        .or_else(|| session.own_lid())
                        .unwrap_or_default();
                    let name = session.push_name();
                    app::diag(&format!("identity {jid} {name}"));
                    let _ = startup_events.send(wa_types::UiEvent::SelfIdentity { jid, name });
                    *slot.lock().expect("session slot") = Some(session);
                }
                Err(err) => tracing::error!(%err, "session start failed"),
            }
        });
    }

    // The appointment list, seeded once and walked in batches.
    let sync_contacts: Arc<std::sync::Mutex<Vec<(String, String)>>> =
        Arc::new(std::sync::Mutex::new(Vec::new()));
    let sync_cursor: Arc<std::sync::Mutex<usize>> = Arc::new(std::sync::Mutex::new(0));
    let command_contacts = sync_contacts.clone();
    let command_cursor = sync_cursor.clone();
    // Commands from the UI thread are executed on the runtime.
    let slot = session_slot.clone();
    handle.spawn(async move {
        while let Ok(command) = command_rx.recv() {
            let Some(session) = slot.lock().expect("session slot").clone() else {
                continue;
            };
            match command {
                model::Command::SendText { chat, text } => {
                    if let Err(err) = session.send_text(&chat, &text).await {
                        tracing::warn!(%err, "send failed");
                    }
                }
                model::Command::MarkRead { chat } => {
                    let _ = session.mark_read(&chat).await;
                }
                model::Command::Typing { chat, typing } => {
                    let _ = session.set_typing(&chat, typing).await;
                }
                model::Command::PairCode { phone } => {
                    match session.request_pair_code(&phone).await {
                        Ok(code) => tracing::info!(%code, "pairing code requested"),
                        Err(err) => tracing::warn!(%err, "pairing code failed"),
                    }
                }
                model::Command::FetchMedia { id } => {
                    let reply = session.fetch_media(&id).await;
                    let _ = media_tx.send((id, reply));
                }
                model::Command::GroupInfo { chat } => {
                    let reply = session.group_info(&chat).await;
                    let _ = group_tx.send((chat, reply));
                }
                model::Command::FavoriteStickers => {
                    let reply = session.fetch_favorite_stickers().await;
                    let _ = fav_tx.send(reply);
                }
                model::Command::SendFavoriteSticker { chat, id } => {
                    if let Err(err) = session.send_favorite_sticker(&chat, &id).await {
                        tracing::warn!(%err, "favourite sticker send failed");
                    }
                }
                model::Command::ForwardSticker { chat, message_id } => {
                    if let Err(err) = session.forward_sticker(&chat, &message_id).await {
                        tracing::warn!(%err, "sticker forward failed");
                    }
                }
                model::Command::SendReply {
                    chat,
                    text,
                    reply_id,
                    quoted,
                } => {
                    if let Err(err) = session
                        .send_text_reply(&chat, &text, &reply_id, quoted.as_deref())
                        .await
                    {
                        tracing::warn!(%err, "reply failed");
                    }
                }
                model::Command::React {
                    chat,
                    id,
                    from_me,
                    participant,
                    emoji,
                } => {
                    if let Err(err) = session
                        .send_reaction(&chat, &id, from_me, participant.as_deref(), &emoji)
                        .await
                    {
                        tracing::warn!(%err, "reaction failed");
                    }
                }
                model::Command::EditMessage { chat, id, text } => {
                    if let Err(err) = session.edit_message(&chat, &id, &text).await {
                        tracing::warn!(%err, "edit failed");
                    }
                }
                model::Command::DeleteForMe {
                    chat,
                    id,
                    from_me,
                    participant,
                } => {
                    if let Err(err) = session
                        .delete_for_me(&chat, &id, from_me, participant.as_deref())
                        .await
                    {
                        tracing::warn!(%err, "delete for me failed");
                    }
                }
                model::Command::DeleteForEveryone { chat, id } => {
                    if let Err(err) = session.delete_for_everyone(&chat, &id).await {
                        tracing::warn!(%err, "delete for everyone failed");
                    }
                }
                model::Command::Forward { chats, id } => {
                    if let Err(err) = session.forward_message(&chats, &id).await {
                        tracing::warn!(%err, "forward failed");
                    }
                }
                model::Command::InitialSync => {
                    // The address book is keyed by bare phone numbers; WhatsApp
                    // needs `<number>@s.whatsapp.net`.
                    let mut contacts: Vec<(String, String)> = session
                        .contacts()
                        .into_iter()
                        .map(|(key, name)| (as_jid(&key), name))
                        .collect();
                    contacts.sort_by(|a, b| a.1.cmp(&b.1));
                    app::diag(&format!("initial sync: {} contacts", contacts.len()));
                    for (jid, name) in contacts.iter().take(80) {
                        let _ = events_out.send(wa_types::UiEvent::Chat {
                            jid: jid.clone(),
                            name: name.clone(),
                            is_group: jid.ends_with("@g.us"),
                            timestamp: 0,
                        });
                    }
                    *command_contacts.lock().expect("sync contacts") = contacts;
                    *command_cursor.lock().expect("sync cursor") = 0;
                    sync_batch(&session, &command_contacts, &command_cursor, 0).await;
                }
                model::Command::SyncMore => {
                    sync_batch(&session, &command_contacts, &command_cursor, 8).await;
                }
                model::Command::Avatar { jid } => {
                    let url = match session.contact_photo(&jid).await {
                        Ok(url) => url,
                        Err(err) => {
                            app::diag(&format!("avatar {jid} error: {err}"));
                            None
                        }
                    };
                    app::diag(&format!(
                        "avatar {jid}: {}",
                        if url.is_some() { "url" } else { "none" }
                    ));
                    let _ = avatar_tx.send((jid, url));
                }
                model::Command::StatusHistory => {
                    if let Err(err) = session.fetch_status_history().await {
                        tracing::warn!(%err, "status history failed");
                    }
                }
                model::Command::ChatHistory { chat } => {
                    let request = wa_types::HistoryRequest {
                        chat: chat.clone(),
                        oldest_id: String::new(),
                        oldest_from_me: false,
                        oldest_timestamp: 0,
                        count: 30,
                    };
                    match session.sync_chat_history(&[request]).await {
                        Ok(sent) => app::diag(&format!("chat history {chat}: sent {sent}")),
                        Err(err) => app::diag(&format!("chat history {chat}: {err}")),
                    }
                }
                model::Command::SendDocument {
                    chat,
                    path,
                    caption,
                } => {
                    use base64::Engine as _;
                    let result = match std::fs::metadata(&path) {
                        Ok(meta) if meta.len() > 48 * 1024 * 1024 => {
                            Err("that file is larger than the 48 MB limit".to_string())
                        }
                        Ok(_) => match std::fs::read(&path) {
                            Ok(bytes) => {
                                let name = std::path::Path::new(&path)
                                    .file_name()
                                    .map(|name| name.to_string_lossy().into_owned())
                                    .unwrap_or_else(|| "file".to_string());
                                let mime = mime_for(&path);
                                let payload =
                                    base64::engine::general_purpose::STANDARD.encode(&bytes);
                                session
                                    .send_attachment(
                                        &chat,
                                        &name,
                                        mime,
                                        caption.as_deref(),
                                        &payload,
                                    )
                                    .await
                                    .map(|_| format!("sent {name}"))
                                    .map_err(|err| err.to_string())
                            }
                            Err(err) => Err(format!("could not read the file: {err}")),
                        },
                        Err(err) => Err(format!("could not read the file: {err}")),
                    };
                    app::diag(&format!(
                        "document {path}: {}",
                        match &result {
                            Ok(note) => note.clone(),
                            Err(err) => err.clone(),
                        }
                    ));
                    let _ = attach_tx.send(match result {
                        Ok(note) => Ok(note),
                        Err(err) => Err(err),
                    });
                }
                model::Command::Restart => {
                    // The session reconnects itself; a reconnect is what asks
                    // WhatsApp for a fresh QR.
                    if let Err(err) = session.reconnect_safely().await {
                        tracing::warn!(%err, "reconnect failed");
                    }
                }
            }
        }
    });

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1120.0, 780.0])
            .with_min_inner_size([720.0, 480.0])
            .with_title("WA Desk"),
        ..Default::default()
    };

    let app = app::NativeApp::new(
        event_rx, command_tx, handle, media_rx, group_rx, fav_rx, avatar_rx, attach_rx,
    );
    let screenshot_path = screenshot.clone();
    if let Err(err) = eframe::run_native(
        "WA Desk",
        options,
        Box::new(move |cc| {
            let mut window = app::WaDesk::new(cc, app);
            if preview {
                window.preview_mode();
            }
            if let Some(path) = screenshot_path.clone() {
                window.set_screenshot(path);
            }
            Ok(Box::new(window))
        }),
    ) {
        eprintln!("could not open the window: {err}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contact_keys_become_jids() {
        assert_eq!(as_jid("628123"), "628123@s.whatsapp.net");
        assert_eq!(as_jid("1203@g.us"), "1203@g.us");
    }

    #[test]
    fn mime_types_come_from_the_extension() {
        assert_eq!(mime_for("a/b/report.pdf"), "application/pdf");
        assert_eq!(mime_for("photo.JPG"), "image/jpeg");
        assert_eq!(mime_for("archive.unknown"), "application/octet-stream");
    }
}
