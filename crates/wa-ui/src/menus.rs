//! Which menu rows are usable: the chat context menu, its mute submenu, the
//! message context menu and the delete confirmation. The markup is static in
//! `index.html`; the rules for showing a row live here so they are tested.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ChatMenuInput {
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub mute_until: i64,
    #[serde(default)]
    pub mute_forever: bool,
    #[serde(default)]
    pub now: i64,
    #[serde(default)]
    pub archived: bool,
    #[serde(default)]
    pub unread: i64,
    #[serde(default)]
    pub is_group: bool,
    #[serde(default)]
    pub jid: String,
}

impl ChatMenuInput {
    /// A chat is muted while `muteForever` is set or the expiry is in the future.
    pub fn muted(&self) -> bool {
        self.mute_forever || self.mute_until > self.now
    }
}

#[derive(Debug, Serialize)]
pub struct ChatMenuOutput {
    pub pin: bool,
    pub unpin: bool,
    pub mute: bool,
    pub unmute: bool,
    pub archive: bool,
    pub unarchive: bool,
    pub unread: bool,
    pub read: bool,
    pub block: bool,
}

/// The official chat menu: pin↔unpin, mute↔unmute, mark unread↔read, archive
/// when not archived and block only for one-to-one chats.
pub fn chat_menu(input: &ChatMenuInput) -> ChatMenuOutput {
    let muted = input.muted();
    ChatMenuOutput {
        pin: !input.pinned,
        unpin: input.pinned,
        mute: !muted,
        unmute: muted,
        archive: !input.archived,
        unarchive: input.archived,
        unread: input.unread == 0,
        read: input.unread > 0,
        block: !input.is_group,
    }
}

#[derive(Debug, Serialize)]
pub struct MuteMenuOutput {
    /// "Unmute" replaces the durations while the chat is muted.
    pub unmute: bool,
    pub durations: bool,
}

pub fn mute_menu(input: &ChatMenuInput) -> MuteMenuOutput {
    let muted = input.muted();
    MuteMenuOutput {
        unmute: muted,
        durations: !muted,
    }
}

/// The JIDs that should not notify, for the Rust notification pump.
pub fn muted_chats(chats: &[ChatMenuInput]) -> Vec<String> {
    chats
        .iter()
        .filter(|chat| chat.muted())
        .map(|chat| chat.jid.clone())
        .filter(|jid| !jid.is_empty())
        .collect()
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct MessageMenuInput {
    #[serde(default)]
    pub from_me: bool,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub deleted: bool,
    #[serde(default)]
    pub deleted_for_me: bool,
    #[serde(default)]
    pub text: String,
    /// `sender_name`, `contactNameFor(sender)` or "sender" — resolved already.
    #[serde(default)]
    pub sender_label: String,
    #[serde(default)]
    pub chat_is_group: bool,
    #[serde(default)]
    pub has_sender: bool,
    #[serde(default)]
    pub starred: bool,
}

#[derive(Debug, Serialize)]
pub struct MessageMenuOutput {
    pub edit: bool,
    pub forward: bool,
    pub reply: bool,
    pub copy: bool,
    pub message_sender: bool,
    pub reply_privately: bool,
    pub message_info: bool,
    pub star: bool,
    pub star_label: String,
    pub sender_label: String,
}

/// The message menu rules: edit only our own text, no forward/reply/copy/star
/// for a message that is gone, "Message <sender>" only in a group.
pub fn message_menu(input: &MessageMenuInput) -> MessageMenuOutput {
    let gone = input.deleted || input.deleted_for_me;
    let can_edit = input.from_me && input.kind == "text" && !gone;
    let can_message_sender = input.chat_is_group && input.has_sender && !input.from_me;
    MessageMenuOutput {
        edit: can_edit,
        forward: !gone,
        reply: !gone,
        copy: !gone && !input.text.trim().is_empty(),
        message_sender: can_message_sender,
        reply_privately: can_message_sender,
        message_info: input.from_me,
        star: !gone,
        star_label: if input.starred {
            "Unstar message".to_string()
        } else {
            "Star message".to_string()
        },
        sender_label: if can_message_sender {
            format!("Message {}", input.sender_label)
        } else {
            String::new()
        },
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct DeleteItem {
    #[serde(default)]
    pub from_me: bool,
    #[serde(default)]
    pub deleted: bool,
    #[serde(default)]
    pub chat: String,
}

#[derive(Debug, Serialize)]
pub struct DeleteConfirmOutput {
    pub title: String,
    /// "Delete for everyone" needs every message to be ours and the chat to be
    /// a real conversation (not "Message yourself").
    pub can_everyone: bool,
}

pub fn delete_confirm(items: &[DeleteItem], self_chat: bool) -> DeleteConfirmOutput {
    let count = items.len();
    let can_everyone =
        count > 0 && items.iter().all(|item| item.from_me && !item.deleted) && !self_chat;
    DeleteConfirmOutput {
        title: if count > 1 {
            format!("Delete {count} messages?")
        } else {
            "Delete message?".to_string()
        },
        can_everyone,
    }
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
pub struct ClampedPosition {
    pub left: f64,
    pub top: f64,
}

/// Keep a popover inside the window: 8 px from every edge, scrolling menus may
/// be taller than the viewport (they scroll internally).
pub fn clamp_popover(
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    viewport_w: f64,
    viewport_h: f64,
    margin: f64,
) -> ClampedPosition {
    let left = x.max(margin).min(viewport_w - width - margin);
    let top = y.max(margin).min(viewport_h - height - margin);
    ClampedPosition { left, top }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn popovers_clamp_inside_the_window() {
        // Fits as asked.
        let pos = clamp_popover(100.0, 120.0, 200.0, 150.0, 1000.0, 800.0, 8.0);
        assert_eq!(
            pos,
            ClampedPosition {
                left: 100.0,
                top: 120.0
            }
        );
        // Past the right/bottom edge.
        let pos = clamp_popover(950.0, 780.0, 200.0, 150.0, 1000.0, 800.0, 8.0);
        assert_eq!(
            pos,
            ClampedPosition {
                left: 792.0,
                top: 642.0
            }
        );
        // Negative coordinates and an oversized menu still keep the margin.
        let pos = clamp_popover(-50.0, -20.0, 200.0, 150.0, 1000.0, 800.0, 8.0);
        assert_eq!(
            pos,
            ClampedPosition {
                left: 8.0,
                top: 8.0
            }
        );
        let tall = clamp_popover(10.0, 10.0, 300.0, 2000.0, 1000.0, 800.0, 8.0);
        assert_eq!(
            tall.top, -1208.0,
            "taller menus scroll instead of overflowing"
        );
    }

    #[test]
    fn chat_menu_rows_follow_the_chat_state() {
        let menu = chat_menu(&ChatMenuInput::default());
        assert!(menu.pin && !menu.unpin);
        assert!(menu.mute && !menu.unmute);
        assert!(menu.archive && !menu.unarchive);
        assert!(menu.unread && !menu.read);
        assert!(menu.block);

        let menu = chat_menu(&ChatMenuInput {
            pinned: true,
            mute_forever: true,
            now: 1,
            archived: true,
            unread: 3,
            is_group: true,
            ..Default::default()
        });
        assert!(!menu.pin && menu.unpin);
        assert!(!menu.mute && menu.unmute);
        assert!(!menu.archive && menu.unarchive);
        assert!(!menu.unread && menu.read);
        assert!(!menu.block, "groups cannot be blocked");
    }

    #[test]
    fn mute_submenu_swaps_unmute_for_durations() {
        assert!(
            mute_menu(&ChatMenuInput {
                mute_forever: true,
                ..Default::default()
            })
            .unmute
        );
        assert!(
            !mute_menu(&ChatMenuInput {
                mute_forever: true,
                ..Default::default()
            })
            .durations
        );
        assert!(!mute_menu(&ChatMenuInput::default()).unmute);
        assert!(mute_menu(&ChatMenuInput::default()).durations);
    }

    #[test]
    fn message_menu_rules() {
        let own = message_menu(&MessageMenuInput {
            from_me: true,
            kind: "text".into(),
            text: "hi".into(),
            ..Default::default()
        });
        assert!(own.edit);
        assert!(own.copy);
        assert!(own.message_info);
        assert!(own.star);
        assert_eq!(own.star_label, "Star message");
        assert!(!own.message_sender);

        let media = message_menu(&MessageMenuInput {
            from_me: true,
            kind: "image".into(),
            text: String::new(),
            starred: true,
            ..Default::default()
        });
        assert!(!media.edit, "only text can be edited");
        assert!(!media.copy, "nothing to copy");
        assert_eq!(media.star_label, "Unstar message");

        let incoming = message_menu(&MessageMenuInput {
            chat_is_group: true,
            has_sender: true,
            sender_label: "Budi".into(),
            ..Default::default()
        });
        assert!(!incoming.edit);
        assert!(!incoming.message_info);
        assert!(incoming.message_sender);
        assert!(incoming.reply_privately);
        assert_eq!(incoming.sender_label, "Message Budi");

        let gone = message_menu(&MessageMenuInput {
            deleted: true,
            text: "x".into(),
            ..Default::default()
        });
        assert!(!gone.forward && !gone.reply && !gone.copy && !gone.star);
    }

    #[test]
    fn delete_confirm_counts_and_everyone_rules() {
        let two = vec![
            DeleteItem {
                from_me: true,
                deleted: false,
                chat: "1@s.whatsapp.net".into(),
            },
            DeleteItem {
                from_me: true,
                deleted: false,
                chat: "1@s.whatsapp.net".into(),
            },
        ];
        let out = delete_confirm(&two, false);
        assert_eq!(out.title, "Delete 2 messages?");
        assert!(out.can_everyone);

        assert!(!delete_confirm(&two, true).can_everyone, "self chat");
        let theirs = vec![DeleteItem::default()];
        assert!(!delete_confirm(&theirs, false).can_everyone);
        assert_eq!(delete_confirm(&theirs, false).title, "Delete message?");
    }
}
