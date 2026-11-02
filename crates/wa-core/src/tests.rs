use super::*;

use std::io::Write;

use whatsapp_rust::buffa::MessageField;
use whatsapp_rust::wacore::types::events::LazyHistorySync;

fn history_event(sync: wa::HistorySync) -> Event {
    let raw = whatsapp_rust::waproto::codec::history_sync_to_vec(&sync);
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(&raw).expect("compress");
    let compressed = encoder.finish().expect("finish");
    let lazy = LazyHistorySync::new(compressed.into(), raw.len(), 0, None, None);
    Event::HistorySync(Box::new(lazy))
}

fn conversation(id: &str) -> wa::Conversation {
    wa::Conversation {
        id: id.to_owned(),
        conversation_timestamp: Some(1_700_000_000),
        ..Default::default()
    }
}

fn text_message(chat: &str, id: &str, body: &str) -> wa::HistorySyncMsg {
    wa::HistorySyncMsg {
        message: MessageField::some(wa::WebMessageInfo {
            key: MessageField::some(wa::MessageKey {
                remote_jid: Some(chat.to_owned()),
                from_me: Some(false),
                id: Some(id.to_owned()),
                participant: None,
            }),
            message: MessageField::some(wa::Message {
                conversation: Some(body.to_owned()),
                ..Default::default()
            }),
            message_timestamp: Some(1_700_000_100),
            push_name: Some("Alice".to_owned()),
            ..Default::default()
        }),
        msg_order_id: None,
    }
}

#[test]
fn extracts_mentions_from_media_context() {
    let message = wa::Message {
        image_message: MessageField::some(wa::message::ImageMessage {
            context_info: MessageField::some(wa::ContextInfo {
                mentioned_jid: vec!["6281234567890@s.whatsapp.net".to_owned()],
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    };
    assert_eq!(
        mentioned_jids(&message),
        vec!["6281234567890@s.whatsapp.net".to_owned()]
    );
}

#[test]
fn extracts_quoted_reply_context() {
    let message = wa::Message {
        image_message: MessageField::some(wa::message::ImageMessage {
            context_info: MessageField::some(wa::ContextInfo {
                stanza_id: Some("orig-1".to_owned()),
                participant: Some("6281234567890@s.whatsapp.net".to_owned()),
                quoted_message: MessageField::some(wa::Message {
                    conversation: Some("the quoted text".to_owned()),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    };
    let names = Mutex::new(HashMap::new());
    let aliases = Mutex::new(HashMap::new());
    let contacts = Mutex::new(HashMap::new());
    let reply = reply_view(&message, &names, &aliases, &contacts).expect("reply");
    assert_eq!(reply.id, "orig-1");
    assert_eq!(reply.kind, "text");
    assert_eq!(reply.text, "the quoted text");
}

#[test]
fn quoted_documents_show_their_file_name() {
    let message = wa::Message {
        extended_text_message: MessageField::some(wa::message::ExtendedTextMessage {
            text: Some("this one?".to_owned()),
            context_info: MessageField::some(wa::ContextInfo {
                stanza_id: Some("doc-1".to_owned()),
                participant: Some("6281234567890@s.whatsapp.net".to_owned()),
                quoted_message: MessageField::some(wa::Message {
                    document_message: MessageField::some(wa::message::DocumentMessage {
                        file_name: Some("sample-report.pdf".to_owned()),
                        page_count: Some(3),
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    };
    let names = Mutex::new(HashMap::new());
    let aliases = Mutex::new(HashMap::new());
    let contacts = Mutex::new(HashMap::new());
    let reply = reply_view(&message, &names, &aliases, &contacts).expect("reply");
    assert_eq!(reply.kind, "document");
    assert_eq!(reply.text, "sample-report.pdf (3 pages)");
}

struct Harness {
    chats: Mutex<HashMap<String, Jid>>,
    names: Mutex<HashMap<String, String>>,
    aliases: Mutex<HashMap<String, String>>,
    canon: Mutex<HashMap<String, String>>,
    contacts: Mutex<HashMap<String, String>>,
    messages: Mutex<HashMap<String, CachedMedia>>,
    rx: mpsc::UnboundedReceiver<UiEvent>,
    tx: mpsc::UnboundedSender<UiEvent>,
}

impl Harness {
    fn new() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        Self {
            chats: Mutex::new(HashMap::new()),
            names: Mutex::new(HashMap::new()),
            aliases: Mutex::new(HashMap::new()),
            canon: Mutex::new(HashMap::new()),
            contacts: Mutex::new(HashMap::new()),
            messages: Mutex::new(HashMap::new()),
            rx,
            tx,
        }
    }

    fn feed(&self, sync: wa::HistorySync) {
        dispatch(
            None,
            &history_event(sync),
            None,
            None,
            &self.chats,
            &self.names,
            &self.aliases,
            &self.canon,
            &self.contacts,
            std::path::Path::new("."),
            &self.messages,
            std::path::Path::new("."),
            &Mutex::new(HashMap::new()),
            &Mutex::new(Vec::<(String, i64)>::new()),
            &Mutex::new(std::collections::HashSet::new()),
            &Mutex::new(std::collections::HashSet::new()),
            &self.tx,
        );
    }

    /// Dispatch a status "view" receipt and return the UI events it produced.
    fn feed_status_view(&mut self, id: &str, viewer: &str) -> Vec<UiEvent> {
        use whatsapp_rust::types::events::Receipt;
        use whatsapp_rust::wacore::types::message::MessageSource;
        let receipt = Receipt::builder()
            .source(MessageSource {
                chat: viewer.parse().expect("viewer jid"),
                sender: viewer.parse().expect("viewer jid"),
                ..Default::default()
            })
            .message_ids(vec![id.to_owned()])
            .timestamp(chrono::Utc::now())
            .r#type(ReceiptType::Other("view".into()))
            .offline(false)
            .build();
        let own_statuses = Mutex::new(std::collections::HashSet::from([id.to_owned()]));
        self.feed_receipt(receipt, Some(&own_statuses))
    }

    /// Dispatch a chat message receipt and return the UI events it produced.
    fn feed_receipt(
        &mut self,
        receipt: whatsapp_rust::types::events::Receipt,
        own_statuses: Option<&Mutex<std::collections::HashSet<String>>>,
    ) -> Vec<UiEvent> {
        let empty = Mutex::new(std::collections::HashSet::new());
        dispatch(
            None,
            &Event::Receipt(receipt),
            None,
            None,
            &self.chats,
            &self.names,
            &self.aliases,
            &self.canon,
            &self.contacts,
            std::path::Path::new("."),
            &self.messages,
            std::path::Path::new("."),
            &Mutex::new(HashMap::new()),
            &Mutex::new(Vec::<(String, i64)>::new()),
            &Mutex::new(std::collections::HashSet::new()),
            own_statuses.unwrap_or(&empty),
            &self.tx,
        );
        self.drain()
    }

    fn drain(&mut self) -> Vec<UiEvent> {
        let mut events = Vec::new();
        while let Ok(event) = self.rx.try_recv() {
            events.push(event);
        }
        events
    }

    /// Dispatch one protocol event through the real dispatcher and keep the
    /// UI events queued for `drain()`.
    fn dispatch_event(&mut self, event: Event) {
        dispatch(
            None,
            &event,
            None,
            None,
            &self.chats,
            &self.names,
            &self.aliases,
            &self.canon,
            &self.contacts,
            std::path::Path::new("."),
            &self.messages,
            std::path::Path::new("."),
            &Mutex::new(HashMap::new()),
            &Mutex::new(Vec::<(String, i64)>::new()),
            &Mutex::new(std::collections::HashSet::new()),
            &Mutex::new(std::collections::HashSet::new()),
            &self.tx,
        );
    }
}

#[test]
fn contact_numbers_are_normalized() {
    assert_eq!(
        normalize_phone("+62 812-3456 7890").expect("valid number"),
        "6281234567890"
    );
    assert!(normalize_phone("123").is_err(), "too short");
    assert!(normalize_phone("1234567890123456").is_err(), "too long");
}

#[test]
fn parses_group_participants() {
    assert!(parse_jids(&[]).is_err());
    assert!(parse_jids(&["not-a-jid".to_owned()]).is_err());
    let jids = parse_jids(&[
        "628111@s.whatsapp.net".to_owned(),
        "not-a-jid".to_owned(),
        "628222:12@s.whatsapp.net".to_owned(),
    ])
    .expect("valid participants");
    assert_eq!(jids.len(), 2);
}

#[test]
fn reactions_target_their_message() {
    let mut harness = Harness::new();
    let reaction = wa::Message {
        reaction_message: MessageField::some(wa::message::ReactionMessage {
            key: MessageField::some(wa::MessageKey {
                remote_jid: Some("628111@s.whatsapp.net".to_owned()),
                from_me: Some(false),
                id: Some("TARGET".to_owned()),
                participant: None,
            }),
            text: Some("\u{1F44D}".to_owned()),
            ..Default::default()
        }),
        ..Default::default()
    };

    dispatch(
        None,
        &batch(vec![inbound("628111@s.whatsapp.net", "R1", reaction)]),
        None,
        None,
        &harness.chats,
        &harness.names,
        &harness.aliases,
        &harness.canon,
        &harness.contacts,
        std::path::Path::new("."),
        &harness.messages,
        std::path::Path::new("."),
        &Mutex::new(HashMap::new()),
        &Mutex::new(Vec::<(String, i64)>::new()),
        &Mutex::new(std::collections::HashSet::new()),
        &Mutex::new(std::collections::HashSet::new()),
        &harness.tx,
    );

    let reaction = harness
        .drain()
        .into_iter()
        .find_map(|event| match event {
            UiEvent::Reaction {
                id,
                sender,
                from_me,
                emoji,
                ..
            } => Some((id, sender, from_me, emoji)),
            _ => None,
        })
        .expect("a reaction event");
    assert_eq!(reaction.0, "TARGET");
    assert_eq!(reaction.1, "628111@s.whatsapp.net");
    assert!(!reaction.2);
    assert_eq!(reaction.3, "\u{1F44D}");
}

#[test]
fn link_preview_reads_whatsapp_metadata() {
    let message = wa::Message {
        extended_text_message: MessageField::some(wa::message::ExtendedTextMessage {
            text: Some("https://youtu.be/x".to_owned()),
            matched_text: Some("https://youtu.be/x".to_owned()),
            title: Some("A video".to_owned()),
            description: Some("about the video".to_owned()),
            jpeg_thumbnail: Some(vec![0xFF, 0xD8, 0xFF, 0xD9]),
            ..Default::default()
        }),
        ..Default::default()
    };

    let link = link_preview(&message).expect("embedded preview");
    assert_eq!(link.url, "https://youtu.be/x");
    assert_eq!(link.title, "A video");
    assert_eq!(link.description, "about the video");
    assert!(
        link.thumb
            .as_deref()
            .is_some_and(|thumb| thumb.starts_with("data:image/jpeg;base64,")),
        "the thumbnail is exposed as a data URI"
    );

    assert!(link_preview(&wa::Message::default()).is_none());
}

#[test]
fn history_link_message_emits_the_embedded_preview() {
    let mut harness = Harness::new();
    let link_msg = wa::HistorySyncMsg {
        message: MessageField::some(wa::WebMessageInfo {
            key: MessageField::some(wa::MessageKey {
                remote_jid: Some("628111@s.whatsapp.net".to_owned()),
                from_me: Some(false),
                id: Some("LINK1".to_owned()),
                participant: None,
            }),
            message: MessageField::some(wa::Message {
                extended_text_message: MessageField::some(wa::message::ExtendedTextMessage {
                    text: Some("https://youtu.be/x".to_owned()),
                    matched_text: Some("https://youtu.be/x".to_owned()),
                    title: Some("A video".to_owned()),
                    description: Some("about the video".to_owned()),
                    jpeg_thumbnail: Some(vec![0xFF, 0xD8, 0xFF, 0xD9]),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            message_timestamp: Some(1_700_000_100),
            push_name: Some("Alice".to_owned()),
            ..Default::default()
        }),
        msg_order_id: None,
    };
    let chat = wa::Conversation {
        messages: vec![link_msg],
        ..conversation("628111@s.whatsapp.net")
    };
    harness.feed(wa::HistorySync {
        conversations: vec![chat],
        ..Default::default()
    });

    let link = harness
        .drain()
        .into_iter()
        .find_map(|event| match event {
            UiEvent::HistoryMessage(message) => message.link,
            _ => None,
        })
        .expect("the history message carries the preview");
    assert_eq!(link.url, "https://youtu.be/x");
    assert_eq!(link.title, "A video");
    assert!(link.thumb.is_some(), "the JPEG thumbnail is inlined");
}

#[test]
fn history_replay_emits_metadata_and_messages() {
    let mut harness = Harness::new();

    let chat = conversation("628111@s.whatsapp.net");
    let chat = wa::Conversation {
        messages: vec![text_message("628111@s.whatsapp.net", "MSG1", "hello there")],
        ..chat
    };
    let sync = wa::HistorySync {
        conversations: vec![chat],
        ..Default::default()
    };

    harness.feed(sync);
    let events = harness.drain();

    assert!(
        events.iter().any(
            |event| matches!(event, UiEvent::Chat { jid, .. } if jid == "628111@s.whatsapp.net")
        ),
        "chat metadata emitted"
    );
    let message = events
        .iter()
        .find_map(|event| match event {
            UiEvent::HistoryMessage(view) => Some(view),
            _ => None,
        })
        .expect("history message emitted");
    assert_eq!(message.text, "hello there");
    assert_eq!(message.sender_name.as_deref(), Some("Alice"));
    assert_eq!(message.timestamp, 1_700_000_100_000);
}

#[test]
fn push_names_in_a_later_chunk_rename_known_chats() {
    let mut harness = Harness::new();

    let first = wa::HistorySync {
        conversations: vec![conversation("628111@s.whatsapp.net")],
        ..Default::default()
    };
    harness.feed(first);

    let second = wa::HistorySync {
        pushnames: vec![wa::Pushname {
            id: Some("999@lid".to_owned()),
            pushname: Some("Alice".to_owned()),
        }],
        phone_number_to_lid_mappings: vec![wa::PhoneNumberToLIDMapping {
            pn_jid: Some("628111@s.whatsapp.net".to_owned()),
            lid_jid: Some("999@lid".to_owned()),
        }],
        ..Default::default()
    };
    harness.feed(second);

    let events = harness.drain();
    assert!(
        events
            .iter()
            .any(|event| matches!(event, UiEvent::ChatName { name, .. } if name == "Alice")),
        "late push name reaches the already announced chat: {events:?}"
    );
}

#[test]
fn media_messages_expose_download_params() {
    let image = wa::Message {
        image_message: MessageField::some(wa::message::ImageMessage {
            direct_path: Some("/v/t62/photo".to_owned()),
            media_key: Some(vec![7u8; 32]),
            file_sha256: Some(vec![1u8; 32]),
            file_enc_sha256: Some(vec![2u8; 32]),
            file_length: Some(4096),
            mimetype: Some("image/jpeg".to_owned()),
            ..Default::default()
        }),
        ..Default::default()
    };
    let (params, extension) = media_params(&image).expect("image is downloadable");
    assert_eq!(extension, "jpg");
    assert_eq!(params.direct_path, "/v/t62/photo");
    assert_eq!(params.file_length, 4096);

    let video = wa::Message {
        video_message: MessageField::some(wa::message::VideoMessage {
            direct_path: Some("/v/t62/clip".to_owned()),
            media_key: Some(vec![8u8; 32]),
            file_sha256: Some(vec![3u8; 32]),
            file_enc_sha256: Some(vec![4u8; 32]),
            file_length: Some(900_000),
            mimetype: Some("video/mp4".to_owned()),
            ..Default::default()
        }),
        ..Default::default()
    };
    assert_eq!(
        media_params(&video).expect("video is downloadable").1,
        "mp4"
    );

    let broken = wa::Message {
        image_message: MessageField::some(wa::message::ImageMessage {
            direct_path: Some("/v/t62/missing-keys".to_owned()),
            ..Default::default()
        }),
        ..Default::default()
    };
    assert!(
        media_params(&broken).is_none(),
        "messages without media keys cannot be downloaded"
    );
}

#[test]
fn document_messages_expose_the_file_card() {
    let document = wa::Message {
        document_message: MessageField::some(wa::message::DocumentMessage {
            file_name: Some("sample-report.pdf".to_owned()),
            title: Some("sample-report".to_owned()),
            mimetype: Some("application/pdf".to_owned()),
            file_length: Some(2_097_152),
            page_count: Some(3),
            jpeg_thumbnail: Some(vec![0xFF, 0xD8, 0xFF, 0xD9]),
            direct_path: Some("/v/t62/document".to_owned()),
            media_key: Some(vec![7u8; 32]),
            file_sha256: Some(vec![1u8; 32]),
            file_enc_sha256: Some(vec![2u8; 32]),
            ..Default::default()
        }),
        ..Default::default()
    };
    let view = document_view(&document).expect("document metadata");
    assert_eq!(view.name, "sample-report.pdf");
    assert_eq!(view.mime, "application/pdf");
    assert_eq!(view.size, 2_097_152);
    assert_eq!(view.pages, 3);
    assert!(
        view.thumb
            .as_deref()
            .is_some_and(|thumb| thumb.starts_with("data:image/jpeg;base64,")),
        "the document thumbnail is exposed as a data URI"
    );
    assert_eq!(
        media_params(&document).expect("document is downloadable").1,
        "pdf"
    );

    // Older clients only set the title; the card falls back to it.
    let titled = wa::Message {
        document_message: MessageField::some(wa::message::DocumentMessage {
            title: Some("spreadsheet.xlsx".to_owned()),
            ..Default::default()
        }),
        ..Default::default()
    };
    let view = document_view(&titled).expect("title fallback");
    assert_eq!(view.name, "spreadsheet.xlsx");
    assert_eq!(view.pages, 0);

    assert!(document_view(&wa::Message::default()).is_none());
}

#[test]
fn audio_view_carries_duration_ptt_and_waveform() {
    let voice = wa::Message {
        audio_message: MessageField::some(wa::message::AudioMessage {
            seconds: Some(17),
            ptt: Some(true),
            waveform: Some(vec![0u8, 40, 120, 255, 60]),
            ..Default::default()
        }),
        ..Default::default()
    };
    let view = audio_view(&voice).expect("audio metadata");
    assert_eq!(view.seconds, 17);
    assert!(view.ptt);
    assert_eq!(view.waveform, vec![0u8, 40, 120, 255, 60]);

    // A plain audio file (not recorded in-app) has no waveform and no voice dot.
    let music = wa::Message {
        audio_message: MessageField::some(wa::message::AudioMessage {
            mimetype: Some("audio/mpeg".to_owned()),
            ..Default::default()
        }),
        ..Default::default()
    };
    let view = audio_view(&music).expect("audio metadata");
    assert_eq!(view.seconds, 0);
    assert!(!view.ptt);
    assert!(view.waveform.is_empty());

    assert!(audio_view(&wa::Message::default()).is_none());
}

/// The live message path exposes the audio bubble through `UiEvent::Message`.
#[test]
fn live_audio_messages_carry_the_player_metadata() {
    let mut harness = Harness::new();

    let audio = wa::Message {
        audio_message: MessageField::some(wa::message::AudioMessage {
            seconds: Some(42),
            ptt: Some(true),
            ..Default::default()
        }),
        ..Default::default()
    };

    dispatch(
        None,
        &batch(vec![inbound("628111@s.whatsapp.net", "AUD1", audio)]),
        None,
        None,
        &harness.chats,
        &harness.names,
        &harness.aliases,
        &harness.canon,
        &harness.contacts,
        std::path::Path::new("."),
        &harness.messages,
        std::path::Path::new("."),
        &Mutex::new(HashMap::new()),
        &Mutex::new(Vec::<(String, i64)>::new()),
        &Mutex::new(std::collections::HashSet::new()),
        &Mutex::new(std::collections::HashSet::new()),
        &harness.tx,
    );

    let events = harness.drain();
    let player = events
        .iter()
        .find_map(|event| match event {
            UiEvent::Message(message) if message.id == "AUD1" => Some(message),
            _ => None,
        })
        .and_then(|message| message.audio.as_ref())
        .expect("live audio carries player metadata");
    assert_eq!(player.seconds, 42);
    assert!(player.ptt);
}

#[test]
fn live_document_messages_carry_the_file_card() {
    let mut harness = Harness::new();

    let document = wa::Message {
        document_message: MessageField::some(wa::message::DocumentMessage {
            file_name: Some("sample-report.pdf".to_owned()),
            mimetype: Some("application/pdf".to_owned()),
            file_length: Some(2_097_152),
            page_count: Some(3),
            caption: Some("Data sheet".to_owned()),
            ..Default::default()
        }),
        ..Default::default()
    };

    dispatch(
        None,
        &batch(vec![inbound("628111@s.whatsapp.net", "DOC1", document)]),
        None,
        None,
        &harness.chats,
        &harness.names,
        &harness.aliases,
        &harness.canon,
        &harness.contacts,
        std::path::Path::new("."),
        &harness.messages,
        std::path::Path::new("."),
        &Mutex::new(HashMap::new()),
        &Mutex::new(Vec::<(String, i64)>::new()),
        &Mutex::new(std::collections::HashSet::new()),
        &Mutex::new(std::collections::HashSet::new()),
        &harness.tx,
    );

    let events = harness.drain();
    let card = events
        .iter()
        .find_map(|event| match event {
            UiEvent::Message(message) if message.id == "DOC1" => Some(message),
            _ => None,
        })
        .and_then(|message| message.document.as_ref())
        .expect("live document carries a file card");
    assert_eq!(card.name, "sample-report.pdf");
    assert_eq!(card.mime, "application/pdf");
    assert_eq!(card.pages, 3);
    assert_eq!(card.size, 2_097_152);
}

/// A reaction or reply sent against a status update points its context at
/// `status@broadcast`; the UI renders those with the official "· Status" card,
/// so the flag has to survive the decode (and must not fire for plain replies).
#[test]
fn status_interactions_are_marked_for_the_status_card() {
    let mut harness = Harness::new();

    let status_body = wa::Message {
        image_message: MessageField::some(wa::message::ImageMessage {
            mimetype: Some("image/jpeg".to_owned()),
            ..Default::default()
        }),
        ..Default::default()
    };
    let reaction = wa::Message {
        extended_text_message: MessageField::some(wa::message::ExtendedTextMessage {
            text: Some("❤".to_owned()),
            context_info: MessageField::some(wa::ContextInfo {
                remote_jid: Some("status@broadcast".to_owned()),
                stanza_id: Some("STATUS1".to_owned()),
                participant: Some("628111@s.whatsapp.net".to_owned()),
                quoted_message: MessageField::some(status_body),
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    };
    harness.dispatch_event(batch(vec![inbound(
        "628111@s.whatsapp.net",
        "REACT1",
        reaction,
    )]));

    let plain = wa::Message {
        extended_text_message: MessageField::some(wa::message::ExtendedTextMessage {
            text: Some("hai".to_owned()),
            context_info: MessageField::some(wa::ContextInfo {
                remote_jid: Some("628111@s.whatsapp.net".to_owned()),
                stanza_id: Some("MSG1".to_owned()),
                quoted_message: MessageField::some(wa::Message {
                    conversation: Some("yo".to_owned()),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    };
    harness.dispatch_event(batch(vec![inbound(
        "628111@s.whatsapp.net",
        "REPLY1",
        plain,
    )]));

    let rows: Vec<_> = harness
        .drain()
        .iter()
        .filter_map(|event| match event {
            UiEvent::Message(message) => Some((
                message.id.as_str().to_owned(),
                message.status_quote,
                message.reply_to.as_ref().map(|reply| reply.kind.clone()),
            )),
            _ => None,
        })
        .collect();
    assert_eq!(
        rows,
        vec![
            ("REACT1".to_owned(), true, Some("image".to_owned())),
            ("REPLY1".to_owned(), false, Some("text".to_owned())),
        ]
    );
}

/// A reply to a status travels as an extended text whose context quotes the
/// status, so every client renders it with the "· Status" card. Without a body
/// to quote the reply stays plain (no half-formed quote).
#[test]
fn status_replies_quote_the_status() {
    use crate::status::status_reply_body;

    let author: Jid = "628111@s.whatsapp.net".parse().expect("author");
    let quoted = wa::Message {
        image_message: MessageField::some(wa::message::ImageMessage {
            mimetype: Some("image/jpeg".to_owned()),
            ..Default::default()
        }),
        ..Default::default()
    };

    let with_quote = status_reply_body("STATUS9", &author, "keren", Some(quoted));
    let context = with_quote
        .get_base_message()
        .extended_text_message
        .as_option()
        .and_then(|body| body.context_info.as_option())
        .expect("the reply quotes the status");
    assert_eq!(context.remote_jid.as_deref(), Some("status@broadcast"));
    assert_eq!(context.stanza_id.as_deref(), Some("STATUS9"));
    assert_eq!(
        context.participant.as_deref(),
        Some("628111@s.whatsapp.net")
    );
    assert!(
        context.quoted_message.as_option().is_some(),
        "the status body rides along as the quote"
    );
    assert!(
        crate::util::quotes_status(&with_quote),
        "the reply must wear the status card"
    );

    let plain = status_reply_body("STATUS9", &author, "keren", None);
    assert!(!crate::util::quotes_status(&plain));
}

fn inbound(chat: &str, id: &str, body: wa::Message) -> InboundMessage {
    let mut info = MessageInfo::default();
    info.source.chat = chat.parse().expect("chat jid");
    info.source.sender = "628111@s.whatsapp.net".parse().expect("sender jid");
    info.id = id.to_owned();
    info.push_name = "Alice".to_owned();
    info.timestamp = chrono::Utc::now();

    InboundMessage::builder()
        .message(Arc::new(body))
        .info(Arc::new(info))
        .build()
}

fn batch(messages: Vec<InboundMessage>) -> Event {
    Event::Messages(
        MessageBatch::builder()
            .messages(messages.into())
            .origin(BatchOrigin::Live)
            .build(),
    )
}

#[test]
fn revokes_and_edits_update_local_messages() {
    let mut harness = Harness::new();

    let revoke = wa::Message {
        protocol_message: MessageField::some(wa::message::ProtocolMessage {
            key: MessageField::some(wa::MessageKey {
                id: Some("TARGET-REVOKE".to_owned()),
                ..Default::default()
            }),
            r#type: Some(wa::message::protocol_message::Type::REVOKE),
            ..Default::default()
        }),
        ..Default::default()
    };

    let edit = wa::Message {
        protocol_message: MessageField::some(wa::message::ProtocolMessage {
            key: MessageField::some(wa::MessageKey {
                id: Some("TARGET-EDIT".to_owned()),
                ..Default::default()
            }),
            r#type: Some(wa::message::protocol_message::Type::MESSAGE_EDIT),
            edited_message: MessageField::some(wa::Message {
                conversation: Some("edited text".to_owned()),
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    };

    dispatch(
        None,
        &batch(vec![
            inbound("628111@s.whatsapp.net", "P1", revoke),
            inbound("628111@s.whatsapp.net", "P2", edit),
        ]),
        None,
        None,
        &harness.chats,
        &harness.names,
        &harness.aliases,
        &harness.canon,
        &harness.contacts,
        std::path::Path::new("."),
        &harness.messages,
        std::path::Path::new("."),
        &Mutex::new(HashMap::new()),
        &Mutex::new(Vec::<(String, i64)>::new()),
        &Mutex::new(std::collections::HashSet::new()),
        &Mutex::new(std::collections::HashSet::new()),
        &harness.tx,
    );

    let events = harness.drain();
    assert!(
        events.iter().any(
            |event| matches!(event, UiEvent::MessageDeleted { id, .. } if id == "TARGET-REVOKE")
        ),
        "revoke targets the local message"
    );
    assert!(
        events.iter().any(|event| matches!(
            event,
            UiEvent::MessageEdited { id, text, .. }
                if id == "TARGET-EDIT" && text == "edited text"
        )),
        "an edit carries the new text"
    );
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, UiEvent::Message(_))),
        "protocol messages are not rendered as chat bubbles"
    );
}

#[test]
fn deleted_chats_are_reported_for_the_local_archive() {
    use whatsapp_rust::wacore::types::events::DeleteMessageForMeUpdate;

    let mut harness = Harness::new();
    let update = DeleteMessageForMeUpdate::builder()
        .chat_jid("628111@s.whatsapp.net".parse().expect("chat"))
        .message_id("GONE1".to_owned())
        .from_me(false)
        .timestamp(chrono::Utc::now())
        .action(Box::new(
            wa::sync_action_value::DeleteMessageForMeAction::default(),
        ))
        .from_full_sync(false)
        .build();

    dispatch(
        None,
        &Event::DeleteMessageForMeUpdate(update),
        None,
        None,
        &harness.chats,
        &harness.names,
        &harness.aliases,
        &harness.canon,
        &harness.contacts,
        std::path::Path::new("."),
        &harness.messages,
        std::path::Path::new("."),
        &Mutex::new(HashMap::new()),
        &Mutex::new(Vec::<(String, i64)>::new()),
        &Mutex::new(std::collections::HashSet::new()),
        &Mutex::new(std::collections::HashSet::new()),
        &harness.tx,
    );

    let events = harness.drain();
    assert!(
        events.iter().any(|event| matches!(
            event,
            UiEvent::MessageDeleted { chat, id }
                if chat == "628111@s.whatsapp.net" && id == "GONE1"
        )),
        "delete-for-me reaches the UI so the chat copy can be flagged"
    );
}

#[test]
fn deleted_and_cleared_chats_are_reported_for_the_local_archive() {
    use whatsapp_rust::wacore::types::events::{ClearChatUpdate, DeleteChatUpdate};

    let mut harness = Harness::new();

    let deleted = DeleteChatUpdate::builder()
        .jid("628111@s.whatsapp.net".parse().expect("chat"))
        .delete_media(true)
        .timestamp(chrono::Utc::now())
        .action(Box::new(wa::sync_action_value::DeleteChatAction::default()))
        .from_full_sync(false)
        .build();

    dispatch(
        None,
        &Event::DeleteChatUpdate(deleted),
        None,
        None,
        &harness.chats,
        &harness.names,
        &harness.aliases,
        &harness.canon,
        &harness.contacts,
        std::path::Path::new("."),
        &harness.messages,
        std::path::Path::new("."),
        &Mutex::new(HashMap::new()),
        &Mutex::new(Vec::<(String, i64)>::new()),
        &Mutex::new(std::collections::HashSet::new()),
        &Mutex::new(std::collections::HashSet::new()),
        &harness.tx,
    );

    let cleared = ClearChatUpdate::builder()
        .jid("628222@s.whatsapp.net".parse().expect("chat"))
        .delete_starred(true)
        .delete_media(false)
        .timestamp(chrono::Utc::now())
        .action(Box::new(wa::sync_action_value::ClearChatAction::default()))
        .from_full_sync(false)
        .build();

    dispatch(
        None,
        &Event::ClearChatUpdate(cleared),
        None,
        None,
        &harness.chats,
        &harness.names,
        &harness.aliases,
        &harness.canon,
        &harness.contacts,
        std::path::Path::new("."),
        &harness.messages,
        std::path::Path::new("."),
        &Mutex::new(HashMap::new()),
        &Mutex::new(Vec::<(String, i64)>::new()),
        &Mutex::new(std::collections::HashSet::new()),
        &Mutex::new(std::collections::HashSet::new()),
        &harness.tx,
    );

    let events = harness.drain();
    assert!(
        events.iter().any(|event| matches!(
            event,
            UiEvent::ChatDeleted { chat } if chat == "628111@s.whatsapp.net"
        )),
        "a deleted chat reaches the UI so it can be archived"
    );
    assert!(
        events.iter().any(|event| matches!(
            event,
            UiEvent::ChatCleared { chat } if chat == "628222@s.whatsapp.net"
        )),
        "a cleared chat reaches the UI so its history can be kept"
    );

    // The webview contract: both events serialize with a `type` tag.
    let deleted_json = serde_json::to_value(UiEvent::ChatDeleted {
        chat: "628111@s.whatsapp.net".to_owned(),
    })
    .expect("chat_deleted serializes");
    assert_eq!(deleted_json["type"], "chat_deleted");
    assert_eq!(deleted_json["chat"], "628111@s.whatsapp.net");

    let cleared_json = serde_json::to_value(UiEvent::ChatCleared {
        chat: "628222@s.whatsapp.net".to_owned(),
    })
    .expect("chat_cleared serializes");
    assert_eq!(cleared_json["type"], "chat_cleared");
    assert_eq!(cleared_json["chat"], "628222@s.whatsapp.net");
}

/// A view-once fanout (`<unavailable type="view_once">`) becomes a
/// `view_once` bubble like the official placeholder, and Hide fanouts (the
/// message already opened elsewhere) surface nothing.
#[test]
fn view_once_fanouts_become_placeholder_bubbles() {
    use whatsapp_rust::wacore::types::events::{
        DecryptFailMode, UnavailableType, UndecryptableMessage,
    };
    use whatsapp_rust::wacore::types::message::MessageInfo;

    let mut harness = Harness::new();

    let build = |id: &str, mode: DecryptFailMode| {
        let mut info = MessageInfo::default();
        info.source.chat = "628788@s.whatsapp.net".parse().expect("chat");
        info.source.sender = "628788@s.whatsapp.net".parse().expect("sender");
        info.id = id.to_owned();
        info.push_name = "Sasi".to_owned();
        info.timestamp = chrono::Utc::now();
        UndecryptableMessage::builder()
            .info(std::sync::Arc::new(info))
            .is_unavailable(true)
            .unavailable_type(UnavailableType::ViewOnce)
            .decrypt_fail_mode(mode)
            .build()
    };

    harness.dispatch_event(Event::UndecryptableMessage(build(
        "VO1",
        DecryptFailMode::Show,
    )));
    harness.dispatch_event(Event::UndecryptableMessage(build(
        "VO2",
        DecryptFailMode::Hide,
    )));

    // The fanout's push name joins the names map for the sender's user part,
    // so the placeholder and later folds label the real sender.
    assert_eq!(
        harness.names.lock().expect("names").get("628788"),
        Some(&"Sasi".to_owned())
    );

    let events = harness.drain();
    assert_eq!(events.len(), 1, "Hide fanouts are not surfaced");
    let bubble = match &events[0] {
        UiEvent::Message(view) if view.kind == "view_once" && view.id == "VO1" => view,
        other => panic!("expected a view_once bubble, got {other:?}"),
    };
    assert_eq!(bubble.chat, "628788@s.whatsapp.net".to_owned());
    assert!(bubble.sender_name.as_deref() == Some("Sasi"));
    assert!(!bubble.from_me);
    let json = serde_json::to_value(events[0].clone()).expect("view_once serializes");
    assert_eq!(json["type"], "message");
    assert_eq!(json["kind"], "view_once");
}

/// A view-once fanout inside a GROUP must never plant the sender's push name
/// under the group's id: the group's display name is its subject, and a push
/// name stored there renamed whole groups (Alice🧢 instead of the subject).
#[test]
fn fanout_push_names_never_rename_groups() {
    use whatsapp_rust::wacore::types::events::{
        DecryptFailMode, UnavailableType, UndecryptableMessage,
    };
    use whatsapp_rust::wacore::types::message::MessageInfo;

    let mut harness = Harness::new();

    let mut info = MessageInfo::default();
    info.source.chat = "120363000000000001@g.us".parse().expect("group jid");
    info.source.sender = "6281234567890@s.whatsapp.net".parse().expect("sender jid");
    info.source.is_group = true;
    info.id = "VO-GROUP".to_owned();
    info.push_name = "Alice🧢".to_owned();
    info.timestamp = chrono::Utc::now();
    harness.dispatch_event(Event::UndecryptableMessage(
        UndecryptableMessage::builder()
            .info(std::sync::Arc::new(info))
            .is_unavailable(true)
            .unavailable_type(UnavailableType::ViewOnce)
            .decrypt_fail_mode(DecryptFailMode::Show)
            .build(),
    ));

    // The sender's own push name is still registered for sender attribution…
    assert_eq!(
        harness.names.lock().expect("names").get("6281234567890"),
        Some(&"Alice🧢".to_owned())
    );
    // …but the GROUP's id must stay clean: its name is the subject, not a
    // member's push name.
    assert!(
        !harness
            .names
            .lock()
            .expect("names")
            .contains_key("120363000000000001"),
        "a group id must never receive a member's push name"
    );

    // A later group message resolves its chat_name from the raw id (the
    // subject arrives via metadata), NOT from the polluting push name.
    let mut info = MessageInfo::default();
    info.source.chat = "120363000000000001@g.us".parse().expect("group jid");
    info.source.sender = "628111@s.whatsapp.net".parse().expect("sender jid");
    info.source.is_group = true;
    info.id = "G-MSG".to_owned();
    info.timestamp = chrono::Utc::now();
    let body = wa::Message {
        conversation: Some("hello group".to_owned()),
        ..Default::default()
    };
    harness.dispatch_event(batch(vec![
        InboundMessage::builder()
            .message(std::sync::Arc::new(body))
            .info(std::sync::Arc::new(info))
            .build(),
    ]));

    let renamed = harness
        .drain()
        .iter()
        .find_map(|event| match event {
            UiEvent::Message(view) if view.id == "G-MSG" => Some(view.chat_name.clone()),
            _ => None,
        })
        .expect("group message reached the UI");
    assert_ne!(renamed, "Alice🧢", "group renamed by a member's push name");
}

/// A pin change made on any other client (phone or desktop) arrives as
/// `Event::PinUpdate` and reaches the UI as `chat_pin`, so the pinned row
/// reorders live for everyone.
#[test]
fn pin_updates_from_other_clients_reach_the_ui() {
    use whatsapp_rust::wacore::types::events::PinUpdate;

    let mut harness = Harness::new();
    let chat: Jid = "6281234567890@s.whatsapp.net".parse().expect("chat");

    let pinned = PinUpdate::builder()
        .jid(chat.clone())
        .timestamp(chrono::Utc::now())
        .action(Box::new(wa::sync_action_value::PinAction {
            pinned: Some(true),
        }))
        .from_full_sync(false)
        .build();
    harness.dispatch_event(Event::PinUpdate(pinned));

    // Unpinning the same chat flips the flag on the same event path.
    let unpinned = PinUpdate::builder()
        .jid(chat.clone())
        .timestamp(chrono::Utc::now())
        .action(Box::new(wa::sync_action_value::PinAction {
            pinned: Some(false),
        }))
        .from_full_sync(false)
        .build();
    harness.dispatch_event(Event::PinUpdate(unpinned));

    let events = harness.drain();
    let pins: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            UiEvent::ChatPin { chat, pinned, .. } => Some((chat.as_str(), *pinned)),
            _ => None,
        })
        .collect();
    assert_eq!(
        pins,
        vec![
            ("6281234567890@s.whatsapp.net", true),
            ("6281234567890@s.whatsapp.net", false)
        ]
    );

    // The chat gets registered so later messages/events share the fold.
    assert!(
        harness
            .chats
            .lock()
            .expect("chats")
            .contains_key("6281234567890@s.whatsapp.net"),
        "a pin event registers the chat for later folds"
    );

    let json = serde_json::to_value(UiEvent::ChatPin {
        chat: "6281234567890@s.whatsapp.net".to_owned(),
        pinned: true,
        timestamp: 0,
    })
    .expect("chat_pin serializes");
    assert_eq!(json["type"], "chat_pin");
    assert_eq!(json["pinned"], true);
}

/// A star made on any other client arrives as `Event::StarUpdate` and reaches
/// the UI as `star_message`, so the starred list stays in step for everyone.
#[test]
fn star_updates_from_other_clients_reach_the_ui() {
    use whatsapp_rust::wacore::types::events::StarUpdate;

    let mut harness = Harness::new();
    let chat: Jid = "120363000000000000@g.us".parse().expect("chat");
    let author: Jid = "6281234567890@s.whatsapp.net".parse().expect("author");

    let starred = StarUpdate::builder()
        .chat_jid(chat.clone())
        .maybe_participant_jid(Some(author.clone()))
        .message_id("3EB0STAR0001".to_owned())
        .from_me(false)
        .timestamp(chrono::Utc::now())
        .action(Box::new(wa::sync_action_value::StarAction {
            starred: Some(true),
        }))
        .from_full_sync(false)
        .build();
    harness.dispatch_event(Event::StarUpdate(starred));

    // Unstarring rides the same event with the flag flipped.
    let unstarred = StarUpdate::builder()
        .chat_jid(chat.clone())
        .maybe_participant_jid(None)
        .message_id("3EB0STAR0002".to_owned())
        .from_me(true)
        .timestamp(chrono::Utc::now())
        .action(Box::new(wa::sync_action_value::StarAction {
            starred: Some(false),
        }))
        .from_full_sync(false)
        .build();
    harness.dispatch_event(Event::StarUpdate(unstarred));

    let events = harness.drain();
    let stars: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            UiEvent::StarMessage { id, starred, .. } => Some((id.as_str(), *starred)),
            _ => None,
        })
        .collect();
    assert_eq!(stars, vec![("3EB0STAR0001", true), ("3EB0STAR0002", false)]);

    let json = serde_json::to_value(UiEvent::StarMessage {
        chat: "120363000000000000@g.us".to_owned(),
        id: "3EB0STAR0001".to_owned(),
        starred: true,
        timestamp: 0,
    })
    .expect("star_message serializes");
    assert_eq!(json["type"], "star_message");
    assert_eq!(json["starred"], true);
}

/// Reply quoting recovers a view-once fanout: the quoting client embeds the
/// original body (view-once wrapper around image/video with the media key) in
/// `contextInfo.quotedMessage`, and the wa-core flow caches it under the
/// quoted id and surfaces a normal kept bubble for the target.
#[test]
fn reply_quotes_recover_view_once_media() {
    let mut harness = Harness::new();

    let quoted_image = wa::Message {
        image_message: MessageField::some(wa::message::ImageMessage {
            caption: Some("with animeh".to_owned()),
            mimetype: Some("image/jpeg".to_owned()),
            url: Some("https://mmg.whatsapp.net/v/t62.7118-24/photo.jpg".to_owned()),
            direct_path: Some("/v/t62.7118-24/photo.jpg".to_owned()),
            media_key: Some(vec![9u8; 32]),
            file_enc_sha256: Some(vec![7u8; 32]),
            file_sha256: Some(vec![6u8; 32]),
            file_length: Some(12_345),
            ..Default::default()
        }),
        ..Default::default()
    };
    let reply = wa::Message {
        conversation: Some("Buset comifuro?".to_owned()),
        extended_text_message: MessageField::some(wa::message::ExtendedTextMessage {
            text: Some("Buset comifuro?".to_owned()),
            context_info: MessageField::some(wa::ContextInfo {
                stanza_id: Some("SASI-IMG".to_owned()),
                participant: Some("628788@s.whatsapp.net".to_owned()),
                quoted_message: MessageField::some(wa::Message {
                    view_once_message_v2: MessageField::some(wa::message::FutureProofMessage {
                        message: MessageField::some(quoted_image),
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    };

    dispatch(
        None,
        &batch(vec![inbound(
            "6281234567890@s.whatsapp.net",
            "REPLY1",
            reply,
        )]),
        None,
        None,
        &harness.chats,
        &harness.names,
        &harness.aliases,
        &harness.canon,
        &harness.contacts,
        std::path::Path::new("."),
        &harness.messages,
        std::path::Path::new("."),
        &Mutex::new(HashMap::new()),
        &Mutex::new(Vec::<(String, i64)>::new()),
        &Mutex::new(std::collections::HashSet::new()),
        &Mutex::new(std::collections::HashSet::new()),
        &harness.tx,
    );

    let events = harness.drain();
    // The target bubble upgrades to a normal kept image on the same id (the
    // placeholder merge path in the UI can swap it in), and the reply bubble
    // itself still arrives.
    let recovered = events
        .iter()
        .find_map(|event| match event {
            UiEvent::Message(view) if view.id == "SASI-IMG" => Some(view),
            _ => None,
        })
        .expect("quoted view-once body recovered as its own bubble");
    assert_eq!(recovered.kind, "image");
    assert_eq!(recovered.text, "with animeh");
    assert!(!recovered.from_me);
    // The recovered copy belongs to the quoted message's author
    // (`contextInfo.participant`), never to the replier.
    assert_eq!(recovered.sender.as_deref(), Some("628788@s.whatsapp.net"));

    let cached = harness.messages.lock().expect("messages");
    let entry = cached.get("SASI-IMG").expect("quoted body cached");
    let has_media_key = entry
        .message
        .get_base_message()
        .image_message
        .as_option()
        .and_then(|image| image.media_key.clone())
        .is_some();
    assert!(
        has_media_key,
        "the cached body keeps the view-once media key for fetch_media"
    );
}

/// Our own account replying to someone else's view-once must render the
/// recovered copy on the OTHER side (incoming, authored by the quoted
/// participant) — the official client shows the kept view-once there, not as
/// an outgoing bubble from us.
#[test]
fn own_reply_keeps_the_recovered_view_once_on_the_quoted_side() {
    let mut harness = Harness::new();

    let quoted_image = wa::Message {
        image_message: MessageField::some(wa::message::ImageMessage {
            caption: Some("It should render like this".to_owned()),
            mimetype: Some("image/jpeg".to_owned()),
            url: Some("https://mmg.whatsapp.net/v/t62.7118-24/photo.jpg".to_owned()),
            direct_path: Some("/v/t62.7118-24/photo.jpg".to_owned()),
            media_key: Some(vec![9u8; 32]),
            file_enc_sha256: Some(vec![7u8; 32]),
            file_sha256: Some(vec![6u8; 32]),
            file_length: Some(12_345),
            ..Default::default()
        }),
        ..Default::default()
    };
    let reply = wa::Message {
        conversation: Some("Nice".to_owned()),
        extended_text_message: MessageField::some(wa::message::ExtendedTextMessage {
            text: Some("Nice".to_owned()),
            context_info: MessageField::some(wa::ContextInfo {
                stanza_id: Some("YI-IMG".to_owned()),
                participant: Some("628788@s.whatsapp.net".to_owned()),
                quoted_message: MessageField::some(wa::Message {
                    view_once_message_v2: MessageField::some(wa::message::FutureProofMessage {
                        message: MessageField::some(quoted_image),
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    };

    let mut info = MessageInfo::default();
    info.source.chat = "120363000000000002@g.us".parse().expect("group jid");
    info.source.sender = "628999@s.whatsapp.net".parse().expect("own jid");
    info.source.is_from_me = true;
    info.source.is_group = true;
    info.id = "REPLY-OUT".to_owned();
    info.timestamp = chrono::Utc::now();
    let outgoing = InboundMessage::builder()
        .message(Arc::new(reply))
        .info(Arc::new(info))
        .build();

    dispatch(
        None,
        &batch(vec![outgoing]),
        None,
        None,
        &harness.chats,
        &harness.names,
        &harness.aliases,
        &harness.canon,
        &harness.contacts,
        std::path::Path::new("."),
        &harness.messages,
        std::path::Path::new("."),
        &Mutex::new(HashMap::new()),
        &Mutex::new(Vec::<(String, i64)>::new()),
        &Mutex::new(std::collections::HashSet::new()),
        &Mutex::new(std::collections::HashSet::new()),
        &harness.tx,
    );

    let events = harness.drain();
    let recovered = events
        .iter()
        .find_map(|event| match event {
            UiEvent::Message(view) if view.id == "YI-IMG" => Some(view),
            _ => None,
        })
        .expect("quoted view-once recovered from our own reply");
    // The whole point: not from me even though the REPLY was ours.
    assert!(!recovered.from_me, "recovered copy must stay incoming");
    assert_eq!(recovered.sender.as_deref(), Some("628788@s.whatsapp.net"));
    assert_eq!(recovered.kind, "image");
    assert_eq!(recovered.text, "It should render like this");
    assert!(recovered.is_group);
}

/// In a direct chat the quote carries no participant: the quoted message then
/// belongs to the opposite side of the reply (we replied to their view-once,
/// so the kept copy is theirs, sent in this chat).
#[test]
fn direct_reply_without_participant_quotes_the_peer() {
    let mut harness = Harness::new();

    let quoted_image = wa::Message {
        image_message: MessageField::some(wa::message::ImageMessage {
            caption: Some("dm view once".to_owned()),
            media_key: Some(vec![9u8; 32]),
            ..Default::default()
        }),
        ..Default::default()
    };
    let reply = wa::Message {
        conversation: Some("wow".to_owned()),
        extended_text_message: MessageField::some(wa::message::ExtendedTextMessage {
            text: Some("wow".to_owned()),
            context_info: MessageField::some(wa::ContextInfo {
                stanza_id: Some("DM-IMG".to_owned()),
                quoted_message: MessageField::some(wa::Message {
                    view_once_message_v2: MessageField::some(wa::message::FutureProofMessage {
                        message: MessageField::some(quoted_image),
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    };

    let mut info = MessageInfo::default();
    info.source.chat = "6281234567890@s.whatsapp.net".parse().expect("peer jid");
    info.source.sender = "628999@s.whatsapp.net".parse().expect("own jid");
    info.source.is_from_me = true;
    info.id = "REPLY-DM".to_owned();
    info.timestamp = chrono::Utc::now();
    let outgoing = InboundMessage::builder()
        .message(Arc::new(reply))
        .info(Arc::new(info))
        .build();

    dispatch(
        None,
        &batch(vec![outgoing]),
        None,
        None,
        &harness.chats,
        &harness.names,
        &harness.aliases,
        &harness.canon,
        &harness.contacts,
        std::path::Path::new("."),
        &harness.messages,
        std::path::Path::new("."),
        &Mutex::new(HashMap::new()),
        &Mutex::new(Vec::<(String, i64)>::new()),
        &Mutex::new(std::collections::HashSet::new()),
        &Mutex::new(std::collections::HashSet::new()),
        &harness.tx,
    );

    let events = harness.drain();
    let recovered = events
        .iter()
        .find_map(|event| match event {
            UiEvent::Message(view) if view.id == "DM-IMG" => Some(view),
            _ => None,
        })
        .expect("quoted view-once recovered from the DM reply");
    assert!(!recovered.from_me);
    assert_eq!(
        recovered.sender.as_deref(),
        Some("6281234567890@s.whatsapp.net")
    );
}

#[test]
fn status_messages_become_status_updates_not_chats() {
    let mut harness = Harness::new();

    let mut info = MessageInfo::default();
    info.source.chat = "status@broadcast".parse().expect("status chat jid");
    info.source.sender = "628111@s.whatsapp.net".parse().expect("sender jid");
    info.id = "STATUS1".to_owned();
    info.push_name = "Alice".to_owned();
    info.timestamp = chrono::Utc::now();

    let body = wa::Message {
        conversation: Some("hello status".to_owned()),
        ..Default::default()
    };

    let inbound = InboundMessage::builder()
        .message(Arc::new(body))
        .info(Arc::new(info))
        .build();

    let event = Event::Messages(
        MessageBatch::builder()
            .messages(vec![inbound].into())
            .origin(BatchOrigin::Live)
            .build(),
    );

    dispatch(
        None,
        &event,
        None,
        None,
        &harness.chats,
        &harness.names,
        &harness.aliases,
        &harness.canon,
        &harness.contacts,
        std::path::Path::new("."),
        &harness.messages,
        std::path::Path::new("."),
        &Mutex::new(HashMap::new()),
        &Mutex::new(Vec::<(String, i64)>::new()),
        &Mutex::new(std::collections::HashSet::new()),
        &Mutex::new(std::collections::HashSet::new()),
        &harness.tx,
    );

    let events = harness.drain();
    let status = events
        .iter()
        .find_map(|event| match event {
            UiEvent::Status(status) => Some(status),
            _ => None,
        })
        .expect("status update is emitted");

    assert_eq!(status.text, "hello status");
    assert_eq!(status.name, "Alice");
    assert!(
        harness.chats.lock().expect("chats").is_empty(),
        "a status must not create a chat bar"
    );
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, UiEvent::Chat { .. } | UiEvent::Message(_))),
        "status updates never look like chat messages"
    );
}

#[test]
fn protocol_stanzas_are_not_history_messages() {
    let mut harness = Harness::new();

    let chat = conversation("628111@s.whatsapp.net");
    let chat = wa::Conversation {
        messages: vec![wa::HistorySyncMsg {
            message: MessageField::some(wa::WebMessageInfo {
                key: MessageField::some(wa::MessageKey {
                    remote_jid: Some("628111@s.whatsapp.net".to_owned()),
                    from_me: Some(true),
                    id: Some("PROTO".to_owned()),
                    participant: None,
                }),
                message: MessageField::none(),
                message_timestamp: Some(1_700_000_200),
                ..Default::default()
            }),
            msg_order_id: None,
        }],
        ..chat
    };
    let sync = wa::HistorySync {
        conversations: vec![chat],
        ..Default::default()
    };
    harness.feed(sync);

    let events = harness.drain();
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, UiEvent::HistoryMessage(_))),
        "messages without a body must be skipped"
    );
}

#[test]
fn status_background_is_hex_from_argb() {
    let message = wa::Message {
        extended_text_message: MessageField::some(wa::message::ExtendedTextMessage {
            text: Some("hello".into()),
            background_argb: Some(0xFF7E_C8F0),
            ..Default::default()
        }),
        ..Default::default()
    };
    assert_eq!(status_background(&message).as_deref(), Some("#7ec8f0"));

    // WhatsApp omits the colour when it is the default; nothing is painted then.
    let plain = wa::Message {
        extended_text_message: MessageField::some(wa::message::ExtendedTextMessage {
            text: Some("hello".into()),
            background_argb: Some(0),
            ..Default::default()
        }),
        ..Default::default()
    };
    assert_eq!(status_background(&plain), None);
}

#[test]
fn status_view_receipt_emits_status_viewed() {
    let mut harness = Harness::new();
    harness
        .contacts
        .lock()
        .expect("contacts")
        .insert("628111".to_owned(), "Reyhan Nugraha".to_owned());
    let events = harness.feed_status_view("3EB0STATUS1", "628111@s.whatsapp.net");
    let viewed: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            UiEvent::StatusViewed { id, jid, name, .. } => {
                Some((id.as_str(), jid.as_str(), name.as_str()))
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        viewed,
        vec![("3EB0STATUS1", "628111@s.whatsapp.net", "Reyhan Nugraha")]
    );
}

/// A peer's read receipt (the one that turns our own sent ticks blue) is
/// forwarded as `UiEvent::Receipt` with the message ids intact, so the webview
/// can mark `status: "read"`. Delivered and played receipts use the same path.
#[test]
fn chat_read_receipt_moves_our_sent_state() {
    let mut harness = Harness::new();

    for (kind, expected) in [
        (ReceiptType::Delivered, "delivered"),
        (ReceiptType::Read, "read"),
        (ReceiptType::Played, "played"),
    ] {
        use whatsapp_rust::types::events::Receipt;
        use whatsapp_rust::wacore::types::message::MessageSource;
        let receipt = Receipt::builder()
            .source(MessageSource {
                // The reader's own id, exactly like WhatsApp narrates reads:
                // a LID sender speaks for accounts that carry one.
                chat: "111111111111111@lid".parse().expect("reader jid"),
                sender: "111111111111111@lid".parse().expect("reader jid"),
                ..Default::default()
            })
            .message_ids(vec!["SENT1".to_owned(), "SENT2".to_owned()])
            .timestamp(chrono::Utc::now())
            .r#type(kind)
            .offline(false)
            .build();

        let events = harness.feed_receipt(receipt, None);
        let applied = events.iter().filter_map(|event| match event {
            UiEvent::Receipt { chat, ids, status } => Some((chat.as_str(), ids, status.as_str())),
            _ => None,
        });
        let applied: Vec<_> = applied
            .filter(|(chat, ids, status)| {
                *status == expected && !ids.is_empty() && chat.contains("@lid")
            })
            .collect();
        assert_eq!(applied.len(), 1, "{expected} receipt reaches the UI once");
    }
}

#[test]
fn channel_fanout_envelope_is_peeled() {
    use crate::channels::decode_newsletter_body;

    fn length_delimited(field: u32, payload: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut key = u64::from(field) << 3 | 2;
        while key >= 0x80 {
            out.push((key as u8 & 0x7f) | 0x80);
            key >>= 7;
        }
        out.push(key as u8);
        let mut length = payload.len() as u64;
        while length >= 0x80 {
            out.push((length as u8 & 0x7f) | 0x80);
            length >>= 7;
        }
        out.push(length as u8);
        out.extend_from_slice(payload);
        out
    }

    let message = wa::Message {
        conversation: Some("caption text".to_owned()),
        ..Default::default()
    };
    let encoded = whatsapp_rust::waproto::codec::message_to_vec(&message);

    // A channel `<plaintext>` wraps the real Message in one fanout field
    // (101 for media posts, 106 for text posts).
    for field in [101u32, 106] {
        let envelope = length_delimited(field, &encoded);
        let decoded = decode_newsletter_body(&envelope).expect("envelope peeled");
        assert_eq!(decoded.conversation.as_deref(), Some("caption text"));
    }
    // A bare Message keeps decoding directly.
    let plain = decode_newsletter_body(&encoded).expect("plain body");
    assert_eq!(plain.conversation.as_deref(), Some("caption text"));
}

#[test]
fn channel_page_decodes_plaintext_posts() {
    use crate::channels::decode_channel_page;
    use whatsapp_rust::NodeBuilder;

    let body = |text: &str| {
        let message = wa::Message {
            conversation: Some(text.to_owned()),
            ..Default::default()
        };
        whatsapp_rust::waproto::codec::message_to_vec(&message)
    };
    let node = NodeBuilder::new("messages")
        .children([
            NodeBuilder::new("message")
                .attr("id", "A")
                .attr("server_id", "42")
                .attr("t", "1700000000")
                .attr("type", "text")
                .children([
                    NodeBuilder::new("plaintext")
                        .bytes(body("halo channel"))
                        .build(),
                    // Counters the desktop app shows under the card.
                    NodeBuilder::new("views_count")
                        .attr("count", "95000")
                        .build(),
                    NodeBuilder::new("forwards_count")
                        .attr("count", "14000")
                        .build(),
                    NodeBuilder::new("reactions")
                        .children([
                            NodeBuilder::new("reaction")
                                .attr("code", "❤️")
                                .attr("count", "300")
                                .build(),
                            NodeBuilder::new("reaction")
                                .attr("code", "👍")
                                .attr("count", "900")
                                .build(),
                        ])
                        .build(),
                ])
                .build(),
            NodeBuilder::new("message")
                .attr("id", "B")
                .attr("server_id", "40")
                .attr("t", "1699990000")
                .attr("type", "text")
                .children([NodeBuilder::new("plaintext")
                    .bytes(body("post kedua"))
                    .build()])
                .build(),
            // No server_id: cannot be paged or keyed, so it is skipped.
            NodeBuilder::new("message")
                .attr("id", "C")
                .children([NodeBuilder::new("plaintext").bytes(body("skipped")).build()])
                .build(),
        ])
        .build();
    let contacts: Mutex<HashMap<String, String>> = Mutex::new(HashMap::new());
    let jid: Jid = "120363000000000000@newsletter".parse().expect("jid");

    let page = decode_channel_page(&node.as_node_ref(), &jid, "Channel", &contacts);
    assert_eq!(page.posts.len(), 2, "posts: {:?}", page.posts.len());
    assert_eq!(page.posts[0].0.text, "halo channel");
    assert_eq!(page.posts[1].0.text, "post kedua");
    assert_eq!(page.posts[0].0.chat, jid.to_string());
    assert!(page.posts[0].1.is_none(), "text posts carry no media");
    // The cursor is the oldest server_id in the page.
    assert_eq!(page.cursor, Some(40));
    // A page carries every `<message>` the server returned, even skipped ones,
    // so the caller can tell whether more history exists.
    assert_eq!(page.messages, 3);
    let stats = page.posts[0].0.stats.as_ref().expect("counters decoded");
    assert_eq!(stats.views, Some(95000));
    assert_eq!(stats.forwards, Some(14000));
    assert_eq!(stats.reactions.len(), 2);
    // Sorted by count so the busiest reaction pill comes first.
    assert_eq!(stats.reactions[0].code, "👍");
    assert_eq!(stats.reactions[0].count, 900);
    assert!(
        page.posts[1].0.stats.is_none(),
        "no counters on the second post"
    );
}
