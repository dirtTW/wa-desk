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

/// One Material icon as inline SVG (used by the few runtime icon swaps).
#[tauri::command]
pub fn ui_icon(name: String, class: Option<String>) -> String {
    wa_ui::html::icon_with_class(&name, class.as_deref().unwrap_or(""))
}

/// `"/ai explain this"` -> `"explain this"`.
#[tauri::command]
pub fn ui_ai_prompt(text: String) -> Option<String> {
    wa_ui::text::ai_prompt(&text)
}

/// Whether a message is currently visible (eye toggle rules).
#[tauri::command]
pub fn ui_message_visible(deleted_for_me: bool, cleared: bool, show_deleted: bool) -> bool {
    wa_ui::text::MessageRow {
        deleted_for_me,
        cleared,
        ..Default::default()
    }
    .is_visible(show_deleted)
}

/// A phone number formatted the WhatsApp way.
#[tauri::command]
pub fn ui_format_phone(phone: String) -> String {
    wa_ui::text::format_phone(&phone)
}

/// The user part of a JID (no device suffix).
#[tauri::command]
pub fn ui_phone_from_jid(jid: String) -> String {
    wa_ui::text::phone_from_jid(&jid)
}

/// The first web link in a message body.
#[tauri::command]
pub fn ui_first_url(text: String) -> Option<String> {
    wa_ui::text::first_url(&text)
}

/// Whether a display name is still the bare JID user part.
#[tauri::command]
pub fn ui_placeholder_name(jid: String, name: String) -> bool {
    wa_ui::text::is_placeholder_name(&jid, &name)
}

/// What a message event does to an existing chat.
#[tauri::command]
pub fn ui_incoming_classify(
    existing: Option<wa_ui::incoming::ExistingView>,
    message: wa_ui::incoming::IncomingView,
    echo: Option<usize>,
) -> wa_ui::incoming::Decision {
    wa_ui::incoming::classify(existing.as_ref(), &message, echo)
}

/// The optimistic bubble an echoed own message belongs to.
#[tauri::command]
pub fn ui_pending_echo(
    candidates: Vec<wa_ui::incoming::EchoView>,
    message: wa_ui::incoming::IncomingView,
) -> Option<usize> {
    wa_ui::incoming::pending_echo(&candidates, &message)
}

/// Clear vs bump the unread badge for one incoming message.
#[tauri::command]
pub fn ui_unread_rule(active: bool, from_me: bool) -> wa_ui::incoming::UnreadRule {
    wa_ui::incoming::unread_rule(active, from_me)
}

/// Whether a receipt may advance a message's status.
#[tauri::command]
pub fn ui_receipt_status(current: String, incoming: String) -> bool {
    wa_ui::incoming::receipt_should_apply(&current, &incoming)
}

/// The delivery status a sent message starts with.
#[tauri::command]
pub fn ui_self_chat_status(is_self_chat: bool) -> String {
    wa_ui::incoming::self_chat_status(is_self_chat).to_string()
}

/// The newest optimistic bubble index.
#[tauri::command]
pub fn ui_newest_pending(flags: Vec<bool>) -> Option<usize> {
    wa_ui::incoming::newest_pending(&flags)
}

/// Whether an AI reply is already shown as an echo.
#[tauri::command]
pub fn ui_ai_echo(messages: Vec<(bool, String, i64)>, text: String, now: i64) -> bool {
    wa_ui::incoming::is_ai_echo(&messages, &text, now)
}

/// Validate an attachment and classify it (image/video/document).
#[tauri::command]
pub fn ui_attachment_check(size: u64, mime: String) -> wa_ui::actions::AttachmentCheck {
    wa_ui::actions::attachment_check(size, &mime)
}

/// Indices of the rendered bubbles that match a search query.
#[tauri::command]
pub fn ui_search_matches(texts: Vec<String>, query: String) -> Vec<usize> {
    wa_ui::search::matched_indices(&texts, &query)
}

/// The in-chat search counter (`2/5`, `0/0`, empty).
#[tauri::command]
pub fn ui_search_label(index: i64, total: usize, query: String) -> wa_ui::search::SearchLabel {
    wa_ui::search::count_label(index, total, &query)
}

/// Whether a status "view" receipt should be recorded, and the viewer row.
#[tauri::command]
pub fn ui_status_note_view(
    viewers: Vec<wa_ui::status::ViewerRow>,
    jid: String,
    name: String,
    timestamp: i64,
) -> wa_ui::status::NoteView {
    wa_ui::status::note_view(&viewers, &jid, &name, timestamp)
}

/// Which replies become "· Status" cards and which phantom bubbles go away.
#[tauri::command]
pub fn ui_status_heal(
    chats: Vec<wa_ui::status::HealChat>,
    update_ids: Vec<String>,
) -> Vec<wa_ui::status::HealPlan> {
    wa_ui::status::heal_plan(&chats, &update_ids)
}

/// The image-draft stack plan (cap and MIME gating).
#[tauri::command]
pub fn ui_draft_plan(
    existing: usize,
    files: Vec<wa_ui::actions::DraftFile>,
) -> wa_ui::actions::DraftPlan {
    wa_ui::actions::draft_plan(existing, &files)
}

/// The viewer's group queue (newest activity first, updates oldest first).
#[tauri::command]
pub fn ui_status_groups(request: wa_ui::StatusRequest) -> Vec<wa_ui::status::ViewerGroup> {
    wa_ui::status::viewer_groups(&request)
}

/// Ids of our own stories, for view-receipt attribution.
#[tauri::command]
pub fn ui_status_own_ids(request: wa_ui::StatusRequest) -> Vec<String> {
    wa_ui::status::own_update_ids(&request)
}

/// Keep a popover inside the window.
#[tauri::command]
pub fn ui_clamp_popover(
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    viewport_w: f64,
    viewport_h: f64,
    margin: Option<f64>,
) -> wa_ui::menus::ClampedPosition {
    wa_ui::menus::clamp_popover(
        x,
        y,
        width,
        height,
        viewport_w,
        viewport_h,
        margin.unwrap_or(8.0),
    )
}

/// Which call-overlay controls show, plus the status line.
#[tauri::command]
pub fn ui_call_overlay(input: wa_ui::calls::CallOverlayInput) -> CallOverlayOutput {
    let overlay = wa_ui::calls::overlay(&input);
    CallOverlayOutput {
        status: overlay.status(&input),
        ringing: overlay.ringing,
        video_stage: overlay.video_stage,
        self_view: overlay.self_view,
        accept: overlay.accept,
        reject: overlay.reject,
        mute: overlay.mute,
        hangup: overlay.hangup,
        mute_pressed: overlay.mute_pressed,
    }
}

#[derive(serde::Serialize)]
pub struct CallOverlayOutput {
    pub status: String,
    pub ringing: bool,
    pub video_stage: bool,
    pub self_view: bool,
    pub accept: bool,
    pub reject: bool,
    pub mute: bool,
    pub hangup: bool,
    pub mute_pressed: bool,
}

/// Whether a new outgoing call may start.
#[tauri::command]
pub fn ui_call_guard(is_group: bool, has_active_call: bool) -> wa_ui::calls::CallGuard {
    wa_ui::calls::call_guard(is_group, has_active_call)
}

/// The toast when the call media inputs are missing.
#[tauri::command]
pub fn ui_call_media_toast(video: bool) -> String {
    wa_ui::calls::media_unavailable_toast(video).to_string()
}

/// The status composer capture limits.
#[tauri::command]
pub fn ui_status_capture() -> wa_ui::status::CaptureLimits {
    wa_ui::status::capture_limits()
}

/// The duration a posted video story reports.
#[tauri::command]
pub fn ui_status_duration_seconds(duration: f64) -> i64 {
    wa_ui::status::video_duration_seconds(duration)
}

/// Rows recovered from the disk chat-list mirror.
#[tauri::command]
pub fn ui_restore_disk_cache(raw: String) -> Vec<wa_ui::cache::DiskChatRow> {
    wa_ui::cache::restore_disk(&raw)
}

/// Address-book renames to apply to chats and sender bubbles.
#[tauri::command]
pub fn ui_contact_plan(
    updates: Vec<(String, String)>,
    chats: Vec<wa_ui::state::ContactChatView>,
) -> wa_ui::state::ContactPlan {
    wa_ui::state::contact_rename_plan(&updates, &chats)
}

/// The bounded AI transcript for one chat.
#[tauri::command]
pub fn ui_ai_context(
    messages: Vec<wa_ui::context::ContextMessage>,
    limit: usize,
) -> Vec<wa_ui::context::ContextLine> {
    wa_ui::context::ai_context(&messages, limit)
}

/// Recent sticker ids (deduped, newest first, capped).
#[tauri::command]
pub fn ui_recent_stickers(
    candidates: Vec<wa_ui::context::StickerCandidate>,
    limit: usize,
) -> Vec<String> {
    wa_ui::context::recent_stickers(&candidates, limit)
}

/// Sticker ids whose media is cached, for the AI.
#[tauri::command]
pub fn ui_ai_sticker_ids(ids: Vec<String>, ready: Vec<String>, limit: usize) -> Vec<String> {
    wa_ui::context::ai_sticker_ids(&ids, &ready, limit)
}

/// Media hydration limits and poster sampling parameters.
#[tauri::command]
pub fn ui_media_limits() -> wa_ui::context::MediaLimits {
    wa_ui::context::media_limits()
}

/// Where to seek when sampling a video poster.
#[tauri::command]
pub fn ui_poster_seek(duration: f64) -> f64 {
    wa_ui::context::poster_seek_seconds(duration)
}

/// The poster canvas height for a width.
#[tauri::command]
pub fn ui_poster_height(width: u32, video_width: f64, video_height: f64) -> u32 {
    wa_ui::context::poster_height(width, video_width, video_height)
}

/// What the composer's send button does.
#[tauri::command]
pub fn ui_send_plan(
    is_channel: bool,
    has_draft: bool,
    editing: bool,
    text: String,
) -> wa_ui::actions::SendPlan {
    wa_ui::actions::send_plan(is_channel, has_draft, editing, &text)
}

/// The chats to seed from the address book when the cache is empty.
#[tauri::command]
pub fn ui_seed_chats(
    contacts: Vec<wa_ui::state::SeedContact>,
    me_phone: String,
    existing: Vec<String>,
) -> Vec<wa_ui::state::SeededChat> {
    wa_ui::state::seed_plan(&contacts, &me_phone, &existing)
}

/// How peer lookups fold LID rows into phone-number rows.
#[tauri::command]
pub fn ui_alias_plan(
    lookups: Vec<wa_ui::state::AliasLookup>,
    chats: Vec<wa_ui::state::AliasChat>,
) -> wa_ui::state::AliasPlan {
    wa_ui::state::peer_fold_plan(&lookups, &chats)
}

/// What a chat-menu row does.
#[tauri::command]
pub fn ui_chat_action(
    input: wa_ui::chat_actions::ChatActionInput,
) -> wa_ui::chat_actions::ChatActionPlan {
    wa_ui::chat_actions::plan_chat_action(&input)
}

/// One delete command per selected message.
#[tauri::command]
pub fn ui_delete_commands(
    items: Vec<wa_ui::chat_actions::DeleteTarget>,
    everyone: bool,
) -> Vec<wa_ui::chat_actions::DeleteCommand> {
    wa_ui::chat_actions::delete_commands(&items, everyone)
}

/// The message-info rows and their dialog body.
#[tauri::command]
pub fn ui_message_info(input: wa_ui::chat_actions::MessageInfoInput) -> MessageInfoOutput {
    let rows = wa_ui::chat_actions::message_info(&input);
    let html = wa_ui::chat_actions::message_info_html(&rows);
    MessageInfoOutput { rows, html }
}

#[derive(serde::Serialize)]
pub struct MessageInfoOutput {
    pub rows: Vec<wa_ui::chat_actions::InfoRow>,
    pub html: String,
}

/// Whether a story can be replied to / reacted to / deleted.
#[tauri::command]
pub fn ui_status_permissions(from_me: bool) -> wa_ui::actions::StatusPermissions {
    wa_ui::actions::status_permissions(from_me)
}

/// Validate a status draft before posting.
#[tauri::command]
pub fn ui_status_post_check(
    has_file: bool,
    mime: String,
    text: String,
) -> wa_ui::actions::PostCheck {
    wa_ui::actions::status_post_check(has_file, &mime, &text)
}

/// The reply banner's author, preview and colour.
#[tauri::command]
pub fn ui_reply_preview(message: wa_ui::actions::ReplySource) -> wa_ui::actions::ReplyPreview {
    wa_ui::actions::reply_preview(&message)
}

/// The multi-select bar label and enabled state.
#[tauri::command]
pub fn ui_selection_bar(count: usize) -> wa_ui::actions::SelectionBar {
    wa_ui::actions::selection_bar(count)
}

/// The disk mirror of the chat list (metadata only, newest first).
#[tauri::command]
pub fn ui_disk_cache_payload(chats: Vec<wa_ui::cache::DiskChatInput>, limit: usize) -> String {
    wa_ui::cache::disk_payload(&chats, limit)
}

/// Parse a WhatsApp chat export.
#[tauri::command]
pub fn ui_parse_export(text: String, allow_no_sender: bool) -> Vec<wa_ui::export::ExportMessage> {
    wa_ui::export::parse(&text, allow_no_sender)
}

/// Which parsed messages an import adds and which export rows it supersedes.
#[tauri::command]
pub fn ui_import_plan(
    parsed: Vec<wa_ui::export::ExportMessage>,
    existing: Vec<wa_ui::export::ExistingMessage>,
) -> wa_ui::export::ImportPlan {
    wa_ui::export::import_plan(&parsed, &existing)
}

/// The on-demand history anchor for one chat.
#[tauri::command]
pub fn ui_history_anchor(
    chat: wa_ui::history::HistoryChat,
) -> Option<wa_ui::history::AnchorRequest> {
    wa_ui::history::anchor_request(&chat)
}

/// The order chats are re-synced from the phone.
#[tauri::command]
pub fn ui_history_targets(chats: Vec<wa_ui::history::HistoryChat>, limit: usize) -> Vec<String> {
    wa_ui::history::sync_targets(&chats, limit)
}

/// The cache change fingerprint (see `wa_ui::cache`).
#[tauri::command]
pub fn ui_cache_fingerprint(
    chats: Vec<wa_ui::cache::FingerChat>,
    active: String,
    photos: usize,
    starred: usize,
) -> String {
    wa_ui::cache::fingerprint(&chats, &active, photos, starred)
}

/// The JSON text stored under the webview's cache key.
#[tauri::command]
pub fn ui_cache_payload(input: wa_ui::cache::CachePayloadInput) -> String {
    wa_ui::cache::payload(&input)
}

/// Parse a stored cache payload into the restore shape.
#[tauri::command]
pub fn ui_cache_restore(raw: String) -> wa_ui::cache::RestoreOutput {
    wa_ui::cache::restore(&raw)
}

/// The profile-photo candidates worth requesting, deduped and filtered.
#[tauri::command]
pub fn ui_photo_queue(
    candidates: Vec<String>,
    connected: bool,
    have: Vec<String>,
    failed: Vec<String>,
    queued: Vec<String>,
) -> Vec<String> {
    wa_ui::cache::photo_queue(&candidates, connected, &have, &failed, &queued)
}

/// How duplicate conversations should be folded (the webview applies it).
#[tauri::command]
pub fn ui_merge_chats(chats: Vec<wa_ui::state::MergeChatView>) -> wa_ui::state::MergePlan {
    wa_ui::state::merge_plan(&chats)
}

/// Which messages to drop from the chat cache (the webview applies it).
#[tauri::command]
pub fn ui_trim_messages(
    chats: Vec<wa_ui::state::TrimChatView>,
    per_chat: usize,
    budget: usize,
    keep_jid: String,
) -> wa_ui::state::TrimPlan {
    wa_ui::state::trim_plan(&chats, per_chat, budget, &keep_jid)
}

/// The status composer palette, fonts and font wire names.
#[tauri::command]
pub fn ui_status_palette() -> wa_ui::composer::StatusPalette {
    wa_ui::composer::palette()
}

/// The opaque ARGB integer for a status background colour.
#[tauri::command]
pub fn ui_status_argb(hex: String) -> u32 {
    wa_ui::composer::argb(&hex)
}

/// The status composer category strip.
#[tauri::command]
pub fn ui_status_emoji_categories() -> String {
    wa_ui::composer::emoji_categories_html()
}

/// One status emoji category or the search results.
#[tauri::command]
pub fn ui_status_emoji_group(index: usize, query: String) -> wa_ui::composer::EmojiPanel {
    wa_ui::composer::emoji_group_html(index, &query)
}

/// The recent-emoji row for the picker.
#[tauri::command]
pub fn ui_emoji_recents(chars: Vec<String>) -> String {
    wa_ui::emoji::recents_html(&chars)
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
pub fn ui_mute_menu(input: wa_ui::ChatMenuInput) -> wa_ui::menus::MuteMenuOutput {
    wa_ui::menus::mute_menu(&input)
}

/// The JIDs that should not notify.
#[tauri::command]
pub fn ui_muted_chats(chats: Vec<wa_ui::ChatMenuInput>) -> Vec<String> {
    wa_ui::menus::muted_chats(&chats)
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

/// A story frame's duration for its kind and text length.
#[tauri::command]
pub fn ui_status_frame_duration(kind: String, text: String) -> i64 {
    wa_ui::status::frame_duration(&kind, &text)
}

/// The story timing constants (ms) and capture limits.
#[tauri::command]
pub fn ui_status_timing() -> StatusTiming {
    StatusTiming {
        text_ms: wa_ui::status::STATUS_TEXT_MS,
        image_ms: wa_ui::status::STATUS_IMAGE_MS,
        video_fallback_ms: wa_ui::status::STATUS_VIDEO_FALLBACK_MS,
        media_failed_ms: wa_ui::status::STATUS_MEDIA_FAILED_MS,
        thumb_max_px: wa_ui::status::STATUS_THUMB_MAX_PX,
        poster_timeout_ms: wa_ui::status::STATUS_POSTER_TIMEOUT_MS,
    }
}

#[derive(serde::Serialize)]
pub struct StatusTiming {
    pub text_ms: i64,
    pub image_ms: i64,
    pub video_fallback_ms: i64,
    pub media_failed_ms: i64,
    pub thumb_max_px: u32,
    pub poster_timeout_ms: i64,
}

/// Anchor a story frame (total, remaining, end instant).
#[tauri::command]
pub fn ui_status_anchor(total: i64, remaining: i64, now: f64) -> wa_ui::status::FrameAnchor {
    wa_ui::status::anchor(total, remaining, now)
}

/// Elapsed time in a story frame.
#[tauri::command]
pub fn ui_status_elapsed(total: i64, remaining: i64, ends_at: f64, now: f64) -> i64 {
    wa_ui::status::elapsed(total, remaining, ends_at, now)
}

/// Remaining time when a story frame is paused.
#[tauri::command]
pub fn ui_status_pause_remaining(remaining: i64, ends_at: f64, now: f64) -> i64 {
    wa_ui::status::pause_remaining(remaining, ends_at, now)
}

/// Totals when a story frame resumes.
#[tauri::command]
pub fn ui_status_resume(total: i64, remaining: i64) -> (i64, i64) {
    wa_ui::status::resume_totals(total, remaining)
}

/// The progress bar fill (0..1).
#[tauri::command]
pub fn ui_status_progress(total: i64, remaining: i64) -> f64 {
    wa_ui::status::progress_ratio(total, remaining)
}

/// Fit a captured thumbnail into its box.
#[tauri::command]
pub fn ui_status_thumb_scale(
    width: u32,
    height: u32,
    max: Option<u32>,
) -> wa_ui::status::ThumbSize {
    wa_ui::status::thumb_scale(
        width,
        height,
        max.unwrap_or(wa_ui::status::STATUS_THUMB_MAX_PX),
    )
}

/// Where to seek for a video poster frame.
#[tauri::command]
pub fn ui_status_poster_seek(duration: f64) -> f64 {
    wa_ui::status::poster_seek_seconds(duration)
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
