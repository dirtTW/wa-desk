//! Session lifecycle, identity and chat resolution.

use super::*;

impl Session {
    /// Open the device store and start the connection supervisor.
    ///
    /// `db_path` is a plain filesystem path (an absolute path is recommended);
    /// `whatsapp-rust` stores the Signal session, pre-keys and app state there.
    pub async fn start(
        db_path: &str,
        media_dir: PathBuf,
        tx: mpsc::UnboundedSender<UiEvent>,
    ) -> anyhow::Result<Arc<Self>> {
        let msg_dir = media_dir.with_file_name("msgcache");
        prune_cache(&msg_dir, 3000);
        prune_cache(&media_dir, 4000);
        let store = SqliteStore::new(db_path)
            .await
            .map_err(|e| anyhow::anyhow!("cannot open device store at {db_path}: {e}"))?;

        let chats: Arc<Mutex<HashMap<String, Jid>>> = Arc::new(Mutex::new(HashMap::new()));
        let names: Arc<Mutex<HashMap<String, String>>> = Arc::new(Mutex::new(HashMap::new()));
        let aliases: Arc<Mutex<HashMap<String, String>>> = Arc::new(Mutex::new(HashMap::new()));
        let canon: Arc<Mutex<HashMap<String, String>>> = Arc::new(Mutex::new(HashMap::new()));
        let contacts_path = PathBuf::from(db_path).with_file_name("contacts.json");
        let loaded_contacts = load_contacts(&contacts_path);
        if !loaded_contacts.is_empty() {
            // Names are already on disk: no need to rewind and re-sync the
            // whole address book on this launch.
            let _ = std::fs::write(contacts_path.with_extension("synced"), b"1");
        }
        let contacts: Arc<Mutex<HashMap<String, String>>> = Arc::new(Mutex::new(loaded_contacts));
        let messages: Arc<Mutex<HashMap<String, CachedMedia>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let incoming_calls: Arc<Mutex<HashMap<String, IncomingCall>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let history_targets: Arc<Mutex<Vec<(String, i64)>>> = Arc::new(Mutex::new(Vec::new()));
        let requested_stubs: Arc<Mutex<std::collections::HashSet<String>>> =
            Arc::new(Mutex::new(std::collections::HashSet::new()));
        // Message ids of the stories this account posted. Status "view"
        // receipts name one of these ids, which is how a view is told apart
        // from a normal chat read receipt.
        let own_statuses: Arc<Mutex<std::collections::HashSet<String>>> =
            Arc::new(Mutex::new(std::collections::HashSet::new()));

        let event_tx = tx.clone();
        let event_chats = chats.clone();
        let event_names = names.clone();
        let event_aliases = aliases.clone();
        let event_canon = canon.clone();
        let event_contacts = contacts.clone();
        let event_contacts_path = contacts_path.clone();
        let event_messages = messages.clone();
        let event_msg_dir = msg_dir.clone();
        let event_incoming_calls = incoming_calls.clone();
        let event_history_targets = history_targets.clone();
        let event_requested_stubs = requested_stubs.clone();
        let event_own_statuses = own_statuses.clone();
        let bot = Bot::builder()
            .with_backend(store)
            // Do NOT set `with_push_name` here: on an already-registered device
            // it overwrites the account's push name in the local snapshot.
            .on_event(move |event, client| {
                let tx = event_tx.clone();
                let chats = event_chats.clone();
                let names = event_names.clone();
                let aliases = event_aliases.clone();
                let canon = event_canon.clone();
                let contacts = event_contacts.clone();
                let contacts_path = event_contacts_path.clone();
                let messages = event_messages.clone();
                let msg_dir = event_msg_dir.clone();
                let incoming_calls = event_incoming_calls.clone();
                let history_targets = event_history_targets.clone();
                let requested_stubs = event_requested_stubs.clone();
                let own_statuses = event_own_statuses.clone();
                async move {
                    // Keep the read loop responsive: this handler only builds
                    // small payloads and hands them to an unbounded channel.
                    seed_canon(&client, &canon, &aliases, &event).await;
                    let pn = client.pn();
                    let lid = client.lid();
                    dispatch(
                        Some(&client),
                        &event,
                        pn.as_ref(),
                        lid.as_ref(),
                        &chats,
                        &names,
                        &aliases,
                        &canon,
                        &contacts,
                        &contacts_path,
                        &messages,
                        &msg_dir,
                        &incoming_calls,
                        &history_targets,
                        &requested_stubs,
                        &own_statuses,
                        &tx,
                    );
                }
            })
            .build()
            .await
            .map_err(|e| anyhow::anyhow!("cannot start WhatsApp client: {e:?}"))?;

        let handle = bot.spawn();
        let client = handle.client();

        // Debug aid: log every notification kind, so a missing `mediaretry`
        // (media reupload) is distinguishable from a filter that never matched.
        if tracing::enabled!(tracing::Level::DEBUG) {
            let trace_client = client.clone();
            tokio::spawn(async move {
                loop {
                    let waiter =
                        trace_client.wait_for_node(whatsapp_rust::NodeFilter::tag("notification"));
                    let Ok(node) = waiter.await else { break };
                    let node = node.get();
                    let kind = node
                        .get_attr("type")
                        .map(|value| value.as_str().into_owned())
                        .unwrap_or_default();
                    let id = node
                        .get_attr("id")
                        .map(|value| value.as_str().into_owned())
                        .unwrap_or_default();
                    tracing::debug!(kind = %kind, id = %id, "notification received");
                }
            });
        }

        Ok(Arc::new(Self {
            client,
            chats,
            contacts,
            contacts_path,
            messages,
            media_dir,
            msg_dir,
            incoming_calls,
            history_targets,
            own_statuses,
            active_call: Arc::new(Mutex::new(None)),
            last_reconnect_ms: AtomicU64::new(0),
            handle: Mutex::new(Some(handle)),
            stopper: Stopper::default(),
            tx,
        }))
    }

    /// True once the Noise socket is up.
    pub fn is_connected(&self) -> bool {
        self.client.is_connected()
    }

    /// True once the account is authenticated (paired).
    pub fn is_logged_in(&self) -> bool {
        self.client.is_logged_in()
    }

    /// True when the device store already holds credentials for an account.
    ///
    /// Unlike [`is_logged_in`](Self::is_logged_in) this is known as soon as the
    /// store is open, before the first handshake, so the UI can skip the
    /// linking screen on a normal restart.
    pub fn is_paired(&self) -> bool {
        self.client.pn().is_some()
    }

    /// The account's own WhatsApp id, when paired.
    pub fn own_jid(&self) -> Option<String> {
        self.client.pn().map(|jid| jid.to_string())
    }

    /// The account's LID form, which is what profile-picture lookups accept for
    /// our own account (the server ignores a PN self-lookup).
    pub fn own_lid(&self) -> Option<String> {
        self.client.lid().map(|jid| jid.to_string())
    }

    /// Send a plain text message to an existing chat.
    /// Reconnect after a poisoned outbound Noise sender.
    ///
    /// The poison is per-connection: the write keystream cannot be reused after
    /// a transport failure, so only a fresh handshake is safe. Reconnects are
    /// rate-limited and wait for full readiness before the caller retries once;
    /// this avoids a reconnect/retry storm that could look abusive.
    pub async fn reconnect_safely(&self) -> Result<(), String> {
        let now = now_ms().max(0) as u64;
        let last = self.last_reconnect_ms.load(Ordering::Relaxed);
        if last != 0 && now.saturating_sub(last) < 8_000 {
            return Err("a reconnect was attempted moments ago; wait a few seconds".to_string());
        }
        self.last_reconnect_ms.store(now, Ordering::Relaxed);
        tracing::warn!("reconnecting after a poisoned outbound pipeline");
        self.client.reconnect_immediately().await;
        self.client
            .wait_for_connected(Duration::from_secs(20))
            .await
            .map_err(|err| format!("reconnect failed: {err}"))?;
        // Give the server-side routing a beat before the single retry.
        tokio::time::sleep(Duration::from_millis(500)).await;
        Ok(())
    }

    /// Unlink this device and stop the session.
    pub async fn logout(&self) {
        self.client.logout().await;
    }

    /// Flush pending state and wait for the run loop to stop.
    ///
    /// Call this on app exit: dropping the [`BotHandle`] aborts the bot and can
    /// lose the last few seconds of protocol state.
    pub async fn shutdown(&self) {
        // Wake background retries first: they hold `Arc<Client>`, and the client
        // owns the SQLite store, so a task still sleeping on a retry would keep
        // `whatsapp.db` locked and break the logout wipe.
        self.stopper.stop();
        let handle = self.handle.lock().expect("handle lock").take();
        if let Some(handle) = handle {
            handle.shutdown().await;
        }
    }

    pub(crate) fn resolve_chat(&self, chat: &str) -> Result<Jid, String> {
        if let Some(jid) = self.chats.lock().expect("chats lock").get(chat) {
            return Ok(jid.clone());
        }
        chat.parse::<Jid>()
            .map_err(|e| format!("unknown chat {chat}: {e}"))
    }
}
