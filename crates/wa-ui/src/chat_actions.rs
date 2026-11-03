//! Chat-context-menu action planning, the delete commands for a selection and
//! the message-info rows.

use serde::{Deserialize, Serialize};

use crate::labels::clock_label;
use crate::text::format_phone;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ChatActionInput {
    #[serde(default)]
    pub action: String,
    #[serde(default)]
    pub jid: String,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub archived: bool,
    #[serde(default)]
    pub blocked: bool,
    #[serde(default)]
    pub is_open: bool,
    #[serde(default)]
    pub is_group: bool,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ChatActionPlan {
    /// The Tauri command to call, if any (`chat_pin`, `chat_archive`, …).
    pub command: Option<String>,
    /// Arguments for that command (camelCase keys for the wire).
    pub args: serde_json::Value,
    /// Local state to apply after the command succeeds.
    pub pin: Option<bool>,
    pub archived: Option<bool>,
    pub unread: Option<bool>,
    pub blocked: Option<bool>,
    /// The chat's message list is discarded locally.
    pub clear: bool,
    /// The chat row is hidden (deleted on all devices).
    pub delete: bool,
    /// The panel closes (the entry stays).
    pub close: bool,
    /// A confirmation prompt before running the command.
    pub confirm: Option<String>,
    pub toast: Option<String>,
    /// The sidebar/chat must repaint.
    pub refresh: bool,
}

fn plan(command: Option<&str>, args: serde_json::Value, toast: Option<&str>) -> ChatActionPlan {
    ChatActionPlan {
        command: command.map(str::to_string),
        args,
        toast: toast.map(str::to_string),
        refresh: true,
        ..Default::default()
    }
}

/// What a chat-menu row does: the wire command, the local mutation and the toast.
pub fn plan_chat_action(input: &ChatActionInput) -> ChatActionPlan {
    let jid = serde_json::json!(input.jid);
    match input.action.as_str() {
        "pin" | "unpin" => {
            let pinned = input.action == "pin";
            let mut plan = plan(
                Some("chat_pin"),
                serde_json::json!({ "chat": jid, "pinned": pinned }),
                Some(if pinned {
                    "Chat pinned"
                } else {
                    "Chat unpinned"
                }),
            );
            plan.pin = Some(pinned);
            plan
        }
        "archive" | "unarchive" => {
            let archived = input.action == "archive";
            let mut plan = plan(
                Some("chat_archive"),
                serde_json::json!({ "chat": jid, "archived": archived }),
                Some(if archived {
                    "Chat archived"
                } else {
                    "Chat unarchived"
                }),
            );
            plan.archived = Some(archived);
            plan
        }
        "unread" | "read" => {
            let unread = input.action == "unread";
            let mut plan = plan(
                Some("chat_read_mark"),
                serde_json::json!({ "chat": jid, "unread": unread }),
                if unread {
                    Some("Marked as unread")
                } else {
                    None
                },
            );
            plan.unread = Some(unread);
            plan
        }
        "block" => {
            let blocked = !input.blocked;
            let mut plan = plan(
                Some("chat_block"),
                serde_json::json!({ "chat": jid, "blocked": blocked }),
                Some(if blocked {
                    "Contact blocked"
                } else {
                    "Unblocked contact"
                }),
            );
            plan.blocked = Some(blocked);
            plan
        }
        "close" => {
            let mut plan = plan(None, serde_json::json!({}), None);
            plan.close = input.is_open;
            plan.refresh = input.is_open;
            plan
        }
        "clear" => {
            let mut plan = plan(
                Some("chat_clear"),
                serde_json::json!({ "chat": jid }),
                Some("Chat cleared"),
            );
            plan.confirm = Some("Clear all messages in this chat?".to_string());
            plan.clear = true;
            plan
        }
        "delete" => {
            let mut plan = plan(
                Some("chat_delete"),
                serde_json::json!({ "chat": jid }),
                Some("Chat deleted"),
            );
            plan.confirm = Some("Delete this chat on all linked devices?".to_string());
            plan.delete = true;
            plan
        }
        _ => ChatActionPlan::default(),
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct DeleteTarget {
    #[serde(default)]
    pub chat: String,
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub from_me: bool,
    /// The sender of an incoming group message.
    #[serde(default)]
    pub participant: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeleteCommand {
    /// `delete_for_everyone` or `delete_for_me`.
    pub command: String,
    pub chat: String,
    pub id: String,
    /// Only sent by `delete_for_me`.
    pub from_me: Option<bool>,
    pub participant: Option<String>,
}

/// One command per selected message: "for everyone" ignores the participant,
/// "for me" needs the author of an incoming group message.
pub fn delete_commands(items: &[DeleteTarget], everyone: bool) -> Vec<DeleteCommand> {
    items
        .iter()
        .map(|item| DeleteCommand {
            command: if everyone {
                "delete_for_everyone".to_string()
            } else {
                "delete_for_me".to_string()
            },
            chat: item.chat.clone(),
            id: item.id.clone(),
            from_me: if everyone { None } else { Some(item.from_me) },
            participant: if everyone {
                None
            } else {
                item.participant.clone()
            },
        })
        .collect()
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct MessageInfoInput {
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub timestamp: i64,
    #[serde(default)]
    pub from_me: bool,
    #[serde(default)]
    pub sender_name: String,
    #[serde(default)]
    pub sender: String,
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub pending: bool,
    #[serde(default)]
    pub failed: bool,
    #[serde(default)]
    pub status: String,
}

/// `Sending…`, `Failed`, `Read`, `Delivered` or `Sent`.
pub fn info_status(pending: bool, failed: bool, status: &str) -> &'static str {
    if pending {
        "Sending…"
    } else if failed {
        "Failed"
    } else if status == "read" {
        "Read"
    } else if status == "delivered" {
        "Delivered"
    } else {
        "Sent"
    }
}

/// The From row: "You", the push/saved name, or the formatted number.
pub fn info_from(from_me: bool, sender_name: &str, sender: &str) -> String {
    if from_me {
        return "You".to_string();
    }
    if !sender_name.is_empty() {
        return sender_name.to_string();
    }
    if sender.is_empty() {
        return "Unknown".to_string();
    }
    let phone = format_phone(&crate::text::phone_from_jid(sender));
    if phone.is_empty() {
        "Unknown".to_string()
    } else {
        phone
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct InfoRow {
    pub label: String,
    pub value: String,
}

/// The message-info rows, in the official order.
pub fn message_info(input: &MessageInfoInput) -> Vec<InfoRow> {
    vec![
        InfoRow {
            label: "Type".to_string(),
            value: if input.kind.is_empty() {
                "text".to_string()
            } else {
                input.kind.clone()
            },
        },
        InfoRow {
            label: "Sent".to_string(),
            value: clock_label(input.timestamp),
        },
        InfoRow {
            label: "From".to_string(),
            value: info_from(input.from_me, &input.sender_name, &input.sender),
        },
        InfoRow {
            label: "Message ID".to_string(),
            value: input.id.clone(),
        },
        InfoRow {
            label: "Status".to_string(),
            value: info_status(input.pending, input.failed, &input.status).to_string(),
        },
    ]
}

/// The message-info dialog body.
pub fn message_info_html(rows: &[InfoRow]) -> String {
    rows.iter()
        .map(|row| {
            format!(
                r#"<div><dt>{}</dt><dd>{}</dd></div>"#,
                crate::html::escape(&row.label),
                crate::html::escape(&row.value)
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(action: &str) -> ChatActionInput {
        ChatActionInput {
            action: action.into(),
            jid: "1@s.whatsapp.net".into(),
            ..Default::default()
        }
    }

    #[test]
    fn chat_actions_plan_commands_and_local_state() {
        let pin = plan_chat_action(&input("pin"));
        assert_eq!(pin.command.as_deref(), Some("chat_pin"));
        assert_eq!(pin.pin, Some(true));
        assert_eq!(pin.toast.as_deref(), Some("Chat pinned"));
        assert!(pin.refresh);
        assert!(pin.args["pinned"].as_bool().unwrap());

        let unpin = plan_chat_action(&ChatActionInput {
            pinned: true,
            ..input("unpin")
        });
        assert_eq!(unpin.pin, Some(false));

        let archive = plan_chat_action(&input("archive"));
        assert_eq!(archive.command.as_deref(), Some("chat_archive"));
        assert_eq!(archive.archived, Some(true));

        let unread = plan_chat_action(&input("unread"));
        assert_eq!(unread.unread, Some(true));
        assert_eq!(unread.toast.as_deref(), Some("Marked as unread"));
        let read = plan_chat_action(&input("read"));
        assert_eq!(read.unread, Some(false));
        assert!(read.toast.is_none());
    }

    #[test]
    fn block_is_toggled_against_the_current_state() {
        let blocked = plan_chat_action(&input("block"));
        assert_eq!(blocked.blocked, Some(true));
        assert_eq!(blocked.toast.as_deref(), Some("Contact blocked"));
        let unblocked = plan_chat_action(&ChatActionInput {
            blocked: true,
            ..input("block")
        });
        assert_eq!(unblocked.blocked, Some(false));
        assert_eq!(unblocked.toast.as_deref(), Some("Unblocked contact"));
    }

    #[test]
    fn clear_and_delete_ask_first() {
        let clear = plan_chat_action(&input("clear"));
        assert_eq!(clear.command.as_deref(), Some("chat_clear"));
        assert!(clear.clear && clear.confirm.is_some());
        let delete = plan_chat_action(&input("delete"));
        assert!(delete.delete && delete.confirm.is_some());
        assert_eq!(delete.toast.as_deref(), Some("Chat deleted"));
    }

    #[test]
    fn closing_only_affects_the_open_chat() {
        let closed = plan_chat_action(&ChatActionInput {
            is_open: true,
            ..input("close")
        });
        assert!(closed.command.is_none() && closed.close);
        let other = plan_chat_action(&input("close"));
        assert!(!other.close);
        assert!(!plan_chat_action(&input("unknown")).refresh);
    }

    fn target(chat: &str, id: &str, from_me: bool, participant: Option<&str>) -> DeleteTarget {
        DeleteTarget {
            chat: chat.into(),
            id: id.into(),
            from_me,
            participant: participant.map(str::to_string),
        }
    }

    #[test]
    fn delete_commands_shape_the_args() {
        let items = vec![
            target("1@s.whatsapp.net", "a", true, None),
            target("2@g.us", "b", false, Some("6285@s.whatsapp.net")),
        ];
        let everyone = delete_commands(&items, true);
        assert_eq!(everyone.len(), 2);
        assert_eq!(everyone[0].command, "delete_for_everyone");
        assert!(everyone[0].from_me.is_none());
        assert!(everyone[1].participant.is_none());

        let mine = delete_commands(&items, false);
        assert_eq!(mine[0].command, "delete_for_me");
        assert_eq!(mine[1].from_me, Some(false));
        assert_eq!(mine[1].participant.as_deref(), Some("6285@s.whatsapp.net"));
    }

    #[test]
    fn message_info_rows() {
        let input = MessageInfoInput {
            kind: String::new(),
            timestamp: 1_770_000_000_000,
            from_me: false,
            sender_name: "Budi".into(),
            sender: "628770000000@s.whatsapp.net".into(),
            id: "3EB0".into(),
            pending: false,
            failed: false,
            status: "delivered".into(),
        };
        let rows = message_info(&input);
        assert_eq!(rows.len(), 5);
        assert_eq!(rows[0].value, "text");
        assert_eq!(rows[2].value, "Budi");
        assert_eq!(rows[4].value, "Delivered");
        assert!(message_info_html(&rows).contains("<dt>Message ID</dt><dd>3EB0</dd>"));

        assert_eq!(info_status(true, false, "read"), "Sending…");
        assert_eq!(info_status(false, true, "read"), "Failed");
        assert_eq!(info_status(false, false, "read"), "Read");
        assert_eq!(info_status(false, false, "sent"), "Sent");
        assert_eq!(info_from(true, "", ""), "You");
        assert_eq!(
            info_from(false, "", "628770000000@s.whatsapp.net"),
            "+62 877-0000-000"
        );
        assert_eq!(info_from(false, "", ""), "Unknown");
    }
}
