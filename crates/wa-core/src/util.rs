//! Shared parsing, media-cache and formatting helpers.

use super::*;

pub(crate) fn normalize_phone(input: &str) -> Result<String, String> {
    let digits: String = input.chars().filter(char::is_ascii_digit).collect();
    if digits.len() < 8 || digits.len() > 15 {
        return Err("enter a full international number, e.g. +62 812 3456 7890".into());
    }
    Ok(digits)
}

/// Parse PN/LID strings into JIDs for group participant updates.
pub(crate) fn parse_jids(participants: &[String]) -> Result<Vec<Jid>, String> {
    let jids: Vec<Jid> = participants
        .iter()
        .filter_map(|jid| jid.parse::<Jid>().ok())
        .collect();
    if jids.is_empty() {
        return Err("no valid participants".into());
    }
    Ok(jids)
}

/// A group change succeeds if at least one participant was accepted.
pub(crate) fn group_change_result(
    results: Vec<whatsapp_rust::ParticipantChangeResponse>,
    action: &str,
) -> Result<(), String> {
    if results.is_empty() || results.iter().any(|result| result.is_ok()) {
        return Ok(());
    }
    let reason = results
        .iter()
        .filter_map(|result| result.error.as_deref())
        .collect::<Vec<_>>()
        .join(", ");
    Err(format!("{action} failed: {reason}"))
}

/// Render the pairing payload as an inline SVG.
pub(crate) fn render_qr(data: &str) -> String {
    use qrcode::render::svg;

    match qrcode::QrCode::new(data.as_bytes()) {
        Ok(code) => code
            .render::<svg::Color>()
            .min_dimensions(240, 240)
            .quiet_zone(true)
            .dark_color(svg::Color("#0b141a"))
            .light_color(svg::Color("#ffffff"))
            .build(),
        Err(err) => {
            tracing::warn!(%err, "could not render pairing QR");
            String::new()
        }
    }
}

/// `123@lid` -> `123`
pub(crate) fn user_part(jid: &str) -> &str {
    let user = jid.split('@').next().unwrap_or(jid);
    user.split(':').next().unwrap_or(user)
}

/// Display name for a user id. Saved contact names win over push names.
pub(crate) fn look_up_name(
    user: &str,
    names: &Mutex<HashMap<String, String>>,
    aliases: &Mutex<HashMap<String, String>>,
    contacts: &Mutex<HashMap<String, String>>,
) -> Option<String> {
    if let Some(name) = contacts.lock().expect("contacts lock").get(user) {
        return Some(name.clone());
    }
    let alias = aliases.lock().expect("aliases lock").get(user).cloned();
    if let Some(alias) = alias.as_deref()
        && let Some(name) = contacts.lock().expect("contacts lock").get(alias)
    {
        return Some(name.clone());
    }
    if let Some(name) = names.lock().expect("names lock").get(user) {
        return Some(name.clone());
    }
    alias.and_then(|alias| names.lock().expect("names lock").get(&alias).cloned())
}

/// PN user part for a sender, resolving a LID through the known aliases.
pub(crate) fn phone_for(jid: &Jid, aliases: &Mutex<HashMap<String, String>>) -> Option<String> {
    if jid.server == Server::Lid {
        return aliases
            .lock()
            .expect("aliases lock")
            .get(jid.user.as_str())
            .cloned();
    }
    (jid.server == Server::Pn).then(|| jid.user.to_string())
}

/// Saved-contact names live next to the device store so they survive restarts.
pub(crate) fn load_contacts(path: &std::path::Path) -> HashMap<String, String> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default()
}

pub(crate) fn save_contacts(path: &std::path::Path, contacts: &HashMap<String, String>) {
    if let Ok(json) = serde_json::to_string(contacts) {
        let _ = std::fs::write(path, json);
    }
}

/// WhatsApp ships the link preview (title, description, thumbnail) inside the
/// text message, so receivers do not have to scrape the page themselves.
pub(crate) fn link_preview(message: &wa::Message) -> Option<LinkPreviewView> {
    use base64::Engine as _;

    let ext = message
        .get_base_message()
        .extended_text_message
        .as_option()?;
    let url = ext
        .matched_text
        .as_deref()
        .map(str::trim)
        .filter(|url| !url.is_empty())?
        .to_owned();
    let title = ext.title.as_deref().unwrap_or("").trim().to_owned();
    let description = ext.description.as_deref().unwrap_or("").trim().to_owned();
    // Large previews (channel/broadcast link posts) ship the image as a CDN
    // path instead of an inline JPEG, so the UI can fetch it on demand.
    let thumb_path = ext
        .thumbnail_direct_path
        .as_deref()
        .map(str::to_owned)
        .filter(|path| !path.is_empty());
    let thumb = ext
        .jpeg_thumbnail
        .as_deref()
        .filter(|bytes| !bytes.is_empty())
        .map(|bytes| {
            let mime = if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
                "image/png"
            } else if bytes.starts_with(b"GIF8") {
                "image/gif"
            } else if bytes.len() > 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
                "image/webp"
            } else {
                "image/jpeg"
            };
            format!(
                "data:{mime};base64,{}",
                base64::engine::general_purpose::STANDARD.encode(bytes)
            )
        });

    if title.is_empty() && description.is_empty() && thumb.is_none() && thumb_path.is_none() {
        return None;
    }
    Some(LinkPreviewView {
        url,
        title: title.chars().take(180).collect(),
        description: description.chars().take(260).collect(),
        thumb,
        thumb_path,
    })
}

/// Best-effort media classification straight from the protobuf, for history
/// entries (live messages carry the server's `mediatype` attribute instead).
/// Media attached to a business template. `get_base_message()` does not reach
/// it: the image/video lives under `template_message.format`.
#[derive(Clone, Copy)]
pub(crate) enum TemplateMedia<'a> {
    Image(&'a wa::message::ImageMessage),
    Video(&'a wa::message::VideoMessage),
    Document(&'a wa::message::DocumentMessage),
}

pub(crate) fn template_media(message: &wa::Message) -> Option<TemplateMedia<'_>> {
    use wa::message::interactive_message::header::Media;
    use wa::message::template_message::Format;
    use wa::message::template_message::four_row_template::Title as FourRowTitle;
    use wa::message::template_message::hydrated_four_row_template::Title as HydratedTitle;

    let template = message.template_message.as_option()?;
    match template.format.as_ref()? {
        Format::InteractiveMessageTemplate(interactive) => {
            let header = interactive.header.as_option()?;
            match header.media.as_ref()? {
                Media::ImageMessage(image) => Some(TemplateMedia::Image(image)),
                Media::VideoMessage(video) => Some(TemplateMedia::Video(video)),
                Media::DocumentMessage(document) => Some(TemplateMedia::Document(document)),
                _ => None,
            }
        }
        Format::HydratedFourRowTemplate(hydrated) => match hydrated.title.as_ref()? {
            HydratedTitle::ImageMessage(image) => Some(TemplateMedia::Image(image)),
            HydratedTitle::VideoMessage(video) => Some(TemplateMedia::Video(video)),
            HydratedTitle::DocumentMessage(document) => Some(TemplateMedia::Document(document)),
            _ => None,
        },
        Format::FourRowTemplate(four) => match four.title.as_ref()? {
            FourRowTitle::ImageMessage(image) => Some(TemplateMedia::Image(image)),
            FourRowTitle::VideoMessage(video) => Some(TemplateMedia::Video(video)),
            FourRowTitle::DocumentMessage(document) => Some(TemplateMedia::Document(document)),
            _ => None,
        },
    }
}

/// Text a bubble can show. Plain text and captions cover normal chats; business
/// accounts instead send a template whose body lives in `template_message`,
/// which `text_content()` does not reach.
/// The sender forwarded this message (WhatsApp sets `isForwarded` and bumps
/// `forwardingScore`; anything above 4 becomes "Forwarded many times").
pub(crate) fn is_forwarded(message: &wa::Message) -> bool {
    context_info(message)
        .map(|info| info.is_forwarded.unwrap_or(false) || info.forwarding_score.unwrap_or(0) > 0)
        .unwrap_or(false)
}

pub(crate) fn message_text(message: &wa::Message) -> Option<String> {
    if let Some(text) = message.text_content().filter(|text| !text.is_empty()) {
        return Some(text.to_owned());
    }
    if let Some(caption) = message.get_caption().filter(|text| !text.is_empty()) {
        return Some(caption.to_owned());
    }
    if let Some(template) = message.template_message.as_option() {
        if let Some(text) = template
            .hydrated_template
            .as_option()
            .and_then(|hydrated| hydrated.hydrated_content_text.as_deref())
            .filter(|text| !text.is_empty())
        {
            return Some(text.to_owned());
        }
        if let Some(format) = template.format.as_ref() {
            match format {
                wa::message::template_message::Format::InteractiveMessageTemplate(interactive) => {
                    if let Some(text) = interactive
                        .body
                        .as_option()
                        .and_then(|body| body.text.as_deref())
                        .filter(|text| !text.is_empty())
                    {
                        return Some(text.to_owned());
                    }
                    if let Some(text) = interactive
                        .header
                        .as_option()
                        .and_then(|header| header.title.as_deref())
                        .filter(|text| !text.is_empty())
                    {
                        return Some(text.to_owned());
                    }
                }
                wa::message::template_message::Format::HydratedFourRowTemplate(hydrated) => {
                    if let Some(
                        wa::message::template_message::hydrated_four_row_template::Title::HydratedTitleText(
                            text,
                        ),
                    ) = hydrated.title.as_ref()
                        && !text.is_empty()
                    {
                        return Some(text.clone());
                    }
                }
                _ => {}
            }
        }
    }
    // Business templates keep their caption on the header media.
    match template_media(message)? {
        TemplateMedia::Image(image) => image
            .caption
            .as_deref()
            .filter(|text| !text.is_empty())
            .map(str::to_owned),
        TemplateMedia::Video(video) => video
            .caption
            .as_deref()
            .filter(|text| !text.is_empty())
            .map(str::to_owned),
        TemplateMedia::Document(document) => document
            .caption
            .as_deref()
            .filter(|text| !text.is_empty())
            .map(str::to_owned),
    }
}

/// Background colour of a text status (`0xAARRGGBB` in the protobuf) as the
/// `#rrggbb` string the webview paints, so a story looks the same for us as it
/// does for everyone else.
pub(crate) fn status_background(message: &wa::Message) -> Option<String> {
    let argb = message
        .extended_text_message
        .as_option()
        .and_then(|text| text.background_argb)?;
    if argb & 0x00FF_FFFF == 0 {
        return None;
    }
    Some(format!(
        "#{:02x}{:02x}{:02x}",
        (argb >> 16) & 0xFF,
        (argb >> 8) & 0xFF,
        argb & 0xFF
    ))
}

pub(crate) fn media_kind(message: &wa::Message) -> Option<&'static str> {
    let base = message.get_base_message();
    if base.image_message.as_option().is_some() {
        Some("image")
    } else if base.video_message.as_option().is_some() {
        Some("video")
    } else if base.audio_message.as_option().is_some() {
        Some("audio")
    } else if base.document_message.as_option().is_some() {
        Some("document")
    } else if base.sticker_message.as_option().is_some() {
        Some("sticker")
    } else {
        template_media(message).map(|media| match media {
            TemplateMedia::Image(_) => "image",
            TemplateMedia::Video(_) => "video",
            TemplateMedia::Document(_) => "document",
        })
    }
}

/// File card metadata for a document message: name, MIME, size, pages and the
/// sender's JPEG thumbnail, which WhatsApp Web shows next to the card.
pub(crate) fn document_view(message: &wa::Message) -> Option<DocumentView> {
    let base = message.get_base_message();
    let document = match base.document_message.as_option() {
        Some(document) => document,
        None => match template_media(message)? {
            TemplateMedia::Document(document) => document,
            _ => return None,
        },
    };

    let name = [document.file_name.as_deref(), document.title.as_deref()]
        .into_iter()
        .flatten()
        .map(str::trim)
        .find(|value| !value.is_empty())
        .unwrap_or("document")
        .to_owned();
    let mime = document
        .mimetype
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("")
        .to_owned();
    let thumb = document
        .jpeg_thumbnail
        .as_deref()
        .filter(|bytes| !bytes.is_empty())
        .map(|bytes| {
            use base64::Engine as _;
            format!(
                "data:image/jpeg;base64,{}",
                base64::engine::general_purpose::STANDARD.encode(bytes)
            )
        });

    Some(DocumentView {
        name,
        mime,
        size: document.file_length.unwrap_or(0),
        pages: document.page_count.unwrap_or(0),
        thumb,
    })
}

/// Playback metadata for an audio message: sender duration, voice-note flag
/// (`ptt`) and the waveform bytes, which the UI draws around the seek bar.
pub(crate) fn audio_view(message: &wa::Message) -> Option<AudioView> {
    let base = message.get_base_message();
    let audio = base.audio_message.as_option()?;
    Some(AudioView {
        seconds: audio.seconds.unwrap_or(0),
        ptt: audio.ptt.unwrap_or(false),
        waveform: audio.waveform.clone().unwrap_or_default(),
    })
}

/// A reply or reaction sent against a status update points its context at the
/// status broadcast; the official client prints those with the "· Status" card
/// rather than a plain reply quote.
pub(crate) fn quotes_status(message: &wa::Message) -> bool {
    context_info(message).and_then(|context| context.remote_jid.as_deref())
        == Some("status@broadcast")
}

/// Reads the first `contextInfo` attached to a chat message.
pub(crate) fn context_info(message: &wa::Message) -> Option<&wa::ContextInfo> {
    let base = message.get_base_message();
    base.extended_text_message
        .as_option()
        .and_then(|m| m.context_info.as_option())
        .or_else(|| {
            base.image_message
                .as_option()
                .and_then(|m| m.context_info.as_option())
        })
        .or_else(|| {
            base.video_message
                .as_option()
                .and_then(|m| m.context_info.as_option())
        })
        .or_else(|| {
            base.audio_message
                .as_option()
                .and_then(|m| m.context_info.as_option())
        })
        .or_else(|| {
            base.document_message
                .as_option()
                .and_then(|m| m.context_info.as_option())
        })
        .or_else(|| {
            base.sticker_message
                .as_option()
                .and_then(|m| m.context_info.as_option())
        })
}

pub(crate) fn mentioned_jids(message: &wa::Message) -> Vec<String> {
    context_info(message)
        .map(|context| context.mentioned_jid.clone())
        .unwrap_or_default()
}

/// WhatsApp embeds the quoted message inside `contextInfo.quotedMessage`.
/// Keep only display-safe fields; media bytes are limited to the inline JPEG
/// thumbnail so a chat event stays small.
pub(crate) fn reply_view(
    message: &wa::Message,
    names: &Mutex<HashMap<String, String>>,
    aliases: &Mutex<HashMap<String, String>>,
    contacts: &Mutex<HashMap<String, String>>,
) -> Option<ReplyView> {
    use base64::Engine as _;

    let context = context_info(message)?;
    let quoted = context.quoted_message.as_option()?;
    let kind = media_kind(quoted).unwrap_or("text").to_owned();
    let mut text = quoted
        .text_content()
        .or_else(|| quoted.get_caption())
        .unwrap_or_default()
        .trim()
        .to_owned();
    // A quoted document shows its file name (and page count) instead of a
    // generic "[Document]", like WhatsApp's reply box.
    if kind == "document"
        && let Some(document) = document_view(quoted)
    {
        if text.is_empty() {
            text = document.name;
        }
        if document.pages > 1 {
            text.push_str(&format!(" ({} pages)", document.pages));
        }
    }
    let sender = context.participant.clone();
    let sender_name = sender.as_deref().and_then(|raw| {
        raw.parse::<Jid>()
            .ok()
            .and_then(|jid| look_up_name(jid.user.as_str(), names, aliases, contacts))
    });
    let thumb = quoted
        .get_base_message()
        .image_message
        .as_option()
        .and_then(|image| image.jpeg_thumbnail.as_deref())
        .or_else(|| {
            quoted
                .get_base_message()
                .video_message
                .as_option()
                .and_then(|video| video.jpeg_thumbnail.as_deref())
        })
        .filter(|bytes| !bytes.is_empty())
        .map(|bytes| {
            format!(
                "data:image/jpeg;base64,{}",
                base64::engine::general_purpose::STANDARD.encode(bytes)
            )
        });

    Some(ReplyView {
        id: context.stanza_id.clone().unwrap_or_default(),
        sender,
        sender_name,
        kind,
        text,
        thumb,
    })
}

/// Learn LID -> PN for the chats and senders in this batch from the client's
/// persisted mapping cache, so live LID traffic joins the existing PN
/// conversation and group bubbles can show the sender's phone number.
pub(crate) async fn seed_canon(
    client: &Client,
    canon: &Mutex<HashMap<String, String>>,
    aliases: &Mutex<HashMap<String, String>>,
    event: &Event,
) {
    let Event::Messages(batch) = event else {
        return;
    };

    for inbound in batch.messages.iter() {
        let source = &inbound.info.source;
        for jid in [&source.chat, &source.sender] {
            if jid.server != Server::Lid {
                continue;
            }
            let user = jid.user.to_string();
            if aliases.lock().expect("aliases lock").contains_key(&user) {
                continue;
            }
            if let Ok(Some(entry)) = client.get_lid_pn_entry(jid).await {
                let pn = user_part(&entry.phone_number).to_owned();
                if pn.is_empty() {
                    continue;
                }
                canon
                    .lock()
                    .expect("canon lock")
                    .insert(user.clone(), pn.clone());
                let mut aliases = aliases.lock().expect("aliases lock");
                aliases.insert(user, pn.clone());
                aliases.insert(pn, jid.user.to_string());
            }
        }
    }
}

/// Media file extensions the cache may use, ordered by likelihood.
pub(crate) const MEDIA_EXTENSIONS: [&str; 12] = [
    "jpg", "jpeg", "png", "webp", "mp4", "3gp", "mov", "ogg", "m4a", "mp3", "pdf", "bin",
];

/// Message ids are hex-ish, but keep them filesystem-safe for the media and
/// message caches.
pub(crate) fn safe_media_id(id: &str) -> String {
    id.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

/// Keep the protobuf of a media message around so it can be downloaded later,
/// in memory and on disk (history is only replayed once per pairing).
pub(crate) fn remember_message(
    cache: &Mutex<HashMap<String, CachedMedia>>,
    msg_dir: &std::path::Path,
    id: &str,
    message: Arc<wa::Message>,
    chat: &Jid,
    from_me: bool,
    participant: Option<&Jid>,
) {
    let entry = CachedMedia {
        message,
        chat: chat.to_string(),
        from_me,
        participant: participant.map(|jid| jid.to_string()),
    };

    if let Err(err) = persist_media(msg_dir, id, &entry) {
        tracing::debug!(%err, "could not persist media message");
    }

    let mut cache = cache.lock().expect("media lock");
    cache.insert(id.to_owned(), entry);
    if cache.len() > 1500 {
        let stale: Vec<String> = cache.keys().take(500).cloned().collect();
        for key in stale {
            cache.remove(&key);
        }
    }
}

pub(crate) fn media_cache_path(msg_dir: &std::path::Path, id: &str) -> PathBuf {
    msg_dir.join(format!("{}.bin", safe_media_id(id)))
}

pub(crate) fn persist_media(
    msg_dir: &std::path::Path,
    id: &str,
    entry: &CachedMedia,
) -> std::io::Result<()> {
    let proto = whatsapp_rust::waproto::codec::message_to_vec(&entry.message);
    let chat = entry.chat.as_bytes();
    let participant = entry.participant.as_deref().unwrap_or("").as_bytes();

    let mut buffer = Vec::with_capacity(proto.len() + chat.len() + participant.len() + 8);
    buffer.push(1u8);
    buffer.push(u8::from(entry.from_me));
    buffer.extend_from_slice(&(chat.len() as u16).to_le_bytes());
    buffer.extend_from_slice(chat);
    buffer.extend_from_slice(&(participant.len() as u16).to_le_bytes());
    buffer.extend_from_slice(participant);
    buffer.extend_from_slice(&proto);

    std::fs::create_dir_all(msg_dir)?;
    std::fs::write(media_cache_path(msg_dir, id), buffer)
}

pub(crate) fn load_media(msg_dir: &std::path::Path, id: &str) -> Option<CachedMedia> {
    let bytes = std::fs::read(media_cache_path(msg_dir, id)).ok()?;
    let mut cursor = 0usize;

    let version = *bytes.get(cursor)?;
    cursor += 1;
    if version != 1 {
        return None;
    }
    let from_me = *bytes.get(cursor)? != 0;
    cursor += 1;

    let chat_len = u16::from_le_bytes(bytes.get(cursor..cursor + 2)?.try_into().ok()?) as usize;
    cursor += 2;
    let chat = String::from_utf8(bytes.get(cursor..cursor + chat_len)?.to_vec()).ok()?;
    cursor += chat_len;

    let participant_len =
        u16::from_le_bytes(bytes.get(cursor..cursor + 2)?.try_into().ok()?) as usize;
    cursor += 2;
    let participant_raw = bytes.get(cursor..cursor + participant_len)?.to_vec();
    cursor += participant_len;
    let participant = String::from_utf8(participant_raw)
        .ok()
        .filter(|value| !value.is_empty());

    let message = whatsapp_rust::waproto::codec::message_decode(bytes.get(cursor..)?).ok()?;

    Some(CachedMedia {
        message: Arc::new(message),
        chat,
        from_me,
        participant,
    })
}

pub(crate) struct RawMedia<'a> {
    pub(crate) direct_path: &'a str,
    pub(crate) media_key: &'a [u8],
    pub(crate) file_sha256: &'a [u8],
    pub(crate) file_enc_sha256: &'a [u8],
    pub(crate) file_length: u64,
    pub(crate) mimetype: Option<&'a str>,
    pub(crate) media_type: MediaType,
}

/// Pull the CDN fields out of whichever media payload the message carries.
pub(crate) fn raw_media(message: &wa::Message) -> Option<RawMedia<'_>> {
    let base = message.get_base_message();

    macro_rules! raw_from {
        ($media:expr, $ty:expr) => {{
            let media = $media;
            RawMedia {
                direct_path: media.direct_path.as_deref()?,
                media_key: media.media_key.as_deref()?,
                file_sha256: media.file_sha256.as_deref()?,
                file_enc_sha256: media.file_enc_sha256.as_deref()?,
                file_length: media.file_length?,
                mimetype: media.mimetype.as_deref(),
                media_type: $ty,
            }
        }};
    }

    if let Some(media) = base.image_message.as_option() {
        Some(raw_from!(media, MediaType::Image))
    } else if let Some(media) = base.video_message.as_option() {
        Some(raw_from!(media, MediaType::Video))
    } else if let Some(media) = base.audio_message.as_option() {
        Some(raw_from!(media, MediaType::Audio))
    } else if let Some(media) = base.sticker_message.as_option() {
        Some(raw_from!(media, MediaType::Sticker))
    } else if let Some(media) = base.document_message.as_option() {
        Some(raw_from!(media, MediaType::Document))
    } else {
        // Business templates carry their image/video under the header.
        match template_media(message)? {
            TemplateMedia::Image(media) => Some(raw_from!(media, MediaType::Image)),
            TemplateMedia::Video(media) => Some(raw_from!(media, MediaType::Video)),
            TemplateMedia::Document(media) => Some(raw_from!(media, MediaType::Document)),
        }
    }
}

pub(crate) fn media_params(message: &wa::Message) -> Option<(DownloadParams, &'static str)> {
    let raw = raw_media(message)?;
    let params = DownloadParams::encrypted(
        raw.direct_path,
        raw.media_key,
        raw.file_sha256,
        raw.file_enc_sha256,
        raw.file_length,
        raw.media_type,
    );
    Some((params, extension_for(raw.mimetype, raw.media_type)))
}

/// Newsletter media can ship only `directPath` — no inline CDN `url`, no
/// `mediaKey`/`fileEncSha256` keys, because channel posts are not end-to-end
/// encrypted. The object still downloads as plain bytes with the media
/// connection's auth token.
pub(crate) fn plain_direct_media(
    message: &wa::Message,
) -> Option<(String, MediaType, Option<&str>)> {
    let base = message.get_base_message();
    let (direct_path, media_type, mimetype) = if let Some(media) = base.image_message.as_option() {
        (
            media.direct_path.as_deref()?,
            MediaType::Image,
            media.mimetype.as_deref(),
        )
    } else if let Some(media) = base.video_message.as_option() {
        (
            media.direct_path.as_deref()?,
            MediaType::Video,
            media.mimetype.as_deref(),
        )
    } else if let Some(media) = base.audio_message.as_option() {
        (
            media.direct_path.as_deref()?,
            MediaType::Audio,
            media.mimetype.as_deref(),
        )
    } else if let Some(media) = base.sticker_message.as_option() {
        (
            media.direct_path.as_deref()?,
            MediaType::Sticker,
            media.mimetype.as_deref(),
        )
    } else {
        let media = base.document_message.as_option()?;
        (
            media.direct_path.as_deref()?,
            MediaType::Document,
            media.mimetype.as_deref(),
        )
    };
    let direct_path = direct_path.trim();
    if direct_path.is_empty() {
        return None;
    }
    Some((direct_path.to_owned(), media_type, mimetype))
}

/// Channel (newsletter) posts are not end-to-end encrypted, so their media has
/// no `media_key`/`file_enc_sha256` and is served as a plain CDN object. Return
/// its URL plus the file extension to save it under.
pub(crate) fn plain_media_url(message: &wa::Message) -> Option<(String, &'static str)> {
    let base = message.get_base_message();
    let (url, mimetype, media_type) = if let Some(media) = base.image_message.as_option() {
        (
            media.url.as_deref(),
            media.mimetype.as_deref(),
            MediaType::Image,
        )
    } else if let Some(media) = base.video_message.as_option() {
        (
            media.url.as_deref(),
            media.mimetype.as_deref(),
            MediaType::Video,
        )
    } else if let Some(media) = base.audio_message.as_option() {
        (
            media.url.as_deref(),
            media.mimetype.as_deref(),
            MediaType::Audio,
        )
    } else if let Some(media) = base.sticker_message.as_option() {
        (
            media.url.as_deref(),
            media.mimetype.as_deref(),
            MediaType::Sticker,
        )
    } else {
        let media = base.document_message.as_option()?;
        (
            media.url.as_deref(),
            media.mimetype.as_deref(),
            MediaType::Document,
        )
    };
    let url = url?.trim();
    if url.is_empty() {
        return None;
    }
    Some((url.to_owned(), extension_for(mimetype, media_type)))
}

pub(crate) fn extension_for(mimetype: Option<&str>, media_type: MediaType) -> &'static str {
    match mimetype {
        Some("image/jpeg") => "jpg",
        Some("image/png") => "png",
        Some("image/webp") => "webp",
        Some("video/mp4") => "mp4",
        Some("video/3gpp") => "3gp",
        Some("video/quicktime") => "mov",
        Some("audio/ogg; codecs=opus") | Some("audio/ogg") => "ogg",
        Some("audio/mp4") => "m4a",
        Some("audio/mpeg") => "mp3",
        Some("application/pdf") => "pdf",
        _ => match media_type {
            MediaType::Image | MediaType::Sticker => "jpg",
            MediaType::Video => "mp4",
            MediaType::Audio => "ogg",
            _ => "bin",
        },
    }
}

/// Canvas size and animation flag from a WebP container.
pub(crate) fn webp_size(data: &[u8]) -> Option<(u32, u32, bool)> {
    if data.len() < 30 || &data[0..4] != b"RIFF" || &data[8..12] != b"WEBP" {
        return None;
    }

    match &data[12..16] {
        b"VP8X" => {
            let flags = data[20];
            let width = 1 + u32::from_le_bytes([data[24], data[25], data[26], 0]);
            let height = 1 + u32::from_le_bytes([data[27], data[28], data[29], 0]);
            Some((width, height, flags & 0x02 != 0))
        }
        b"VP8 " => {
            let width = 1 + (u16::from_le_bytes([data[26], data[27]]) & 0x3fff) as u32;
            let height = 1 + (u16::from_le_bytes([data[28], data[29]]) & 0x3fff) as u32;
            Some((width, height, false))
        }
        b"VP8L" => {
            let bits = u32::from_le_bytes([data[21], data[22], data[23], data[24]]);
            let width = 1 + (bits & 0x3fff);
            let height = 1 + ((bits >> 14) & 0x3fff);
            Some((width, height, false))
        }
        _ => None,
    }
}

/// Keep a cache directory bounded: newest `keep` files survive.
pub(crate) fn prune_cache(dir: &std::path::Path, keep: usize) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };

    let mut files: Vec<(std::time::SystemTime, PathBuf)> = entries
        .flatten()
        .filter_map(|entry| {
            let metadata = entry.metadata().ok()?;
            if !metadata.is_file() {
                return None;
            }
            Some((metadata.modified().ok()?, entry.path()))
        })
        .collect();

    if files.len() <= keep {
        return;
    }

    files.sort_by_key(|(modified, _)| *modified);
    let excess = files.len() - keep;
    for (_, path) in files.into_iter().take(excess) {
        let _ = std::fs::remove_file(path);
    }
}

pub(crate) fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or_default()
}

/// A poisoned Noise sender must be replaced by a fresh connection; inspecting
/// the debug chain catches the wrapped `IqError`/`ClientError` forms too.
pub(crate) fn is_sender_poison(error: &impl std::fmt::Debug) -> bool {
    let detail = format!("{error:?}");
    detail.contains("sender poisoned") || detail.contains("Poisoned")
}
