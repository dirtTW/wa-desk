//! Protocol event translation into UI events.

use super::*;

/// Translate one protocol event into zero or more UI events.
#[allow(clippy::too_many_arguments)]
pub(crate) fn dispatch(
    client: Option<&Arc<Client>>,
    event: &Event,
    own_pn: Option<&Jid>,
    own_lid: Option<&Jid>,
    chats: &Mutex<HashMap<String, Jid>>,
    names: &Mutex<HashMap<String, String>>,
    aliases: &Mutex<HashMap<String, String>>,
    canon: &Mutex<HashMap<String, String>>,
    contacts: &Mutex<HashMap<String, String>>,
    contacts_path: &std::path::Path,
    messages: &Mutex<HashMap<String, CachedMedia>>,
    msg_dir: &std::path::Path,
    incoming_calls: &Mutex<HashMap<String, IncomingCall>>,
    history_targets: &Mutex<Vec<(String, i64)>>,
    requested_stubs: &Mutex<std::collections::HashSet<String>>,
    own_statuses: &Mutex<std::collections::HashSet<String>>,
    tx: &mpsc::UnboundedSender<UiEvent>,
) {
    // The "message yourself" chat is addressed by our own LID or PN.
    let is_self = |jid: &Jid| {
        own_pn.is_some_and(|own| own.user == jid.user && own.server == jid.server)
            || own_lid.is_some_and(|own| own.user == jid.user && own.server == jid.server)
    };

    // History may address a person by LID while live traffic uses the PN (or
    // the reverse). Key DMs by their phone-number form so both land in one
    // conversation, like WhatsApp Web.
    let canonical = |jid: &Jid| -> Jid {
        if is_self(jid) {
            return own_lid.or(own_pn).cloned().unwrap_or_else(|| jid.clone());
        }
        if jid.server == Server::Lid
            && let Some(pn) = canon.lock().expect("canon lock").get(jid.user.as_str())
        {
            return Jid::new(pn.clone(), Server::Pn);
        }
        // Receipts, revokes and edits often narrate the peer's LID while the
        // chat itself was keyed by its phone number (no history chunk brought
        // the mapping yet). When the LID chat does not exist but a
        // phone-number chat does, fold to it so those events can land.
        if jid.server == Server::Lid
            && !chats.lock().expect("chats lock").contains_key(jid.to_string().as_str())
            && let Some(pn) = aliases.lock().expect("aliases lock").get(jid.user.as_str())
        {
            let pn_jid = Jid::new(pn.clone(), Server::Pn);
            if chats.lock().expect("chats lock").contains_key(pn_jid.to_string().as_str()) {
                return pn_jid;
            }
        }
        jid.clone()
    };

    match event {
        Event::PairingQrCode(qr) => {
            let _ = tx.send(UiEvent::Qr {
                svg: render_qr(&qr.code),
                timeout_secs: qr.timeout.as_secs(),
            });
        }
        Event::PairingCode(code) => {
            let _ = tx.send(UiEvent::PairCode {
                code: code.code.clone(),
            });
        }
        // The rotation budget is spent: WhatsApp Web stops rotating and leaves
        // the "click to reload" decision to the surface, so the login screen
        // gets told instead of showing a dead code.
        Event::PairingQrCodesExhausted(_) => {
            let _ = tx.send(UiEvent::QrExpired);
        }
        Event::PairSuccess(info) => {
            let _ = tx.send(UiEvent::Paired {
                jid: info.id.to_string(),
                name: info.business_name.clone(),
            });
        }
        Event::PairError(err) => {
            let _ = tx.send(UiEvent::PairError {
                message: err.error.clone(),
            });
        }
        Event::PairingCodeError(err) => {
            let _ = tx.send(UiEvent::PairError {
                message: err.error.clone(),
            });
        }
        Event::Connected(_) => {
            let _ = tx.send(UiEvent::Connected);
        }
        Event::Disconnected(disconnected) => {
            let _ = tx.send(UiEvent::Disconnected {
                reason: format!("{:?}", disconnected.reason),
            });
        }
        Event::StreamReplaced(_) => {
            let _ = tx.send(UiEvent::Disconnected {
                reason: "replaced by another device".into(),
            });
        }
        Event::ConnectFailure(failure) => {
            let _ = tx.send(UiEvent::Disconnected {
                reason: failure
                    .message
                    .clone()
                    .unwrap_or_else(|| format!("{:?}", failure.reason)),
            });
        }
        Event::LoggedOut(out) => {
            let _ = tx.send(UiEvent::LoggedOut {
                reason: format!("{:?}", out.reason),
                message: out.logout_message.as_ref().and_then(|m| m.header.clone()),
            });
        }
        Event::HistorySync(sync) => {
            // History chunks carry conversation metadata only; message bodies
            // stay encrypted and are not decoded here.
            let Some(history) = sync.get() else {
                return;
            };

            // Names and PN/LID aliases may arrive in a later chunk than the
            // conversations they belong to, so both live in shared maps that
            // accumulate across chunks.
            {
                let mut names = names.lock().expect("names lock");
                for entry in &history.pushnames {
                    if let (Some(id), Some(name)) = (entry.id.as_deref(), entry.pushname.as_deref())
                        && !name.is_empty()
                    {
                        names.insert(user_part(id).to_owned(), name.to_owned());
                    }
                }
            }

            {
                let mut aliases = aliases.lock().expect("aliases lock");
                let mut canon = canon.lock().expect("canon lock");
                for mapping in &history.phone_number_to_lid_mappings {
                    if let (Some(pn), Some(lid)) =
                        (mapping.pn_jid.as_deref(), mapping.lid_jid.as_deref())
                    {
                        let pn = user_part(pn).to_owned();
                        let lid = user_part(lid).to_owned();
                        canon.insert(lid.clone(), pn.clone());
                        aliases.insert(pn.clone(), lid.clone());
                        aliases.insert(lid, pn);
                    }
                }
            }

            tracing::debug!(
                conversations = history.conversations.len(),
                push_names = history.pushnames.len(),
                aliases = history.phone_number_to_lid_mappings.len(),
                "history chunk"
            );

            // Bodies omitted from the chunk are requested in one batch after
            // the conversations are processed; the phone answers each with a
            // placeholder resend that flows through the normal message event.
            let mut stubs: Vec<(Jid, String, bool, Option<Jid>)> = Vec::new();

            for conversation in &history.conversations {
                let Ok(raw_jid) = conversation.id.parse::<Jid>() else {
                    continue;
                };
                let jid = canonical(&raw_jid);
                let key = jid.to_string();
                chats
                    .lock()
                    .expect("chats lock")
                    .insert(key.clone(), jid.clone());

                // The on-demand reply is addressed by the peer's canonical id.
                // A chat clicked under its LID is still the same conversation;
                // tell the UI to fold that duplicate row into this one.
                {
                    let now = now_ms();
                    let mut targets = history_targets.lock().expect("history targets lock");
                    targets.retain(|(_, at)| now.saturating_sub(*at) < 120_000);
                    if let Some(index) = targets.iter().position(|(requested, _)| {
                        requested == &key
                            || user_part(requested) == raw_jid.user.as_str()
                            || aliases
                                .lock()
                                .expect("aliases lock")
                                .get(raw_jid.user.as_str())
                                .is_some_and(|alias| requested == alias)
                    }) {
                        let (requested, _) = targets.remove(index);
                        if requested != key {
                            chats
                                .lock()
                                .expect("chats lock")
                                .insert(requested.clone(), jid.clone());
                            let _ = tx.send(UiEvent::ChatAlias {
                                from: requested,
                                to: key.clone(),
                            });
                        }
                    }
                }

                tracing::debug!(
                    raw = %conversation.id,
                    chat = %key,
                    messages = conversation.messages.len(),
                    "history conversation"
                );

                let user = jid.user.as_str();
                let name = if is_self(&jid) {
                    "You".to_owned()
                } else {
                    conversation
                        .name
                        .as_deref()
                        .filter(|name| !name.is_empty())
                        .map(str::to_owned)
                        .or_else(|| look_up_name(user, names, aliases, contacts))
                        .unwrap_or_else(|| user.to_owned())
                };

                if name != user {
                    names
                        .lock()
                        .expect("names lock")
                        .insert(user.to_owned(), name.clone());
                }

                // Status updates are stories. History sync can carry the last
                // 24 h of status@broadcast even when the app was closed, so
                // route those messages to the status page instead of dropping
                // the whole broadcast conversation.
                if key == "status@broadcast" {
                    for item in &conversation.messages {
                        let Some(info) = item.message.as_option() else {
                            continue;
                        };
                        let Some(keys) = info.key.as_option() else {
                            continue;
                        };
                        let Some(body) = info.message.as_option() else {
                            continue;
                        };
                        let Some(id) = keys.id.as_deref().filter(|id| !id.is_empty()) else {
                            continue;
                        };
                        let text = message_text(body.get_base_message()).unwrap_or_default();
                        let kind = media_kind(body.get_base_message()).unwrap_or("text");
                        if kind == "text" && text.is_empty() {
                            continue;
                        }
                        let from_me = keys.from_me.unwrap_or(false);
                        let mut participant = keys
                            .participant
                            .as_deref()
                            .or(info.participant.as_deref())
                            .and_then(|raw| raw.parse::<Jid>().ok());
                        if let Some(author) = participant.as_mut() {
                            author.device = 0;
                            author.agent = 0;
                        }
                        // Link posts that ship a big CDN thumbnail also need
                        // their protobuf cached, or `link_thumbnail` cannot
                        // re-fetch the image later.
                        let wants_link_thumb =
                            link_preview(body).is_some_and(|link| link.thumb_path.is_some());
                        if kind != "text" || wants_link_thumb {
                            remember_message(
                                messages,
                                msg_dir,
                                id,
                                Arc::new(body.clone()),
                                &jid,
                                from_me,
                                participant.as_ref(),
                            );
                        }
                        let mut sender = participant.clone().unwrap_or_else(|| jid.clone());
                        sender.device = 0;
                        sender.agent = 0;
                        let push_name = info.push_name.as_deref().unwrap_or("");
                        let name = if is_self(&sender) {
                            "You".to_owned()
                        } else {
                            look_up_name(sender.user.as_str(), names, aliases, contacts)
                                .or_else(|| (!push_name.is_empty()).then(|| push_name.to_owned()))
                                .unwrap_or_else(|| sender.user.to_string())
                        };
                        let _ = tx.send(UiEvent::Status(StatusUpdate {
                            id: id.to_owned(),
                            sender: sender.to_string(),
                            name,
                            kind: kind.to_owned(),
                            text,
                            background: status_background(body),
                            timestamp: (info.message_timestamp.unwrap_or(0) as i64)
                                .saturating_mul(1000),
                            from_me,
                        }));
                        if from_me {
                            own_statuses
                                .lock()
                                .expect("statuses lock")
                                .insert(id.to_owned());
                        }
                    }
                    continue;
                }

                // The status feed is not a chat; WhatsApp Web lists it elsewhere.
                // Channels (@newsletter) are kept: they get their own panel.
                if key.ends_with("@broadcast") {
                    continue;
                }

                let timestamp = conversation
                    .conversation_timestamp
                    .or(conversation.last_msg_timestamp)
                    .unwrap_or(0) as i64;

                let _ = tx.send(UiEvent::Chat {
                    jid: key.clone(),
                    name: name.clone(),
                    is_group: key.ends_with("@g.us"),
                    timestamp: timestamp.saturating_mul(1000),
                });

                // History payloads carry message bodies in the clear (they are
                // protected by the encrypted transport, not per-message Signal),
                // so the backfill can be rendered like WhatsApp Web does.
                let mut skipped = 0usize;
                for item in &conversation.messages {
                    let Some(info) = item.message.as_option() else {
                        skipped += 1;
                        tracing::debug!("history item without message info");
                        continue;
                    };
                    let Some(keys) = info.key.as_option() else {
                        skipped += 1;
                        tracing::debug!("history item without key");
                        continue;
                    };
                    let Some(body) = info.message.as_option() else {
                        // Key-only entry: the phone left the body out of the
                        // chunk. Ask it once, in the batch after this loop.
                        if let Some(keys) = info.key.as_option()
                            && let Some(id) = keys.id.as_deref().filter(|id| !id.is_empty())
                        {
                            let fresh = {
                                let mut requested =
                                    requested_stubs.lock().expect("requested stubs lock");
                                requested.len() < 10_000 && requested.insert(id.to_owned())
                            };
                            if fresh {
                                let from_me = keys.from_me.unwrap_or(false);
                                // WA Web omits the participant on our own
                                // messages; the phone uses the group +
                                // fromMe + id to locate them.
                                let participant = (!from_me)
                                    .then(|| {
                                        keys.participant
                                            .as_deref()
                                            .or(info.participant.as_deref())
                                            .and_then(|raw| raw.parse::<Jid>().ok())
                                    })
                                    .flatten();
                                tracing::debug!(
                                    id,
                                    from_me,
                                    participant = participant.as_ref().map(|jid| jid.to_string()),
                                    "history stub"
                                );
                                stubs.push((raw_jid.clone(), id.to_owned(), from_me, participant));
                            }
                        }
                        skipped += 1;
                        tracing::debug!(
                            id = keys.id.as_deref().unwrap_or(""),
                            "history item without body"
                        );
                        continue;
                    };
                    let Some(id) = keys.id.as_deref() else {
                        skipped += 1;
                        tracing::debug!("history item without id");
                        continue;
                    };
                    if id.is_empty() {
                        skipped += 1;
                        continue;
                    }

                    let text = message_text(body.get_base_message()).unwrap_or_default();
                    let media = media_kind(body.get_base_message());
                    if text.is_empty() && media.is_none() {
                        skipped += 1;
                        tracing::debug!(id, "history item with no text or media");
                        continue;
                    }
                    let link = link_preview(body);

                    let from_me = keys.from_me.unwrap_or(false);
                    let participant = keys
                        .participant
                        .as_deref()
                        .or(info.participant.as_deref())
                        .and_then(|raw| raw.parse::<Jid>().ok());

                    // Reactions are not bubbles: they decorate the target.
                    if let Some(reaction) = body.get_base_message().reaction_message.as_option() {
                        let target = reaction
                            .key
                            .as_option()
                            .and_then(|key| key.id.clone())
                            .unwrap_or_default();
                        if !target.is_empty() {
                            let _ = tx.send(UiEvent::Reaction {
                                chat: key.clone(),
                                id: target,
                                sender: participant
                                    .as_ref()
                                    .map(|jid| jid.to_string())
                                    .unwrap_or_default(),
                                from_me,
                                emoji: reaction.text.clone().unwrap_or_default(),
                                timestamp: reaction.sender_timestamp_ms.unwrap_or_else(|| {
                                    (info.message_timestamp.unwrap_or(0) as i64)
                                        .saturating_mul(1000)
                                }),
                            });
                        }
                        continue;
                    }

                    if media.is_some() {
                        remember_message(
                            messages,
                            msg_dir,
                            id,
                            Arc::new(body.clone()),
                            &jid,
                            from_me,
                            participant.as_ref(),
                        );
                    }

                    let sender = participant.as_ref().map(|jid| jid.to_string());
                    let push_name = info.push_name.as_deref().unwrap_or_default();
                    let sender_name = (!from_me)
                        .then(|| {
                            participant.as_ref().and_then(|jid| {
                                look_up_name(jid.user.as_str(), names, aliases, contacts)
                            })
                        })
                        .flatten()
                        .or_else(|| {
                            (!from_me && !push_name.is_empty()).then(|| push_name.to_owned())
                        });
                    let sender_phone = (!from_me)
                        .then(|| participant.as_ref().and_then(|jid| phone_for(jid, aliases)))
                        .flatten();

                    tracing::debug!(
                        chat = %key,
                        id,
                        ts = info.message_timestamp.unwrap_or(0),
                        from_me,
                        "history message"
                    );

                    let _ = tx.send(UiEvent::HistoryMessage(MessageView {
                        chat: key.clone(),
                        chat_name: name.clone(),
                        sender,
                        sender_name,
                        sender_phone,
                        id: id.to_owned(),
                        from_me,
                        is_group: key.ends_with("@g.us"),
                        timestamp: (info.message_timestamp.unwrap_or(0) as i64)
                            .saturating_mul(1000),
                        kind: media.unwrap_or("text").to_owned(),
                        text,
                        mentions: mentioned_jids(body),
                        reply_to: reply_view(body, names, aliases, contacts),
                        link,
                        document: document_view(body),
                        audio: audio_view(body),
                        stats: None,
                        forwarded: is_forwarded(body),
                        once_recovered: false,
                        status_quote: quotes_status(body),
                    }));
                }
                tracing::debug!(
                    chat = %key,
                    total = conversation.messages.len(),
                    skipped,
                    "history conversation messages"
                );
            }

            if !stubs.is_empty()
                && let Some(client) = client
            {
                tracing::info!(count = stubs.len(), "requesting history stub bodies");
                let client = Arc::clone(client);
                tokio::spawn(async move {
                    if let Err(err) = client.send_placeholder_resend_batch(stubs).await {
                        tracing::warn!(%err, "history stub resend failed");
                    }
                });
            }

            // A chunk that carries only push names still has to reach chats we
            // already announced, otherwise they keep their numeric placeholder.
            let known: Vec<String> = chats.lock().expect("chats lock").keys().cloned().collect();
            for jid in known {
                let user = user_part(&jid).to_owned();
                let Some(name) = look_up_name(&user, names, aliases, contacts) else {
                    continue;
                };
                if name == user {
                    continue;
                }
                let _ = tx.send(UiEvent::ChatName {
                    jid: jid.clone(),
                    name,
                });
            }
        }
        Event::DeleteChatUpdate(update) => {
            let _ = tx.send(UiEvent::ChatDeleted {
                chat: canonical(&update.jid).to_string(),
            });
        }
        Event::ClearChatUpdate(update) => {
            let _ = tx.send(UiEvent::ChatCleared {
                chat: canonical(&update.jid).to_string(),
            });
        }
        Event::DeleteMessageForMeUpdate(update) => {
            let _ = tx.send(UiEvent::MessageDeleted {
                chat: canonical(&update.chat_jid).to_string(),
                id: update.message_id.clone(),
            });
        }
        // View-once fanouts (`<unavailable type="view_once">`) and other
        // companion-unreadable content (hosted, bot) never decrypt on a linked
        // device — the phone keeps the content. The official UI still shows a
        // placeholder bubble ("You received a view once message…"), so mirror
        // that instead of silently dropping the fanout.
        Event::UndecryptableMessage(update) => {
            if update.decrypt_fail_mode != whatsapp_rust::wacore::types::events::DecryptFailMode::Show {
                return;
            }
            // Status broadcasts have their own surface; newsletters the same.
            let raw_chat = &update.info.source.chat;
            if matches!(
                raw_chat.server,
                Server::Broadcast | Server::Newsletter
            ) {
                return;
            }
            let kind = match update.unavailable_type {
                whatsapp_rust::wacore::types::events::UnavailableType::ViewOnce => "view_once",
                _ => "unsupported",
            };
            let from_me = update.info.source.is_from_me;
            let chat = canonical(raw_chat);
            chats
                .lock()
                .expect("chats lock")
                .insert(chat.to_string(), chat.clone());
            // Register the fanout's push name the same way live messages do, so
            // the placeholder labels the real sender and later folds resolve.
            // Never plant it under a GROUP chat key: the group's display name
            // comes from its subject, and a sender's push name stored here
            // renamed whole groups (a member's push name instead of the real subject).
            if !from_me && !update.info.push_name.is_empty() {
                let sender_user = update.info.source.sender.user.to_string();
                let mut stored = names.lock().expect("names lock");
                stored
                    .entry(sender_user.clone())
                    .or_insert_with(|| update.info.push_name.clone());
                if chat.user != sender_user && !update.info.source.is_group {
                    stored
                        .entry(chat.user.to_string())
                        .or_insert_with(|| update.info.push_name.clone());
                }
            }
            let chat_name = if is_self(&chat) {
                "You".to_owned()
            } else {
                look_up_name(chat.user.as_str(), names, aliases, contacts)
                    .unwrap_or_else(|| chat.user.to_string())
            };
            let sender_name = (!from_me).then(|| {
                look_up_name(
                    update.info.source.sender.user.as_str(),
                    names,
                    aliases,
                    contacts,
                )
                .unwrap_or_else(|| update.info.push_name.clone())
            });
            let _ = tx.send(UiEvent::Message(MessageView {
                chat: chat.to_string(),
                chat_name: chat_name.clone(),
                sender: (!from_me).then(|| update.info.source.sender.to_string()),
                sender_name,
                sender_phone: (!from_me)
                    .then(|| phone_for(&update.info.source.sender, aliases))
                    .flatten(),
                id: update.info.id.clone(),
                from_me,
                is_group: update.info.source.is_group,
                timestamp: update.info.timestamp.timestamp_millis(),
                kind: kind.to_owned(),
                text: String::new(),
                mentions: Vec::new(),
                reply_to: None,
                link: None,
                document: None,
                audio: None,
                stats: None,
                forwarded: false,
                once_recovered: false,
                status_quote: false,
            }));
        }
        Event::Receipt(receipt) => {
            // A "view" receipt (or a receipt for one of our own stories) means a
            // contact watched that story, not that a chat message was read.
            let is_status_view = matches!(
                &receipt.r#type,
                ReceiptType::Other(kind) if kind == "view"
            ) || receipt
                .message_ids
                .iter()
                .any(|id| own_statuses.lock().expect("statuses lock").contains(id));
            if is_status_view {
                let mut viewer = receipt.source.sender.clone();
                viewer.device = 0;
                viewer.agent = 0;
                // Our own devices acknowledge the story too, and the broadcast
                // chat itself can appear as the sender; neither is a viewer.
                if is_self(&viewer)
                    || viewer.user.as_str() == "status"
                    || matches!(viewer.server, Server::Broadcast | Server::Newsletter)
                {
                    return;
                }
                let name = look_up_name(viewer.user.as_str(), names, aliases, contacts)
                    .unwrap_or_else(|| viewer.user.to_string());
                for id in &receipt.message_ids {
                    let _ = tx.send(UiEvent::StatusViewed {
                        id: id.clone(),
                        jid: viewer.to_string(),
                        name: name.clone(),
                        timestamp: receipt.timestamp.timestamp_millis(),
                    });
                }
                return;
            }
            // Echoes of our own read state for other devices are not useful;
            // everything else moves the ticks on a message we sent.
            let status = match receipt.r#type {
                ReceiptType::Read | ReceiptType::ReadSelf => "read",
                ReceiptType::Played | ReceiptType::PlayedSelf => "played",
                ReceiptType::Delivered => "delivered",
                _ => return,
            };
            if receipt.message_ids.is_empty() {
                return;
            }
            let _ = tx.send(UiEvent::Receipt {
                chat: canonical(&receipt.source.chat).to_string(),
                ids: receipt.message_ids.clone(),
                status: status.to_owned(),
            });
        }
        Event::Messages(batch) => {
            for inbound in batch.messages.iter() {
                let info = &inbound.info;

                // Edits and revokes arrive as protocol messages: apply them to
                // the local copy rather than showing a bubble.
                if let Some(protocol) = inbound.message.protocol_message.as_option() {
                    match protocol.r#type {
                        Some(wa::message::protocol_message::Type::REVOKE) => {
                            if let Some(keys) = protocol.key.as_option()
                                && let Some(target) = keys.id.as_deref()
                                && !target.is_empty()
                            {
                                let _ = tx.send(UiEvent::MessageDeleted {
                                    chat: canonical(&info.source.chat).to_string(),
                                    id: target.to_owned(),
                                });
                            }
                            continue;
                        }
                        Some(wa::message::protocol_message::Type::MESSAGE_EDIT) => {
                            if let (Some(keys), Some(edited)) = (
                                protocol.key.as_option(),
                                protocol.edited_message.as_option(),
                            ) {
                                let id = keys.id.clone().unwrap_or_default();
                                let text = edited
                                    .text_content()
                                    .or_else(|| edited.get_caption())
                                    .unwrap_or_default()
                                    .to_owned();
                                if !id.is_empty() && !text.is_empty() {
                                    let _ = tx.send(UiEvent::MessageEdited {
                                        chat: canonical(&info.source.chat).to_string(),
                                        id,
                                        text,
                                    });
                                }
                            }
                            continue;
                        }
                        _ => {}
                    }
                }

                let raw_chat = &info.source.chat;

                // Status updates are stories, not chats: route them to the
                // status page and never create a conversation for them.
                if raw_chat.server == Server::Broadcast && raw_chat.user == "status" {
                    let mut sender = info.source.sender.clone();
                    // Status authors arrive with a device suffix in some history
                    // chunks; message keys and status entries use the bare JID.
                    sender.device = 0;
                    sender.agent = 0;
                    let text = inbound
                        .message
                        .text_content()
                        .or_else(|| inbound.message.get_caption())
                        .unwrap_or_default()
                        .to_owned();
                    let kind = media_kind(&inbound.message).unwrap_or("text").to_owned();
                    if kind == "text" && text.is_empty() {
                        continue;
                    }
                    let wants_link_thumb =
                        link_preview(&inbound.message).is_some_and(|link| link.thumb_path.is_some());
                    if kind != "text" || wants_link_thumb {
                        remember_message(
                            messages,
                            msg_dir,
                            &info.id,
                            Arc::clone(&inbound.message),
                            raw_chat,
                            info.source.is_from_me,
                            Some(&sender),
                        );
                    }

                    let name = look_up_name(sender.user.as_str(), names, aliases, contacts)
                        .or_else(|| (!info.push_name.is_empty()).then(|| info.push_name.clone()))
                        .unwrap_or_else(|| sender.user.to_string());

                    let _ = tx.send(UiEvent::Status(StatusUpdate {
                        id: info.id.clone(),
                        sender: sender.to_string(),
                        name,
                        kind,
                        text,
                        background: status_background(&inbound.message),
                        timestamp: info.timestamp.timestamp_millis(),
                        from_me: info.source.is_from_me,
                    }));
                    if info.source.is_from_me {
                        own_statuses
                            .lock()
                            .expect("statuses lock")
                            .insert(info.id.clone());
                    }
                    continue;
                }

                let chat = &canonical(raw_chat);
                chats
                    .lock()
                    .expect("chats lock")
                    .insert(chat.to_string(), chat.clone());

                // A reaction targets an existing message; emit it as an
                // annotation instead of a new bubble.
                if let Some(reaction) = inbound
                    .message
                    .get_base_message()
                    .reaction_message
                    .as_option()
                {
                    let target = reaction
                        .key
                        .as_option()
                        .and_then(|key| key.id.clone())
                        .unwrap_or_default();
                    if !target.is_empty() {
                        let _ = tx.send(UiEvent::Reaction {
                            chat: chat.to_string(),
                            id: target,
                            sender: info.source.sender.to_string(),
                            from_me: info.source.is_from_me,
                            emoji: reaction.text.clone().unwrap_or_default(),
                            timestamp: reaction
                                .sender_timestamp_ms
                                .unwrap_or_else(|| info.timestamp.timestamp_millis()),
                        });
                    }
                    continue;
                }

                let from_me = info.source.is_from_me;
                // Our own sends to "Message yourself" arrive wrapped in a
                // `deviceSentMessage`, so read the unwrapped body or the bubble
                // renders empty.
                let base = inbound.message.get_base_message();
                let text = message_text(base).unwrap_or_default();

                // The server's `mediatype` attribute is not always present on
                // the stanza; fall back to the protobuf so an image sent with a
                // caption is not reduced to a text bubble.
                let media = if info.media_type.is_empty() {
                    media_kind(base).map(str::to_owned)
                } else {
                    Some(info.media_type.to_ascii_lowercase())
                };

                // Protocol/notification stanzas (history-sync notices, receipts,
                // key distribution) carry neither text nor media. They are not
                // chat content, so they never reach the UI.
                if text.is_empty() && media.is_none() {
                    continue;
                }

                if media.is_some() {
                    remember_message(
                        messages,
                        msg_dir,
                        &info.id,
                        Arc::clone(&inbound.message),
                        chat,
                        from_me,
                        info.source.is_group.then_some(&info.source.sender),
                    );
                }

                if !from_me && !info.push_name.is_empty() {
                    let mut names = names.lock().expect("names lock");
                    names.insert(info.source.sender.user.to_string(), info.push_name.clone());
                    if !info.source.is_group {
                        names.insert(chat.user.to_string(), info.push_name.clone());
                    }
                }

                // A business sender's envelope carries the verified display
                // name, which beats a push name or a numeric placeholder —
                // but only for the sender and the chat when it is that
                // sender's DM; a group's name is its subject, never a
                // member's verified name.
                if let Some(verified) = info
                    .verified_name
                    .as_ref()
                    .and_then(|verified| verified.name.as_deref())
                    .filter(|name| !name.trim().is_empty())
                {
                    let mut store = names.lock().expect("names lock");
                    if !info.source.is_group {
                        store.insert(chat.user.to_string(), verified.to_owned());
                    }
                    store.insert(info.source.sender.user.to_string(), verified.to_owned());
                }

                let chat_name = if is_self(chat) {
                    "You".to_owned()
                } else {
                    look_up_name(chat.user.as_str(), names, aliases, contacts)
                        .unwrap_or_else(|| chat.user.to_string())
                };

                // Reply-quoting recovers view-once content: the quoting client
                // embeds the original body (media key + direct path included)
                // in `contextInfo.quotedMessage`, exactly the copy the official
                // preview shows. When the target has no cached body (a kept
                // `<unavailable type="view_once">` fanout for which the phone
                // refuses resends), cache the quoted copy under the target id
                // and surface it as a normal kept bubble. A status is not a chat
                // message, so its quote is never recovered as a bubble — the
                // official client only shows it inside the "· Status" card.
                if let Some(context) = context_info(base)
                    && !quotes_status(base)
                    && let Some(quoted_id) = context.stanza_id.clone().filter(|id| !id.is_empty())
                    && let Some(quoted) = context.quoted_message.as_option()
                    && let Some(quoted_kind) = {
                        let quoted_base = quoted.get_base_message();
                        media_kind(quoted_base).filter(|kind| kind != &"text")
                    }
                    // Only recover targets without a cached body: a quote of a
                    // normal message is already covered by the original bubble.
                    && !messages.lock().expect("media lock").contains_key(&quoted_id)
                {
                    let quoted_text = quoted
                        .text_content()
                        .or_else(|| quoted.get_caption())
                        .unwrap_or_default()
                        .to_owned();
                    // The recovered copy keeps the ORIGINAL message's side, not
                    // the reply's: `contextInfo.participant` names who sent the
                    // quoted message (the official client renders the kept
                    // view-once on that side). Without a participant — direct
                    // chats — the quote belongs to the opposite side of the
                    // reply, and the "message yourself" chat is always ours.
                    let participant = context
                        .participant
                        .as_deref()
                        .filter(|p| !p.is_empty())
                        .and_then(|p| p.parse::<Jid>().ok());
                    let quoted_from_me = match &participant {
                        Some(p) => is_self(p),
                        None => is_self(chat) || !from_me,
                    };
                    let quoted_sender = (!quoted_from_me).then(|| {
                        participant.clone().unwrap_or_else(|| {
                            if info.source.is_group {
                                info.source.sender.clone()
                            } else {
                                chat.clone()
                            }
                        })
                    });
                    let sender_name = quoted_sender
                        .as_ref()
                        .and_then(|sender| look_up_name(sender.user.as_str(), names, aliases, contacts));
                    let sender_phone = quoted_sender
                        .as_ref()
                        .and_then(|sender| phone_for(sender, aliases));
                    let recovered = MessageView {
                        chat: chat.to_string(),
                        chat_name: chat_name.clone(),
                        sender: quoted_sender.as_ref().map(|sender| sender.to_string()),
                        sender_name,
                        sender_phone,
                        id: quoted_id,
                        from_me: quoted_from_me,
                        is_group: info.source.is_group,
                        // The protocol carries no quoted timestamp; sit the
                        // recovery right above the reply that carried it.
                        timestamp: info.timestamp.timestamp_millis().saturating_sub(1),
                        kind: quoted_kind.to_owned(),
                        text: quoted_text,
                        mentions: Vec::new(),
                        reply_to: None,
                        link: None,
                        document: document_view(&quoted.clone().into_base_message()),
                        audio: audio_view(quoted),
                        stats: None,
                        forwarded: false,
                        once_recovered: true,
                        status_quote: false,
                    };
                    remember_message(
                        messages,
                        msg_dir,
                        &recovered.id,
                        Arc::new(quoted.clone().into_base_message()),
                        chat,
                        quoted_from_me,
                        info.source
                            .is_group
                            .then_some(quoted_sender.as_ref())
                            .flatten(),
                    );
                    let _ = tx.send(UiEvent::Message(recovered));
                }

                let kind = media.unwrap_or_else(|| "text".to_owned());
                let sender_name = (!from_me).then(|| {
                    look_up_name(info.source.sender.user.as_str(), names, aliases, contacts)
                        .unwrap_or_else(|| info.push_name.clone())
                });
                let sender_phone = (!from_me)
                    .then(|| phone_for(&info.source.sender, aliases))
                    .flatten();

                let _ = tx.send(UiEvent::Message(MessageView {
                    chat: chat.to_string(),
                    chat_name,
                    sender: (!from_me).then(|| info.source.sender.to_string()),
                    sender_name,
                    sender_phone,
                    id: info.id.clone(),
                    from_me,
                    is_group: info.source.is_group,
                    timestamp: info.timestamp.timestamp_millis(),
                    kind,
                    text,
                    mentions: mentioned_jids(&inbound.message),
                    reply_to: reply_view(&inbound.message, names, aliases, contacts),
                    link: link_preview(&inbound.message),
                    document: document_view(&inbound.message),
                    audio: audio_view(&inbound.message),
                    stats: None,
                    forwarded: is_forwarded(&inbound.message),
                    once_recovered: false,
                    status_quote: quotes_status(&inbound.message),
                }));
            }
        }
        Event::ContactUpdate(update) => {
            // The phone's address book reaches linked devices as app-state
            // `contact` mutations: fullName/firstName are the saved name.
            let action = &update.action;
            let name = action
                .full_name
                .as_deref()
                .or(action.first_name.as_deref())
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(str::to_owned);

            let mut keys: Vec<String> = vec![user_part(&update.jid.to_string()).to_owned()];
            for raw in [action.pn_jid.as_deref(), action.lid_jid.as_deref()] {
                if let Some(raw) = raw
                    && !raw.is_empty()
                {
                    keys.push(user_part(raw).to_owned());
                }
            }
            keys.sort();
            keys.dedup();

            {
                let mut store = contacts.lock().expect("contacts lock");
                for key in &keys {
                    match &name {
                        Some(name) => {
                            store.insert(key.clone(), name.clone());
                        }
                        None => {
                            store.remove(key);
                        }
                    }
                }
                save_contacts(contacts_path, &store);
            }

            for key in keys {
                let _ = tx.send(UiEvent::ContactName {
                    jid: key,
                    name: name.clone().unwrap_or_default(),
                });
            }
        }
        // Chat settings (pin/mute/archive) sync through app-state mutations,
        // which reach every linked client — the webview folds them onto its
        // cached chat so the list stays in step with the phone.
        Event::PinUpdate(update) => {
            let jid = canonical(&update.jid);
            chats
                .lock()
                .expect("chats lock")
                .insert(jid.to_string(), jid.clone());
            let _ = tx.send(UiEvent::ChatPin {
                chat: jid.to_string(),
                pinned: update.action.pinned.unwrap_or(false),
                timestamp: update.timestamp.timestamp_millis(),
            });
        }
        Event::ArchiveUpdate(update) => {
            let jid = canonical(&update.jid);
            chats
                .lock()
                .expect("chats lock")
                .insert(jid.to_string(), jid.clone());
            let _ = tx.send(UiEvent::ChatArchive {
                chat: jid.to_string(),
                archived: update.action.archived.unwrap_or(false),
            });
        }
        Event::MuteUpdate(update) => {
            let jid = canonical(&update.jid);
            chats
                .lock()
                .expect("chats lock")
                .insert(jid.to_string(), jid.clone());
            // MuteAction's `muted` decides the outcome; `muteEndTimestamp` is
            // the expiry (seconds) while "always" stores -1. Unmute also keeps
            // a stale expiry row, so `muted` leads and "always" maps to a
            // sentinel far in the future.
            let action = &update.action;
            let muted = action.muted.unwrap_or(false);
            let until = if !muted {
                0
            } else if action.mute_end_timestamp.unwrap_or(0) < 0 {
                i64::MAX / 4
            } else {
                action.mute_end_timestamp.unwrap_or(0).saturating_mul(1000)
            };
            let _ = tx.send(UiEvent::ChatMute {
                chat: jid.to_string(),
                until,
                timestamp: update.timestamp.timestamp_millis(),
            });
        }
        Event::MarkChatAsReadUpdate(update) => {
            let jid = canonical(&update.jid);
            chats
                .lock()
                .expect("chats lock")
                .insert(jid.to_string(), jid.clone());
            let _ = tx.send(UiEvent::ChatReadMark {
                chat: jid.to_string(),
                read: update.action.read.unwrap_or(true),
            });
        }
        // Starred messages sync through the `star` app-state collection; a full
        // sync replays every star, so the webview's list rebuilds on each start.
        Event::StarUpdate(update) => {
            let jid = canonical(&update.chat_jid);
            chats
                .lock()
                .expect("chats lock")
                .insert(jid.to_string(), jid.clone());
            let _ = tx.send(UiEvent::StarMessage {
                chat: jid.to_string(),
                id: update.message_id.clone(),
                starred: update.action.starred.unwrap_or(false),
                timestamp: update.timestamp.timestamp_millis(),
            });
        }
        Event::IncomingCall(call) => {
            if let CallAction::Offer {
                call_id, is_video, ..
            } = &call.action
            {
                incoming_calls
                    .lock()
                    .expect("call lock")
                    .insert(call_id.clone(), call.clone());
                let _ = tx.send(UiEvent::CallIncoming {
                    call_id: call_id.clone(),
                    peer: call.from.to_string(),
                    video: *is_video,
                });
            }
        }
        Event::MissedCall(call) => {
            incoming_calls
                .lock()
                .expect("call lock")
                .remove(&call.call_id);
            let _ = tx.send(UiEvent::CallEnded {
                call_id: call.call_id.clone(),
                reason: "missed".to_string(),
            });
        }
        Event::CallEndedElsewhere(call) => {
            incoming_calls
                .lock()
                .expect("call lock")
                .remove(&call.call_id);
            let _ = tx.send(UiEvent::CallEnded {
                call_id: call.call_id.clone(),
                reason: "ended elsewhere".to_string(),
            });
        }
        _ => {}
    }
}
