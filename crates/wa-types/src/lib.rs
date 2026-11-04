//! Serializable data types shared by `wa-core` and the desktop shell.
//!
//! Keeping these in their own crate lets Cargo compile the protocol-independent
//! vocabulary in parallel with the session layer and keeps `wa-core/src/lib.rs`
//! focused on event translation.

use serde::{Deserialize, Serialize};

/// Events pushed from the protocol layer towards the frontend.
///
/// Serialized as `{ "type": "message", ... }`, so the webview can switch on
/// `payload.type`.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UiEvent {
    /// A pairing QR code, already rendered to SVG so the UI needs no QR lib.
    Qr { svg: String, timeout_secs: u64 },
    /// The QR rotation budget is spent: no new code will rotate in, so the login
    /// screen must offer to restart the pairing session.
    QrExpired,
    /// An 8 character pairing code the user types into their phone.
    PairCode { code: String },
    /// Linking finished; `jid` is the account's own WhatsApp id.
    Paired { jid: String, name: String },
    /// Session authenticated and syncing.
    Connected,
    /// Transport dropped; the library reconnects on its own.
    Disconnected { reason: String },
    /// The session is dead and the account must be linked again.
    LoggedOut {
        reason: String,
        message: Option<String>,
    },
    /// A link attempt failed (bad number, throttling, ...).
    PairError { message: String },
    /// One decrypted inbound message.
    Message(MessageView),
    /// A message replayed from WhatsApp's history payload. Existing content:
    /// the UI must not treat it as new or unread.
    HistoryMessage(MessageView),
    /// A status (story) update from a contact, or one we posted.
    Status(StatusUpdate),
    /// Someone watched one of our stories (`id` is the status message id).
    StatusViewed {
        id: String,
        /// Viewer JID.
        jid: String,
        name: String,
        timestamp: i64,
    },
    /// A chat discovered in WhatsApp's history sync. Metadata only: the
    /// conversation payload is encrypted and not materialized here.
    Chat {
        jid: String,
        name: String,
        is_group: bool,
        timestamp: i64,
    },
    /// A better display name arrived after the chat was already announced
    /// (push names and conversations can land in different history chunks).
    ChatName { jid: String, name: String },
    /// The same peer is stored under two ids (typically a LID chat and its
    /// phone-number chat). The UI should fold `from` into `to`.
    ChatAlias { from: String, to: String },
    /// An outbound message was accepted by the server.
    Sent {
        chat: String,
        id: String,
        timestamp: i64,
    },
    /// Delivery/read state for messages we sent.
    Receipt {
        chat: String,
        ids: Vec<String>,
        /// `delivered`, `read` or `played`.
        status: String,
    },
    /// Progress of a batched history re-sync (done, total chats).
    HistorySyncProgress { done: u32, total: u32 },
    /// Metadata for a followed channel (newsletter): display name, follower
    /// count and avatar URL. Emitted after `sync_channels`.
    ChannelInfo {
        jid: String,
        name: String,
        subscribers: u64,
        picture: Option<String>,
    },
    /// A chat was deleted on this or another linked device. The client keeps a
    /// local copy so it can be revealed with the eye toggle.
    ChatDeleted { chat: String },
    /// A chat's history was cleared.
    ChatCleared { chat: String },
    /// A message was deleted (for me, or revoked for everyone).
    MessageDeleted { chat: String, id: String },
    /// A message was edited by its sender.
    MessageEdited {
        chat: String,
        id: String,
        text: String,
    },
    /// A saved-contact name synced from the phone's address book.
    ContactName { jid: String, name: String },
    /// The linked account's own identity (JID and push name), for avatars and
    /// the self-chat label.
    SelfIdentity { jid: String, name: String },
    /// The chat's pinned state changed - ours (a pin action we sent) or
    /// another device's, or a full-sync replay of an existing pin.
    ChatPin {
        chat: String,
        pinned: bool,
        timestamp: i64,
    },
    /// A chat was archived or unarchived (app-state sync, every client).
    ChatArchive { chat: String, archived: bool },
    /// A chat's mute state changed; `until` is Unix ms (huge value = "always").
    ChatMute {
        chat: String,
        until: i64,
        timestamp: i64,
    },
    /// The chat was marked read or unread (app-state sync, every client).
    ChatReadMark { chat: String, read: bool },
    /// A message was starred or unstarred (app-state sync, every client; a
    /// full sync replays every starred message, so the list rebuilds itself).
    StarMessage {
        chat: String,
        id: String,
        starred: bool,
        timestamp: i64,
    },
    /// An emoji reaction on an existing message (empty emoji removes it).
    Reaction {
        chat: String,
        id: String,
        sender: String,
        from_me: bool,
        emoji: String,
        /// Sender timestamp (ms) so stale echoes cannot revert a newer change.
        timestamp: i64,
    },
    /// An incoming voice/video call offer.
    CallIncoming {
        call_id: String,
        peer: String,
        video: bool,
    },
    /// The call ended locally or on the peer.
    CallEnded { call_id: String, reason: String },
    /// Decoded 16 kHz mono PCM playout frames from the peer.
    CallAudio { call_id: String, samples: Vec<i16> },
    /// One Annex-B H.264 access unit from the peer (base64).
    CallVideo {
        call_id: String,
        data: String,
        keyframe: bool,
    },
    /// Coarse call state for the overlay ("calling", "connected", ...).
    CallState { call_id: String, state: String },
    /// Our own camera preview (RGBA, base64) for the call sheet's self-view.
    CallSelfVideo {
        data: String,
        width: u32,
        height: u32,
    },
    /// A sticker pack someone shared in a chat.
    StickerPack(StickerPackView),
}

impl UiEvent {
    /// Stable discriminant for logging and tests.
    pub fn kind(&self) -> &'static str {
        match self {
            UiEvent::Qr { .. } => "qr",
            UiEvent::QrExpired => "qr_expired",
            UiEvent::PairCode { .. } => "pair_code",
            UiEvent::Paired { .. } => "paired",
            UiEvent::Connected => "connected",
            UiEvent::Disconnected { .. } => "disconnected",
            UiEvent::LoggedOut { .. } => "logged_out",
            UiEvent::PairError { .. } => "pair_error",
            UiEvent::Message(_) => "message",
            UiEvent::HistoryMessage(_) => "history_message",
            UiEvent::Status(_) => "status",
            UiEvent::StatusViewed { .. } => "status_viewed",
            UiEvent::Chat { .. } => "chat",
            UiEvent::ChatName { .. } => "chat_name",
            UiEvent::ChatAlias { .. } => "chat_alias",
            UiEvent::HistorySyncProgress { .. } => "history_sync_progress",
            UiEvent::ChannelInfo { .. } => "channel_info",
            UiEvent::Sent { .. } => "sent",
            UiEvent::Receipt { .. } => "receipt",
            UiEvent::ChatDeleted { .. } => "chat_deleted",
            UiEvent::ChatCleared { .. } => "chat_cleared",
            UiEvent::MessageDeleted { .. } => "message_deleted",
            UiEvent::MessageEdited { .. } => "message_edited",
            UiEvent::ContactName { .. } => "contact_name",
            UiEvent::SelfIdentity { .. } => "self_identity",
            UiEvent::ChatPin { .. } => "chat_pin",
            UiEvent::ChatArchive { .. } => "chat_archive",
            UiEvent::ChatMute { .. } => "chat_mute",
            UiEvent::ChatReadMark { .. } => "chat_read_mark",
            UiEvent::StarMessage { .. } => "star_message",
            UiEvent::Reaction { .. } => "reaction",
            UiEvent::CallIncoming { .. } => "call_incoming",
            UiEvent::CallEnded { .. } => "call_ended",
            UiEvent::CallAudio { .. } => "call_audio",
            UiEvent::CallVideo { .. } => "call_video",
            UiEvent::CallState { .. } => "call_state",
            UiEvent::CallSelfVideo { .. } => "call_self_video",
            UiEvent::StickerPack(_) => "sticker_pack",
        }
    }
}

/// One chat to re-fetch from the primary phone. `oldest_id` empty means the
/// chat has no local messages yet, so the phone returns its latest window.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryRequest {
    pub chat: String,
    #[serde(default)]
    pub oldest_id: String,
    #[serde(default)]
    pub oldest_from_me: bool,
    #[serde(default)]
    pub oldest_timestamp: i64,
    #[serde(default = "default_history_count")]
    pub count: i32,
}

fn default_history_count() -> i32 {
    50
}

/// Identity WhatsApp returns for a peer: the phone-number and LID forms, plus
/// the verified display name of a business account.
#[derive(Debug, Clone, Serialize)]
pub struct PeerLookup {
    /// Chat key the UI asked about.
    pub chat: String,
    /// Phone-number user part, when the peer has one.
    pub phone: Option<String>,
    /// LID user part, when the peer has one.
    pub lid: Option<String>,
    /// Verified business name, when the account is a verified business.
    pub name: Option<String>,
    pub is_business: bool,
}

/// Rich link preview carried inside a text message by WhatsApp itself.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkPreviewView {
    pub url: String,
    pub title: String,
    pub description: String,
    /// Inline thumbnail as a data URI (WhatsApp ships a few KB JPEG).
    pub thumb: Option<String>,
    /// CDN path of the large preview image (channel/broadcast link posts ship
    /// this instead of an inline JPEG). Fetched on demand by the UI.
    pub thumb_path: Option<String>,
}

/// A shared location: the coordinates, an optional place name and whether it
/// is a live location (which keeps updating on the sender's phone).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct LocationView {
    pub lat: f64,
    pub lng: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub live: bool,
}

/// A contact message: the display name and one phone number.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContactCardView {
    pub name: String,
    pub phone: Option<String>,
}

/// A poll: the question, its options and whether it allows several answers.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct PollView {
    pub question: String,
    pub options: Vec<String>,
    pub multiple: bool,
}

/// File metadata for a document message, shaped for the WhatsApp file card.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentView {
    /// Original file name, including the extension (e.g. `report.pdf`).
    pub name: String,
    /// MIME type, e.g. `application/pdf`; empty when the sender omitted it.
    pub mime: String,
    /// Size in bytes, `0` when unknown.
    pub size: u64,
    /// Page count reported by the sender, `0` when unknown.
    pub pages: u32,
    /// Embedded JPEG thumbnail as a data URI, when the document carries one.
    pub thumb: Option<String>,
}

/// Playback metadata for an audio message, shaped for the audio bubble:
/// the sender's duration, the voice-note flag drawn with a waveform, and the
/// waveform samples (0..=255 amplitudes) themselves.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioView {
    /// Duration reported by the sender, in seconds (0 when unknown — the
    /// player then reads it from the decoded file).
    pub seconds: u32,
    /// `true` for a recorded voice note (`ptt`), which WhatsApp renders with a
    /// waveform instead of a plain seek line.
    pub ptt: bool,
    /// Sender's waveform samples; empty when WhatsApp did not ship one.
    pub waveform: Vec<u8>,
}

/// The message quoted by a WhatsApp reply.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplyView {
    /// Id of the quoted message.
    pub id: String,
    /// Author of the quoted message.
    pub sender: Option<String>,
    /// Best-known display name for the quoted author.
    pub sender_name: Option<String>,
    /// `text`, `image`, `video`, `document`, `sticker`, ...
    pub kind: String,
    /// Caption/body of the quoted message.
    pub text: String,
    /// Embedded JPEG thumbnail, when the quoted message is a photo.
    pub thumb: Option<String>,
}

/// A message shaped for rendering: no protobuf, no secrets, no JIDs to parse.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageView {
    /// Chat id (`<user>@s.whatsapp.net` or `<id>@g.us`).
    pub chat: String,
    /// Best-known display name for the chat.
    pub chat_name: String,
    /// Sender id; `None` for messages we sent.
    pub sender: Option<String>,
    /// Sender display name as pushed by WhatsApp, or the saved contact name.
    pub sender_name: Option<String>,
    /// Sender's phone-number id (digits only), resolved from a LID when the
    /// session knows the PN mapping.
    pub sender_phone: Option<String>,
    pub id: String,
    pub from_me: bool,
    pub is_group: bool,
    /// Milliseconds since the Unix epoch.
    pub timestamp: i64,
    /// `text`, `image`, `video`, `audio`, `document`, `sticker`, `reaction`, ...
    pub kind: String,
    /// Body text, or the caption for media messages.
    pub text: String,
    /// JIDs explicitly `@mentioned` in the message (`contextInfo.mentionedJid`).
    pub mentions: Vec<String>,
    /// Quoted message when this message is a WhatsApp reply.
    pub reply_to: Option<ReplyView>,
    /// Link preview embedded by the sender's WhatsApp client.
    pub link: Option<LinkPreviewView>,
    /// File card metadata for `document` messages.
    pub document: Option<DocumentView>,
    /// Playback metadata for `audio`/voice-note messages.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio: Option<AudioView>,
    /// A shared contact card (`contactMessage` / `contactsArrayMessage`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contact: Option<ContactCardView>,
    /// A location (`locationMessage` / `liveLocationMessage`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<LocationView>,
    /// A poll (`pollCreationMessage`): the question and its options.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub poll: Option<PollView>,
    /// Channel post counters (`<views_count>`, `<forwards_count>`, `<reactions>`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stats: Option<ChannelStats>,
    /// The sender forwarded this message (`contextInfo.isForwarded` or a
    /// non-zero forwarding score).
    #[serde(default, skip_serializing_if = "is_false")]
    pub forwarded: bool,
    /// This copy came from reply-quote recovery of a view-once fanout: the
    /// body is kept locally, so the rail eye reveals or hides it like the
    /// official client's hidden-media placeholder.
    #[serde(default, skip_serializing_if = "is_false")]
    pub once_recovered: bool,
    /// A reply or reaction sent against a status update (`contextInfo` points at
    /// `status@broadcast`): WhatsApp renders these with the "· Status" card
    /// instead of a plain reply quote.
    #[serde(default, skip_serializing_if = "is_false")]
    pub status_quote: bool,
    /// A video the sender marked as a GIF (`videoMessage.gifPlayback`): the
    /// official client loops it without player controls.
    #[serde(default, skip_serializing_if = "is_false")]
    pub gif: bool,
}

pub(crate) fn is_false(value: &bool) -> bool {
    !*value
}

/// Counters the server attaches to a channel post, shown under the card like the
/// desktop app (reaction pills, share count, view count).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChannelStats {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub views: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub forwards: Option<u64>,
    /// The busiest few emoji, so a post never stores the server's full tally.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reactions: Vec<ReactionCount>,
    /// Everyone who reacted, including the emoji that were not kept above.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reaction_total: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReactionCount {
    pub code: String,
    pub count: u32,
}

/// One status (story) update.
#[derive(Debug, Clone, Serialize)]
pub struct StatusUpdate {
    pub id: String,
    /// Author JID.
    pub sender: String,
    pub name: String,
    /// `text`, `image` or `video`.
    pub kind: String,
    pub text: String,
    /// Background colour of a text status (`#rrggbb`), as shipped in the
    /// protobuf, so the story paints the same colour the poster picked.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background: Option<String>,
    pub timestamp: i64,
    /// True when the account itself posted this update.
    pub from_me: bool,
}

/// Result of uploading and sending a file.
#[derive(Debug, Clone, Serialize)]
pub struct AttachmentResult {
    pub id: String,
    /// `image`, `video` or `document`.
    pub kind: String,
}

/// A favorite sticker downloaded from the linked account's app state.
#[derive(Debug, Clone, Serialize)]
pub struct FavoriteStickerView {
    pub id: String,
    /// Local file path, ready for `convertFileSrc`.
    pub path: String,
    pub width: u32,
    pub height: u32,
}

/// One member of a group, shaped for the group-info screen.
#[derive(Debug, Clone, Serialize)]
pub struct GroupMemberView {
    pub jid: String,
    pub phone: Option<String>,
    pub admin: bool,
}

/// Group metadata shown in the group-info screen.
#[derive(Debug, Clone, Serialize)]
pub struct GroupInfoView {
    pub jid: String,
    pub subject: String,
    pub description: Option<String>,
    pub members: Vec<GroupMemberView>,
    /// This group is a WhatsApp Community parent (its rows host child groups).
    #[serde(default, skip_serializing_if = "is_false")]
    pub is_community: bool,
    /// For a child group: the community parent it belongs to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_jid: Option<String>,
    /// The community's default announcement subgroup — hidden from the chat
    /// list like the official client; its activity folds into the community.
    #[serde(default, skip_serializing_if = "is_false")]
    pub is_announce: bool,
}

/// One sticker inside a pack someone shared in a chat.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StickerItemView {
    pub name: String,
    pub animated: bool,
    /// The cached WebP, once downloaded.
    #[serde(default)]
    pub path: String,
}

/// A sticker pack shared in a chat (`stickerPackMessage`). WhatsApp never syncs
/// the account's pack list to linked devices, so packs appear when shared.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StickerPackView {
    pub id: String,
    pub name: String,
    pub publisher: String,
    pub stickers: Vec<StickerItemView>,
}

/// Serializable call metadata returned to the UI.
#[derive(Debug, Clone, Serialize)]
pub struct CallInfo {
    pub call_id: String,
    pub peer: String,
    pub video: bool,
    pub outgoing: bool,
}
