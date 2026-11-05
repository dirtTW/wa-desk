//! The window itself: the `eframe::App` impl (one `update` that pumps events,
//! applies the theme and picks which screen paints) and the drop hook that
//! saves the cache.

use crate::app::{WaDesk, diag, install_fonts, now_ms, prune_textures};
use crate::model::Command;
use crate::model::Model;
use crate::settings::NativeSettings;
use crate::settings_ui::settings_file;
use crate::updates::UpdateState;

impl WaDesk {
    pub(crate) fn new(_cc: &eframe::CreationContext<'_>, native: crate::app::NativeApp) -> Self {
        install_fonts(&_cc.egui_ctx);
        let (update_tx, update_rx) = std::sync::mpsc::channel();
        let system_dark = _cc.egui_ctx.style().visuals.dark_mode;
        let settings = NativeSettings::load(&settings_file());
        let palette = crate::theme::palette(&settings.theme, system_dark);
        let cache_path =
            std::path::Path::new(&crate::model::store_path()).with_file_name("chats.json");
        let (avatar_img_tx, avatar_img_rx) = std::sync::mpsc::channel();
        let (video_tx, video_rx) = std::sync::mpsc::channel();
        let mut model = Model::new();
        model.load_cache_file(&cache_path);
        // The media paths from the last run: stickers, photos and posters paint
        // from disk immediately instead of downloading again.
        let media_cache = cache_path.with_file_name("media.json");
        if let Ok(text) = std::fs::read_to_string(&media_cache) {
            model.load_media_cache(&text);
        }
        Self {
            model,
            events: native.events,
            commands: native.commands,
            runtime: native.runtime,
            typing_sent: false,
            show_status: false,
            show_calls: false,
            calls_query: String::new(),
            channels_query: String::new(),
            communities_all: false,
            show_communities: false,
            show_media: std::env::var("WA_DESK_SCREEN")
                .map(|screen| screen == "media")
                .unwrap_or(false),
            media_tab: 0,
            media_search: String::new(),
            media_search_open: false,
            status_emoji_open: false,
            status_sticker_open: false,
            posters_asked: std::collections::HashSet::new(),
            group_desc_open: false,
            group_members_all: false,
            settings_search: String::new(),
            settings_search_open: false,
            communities_active: None,
            voice: None,
            show_attach: std::env::var("WA_DESK_ATTACH").is_ok(),
            poll_draft: None,
            contact_draft: None,
            location_draft: None,
            list_draft: None,
            smoke_sent: false,
            preview_scroll_top: std::env::var("WA_DESK_SCROLL_TOP").is_ok(),
            enter_test: if std::env::var("WA_DESK_ENTER_TEST").is_ok() {
                6
            } else {
                0
            },
            status_compose: None,
            status_compose_background: 0,
            status_compose_font: 0,
            show_channels: false,
            show_settings: false,
            settings,
            settings_path: settings_file(),
            settings_note: String::new(),
            qr: None,
            gate_phone: false,
            media_rx: native.media,
            group_rx: native.group,
            show_group: false,
            textures: std::collections::HashMap::new(),
            sticker_textures: std::collections::HashMap::new(),
            favorites_rx: native.favorites,
            scrolled_hit: -1,
            note: String::new(),
            notice: None,
            update: UpdateState::Idle,
            update_tx: update_tx.clone(),
            update_rx,
            update_checked: false,
            palette,
            system_dark,
            screenshot: None,
            last_memory_log: std::time::Instant::now(),
            last_paint: std::time::Instant::now(),
            paint_gap_sum: 0.0,
            paint_gap_count: 0,
            cache_path,
            cache_dirty: false,
            cache_saved: std::time::Instant::now(),
            avatars: std::collections::HashMap::new(),
            avatar_asked: std::collections::HashSet::new(),
            avatar_disk: std::collections::HashSet::new(),
            auto_opened: false,
            avatar_rx: native.avatars,
            avatar_img_tx,
            avatar_img_rx,
            attach_rx: native.attach,
            attach_note: None,
            preview: false,
            preview_autoplay: false,
            sync_rounds: 6,
            last_sync: std::time::Instant::now(),
            focus_search: false,
            seen_epoch: 0,
            last_state_sync: std::time::Instant::now(),
            last_full_sync: std::time::Instant::now(),
            videos: std::collections::HashMap::new(),
            video_tx: video_tx.clone(),
            video_rx,
            video_textures: std::collections::HashMap::new(),
            video_order: std::collections::VecDeque::new(),
            video_prefetch: None,
            video_playing: false,
            video_started: None,
            video_offset: 0.0,
            video_muted: false,
            groups_asked: std::collections::HashSet::new(),
            show_chat_menu: std::env::var("WA_DESK_MENU").is_ok(),
            settings_section: 0,
            show_chat_actions: false,
            show_new_chat: false,
            new_chat_search: String::new(),
            contacts: Vec::new(),
            contacts_rx: native.contacts,
            audio: crate::audio::Audio::new(),
            tray: crate::tray::Tray::new(&crate::icon::window_icon()),
            call_video: None,
            call_frame: None,
            call_frame_generation: 0,
            call_self_frame: None,
            call_self_source: None,
            sticker_tab: 0,
            packs_requested: false,
            sticker_query: String::new(),
            sticker_store: false,
            emoji_group: 0,
            emoji_gif: false,
            emoji_textures: std::collections::HashMap::new(),
            show_starred: false,
            show_new_group: false,
            new_group_subject: String::new(),
            new_group_members: std::collections::HashSet::new(),
            show_new_contact: false,
            new_contact_phone: String::new(),
            new_contact_name: String::new(),
            group_add_open: false,
            group_add_members: std::collections::HashSet::new(),
        }
    }
}

impl eframe::App for WaDesk {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.smoke_send();
        // `WA_DESK_ENTER_TEST=1` puts a draft in the composer, focuses the field
        // after a few frames and pushes a real Return event through egui, so the
        // Enter path can be checked with `--screenshot` and no OS automation.
        if self.enter_test > 0 {
            self.enter_test -= 1;
            // Two frames: egui only reports `lost_focus` for a field that was
            // focused on the *previous* frame.
            if self.enter_test == 4 {
                ctx.memory_mut(|memory| {
                    memory.request_focus(egui::Id::new("composer-input"));
                });
            }
            if self.enter_test == 2 {
                ctx.input_mut(|input| {
                    input.events.push(egui::Event::Key {
                        key: egui::Key::Enter,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: egui::Modifiers::NONE,
                    });
                });
            }
        }
        // Texture caches are capped so memory does not grow with a long session.
        prune_textures(ctx, &mut self.textures, 96);
        prune_textures(ctx, &mut self.sticker_textures, 240);
        if let Some(recording) = self.voice.as_mut() {
            recording.pump();
        }
        // Frame pacing: while a clip plays, average the gap between paints and
        // report it once per 30 frames, so "smooth" is a measured number.
        let paint_now = std::time::Instant::now();
        let gap_ms = paint_now.duration_since(self.last_paint).as_secs_f32() * 1000.0;
        self.last_paint = paint_now;
        if self.video_playing && self.model.media_viewer.is_some() {
            // A resume frame carries the whole pause; ignore anything long.
            if gap_ms < 250.0 {
                self.paint_gap_sum += gap_ms;
                self.paint_gap_count += 1;
            }
            if self.paint_gap_count >= 30 {
                let average = self.paint_gap_sum / self.paint_gap_count as f32;
                diag(&format!(
                    "paint: {:.1} ms average over {} frames ({:.1} fps)",
                    average,
                    self.paint_gap_count,
                    1000.0 / average.max(0.01),
                ));
                self.paint_gap_sum = 0.0;
                self.paint_gap_count = 0;
            }
        } else {
            self.paint_gap_sum = 0.0;
            self.paint_gap_count = 0;
        }
        // `WA_DESK_DIAG=1` gets a line every ten seconds, so a long session's
        // growth (or lack of it) can be read from diagnostics.log.
        if self.last_memory_log.elapsed() >= std::time::Duration::from_secs(10) {
            self.last_memory_log = std::time::Instant::now();
            diag(&format!(
                "memory: textures {} stickers {} video-frames {} chats {} messages {}",
                self.textures.len(),
                self.sticker_textures.len(),
                self.video_textures.len(),
                self.model.chats.len(),
                self.model
                    .chats
                    .values()
                    .map(|row| row.messages.len())
                    .sum::<usize>(),
            ));
        }
        self.palette = crate::theme::palette(&self.settings.theme, self.system_dark);
        crate::theme::apply(ctx, &self.palette);
        self.model.now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        // A preview-only self-view image (screenshots without a camera).
        if let Some(path) = self.call_self_source.clone()
            && self.call_self_frame.is_none()
            && let Ok(bytes) = std::fs::read(&path)
            && let Ok(image) = image::load_from_memory(&bytes)
        {
            let image = image.to_rgba8();
            let size = [image.width() as usize, image.height() as usize];
            let color = egui::ColorImage::from_rgba_unmultiplied(size, image.as_raw());
            self.call_self_frame =
                Some(ctx.load_texture("call-self-preview", color, egui::TextureOptions::LINEAR));
        }
        self.drain_events(ctx);
        self.poll_tray(ctx);
        // A new remote call frame becomes a texture once per generation.
        if let Some(decoder) = &self.call_video {
            let generation = decoder.latest().generation();
            if generation != self.call_frame_generation
                && let Some(frame) = decoder.latest().frame()
            {
                let image = egui::ColorImage::from_rgba_unmultiplied(
                    [crate::callvideo::WIDTH, crate::callvideo::HEIGHT],
                    &frame,
                );
                self.call_frame =
                    Some(ctx.load_texture("call-frame", image, egui::TextureOptions::LINEAR));
                self.call_frame_generation = generation;
                ctx.request_repaint();
            }
        }
        self.model.now = now_ms();
        // Screenshot testing: open a seeded chat so history can be exercised
        // without clicking.
        // `WA_DESK_OPEN_CHAT=<text>` opens the first chat whose title contains it
        // (diagnosing a specific conversation without clicking).
        if let Ok(needle) = std::env::var("WA_DESK_OPEN_CHAT") {
            let needle = needle.to_lowercase();
            if let Some(jid) = self
                .model
                .visible_chats()
                .iter()
                .find(|row| self.model.title(row).to_lowercase().contains(&needle))
                .map(|row| row.jid.clone())
            {
                self.model.open_chat(&jid);
                self.send(Command::MarkRead { chat: jid.clone() });
                self.send(Command::ChatHistory { chat: jid });
                self.auto_opened = true;
                // Opened once; auto_opened keeps it from repeating.
            }
        }
        if !self.auto_opened
            && self.model.active.is_none()
            && std::env::var("WA_DESK_OPEN_FIRST").is_ok()
            && ctx.input(|input| input.time) > 6.0
        {
            self.auto_opened = true;
            if let Some(jid) = self
                .model
                .visible_chats()
                .first()
                .map(|row| row.jid.clone())
            {
                self.model.open_chat(&jid);
                self.send(Command::ChatHistory { chat: jid });
            }
        }
        if !self.model.login.is_linked() {
            self.gate(ctx);
        } else {
            // Check for a newer release once, quietly, after the gate.
            if !self.update_checked {
                self.update_checked = true;
                self.check_updates();
            }
            if let Some((text, started)) = &self.notice
                && started.elapsed() < std::time::Duration::from_secs(4)
            {
                let text = text.clone();
                let palette = self.palette.clone();
                egui::Area::new(egui::Id::new("notice"))
                    .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 72.0))
                    .interactable(false)
                    .show(ctx, |ui| {
                        egui::Frame::new()
                            .fill(palette.elevated)
                            .corner_radius(egui::CornerRadius::same(8))
                            .inner_margin(egui::Margin::symmetric(14, 7))
                            .show(ui, |ui| {
                                ui.label(egui::RichText::new(text).size(12.5).color(palette.text));
                            });
                    });
            }

            if let Some(call) = self.model.call.clone() {
                self.call_overlay(ctx, &call);
            }

            self.sidebar(ctx);
            if self.show_group {
                self.group_panel(ctx);
            } else if self.show_settings {
                self.settings_panel(ctx);
            } else if self.show_media {
                self.media_panel(ctx);
            } else if self.show_channels {
                self.channels_panel(ctx);
            } else if self.show_communities {
                self.communities_panel(ctx);
            } else if self.show_calls {
                self.calls_panel(ctx);
                self.conversation(ctx);
            } else if self.show_status {
                self.status_panel(ctx);
            } else {
                self.conversation(ctx);
                self.emoji_window(ctx);
                self.attach_window(ctx);
                self.poll_window(ctx);
                self.contact_window(ctx);
                self.location_window(ctx);
                self.list_window(ctx);
                self.stickers_window(ctx);
                self.delete_window(ctx);
                self.forward_window(ctx);
            }
            if self.model.media_viewer.is_some() {
                self.media_overlay(ctx);
            }
        }
        let _ = &self.runtime;
        self.chat_menu(ctx);
        self.chat_actions_menu(ctx);
        self.new_chat_window(ctx);
        self.starred_window(ctx);
        self.new_group_window(ctx);
        self.new_contact_window(ctx);
        self.group_add_window(ctx);
        self.screenshot_tick(ctx);
        // A restart only finds the chat list because it is mirrored here: the
        // phone does not replay history to an already-linked device.
        if !self.preview
            && self.cache_dirty
            && self.cache_saved.elapsed() >= std::time::Duration::from_secs(2)
        {
            self.cache_dirty = false;
            self.cache_saved = std::time::Instant::now();
            self.model.save_cache_file(&self.cache_path);
            let media_cache = self.cache_path.with_file_name("media.json");
            let _ = std::fs::write(&media_cache, self.model.media_cache_json());
        }
        // Nothing cached and freshly connected: seed from the address book and
        // ask the phone for a small batch of history.
        if self.model.needs_history && self.model.login.is_linked() {
            self.model.needs_history = false;
            self.send(Command::InitialSync);
            self.send(Command::SyncChatState);
            self.send(Command::SyncGroups);
            // Keep pulling batches so the recent chats surface, like the phone.
            self.sync_rounds = 12;
            self.last_sync = std::time::Instant::now();
        }
        // A fresh connection (not just the first) resumes the backfill: rows
        // without history have no timestamp and would otherwise sit at the
        // bottom of the list forever.
        if self.model.connection_epoch != self.seen_epoch {
            self.seen_epoch = self.model.connection_epoch;
            // Every connection refreshes everything the phone can give us:
            // address book + history batches, groups, statuses and chat state.
            self.send(Command::InitialSync);
            self.send(Command::SyncChatState);
            self.send(Command::SyncGroups);
            self.send(Command::StatusHistory);
            self.sync_rounds = 6;
            self.last_sync = std::time::Instant::now();
            self.last_state_sync = std::time::Instant::now();
            self.last_full_sync = std::time::Instant::now();
        }
        // Read marks, mutes and pins live in the chat app-state collection. A
        // slow poll keeps the badges in step with the phone (the official client
        // receives the same patches live, but a missed one would stick forever).
        if self.model.login.is_linked()
            && self.last_state_sync.elapsed() >= std::time::Duration::from_secs(45)
        {
            self.last_state_sync = std::time::Instant::now();
            self.send(Command::SyncChatState);
        }
        // Groups and statuses change slowly; refresh them every ten minutes so
        // the list and the rings keep up without a restart.
        if self.model.login.is_linked()
            && self.last_full_sync.elapsed() >= std::time::Duration::from_secs(600)
        {
            self.last_full_sync = std::time::Instant::now();
            self.send(Command::SyncGroups);
            self.send(Command::StatusHistory);
        }
        if self.model.login.is_linked()
            && self.sync_rounds > 0
            && self.last_sync.elapsed() >= std::time::Duration::from_secs(8)
        {
            self.sync_rounds -= 1;
            self.last_sync = std::time::Instant::now();
            self.send(Command::SyncMore);
        }
        // The session runs on other threads and pushes into a channel; without a
        // timer egui would only repaint on mouse/keyboard input, so the chat list
        // (and new messages) could sit unseen until the window was touched.
        let gif_on_screen = crate::gif::any_prepared()
            && self
                .model
                .active_row()
                .is_some_and(|row| row.messages.iter().any(|message| message.gif));
        let sticker_on_screen = crate::webp_anim::any_ready()
            && self
                .model
                .active_row()
                .is_some_and(|row| row.messages.iter().any(|message| message.kind == "sticker"));
        if self.audio.state().playing || self.video_playing || gif_on_screen || sticker_on_screen {
            // An animated sticker knows its own frame duration; otherwise the
            // 100 ms tick is enough for GIFs and audio meters.
            let per_frame = if sticker_on_screen {
                crate::webp_anim::shortest_frame_ms()
                    .map(|ms| ms.clamp(16, 100))
                    .unwrap_or(100)
            } else {
                100
            };
            ctx.request_repaint_after(std::time::Duration::from_millis(u64::from(per_frame)));
        } else {
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
        }
    }
}

impl Drop for WaDesk {
    fn drop(&mut self) {
        if !self.preview {
            self.model.save_cache_file(&self.cache_path);
        }
    }
}
