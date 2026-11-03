//! `wa-ui`: the desktop client's UI logic and HTML renderers.
//!
//! The Tauri shell owns the protocol state and the webview owns the DOM, but the
//! text formatting, labels, icon set, emoji catalogue and list/chat markup live
//! here so they are ordinary Rust: unit-tested, reusable by a CLI or another
//! frontend, and impossible to drift between two implementations.
//!
//! Renderers take a serde request built from the webview's state and return
//! ready-to-inject HTML, which keeps the JavaScript a thin DOM/event shim.

pub mod chat;
pub mod emoji;
pub mod html;
pub mod icons_data;
pub mod labels;
pub mod screens;
pub mod sidebar;
pub mod text;

mod emoji_data;

pub use emoji::{EmojiGroupView, EmojiItem};
pub use html::{avatar, escape, icon, icon_with_class, initials};
pub use labels::{clock_label, day_of, format_followers, stamp_of};
pub use screens::{ChannelsOutput, CommunityOutput, StarredEntry, StarredOutput};
pub use sidebar::{SidebarChat, SidebarOutput, SidebarRequest};
pub use text::{MentionResolver, MessageRow, format_phone, preview_of};

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
