//! messaging operations for `Session`.

use super::*;

impl Session {
    /// Forward one cached message to several chats.
    ///
    /// The library builds the forward copy (`is_forwarded`, bumped forwarding
    /// score, quoted chain stripped, `message_secret` dropped) and relays any
    /// media from its original CDN blob instead of re-uploading it.
    pub async fn forward_message(
        &self,
        chats: &[String],
        message_id: &str,
    ) -> Result<usize, String> {
        let cached = {
            let mut cache = self.messages.lock().expect("media lock");
            match cache.get(message_id).cloned() {
                Some(entry) => Some(entry),
                None => load_media(&self.msg_dir, message_id).inspect(|entry| {
                    cache.insert(message_id.to_owned(), entry.clone());
                }),
            }
        }
        .ok_or_else(|| "that message is no longer cached".to_string())?;

        self.forward_bodies(chats, &cached.message).await
    }

    /// Forward plain text when the original protobuf is not cached (chats that
    /// were stored before every message was cached). The copy still carries the
    /// forwarded marker the recipients see.
    pub async fn forward_text(&self, chats: &[String], text: &str) -> Result<usize, String> {
        let text = text.trim();
        if text.is_empty() {
            return Err("that message is no longer cached".to_string());
        }
        let body = wa::Message {
            conversation: Some(text.to_owned()),
            ..Default::default()
        };
        self.forward_bodies(chats, &body).await
    }

    async fn forward_bodies(&self, chats: &[String], body: &wa::Message) -> Result<usize, String> {
        let mut count = 0usize;
        let mut last_error = None;
        for chat in chats {
            let jid = match self.resolve_chat(chat) {
                Ok(jid) => jid,
                Err(err) => {
                    last_error = Some(err);
                    continue;
                }
            };
            match self.client.forward_message(jid, body).await {
                Ok(result) => {
                    count += 1;
                    let _ = self.tx.send(UiEvent::Sent {
                        chat: chat.clone(),
                        id: result.message_id,
                        timestamp: now_ms(),
                    });
                }
                Err(err) => last_error = Some(format!("send failed: {err}")),
            }
        }
        if count == 0 {
            return Err(last_error.unwrap_or_else(|| "forward failed".to_string()));
        }
        Ok(count)
    }

    /// Send text that quotes another message (`contextInfo.quotedMessage`), which
    /// is how WhatsApp renders a reply.
    pub async fn send_text_reply(
        &self,
        chat: &str,
        text: &str,
        reply_to: &str,
        quoted_text: Option<&str>,
    ) -> Result<(), String> {
        let text = text.trim();
        if text.is_empty() {
            return Err("cannot send an empty message".to_string());
        }
        let cached = {
            let mut cache = self.messages.lock().expect("media lock");
            match cache.get(reply_to).cloned() {
                Some(entry) => Some(entry),
                None => load_media(&self.msg_dir, reply_to).inspect(|entry| {
                    cache.insert(reply_to.to_owned(), entry.clone());
                }),
            }
        };
        // A message stored before bodies were cached still quotes its text so the
        // reply renders with a preview instead of failing.
        let (quoted, participant, remote_jid) = match cached {
            Some(entry) => (
                (*entry.message).clone(),
                entry
                    .participant
                    .clone()
                    .or_else(|| (!entry.from_me).then(|| entry.chat.clone())),
                Some(entry.chat.clone()),
            ),
            None => {
                let fallback = quoted_text.unwrap_or_default().trim().to_owned();
                if fallback.is_empty() {
                    return Err("that message is no longer cached".to_string());
                }
                (
                    wa::Message {
                        conversation: Some(fallback),
                        ..Default::default()
                    },
                    None,
                    Some(chat.to_owned()),
                )
            }
        };
        let context = wa::ContextInfo {
            stanza_id: Some(reply_to.to_owned()),
            participant,
            remote_jid,
            quoted_message: MessageField::some(quoted),
            ..Default::default()
        };
        let body = wa::Message {
            extended_text_message: MessageField::some(wa::message::ExtendedTextMessage {
                text: Some(text.to_owned()),
                context_info: MessageField::some(context),
                ..Default::default()
            }),
            ..Default::default()
        };

        let jid = self.resolve_chat(chat)?;
        let sent = self
            .client
            .send_message_with_options(jid, body, Default::default())
            .await
            .map_err(|err| format!("send failed: {err}"))?;
        let _ = self.tx.send(UiEvent::Sent {
            chat: chat.to_owned(),
            id: sent.message_id,
            timestamp: now_ms(),
        });
        Ok(())
    }

    pub async fn send_text(&self, chat: &str, text: &str) -> Result<String, String> {
        let text = text.trim();
        if text.is_empty() {
            return Err("cannot send an empty message".into());
        }
        let jid = self.resolve_chat(chat)?;
        let sent = match self.client.send_text(jid.clone(), text).await {
            Ok(sent) => sent,
            Err(err) if is_sender_poison(&err) => {
                // A poisoned Noise sender means this frame never left the
                // socket. One reconnect + one retry is safe; never loop.
                self.reconnect_safely().await?;
                self.client
                    .send_text(jid, text)
                    .await
                    .map_err(|err| format!("send failed after reconnect: {err}"))?
            }
            Err(err) => return Err(format!("send failed: {err}")),
        };

        let _ = self.tx.send(UiEvent::Sent {
            chat: chat.to_owned(),
            id: sent.message_id.clone(),
            timestamp: now_ms(),
        });
        // The caller (an AI reply, for example) binds its optimistic bubble to
        // this server id so later REVOKEs can find it.
        Ok(sent.message_id)
    }

    /// Mark a chat as read on every linked device.
    pub async fn mark_read(&self, chat: &str) -> Result<(), String> {
        let jid = self.resolve_chat(chat)?;
        self.client
            .chat_actions()
            .mark_chat_as_read(&jid, true, None)
            .await
            .map_err(|e| format!("mark read failed: {e}"))
    }

    /// Tell the other side that we are typing (best effort).
    pub async fn set_typing(&self, chat: &str, typing: bool) -> Result<(), String> {
        let jid = self.resolve_chat(chat)?;
        let result = if typing {
            self.client.chatstate().send_composing(&jid).await
        } else {
            self.client.chatstate().send_paused(&jid).await
        };
        result.map_err(|e| format!("typing indicator failed: {e}"))
    }

    /// Ask WhatsApp to mint a phone-number pairing code.
    pub async fn request_pair_code(&self, phone_number: &str) -> Result<String, String> {
        let options = PairCodeOptions {
            phone_number: phone_number.trim().to_owned(),
            ..Default::default()
        };
        self.client
            .pair_with_code(options)
            .await
            .map_err(|e| format!("pair code failed: {e}"))
    }

    /// Delete a message only on this device (kept in the local archive).
    ///
    /// Returns as soon as the work is queued: the library's app-state mutation
    /// waits for a server ack that can take a long time, and WhatsApp Web hides
    /// the bubble immediately anyway.
    pub async fn delete_for_me(
        &self,
        chat: &str,
        id: &str,
        from_me: bool,
        participant: Option<&str>,
    ) -> Result<(), String> {
        let jid = self.resolve_chat(chat)?;
        let participant_jid = participant.and_then(|raw| raw.parse::<Jid>().ok());
        let result = self
            .client
            .chat_actions()
            .delete_message_for_me(&jid, participant_jid.as_ref(), id, from_me, true, None)
            .await;

        let err = match result {
            Ok(()) => return Ok(()),
            Err(err) => err.to_string(),
        };
        // Right after startup the local app-state base can lag behind the
        // server's while the initial sync retries ("app-state patch for
        // regular_high still conflicting after 5 attempts"). The conflict
        // resolves once that sync settles, and the bubble is already hidden,
        // so retry the mutation in the background for a couple of cycles
        // instead of leaving the user with an error toast.
        if err.contains("still conflicting") {
            let client = Arc::clone(&self.client);
            let stopper = self.stopper.clone();
            let chat = jid.clone();
            let id = id.to_owned();
            tracing::warn!(chat = %chat, id = %id, "delete-for-me patch conflicted; retrying after the app-state sync settles");
            tokio::spawn(async move {
                for delay in [15u64, 60, 120] {
                    if !stopper.pause(std::time::Duration::from_secs(delay)).await {
                        return;
                    }
                    match client
                        .chat_actions()
                        .delete_message_for_me(
                            &chat,
                            participant_jid.as_ref(),
                            &id,
                            from_me,
                            true,
                            None,
                        )
                        .await
                    {
                        Ok(()) => {
                            tracing::info!(%chat, %id, "deferred delete-for-me patch accepted");
                            return;
                        }
                        Err(e) if e.to_string().contains("still conflicting") => {
                            tracing::warn!(%chat, %id, "deferred delete-for-me patch still conflicting");
                        }
                        Err(e) => {
                            tracing::warn!(%e, %chat, %id, "deferred delete-for-me patch failed");
                            return;
                        }
                    }
                }
            });
            return Ok(());
        }
        Err(err)
    }

    /// Revoke a message for everyone.
    ///
    /// Use the library's revoke path so the message is tagged as a sender revoke
    /// in the send pipeline (a raw ProtocolMessage send is not sufficient).
    pub async fn delete_for_everyone(&self, chat: &str, id: &str) -> Result<(), String> {
        let jid = self.resolve_chat(chat)?;
        self.client
            .revoke_message(jid, id.to_owned(), whatsapp_rust::RevokeType::Sender)
            .await
            .map_err(|err| format!("delete for everyone failed: {err}"))?;
        Ok(())
    }

    /// Pin or unpin a chat instantly: the `PinAction` app-state mutation
    /// reaches every linked client (phone + other desktops), but its send
    /// waits on the app-state send lock and up to five conflict rounds, so it
    /// runs in the background and the caller returns at once — the UI has
    /// already pinned optimistically, exactly like WhatsApp Web.
    pub async fn pin_chat(&self, chat: &str, pinned: bool) -> Result<(), String> {
        let jid = self.resolve_chat(chat)?;
        // Fire the patch + retries in the background; report the local state
        // to connected UIs immediately.
        let _ = self.tx.send(UiEvent::ChatPin {
            chat: jid.to_string(),
            pinned,
            timestamp: now_ms(),
        });
        let client = Arc::clone(&self.client);
        let stopper = self.stopper.clone();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let send = || async {
                if pinned {
                    client.chat_actions().pin_chat(&jid).await
                } else {
                    client.chat_actions().unpin_chat(&jid).await
                }
            };
            let mut outcome = send().await;
            // The same post-startup app-state conflict window that hit
            // delete-for-me ("still conflicting after N attempts") — retry
            // until the sync settles instead of silently dropping the pin.
            for delay in [10u64, 30, 90, 240] {
                if outcome.is_ok() {
                    break;
                }
                if !outcome
                    .as_ref()
                    .err()
                    .is_some_and(|e| e.to_string().contains("still conflicting"))
                {
                    break;
                }
                tracing::warn!(chat = %jid, pinned, "chat pin patch conflicted; retrying to keep every client in sync");
                if !stopper.pause(std::time::Duration::from_secs(delay)).await {
                    return;
                }
                outcome = send().await;
            }
            match outcome {
                Ok(()) => {
                    tracing::info!(chat = %jid, pinned, "chat pin patch accepted");
                }
                Err(e) => {
                    tracing::warn!(%e, chat = %jid, pinned, "chat pin patch failed after retries");
                    // Revert the optimistic state so the UI does not promise a
                    // pin that never landed.
                    let _ = tx.send(UiEvent::ChatPin {
                        chat: jid.to_string(),
                        pinned: !pinned,
                        timestamp: now_ms(),
                    });
                }
            }
        });
        Ok(())
    }

    /// Edit one of our own text messages.
    pub async fn edit_message(&self, chat: &str, id: &str, text: &str) -> Result<(), String> {
        let jid = self.resolve_chat(chat)?;
        let text = text.trim();
        if text.is_empty() {
            return Err("the edited message is empty".into());
        }
        self.client
            .edit_message(jid, id.to_owned(), wa::Message::text(text))
            .await
            .map(|_| ())
            .map_err(|e| format!("edit failed: {e}"))
    }

    /// Archive or unarchive a chat (every linked client follows the mutation).
    pub async fn archive_chat(&self, chat: &str, archived: bool) -> Result<(), String> {
        let jid = self.resolve_chat(chat)?;
        if archived {
            self.client.chat_actions().archive_chat(&jid, None).await
        } else {
            self.client.chat_actions().unarchive_chat(&jid, None).await
        }
        .map_err(|e| format!("archive failed: {e}"))?;
        let _ = self.tx.send(UiEvent::ChatArchive {
            chat: jid.to_string(),
            archived,
        });
        Ok(())
    }

    /// Mute or unmute a chat. `until_ms` is a Unix-milliseconds expiry; `-1`
    /// means "Always" and `0` unmutes.
    pub async fn mute_chat(&self, chat: &str, until_ms: i64) -> Result<(), String> {
        let jid = self.resolve_chat(chat)?;
        let now = now_ms();
        if until_ms == 0 {
            self.client
                .chat_actions()
                .unmute_chat(&jid)
                .await
                .map_err(|e| format!("unmute failed: {e}"))?;
        } else if until_ms < 0 {
            // "Always": a sentinel far in the future.
            self.client
                .chat_actions()
                .mute_chat(&jid)
                .await
                .map_err(|e| format!("mute failed: {e}"))?;
        } else if until_ms <= now {
            return Err("the mute duration is already over".into());
        } else {
            self.client
                .chat_actions()
                .mute_chat_until(&jid, until_ms)
                .await
                .map_err(|e| format!("mute failed: {e}"))?;
        }
        let until_sentinel = if until_ms < 0 { i64::MAX / 4 } else { until_ms };
        let _ = self.tx.send(UiEvent::ChatMute {
            chat: jid.to_string(),
            until: until_sentinel,
            timestamp: now,
        });
        Ok(())
    }

    /// Mark a chat read or unread (the 📱 app-state mutation every client syncs).
    pub async fn mark_chat_unread(&self, chat: &str, unread: bool) -> Result<(), String> {
        let jid = self.resolve_chat(chat)?;
        self.client
            .chat_actions()
            .mark_chat_as_read(&jid, !unread, None)
            .await
            .map_err(|e| format!("mark chat as unread failed: {e}"))?;
        let _ = self.tx.send(UiEvent::ChatReadMark {
            chat: jid.to_string(),
            read: !unread,
        });
        Ok(())
    }

    /// Star or unstar a message on every linked client (the `star` app-state
    /// collection). `participant` names the author of someone else's group
    /// message; the phone needs it to address the message key, and it is
    /// ignored for our own messages and 1:1 chats.
    pub async fn star_message(
        &self,
        chat: &str,
        id: &str,
        starred: bool,
        from_me: bool,
        participant: Option<&str>,
    ) -> Result<(), String> {
        let jid = self.resolve_chat(chat)?;
        let participant_jid = match participant {
            Some(value) if !from_me && !value.is_empty() => Some(
                value
                    .parse::<Jid>()
                    .map_err(|e| format!("unknown participant {value}: {e}"))?,
            ),
            _ => None,
        };
        let actions = self.client.chat_actions();
        let result = if starred {
            actions
                .star_message(&jid, participant_jid.as_ref(), id, from_me)
                .await
        } else {
            actions
                .unstar_message(&jid, participant_jid.as_ref(), id, from_me)
                .await
        };
        result.map_err(|e| format!("star failed: {e}"))?;
        let _ = self.tx.send(UiEvent::StarMessage {
            chat: jid.to_string(),
            id: id.to_string(),
            starred,
            timestamp: now_ms(),
        });
        Ok(())
    }

    /// Delete a whole chat on every device (the official desktop's action).
    pub async fn delete_chat(&self, chat: &str) -> Result<(), String> {
        let jid = self.resolve_chat(chat)?;
        self.client
            .chat_actions()
            .delete_chat(&jid, true, None)
            .await
            .map_err(|e| format!("delete chat failed: {e}"))?;
        let _ = self.tx.send(UiEvent::ChatDeleted {
            chat: jid.to_string(),
        });
        Ok(())
    }

    /// Clear a chat's history while keeping it (the official desktop's action).
    pub async fn clear_chat(&self, chat: &str) -> Result<(), String> {
        let jid = self.resolve_chat(chat)?;
        self.client
            .chat_actions()
            .clear_chat(&jid, false, false, None)
            .await
            .map_err(|e| format!("clear chat failed: {e}"))?;
        let _ = self.tx.send(UiEvent::ChatCleared {
            chat: jid.to_string(),
        });
        Ok(())
    }

    /// Block or unblock a contact.
    pub async fn block_contact(&self, chat: &str, blocked: bool) -> Result<(), String> {
        let jid = self.resolve_chat(chat)?;
        if blocked {
            self.client
                .blocking()
                .block(&jid)
                .await
                .map_err(|e| format!("block failed: {e}"))?;
            Ok(())
        } else {
            self.client
                .blocking()
                .unblock(&jid)
                .await
                .map_err(|e| format!("unblock failed: {e}"))?;
            Ok(())
        }
    }

    /// React to a message with an emoji. An empty emoji removes our reaction.
    pub async fn send_reaction(
        &self,
        chat: &str,
        id: &str,
        from_me: bool,
        participant: Option<&str>,
        emoji: &str,
    ) -> Result<(), String> {
        let mut jid = self.resolve_chat(chat)?;
        // The "Message yourself" chat is stored as our LID, but message keys
        // use the phone-number JID; reactions addressed to the LID are ignored
        // by WhatsApp Web.
        if self.client.lid().is_some_and(|lid| lid.user == jid.user)
            && let Some(pn) = self.client.pn()
        {
            jid = pn;
        }
        // Message keys never carry a device suffix (`:53`); a target with one
        // is silently ignored by the server.
        let jid = jid.to_non_ad();
        let key = wa::MessageKey {
            remote_jid: Some(jid.to_string()),
            from_me: Some(from_me),
            id: Some(id.to_owned()),
            participant: participant.map(str::to_owned),
        };
        self.client
            .send_reaction(jid, key, emoji)
            .await
            .map(|_| ())
            .map_err(|e| format!("reaction failed: {e}"))
    }

    /// Mark specific incoming messages as read (this is what turns the ticks
    /// blue for the sender).
    pub async fn mark_read_messages(&self, chat: &str, ids: Vec<String>) -> Result<(), String> {
        if ids.is_empty() {
            return Ok(());
        }
        let jid = self.resolve_chat(chat)?;
        let refs: Vec<&str> = ids.iter().map(String::as_str).collect();
        self.client
            .mark_as_read(&jid, None, &refs)
            .await
            .map_err(|e| format!("read receipt failed: {e}"))
    }
}
