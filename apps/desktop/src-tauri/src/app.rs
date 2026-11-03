//! Tauri application wiring: plugins, webview permissions, tray and the
//! protocol event pump between `wa-core` and the webview.

use std::sync::atomic::Ordering;

use tauri::menu::{MenuBuilder, MenuItemBuilder};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Manager};
use wa_core::{Session, UiEvent};

use crate::ai;
use crate::state::{AppState, show_main};

/// Windows adds a DWM frame and drop shadow outside the size Tauri reports as
/// `outer_size`, so a window clamped exactly to the work area still overhangs
/// it (measured: ~16px wider, ~39px taller). Leave that much headroom.
const FRAME_INSET_X: u32 = 16;
const FRAME_INSET_Y: u32 = 48;

/// The largest window size that fits the work area.
fn max_window_size(work_area: (u32, u32)) -> (u32, u32) {
    (
        work_area.0.saturating_sub(FRAME_INSET_X),
        work_area.1.saturating_sub(FRAME_INSET_Y),
    )
}

/// The window size to use, given the configured size and the largest size that
/// fits. `None` when the configured size already fits.
fn shrink_to_fit(size: (u32, u32), max: (u32, u32)) -> Option<(u32, u32)> {
    let width = size.0.min(max.0);
    let height = size.1.min(max.1);
    (width != size.0 || height != size.1).then_some((width, height))
}

/// Shrink the window when the configured default is larger than the monitor's
/// work area. The default is 1120x780, which is taller than a 1366x768 laptop
/// once the taskbar is accounted for, so the composer ended up off-screen and
/// the window had to be resized on every launch.
fn fit_window_to_work_area(window: &tauri::WebviewWindow) {
    let monitor = window
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| window.primary_monitor().ok().flatten());
    let Some(monitor) = monitor else {
        return;
    };
    let area = monitor.work_area();
    let Ok(size) = window.outer_size() else {
        return;
    };
    let max = max_window_size((area.size.width, area.size.height));
    let fitted = shrink_to_fit((size.width, size.height), max);
    tracing::info!(
        "window {}x{} against a {}x{} work area (max {}x{}) -> {:?}",
        size.width,
        size.height,
        area.size.width,
        area.size.height,
        max.0,
        max.1,
        fitted
    );
    if let Some((width, height)) = fitted
        && window
            .set_size(tauri::PhysicalSize::new(width, height))
            .is_ok()
    {
        let _ = window.center();
    }
}

pub fn run() {
    init_tracing();
    if !claim_single_instance() {
        return;
    }
    configure_webview2();

    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            crate::commands::session::get_status,
            crate::commands::session::send_text,
            crate::commands::session::forward_message,
            crate::commands::session::mark_read,
            crate::commands::session::mark_read_messages,
            crate::commands::chat::chat_archive,
            crate::commands::chat::chat_mute,
            crate::commands::chat::chat_read_mark,
            crate::commands::chat::chat_star,
            crate::commands::chat::chat_delete,
            crate::commands::chat::chat_clear,
            crate::commands::chat::chat_block,
            crate::commands::chat::chat_pin,
            crate::commands::session::set_typing,
            crate::commands::session::request_pair_code,
            crate::commands::media::fetch_media,
            crate::commands::media::media_thumbnail,
            crate::commands::media::link_thumbnail,
            crate::commands::media::open_link,
            crate::commands::media::cached_media_path,
            crate::commands::media::document_info,
            crate::commands::media::file_thumbnail,
            crate::commands::media::download_media,
            crate::commands::media::open_media,
            crate::commands::media::reveal_media,
            crate::commands::calls::start_call,
            crate::commands::calls::accept_call,
            crate::commands::calls::reject_call,
            crate::commands::calls::hangup_call,
            crate::commands::calls::set_call_muted,
            crate::commands::calls::call_audio_frame,
            crate::commands::calls::call_video_frame,
            crate::commands::session::reconnect_session,
            crate::commands::session::restart_session,
            crate::commands::history::fetch_chat_history,
            crate::commands::history::resolve_peers,
            crate::commands::history::sync_channels,
            crate::commands::history::fetch_channel_history,
            crate::commands::history::sync_chat_history,
            crate::commands::history::save_chat_cache,
            crate::commands::history::load_chat_cache,
            crate::commands::media::fetch_favorite_stickers,
            crate::commands::media::send_favorite_sticker,
            crate::commands::media::contact_photo,
            crate::commands::media::send_attachment,
            crate::commands::media::send_sticker,
            crate::commands::media::forward_sticker,
            crate::commands::status::post_status_text,
            crate::commands::status::post_status_media,
            crate::commands::status::fetch_status_history,
            crate::commands::status::mark_status_viewed,
            crate::commands::status::react_status,
            crate::commands::status::reply_status,
            crate::commands::status::delete_status,
            crate::commands::status::watch_status_ids,
            crate::commands::session::set_push_name,
            crate::commands::session::set_status_text,
            crate::commands::contacts::get_contacts,
            crate::commands::contacts::sync_contacts,
            crate::commands::contacts::save_contact,
            crate::commands::contacts::resolve_phone,
            crate::commands::links::link_preview,
            crate::commands::messages::send_reaction,
            crate::commands::groups::create_group,
            crate::commands::groups::group_info,
            crate::commands::groups::leave_group,
            crate::commands::groups::group_add_members,
            crate::commands::groups::group_remove_members,
            crate::commands::groups::group_promote_members,
            crate::commands::groups::group_demote_members,
            crate::ai::commands::get_ai_settings,
            crate::ai::commands::save_ai_settings,
            crate::ai::commands::set_ai_enabled,
            crate::ai::commands::test_ai,
            crate::ai::commands::ai_draft_reply,
            crate::ai::commands::ai_reply_now,
            crate::ai::commands::set_ai_stickers,
            crate::ai::commands::set_ai_context,
            crate::commands::messages::delete_for_me,
            crate::commands::messages::delete_for_everyone,
            crate::commands::messages::edit_message,
            crate::commands::session::logout,
            crate::notifications::set_notify_state,
            crate::notifications::test_notify,
            crate::commands::update::check_for_update,
            crate::commands::update::install_update,
            crate::commands::ui::ui_bootstrap,
            crate::commands::ui::ui_icon,
            crate::commands::ui::ui_message_visible,
            crate::commands::ui::ui_ai_prompt,
            crate::commands::ui::ui_first_url,
            crate::commands::ui::ui_placeholder_name,
            crate::commands::ui::ui_format_phone,
            crate::commands::ui::ui_phone_from_jid,
            crate::commands::ui::ui_sidebar,
            crate::commands::ui::ui_chat,
            crate::commands::ui::ui_emoji_search,
            crate::commands::ui::ui_emoji_groups,
            crate::commands::ui::ui_emoji_recents,
            crate::commands::ui::ui_media_icon,
            crate::commands::ui::ui_avatar_palette,
            crate::commands::ui::ui_group_avatar_icon,
            crate::commands::ui::ui_avatar_style,
            crate::commands::ui::ui_initials,
            crate::commands::ui::ui_is_self_jid,
            crate::commands::ui::ui_forward_payload,
            crate::commands::ui::ui_delete_targets,
            crate::commands::ui::ui_message_action,
            crate::commands::ui::ui_copy_text,
            crate::commands::ui::ui_can_post_text,
            crate::commands::ui::ui_status_wizard_back,
            crate::commands::ui::ui_ai_save_plan,
            crate::commands::ui::ui_ai_toggle,
            crate::commands::ui::ui_ai_require_toast,
            crate::commands::ui::ui_ai_preview,
            crate::commands::ui::ui_toast_excerpt,
            crate::commands::ui::ui_profile_name_error,
            crate::commands::ui::ui_fold_plan,
            crate::commands::ui::ui_outgoing_message,
            crate::commands::ui::ui_upsert_plan,
            crate::commands::ui::ui_insert_index,
            crate::commands::ui::ui_photo_pump,
            crate::commands::ui::ui_photo_result_ok,
            crate::commands::ui::ui_emoji_active_group,
            crate::commands::ui::ui_theme_label,
            crate::commands::ui::ui_theme_resolve,
            crate::commands::ui::ui_theme_toggle,
            crate::commands::ui::ui_video_config,
            crate::commands::ui::ui_is_keyframe,
            crate::commands::ui::ui_audio_format,
            crate::commands::ui::ui_audio_schedule,
            crate::commands::ui::ui_pcm_sample,
            crate::commands::ui::ui_scroll_distance,
            crate::commands::ui::ui_scroll_constants,
            crate::commands::ui::ui_scroll_following,
            crate::commands::ui::ui_scroll_latest_visible,
            crate::commands::ui::ui_scroll_anchor,
            crate::commands::ui::ui_draft_send_plan,
            crate::commands::ui::ui_attachment_message,
            crate::commands::ui::ui_attachment_result,
            crate::commands::ui::ui_hydrate_plan,
            crate::commands::ui::ui_download_action,
            crate::commands::ui::ui_evictions,
            crate::commands::ui::ui_toggle_selection,
            crate::commands::ui::ui_status_note_view,
            crate::commands::ui::ui_status_heal,
            crate::commands::ui::ui_draft_plan,
            crate::commands::ui::ui_status_groups,
            crate::commands::ui::ui_status_own_ids,
            crate::commands::ui::ui_clamp_popover,
            crate::commands::ui::ui_menu_anchor,
            crate::commands::ui::ui_call_overlay,
            crate::commands::ui::ui_call_guard,
            crate::commands::ui::ui_call_media_toast,
            crate::commands::ui::ui_status_capture,
            crate::commands::ui::ui_status_duration_seconds,
            crate::commands::ui::ui_restore_disk_cache,
            crate::commands::ui::ui_contact_plan,
            crate::commands::ui::ui_ai_context,
            crate::commands::ui::ui_recent_stickers,
            crate::commands::ui::ui_ai_sticker_ids,
            crate::commands::ui::ui_media_limits,
            crate::commands::ui::ui_poster_seek,
            crate::commands::ui::ui_poster_height,
            crate::commands::ui::ui_send_plan,
            crate::commands::ui::ui_seed_chats,
            crate::commands::ui::ui_alias_plan,
            crate::commands::ui::ui_bulk_chat_plan,
            crate::commands::ui::ui_chat_action,
            crate::commands::ui::ui_delete_commands,
            crate::commands::ui::ui_message_info,
            crate::commands::ui::ui_incoming_classify,
            crate::commands::ui::ui_pending_echo,
            crate::commands::ui::ui_unread_rule,
            crate::commands::ui::ui_receipt_status,
            crate::commands::ui::ui_self_chat_status,
            crate::commands::ui::ui_newest_pending,
            crate::commands::ui::ui_ai_echo,
            crate::commands::ui::ui_attachment_check,
            crate::commands::ui::ui_search_matches,
            crate::commands::ui::ui_search_label,
            crate::commands::ui::ui_status_permissions,
            crate::commands::ui::ui_status_post_check,
            crate::commands::ui::ui_reply_preview,
            crate::commands::ui::ui_selection_bar,
            crate::commands::ui::ui_disk_cache_payload,
            crate::commands::ui::ui_parse_export,
            crate::commands::ui::ui_import_plan,
            crate::commands::ui::ui_history_anchor,
            crate::commands::ui::ui_history_targets,
            crate::commands::ui::ui_cache_fingerprint,
            crate::commands::ui::ui_cache_payload,
            crate::commands::ui::ui_cache_restore,
            crate::commands::ui::ui_photo_queue,
            crate::commands::ui::ui_merge_chats,
            crate::commands::ui::ui_trim_messages,
            crate::commands::ui::ui_status_palette,
            crate::commands::ui::ui_status_argb,
            crate::commands::ui::ui_status_emoji_categories,
            crate::commands::ui::ui_status_emoji_group,
            crate::commands::ui::ui_labels,
            crate::commands::ui::ui_chat_subtitle,
            crate::commands::ui::ui_settings,
            crate::commands::ui::ui_media_viewer,
            crate::commands::ui::ui_viewer_stage,
            crate::commands::ui::ui_viewer_strip_thumb,
            crate::commands::ui::ui_viewer_placeholder,
            crate::commands::ui::ui_sticker_grid,
            crate::commands::ui::ui_chat_menu,
            crate::commands::ui::ui_mute_menu,
            crate::commands::ui::ui_muted_chats,
            crate::commands::ui::ui_message_menu,
            crate::commands::ui::ui_delete_confirm,
            crate::commands::ui::ui_group_members,
            crate::commands::ui::ui_group_contacts,
            crate::commands::ui::ui_picker,
            crate::commands::ui::ui_status_frame_duration,
            crate::commands::ui::ui_status_timing,
            crate::commands::ui::ui_status_anchor,
            crate::commands::ui::ui_status_elapsed,
            crate::commands::ui::ui_status_pause_remaining,
            crate::commands::ui::ui_status_resume,
            crate::commands::ui::ui_status_progress,
            crate::commands::ui::ui_status_thumb_scale,
            crate::commands::ui::ui_status_poster_seek,
            crate::commands::ui::ui_status_list,
            crate::commands::ui::ui_status_frame,
            crate::commands::ui::ui_status_media,
            crate::commands::ui::ui_status_media_error,
            crate::commands::ui::ui_status_views,
            crate::commands::ui::ui_channels,
            crate::commands::ui::ui_community,
            crate::commands::ui::ui_starred
        ])
        .setup(|app| {
            #[cfg(windows)]
            allow_webview_microphone_and_camera(app);

            // Register the toast identity (AUMID) so notifications show as
            // "WA Desk" instead of falling back to Windows PowerShell.
            #[cfg(windows)]
            crate::notifications::register_aumid();

            // Lock the webview zoom: Ctrl+wheel would otherwise scale the
            // whole UI and break the rail/panel alignment. Closing hides to
            // the tray; Quit is on the tray menu.
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_zoom(1.0);
                // Created hidden so the work-area fit is applied before the
                // first paint — otherwise a small screen sees the default
                // window, then a jump as it is shrunk.
                fit_window_to_work_area(&window);
                let _ = window.show();
                let hide_window = window.clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = hide_window.hide();
                    }
                });
            }

            let (db_path, media_dir) = {
                let dir = app.path().app_data_dir()?;
                std::fs::create_dir_all(&dir)?;
                app.manage(ai::AiState::load(&dir));
                // Purge every store file that is not the active generation
                // before the session opens one: nothing holds them at startup,
                // so this cleans up after a logout whose wipe could not unlink
                // the old file.
                let store = crate::state::store_path(&dir, crate::state::store_generation(&dir));
                crate::state::purge_stale_stores(&dir, &store);
                (store.to_string_lossy().into_owned(), dir.join("media"))
            };
            tracing::info!(%db_path, media_dir = %media_dir.display(), "device store");

            let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<UiEvent>();

            // Keep the inputs needed to open a fresh session after logout.
            {
                let state = app.state::<AppState>();
                *state.db_path.lock().expect("db path lock") = Some(db_path.clone());
                *state.media_dir.lock().expect("media dir lock") = Some(media_dir.clone());
                *state.event_tx.lock().expect("event tx lock") = Some(tx.clone());
            }

            spawn_event_pump(app.handle().clone(), rx);
            spawn_session_start(app.handle().clone(), db_path, media_dir, tx);
            install_tray(app)?;

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(on_run_event);
}

/// A second copy cannot work: WebView2 refuses to share its user-data folder and
/// the SQLite store takes an exclusive lock, so the loser used to die with a raw
/// "failed to create webview" panic. Claim a named mutex first and say so.
///
/// The handle is deliberately leaked: Windows releases it when the process ends,
/// which is exactly the lifetime we want.
#[cfg(windows)]
fn claim_single_instance() -> bool {
    use windows::Win32::Foundation::{ERROR_ALREADY_EXISTS, GetLastError};
    use windows::Win32::System::Threading::CreateMutexW;
    use windows::Win32::UI::WindowsAndMessaging::{MB_ICONINFORMATION, MB_OK, MessageBoxW};
    use windows::core::{HSTRING, w};

    unsafe {
        let Ok(handle) = CreateMutexW(None, true, &HSTRING::from("Local\\wa-desk-single-instance"))
        else {
            return true; // cannot tell: let the app try
        };
        if GetLastError() == ERROR_ALREADY_EXISTS {
            MessageBoxW(
                None,
                w!(
                    "WA Desk is already running.\n\nOpen the existing window, or quit it from the tray icon, and try again."
                ),
                w!("WA Desk"),
                MB_OK | MB_ICONINFORMATION,
            );
            return false;
        }
        // The mutex is never closed: Windows releases it when the process ends,
        // which is exactly the lifetime we want.
        let _ = handle;
        true
    }
}

#[cfg(not(windows))]
fn claim_single_instance() -> bool {
    true
}

fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "wa_core=info,wa_desktop=info,whatsapp_rust=warn".into()),
        )
        .init();
}

/// Keep the WebView2 process tree predictable: bound the JS heap and stop
/// background download/update work the UI does not need.
fn configure_webview2() {
    #[cfg(windows)]
    {
        let existing = std::env::var("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS").unwrap_or_default();
        if !existing.contains("max-old-space-size") {
            let extra = "--js-flags=--max-old-space-size=256 \
                         --disable-background-networking --disable-component-update \
                         --disable-sync --disk-cache-size=33554432 --media-cache-size=33554432";
            unsafe {
                std::env::set_var(
                    "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS",
                    format!("{existing} {extra}").trim(),
                );
            }
        }
    }
}

/// Calls need microphone/camera access; WebView2 asks by default and the
/// default prompt is not reachable from this shell.
#[cfg(windows)]
fn allow_webview_microphone_and_camera(app: &tauri::App) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let _ = window.with_webview(|webview| {
        use webview2_com::Microsoft::Web::WebView2::Win32::*;
        use webview2_com::PermissionRequestedEventHandler;
        unsafe {
            let Ok(core) = webview.controller().CoreWebView2() else {
                return;
            };
            let mut token = 0i64;
            let _ = core.add_PermissionRequested(
                &PermissionRequestedEventHandler::create(Box::new(|_, args| {
                    let Some(args) = args else { return Ok(()) };
                    let mut kind = COREWEBVIEW2_PERMISSION_KIND::default();
                    args.PermissionKind(&mut kind)?;
                    if kind == COREWEBVIEW2_PERMISSION_KIND_MICROPHONE
                        || kind == COREWEBVIEW2_PERMISSION_KIND_CAMERA
                    {
                        args.SetState(COREWEBVIEW2_PERMISSION_STATE_ALLOW)?;
                    }
                    Ok(())
                })),
                &mut token,
            );
        }
    });
}

/// Protocol events -> webview, plus the AI handler for each event.
fn spawn_event_pump(
    handle: tauri::AppHandle,
    mut rx: tokio::sync::mpsc::UnboundedReceiver<UiEvent>,
) {
    tauri::async_runtime::spawn(async move {
        while let Some(event) = rx.recv().await {
            tracing::debug!(kind = event.kind(), "ui event");
            if let Err(err) = handle.emit("wa:event", &event) {
                tracing::warn!(%err, "could not emit ui event");
            }
            // Windows push notifications straight from the pump: they must
            // not depend on the webview being alive.
            crate::notifications::handle_event(&handle, &handle.state::<AppState>(), &event);
            // A logged-out session is terminal: the credentials on disk are dead
            // (we unlinked, or the phone removed this device), so wipe the store
            // and open a fresh session — otherwise the login screen spins on
            // "Session ended (LoggedOut)" and never shows a QR.
            if matches!(event, UiEvent::LoggedOut { .. }) {
                let logout_handle = handle.clone();
                tauri::async_runtime::spawn(async move {
                    crate::state::restart_after_logout(&logout_handle).await;
                });
            }
            let ai_handle = handle.clone();
            let ai_event = event.clone();
            tauri::async_runtime::spawn(async move {
                ai::handle_event(&ai_handle, &ai_event).await;
            });
        }
    });
}

/// Opening the SQLite store and resolving the WhatsApp web version touches the
/// network, so it must not block the setup callback.
fn spawn_session_start(
    handle: tauri::AppHandle,
    db_path: String,
    media_dir: std::path::PathBuf,
    event_tx: tokio::sync::mpsc::UnboundedSender<UiEvent>,
) {
    tauri::async_runtime::spawn(async move {
        match Session::start(&db_path, media_dir, event_tx).await {
            Ok(session) => {
                let state = handle.state::<AppState>();
                *state.session.lock().expect("session lock") = Some(session);
                let _ = handle.emit("wa:event", serde_json::json!({ "type": "ready" }));
            }
            Err(err) => {
                tracing::error!(%err, "session start failed");
                let _ = handle.emit(
                    "wa:event",
                    serde_json::json!({ "type": "fatal", "message": err.to_string() }),
                );
            }
        }
    });
}

/// Tray icon: chat apps stay reachable when the window is hidden.
fn install_tray(app: &mut tauri::App) -> tauri::Result<()> {
    let show_item = MenuItemBuilder::with_id("show", "Open WA Desk").build(app)?;
    let quit_item = MenuItemBuilder::with_id("quit", "Quit WA Desk").build(app)?;
    let tray_menu = MenuBuilder::new(app)
        .items(&[&show_item, &quit_item])
        .build()?;
    let mut tray = TrayIconBuilder::with_id("wa-tray")
        .menu(&tray_menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => show_main(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon().cloned() {
        tray = tray.icon(icon);
    }
    tray.build(app)?;
    Ok(())
}

/// Give the protocol layer a chance to flush before the process dies.
fn on_run_event(handle: &tauri::AppHandle, event: tauri::RunEvent) {
    if let tauri::RunEvent::ExitRequested { api, .. } = event {
        let state = handle.state::<AppState>();
        if state.exiting.swap(true, Ordering::SeqCst) {
            return;
        }
        let session = state.session.lock().expect("session lock").take();
        if let Some(session) = session {
            api.prevent_exit();
            let handle = handle.clone();
            tauri::async_runtime::spawn(async move {
                session.shutdown().await;
                handle.exit(0);
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{max_window_size, shrink_to_fit};

    #[test]
    fn window_is_shrunk_only_when_it_does_not_fit() {
        // 1366x768 laptop: the taskbar leaves a 1366x728 work area, and the
        // 1120x780 default is taller than the room left for the window frame —
        // that is what cut the composer off.
        let small = max_window_size((1366, 728));
        assert_eq!(small, (1350, 680));
        assert_eq!(shrink_to_fit((1120, 780), small), Some((1120, 680)));
        // 1920x1080: the configured default already fits, so nothing changes.
        assert_eq!(
            shrink_to_fit((1120, 780), max_window_size((1920, 1032))),
            None
        );
        // A window wider than the screen is narrowed as well.
        assert_eq!(shrink_to_fit((2000, 900), small), Some((1350, 680)));
    }
}
