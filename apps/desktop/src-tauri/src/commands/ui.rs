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
