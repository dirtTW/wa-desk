//! `#[tauri::command]` handlers backed by `wa-ui`.
//!
//! The webview keeps the DOM and the event wiring; every list/sidebar/picker
//! paint asks Rust for the markup. Nothing here touches the network or the
//! session, so these calls are cheap and cannot fail.

use wa_ui::{Bootstrap, SidebarOutput, SidebarRequest};

/// Icons, emoji catalogue and picker markup, fetched once at boot.
#[tauri::command]
pub fn ui_bootstrap() -> Bootstrap {
    wa_ui::bootstrap()
}

/// The chat list: filtering, pin order, community nesting and search sections.
#[tauri::command]
pub fn ui_sidebar(request: SidebarRequest) -> SidebarOutput {
    wa_ui::sidebar::render(&request)
}

/// The message list for the open chat: bubbles, media blocks, quotes,
/// reactions, ticks and channel counters.
#[tauri::command]
pub fn ui_chat(request: wa_ui::chat::ChatRequest) -> wa_ui::chat::ChatOutput {
    wa_ui::chat::render(&request)
}

/// Emoji picker results for a query, as one grid.
#[tauri::command]
pub fn ui_emoji_search(query: String, limit: Option<usize>) -> String {
    wa_ui::emoji::search_html(&query, limit.unwrap_or(240))
}

/// Emoji catalogue, for the picker (and for tests/tools that need the data).
#[tauri::command]
pub fn ui_emoji_groups() -> Vec<wa_ui::EmojiGroupView> {
    wa_ui::emoji::groups()
}

/// The full-screen media viewer chrome (header, caption, stage, filmstrip).
#[tauri::command]
pub fn ui_media_viewer(request: wa_ui::ViewerRequest) -> Option<wa_ui::ViewerOutput> {
    wa_ui::viewer::render(&request)
}

/// The viewer stage for a resolved media URL.
#[tauri::command]
pub fn ui_viewer_stage(url: String, kind: String, text: String, poster: Option<String>) -> String {
    wa_ui::viewer::stage_html(&url, &kind, &text, poster.as_deref())
}

/// One refreshed filmstrip thumbnail.
#[tauri::command]
pub fn ui_viewer_strip_thumb(
    id: String,
    kind: String,
    url: String,
    poster: Option<String>,
    duration: i64,
) -> String {
    wa_ui::viewer::strip_thumb_html(&id, &kind, &url, poster.as_deref(), duration)
}

/// The viewer placeholders.
#[tauri::command]
pub fn ui_viewer_placeholder(kind: String) -> String {
    if kind == "unavailable" {
        wa_ui::viewer::unavailable_html().to_string()
    } else {
        wa_ui::viewer::loading_html().to_string()
    }
}

/// The sticker picker grid.
#[tauri::command]
pub fn ui_sticker_grid(request: wa_ui::StickerGridRequest) -> wa_ui::StickerGridOutput {
    wa_ui::stickers::grid(&request)
}

/// Which rows the chat context menu shows.
#[tauri::command]
pub fn ui_chat_menu(input: wa_ui::ChatMenuInput) -> wa_ui::ChatMenuOutput {
    wa_ui::menus::chat_menu(&input)
}

/// The mute submenu: "Unmute" against the duration choices.
#[tauri::command]
pub fn ui_mute_menu(muted: bool) -> wa_ui::menus::MuteMenuOutput {
    wa_ui::menus::mute_menu(muted)
}

/// Which rows the message context menu shows.
#[tauri::command]
pub fn ui_message_menu(input: wa_ui::MessageMenuInput) -> wa_ui::MessageMenuOutput {
    wa_ui::menus::message_menu(&input)
}

/// The delete confirmation title and whether "for everyone" applies.
#[tauri::command]
pub fn ui_delete_confirm(
    items: Vec<wa_ui::menus::DeleteItem>,
    self_chat: bool,
) -> wa_ui::menus::DeleteConfirmOutput {
    wa_ui::menus::delete_confirm(&items, self_chat)
}

/// The Settings screen: title and body for one page.
#[tauri::command]
pub fn ui_settings(request: wa_ui::SettingsRequest) -> wa_ui::SettingsOutput {
    wa_ui::settings::render(&request)
}

/// The group-info member list.
#[tauri::command]
pub fn ui_group_members(
    members: Vec<wa_ui::screens::GroupMemberRow>,
    contacts: std::collections::HashMap<String, String>,
    photos: std::collections::HashMap<String, String>,
) -> wa_ui::screens::GroupMembersOutput {
    wa_ui::screens::group_members(&members, &contacts, &photos)
}

/// The "New group" / "Add members" contact checklist.
#[tauri::command]
pub fn ui_group_contacts(
    request: wa_ui::picker::GroupContactsRequest,
) -> wa_ui::picker::GroupContactsOutput {
    wa_ui::picker::group_contacts(&request)
}

/// The new-chat / forward picker list.
#[tauri::command]
pub fn ui_picker(request: wa_ui::PickerRequest) -> wa_ui::PickerOutput {
    wa_ui::picker::render(&request)
}

/// The status rail list (recent/viewed sections, my own row).
#[tauri::command]
pub fn ui_status_list(request: wa_ui::StatusRequest) -> wa_ui::StatusListOutput {
    wa_ui::status::list(&request)
}

/// The story viewer shell for one frame.
#[tauri::command]
pub fn ui_status_frame(request: wa_ui::StatusFrameRequest) -> Option<wa_ui::StatusFrameOutput> {
    wa_ui::status::frame(&request)
}

/// The story body for a resolved media URL.
#[tauri::command]
pub fn ui_status_media(url: String, kind: String, name: String) -> String {
    wa_ui::status::media_html(&url, &kind, &name)
}

/// The body shown when story media is gone from the server.
#[tauri::command]
pub fn ui_status_media_error() -> String {
    wa_ui::status::media_error_html()
}

/// The "who viewed my status" list.
#[tauri::command]
pub fn ui_status_views(
    viewers: Vec<wa_ui::status::ViewerRow>,
    request: wa_ui::StatusRequest,
) -> wa_ui::StatusViewsOutput {
    wa_ui::status::views(&viewers, &request)
}

/// The channels rail panel.
#[tauri::command]
pub fn ui_channels(request: SidebarRequest, needle: String) -> wa_ui::ChannelsOutput {
    wa_ui::screens::channels(&request, &needle)
}

/// The community screen for one community JID.
#[tauri::command]
pub fn ui_community(request: SidebarRequest, jid: String) -> wa_ui::CommunityOutput {
    wa_ui::screens::community(&request, &jid)
}

/// The starred-messages screen.
#[tauri::command]
pub fn ui_starred(
    request: SidebarRequest,
    entries: Vec<wa_ui::StarredEntry>,
) -> wa_ui::StarredOutput {
    wa_ui::screens::starred(&entries, &request)
}

/// Clock/stamp/day labels for a batch of timestamps, so the webview never
/// formats a date itself (the official clock is always `8:44 pm`).
#[tauri::command]
pub fn ui_labels(timestamps: Vec<i64>, now: i64) -> Vec<UiLabel> {
    timestamps
        .iter()
        .map(|ms| UiLabel {
            timestamp: *ms,
            clock: wa_ui::labels::clock_label(*ms),
            stamp: wa_ui::labels::stamp_of(*ms, now),
            day: wa_ui::labels::day_of(*ms, now),
        })
        .collect()
}

#[derive(serde::Serialize)]
pub struct UiLabel {
    pub timestamp: i64,
    pub clock: String,
    pub stamp: String,
    pub day: String,
}

/// Phone/detail text for a JID: the header subtitle plus the formatted number.
#[tauri::command]
pub fn ui_chat_subtitle(
    jid: String,
    is_group: bool,
    me_jid: Option<String>,
    me_lid: Option<String>,
    phone: Option<String>,
) -> UiSubtitle {
    UiSubtitle {
        subtitle: wa_ui::text::chat_subtitle(
            &jid,
            is_group,
            me_jid.as_deref().unwrap_or(""),
            me_lid.as_deref().unwrap_or(""),
            phone.as_deref().unwrap_or(""),
        ),
        phone: wa_ui::text::format_phone(&wa_ui::text::phone_from_jid(&jid)),
    }
}

#[derive(serde::Serialize)]
pub struct UiSubtitle {
    pub subtitle: String,
    pub phone: String,
}
