//! history operations for `Session`.

use super::*;

impl Session {
    /// Ask the primary phone for this chat's history. An empty `oldest_id`
    /// means the chat has no local messages yet, so the phone is asked for the
    /// latest window instead of paging behind an anchor. The phone answers
    /// asynchronously with a history-sync chunk, which the normal
    /// `history_message` event path feeds back into the UI.
    pub async fn fetch_chat_history(
        &self,
        chat: &str,
        oldest_id: &str,
        oldest_from_me: bool,
        oldest_timestamp_ms: i64,
        count: i32,
    ) -> Result<(), String> {
        let mut jid = self.resolve_chat(chat)?;
        // Chat JIDs are not device-addressed. The self chat is stored with our
        // companion suffix (:53), which the phone cannot match in a history
        // request.
        jid.device = 0;
        jid.agent = 0;
        // The response identifies the conversation by its canonical id, so
        // remember which row the user actually opened.
        self.remember_history_target(chat);
        self.client
            .fetch_message_history(
                &jid,
                oldest_id,
                oldest_from_me,
                oldest_timestamp_ms,
                count.clamp(1, 100),
            )
            .await
            .map(|_| ())
            .map_err(|err| format!("history request failed: {err}"))
    }

    /// Ask one named linked device for this chat's history.
    ///
    /// The primary phone only stores what it received; messages exchanged while
    /// it was offline live on other linked devices. Non-SMB accounts cannot use
    /// `FULL_HISTORY_SYNC_ON_DEMAND`, so the client addresses the linked device
    /// that still has the window (usually WhatsApp Web) directly.
    pub async fn fetch_chat_history_to(
        &self,
        chat: &str,
        target: &str,
        oldest_id: &str,
        oldest_from_me: bool,
        oldest_timestamp_ms: i64,
        count: i32,
    ) -> Result<(), String> {
        let mut jid = self.resolve_chat(chat)?;
        jid.device = 0;
        jid.agent = 0;
        let target: Jid = target
            .parse()
            .map_err(|err| format!("invalid target device: {err}"))?;
        self.remember_history_target(chat);
        self.client
            .fetch_message_history_to(
                &jid,
                target,
                oldest_id,
                oldest_from_me,
                oldest_timestamp_ms,
                count.clamp(1, 100),
            )
            .await
            .map(|_| ())
            .map_err(|err| format!("history request failed: {err}"))
    }

    /// Re-request history for a batch of chats, one request at a time.
    ///
    /// The phone only answers on-demand history, so a fresh window (new
    /// install, cleared cache, or a chat whose messages were never fetched)
    /// rebuilds itself from the chats the UI already knows. Requests are
    /// spaced out and capped so a large list cannot turn into a burst that
    /// looks like abuse to WhatsApp.
    pub async fn sync_chat_history(&self, requests: &[HistoryRequest]) -> Result<usize, String> {
        if !self.client.is_connected() {
            return Err("not connected".to_string());
        }

        let mut sent = 0usize;
        let total = requests.len().min(8) as u32;
        let _ = self
            .tx
            .send(UiEvent::HistorySyncProgress { done: 0, total });
        // Eight chats with three quiet seconds between them. Anything larger
        // starts to look like a flood to the primary phone.
        for (index, request) in requests.iter().take(8).enumerate() {
            let Ok(mut jid) = self
                .resolve_chat(&request.chat)
                .or_else(|_| request.chat.parse::<Jid>().map_err(|err| err.to_string()))
            else {
                continue;
            };
            if jid.server != Server::Pn && jid.server != Server::Lid && jid.server != Server::Group
            {
                continue;
            }
            jid.device = 0;
            jid.agent = 0;
            self.remember_history_target(&request.chat);

            match self
                .client
                .fetch_message_history(
                    &jid,
                    &request.oldest_id,
                    request.oldest_from_me,
                    request.oldest_timestamp,
                    request.count.clamp(1, 50),
                )
                .await
            {
                Ok(_) => sent += 1,
                Err(err) => {
                    tracing::warn!(chat = %request.chat, %err, "history sync request failed")
                }
            }

            let _ = self.tx.send(UiEvent::HistorySyncProgress {
                done: (index + 1) as u32,
                total,
            });
            tokio::time::sleep(std::time::Duration::from_millis(3000)).await;
        }
        Ok(sent)
    }

    /// Persist the chat list (metadata only) next to the contact store, so a
    /// restart still has a sidebar even if the webview storage was cleared.
    pub fn save_chat_cache(&self, json: &str) -> Result<(), String> {
        let path = self.chat_cache_path();
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, json).map_err(|err| format!("cannot write chat cache: {err}"))?;
        std::fs::rename(&tmp, &path).map_err(|err| format!("cannot replace chat cache: {err}"))
    }

    /// Read the disk chat cache written by [`save_chat_cache`](Self::save_chat_cache).
    pub fn load_chat_cache(&self) -> Option<String> {
        std::fs::read_to_string(self.chat_cache_path()).ok()
    }

    fn chat_cache_path(&self) -> PathBuf {
        self.contacts_path.with_file_name("chats.json")
    }

    /// Remember which UI chat a history request belongs to, so the reply can be
    /// folded back when the phone answers under a LID/PN alias.
    fn remember_history_target(&self, chat: &str) {
        let now = now_ms();
        let mut targets = self.history_targets.lock().expect("history targets lock");
        targets.retain(|(_, at)| now.saturating_sub(*at) < 120_000);
        targets.retain(|(key, _)| key != chat);
        targets.push((chat.to_owned(), now));
    }
}
