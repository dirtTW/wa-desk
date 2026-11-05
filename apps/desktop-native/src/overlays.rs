//! The two full-window overlays: the media viewer (image/video stage, filmstrip,
//! transport bar, Save) and the call sheet (avatar, state, mute/hang-up, remote
//! video). Their video helpers live here with them.

use crate::app::{Icon, VideoState, WaDesk, call_initials, diag, draw_icon, now_ms, palette_thumb};
use crate::bubbles::{PHOTO_PX, texture_at};
use crate::model::Command;
use crate::widgets::{avatar_color, fit_text, initials};

impl WaDesk {
    /// The media viewer: a full-window stage, the filmstrip, prev/next, Esc to
    /// close, arrow keys to step, and Save into `Downloads\WA Desk`.
    pub(crate) fn media_overlay(&mut self, ctx: &egui::Context) {
        let Some(item) = self.model.viewer_item() else {
            self.model.close_viewer();
            return;
        };
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.stop_video();
            self.release_video();
            self.model.close_viewer();
            return;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::ArrowRight)) {
            self.stop_video();
            self.model.step_viewer(true);
            return;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::ArrowLeft)) {
            self.stop_video();
            self.model.step_viewer(false);
            return;
        }
        if item.kind == "video" && ctx.input(|i| i.key_pressed(egui::Key::Space)) {
            self.toggle_video(&item.id);
        }
        if item.kind == "video"
            && !self.videos.contains_key(&item.id)
            && let Some(path) = self.model.media_path(&item.id)
        {
            self.videos.insert(item.id.clone(), VideoState::Preparing);
            diag(&format!("video prepare {} <- {path}", item.id));
            let tx = self.video_tx.clone();
            let id = item.id.clone();
            std::thread::spawn(move || {
                let reply = crate::video::prepare(&path, &id);
                let _ = tx.send((id, reply));
            });
        }
        // Reached the end: park on the last frame.
        if item.kind == "video"
            && self.preview_autoplay
            && !self.video_playing
            && self.video_elapsed() < 0.5
            && matches!(self.videos.get(&item.id), Some(VideoState::Ready(_)))
        {
            self.toggle_video(&item.id);
        }
        if item.kind == "video"
            && self.video_playing
            && let Some(VideoState::Ready(video)) = self.videos.get(&item.id)
            && self.video_elapsed() >= video.duration
        {
            self.video_offset = video.duration;
            self.video_playing = false;
            self.video_started = None;
            self.audio.stop();
        }
        if self.model.request_media(&item.id) {
            self.send(Command::FetchMedia {
                id: item.id.clone(),
            });
        }

        let items = self.model.media_items();
        let index = self.model.media_viewer.unwrap_or(0);
        let chat_name = self
            .model
            .chats
            .get(&item.chat)
            .map(|row| self.model.title(row))
            .unwrap_or_default();
        let stamp = self.model.bubble_stamp(&item);
        let texture = texture_at(
            ctx,
            &mut self.textures,
            &item.id,
            self.model.media_path(&item.id),
            PHOTO_PX,
        )
        // Not downloaded yet: show the sender's inline JPEG instead of an empty
        // stage, and let the fetch below fill it in.
        .or_else(|| {
            crate::app::thumb_texture(ctx, &mut self.textures, &item.id, item.thumb.as_deref())
        });
        let thumbs: Vec<(String, Option<egui::TextureHandle>)> = items
            .iter()
            .map(|entry| {
                let texture = texture_at(
                    ctx,
                    &mut self.textures,
                    &entry.id,
                    self.model.media_path(&entry.id),
                    PHOTO_PX,
                );
                (entry.id.clone(), texture)
            })
            .collect();

        let screen = ctx.screen_rect();
        // Paint at the clip's own rate: a 30 fps clip sampled by a 10 fps timer
        // looks like it stutters no matter how well the frames are decoded.
        if item.kind == "video" && self.video_playing {
            let fps = match self.videos.get(&item.id) {
                Some(VideoState::Ready(video)) => video.fps,
                _ => 30.0,
            };
            ctx.request_repaint_after(std::time::Duration::from_secs_f32(1.0 / fps.max(1.0)));
        }

        let avatar = self.avatar_for(&item.chat).cloned();
        let chat_title = self
            .model
            .chats
            .get(&item.chat)
            .map(|row| self.model.row_title(row))
            .unwrap_or(chat_name.clone());
        let when = format!("Today at {}", stamp);
        let items_len = items.len();
        let selected = index;
        let mut open: Option<String> = None;
        let mut close = false;
        let mut toggle = false;
        let mut seek_to: Option<f32> = None;
        let mut toggle_mute = false;
        let mut speed_click = false;
        let mut starred = false;
        let mut zoom_step: Option<f32> = None;
        let mut collapse = false;
        let mut save = false;
        let mut nav: Option<bool> = None;
        let description = item.text.clone();
        let note = self.note.clone();
        let kind = item.kind.clone();
        let item_id = item.id.clone();

        egui::Area::new(egui::Id::new("media-viewer"))
            .order(egui::Order::Foreground)
            .fixed_pos(screen.min)
            .show(ctx, |ui| {
                let painter = ui.painter().clone();
                let backdrop = egui::Color32::from_rgb(11, 20, 26);
                painter.rect_filled(screen, 0.0, backdrop);

                // ---- header: who sent it, and the toolbar -----------------
                let header = 58.0;
                let avatar_rect = egui::Rect::from_min_size(
                    egui::pos2(screen.left() + 20.0, screen.top() + 12.0),
                    egui::vec2(33.0, 33.0),
                );
                if let Some(texture) = &avatar {
                    painter.image(
                        texture.id(),
                        avatar_rect,
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        egui::Color32::WHITE,
                    );
                } else {
                    painter.circle_filled(avatar_rect.center(), 16.5, avatar_color(&chat_title));
                    painter.text(
                        avatar_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        initials(&chat_title),
                        egui::FontId::proportional(13.0),
                        egui::Color32::WHITE,
                    );
                }
                painter.text(
                    egui::pos2(screen.left() + 62.0, screen.top() + 12.0),
                    egui::Align2::LEFT_TOP,
                    fit_text(
                        &painter,
                        &chat_title,
                        egui::FontId::proportional(14.5),
                        egui::Color32::from_gray(238),
                        (screen.width() * 0.4).max(120.0),
                    ),
                    egui::FontId::proportional(14.5),
                    egui::Color32::from_gray(238),
                );
                painter.text(
                    egui::pos2(screen.left() + 62.0, screen.top() + 30.0),
                    egui::Align2::LEFT_TOP,
                    &when,
                    egui::FontId::proportional(12.0),
                    egui::Color32::from_gray(150),
                );

                // The toolbar, right to left: close, menu, forward, pin, star,
                // reply - the official order.
                let icons: Vec<(Icon, &str)> = vec![
                    (Icon::Close, "close"),
                    (Icon::Menu, "menu"),
                    (Icon::Forward, "forward"),
                    (Icon::Download, "download"),
                    (Icon::ChevronDown, "collapse"),
                    (Icon::Star, "star"),
                    (Icon::Reply, "reply"),
                    (Icon::ZoomIn, "zoom-in"),
                    (Icon::ZoomOut, "zoom-out"),
                ];
                let mut x = screen.right() - 18.0;
                for (icon, name) in icons {
                    let rect = egui::Rect::from_center_size(
                        egui::pos2(x - 17.0, screen.top() + header / 2.0),
                        egui::vec2(34.0, 34.0),
                    );
                    let response = ui.interact(rect, ui.id().with(name), egui::Sense::click());
                    let color = if response.hovered() {
                        egui::Color32::from_gray(245)
                    } else {
                        egui::Color32::from_gray(200)
                    };
                    if response.hovered() {
                        painter.rect_filled(
                            rect,
                            egui::CornerRadius::same(17),
                            egui::Color32::from_white_alpha(22),
                        );
                    }
                    draw_icon(ui, rect.shrink(8.0), icon, color);
                    if response.clicked() {
                        match name {
                            "close" => close = true,
                            "star" => starred = true,
                            "forward" => open = Some(String::new()),
                            "download" => save = true,
                            "zoom-in" => zoom_step = Some(1.25),
                            "zoom-out" => zoom_step = Some(0.8),
                            "collapse" => collapse = true,
                            _ => {}
                        }
                    }
                    x -= 38.0;
                }

                // ---- the media itself ------------------------------------
                let strip = 74.0;
                let counter_row = 26.0;
                // The caption is measured first so the stage stops short of it:
                // it used to overlap the filmstrip on long captions.
                let caption_galley = (!description.trim().is_empty()).then(|| {
                    painter.layout(
                        description.clone(),
                        egui::FontId::proportional(14.0),
                        egui::Color32::from_gray(226),
                        (screen.width() - 120.0).max(160.0),
                    )
                });
                let caption_height = caption_galley
                    .as_ref()
                    .map(|galley| galley.size().y + 10.0)
                    .unwrap_or(0.0);
                let stage = egui::Rect::from_min_max(
                    egui::pos2(screen.left() + 24.0, screen.top() + header),
                    egui::pos2(
                        screen.right() - 24.0,
                        screen.bottom() - strip - counter_row - 12.0 - caption_height,
                    ),
                );
                let mut media_rect = stage;
                if kind == "video" {
                    // The transport bar is an overlay inside the media, so the
                    // stage is measured to the frame, not around a separate row.
                    match self.video_frame_handle(ctx, &item_id) {
                        Some(frame) => {
                            let size = frame.size_vec2();
                            let scale = (stage.width() / size.x)
                                .min(stage.height() / size.y)
                                .max(0.01)
                                * self.viewer_zoom;
                            let target = egui::Rect::from_center_size(stage.center(), size * scale);
                            media_rect = target;
                            painter.image(
                                frame.id(),
                                target,
                                egui::Rect::from_min_max(
                                    egui::pos2(0.0, 0.0),
                                    egui::pos2(1.0, 1.0),
                                ),
                                egui::Color32::WHITE,
                            );
                            if !self.video_playing {
                                painter.rect_filled(
                                    target,
                                    egui::CornerRadius::same(2),
                                    egui::Color32::from_black_alpha(40),
                                );
                            }
                        }
                        None => {
                            let message = match self.videos.get(&item_id) {
                                Some(VideoState::Failed(err)) => format!("Cannot play: {err}"),
                                Some(VideoState::Ready(_)) => "Loading frame.".to_string(),
                                _ => "Preparing video.".to_string(),
                            };
                            painter.text(
                                stage.center(),
                                egui::Align2::CENTER_CENTER,
                                message,
                                egui::FontId::proportional(14.0),
                                egui::Color32::from_gray(190),
                            );
                        }
                    }
                } else {
                    match &texture {
                        Some(texture) => {
                            let size = texture.size_vec2();
                            let scale = (stage.width() / size.x)
                                .min(stage.height() / size.y)
                                .clamp(0.01, 1.0)
                                * self.viewer_zoom;
                            let target = egui::Rect::from_center_size(stage.center(), size * scale);
                            media_rect = target;
                            painter.image(
                                texture.id(),
                                target,
                                egui::Rect::from_min_max(
                                    egui::pos2(0.0, 0.0),
                                    egui::pos2(1.0, 1.0),
                                ),
                                egui::Color32::WHITE,
                            );
                        }
                        None => {
                            painter.text(
                                stage.center(),
                                egui::Align2::CENTER_CENTER,
                                "Loading.",
                                egui::FontId::proportional(14.0),
                                egui::Color32::from_gray(190),
                            );
                        }
                    }
                }

                // Clicking the frame plays or pauses, like the official viewer.
                let hit = ui.interact(
                    media_rect,
                    ui.id().with("media-stage"),
                    egui::Sense::click(),
                );
                if hit.clicked() && kind == "video" {
                    toggle = true;
                }

                // Previous / next arrows in the gutters, like the official
                // viewer (they only appear when the chat has more than one).
                if items_len > 1 {
                    for (left, icon, step, name) in [
                        (true, Icon::ChevronLeft, false, "prev-media"),
                        (false, Icon::ChevronRight, true, "next-media"),
                    ] {
                        let x = if left {
                            stage.left() + 20.0
                        } else {
                            stage.right() - 20.0
                        };
                        let rect = egui::Rect::from_center_size(
                            egui::pos2(x, stage.center().y),
                            egui::vec2(40.0, 40.0),
                        );
                        let response = ui.interact(rect, ui.id().with(name), egui::Sense::click());
                        let fill = if response.hovered() {
                            egui::Color32::from_black_alpha(150)
                        } else {
                            egui::Color32::from_black_alpha(90)
                        };
                        painter.circle_filled(rect.center(), 20.0, fill);
                        draw_icon(ui, rect.shrink(10.0), icon, egui::Color32::from_gray(235));
                        if response.clicked() {
                            nav = Some(step);
                        }
                    }
                }

                // ---- video transport overlay ------------------------------
                if kind == "video" {
                    let bar = egui::Rect::from_min_max(
                        egui::pos2(media_rect.left(), media_rect.bottom() - 46.0),
                        media_rect.right_bottom(),
                    );
                    painter.rect_filled(
                        bar,
                        egui::CornerRadius {
                            nw: 0,
                            ne: 0,
                            sw: 2,
                            se: 2,
                        },
                        egui::Color32::from_black_alpha(90),
                    );
                    let middle = bar.center().y;
                    let play_rect = egui::Rect::from_center_size(
                        egui::pos2(bar.left() + 27.0, middle),
                        egui::vec2(34.0, 34.0),
                    );
                    let play = ui.interact(play_rect, ui.id().with("v-play"), egui::Sense::click());
                    draw_icon(
                        ui,
                        play_rect.shrink(7.0),
                        if self.video_playing {
                            Icon::Pause
                        } else {
                            Icon::Play
                        },
                        egui::Color32::from_gray(240),
                    );
                    if play.clicked() {
                        toggle = true;
                    }

                    let duration = match self.videos.get(&item_id) {
                        Some(VideoState::Ready(video)) => video.duration,
                        _ => 0.0,
                    };
                    let elapsed = self.video_elapsed().clamp(0.0, duration.max(0.0));
                    let seek_rect = egui::Rect::from_min_max(
                        egui::pos2(bar.left() + 52.0, middle - 9.0),
                        egui::pos2(bar.right() - 130.0, middle + 9.0),
                    );
                    let response = ui.interact(
                        seek_rect,
                        ui.id().with("v-seek"),
                        egui::Sense::click_and_drag(),
                    );
                    let line_y = middle;
                    painter.line_segment(
                        [
                            egui::pos2(seek_rect.left(), line_y),
                            egui::pos2(seek_rect.right(), line_y),
                        ],
                        egui::Stroke::new(3.0_f32, egui::Color32::from_white_alpha(70)),
                    );
                    let ratio = if duration > 0.0 {
                        (elapsed / duration).clamp(0.0, 1.0)
                    } else {
                        0.0
                    };
                    let head = seek_rect.left() + seek_rect.width() * ratio;
                    painter.line_segment(
                        [
                            egui::pos2(seek_rect.left(), line_y),
                            egui::pos2(head, line_y),
                        ],
                        egui::Stroke::new(3.0_f32, egui::Color32::from_gray(240)),
                    );
                    painter.circle_filled(
                        egui::pos2(head, line_y),
                        6.0,
                        egui::Color32::from_gray(245),
                    );
                    if (response.clicked() || response.dragged())
                        && let Some(pos) = response.interact_pointer_pos()
                    {
                        let ratio =
                            ((pos.x - seek_rect.left()) / seek_rect.width()).clamp(0.0, 1.0);
                        seek_to = Some(ratio * duration);
                    }
                    // The speed is a control, not a caption: each tap cycles
                    // 0.5x -> 1x -> 1.5x -> 2x and re-times the playout.
                    let speed_rect = egui::Rect::from_min_size(
                        egui::pos2(bar.right() - 126.0, middle - 9.0),
                        egui::vec2(32.0, 18.0),
                    );
                    let speed_hit =
                        ui.interact(speed_rect, ui.id().with("v-speed"), egui::Sense::click());
                    if speed_hit.clicked() {
                        speed_click = true;
                    }
                    painter.text(
                        egui::pos2(bar.right() - 120.0, middle),
                        egui::Align2::LEFT_CENTER,
                        format!("{:.1}x", self.video_speed),
                        egui::FontId::proportional(12.5),
                        egui::Color32::from_gray(230),
                    );
                    let mute_rect = egui::Rect::from_center_size(
                        egui::pos2(bar.right() - 74.0, middle),
                        egui::vec2(28.0, 28.0),
                    );
                    let mute = ui.interact(mute_rect, ui.id().with("v-mute"), egui::Sense::click());
                    draw_icon(
                        ui,
                        mute_rect.shrink(6.0),
                        if self.video_muted {
                            Icon::Mute
                        } else {
                            Icon::Volume
                        },
                        egui::Color32::from_gray(235),
                    );
                    if mute.clicked() {
                        toggle_mute = true;
                    }
                    let full_rect = egui::Rect::from_center_size(
                        egui::pos2(bar.right() - 40.0, middle),
                        egui::vec2(28.0, 28.0),
                    );
                    draw_icon(
                        ui,
                        full_rect.shrink(6.0),
                        Icon::Fullscreen,
                        egui::Color32::from_gray(235),
                    );
                }

                // ---- caption and the filmstrip (the chevron hides them) -----
                if let Some(galley) = &caption_galley {
                    painter.galley(
                        egui::pos2(
                            screen.center().x - galley.size().x / 2.0,
                            stage.bottom() + 6.0,
                        ),
                        galley.clone(),
                        egui::Color32::from_gray(226),
                    );
                }

                // The strip: 64 px rounded thumbnails, the open one ringed green.
                let thumb_side = 54.0;
                let selected_side = 60.0;
                let gap = 4.0;
                let total = items_len as f32 * (thumb_side + gap) - gap;
                // The live filmstrip runs the full width of the stage: it starts
                // at the stage's left edge when it fits, and only centres when a
                // short strip would otherwise sit hard against one side.
                let span = stage.width().max(1.0);
                let mut thumb_x = if total >= span {
                    stage.left()
                } else {
                    stage.center().x - total / 2.0
                };
                let thumb_y = screen.bottom() - strip + 6.0;
                for (position, (id, thumb)) in thumbs.iter().enumerate() {
                    let side = if position == selected {
                        selected_side
                    } else {
                        thumb_side
                    };
                    let rect = egui::Rect::from_min_size(
                        egui::pos2(thumb_x, thumb_y - (side - 62.0) / 2.0),
                        egui::vec2(side, side),
                    );
                    thumb_x += side + gap;
                    match thumb {
                        Some(texture) => {
                            let size = texture.size_vec2();
                            let scale = (side / size.x).min(side / size.y);
                            let fitted = egui::Rect::from_center_size(rect.center(), size * scale);
                            painter.image(
                                texture.id(),
                                fitted,
                                egui::Rect::from_min_max(
                                    egui::pos2(0.0, 0.0),
                                    egui::pos2(1.0, 1.0),
                                ),
                                egui::Color32::WHITE,
                            );
                        }
                        None => {
                            painter.rect_filled(rect, egui::CornerRadius::same(4), palette_thumb());
                        }
                    }
                    let is_video = items
                        .get(position)
                        .map(|entry| entry.kind == "video")
                        .unwrap_or(false);
                    if is_video {
                        let duration = match self.videos.get(id) {
                            Some(VideoState::Ready(video)) => {
                                Some(crate::app::stamp_seconds(video.duration))
                            }
                            _ => None,
                        };
                        let width = if duration.is_some() { 30.0 } else { 16.0 };
                        let badge = egui::Rect::from_min_size(
                            egui::pos2(rect.left() + 4.0, rect.bottom() - 18.0),
                            egui::vec2(width, 14.0),
                        );
                        painter.rect_filled(
                            badge,
                            egui::CornerRadius::same(3),
                            egui::Color32::from_black_alpha(150),
                        );
                        match duration {
                            Some(label) => {
                                painter.text(
                                    badge.center(),
                                    egui::Align2::CENTER_CENTER,
                                    label,
                                    egui::FontId::proportional(10.0),
                                    egui::Color32::from_gray(240),
                                );
                            }
                            None => {
                                draw_icon(
                                    ui,
                                    badge.shrink(2.0),
                                    Icon::Play,
                                    egui::Color32::from_gray(240),
                                );
                            }
                        }
                    }
                    if position == selected {
                        painter.rect_stroke(
                            rect.expand(1.0),
                            egui::CornerRadius::same(4),
                            egui::Stroke::new(2.0_f32, egui::Color32::from_rgb(0, 168, 132)),
                            egui::StrokeKind::Outside,
                        );
                    }
                    let response = ui.interact(
                        rect,
                        ui.id().with(("thumb", position)),
                        egui::Sense::click(),
                    );
                    if response.clicked() && position != selected {
                        open = Some(id.clone());
                    }
                }

                if !note.is_empty() {
                    painter.text(
                        egui::pos2(screen.right() - 24.0, screen.bottom() - 18.0),
                        egui::Align2::RIGHT_BOTTOM,
                        note,
                        egui::FontId::proportional(11.5),
                        egui::Color32::from_gray(170),
                    );
                }
            });

        if let Some(step) = zoom_step {
            self.viewer_zoom = (self.viewer_zoom * step).clamp(0.5, 4.0);
        }
        if collapse {
            self.viewer_details = !self.viewer_details;
        }
        if close {
            self.stop_video();
            self.release_video();
            self.model.close_viewer();
            return;
        }
        if starred {
            self.model.toggle_star(&item.id);
        }
        if save {
            self.note = match self.model.save_media(&item.id) {
                Ok(path) => format!("Saved to {path}"),
                Err(err) => err,
            };
        }
        if open == Some(String::new()) {
            self.model.forward_target = Some(item.clone());
            self.model.forward_search.clear();
            self.model.forward_chosen.clear();
        } else if let Some(id) = open {
            self.stop_video();
            self.model.open_viewer(&id);
        }
        if let Some(step) = nav {
            self.stop_video();
            self.model.step_viewer(step);
            return;
        }
        if toggle {
            self.toggle_video(&item.id);
        }
        if speed_click {
            self.video_speed = match self.video_speed {
                speed if speed < 0.75 => 1.0,
                speed if speed < 1.25 => 1.5,
                speed if speed < 1.75 => 2.0,
                _ => 0.5,
            };
            self.audio.set_speed(self.video_speed);
            ctx.request_repaint();
        }
        if toggle_mute {
            self.video_muted = !self.video_muted;
            self.audio
                .set_volume(if self.video_muted { 0.0 } else { 1.0 });
            // Without this the speaker glyph kept its old state until the mouse
            // moved: nothing scheduled a frame while a clip was paused.
            ctx.request_repaint();
        }
        if let Some(seconds) = seek_to {
            self.seek_video(&item.id, seconds);
        }
    }

    /// The call sheet: ringing (accept/decline) or the live call (mute,
    /// hang-up), with the remote video stage when the call has one.
    pub(crate) fn call_overlay(&mut self, ctx: &egui::Context, call: &crate::model::CallState) {
        let palette = self.palette.clone();
        let now = now_ms();
        let plan = call.overlay(now);
        let status = plan.status(&call.overlay_input(now));
        let screen = ctx.screen_rect();
        let mut action: Option<Command> = None;

        if ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
            action = Some(if plan.ringing {
                Command::CallReject {
                    call_id: call.call_id.clone(),
                }
            } else {
                Command::CallHangup
            });
        }

        egui::Area::new(egui::Id::new("call-overlay"))
            .order(egui::Order::Foreground)
            .fixed_pos(screen.min)
            .show(ctx, |ui| {
                let (rect, _) = ui.allocate_exact_size(screen.size(), egui::Sense::hover());
                let painter = ui.painter().clone();
                painter.rect_filled(
                    rect,
                    egui::CornerRadius::ZERO,
                    egui::Color32::from_rgb(11, 20, 26),
                );
                if plan.video_stage {
                    // In a video call the peer's frame fills the sheet; the name
                    // and controls sit over a scrim, like the official client.
                    let stage = rect.shrink(12.0);
                    painter.rect_filled(stage, egui::CornerRadius::same(12), egui::Color32::BLACK);
                    match &self.call_frame {
                        Some(texture) => {
                            let size = texture.size_vec2();
                            let scale = (stage.width() / size.x.max(1.0))
                                .min(stage.height() / size.y.max(1.0));
                            painter.image(
                                texture.id(),
                                egui::Rect::from_center_size(stage.center(), size * scale),
                                egui::Rect::from_min_max(
                                    egui::pos2(0.0, 0.0),
                                    egui::pos2(1.0, 1.0),
                                ),
                                egui::Color32::WHITE,
                            );
                        }
                        None => {
                            painter.text(
                                stage.center(),
                                egui::Align2::CENTER_CENTER,
                                "Waiting for the peer's video...",
                                egui::FontId::proportional(12.5),
                                egui::Color32::from_gray(150),
                            );
                        }
                    }
                    // Our own camera preview: a mirror thumbnail in the corner,
                    // the way the official client floats the self-view.
                    if let Some(texture) = &self.call_self_frame {
                        let size = texture.size_vec2();
                        let scale = (196.0 / size.x.max(1.0)).min(147.0 / size.y.max(1.0));
                        let thumb = egui::Rect::from_center_size(
                            egui::pos2(
                                stage.right() - 12.0 - size.x * scale / 2.0,
                                stage.bottom() - 12.0 - size.y * scale / 2.0,
                            ),
                            size * scale,
                        );
                        // Mirrored horizontally, like a selfie camera.
                        painter.image(
                            texture.id(),
                            thumb,
                            egui::Rect::from_min_max(egui::pos2(1.0, 0.0), egui::pos2(0.0, 1.0)),
                            egui::Color32::WHITE,
                        );
                    }
                }
                self.ensure_avatar(ui.ctx(), &call.peer);
                let avatar = self.avatar_for(&call.peer).cloned();
                let video = plan.video_stage;
                let avatar_radius = if video { 20.0 } else { 48.0 };
                let avatar_center = if video {
                    egui::pos2(
                        rect.left() + 28.0 + avatar_radius,
                        rect.top() + 22.0 + avatar_radius,
                    )
                } else {
                    egui::pos2(rect.center().x, rect.center().y - 70.0)
                };
                match avatar {
                    Some(texture) => {
                        let size = texture.size_vec2();
                        let scale = (avatar_radius * 2.0) / size.x.max(1.0);
                        painter.image(
                            texture.id(),
                            egui::Rect::from_center_size(avatar_center, size * scale),
                            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                            egui::Color32::WHITE,
                        );
                    }
                    None => {
                        painter.circle_filled(avatar_center, avatar_radius, palette.selected);
                        painter.text(
                            avatar_center,
                            egui::Align2::CENTER_CENTER,
                            call_initials(&call.peer_name),
                            egui::FontId::proportional(avatar_radius * 0.7),
                            palette.text,
                        );
                    }
                }
                if video {
                    painter.text(
                        egui::pos2(
                            avatar_center.x + avatar_radius + 12.0,
                            avatar_center.y - 9.0,
                        ),
                        egui::Align2::LEFT_CENTER,
                        &call.peer_name,
                        egui::FontId::proportional(16.0),
                        palette.text,
                    );
                    painter.text(
                        egui::pos2(
                            avatar_center.x + avatar_radius + 12.0,
                            avatar_center.y + 11.0,
                        ),
                        egui::Align2::LEFT_CENTER,
                        &status,
                        egui::FontId::proportional(12.5),
                        palette.secondary,
                    );
                } else {
                    painter.text(
                        egui::pos2(rect.center().x, avatar_center.y + avatar_radius + 22.0),
                        egui::Align2::CENTER_CENTER,
                        &call.peer_name,
                        egui::FontId::proportional(20.0),
                        palette.text,
                    );
                    painter.text(
                        egui::pos2(rect.center().x, avatar_center.y + avatar_radius + 50.0),
                        egui::Align2::CENTER_CENTER,
                        &status,
                        egui::FontId::proportional(13.0),
                        palette.secondary,
                    );
                }

                // Controls.
                let buttons_y = if video {
                    rect.bottom() - 86.0
                } else {
                    avatar_center.y + 190.0
                };
                let left = egui::pos2(rect.center().x - 62.0, buttons_y);
                let right = egui::pos2(rect.center().x + 62.0, buttons_y);
                let button = |center: egui::Pos2,
                              icon: Icon,
                              background: egui::Color32,
                              foreground: egui::Color32,
                              id: &str,
                              slash: bool|
                 -> bool {
                    let hit_rect = egui::Rect::from_center_size(center, egui::vec2(60.0, 60.0));
                    let response = ui.interact(hit_rect, egui::Id::new(id), egui::Sense::click());
                    painter.circle_filled(
                        center,
                        30.0,
                        if response.hovered() {
                            background.gamma_multiply(1.15)
                        } else {
                            background
                        },
                    );
                    draw_icon(ui, hit_rect.shrink(17.0), icon, foreground);
                    if slash {
                        painter.line_segment(
                            [
                                egui::pos2(center.x - 13.0, center.y + 13.0),
                                egui::pos2(center.x + 13.0, center.y - 13.0),
                            ],
                            egui::Stroke::new(2.5_f32, foreground),
                        );
                    }
                    response.clicked()
                };

                if plan.ringing {
                    let decline = button(
                        left,
                        Icon::CallEnd,
                        palette.danger,
                        egui::Color32::WHITE,
                        "call-decline",
                        false,
                    );
                    let accept = button(
                        right,
                        Icon::Phone,
                        palette.accent,
                        egui::Color32::from_rgb(11, 20, 26),
                        "call-accept",
                        false,
                    );
                    if decline {
                        action = Some(Command::CallReject {
                            call_id: call.call_id.clone(),
                        });
                    }
                    if accept {
                        action = Some(Command::CallAccept {
                            call_id: call.call_id.clone(),
                            video: call.video,
                        });
                    }
                } else {
                    let muted = plan.mute_pressed;
                    let mute = button(
                        left,
                        Icon::Mic,
                        if muted {
                            palette.danger
                        } else {
                            palette.elevated
                        },
                        egui::Color32::WHITE,
                        "call-mute",
                        muted,
                    );
                    let hangup = button(
                        right,
                        Icon::CallEnd,
                        palette.danger,
                        egui::Color32::WHITE,
                        "call-hangup",
                        false,
                    );
                    if mute {
                        action = Some(Command::CallMute { muted: !call.muted });
                    }
                    if hangup {
                        action = Some(Command::CallHangup);
                    }
                }
            });

        if let Some(command) = action {
            // Reflect the intent immediately; the session confirms with events.
            if let Command::CallMute { muted } = &command
                && let Some(state) = self.model.call.as_mut()
            {
                state.muted = *muted;
            }
            self.send(command);
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(500));
    }

    /// Seconds of the clip that have played.
    pub(crate) fn video_elapsed(&self) -> f32 {
        if self.video_playing {
            // The playhead advances at the chosen rate, so 2x really is 2x.
            self.video_started
                .map(|started| started.elapsed().as_secs_f32() * self.video_speed)
                .unwrap_or(self.video_offset)
        } else {
            self.video_offset
        }
    }

    /// Free the decoded frames and the worker of the open clip. Called when the
    /// viewer closes: a session that looked at ten videos should not keep all
    /// of their frames in memory.
    pub(crate) fn release_video(&mut self) {
        self.video_textures.clear();
        self.video_order.clear();
        self.video_prefetch = None;
    }

    pub(crate) fn stop_video(&mut self) {
        if self.video_playing {
            self.audio.stop();
        }
        self.video_playing = false;
        self.video_started = None;
        self.video_offset = 0.0;
    }

    /// The texture for the current frame of a prepared clip.
    ///
    /// The paint thread never decodes: a worker keeps frames ready around the
    /// playhead ([`crate::frames::Prefetch`]) and this only uploads what has
    /// already been decoded. If the exact frame is still in flight the newest
    /// earlier frame is painted, so the video never blanks or flickers.
    pub(crate) fn video_frame_handle(
        &mut self,
        ctx: &egui::Context,
        id: &str,
    ) -> Option<egui::TextureHandle> {
        let VideoState::Ready(video) = self.videos.get(id)?.clone() else {
            return None;
        };
        let index = video.frame_at(self.video_elapsed());
        let dir_key = video.dir.display().to_string();

        // (Re)start the worker when another clip is open.
        let needs_worker = self
            .video_prefetch
            .as_ref()
            .is_none_or(|(key, _)| key != &dir_key);
        if needs_worker {
            self.video_prefetch = crate::frames::Prefetch::start(video.dir.clone(), video.frames)
                .map(|worker| (dir_key.clone(), worker));
        }
        if let Some((_, worker)) = &self.video_prefetch {
            worker.seek(index);
            let ready = worker.drain(10);
            for (number, image) in ready {
                let key = format!("{dir_key}#{number}");
                if self.video_textures.contains_key(&key) {
                    continue;
                }
                let texture = ctx.load_texture(key.clone(), image, egui::TextureOptions::LINEAR);
                self.video_textures.insert(key.clone(), Some(texture));
                self.video_order.push_back(key);
            }
        }
        // 18 frames at 640 px is ~0.6 s of look-ahead in video memory; the LRU
        // drops the frames behind the playhead as they are passed.
        // Keep a generous window and never drop the playhead's own frames: an
        // evicted current frame is what made the picture blink back to the one
        // behind it.
        let keep = |number: usize| number + 1 >= index.saturating_sub(3) && number <= index + 24;
        while self.video_order.len() > 48 {
            let Some(old) = self.video_order.pop_front() else {
                break;
            };
            let protected = old
                .rsplit('#')
                .next()
                .and_then(|tail| tail.parse::<usize>().ok())
                .is_some_and(keep);
            if protected {
                self.video_order.push_back(old);
                continue;
            }
            self.video_textures.remove(&old);
        }

        // The frame itself, otherwise the nearest cached one in either
        // direction: a seek lands where nothing is decoded yet, and dropping to
        // "Loading frame." mid-drag is what made scrubbing flicker.
        let key = format!("{dir_key}#{index}");
        if let Some(Some(texture)) = self.video_textures.get(&key) {
            return Some(texture.clone());
        }
        let nearest = self
            .video_textures
            .iter()
            .filter_map(|(key, texture)| {
                let number = key
                    .strip_prefix(&dir_key)?
                    .strip_prefix('#')?
                    .parse::<usize>()
                    .ok()?;
                texture.as_ref()?;
                Some((number.abs_diff(index), number))
            })
            .min_by_key(|(distance, _)| *distance);
        if let Some((_, number)) = nearest {
            let key = format!("{dir_key}#{number}");
            if let Some(Some(texture)) = self.video_textures.get(&key) {
                return Some(texture.clone());
            }
        }
        for behind in 1..=8 {
            if behind > index {
                break;
            }
            let key = format!("{dir_key}#{}", index - behind);
            if let Some(Some(texture)) = self.video_textures.get(&key) {
                return Some(texture.clone());
            }
        }
        None
    }
}
