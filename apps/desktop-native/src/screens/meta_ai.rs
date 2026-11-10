//! The Meta AI pane: the live rail entry, but honest. Meta AI needs Meta's own
//! service; this client cannot call it, so the pane says so and points at the
//! assistant it does have (Settings -> AI assistant, bring-your-own key).

use crate::app::{Icon, WaDesk, draw_icon, icon_button};
use crate::settings_ui::settings_link;

impl WaDesk {
    pub(crate) fn meta_ai_panel(&mut self, ui: &mut egui::Ui) {
        let palette = self.palette.clone();
        egui::Panel::left("meta-ai")
            .resizable(false)
            .exact_size(420.0)
            .frame(egui::Frame::new().fill(palette.elevated))
            .show(ui, |ui| {
                egui::Frame::new()
                    .inner_margin(egui::Margin::symmetric(12, 10))
                    .show(ui, |ui| {
                        ui.set_min_height(44.0);
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Meta AI").size(26.0).strong());
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if icon_button(ui, Icon::Close, &palette)
                                        .on_hover_text("Close")
                                        .clicked()
                                    {
                                        self.show_meta_ai = false;
                                    }
                                },
                            );
                        });
                    });
                ui.add_space(18.0);
                ui.vertical_centered(|ui| {
                    let (icon, _) =
                        ui.allocate_exact_size(egui::vec2(52.0, 52.0), egui::Sense::hover());
                    draw_icon(ui, icon, Icon::Sparkle, egui::Color32::from_gray(120));
                    ui.add_space(10.0);
                    ui.label(
                        egui::RichText::new("Meta AI is not available here")
                            .size(17.0)
                            .strong()
                            .color(palette.text),
                    );
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new(
                            "It needs Meta's own service, which a linked device cannot drive. \
                             This client ships its own assistant instead: bring an \
                             OpenAI-compatible key and it answers in the chats you allow.",
                        )
                        .size(13.0)
                        .color(palette.secondary),
                    );
                    ui.add_space(14.0);
                    if settings_link(ui, &palette, "Open AI assistant") {
                        self.show_meta_ai = false;
                        self.show_settings = true;
                        self.settings_section = 6;
                    }
                });
            });

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(palette.panel))
            .show(ui, |_ui| {});
    }
}
