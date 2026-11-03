//! Small action guards: whether a status can be replied to / reacted to /
//! deleted, status post validation, the composer's reply banner and the
//! multi-select bar.

use serde::{Deserialize, Serialize};

use crate::text::sender_color;

#[derive(Debug, Default, Serialize)]
pub struct StatusPermissions {
    pub can_reply: bool,
    pub can_react: bool,
    pub can_delete: bool,
    /// Why an action is unavailable (shown in a toast), `None` when allowed.
    pub delete_reason: Option<String>,
}

/// Your own story has nobody to reply to, and only your own can be deleted.
pub fn status_permissions(from_me: bool) -> StatusPermissions {
    StatusPermissions {
        can_reply: !from_me,
        can_react: !from_me,
        can_delete: from_me,
        delete_reason: if from_me {
            None
        } else {
            Some("Only your own status can be deleted.".to_string())
        },
    }
}

#[derive(Debug, Default, Serialize)]
pub struct PostCheck {
    pub ok: bool,
    pub error: Option<String>,
    /// `text` or `media`.
    pub mode: String,
    pub caption: String,
    pub is_video: bool,
}

/// Validate a status draft before posting: media drafts need an image/video
/// MIME type, text drafts need text.
pub fn status_post_check(has_file: bool, mime: &str, text: &str) -> PostCheck {
    let caption = text.trim().to_string();
    if has_file {
        let mime = mime.to_lowercase();
        if !(mime.starts_with("image/") || mime.starts_with("video/")) {
            return PostCheck {
                ok: false,
                error: Some("Only photos and videos can be posted as a status.".to_string()),
                mode: "media".to_string(),
                caption,
                is_video: false,
            };
        }
        return PostCheck {
            ok: true,
            error: None,
            mode: "media".to_string(),
            caption,
            is_video: mime.starts_with("video/"),
        };
    }
    if caption.is_empty() {
        return PostCheck {
            ok: false,
            error: None,
            mode: "text".to_string(),
            caption,
            is_video: false,
        };
    }
    PostCheck {
        ok: true,
        error: None,
        mode: "text".to_string(),
        caption,
        is_video: false,
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ReplySource {
    #[serde(default)]
    pub from_me: bool,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub sender: String,
    #[serde(default)]
    pub sender_name: String,
    /// The saved contact name for `sender`, resolved by the webview.
    #[serde(default)]
    pub contact_name: String,
}

#[derive(Debug, Serialize)]
pub struct ReplyPreview {
    pub name: String,
    pub preview: String,
    pub colour: String,
}

fn media_label(kind: &str) -> Option<&'static str> {
    match kind {
        "image" => Some("Photo"),
        "video" => Some("Video"),
        "audio" => Some("Audio"),
        "document" => Some("Document"),
        "sticker" => Some("Sticker"),
        _ => None,
    }
}

/// The reply banner: author, one-line preview and the participant's colour.
pub fn reply_preview(source: &ReplySource) -> ReplyPreview {
    let name = if source.from_me {
        "You".to_string()
    } else if !source.contact_name.is_empty() {
        source.contact_name.clone()
    } else if !source.sender_name.is_empty() {
        source.sender_name.clone()
    } else {
        "Unknown".to_string()
    };
    let text = source.text.trim();
    let preview: String = if !text.is_empty() {
        text.chars().take(140).collect()
    } else {
        media_label(&source.kind).unwrap_or("Message").to_string()
    };
    let colour = if source.sender.is_empty() {
        "var(--accent)".to_string()
    } else {
        sender_color(&source.sender).to_string()
    };
    ReplyPreview {
        name,
        preview,
        colour,
    }
}

#[derive(Debug, Serialize)]
pub struct SelectionBar {
    pub label: String,
    pub disabled: bool,
}

#[derive(Debug, Serialize)]
pub struct AttachmentCheck {
    pub ok: bool,
    pub error: Option<String>,
    /// `image`, `video` or `document`.
    pub kind: String,
    /// The MIME to put on the wire (a blank pick becomes octet-stream).
    pub send_mime: String,
    /// The caption to send (trimmed; `None` when there is none).
    pub caption: Option<String>,
}

/// The attachment size limit (48 MB).
pub const MAX_ATTACHMENT_BYTES: u64 = 48 * 1024 * 1024;

/// Validate an attachment before upload and decide how to send it.
pub fn attachment_check(size: u64, mime: &str) -> AttachmentCheck {
    attachment_send(size, mime, "")
}

/// Validate an attachment and shape the `send_attachment` call: the media kind,
/// the wire MIME and the caption.
pub fn attachment_send(size: u64, mime: &str, caption: &str) -> AttachmentCheck {
    let raw = mime.trim();
    let send_mime = if raw.is_empty() {
        "application/octet-stream".to_string()
    } else {
        raw.to_string()
    };
    let caption = {
        let trimmed = caption.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    };
    let mime = mime.to_lowercase();
    let kind = if mime.starts_with("image/") {
        "image"
    } else if mime.starts_with("video/") {
        "video"
    } else {
        "document"
    };
    if size > MAX_ATTACHMENT_BYTES {
        return AttachmentCheck {
            ok: false,
            error: Some("File is too large - 48 MB is the limit.".to_string()),
            kind: kind.to_string(),
            send_mime,
            caption,
        };
    }
    AttachmentCheck {
        ok: true,
        error: None,
        kind: kind.to_string(),
        send_mime,
        caption,
    }
}

#[derive(Debug, Default, Serialize)]
pub struct SendPlan {
    /// `blocked`, `draft`, `edit`, `send` or `noop`.
    pub action: String,
    /// Set for `blocked` (a toast the UI shows).
    pub toast: Option<String>,
}

/// What the composer's send button does: channels are read-only, a media draft
/// sends the files (the text becomes the caption), an edit needs text, and an
/// empty composer does nothing.
pub fn send_plan(is_channel: bool, has_draft: bool, editing: bool, text: &str) -> SendPlan {
    if is_channel {
        return SendPlan {
            action: "blocked".to_string(),
            toast: Some("Channels are read-only: you can follow them, not post here.".to_string()),
        };
    }
    if has_draft {
        return SendPlan {
            action: "draft".to_string(),
            toast: None,
        };
    }
    if editing {
        return SendPlan {
            action: if text.trim().is_empty() {
                "noop".to_string()
            } else {
                "edit".to_string()
            },
            toast: None,
        };
    }
    SendPlan {
        action: if text.trim().is_empty() {
            "noop".to_string()
        } else {
            "send".to_string()
        },
        toast: None,
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SelectionToggle {
    /// The new selection, in insertion order.
    pub selected: Vec<String>,
    /// The selection became empty, so multi-select mode ends.
    pub exit: bool,
    /// The toggled bubble ended up selected.
    pub checked: bool,
}

/// Add/remove one message id from the multi-select set. Toggling the last one
/// out ends select mode (WhatsApp keeps the bar until you leave it manually with
/// more than one item).
pub fn toggle_selection(selected: &[String], id: &str) -> SelectionToggle {
    if id.is_empty() {
        return SelectionToggle {
            selected: selected.to_vec(),
            exit: false,
            checked: false,
        };
    }
    let mut next: Vec<String> = selected.to_vec();
    let checked = match next.iter().position(|entry| entry == id) {
        Some(index) => {
            next.remove(index);
            false
        }
        None => {
            next.push(id.to_string());
            true
        }
    };
    SelectionToggle {
        exit: next.is_empty(),
        selected: next,
        checked,
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SendItem {
    /// Index into the draft stack.
    pub index: usize,
    /// The caption to send with this file (only the first one carries it).
    pub caption: String,
}

/// Sending a media draft: the composer text becomes the caption of the **first**
/// file only, like WhatsApp's media preview.
pub fn draft_send_plan(count: usize, caption: &str) -> Vec<SendItem> {
    let caption = caption.trim();
    (0..count)
        .map(|index| SendItem {
            index,
            caption: if index == 0 {
                caption.to_string()
            } else {
                String::new()
            },
        })
        .collect()
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct QuoteRef {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub sender_name: String,
    #[serde(default)]
    pub from_me: bool,
    #[serde(default)]
    pub kind: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct OutgoingInput {
    #[serde(default)]
    pub temp_id: String,
    #[serde(default)]
    pub chat: String,
    #[serde(default)]
    pub is_group: bool,
    #[serde(default)]
    pub timestamp: i64,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub quote: Option<QuoteRef>,
}

/// The optimistic bubble for a text send, including the pending reply quote
/// ("You" as the author when we quoted ourselves).
pub fn outgoing_message(input: &OutgoingInput) -> serde_json::Value {
    let reply_to = match input.quote.as_ref() {
        Some(quote) => {
            let sender_name = if !quote.sender_name.is_empty() {
                serde_json::Value::String(quote.sender_name.clone())
            } else if quote.from_me {
                serde_json::Value::String("You".to_string())
            } else {
                serde_json::Value::Null
            };
            serde_json::json!({
                "id": quote.id,
                "text": quote.text,
                "sender_name": sender_name,
                "kind": quote.kind,
            })
        }
        None => serde_json::Value::Null,
    };
    serde_json::json!({
        "id": input.temp_id,
        "chat": input.chat,
        "sender": serde_json::Value::Null,
        "sender_name": serde_json::Value::Null,
        "from_me": true,
        "is_group": input.is_group,
        "timestamp": input.timestamp,
        "kind": "text",
        "text": input.text,
        "pending": true,
        "reply_to": reply_to,
    })
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct AttachmentInput {
    #[serde(default)]
    pub temp_id: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub caption: String,
    #[serde(default)]
    pub file_name: String,
    #[serde(default)]
    pub mime: String,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub is_group: bool,
    #[serde(default)]
    pub timestamp: i64,
}

/// The optimistic bubble for an upload in flight. Documents also carry the file
/// card metadata an incoming document gets (name, MIME, size), so the bubble
/// looks finished while the bytes upload.
pub fn attachment_message(input: &AttachmentInput) -> serde_json::Value {
    let mut message = serde_json::json!({
        "id": input.temp_id,
        "sender": null,
        "sender_name": null,
        "from_me": true,
        "is_group": input.is_group,
        "timestamp": input.timestamp,
        "kind": input.kind,
        "text": input.caption,
        "pending": true,
    });
    if input.kind == "document" {
        message["document"] = serde_json::json!({
            "name": input.file_name,
            "mime": input.mime,
            "size": input.size,
            "pages": 0,
            "thumb": null,
        });
    }
    message
}

#[derive(Debug, Clone, Serialize)]
pub struct AttachmentResult {
    /// The optimistic bubble keeps its place; it only learns the server id.
    pub id: String,
    pub pending: bool,
    pub failed: bool,
    /// The temp id whose media entry moves to `id`.
    pub temp_id: String,
}

/// A completed upload: the bubble swaps its temp id for the server id.
pub fn attachment_result(temp_id: &str, server_id: &str, ok: bool) -> AttachmentResult {
    AttachmentResult {
        id: if ok {
            server_id.to_string()
        } else {
            temp_id.to_string()
        },
        pending: false,
        failed: !ok,
        temp_id: temp_id.to_string(),
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct DraftFile {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub mime: String,
}

#[derive(Debug, Default, Serialize)]
pub struct DraftPlan {
    /// Indices of the offered files that were accepted into the stack.
    pub accepted: Vec<usize>,
    /// True once the stack is full (the picker hides its "more" affordance).
    pub full: bool,
    /// The toast for the first rejected file, if any.
    pub toast: Option<String>,
}

/// The image-draft stack: at most `MAX_DRAFT_FILES` entries, images only.
pub const MAX_DRAFT_FILES: usize = 10;

pub fn draft_plan(existing: usize, files: &[DraftFile]) -> DraftPlan {
    let mut accepted = Vec::new();
    let mut full = existing >= MAX_DRAFT_FILES;
    let mut toast: Option<String> = None;
    for (index, file) in files.iter().enumerate() {
        if full {
            toast.get_or_insert_with(|| {
                format!("You can attach up to {MAX_DRAFT_FILES} images at once.")
            });
            break;
        }
        if !file.mime.to_lowercase().starts_with("image/") {
            toast.get_or_insert_with(|| "Only images can be added to a draft.".to_string());
            continue;
        }
        accepted.push(index);
        if existing + accepted.len() >= MAX_DRAFT_FILES {
            full = true;
        }
    }
    DraftPlan {
        full,
        accepted,
        toast,
    }
}

/// "1 selected" / "N selected" and the action buttons' enabled state.
pub fn selection_bar(count: usize) -> SelectionBar {
    SelectionBar {
        label: if count == 1 {
            "1 selected".to_string()
        } else {
            format!("{count} selected")
        },
        disabled: count == 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn own_stories_can_only_be_deleted() {
        let mine = status_permissions(true);
        assert!(!mine.can_reply && !mine.can_react && mine.can_delete);
        assert!(mine.delete_reason.is_none());

        let theirs = status_permissions(false);
        assert!(theirs.can_reply && theirs.can_react && !theirs.can_delete);
        assert_eq!(
            theirs.delete_reason.as_deref(),
            Some("Only your own status can be deleted.")
        );
    }

    #[test]
    fn status_posts_are_validated() {
        let empty = status_post_check(false, "", "   ");
        assert!(!empty.ok && empty.error.is_none() && empty.mode == "text");

        let text = status_post_check(false, "", "  hello  ");
        assert!(text.ok);
        assert_eq!(text.caption, "hello");
        assert_eq!(text.mode, "text");

        let photo = status_post_check(true, "image/jpeg", " caption ");
        assert!(photo.ok && !photo.is_video);
        assert_eq!(photo.caption, "caption");
        assert_eq!(photo.mode, "media");

        let video = status_post_check(true, "video/mp4", "");
        assert!(video.ok && video.is_video);

        let bad = status_post_check(true, "application/pdf", "x");
        assert!(!bad.ok);
        assert!(bad.error.unwrap().contains("photos and videos"));
    }

    #[test]
    fn reply_previews_prefer_names_and_text() {
        let own = reply_preview(&ReplySource {
            from_me: true,
            kind: "text".into(),
            text: "hi".into(),
            ..Default::default()
        });
        assert_eq!(own.name, "You");
        assert_eq!(own.preview, "hi");

        let saved = reply_preview(&ReplySource {
            sender: "6285@s.whatsapp.net".into(),
            sender_name: "push".into(),
            contact_name: "Saved".into(),
            kind: "image".into(),
            ..Default::default()
        });
        assert_eq!(saved.name, "Saved");
        assert_eq!(saved.preview, "Photo");
        assert_eq!(saved.colour, sender_color("6285@s.whatsapp.net"));

        let unknown = reply_preview(&ReplySource {
            kind: "other".into(),
            ..Default::default()
        });
        assert_eq!(unknown.name, "Unknown");
        assert_eq!(unknown.preview, "Message");
        assert_eq!(unknown.colour, "var(--accent)");

        let long = reply_preview(&ReplySource {
            text: "x".repeat(300),
            ..Default::default()
        });
        assert_eq!(long.preview.chars().count(), 140);
    }

    #[test]
    fn attachments_are_validated_and_classified() {
        let photo = attachment_check(1024, "image/png");
        assert!(photo.ok);
        assert_eq!(photo.kind, "image");

        let clip = attachment_check(MAX_ATTACHMENT_BYTES, "video/mp4");
        assert!(clip.ok);
        assert_eq!(clip.kind, "video");

        let doc = attachment_check(10, "application/pdf");
        assert!(doc.ok && doc.kind == "document");

        let huge = attachment_check(MAX_ATTACHMENT_BYTES + 1, "image/png");
        assert!(!huge.ok);
        assert_eq!(
            huge.error.as_deref(),
            Some("File is too large - 48 MB is the limit.")
        );
        assert_eq!(huge.kind, "image");
    }

    #[test]
    fn the_send_shapes_the_wire_mime_and_caption() {
        // A blank pick type becomes octet-stream, the caption is trimmed.
        let plan = attachment_send(10, "", "  hello  ");
        assert!(plan.ok);
        assert_eq!(plan.send_mime, "application/octet-stream");
        assert_eq!(plan.caption.as_deref(), Some("hello"));
        assert_eq!(plan.kind, "document", "an unknown type is a document");

        // A real type is passed through untouched (case preserved).
        let image = attachment_send(10, "image/PNG", "");
        assert_eq!(image.send_mime, "image/PNG");
        assert_eq!(image.kind, "image", "the kind comparison is lowercased");
        assert!(image.caption.is_none(), "a blank caption sends none");

        // The caption survives on a rejected file so the toast path can show it.
        let huge = attachment_send(MAX_ATTACHMENT_BYTES + 1, "image/png", "caption");
        assert!(!huge.ok);
        assert_eq!(huge.caption.as_deref(), Some("caption"));
    }

    #[test]
    fn selection_toggles_and_ends_when_empty() {
        let first = toggle_selection(&[], "a");
        assert_eq!(first.selected, vec!["a".to_string()]);
        assert!(first.checked);
        assert!(!first.exit);

        let second = toggle_selection(&first.selected, "b");
        assert_eq!(second.selected, vec!["a".to_string(), "b".to_string()]);
        assert!(second.checked && !second.exit);

        let removed = toggle_selection(&second.selected, "a");
        assert_eq!(removed.selected, vec!["b".to_string()]);
        assert!(!removed.checked && !removed.exit);

        let last = toggle_selection(&removed.selected, "b");
        assert!(last.selected.is_empty());
        assert!(last.exit, "toggling the last item out ends select mode");

        let noop = toggle_selection(&["a".to_string()], "");
        assert_eq!(noop.selected, vec!["a".to_string()]);
        assert!(!noop.exit && !noop.checked);
    }

    #[test]
    fn draft_sends_only_the_first_caption() {
        let plan = draft_send_plan(3, "  hello  ");
        assert_eq!(plan.len(), 3);
        assert_eq!(plan[0].caption, "hello");
        assert!(plan[1].caption.is_empty() && plan[2].caption.is_empty());
        assert!(draft_send_plan(0, "x").is_empty());
    }

    #[test]
    fn outgoing_messages_carry_the_reply_quote() {
        let plain = outgoing_message(&OutgoingInput {
            temp_id: "local-1".into(),
            chat: "1@s.whatsapp.net".into(),
            is_group: false,
            timestamp: 42,
            text: "hello".into(),
            quote: None,
        });
        assert_eq!(plain["id"], "local-1");
        assert_eq!(plain["kind"], "text");
        assert_eq!(plain["text"], "hello");
        assert_eq!(plain["pending"], true);
        assert!(plain["reply_to"].is_null());
        assert!(plain["sender"].is_null());

        let quoted = outgoing_message(&OutgoingInput {
            temp_id: "local-2".into(),
            chat: "1@g.us".into(),
            is_group: true,
            timestamp: 43,
            text: "answer".into(),
            quote: Some(QuoteRef {
                id: "q1".into(),
                text: "the question".into(),
                sender_name: String::new(),
                from_me: true,
                kind: "text".into(),
            }),
        });
        assert_eq!(quoted["reply_to"]["id"], "q1");
        assert_eq!(quoted["reply_to"]["sender_name"], "You");
        assert_eq!(quoted["is_group"], true);

        let named = outgoing_message(&OutgoingInput {
            temp_id: "local-3".into(),
            chat: "1@g.us".into(),
            is_group: false,
            timestamp: 44,
            text: "x".into(),
            quote: Some(QuoteRef {
                id: "q2".into(),
                sender_name: "Budi".into(),
                from_me: false,
                kind: "image".into(),
                ..Default::default()
            }),
        });
        assert_eq!(named["reply_to"]["sender_name"], "Budi");
        assert_eq!(named["reply_to"]["kind"], "image");
    }

    #[test]
    fn attachment_bubbles_carry_document_metadata() {
        let photo = attachment_message(&AttachmentInput {
            temp_id: "local-1".into(),
            kind: "image".into(),
            caption: "cap".into(),
            file_name: "a.png".into(),
            mime: "image/png".into(),
            size: 10,
            is_group: false,
            timestamp: 42,
        });
        assert_eq!(photo["id"], "local-1");
        assert_eq!(photo["kind"], "image");
        assert_eq!(photo["text"], "cap");
        assert_eq!(photo["pending"], true);
        assert!(photo.get("document").is_none());

        let doc = attachment_message(&AttachmentInput {
            kind: "document".into(),
            file_name: "a.pdf".into(),
            mime: "application/pdf".into(),
            size: 2048,
            ..Default::default()
        });
        assert_eq!(doc["document"]["name"], "a.pdf");
        assert_eq!(doc["document"]["size"], 2048);
        assert_eq!(doc["document"]["pages"], 0);
        assert!(doc["document"]["thumb"].is_null());
    }

    #[test]
    fn upload_results_swap_the_temp_id() {
        let ok = attachment_result("local-1", "3EB0", true);
        assert_eq!(ok.id, "3EB0");
        assert!(!ok.pending && !ok.failed);
        assert_eq!(ok.temp_id, "local-1");

        let failed = attachment_result("local-1", "3EB0", false);
        assert_eq!(failed.id, "local-1", "a failed upload keeps its temp id");
        assert!(failed.failed && !failed.pending);
    }

    #[test]
    fn draft_stack_caps_and_filters_images() {
        let files = vec![
            DraftFile {
                name: "a.png".into(),
                mime: "image/png".into(),
            },
            DraftFile {
                name: "b.pdf".into(),
                mime: "application/pdf".into(),
            },
            DraftFile {
                name: "c.jpg".into(),
                mime: "image/jpeg".into(),
            },
        ];
        let plan = draft_plan(0, &files);
        assert_eq!(plan.accepted, vec![0, 2]);
        assert_eq!(
            plan.toast.as_deref(),
            Some("Only images can be added to a draft.")
        );
        assert!(!plan.full);

        // Nine already stacked: only one more fits, then the cap toast fires.
        let plan = draft_plan(MAX_DRAFT_FILES - 1, &files);
        assert_eq!(plan.accepted, vec![0]);
        assert!(plan.full);
        assert_eq!(
            plan.toast.as_deref(),
            Some("You can attach up to 10 images at once."),
            "the rest of the batch is refused at the cap"
        );

        // Full stack rejects everything with the cap toast.
        let plan = draft_plan(MAX_DRAFT_FILES, &files);
        assert!(plan.accepted.is_empty());
        assert!(plan.full);
        assert_eq!(
            plan.toast.as_deref(),
            Some("You can attach up to 10 images at once.")
        );
    }

    #[test]
    fn send_plan_covers_the_composer_states() {
        let channel = send_plan(true, false, false, "hi");
        assert_eq!(channel.action, "blocked");
        assert!(channel.toast.unwrap().contains("read-only"));

        assert_eq!(send_plan(false, true, false, "").action, "draft");
        assert_eq!(send_plan(false, false, true, "x").action, "edit");
        assert_eq!(send_plan(false, false, true, "   ").action, "noop");
        assert_eq!(send_plan(false, false, false, "  ").action, "noop");
        assert_eq!(send_plan(false, false, false, "hello").action, "send");
    }

    #[test]
    fn selection_bar_labels() {
        assert_eq!(selection_bar(0).label, "0 selected");
        assert!(selection_bar(0).disabled);
        assert_eq!(selection_bar(1).label, "1 selected");
        assert_eq!(selection_bar(3).label, "3 selected");
        assert!(!selection_bar(3).disabled);
    }
}
