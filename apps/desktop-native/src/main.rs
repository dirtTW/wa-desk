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

mod aec;
mod ai;
mod app;
mod app_impl;
mod audio;
mod autostart;
mod bubbles;
mod callvideo;
mod camera;
mod clipboard;
mod conversation;
mod dialogs;
mod echo;
mod emoji_glyph;
mod frames;
mod gate;
mod gif;
mod icon;
mod icons;
mod login;
mod login_ui;
mod mic;
mod model;
mod notify;
mod overlays;
mod pickers;
mod rich_text;
mod screens;
mod session_glue;
mod settings;
mod settings_ui;
mod sidebar;
mod spawn;
mod sticker;
mod theme;
mod tray;
mod updates;
mod video;
mod voicenote;
mod webp_anim;
mod widgets;

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

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
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

    // `--test-notification` proves the toast path without a message arriving.
    if std::env::args().any(|arg| arg == "--test-notification") {
        match notify::send("WA Desk", "Notifications are working.") {
            Ok(()) => {
                println!("notification sent");
                return;
            }
            Err(err) => {
                eprintln!("notification failed: {err}");
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
    let (contacts_tx, contacts_rx) = std::sync::mpsc::channel::<Vec<(String, String)>>();

    let store = model::store_path();
    let media = model::media_dir();
    tracing::info!(store = %store, media = %media, "starting session");

    let handle = runtime.handle().clone();
    // Set when the call in progress negotiated video, read by the event bridge.
    let call_video_wanted = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
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
        // Session events pass through one bridge: it feeds the AI auto-reply
        // loop and forwards the same event to the window.
        let (core_tx, mut core_rx) = tokio::sync::mpsc::unbounded_channel::<wa_types::UiEvent>();
        let ui_tx = events_out.clone();
        let ai_commands = command_tx.clone();
        let ai_dir = settings::NativeSettings::data_dir();
        let ai_slot = session_slot.clone();
        let bridge_video = call_video_wanted.clone();
        handle.spawn(async move {
            let mut ai = ai::Runtime::new(ai_dir);
            // The microphone only runs while a call is connected.
            let mut microphone: Option<mic::Mic> = None;
            // (camera, its forwarding thread's stop flag)
            #[allow(clippy::type_complexity)]
            let mut camera: Option<(
                std::sync::Arc<camera::Camera>,
                std::sync::Arc<std::sync::atomic::AtomicBool>,
            )> = None;
            while let Some(event) = core_rx.recv().await {
                match &event {
                    wa_types::UiEvent::CallState { state, .. } if state == "connected" => {
                        // Outgoing video: capture, encode, send. The camera runs
                        // only for the call that asked for video.
                        if camera.is_none()
                            && bridge_video.load(std::sync::atomic::Ordering::Relaxed)
                            && let Some(session) = ai_slot.lock().expect("session slot").clone()
                        {
                            match camera::Camera::start() {
                                Some(running) => {
                                    let running = std::sync::Arc::new(running);
                                    let sender = running.clone();
                                    let stop = std::sync::Arc::new(
                                        std::sync::atomic::AtomicBool::new(false),
                                    );
                                    let thread_stop = stop.clone();
                                    let preview_tx = ui_tx.clone();
                                    std::thread::spawn(move || {
                                        use base64::Engine as _;
                                        let mut sent_preview = 0u64;
                                        while !thread_stop
                                            .load(std::sync::atomic::Ordering::Relaxed)
                                        {
                                            if let Some(unit) = sender
                                                .recv_timeout(std::time::Duration::from_millis(120))
                                            {
                                                let _ = session.send_call_video(unit);
                                            }
                                            let generation = sender.preview().generation();
                                            if generation != sent_preview
                                                && let Some(frame) = sender.preview().frame()
                                            {
                                                sent_preview = generation;
                                                let _ = preview_tx
                                                    .send(wa_types::UiEvent::CallSelfVideo {
                                                    data: base64::engine::general_purpose::STANDARD
                                                        .encode(frame),
                                                    width: camera::PREVIEW_WIDTH as u32,
                                                    height: camera::PREVIEW_HEIGHT as u32,
                                                });
                                            }
                                        }
                                    });
                                    camera = Some((running, stop));
                                    app::diag("call: camera started");
                                }
                                None => app::diag("call: no camera"),
                            }
                        }
                        if microphone.is_none()
                            && let Some(session) = ai_slot.lock().expect("session slot").clone()
                        {
                            let (frames_tx, frames_rx) = std::sync::mpsc::channel::<Vec<i16>>();
                            match mic::Mic::start(frames_tx) {
                                Ok(mic) => {
                                    // 16 kHz mono frames straight into the call.
                                    std::thread::spawn(move || {
                                        while let Ok(frame) = frames_rx.recv() {
                                            if session.send_call_audio(frame).is_err() {
                                                break;
                                            }
                                        }
                                    });
                                    microphone = Some(mic);
                                    app::diag("call: microphone started");
                                }
                                Err(err) => app::diag(&format!("call: {err}")),
                            }
                        }
                    }
                    wa_types::UiEvent::CallEnded { .. } => {
                        let was_running = microphone.take().is_some();
                        if was_running {
                            app::diag("call: microphone stopped");
                        }
                        if let Some((_, stop)) = camera.take() {
                            stop.store(true, std::sync::atomic::Ordering::Relaxed);
                            app::diag("call: camera stopped");
                        }
                    }
                    _ => {}
                }
                ai.observe(&event, &ai_commands);
                let _ = ui_tx.send(event);
            }
        });
        handle.spawn(async move {
            match wa_core::Session::start(&store, media.into(), core_tx).await {
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
    // The command loop records whether the call wants video (the bridge reads it).
    let call_video = call_video_wanted.clone();
    // Media downloads run in their own tasks; three at a time keeps the UI
    // responsive without flooding the CDN or the phone.
    let media_limit = Arc::new(tokio::sync::Semaphore::new(16));
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
                        // The bubble is already on screen: tell the window it
                        // never left, instead of leaving a green tick on a
                        // message the server never saw.
                        let _ = events_out.send(wa_types::UiEvent::SendFailed { chat, error: err });
                    }
                }
                model::Command::MarkRead { chat } => {
                    let _ = session.mark_read(&chat).await;
                }
                model::Command::Typing { chat, typing } => {
                    let _ = session.set_typing(&chat, typing).await;
                }
                model::Command::CallStart { chat, video } => {
                    call_video.store(video, std::sync::atomic::Ordering::Relaxed);
                    if let Err(err) = session.start_call(&chat, video).await {
                        tracing::warn!(%err, "call start failed");
                    }
                }
                model::Command::CallAccept { call_id, video } => {
                    call_video.store(video, std::sync::atomic::Ordering::Relaxed);
                    if let Err(err) = session.accept_call(&call_id, video).await {
                        tracing::warn!(%err, "call accept failed");
                    }
                }
                model::Command::CallReject { call_id } => {
                    if let Err(err) = session.reject_call(&call_id).await {
                        tracing::warn!(%err, "call reject failed");
                    }
                }
                model::Command::SendSticker { chat, payload } => {
                    if let Err(err) = session.send_sticker(&chat, &payload).await {
                        tracing::warn!(%err, "sticker send failed");
                    }
                }
                model::Command::StickerPacks => {
                    for pack in session.sticker_packs() {
                        let _ = events_out.send(wa_types::UiEvent::StickerPack(pack));
                    }
                }
                model::Command::StickerPackItems { id } => {
                    match session.fetch_sticker_pack(&id).await {
                        Ok(stickers) => {
                            let view = session
                                .sticker_packs()
                                .into_iter()
                                .find(|pack| pack.id == id)
                                .map(|pack| wa_types::StickerPackView { stickers, ..pack });
                            if let Some(view) = view {
                                let _ = events_out.send(wa_types::UiEvent::StickerPack(view));
                            }
                        }
                        Err(err) => tracing::warn!(%err, "sticker pack download failed"),
                    }
                }
                model::Command::ForwardMessage { chat, id } => {
                    if let Err(err) = session.forward_message(&[chat], &id).await {
                        tracing::warn!(%err, "forward failed");
                    }
                }
                model::Command::CallHangup => {
                    if let Err(err) = session.hangup_call().await {
                        tracing::warn!(%err, "call hangup failed");
                    }
                }
                model::Command::CallMute { muted } => {
                    if let Err(err) = session.set_call_muted(muted) {
                        tracing::warn!(%err, "call mute failed");
                    }
                }
                model::Command::PairCode { phone } => {
                    match session.request_pair_code(&phone).await {
                        Ok(code) => tracing::info!(%code, "pairing code requested"),
                        Err(err) => tracing::warn!(%err, "pairing code failed"),
                    }
                }
                model::Command::FetchMedia { id } => {
                    // Downloads are independent; run a few at a time instead of
                    // serialising the whole gallery behind one command loop.
                    let session = session.clone();
                    let media_tx = media_tx.clone();
                    let limit = media_limit.clone();
                    tokio::spawn(async move {
                        let Ok(_permit) = limit.acquire_owned().await else {
                            return;
                        };
                        let started = std::time::Instant::now();
                        let reply = session.fetch_media(&id).await;
                        let took = started.elapsed().as_millis();
                        match &reply {
                            Ok(path) => app::diag(&format!("media {id} in {took} ms: {path}")),
                            Err(err) => {
                                app::diag(&format!("media {id} failed in {took} ms: {err}"))
                            }
                        }
                        let _ = media_tx.send((id, reply));
                    });
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
                    // The window needs to know which people are *saved* (the
                    // official client marks everyone else `~ name`). The store is
                    // small enough to hand over in one go.
                    for (jid, name) in contacts.iter() {
                        let _ = events_out.send(wa_types::UiEvent::ContactName {
                            jid: jid.clone(),
                            name: name.clone(),
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
                model::Command::SendPoll {
                    chat,
                    question,
                    options,
                    multiple,
                } => match session.send_poll(&chat, &question, options, multiple).await {
                    Ok(id) => app::diag(&format!("poll sent {id}")),
                    Err(err) => tracing::warn!(%err, "poll failed"),
                },
                model::Command::SetDisappearing { chat, seconds } => {
                    match session.set_group_ephemeral(&chat, seconds).await {
                        Ok(()) => {
                            let reply = session.group_info(&chat).await;
                            let _ = group_tx.send((chat, reply));
                        }
                        Err(err) => {
                            tracing::warn!(%err, "disappearing timer failed");
                        }
                    }
                }
                model::Command::SendLocation {
                    chat,
                    lat,
                    lng,
                    name,
                } => match session.send_location(&chat, lat, lng, &name).await {
                    Ok(id) => app::diag(&format!("location sent {id}")),
                    Err(err) => tracing::warn!(%err, "location failed"),
                },
                model::Command::SendContact { chat, name, phone } => {
                    match session.send_contact(&chat, &name, &phone).await {
                        Ok(id) => app::diag(&format!("contact sent {id}")),
                        Err(err) => tracing::warn!(%err, "contact failed"),
                    }
                }
                model::Command::SendVoiceNote {
                    chat,
                    payload,
                    seconds,
                    waveform,
                } => {
                    match session
                        .send_voice_note(&chat, &payload, seconds, waveform)
                        .await
                    {
                        Ok(id) => app::diag(&format!("voice note sent {id}")),
                        Err(err) => tracing::warn!(%err, "voice note failed"),
                    }
                }
                model::Command::PostStatusText {
                    text,
                    background,
                    font,
                } => match session.post_status_text(&text, background, &font).await {
                    Ok(id) => app::diag(&format!("status posted {id}")),
                    Err(err) => tracing::warn!(%err, "status post failed"),
                },
                model::Command::PostStatusMedia { path, caption } => {
                    // The window hands over a file path; the session wants the
                    // bytes, the mime and a small thumbnail.
                    let result = match std::fs::read(&path) {
                        Ok(bytes) => {
                            use base64::Engine as _;
                            let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
                            let mime = if path.to_lowercase().ends_with(".mp4")
                                || path.to_lowercase().ends_with(".mov")
                            {
                                "video/mp4"
                            } else {
                                "image/jpeg"
                            };
                            session
                                .post_status_media(&encoded, mime, &caption, "", 0)
                                .await
                        }
                        Err(err) => Err(format!("could not read the status media: {err}")),
                    };
                    match result {
                        Ok(()) => app::diag(&format!("status media posted from {path}")),
                        Err(err) => tracing::warn!(%err, "status media post failed"),
                    }
                }
                model::Command::ReplyStatus { sender, id, text } => {
                    if let Err(err) = session.reply_status(&sender, &id, &text, None).await {
                        tracing::warn!(%err, "status reply failed");
                    }
                }
                model::Command::ReplyStatusSticker {
                    sender,
                    id,
                    payload,
                } => {
                    if let Err(err) = session
                        .reply_status_sticker(&sender, &id, &payload, None)
                        .await
                    {
                        tracing::warn!(%err, "status sticker reply failed");
                    }
                }
                model::Command::ReactStatus { sender, id, emoji } => {
                    if let Err(err) = session.react_status(&sender, &id, &emoji).await {
                        tracing::warn!(%err, "status reaction failed");
                    }
                }
                model::Command::DeleteStatus { id } => {
                    if let Err(err) = session.delete_status(&id).await {
                        tracing::warn!(%err, "status delete failed");
                    }
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
                model::Command::SyncChatState => {
                    if let Err(err) = session.sync_chat_state().await {
                        app::diag(&format!("chat state sync: {err}"));
                    }
                    let mut groups = 0;
                    for jid in session.known_chats() {
                        if jid.ends_with("@g.us") {
                            groups += 1;
                            let _ = events_out.send(wa_types::UiEvent::Chat {
                                jid,
                                name: String::new(),
                                is_group: true,
                                timestamp: 0,
                            });
                        }
                    }
                    app::diag(&format!("chat state sync: {groups} groups"));
                }
                model::Command::Relink => {
                    // The client emits `LoggedOut`; the window returns to the
                    // gate and the next link receives the full history.
                    app::diag("relink requested");
                    session.logout().await;
                }
                model::Command::SyncGroups => match session.participating_groups().await {
                    Ok(groups) => {
                        app::diag(&format!("participating groups: {}", groups.len()));
                        let requests: Vec<wa_types::HistoryRequest> = groups
                            .iter()
                            .take(8)
                            .map(|(jid, _)| wa_types::HistoryRequest {
                                chat: jid.clone(),
                                oldest_id: String::new(),
                                oldest_from_me: false,
                                oldest_timestamp: 0,
                                count: 20,
                            })
                            .collect();
                        for (jid, name) in &groups {
                            let _ = events_out.send(wa_types::UiEvent::Chat {
                                jid: jid.clone(),
                                name: name.clone(),
                                is_group: true,
                                timestamp: 0,
                            });
                        }
                        if let Err(err) = session.sync_chat_history(&requests).await {
                            app::diag(&format!("group history failed: {err}"));
                        }
                    }
                    Err(err) => app::diag(&format!("participating groups: {err}")),
                },
                model::Command::ChatAction { chat, action } => {
                    let result = match action.as_str() {
                        "pin" => session.pin_chat(&chat, true).await,
                        "unpin" => session.pin_chat(&chat, false).await,
                        "mute8h" => {
                            let until = now_ms() + 8 * 60 * 60 * 1000;
                            session.mute_chat(&chat, until).await
                        }
                        "mute1w" => {
                            let until = now_ms() + 7 * 24 * 60 * 60 * 1000;
                            session.mute_chat(&chat, until).await
                        }
                        "unmute" => session.mute_chat(&chat, 0).await,
                        "archive" => session.archive_chat(&chat, true).await,
                        "unarchive" => session.archive_chat(&chat, false).await,
                        "unread" => session.mark_chat_unread(&chat, true).await,
                        "clear" => session.clear_chat(&chat).await,
                        "delete" => session.delete_chat(&chat).await,
                        other => Err(format!("unknown chat action {other}")),
                    };
                    match &result {
                        Ok(()) => app::diag(&format!("chat action {chat} {action}: ok")),
                        Err(err) => app::diag(&format!("chat action {chat} {action}: {err}")),
                    }
                }
                model::Command::Contacts => {
                    let mut contacts: Vec<(String, String)> = session
                        .contacts()
                        .into_iter()
                        .map(|(key, name)| (as_jid(&key), name))
                        .collect();
                    contacts.sort_by_key(|(_, name)| name.to_lowercase());
                    let _ = contacts_tx.send(contacts);
                }
                model::Command::Star {
                    chat,
                    id,
                    starred,
                    from_me,
                    participant,
                } => {
                    if let Err(err) = session
                        .star_message(&chat, &id, starred, from_me, participant.as_deref())
                        .await
                    {
                        app::diag(&format!("star {id}: {err}"));
                    }
                }
                model::Command::CreateGroup { subject, members } => {
                    match session.create_group(&subject, &members).await {
                        Ok(jid) => {
                            app::diag(&format!("group created {jid}"));
                            let _ = events_out.send(wa_types::UiEvent::Chat {
                                jid,
                                name: subject,
                                is_group: true,
                                timestamp: now_ms(),
                            });
                        }
                        Err(err) => app::diag(&format!("create group: {err}")),
                    }
                }
                model::Command::NewContact { phone, name } => {
                    match session.save_contact(&phone, &name).await {
                        Ok(jid) => {
                            app::diag(&format!("contact saved {jid}"));
                            let _ = events_out.send(wa_types::UiEvent::Chat {
                                jid,
                                name,
                                is_group: false,
                                timestamp: now_ms(),
                            });
                        }
                        Err(err) => app::diag(&format!("new contact: {err}")),
                    }
                }
                model::Command::GroupMember {
                    chat,
                    member,
                    action,
                } => {
                    let members = [member];
                    let result = match action.as_str() {
                        "add" => session.add_group_members(&chat, &members).await,
                        "remove" => session.remove_group_members(&chat, &members).await,
                        "promote" => session.promote_group_members(&chat, &members).await,
                        "demote" => session.demote_group_members(&chat, &members).await,
                        other => Err(format!("unknown member action {other}")),
                    };
                    match result {
                        Ok(()) => {
                            app::diag(&format!("member action {chat} {action}: ok"));
                            let reply = session.group_info(&chat).await;
                            let _ = group_tx.send((chat, reply));
                        }
                        Err(err) => app::diag(&format!("member action {action}: {err}")),
                    }
                }
                model::Command::LeaveGroup { chat } => match session.leave_group(&chat).await {
                    Ok(()) => app::diag(&format!("left group {chat}")),
                    Err(err) => app::diag(&format!("leave group: {err}")),
                },
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

    // `WA_DESK_SIZE=720x480` pins the window for screenshot tests.
    let size = std::env::var("WA_DESK_SIZE")
        .ok()
        .and_then(|value| {
            let (width, height) = value.split_once('x')?;
            Some([width.trim().parse().ok()?, height.trim().parse().ok()?])
        })
        .unwrap_or([1120.0, 780.0]);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size(size)
            .with_min_inner_size([900.0, 560.0])
            .with_title("WA Desk")
            .with_icon(icon::window_icon()),
        ..Default::default()
    };

    let app = app::NativeApp::new(
        event_rx,
        command_tx,
        handle,
        media_rx,
        group_rx,
        fav_rx,
        avatar_rx,
        attach_rx,
        contacts_rx,
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
