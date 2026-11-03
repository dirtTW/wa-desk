//! Which menu rows are usable: the chat context menu, its mute submenu, the
//! message context menu and the delete confirmation. The markup is static in
//! `index.html`; the rules for showing a row live here so they are tested.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ChatMenuInput {
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub muted: bool,
    #[serde(default)]
    pub archived: bool,
    #[serde(default)]
    pub unread: i64,
    #[serde(default)]
    pub is_group: bool,
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
    ChatMenuOutput {
        pin: !input.pinned,
        unpin: input.pinned,
        mute: !input.muted,
        unmute: input.muted,
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

pub fn mute_menu(muted: bool) -> MuteMenuOutput {
    MuteMenuOutput {
        unmute: muted,
        durations: !muted,
    }
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

#[cfg(test)]
mod tests {
    use super::*;

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
            muted: true,
            archived: true,
            unread: 3,
            is_group: true,
        });
        assert!(!menu.pin && menu.unpin);
        assert!(!menu.mute && menu.unmute);
        assert!(!menu.archive && menu.unarchive);
        assert!(!menu.unread && menu.read);
        assert!(!menu.block, "groups cannot be blocked");
    }

    #[test]
    fn mute_submenu_swaps_unmute_for_durations() {
        assert!(mute_menu(true).unmute);
        assert!(!mute_menu(true).durations);
        assert!(!mute_menu(false).unmute);
        assert!(mute_menu(false).durations);
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
