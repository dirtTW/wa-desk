//! `wa-ui`: the desktop client's UI logic and HTML renderers.
//!
//! The Tauri shell owns the protocol state and the webview owns the DOM, but the
//! text formatting, labels, icon set, emoji catalogue and list/chat markup live
//! here so they are ordinary Rust: unit-tested, reusable by a CLI or another
//! frontend, and impossible to drift between two implementations.
//!
//! Renderers take a serde request built from the webview's state and return
//! ready-to-inject HTML, which keeps the JavaScript a thin DOM/event shim.

pub mod cache;
pub mod chat;
pub mod composer;
pub mod emoji;
pub mod html;
pub mod icons_data;
pub mod labels;
pub mod menus;
pub mod picker;
pub mod screens;
pub mod settings;
pub mod sidebar;
pub mod state;
pub mod status;
pub mod stickers;
pub mod text;
pub mod viewer;

mod emoji_data;

pub use cache::{CachePayloadInput, FingerChat, RestoreOutput};
pub use composer::{EmojiPanel, StatusPalette};
pub use emoji::{EmojiGroupView, EmojiItem};
pub use html::{avatar, escape, icon, icon_with_class, initials};
pub use labels::{clock_label, day_of, format_followers, stamp_of};
pub use menus::{
    ChatMenuInput, ChatMenuOutput, DeleteConfirmOutput, MessageMenuInput, MessageMenuOutput,
};
pub use picker::{PickerChat, PickerOutput, PickerRequest};
pub use screens::{ChannelsOutput, CommunityOutput, StarredEntry, StarredOutput};
pub use settings::{SettingsOutput, SettingsRequest};
pub use sidebar::{SidebarChat, SidebarOutput, SidebarRequest};
pub use state::{MergeChatView, MergePlan, TrimChatView, TrimPlan};
pub use status::{
    StatusEntryView, StatusFrameOutput, StatusFrameRequest, StatusListOutput, StatusRequest,
    StatusUpdateView, StatusViewsOutput,
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
