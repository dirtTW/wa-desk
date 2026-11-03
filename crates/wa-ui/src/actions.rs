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
}

/// The attachment size limit (48 MB).
pub const MAX_ATTACHMENT_BYTES: u64 = 48 * 1024 * 1024;

/// Validate an attachment before upload and decide how to send it.
pub fn attachment_check(size: u64, mime: &str) -> AttachmentCheck {
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
        };
    }
    AttachmentCheck {
        ok: true,
        error: None,
        kind: kind.to_string(),
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
