//! `wa-ui`: the desktop client's UI logic and HTML renderers.
//!
//! The Tauri shell owns the protocol state and the webview owns the DOM, but the
//! text formatting, labels, icon set, emoji catalogue and list/chat markup live
//! here so they are ordinary Rust: unit-tested, reusable by a CLI or another
//! frontend, and impossible to drift between two implementations.
//!
//! Renderers take a serde request built from the webview's state and return
//! ready-to-inject HTML, which keeps the JavaScript a thin DOM/event shim.

pub mod actions;
pub mod cache;
pub mod calls;
pub mod chat;
pub mod chat_actions;
pub mod composer;
pub mod context;
pub mod emoji;
pub mod export;
pub mod history;
pub mod html;
pub mod icons_data;
pub mod incoming;
pub mod labels;
pub mod media;
pub mod menus;
pub mod picker;
pub mod reactions;
pub mod screens;
pub mod scroll;
pub mod search;
pub mod settings;
pub mod sidebar;
pub mod state;
pub mod status;
pub mod stickers;
pub mod text;
pub mod viewer;

mod emoji_data;

pub use actions::{
    DraftFile, DraftPlan, PostCheck, ReplyPreview, SelectionBar, SendPlan, StatusPermissions,
};
pub use cache::{CachePayloadInput, DiskChatRow, FingerChat, RestoreOutput};
pub use calls::{CallGuard, CallOverlay, CallOverlayInput};
pub use chat_actions::{
    ChatActionInput, ChatActionPlan, DeleteCommand, DeleteTarget, InfoRow, MessageInfoInput,
};
pub use composer::{EmojiPanel, StatusPalette};
pub use context::{ContextLine, ContextMessage, MediaLimits, StickerCandidate};
pub use emoji::{
    EmojiGroupView, EmojiItem, Insertion, PickerTab, RECENTS_CAP, composer_has_text, insert_text,
    picker_tab, remember_recent,
};
pub use export::{ExistingMessage, ExportMessage, ImportPlan};
pub use history::{
    AnchorRequest, HistoryChat, SyncConstants, SyncProgress, sync_allowed, sync_constants,
    sync_progress,
};
pub use html::{avatar, escape, icon, icon_with_class, initials};
pub use incoming::{
    Decision, DuplicatePlan, IncomingView, ReceiptMessage, SentEntry, SentPlan, UnreadRule,
    sent_plan,
};
pub use labels::{clock_label, day_of, format_followers, group_member_count, stamp_of};
pub use media::{DownloadAction, HydrateAction, HydrateInput};
pub use menus::{
    AnchorBox, ChatMenuInput, ChatMenuOutput, ClampedPosition, DeleteConfirmOutput,
    MessageMenuInput, MessageMenuOutput,
};
pub use picker::{PickerChat, PickerOutput, PickerRequest};
pub use reactions::{
    ReactionEventPlan, ReactionRow, ReactionToggle, after_send, apply_event, toggle,
};
pub use screens::{ChannelsOutput, CommunityOutput, StarredEntry, StarredOutput};
pub use scroll::{
    AnchorNode, ScrollAnchor, ScrollEventPlan, anchor_scroll_top, distance_from_bottom,
    is_following, show_latest_button,
};
pub use search::{SearchLabel, count_label, matched_indices};
pub use settings::{AiFormInput, AiPreview, AiSavePlan, AiToggle, SettingsOutput, SettingsRequest};
pub use sidebar::{SidebarChat, SidebarOutput, SidebarRequest};
pub use state::{
    AliasChat, AliasPlan, MergeChatView, MergePlan, SeedContact, SeededChat, TrimChatView, TrimPlan,
};
pub use status::{
    CaptureLimits, DraftPreview, OwnUpdateInput, StatusEntryView, StatusFrameOutput,
    StatusFrameRequest, StatusListOutput, StatusRequest, StatusUpdateView, StatusViewsOutput,
    StepTarget, ViewerChrome, ViewerChromeInput,
};
pub use stickers::{StickerCell, StickerGridOutput, StickerGridRequest};
pub use text::{MentionResolver, MessageRow, format_phone, preview_of};
pub use viewer::{ViewerItem, ViewerOutput, ViewerRequest};

/// Everything the webview needs to boot in one round-trip: the icon set, the
/// emoji catalogue and the picker markup.
#[derive(serde::Serialize)]
pub struct Bootstrap {
    pub icons: Vec<IconView>,
    pub emoji_groups: Vec<EmojiGroupView>,
    pub emoji_count: usize,
    pub emoji_picker: String,
    pub emoji_categories: String,
}

#[derive(serde::Serialize)]
pub struct IconView {
    pub name: String,
    pub path: String,
}

pub fn bootstrap() -> Bootstrap {
    Bootstrap {
        icons: icons_data::ICONS
            .iter()
            .map(|(name, path)| IconView {
                name: (*name).to_string(),
                path: (*path).to_string(),
            })
            .collect(),
        emoji_groups: emoji::groups(),
        emoji_count: emoji::count(),
        emoji_picker: emoji::picker_html(),
        emoji_categories: emoji::categories_html(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bootstrap_ships_every_asset() {
        let boot = bootstrap();
        assert_eq!(boot.icons.len(), icons_data::ICONS.len());
        assert!(boot.emoji_count > 2000);
        assert_eq!(boot.emoji_groups.len(), 9);
        assert!(boot.emoji_picker.contains("emoji-grid"));
        assert!(boot.emoji_categories.contains("emoji-cat"));
    }
}
