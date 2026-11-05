//! The left side: the 58 px rail with its badges, the Chats panel with the
//! search box, the filter pills (plus the custom lists) and the rows.

use crate::app::{Icon, WaDesk, draw_icon, fab_button, icon_button, pill_button, rail_button};
use crate::model::ChatRow;
use crate::model::Command;
use crate::updates::UpdateState;
use crate::widgets::{avatar_color, fit_text, initials, paint_avatar, single_line};

impl WaDesk {
    /// The rail plus the Chats panel: search, filters, list.
    pub(crate) fn sidebar(&mut self, ctx: &egui::Context) {
        let palette = self.palette.clone();

        // The navigation rail, like the official client.
        egui::SidePanel::left("rail")
            .resizable(false)
            .exact_width(58.0)
            .frame(egui::Frame::new().fill(palette.rail))
            .show(ctx, |ui| {
                ui.add_space(10.0);
                ui.vertical_centered(|ui| {
                    let chats = !self.show_status && !self.show_channels && !self.show_settings;
                    let community_view =
                        chats && self.model.chat_filter == crate::model::ChatFilter::Communities;
                    if rail_button(ui, &palette, Icon::Chats, chats, self.model.total_unread())
                        .on_hover_text("Chats")
                        .clicked()
                    {
                        self.show_status = false;
                        self.show_channels = false;
                        self.show_settings = false;
                        self.model.close_status();
                    }
                    let unseen = self
                        .model
                        .status_groups()
                        .iter()
                        .filter(|group| group.unseen)
                        .count() as i64;
                    if rail_button(ui, &palette, Icon::Phone, self.show_calls, 0)
                        .on_hover_text("Calls")
                        .clicked()
                    {
                        self.show_calls = true;
                        self.show_status = false;
                        self.show_channels = false;
                        self.show_settings = false;
                        self.model.close_status();
                    }
                    if rail_button(ui, &palette, Icon::Status, self.show_status, unseen)
                        .on_hover_text("Status")
                        .clicked()
                    {
                        self.show_status = true;
                        self.show_channels = false;
                        self.show_settings = false;
                        self.model.close_status();
                        self.send(Command::StatusHistory);
                    }
                    if rail_button(ui, &palette, Icon::Channels, self.show_channels, 0)
                        .on_hover_text("Channels")
                        .clicked()
                    {
                        self.show_channels = true;
                        self.show_status = false;
                        self.show_settings = false;
                        self.model.close_status();
                    }
                    if rail_button(ui, &palette, Icon::Groups, community_view, 0)
                        .on_hover_text("Communities")
                        .clicked()
                    {
                        self.show_channels = false;
                        self.show_status = false;
                        self.show_settings = false;
                        self.show_calls = false;
                        self.show_communities = true;
                    }
                    if rail_button(ui, &palette, Icon::Settings, self.show_settings, 0)
                        .on_hover_text("Settings")
                        .clicked()
                    {
                        self.show_settings = true;
                        self.show_channels = false;
                        self.show_status = false;
                        self.model.close_status();
                    }
                });
                ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                    ui.add_space(10.0);
                    self.ensure_avatar(ui.ctx(), &self.model.me_jid.clone());
                    let own_avatar = self.avatar_for(&self.model.me_jid.clone()).cloned();
                    paint_avatar(ui, &self.model.own_name(), 30.0, own_avatar.as_ref());
                });
            });

        // Settings, Status and Channels replace the chat list, like the official
        // client: their own pane takes this column instead of stacking next to it.
        if self.show_settings
            || self.show_status
            || self.show_channels
            || self.show_calls
            || self.show_communities
        {
            return;
        }

        egui::SidePanel::left("chats")
            .resizable(true)
            .default_width(368.0)
            .show(ctx, |ui| {
                let palette = self.palette.clone();
                // The official list header is a 64 px bar in the rail colour
                // with the avatar and title inset by 12 px (measured on
                // web.whatsapp.com: header 64 px tall, padding 10/12).
                egui::Frame::new()
                    .fill(palette.elevated)
                    .inner_margin(egui::Margin::symmetric(12, 10))
                    .show(ui, |ui| {
                        ui.set_min_height(44.0);
                        ui.horizontal(|ui| {
                            // The desktop header carries no avatar (it lives at
                            // the bottom of the rail) and a larger title.
                            ui.label(egui::RichText::new("Chats").size(26.0).strong());
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if fab_button(ui, &palette).on_hover_text("New chat").clicked()
                                    {
                                        self.show_new_chat = true;
                                        self.send(Command::Contacts);
                                    }
                                    if icon_button(ui, Icon::Menu, &palette)
                                        .on_hover_text("Menu")
                                        .clicked()
                                    {
                                        // The sidebar kebab opens the Chats menu - not the
                                        // open chat's menu (that one lives in the header).
                                        self.show_chat_actions = false;
                                        self.show_chat_menu = !self.show_chat_menu;
                                    }
                                    if matches!(
                                        self.update,
                                        UpdateState::Available(_) | UpdateState::Ready(_)
                                    ) {
                                        ui.label(
                                            egui::RichText::new("⬆")
                                                .color(palette.accent)
                                                .size(16.0),
                                        )
                                        .on_hover_text(
                                            "An update is ready - see Settings → Updates",
                                        );
                                    }
                                },
                            );
                        });
                    });
                ui.add_space(8.0);
                // The rounded search field.
                egui::Frame::new()
                    .fill(palette.raised)
                    .corner_radius(egui::CornerRadius::same(9))
                    .inner_margin(egui::Margin::symmetric(10, 4))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let (rect, _) = ui
                                .allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::hover());
                            draw_icon(ui, rect, Icon::Search, palette.secondary);
                            let edit = ui.add(
                                egui::TextEdit::singleline(&mut self.model.search)
                                    .hint_text("Search or start a new chat")
                                    .frame(false)
                                    .desired_width(f32::INFINITY),
                            );
                            if self.focus_search {
                                edit.request_focus();
                                self.focus_search = false;
                            }
                        });
                    });
                ui.add_space(6.0);
                // The filter pills with their counts.
                let (groups, communities, unread, favourites) = self.model.filter_counts();
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    // The desktop client's order: All, Groups, Communities,
                    // Unread, then a caret for the rest (Favorites).
                    for (filter, label) in [
                        (crate::model::ChatFilter::All, "All".to_string()),
                        (crate::model::ChatFilter::Groups, format!("Groups {groups}")),
                        (
                            crate::model::ChatFilter::Communities,
                            format!("Communities {communities}"),
                        ),
                        (crate::model::ChatFilter::Unread, format!("Unread {unread}")),
                    ] {
                        let selected = self.model.chat_filter == filter;
                        if pill_button(ui, &palette, &label, selected) {
                            self.model.chat_filter = filter;
                        }
                    }
                    // Favorites hides behind the caret, as on the desktop.
                    if pill_button(ui, &palette, &format!("v {favourites}"), false) {
                        self.model.chat_filter = crate::model::ChatFilter::Favourites;
                    }
                    // Custom lists land here as their own pills, after the
                    // official "+ New list" control.
                    for list in self.model.lists.clone() {
                        let selected =
                            self.model.chat_filter == crate::model::ChatFilter::List(list.id);
                        if pill_button(ui, &palette, &list.name, selected) {
                            self.model.chat_filter = crate::model::ChatFilter::List(list.id);
                        }
                    }
                    if pill_button(ui, &palette, "+ New list", false) {
                        self.list_draft = Some(String::new());
                    }
                });
                ui.add_space(2.0);
                // The link line only shows when it is not simply "Connected":
                // the official list has no status row there.
                if !self.model.status.eq_ignore_ascii_case("connected") {
                    ui.label(
                        egui::RichText::new(&self.model.status)
                            .size(11.5)
                            .color(palette.secondary),
                    );
                }
                ui.add_space(2.0);

                let rows: Vec<ChatRow> = self.model.visible_chats().into_iter().cloned().collect();

                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    // Always reserve the bar: when it appeared mid-scroll the
                    // rows shifted a few pixels sideways, which flickers.
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
                    .show(ui, |ui| {
                        for row in &rows {
                            let selected = self.model.active.as_deref() == Some(row.jid.as_str());
                            let title = self.model.row_title(row);
                            let preview = self.model.row_preview(row);
                            let stamp = self.model.row_stamp(row);
                            if row.jid.ends_with("@g.us")
                                && row.name.is_empty()
                                && self.groups_asked.insert(row.jid.clone())
                            {
                                self.send(Command::GroupInfo {
                                    chat: row.jid.clone(),
                                });
                            }
                            self.ensure_avatar(ui.ctx(), &row.jid);
                            let avatar = self.avatar_for(&row.jid).cloned();
                            if chat_row(
                                ui,
                                &palette,
                                row,
                                &title,
                                &preview,
                                &stamp,
                                selected,
                                avatar.as_ref(),
                            ) {
                                let jid = row.jid.clone();
                                let empty = row.messages.is_empty();
                                self.audio.stop();
                                crate::gif::clear();
                                crate::webp_anim::clear();
                                self.model.open_chat(&jid);
                                if jid.ends_with("@g.us") && self.model.request_group(&jid) {
                                    self.send(Command::GroupInfo { chat: jid.clone() });
                                }
                                self.send(Command::MarkRead { chat: jid.clone() });
                                if empty {
                                    // WhatsApp does not replay history to an
                                    // existing device; opening a chat asks.
                                    self.send(Command::ChatHistory { chat: jid });
                                }
                            }
                        }
                    });
            });
    }
}

#[allow(clippy::too_many_arguments)]
fn chat_row(
    ui: &mut egui::Ui,
    palette: &crate::theme::Palette,
    row: &ChatRow,
    title: &str,
    preview: &str,
    stamp: &str,
    selected: bool,
    texture: Option<&egui::TextureHandle>,
) -> bool {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 72.0), egui::Sense::click());
    let painter = ui.painter();
    // Official rows are rounded 12 px cards inset from the panel edge.
    let card = rect.shrink2(egui::vec2(6.0, 0.0));
    if selected {
        painter.rect_filled(card, egui::CornerRadius::same(12), palette.selected);
    } else if response.hovered() {
        painter.rect_filled(card, egui::CornerRadius::same(12), palette.hover);
    }

    let center = egui::pos2(rect.left() + 40.0, rect.center().y);
    // The `~` mark and a number are for the text only: the disc keeps the
    // person's initials.
    let avatar_name = title.trim_start_matches('~').trim();
    if let Some(texture) = texture {
        painter.image(
            texture.id(),
            egui::Rect::from_center_size(center, egui::vec2(48.0, 48.0)),
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
    } else {
        painter.circle_filled(center, 24.0, avatar_color(avatar_name));
        painter.text(
            center,
            egui::Align2::CENTER_CENTER,
            initials(avatar_name),
            egui::FontId::proportional(15.0),
            egui::Color32::WHITE,
        );
    }

    let left = rect.left() + 76.0;
    let right = rect.right() - 12.0;
    // Reserve the clock's width so a long name cannot run underneath it.
    let stamp_width = if stamp.is_empty() {
        0.0
    } else {
        painter
            .layout_no_wrap(
                stamp.to_string(),
                egui::FontId::proportional(11.5),
                palette.secondary,
            )
            .size()
            .x
            + 10.0
    };
    painter.text(
        egui::pos2(left, rect.top() + 22.0),
        egui::Align2::LEFT_CENTER,
        fit_text(
            painter,
            &single_line(title),
            egui::FontId::proportional(15.0),
            palette.text,
            right - left - stamp_width,
        ),
        egui::FontId::proportional(15.0),
        palette.text,
    );
    if !stamp.is_empty() {
        let color = if row.unread > 0 {
            palette.accent
        } else {
            palette.secondary
        };
        painter.text(
            egui::pos2(right, rect.top() + 22.0),
            egui::Align2::RIGHT_CENTER,
            stamp,
            egui::FontId::proportional(11.5),
            color,
        );
    }
    painter.text(
        egui::pos2(left, rect.top() + 47.0),
        egui::Align2::LEFT_CENTER,
        fit_text(
            painter,
            &single_line(preview),
            egui::FontId::proportional(13.0),
            palette.secondary,
            right - left,
        ),
        egui::FontId::proportional(13.0),
        palette.secondary,
    );
    if row.unread > 0 {
        let pill = egui::Rect::from_center_size(
            egui::pos2(right - 4.0, rect.bottom() - 16.0),
            egui::vec2(20.0, 18.0),
        );
        painter.rect_filled(pill, 9.0, palette.unread);
        painter.text(
            pill.center(),
            egui::Align2::CENTER_CENTER,
            row.unread.to_string(),
            egui::FontId::proportional(11.0),
            egui::Color32::WHITE,
        );
    }
    response.clicked()
}
