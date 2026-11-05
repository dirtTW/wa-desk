//! The icon vocabulary the window draws itself: the `Icon` enum and its
//! Material Symbols name, the rasterised `draw_icon`, `icon_button` and the
//! forward picker's checkbox.
//!
//! Split out of `app.rs`; `app.rs` re-exports these names so every call site
//! keeps importing them from there.

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Icon {
    Search,
    Menu,
    Status,
    Channels,
    Settings,
    Emoji,
    Sticker,
    Send,
    Reply,
    Forward,
    Copy,
    Edit,
    Delete,
    Star,
    GroupAdd,
    PersonAdd,
    Document,
    Attach,
    Chats,
    Add,
    Groups,
    Logout,
    Play,
    Pause,
    Close,
    Download,
    Volume,
    Mute,
    Fullscreen,
    ChevronLeft,
    ChevronRight,
    Phone,
    CallEnd,
    VideoCall,
    Mic,
    SelectChats,
    MarkRead,
    Lock,
    Check,
    Folder,
    Bell,
    Palette,
    Person,
    Block,
    Shortcut,
    Info,
    Sparkle,
    Mail,
    Pin,
    BellOff,
}

impl Icon {
    /// The Material Symbols name for this icon.
    fn name(self) -> &'static str {
        match self {
            Icon::Search => "search",
            Icon::Menu => "more_vert",
            Icon::Status => "donut_large",
            Icon::Channels => "campaign",
            Icon::Settings => "settings",
            Icon::Emoji => "mood",
            Icon::Sticker => "sticker",
            Icon::Send => "send",
            Icon::Reply => "reply",
            Icon::Forward => "shortcut",
            Icon::Copy => "content_copy",
            Icon::Edit => "edit",
            Icon::Delete => "delete",
            Icon::Star => "star",
            Icon::GroupAdd => "group_add",
            Icon::PersonAdd => "person_add",
            Icon::Document => "description",
            Icon::Attach => "attach_file",
            Icon::Chats => "chat_bubble",
            Icon::Add => "add",
            Icon::Groups => "groups",
            Icon::Logout => "logout",
            Icon::Play => "play_arrow",
            Icon::Pause => "pause",
            Icon::Close => "close",
            Icon::Download => "download",
            Icon::Volume => "volume_up",
            Icon::Mute => "volume_off",
            Icon::Fullscreen => "fullscreen",
            Icon::ChevronLeft => "chevron_left",
            Icon::ChevronRight => "chevron_right",
            Icon::Phone => "call",
            Icon::CallEnd => "call_end",
            Icon::VideoCall => "videocam",
            Icon::Mic => "mic",
            Icon::SelectChats => "done_all",
            Icon::MarkRead => "mark_chat_read",
            Icon::Lock => "lock",
            Icon::Check => "check",
            Icon::Folder => "inventory_2",
            Icon::Bell => "notifications",
            Icon::Palette => "palette",
            Icon::Person => "person",
            Icon::Block => "block",
            Icon::Shortcut => "shortcut",
            Icon::Info => "info",
            Icon::Sparkle => "auto_awesome",
            Icon::Mail => "mail",
            Icon::Pin => "push_pin",
            Icon::BellOff => "notifications_off",
        }
    }
}

/// The forward picker's square checkbox: an outlined box when it is clear, the
/// accent fill with a dark tick when it is set.
pub(crate) fn forward_checkbox(
    ui: &egui::Ui,
    palette: &crate::theme::Palette,
    left: f32,
    center_y: f32,
    checked: bool,
) {
    let box_rect =
        egui::Rect::from_center_size(egui::pos2(left + 10.0, center_y), egui::vec2(20.0, 20.0));
    if checked {
        ui.painter()
            .rect_filled(box_rect, egui::CornerRadius::same(5), palette.accent);
        draw_icon(
            ui,
            box_rect.shrink(3.0),
            Icon::Check,
            egui::Color32::from_rgb(11, 20, 26),
        );
    } else {
        ui.painter().rect_stroke(
            box_rect,
            egui::CornerRadius::same(5),
            egui::Stroke::new(1.5_f32, palette.secondary),
            egui::StrokeKind::Inside,
        );
    }
}

pub(crate) fn icon_button(
    ui: &mut egui::Ui,
    icon: Icon,
    palette: &crate::theme::Palette,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(32.0, 32.0), egui::Sense::click());
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, egui::CornerRadius::same(16), palette.hover);
    }
    let color = if response.hovered() {
        palette.text
    } else {
        palette.secondary
    };
    draw_icon(ui, rect.shrink(6.0), icon, color);
    response
}

/// Paint one Material Symbol inside `rect`; the texture is cached per colour.
pub(crate) fn draw_icon(ui: &egui::Ui, rect: egui::Rect, icon: Icon, color: egui::Color32) {
    let size = rect.width().round().max(8.0) as u32;
    if let Some(texture) = crate::icons::texture(ui.ctx(), icon.name(), size, color) {
        ui.painter().image(
            texture.id(),
            rect,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
    }
}
