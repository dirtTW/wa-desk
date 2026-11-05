//! Bubble chrome shared by every message kind: the message menu, the reaction
//! chips that hang off a bubble, and the day chip. They only see a `Ui`, the
//! palette, the model and the message, so they live outside `app.rs`.

use crate::app::{
    Icon, call_initials, draw_icon, highlight_job, sender_color, stamp_seconds, thumb_texture,
};
use crate::model::BubbleAction;
use crate::model::Model;
use crate::rich_text::{clean_text, emoji_only, has_emoji, has_markup, layout_body, paint_body};
use crate::widgets::{avatar_color, fit_text, initials};
use wa_types::MessageView;

/// WhatsApp's message menu: the quick reaction bar, then the rows `wa-ui`
/// allows for this message.
pub(crate) fn context_menu(
    response: &egui::Response,
    palette: &crate::theme::Palette,
    model: &Model,
    message: &MessageView,
    action: &mut Option<BubbleAction>,
) {
    response.context_menu(|ui| {
        ui.set_min_width(212.0);
        ui.add_space(2.0);
        // The quick reaction bar.
        ui.horizontal(|ui| {
            ui.add_space(2.0);
            for emoji in ["👍", "❤️", "😂", "😮", "😢", "🙏"] {
                let reaction = ui.add(
                    egui::Button::new(egui::RichText::new(emoji).size(19.0))
                        .frame(false)
                        .min_size(egui::vec2(29.0, 29.0)),
                );
                if reaction
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .clicked()
                {
                    *action = Some(BubbleAction::React(message.id.clone(), emoji.to_string()));
                    ui.close();
                }
            }
        });
        ui.add_space(2.0);
        let menu = model.message_menu(message);
        if menu.reply && menu_row(ui, palette, Icon::Reply, "Reply") {
            *action = Some(BubbleAction::Reply(message.id.clone()));
            ui.close();
        }
        if menu.copy && menu_row(ui, palette, Icon::Copy, "Copy") {
            ui.ctx().copy_text(message.text.clone());
            ui.close();
        }
        if menu.forward && menu_row(ui, palette, Icon::Forward, "Forward") {
            *action = Some(BubbleAction::Forward(message.id.clone()));
            ui.close();
        }
        if menu.edit && menu_row(ui, palette, Icon::Edit, "Edit") {
            *action = Some(BubbleAction::Edit(message.id.clone()));
            ui.close();
        }
        if menu.star && menu_row(ui, palette, Icon::Star, &menu.star_label) {
            *action = Some(BubbleAction::Star(message.id.clone()));
            ui.close();
        }
        ui.add_space(2.0);
        if menu_row(ui, palette, Icon::Delete, "Delete") {
            *action = Some(BubbleAction::Delete(message.id.clone()));
            ui.close();
        }
        ui.add_space(2.0);
    });
}

/// The reaction pills under a message, ours in blue like WhatsApp.
pub(crate) fn pills_row(
    ui: &mut egui::Ui,
    palette: &crate::theme::Palette,
    model: &Model,
    message: &MessageView,
    bubble: egui::Rect,
) {
    let pills = model.reactions_for(&message.id);
    if pills.is_empty() {
        return;
    }
    // Left-to-right, so our own reactions under a right-aligned bubble still
    // start at the bubble's left edge like the official client.
    // Left-to-right, so our own reactions under a right-aligned bubble still
    // start at the bubble's edge, not at the message column's.
    let shift = (bubble.left() + 8.0 - ui.cursor().left()).max(0.0);
    ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
        ui.add_space(shift);
        ui.spacing_mut().item_spacing.x = 4.0;
        for (emoji, count, mine) in pills {
            let label = (count > 1).then(|| count.to_string());
            let font = egui::FontId::proportional(11.5);
            let label_width = label
                .as_ref()
                .map(|text| {
                    ui.painter()
                        .layout_no_wrap(text.clone(), font.clone(), palette.secondary)
                        .size()
                        .x
                })
                .unwrap_or(0.0);
            let width = 8.0
                + 14.0
                + if label.is_some() {
                    3.0 + label_width
                } else {
                    0.0
                }
                + 8.0;
            let (slot, _) = ui.allocate_exact_size(egui::vec2(width, 22.0), egui::Sense::hover());
            // Straddle the bubble's bottom edge, like the official chip.
            let rect = slot.translate(egui::vec2(0.0, -12.0));
            ui.painter()
                .rect_filled(rect, egui::CornerRadius::same(11), palette.elevated);
            ui.painter().rect_stroke(
                rect,
                egui::CornerRadius::same(11),
                egui::Stroke::new(1.0_f32, palette.border),
                egui::StrokeKind::Inside,
            );
            let glyph = emoji.chars().next().unwrap_or('?');
            if let Some(texture) = crate::emoji_glyph::texture_shared(ui.ctx(), glyph, 14) {
                let size = texture.size_vec2();
                let scale = (14.0 / size.x.max(1.0)).min(14.0 / size.y.max(1.0));
                ui.painter().image(
                    texture.id(),
                    egui::Rect::from_center_size(
                        egui::pos2(rect.left() + 15.0, rect.center().y),
                        size * scale,
                    ),
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    egui::Color32::WHITE,
                );
            }
            if let Some(label) = label {
                ui.painter().text(
                    egui::pos2(rect.left() + 24.0, rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    label,
                    font,
                    if mine {
                        palette.read
                    } else {
                        palette.secondary
                    },
                );
            }
        }
    });
}

/// A centered date chip ("Today", "12/10/2026") between two days of a chat.
pub(crate) fn day_chip(ui: &mut egui::Ui, palette: &crate::theme::Palette, label: &str) {
    let galley = ui.painter().layout_no_wrap(
        label.to_string(),
        egui::FontId::proportional(11.5),
        palette.secondary,
    );
    let width = galley.size().x + 22.0;
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.add_space(((ui.available_width() - width) / 2.0).max(0.0));
        let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 20.0), egui::Sense::hover());
        ui.painter()
            .rect_filled(rect, egui::CornerRadius::same(10), palette.elevated);
        ui.painter().galley(
            egui::pos2(
                rect.center().x - galley.size().x / 2.0,
                rect.center().y - galley.size().y / 2.0,
            ),
            galley.clone(),
            palette.secondary,
        );
    });
    ui.add_space(4.0);
}

/// Paint one message. Returns the action its context menu chose, or a viewer
/// request when its media was clicked. `search` marks a hit (and highlights the
/// query), `scroll` brings the bubble into view.
#[allow(clippy::too_many_arguments)]
pub(crate) fn bubble(
    ui: &mut egui::Ui,
    palette: &crate::theme::Palette,
    model: &Model,
    message: &MessageView,
    textures: &mut std::collections::HashMap<String, Option<egui::TextureHandle>>,
    ctx: &egui::Context,
    search: Option<&str>,
    scroll: bool,
    playback: &crate::audio::PlaybackState,
    tail: bool,
    avatar: Option<&egui::TextureHandle>,
    show_sender: bool,
) -> Option<BubbleAction> {
    // Group chats wrap every bubble in the same 40 px gutter (photo or initials
    // at the head of a run), so text, photos, stickers and voice notes line up.
    if !(message.is_group && !message.from_me) {
        return bubble_content(
            ui, palette, model, message, textures, ctx, search, scroll, playback, tail,
        );
    }
    ui.horizontal_top(|ui| {
        let (cell, _) = ui.allocate_exact_size(egui::vec2(40.0, 40.0), egui::Sense::hover());
        if show_sender {
            let center = egui::pos2(cell.left() + 16.0, cell.top() + 18.0);
            match avatar {
                Some(texture) => {
                    let size = texture.size_vec2();
                    let scale = (32.0 / size.x.max(1.0)).min(32.0 / size.y.max(1.0));
                    ui.painter().image(
                        texture.id(),
                        egui::Rect::from_center_size(center, size * scale),
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        egui::Color32::WHITE,
                    );
                }
                None => {
                    let name = message.sender_name.clone().unwrap_or_default();
                    ui.painter()
                        .circle_filled(center, 16.0, sender_color(&name, palette));
                    ui.painter().text(
                        center,
                        egui::Align2::CENTER_CENTER,
                        call_initials(&name),
                        egui::FontId::proportional(12.0),
                        egui::Color32::from_rgb(11, 20, 26),
                    );
                }
            }
        }
        ui.vertical(|ui| {
            bubble_content(
                ui, palette, model, message, textures, ctx, search, scroll, playback, tail,
            )
        })
        .inner
    })
    .inner
}

#[allow(clippy::too_many_arguments)]
fn bubble_content(
    ui: &mut egui::Ui,
    palette: &crate::theme::Palette,
    model: &Model,
    message: &MessageView,
    textures: &mut std::collections::HashMap<String, Option<egui::TextureHandle>>,
    ctx: &egui::Context,
    search: Option<&str>,
    scroll: bool,
    playback: &crate::audio::PlaybackState,
    tail: bool,
) -> Option<BubbleAction> {
    let align = if message.from_me {
        egui::Align::Max
    } else {
        egui::Align::Min
    };
    let mut action: Option<BubbleAction> = None;
    let max_width = (ui.available_width() * 0.66).max(160.0);

    ui.with_layout(egui::Layout::top_down(align), |ui| {
        ui.set_max_width(max_width);

        // A sticker paints on its own, without a bubble.
        if message.kind == "sticker" && model.media_path(&message.id).is_none() {
            // Still downloading (or the CDN refused it): the official client
            // shows an empty sticker tile, not a text bubble.
            let (rect, response) =
                ui.allocate_exact_size(egui::vec2(96.0, 96.0), egui::Sense::click());
            ui.painter()
                .rect_filled(rect, egui::CornerRadius::same(10), palette.hover);
            draw_icon(ui, rect.shrink(34.0), Icon::Sticker, palette.secondary);
            context_menu(&response, palette, model, message, &mut action);
            ui.label(
                egui::RichText::new(model.bubble_stamp(message))
                    .size(11.0)
                    .color(palette.secondary),
            );
            pills_row(ui, palette, model, message, rect);
            ui.add_space(if tail { 14.0 } else { 8.0 });
            return;
        }
        // View-once media: the official placeholder - a circled "1", the kind
        // and one of three states. A linked device receives
        // `<unavailable type="view_once">` for the fanout, so the body is only
        // there when a reply quote recovered it (`once_recovered`).
        if message.kind == "view_once" {
            let opened = model.view_once_opened(&message.id);
            let playable = model.view_once_playable(message);
            let width = max_width.min(250.0);
            let (rect, response) =
                ui.allocate_exact_size(egui::vec2(width, 74.0), egui::Sense::click());
            let fill = if message.from_me {
                palette.bubble_out
            } else {
                palette.bubble_in
            };
            ui.painter().rect_filled(
                rect,
                egui::CornerRadius {
                    nw: 7,
                    ne: 7,
                    sw: 7,
                    se: if tail { 0 } else { 7 },
                },
                fill,
            );
            if response.hovered() {
                ui.painter()
                    .rect_filled(rect, egui::CornerRadius::same(7), palette.hover);
            }
            let badge = egui::pos2(rect.left() + 26.0, rect.top() + 28.0);
            ui.painter()
                .circle_stroke(badge, 13.0, egui::Stroke::new(1.6_f32, palette.secondary));
            ui.painter().text(
                badge,
                egui::Align2::CENTER_CENTER,
                "1",
                egui::FontId::proportional(13.0),
                palette.secondary,
            );
            ui.painter().text(
                egui::pos2(rect.left() + 48.0, rect.top() + 22.0),
                egui::Align2::LEFT_CENTER,
                "Photo",
                egui::FontId::proportional(14.0),
                palette.text,
            );
            // The sender's own bubble can never be opened from here, so it says
            // what it is instead of telling the sender to use the phone.
            let state = if message.from_me {
                if opened { "Opened" } else { "View once" }
            } else if opened {
                "Opened"
            } else if playable {
                "Tap to open it once"
            } else {
                "Open this on your phone"
            };
            ui.painter().text(
                egui::pos2(rect.left() + 48.0, rect.top() + 41.0),
                egui::Align2::LEFT_CENTER,
                state,
                egui::FontId::proportional(11.5),
                palette.secondary,
            );
            ui.painter().text(
                egui::pos2(rect.right() - 10.0, rect.bottom() - 7.0),
                egui::Align2::RIGHT_BOTTOM,
                model.bubble_stamp(message),
                egui::FontId::proportional(11.0),
                palette.secondary,
            );
            if playable && response.clicked() {
                action = Some(BubbleAction::OpenMedia(message.id.clone()));
            }
            context_menu(&response, palette, model, message, &mut action);
            pills_row(ui, palette, model, message, rect);
            ui.add_space(if tail { 14.0 } else { 8.0 });
            return;
        }
        // A sticker: still WebP or animated (WhatsApp's animated stickers are
        // WebP with an `ANIM` chunk - the `image` crate only shows frame one, so
        // those are decoded to frames and stepped like a GIF).
        // Animated stickers are WebP animations: `webp_anim` parses the RIFF
        // container and composites the frames itself (this machine's ffmpeg has
        // no `webp_anim` demuxer, so the old "treat it like a GIF" path produced
        // nothing and the sticker stayed a placeholder).
        let sticker_texture = model.media_path(&message.id).and_then(|path| {
            if crate::webp_anim::is_animated(std::path::Path::new(&path)) {
                crate::webp_anim::frame(ctx, textures, &message.id, &path)
            } else {
                texture_at(ctx, textures, &message.id, Some(path), STICKER_PX)
            }
        });
        if message.kind == "sticker"
            && let Some(texture) = sticker_texture
        {
            let size = texture.size_vec2();
            let scale = (128.0 / size.x).min(1.0);
            let response =
                ui.add(egui::Image::new((texture.id(), size * scale)).sense(egui::Sense::click()));
            if scroll {
                response.scroll_to_me(Some(egui::Align::Center));
            }
            if response.clicked() {
                action = Some(BubbleAction::OpenMedia(message.id.clone()));
            }
            context_menu(&response, palette, model, message, &mut action);
            ui.label(
                egui::RichText::new(model.bubble_stamp(message))
                    .size(11.0)
                    .color(palette.secondary),
            );
            pills_row(ui, palette, model, message, response.rect);
            ui.add_space(if tail { 14.0 } else { 8.0 });
            return;
        }

        // A photo paints its bytes once wa-core has the file; a video shows the
        // same texture as its poster. Anything else falls back to the caption or
        // the kind's word from wa-ui.
        // Media that is not on disk yet is still a bubble: the empty tile, the
        // caption and the clock live inside the same rounded background as a
        // loaded photo, exactly like the official client.
        if matches!(message.kind.as_str(), "image" | "video")
            && model.media_path(&message.id).is_none()
        {
            let failed = model.media_error(&message.id);
            let pad = 3.0;
            let gallery = max_width.min(260.0);
            let tile_size = egui::vec2(gallery - pad * 2.0, (gallery - pad * 2.0) * 0.62);
            let time = ui.painter().layout_no_wrap(
                model.bubble_stamp(message),
                egui::FontId::proportional(11.0),
                palette.secondary,
            );
            let tick_width = if message.from_me { 16.0 } else { 0.0 };
            let width = (tile_size.x + pad * 2.0)
                .max(time.size().x + tick_width + 40.0)
                .min(max_width);
            let caption = clean_text(message.text.trim());
            let caption_galley = (!caption.is_empty()).then(|| {
                ui.painter().layout(
                    caption,
                    egui::FontId::proportional(14.0),
                    palette.text,
                    (width - 18.0).max(80.0),
                )
            });
            let caption_pad = 9.0;
            let clock_h = time.size().y + 4.0;
            let mut height = pad + tile_size.y + clock_h + 2.0;
            if let Some(galley) = &caption_galley {
                height += galley.size().y + 8.0;
            }
            let fill = if message.from_me {
                palette.bubble_out
            } else {
                palette.bubble_in
            };
            let square = if tail { 2 } else { 8 };
            let radius = egui::CornerRadius {
                nw: 8,
                ne: 8,
                sw: if message.from_me { 8 } else { square },
                se: if message.from_me { square } else { 8 },
            };
            let (rect, response) =
                ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::click());
            let painter = ui.painter();
            painter.rect_filled(rect, radius, fill);
            let tile = egui::Rect::from_min_size(
                egui::pos2(rect.left() + pad, rect.top() + pad),
                tile_size,
            );
            painter.rect_filled(tile, egui::CornerRadius::same(6), palette.hover);
            // The sender's client embedded a few-kilobyte JPEG; painting it now
            // is what makes a photo appear the moment it arrives, the way the
            // official client does, instead of a grey "loading" tile.
            let preview = thumb_texture(ctx, textures, &message.id, message.thumb.as_deref());
            match &preview {
                Some(texture) => {
                    let size = texture.size_vec2();
                    let scale =
                        (tile.width() / size.x.max(1.0)).max(tile.height() / size.y.max(1.0));
                    let drawn = egui::Rect::from_center_size(tile.center(), size * scale);
                    painter
                        .with_clip_rect(tile.intersect(painter.clip_rect()))
                        .image(
                            texture.id(),
                            drawn,
                            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                            egui::Color32::WHITE,
                        );
                }
                None => {
                    // A play disc for a video, a picture glyph for a photo.
                    let disc = egui::Rect::from_center_size(tile.center(), egui::vec2(46.0, 46.0));
                    painter.circle_filled(disc.center(), 23.0, egui::Color32::from_black_alpha(90));
                    draw_icon(
                        ui,
                        disc.shrink(12.0),
                        if message.kind == "video" {
                            Icon::Play
                        } else {
                            Icon::Document
                        },
                        egui::Color32::from_gray(235),
                    );
                    let kind_label = if message.kind == "video" {
                        "video"
                    } else {
                        "photo"
                    };
                    let label = match &failed {
                        Some(err) => {
                            format!("Couldn't load the {kind_label} - tap to retry ({err})")
                        }
                        None => format!("Loading the {kind_label}…"),
                    };
                    painter.text(
                        egui::pos2(tile.center().x, tile.bottom() - 12.0),
                        egui::Align2::CENTER_BOTTOM,
                        label,
                        egui::FontId::proportional(11.0),
                        palette.secondary,
                    );
                }
            }
            let mut y = tile.bottom() + 5.0;
            if let Some(galley) = &caption_galley {
                let caption_height = galley.size().y;
                painter.galley(
                    egui::pos2(rect.left() + caption_pad, y),
                    galley.clone(),
                    palette.text,
                );
                y += caption_height;
            }
            let _ = y;
            painter.galley(
                egui::pos2(
                    rect.right() - caption_pad - tick_width - time.size().x,
                    rect.bottom() - 5.0 - time.size().y,
                ),
                time,
                palette.secondary,
            );
            if message.from_me {
                let status = model.tick_status(message);
                let read = matches!(status, "read" | "played");
                let delivered = matches!(status, "delivered" | "read" | "played");
                let name = if delivered { "done_all" } else { "check" };
                let color = if read {
                    palette.read
                } else {
                    palette.secondary
                };
                if let Some(icon) = crate::icons::texture(ui.ctx(), name, 14, color) {
                    painter.image(
                        icon.id(),
                        egui::Rect::from_min_size(
                            egui::pos2(rect.right() - caption_pad - 14.0, rect.bottom() - 19.0),
                            egui::vec2(14.0, 14.0),
                        ),
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        egui::Color32::WHITE,
                    );
                }
            }
            if response.clicked() {
                // Opening the viewer also asks for the file again, so a tap
                // never just re-arms a download that never arrived.
                action = Some(BubbleAction::OpenMedia(message.id.clone()));
            }
            context_menu(&response, palette, model, message, &mut action);
            pills_row(ui, palette, model, message, rect);
            ui.add_space(if tail { 14.0 } else { 8.0 });
            return;
        }
        if matches!(message.kind.as_str(), "image" | "video")
            && let Some(texture) = message
                .gif
                .then(|| {
                    model
                        .media_path(&message.id)
                        .and_then(|path| crate::gif::frame(ctx, textures, &message.id, &path))
                })
                .flatten()
                .or_else(|| {
                    // A video paints its poster frame; the mp4 itself is not an
                    // image the texture loader can read.
                    let source = if message.kind == "video" {
                        model.poster_of(&message.id)
                    } else {
                        model.media_path(&message.id)
                    };
                    texture_at(ctx, textures, &message.id, source, PHOTO_PX)
                })
                // Nothing on disk yet: the few-kilobyte JPEG the sender's
                // client embedded paints immediately, which is what makes the
                // official bubble appear the moment a photo arrives.
                .or_else(|| thumb_texture(ctx, textures, &message.id, message.thumb.as_deref()))
        {
            // The photo/video sits inside the same bubble as text: padding,
            // rounded corners with the run's tail, caption and clock inside.
            let size = texture.size_vec2();
            let pad = 3.0;
            let scale = (240.0 / size.x.max(1.0)).min(1.0);
            let image_size = size * scale;
            let time = ui.painter().layout_no_wrap(
                model.bubble_stamp(message),
                egui::FontId::proportional(11.0),
                palette.secondary,
            );
            let tick_width = if message.from_me { 16.0 } else { 0.0 };
            let width = (image_size.x + pad * 2.0)
                .max(time.size().x + tick_width + 40.0)
                .min(max_width);
            let caption = clean_text(message.text.trim());
            let caption_galley = (!caption.is_empty()).then(|| {
                ui.painter().layout(
                    caption,
                    egui::FontId::proportional(14.0),
                    palette.text,
                    (width - 18.0).max(80.0),
                )
            });
            let caption_pad = 9.0;
            let clock_h = time.size().y + 4.0;
            let mut height = pad + image_size.y + clock_h + 2.0;
            if let Some(galley) = &caption_galley {
                height += galley.size().y + 8.0;
            }
            let fill = if message.from_me {
                palette.bubble_out
            } else {
                palette.bubble_in
            };
            let square = if tail { 2 } else { 8 };
            let radius = egui::CornerRadius {
                nw: 8,
                ne: 8,
                sw: if message.from_me { 8 } else { square },
                se: if message.from_me { square } else { 8 },
            };
            let (rect, response) =
                ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::click());
            ui.painter().rect_filled(rect, radius, fill);
            // The photo carries the bubble's rounding, not square corners.
            ui.put(
                egui::Rect::from_min_size(
                    egui::pos2(rect.left() + pad, rect.top() + pad),
                    image_size,
                ),
                egui::Image::new((texture.id(), image_size))
                    .corner_radius(egui::CornerRadius::same(6)),
            );
            let painter = ui.painter();
            let mut y = rect.top() + pad + image_size.y + 6.0;
            if let Some(galley) = caption_galley {
                let height = galley.size().y;
                painter.galley(
                    egui::pos2(rect.left() + caption_pad, y),
                    galley,
                    palette.text,
                );
                y += height;
            }
            let _ = y;
            painter.galley(
                egui::pos2(
                    rect.right() - caption_pad - tick_width - time.size().x,
                    rect.bottom() - 5.0 - time.size().y,
                ),
                time,
                palette.secondary,
            );
            if message.from_me {
                let status = model.tick_status(message);
                let read = matches!(status, "read" | "played");
                let delivered = matches!(status, "delivered" | "read" | "played");
                let name = if delivered { "done_all" } else { "check" };
                let color = if read {
                    palette.read
                } else {
                    palette.secondary
                };
                if let Some(icon) = crate::icons::texture(ui.ctx(), name, 14, color) {
                    painter.image(
                        icon.id(),
                        egui::Rect::from_min_size(
                            egui::pos2(rect.right() - caption_pad - 14.0, rect.bottom() - 19.0),
                            egui::vec2(14.0, 14.0),
                        ),
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        egui::Color32::WHITE,
                    );
                }
            }
            if scroll {
                response.scroll_to_me(Some(egui::Align::Center));
            }
            if response.clicked() {
                action = Some(BubbleAction::OpenMedia(message.id.clone()));
            }
            context_menu(&response, palette, model, message, &mut action);
            pills_row(ui, palette, model, message, rect);
            ui.add_space(if tail { 14.0 } else { 8.0 });
            return;
        }

        // A document card: file icon, name, size and pages, then the caption.
        // A location: the pin, the place and the address, the way the official
        // map card reads (a live location carries its own chip).
        if message.kind == "location"
            && let Some(place) = message.location.clone()
        {
            let width = max_width.min(280.0);
            let title = place
                .name
                .clone()
                .filter(|name| !name.trim().is_empty())
                .unwrap_or_else(|| "Location".to_string());
            let coords = format!("{:.4}, {:.4}", place.lat, place.lng);
            let (rect, response) =
                ui.allocate_exact_size(egui::vec2(width, 84.0), egui::Sense::click());
            let fill = if message.from_me {
                palette.bubble_out
            } else {
                palette.bubble_in
            };
            let painter = ui.painter();
            painter.rect_filled(rect, egui::CornerRadius::same(8), fill);
            // A small map-ish tile with the pin.
            let tile = egui::Rect::from_min_size(
                egui::pos2(rect.left() + 6.0, rect.top() + 6.0),
                egui::vec2(70.0, 72.0),
            );
            painter.rect_filled(tile, egui::CornerRadius::same(6), palette.hover);
            draw_icon(
                ui,
                egui::Rect::from_center_size(tile.center(), egui::vec2(26.0, 26.0)),
                Icon::Status,
                palette.accent,
            );
            painter.text(
                egui::pos2(rect.left() + 86.0, rect.top() + 18.0),
                egui::Align2::LEFT_CENTER,
                fit_text(
                    painter,
                    &title,
                    egui::FontId::proportional(14.5),
                    palette.text,
                    rect.width() - 100.0,
                ),
                egui::FontId::proportional(14.5),
                palette.text,
            );
            painter.text(
                egui::pos2(rect.left() + 86.0, rect.top() + 38.0),
                egui::Align2::LEFT_CENTER,
                coords,
                egui::FontId::proportional(12.5),
                palette.secondary,
            );
            if place.live {
                painter.text(
                    egui::pos2(rect.left() + 86.0, rect.top() + 58.0),
                    egui::Align2::LEFT_CENTER,
                    "Live location",
                    egui::FontId::proportional(11.5),
                    palette.accent,
                );
            } else if let Some(address) = place.address.clone() {
                painter.text(
                    egui::pos2(rect.left() + 86.0, rect.top() + 58.0),
                    egui::Align2::LEFT_CENTER,
                    fit_text(
                        painter,
                        &address,
                        egui::FontId::proportional(11.5),
                        palette.secondary,
                        rect.width() - 100.0,
                    ),
                    egui::FontId::proportional(11.5),
                    palette.secondary,
                );
            }
            context_menu(&response, palette, model, message, &mut action);
            pills_row(ui, palette, model, message, rect);
            ui.add_space(if tail { 14.0 } else { 8.0 });
            return;
        }

        // A poll: the question, then one row per option with the official dot.
        if message.kind == "poll"
            && let Some(poll) = message.poll.clone()
        {
            let width = max_width.min(300.0);
            let question = ui.painter().layout(
                poll.question.clone(),
                egui::FontId::proportional(15.0),
                palette.text,
                width - 20.0,
            );
            let mut height = question.size().y + 26.0;
            for option in &poll.options {
                let row = ui.painter().layout(
                    option.clone(),
                    egui::FontId::proportional(14.0),
                    palette.text,
                    width - 60.0,
                );
                height += row.size().y + 16.0;
            }
            let (rect, response) =
                ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::click());
            let fill = if message.from_me {
                palette.bubble_out
            } else {
                palette.bubble_in
            };
            let painter = ui.painter();
            painter.rect_filled(rect, egui::CornerRadius::same(8), fill);
            painter.galley(
                egui::pos2(rect.left() + 10.0, rect.top() + 10.0),
                question.clone(),
                palette.text,
            );
            let mut y = rect.top() + 10.0 + question.size().y + 8.0;
            for option in &poll.options {
                let dot = egui::pos2(rect.left() + 18.0, y + 9.0);
                painter.circle_stroke(dot, 6.0, egui::Stroke::new(1.5_f32, palette.secondary));
                let row = painter.layout(
                    option.clone(),
                    egui::FontId::proportional(14.0),
                    palette.text,
                    width - 46.0,
                );
                painter.galley(egui::pos2(rect.left() + 32.0, y), row.clone(), palette.text);
                y += row.size().y + 16.0;
            }
            context_menu(&response, palette, model, message, &mut action);
            pills_row(ui, palette, model, message, rect);
            ui.add_space(if tail { 14.0 } else { 8.0 });
            return;
        }

        // A shared contact: initials disc, name and number on a card.
        if message.kind == "contact"
            && let Some(card) = message.contact.clone()
        {
            let width = max_width.min(260.0);
            let (rect, response) =
                ui.allocate_exact_size(egui::vec2(width, 78.0), egui::Sense::click());
            let fill = if message.from_me {
                palette.bubble_out
            } else {
                palette.bubble_in
            };
            let painter = ui.painter();
            painter.rect_filled(rect, egui::CornerRadius::same(8), fill);
            let disc = egui::pos2(rect.left() + 32.0, rect.center().y);
            painter.circle_filled(disc, 20.0, avatar_color(&card.name));
            painter.text(
                disc,
                egui::Align2::CENTER_CENTER,
                initials(&card.name),
                egui::FontId::proportional(14.0),
                egui::Color32::WHITE,
            );
            painter.text(
                egui::pos2(rect.left() + 62.0, rect.center().y - 10.0),
                egui::Align2::LEFT_CENTER,
                fit_text(
                    painter,
                    &card.name,
                    egui::FontId::proportional(14.5),
                    palette.text,
                    width - 80.0,
                ),
                egui::FontId::proportional(14.5),
                palette.text,
            );
            painter.text(
                egui::pos2(rect.left() + 62.0, rect.center().y + 10.0),
                egui::Align2::LEFT_CENTER,
                card.phone
                    .as_deref()
                    .map(wa_ui::text::format_phone)
                    .unwrap_or_default(),
                egui::FontId::proportional(12.5),
                palette.secondary,
            );
            context_menu(&response, palette, model, message, &mut action);
            pills_row(ui, palette, model, message, rect);
            ui.add_space(if tail { 14.0 } else { 8.0 });
            return;
        }

        if message.kind == "document"
            && let Some(document) = message.document.as_ref()
        {
            let width = max_width.min(310.0);
            let pad = 10.0;
            let icon = 38.0;
            let text_width = (width - pad * 3.0 - icon).max(80.0);
            let name = if document.name.trim().is_empty() {
                model.bubble_body(message)
            } else {
                document.name.clone()
            };
            let caption = if document.name.trim().is_empty() {
                String::new()
            } else {
                message.text.clone()
            };
            let name_galley = ui.painter().layout_job(egui::text::LayoutJob::simple(
                name,
                egui::FontId::proportional(14.0),
                palette.text,
                text_width,
            ));
            let mut meta = crate::model::Model::file_size(document.size);
            if document.pages > 0 {
                meta.push_str(&format!(" · {} pages", document.pages));
            }
            let meta_galley = ui.painter().layout_no_wrap(
                meta,
                egui::FontId::proportional(11.5),
                palette.secondary,
            );
            let caption_galley = (!caption.trim().is_empty()).then(|| {
                ui.painter().layout_job(egui::text::LayoutJob::simple(
                    caption,
                    egui::FontId::proportional(13.5),
                    palette.text,
                    width - pad * 2.0,
                ))
            });
            let time = ui.painter().layout_no_wrap(
                model.bubble_stamp(message),
                egui::FontId::proportional(11.0),
                palette.secondary,
            );

            let name_height = name_galley.size().y;
            let row = (name_height + meta_galley.size().y + 3.0).max(icon);
            let mut height = pad + row + 4.0 + time.size().y + 4.0;
            if let Some(galley) = &caption_galley {
                height += galley.size().y + 6.0;
            }
            let (rect, hit) =
                ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::click());

            let fill = if message.from_me {
                palette.bubble_out
            } else {
                palette.bubble_in
            };
            let radius = egui::CornerRadius {
                nw: 8,
                ne: 8,
                sw: if message.from_me { 8 } else { 2 },
                se: if message.from_me { 2 } else { 8 },
            };
            let painter = ui.painter();
            painter.rect_filled(rect, radius, fill);
            let icon_rect = egui::Rect::from_min_size(
                egui::pos2(rect.left() + pad, rect.top() + pad),
                egui::vec2(icon, icon),
            );
            painter.rect_filled(icon_rect, egui::CornerRadius::same(8), palette.elevated);
            draw_icon(ui, icon_rect.shrink(8.0), Icon::Document, palette.text);

            let text_x = rect.left() + pad * 2.0 + icon;
            painter.galley(
                egui::pos2(text_x, rect.top() + pad),
                name_galley,
                palette.text,
            );
            painter.galley(
                egui::pos2(text_x, rect.top() + pad + name_height + 2.0),
                meta_galley,
                palette.secondary,
            );
            let mut y = rect.top() + pad + row + 4.0;
            if let Some(galley) = caption_galley {
                let caption_height = galley.size().y;
                painter.galley(egui::pos2(rect.left() + pad, y), galley, palette.text);
                y += caption_height;
            }
            let _ = y;
            painter.galley(
                egui::pos2(
                    rect.right() - pad - time.size().x,
                    rect.bottom() - 4.0 - time.size().y,
                ),
                time,
                palette.secondary,
            );

            if scroll {
                hit.scroll_to_me(Some(egui::Align::Center));
            }
            context_menu(&hit, palette, model, message, &mut action);
            pills_row(ui, palette, model, message, rect);
            ui.add_space(4.0);
            return;
        }

        // A voice note: the official player bubble (play/pause, waveform,
        // elapsed/duration). Playback lives in `audio.rs`.
        if message.kind == "audio"
            && let Some(audio) = message.audio.as_ref()
        {
            let width = max_width.min(300.0);
            let height = 58.0;
            let (rect, hit) =
                ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::click());
            let fill = if message.from_me {
                palette.bubble_out
            } else {
                palette.bubble_in
            };
            let radius = egui::CornerRadius {
                nw: 8,
                ne: 8,
                sw: if message.from_me { 8 } else { 2 },
                se: if message.from_me { 2 } else { 8 },
            };
            let painter = ui.painter();
            painter.rect_filled(rect, radius, fill);
            let mine = playback.id.as_deref() == Some(message.id.as_str());
            let playing = mine && playback.playing;
            let elapsed = if mine { playback.elapsed } else { 0.0 };
            let duration = audio.seconds as f32;
            let ratio = if duration > 0.0 {
                (elapsed / duration).clamp(0.0, 1.0)
            } else {
                0.0
            };
            // Play / pause disc.
            let disc = egui::Rect::from_center_size(
                egui::pos2(rect.left() + 30.0, rect.center().y),
                egui::vec2(34.0, 34.0),
            );
            painter.circle_filled(disc.center(), 17.0, palette.accent);
            draw_icon(
                ui,
                disc.shrink(9.0),
                if playing { Icon::Pause } else { Icon::Play },
                egui::Color32::from_rgb(11, 20, 26),
            );
            // Waveform bars (fallback shape when the sender sent none).
            let bars: Vec<u8> = if audio.waveform.is_empty() {
                (0..40)
                    .map(|index| {
                        let seed = message.id.bytes().fold(0u32, |acc, byte| {
                            acc.wrapping_mul(31).wrapping_add(byte as u32)
                        });
                        (((seed >> (index % 8)) & 0x1F) as u8 + 40).min(180)
                    })
                    .collect()
            } else {
                audio.waveform.clone()
            };
            let wave_left = rect.left() + 54.0;
            let wave_right = rect.right() - 10.0;
            let slot = (wave_right - wave_left) / bars.len().max(1) as f32;
            let middle = rect.center().y - 3.0;
            for (index, level) in bars.iter().enumerate() {
                let x = wave_left + slot * index as f32 + slot * 0.25;
                let amplitude = 3.0 + (*level as f32 / 255.0) * 16.0;
                let played = (index as f32 / bars.len().max(1) as f32) <= ratio;
                painter.line_segment(
                    [
                        egui::pos2(x, middle - amplitude),
                        egui::pos2(x, middle + amplitude),
                    ],
                    egui::Stroke::new(
                        (slot * 0.5).clamp(1.0, 2.5),
                        if played {
                            palette.accent
                        } else {
                            palette.secondary
                        },
                    ),
                );
            }
            let label = if mine && elapsed > 0.5 {
                format!("{:.0}s / {}", elapsed, stamp_seconds(duration))
            } else {
                stamp_seconds(duration)
            };
            painter.text(
                egui::pos2(rect.right() - 10.0, rect.bottom() - 8.0),
                egui::Align2::RIGHT_BOTTOM,
                label,
                egui::FontId::proportional(10.5),
                palette.secondary,
            );
            if hit.clicked() {
                action = Some(BubbleAction::PlayAudio(message.id.clone()));
            }
            context_menu(&hit, palette, model, message, &mut action);
            if scroll {
                hit.scroll_to_me(Some(egui::Align::Center));
            }
            ui.add_space(4.0);
            return;
        }

        let body = clean_text(&model.bubble_body(message));
        let fill = if message.from_me {
            palette.bubble_out
        } else {
            palette.bubble_in
        };
        // WhatsApp's tail: only the last bubble of a sender run squares the
        // corner on the sender's side, so a run reads as one block.
        let square = if tail { 2 } else { 8 };
        let radius = egui::CornerRadius {
            nw: 8,
            ne: 8,
            sw: if message.from_me { 8 } else { square },
            se: if message.from_me { square } else { 8 },
        };
        let padding = 9.0;
        let max_text = (max_width - padding * 2.0).max(80.0);
        // Measure the body so the bubble hugs the text (a plain `Frame` lets a
        // right-aligned child stretch it to the full width and height).
        let text_job = match search {
            Some(needle) => {
                let mut job = highlight_job(&body, needle, palette.text);
                job.wrap.max_width = max_text;
                job
            }
            None => egui::text::LayoutJob::simple(
                body.clone(),
                egui::FontId::proportional(14.0),
                palette.text,
                max_text,
            ),
        };
        let text = ui.painter().layout_job(text_job);
        // Emoji are painted as colour pictures (the text font has outlines only,
        // which is why they used to show as boxes); emoji-only messages get the
        // larger cell the official client uses.
        let rich = search.is_none() && (has_emoji(&body) || has_markup(&body));
        let rich_body = rich.then(|| {
            let px = if emoji_only(&body) { 44 } else { 19 };
            layout_body(ui, &body, palette.text, max_text, px)
        });
        let body_size = rich_body
            .as_ref()
            .map(|layout| layout.size)
            .unwrap_or_else(|| text.size());
        let time = ui.painter().layout_no_wrap(
            model.bubble_stamp(message),
            egui::FontId::proportional(11.0),
            palette.secondary,
        );
        // Our own bubbles carry the delivery ticks: one grey check once the
        // server has it, two grey when delivered, two blue when read.
        // A send that never left the socket keeps its bubble but shows a red
        // `!` instead of a tick: the tick would claim the server has it.
        let failed = model.send_error(&message.id).is_some();
        let tick = (message.from_me && !failed).then(|| model.tick_status(message).to_string());
        let tick_width = if tick.is_some() { 16.0 } else { 0.0 };
        // The group sender row: `~ name` for a sender outside the address book,
        // with their number at the right of the row, like the official client.
        let sender = model.sender_row(message);
        let sender_galley = sender.as_ref().map(|(name, _)| {
            ui.painter().layout_no_wrap(
                name.clone(),
                egui::FontId::proportional(12.5),
                sender_color(name.trim_start_matches('~').trim(), palette),
            )
        });
        let sender_number = sender
            .as_ref()
            .and_then(|(_, number)| number.clone())
            .map(|number| {
                ui.painter().layout_no_wrap(
                    number,
                    egui::FontId::proportional(11.0),
                    palette.secondary,
                )
            });
        let number_width = sender_number
            .as_ref()
            .map(|galley| galley.size().x + 14.0)
            .unwrap_or(0.0);
        let sender_width = sender_galley
            .as_ref()
            .map(|galley| galley.size().x + number_width)
            .unwrap_or(0.0);
        let width = (body_size
            .x
            .max(time.size().x + tick_width + 10.0)
            .max(sender_width)
            + padding * 2.0)
            .min(max_width);
        // A quoted reply sits in an inset card above the body, like the
        // official client's coloured quote.
        let quote = message.reply_to.clone();
        let quote_height = if quote.is_some() { 34.0 } else { 0.0 };
        // The sender name row (group chats only) sits above the card.
        let name_height = sender_galley
            .as_ref()
            .map(|galley| galley.size().y + 1.0)
            .unwrap_or(0.0);
        // Two extra pixels: an emoji body is a square cell and used to touch
        // the clock line.
        let height = body_size.y + time.size().y + 10.0 + quote_height + name_height;
        let (rect, hit) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::click());
        let painter = ui.painter();
        painter.rect_filled(rect, radius, fill);
        if search.is_some() {
            painter.rect_stroke(
                rect,
                radius,
                egui::Stroke::new(1.5_f32, egui::Color32::from_rgb(240, 200, 60)),
                egui::StrokeKind::Inside,
            );
        }
        if let Some(quote) = &quote {
            let card_top = rect.top() + 5.0 + name_height;
            let card = egui::Rect::from_min_size(
                egui::pos2(rect.left() + 6.0, card_top),
                egui::vec2((rect.width() - 12.0).max(60.0), 28.0),
            );
            let author = model.quote_author(quote);
            let bar = sender_color(&author, palette);
            let card_fill = if message.from_me {
                egui::Color32::from_black_alpha(28)
            } else {
                palette.rail
            };
            painter.rect_filled(card, egui::CornerRadius::same(6), card_fill);
            painter.rect_filled(
                egui::Rect::from_min_size(card.min, egui::vec2(3.0, card.height())),
                egui::CornerRadius::same(2),
                bar,
            );
            painter.text(
                egui::pos2(card.left() + 9.0, card.top() + 3.0),
                egui::Align2::LEFT_TOP,
                &author,
                egui::FontId::proportional(11.5),
                bar,
            );
            let quoted = Model::quote_line(quote);
            let galley =
                painter.layout_no_wrap(quoted, egui::FontId::proportional(12.0), palette.secondary);
            let clipped = painter.with_clip_rect(card.shrink2(egui::vec2(9.0, 0.0)));
            clipped.galley(
                egui::pos2(card.left() + 9.0, card.top() + 15.0),
                galley,
                palette.secondary,
            );
        }
        // The sender's name comes first, then the quoted card, then the body -
        // the official order, so a name never lands on top of the card.
        let mut y = rect.top() + 5.0;
        if let Some(galley) = &sender_galley {
            // The name is clipped so the number beside it always fits.
            let limit = rect.width() - padding * 2.0 - number_width;
            let clipped = painter.with_clip_rect(egui::Rect::from_min_size(
                egui::pos2(rect.left() + padding, y),
                egui::vec2(limit.max(24.0), galley.size().y),
            ));
            clipped.galley(
                egui::pos2(rect.left() + padding, y),
                galley.clone(),
                palette.text,
            );
            if let Some(number) = &sender_number {
                painter.galley(
                    egui::pos2(rect.right() - padding - number.size().x, y + 1.5),
                    number.clone(),
                    palette.secondary,
                );
            }
            y += name_height;
        }
        if quote.is_some() {
            y += quote_height;
        }
        match &rich_body {
            Some(layout) => paint_body(painter, egui::pos2(rect.left() + padding, y), layout),
            None => {
                painter.galley(egui::pos2(rect.left() + padding, y), text, palette.text);
            }
        }
        painter.galley(
            egui::pos2(
                rect.right() - padding - tick_width - time.size().x,
                rect.bottom() - 4.0 - time.size().y,
            ),
            time,
            palette.secondary,
        );
        if failed {
            let mark = egui::pos2(rect.right() - padding - 7.0, rect.bottom() - 10.0);
            painter.circle_filled(mark, 5.5, palette.danger);
            painter.text(
                mark,
                egui::Align2::CENTER_CENTER,
                "!",
                egui::FontId::proportional(8.5),
                egui::Color32::WHITE,
            );
        }
        if let Some(status) = tick {
            let read = matches!(status.as_str(), "read" | "played");
            let delivered = matches!(status.as_str(), "delivered" | "read" | "played");
            let name = if delivered { "done_all" } else { "check" };
            let color = if read {
                palette.read
            } else {
                palette.secondary
            };
            let tick_rect = egui::Rect::from_min_size(
                egui::pos2(rect.right() - padding - 14.0, rect.bottom() - 17.0),
                egui::vec2(14.0, 14.0),
            );
            if let Some(texture) = crate::icons::texture(ui.ctx(), name, 14, color) {
                painter.image(
                    texture.id(),
                    tick_rect,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    egui::Color32::WHITE,
                );
            }
        }
        if scroll {
            hit.scroll_to_me(Some(egui::Align::Center));
        }
        context_menu(&hit, palette, model, message, &mut action);
        pills_row(ui, palette, model, message, rect);
        ui.add_space(4.0);
    });
    action
}

/// One row of the message menu: icon, label, and a rounded hover highlight.
pub(crate) fn menu_row(
    ui: &mut egui::Ui,
    palette: &crate::theme::Palette,
    icon: Icon,
    label: &str,
) -> bool {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 34.0), egui::Sense::click());
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, egui::CornerRadius::same(6), palette.hover);
    }
    draw_icon(
        ui,
        egui::Rect::from_min_size(
            egui::pos2(rect.left() + 9.0, rect.center().y - 9.0),
            egui::vec2(18.0, 18.0),
        ),
        icon,
        palette.secondary,
    );
    ui.painter().text(
        egui::pos2(rect.left() + 35.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(14.0),
        palette.text,
    );
    response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
}

/// Decode a media file into a texture, once. Absent until the file exists —
/// and absent when it is not a readable image.
/// How wide a decoded texture may be, per kind: the window never paints wider.
pub(crate) const PHOTO_PX: u32 = 720;
pub(crate) const STICKER_PX: u32 = 192;
pub(crate) const QUOTE_PX: u32 = 96;

pub(crate) fn texture_at(
    ctx: &egui::Context,
    cache: &mut std::collections::HashMap<String, Option<egui::TextureHandle>>,
    key: &str,
    path: Option<String>,
    max_width_px: u32,
) -> Option<egui::TextureHandle> {
    if let Some(texture) = cache.get(key) {
        return texture.clone();
    }
    let bytes = std::fs::read(path?).ok()?;
    let mut image = image::load_from_memory(&bytes).ok()?;
    // Big screenshots are common; cap the upload size so decoding and texture
    // creation stay cheap *and* the cache has a predictable footprint.
    if image.width() > max_width_px {
        let scale = max_width_px as f32 / image.width() as f32;
        image = image.resize(
            (image.width() as f32 * scale) as u32,
            (image.height() as f32 * scale) as u32,
            image::imageops::FilterType::Triangle,
        );
    }
    let image = image.to_rgba8();
    let size = [image.width() as usize, image.height() as usize];
    let color = egui::ColorImage::from_rgba_unmultiplied(size, image.as_raw());
    let texture = ctx.load_texture(key.to_string(), color, egui::TextureOptions::LINEAR);
    cache.insert(key.to_string(), Some(texture.clone()));
    let frame = ctx.cumulative_frame_nr();
    ctx.data_mut(|data| data.insert_temp(egui::Id::new(("texture-use", key)), frame));
    Some(texture)
}
