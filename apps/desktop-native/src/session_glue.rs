//! The plumbing between the session thread and the window: the `UiEvent` pump,
//! the unfocused-window toast, the tray poll and the screenshot/smoke hooks.

use crate::app::{VideoState, WaDesk, circular_avatar, diag, notification_body};
use crate::model::Command;
use crate::updates::{UpdateMsg, UpdateState};
use wa_types::UiEvent;

impl WaDesk {
    pub(crate) fn drain_events(&mut self, ctx: &egui::Context) {
        let mut changed = false;
        while let Ok(event) = self.events.try_recv() {
            crate::app::diag(&format!("event {}", event.kind()));
            let incoming = match &event {
                UiEvent::Message(message) if !message.from_me => Some((
                    message.chat.clone(),
                    message.chat_name.clone(),
                    message.kind.clone(),
                    message.text.clone(),
                )),
                UiEvent::CallAudio { samples, .. } => {
                    self.audio.play_call_audio(samples.clone());
                    None
                }
                UiEvent::CallSelfVideo {
                    data,
                    width,
                    height,
                } => {
                    use base64::Engine as _;
                    if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(data)
                        && bytes.len() == (*width as usize) * (*height as usize) * 4
                    {
                        let image = egui::ColorImage::from_rgba_unmultiplied(
                            [*width as usize, *height as usize],
                            &bytes,
                        );
                        self.call_self_frame = Some(ctx.load_texture(
                            "call-self-frame",
                            image,
                            egui::TextureOptions::LINEAR,
                        ));
                    }
                    None
                }
                UiEvent::CallVideo { data, .. } => {
                    // Annex-B access units from the peer: decode with ffmpeg and
                    // paint the newest frame in the call stage.
                    use base64::Engine as _;
                    if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(data) {
                        if self.call_video.is_none() {
                            self.call_video = crate::callvideo::CallVideo::start();
                        }
                        if let Some(decoder) = &self.call_video {
                            decoder.push(bytes);
                        }
                    }
                    None
                }
                UiEvent::CallEnded { .. } => {
                    self.audio.stop_call();
                    self.call_video = None;
                    self.call_frame = None;
                    self.call_frame_generation = 0;
                    None
                }
                _ => None,
            };
            self.model.apply(&event);
            changed = true;
            if let Some((chat, name, kind, text)) = incoming {
                // A message that lands in the open chat is read straight away,
                // the way the official client behaves.
                if self.model.active.as_deref() == Some(chat.as_str()) {
                    self.send(Command::MarkRead { chat: chat.clone() });
                }
                self.notify_message(ctx, &chat, &name, &kind, &text);
            }
        }
        while let Ok((chat, reply)) = self.group_rx.try_recv() {
            match reply {
                Ok(info) => self.model.group_loaded(info),
                Err(err) => self.model.group_failed(&err),
            }
            let _ = chat;
            changed = true;
        }
        while let Ok(reply) = self.favorites_rx.try_recv() {
            match reply {
                Ok(favorites) => self.model.favorites_loaded(favorites),
                Err(err) => self.model.favorites_failed(&err),
            }
            changed = true;
        }
        while let Ok(message) = self.update_rx.try_recv() {
            self.update = match message {
                UpdateMsg::Checked(Ok(Some(release))) => UpdateState::Available(release),
                UpdateMsg::Checked(Ok(None)) => UpdateState::UpToDate,
                UpdateMsg::Checked(Err(err)) => UpdateState::Failed(err),
                UpdateMsg::Downloaded(Ok(path)) => UpdateState::Ready(path),
                UpdateMsg::Downloaded(Err(err)) => UpdateState::Failed(err),
            };
            changed = true;
        }
        while let Ok((jid, url)) = self.avatar_rx.try_recv() {
            match url {
                Some(url) => {
                    let tx = self.avatar_img_tx.clone();
                    std::thread::spawn(move || {
                        let result = crate::updates::fetch_bytes(&url);
                        crate::app::diag(&format!(
                            "avatar download {url}: {}",
                            match &result {
                                Ok(bytes) => format!("{} bytes", bytes.len()),
                                Err(err) => err.clone(),
                            }
                        ));
                        let _ = tx.send((jid, result));
                    });
                }
                None => {
                    self.avatars.insert(jid, None);
                }
            }
            changed = true;
        }
        while let Ok((jid, bytes)) = self.avatar_img_rx.try_recv() {
            if let Ok(bytes) = &bytes {
                // Cache the photo so the next launch paints it instantly.
                let _ = std::fs::write(crate::model::avatar_path(&jid), bytes);
            }
            let texture = bytes
                .ok()
                .and_then(|bytes| circular_avatar(ctx, &format!("avatar-{jid}"), &bytes));
            crate::app::diag(&format!(
                "avatar texture {jid}: {}",
                if texture.is_some() { "ok" } else { "failed" }
            ));
            self.avatars.insert(jid, texture);
            changed = true;
        }
        while let Ok(list) = self.contacts_rx.try_recv() {
            self.contacts = list;
            changed = true;
        }
        while let Ok(result) = self.attach_rx.try_recv() {
            let note = match result {
                Ok(note) => note,
                Err(err) => err,
            };
            self.attach_note = Some((note, std::time::Instant::now()));
            changed = true;
        }
        while let Ok((id, reply)) = self.video_rx.try_recv() {
            diag(&format!(
                "video {id}: {}",
                match &reply {
                    Ok(video) => format!("{} frames", video.frames),
                    Err(err) => err.clone(),
                }
            ));
            self.videos.insert(
                id,
                match reply {
                    Ok(video) => VideoState::Ready(video),
                    Err(err) => VideoState::Failed(err),
                },
            );
            changed = true;
        }
        while let Ok((id, reply)) = self.media_rx.try_recv() {
            match reply {
                Ok(path) => self.model.media_ready(&id, &path),
                Err(err) => {
                    diag(&format!("media {id} failed: {err}"));
                    self.model.media_failed_with(&id, &err);
                }
            }
            changed = true;
        }
        if changed {
            self.cache_dirty = true;
            ctx.request_repaint();
        }
    }

    pub(crate) fn screenshot_tick(&mut self, ctx: &egui::Context) {
        let Some((path, started, requested)) = self.screenshot.as_mut() else {
            return;
        };
        if !*requested {
            let delay: u64 = std::env::var("WA_DESK_SCREENSHOT_DELAY")
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(14);
            if started.elapsed() >= std::time::Duration::from_secs(delay) {
                *requested = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
            }
        }
        let events = ctx.input(|input| input.events.clone());
        for event in events {
            if let egui::Event::Screenshot { image, .. } = event {
                let size = [image.width() as u32, image.height() as u32];
                let rgba: Vec<u8> = image
                    .pixels
                    .iter()
                    .flat_map(|pixel| pixel.to_array())
                    .collect();
                if let Some(buffer) = image::RgbaImage::from_raw(size[0], size[1], rgba) {
                    let _ = buffer.save(&*path);
                }
                std::process::exit(0);
            }
        }
        ctx.request_repaint();
    }

    /// A toast for a message that arrived while the window is in the
    /// background, like the official client. History replays never get here
    /// (`HistoryMessage` is a different event), and the open chat is skipped.
    pub(crate) fn notify_message(
        &mut self,
        ctx: &egui::Context,
        chat: &str,
        name: &str,
        kind: &str,
        text: &str,
    ) {
        if !self.settings.notifications
            || chat == "status@broadcast"
            || chat.ends_with("@newsletter")
            || self.model.active.as_deref() == Some(chat)
            || ctx.input(|input| input.viewport().focused.unwrap_or(true))
        {
            return;
        }
        let body = notification_body(kind, text);
        let title = if name.trim().is_empty() {
            "WA Desk".to_string()
        } else {
            name.to_string()
        };
        if let Err(err) = crate::notify::send(&title, &body) {
            diag(&format!("notification: {err}"));
        }
    }

    /// System tray: reopen the window or quit.
    pub(crate) fn poll_tray(&mut self, ctx: &egui::Context) {
        let Some(tray) = &self.tray else {
            return;
        };
        match tray.poll() {
            Some(crate::tray::TrayAction::Open) => {
                self.show_status = false;
                self.show_channels = false;
                self.show_group = false;
                self.show_settings = false;
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            }
            Some(crate::tray::TrayAction::Quit) => {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            None => {}
        }
    }

    /// `WA_DESK_SEND_ONCE="<chat title>|<text>"` sends one message once the chat
    /// list is loaded. It exists so the send path can be smoke-tested against a
    /// real session without driving the mouse; it never runs unless it is set.
    pub(crate) fn smoke_send(&mut self) {
        if self.smoke_sent {
            return;
        }
        let Ok(spec) = std::env::var("WA_DESK_SEND_ONCE") else {
            return;
        };
        let Some((title, body)) = spec.split_once('|') else {
            return;
        };
        let body = body.trim();
        if body.is_empty() {
            return;
        }
        let Some(jid) = self
            .model
            .chats
            .values()
            .find(|row| self.model.row_title(row) == title.trim())
            .map(|row| row.jid.clone())
        else {
            return;
        };
        self.smoke_sent = true;
        self.model.open_chat(&jid);
        self.model.add_own_text(&jid, body, None);
        self.send(Command::SendText {
            chat: jid,
            text: body.to_string(),
        });
        diag(&format!("smoke send to {title}: {body}"));
    }
}
