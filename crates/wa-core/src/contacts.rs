//! contacts operations for `Session`.

use super::*;

impl Session {
    /// Saved contact names synced from the phone's address book.
    pub fn contacts(&self) -> HashMap<String, String> {
        self.contacts.lock().expect("contacts lock").clone()
    }

    /// Register the status ids the UI knows belong to this account. Incoming
    /// "view" receipts name one of these ids; without them a view cannot be told
    /// apart from a chat read receipt, which is why the count stayed at 0 for
    /// stories posted before the app learned their ids.
    pub fn remember_own_statuses(&self, ids: Vec<String>) {
        let mut set = self.own_statuses.lock().expect("statuses lock");
        for id in ids {
            if !id.is_empty() {
                set.insert(id);
            }
        }
        // Statuses live 24h; a few hundred ids is plenty of history.
        if set.len() > 500 {
            let excess = set.len() - 500;
            let stale: Vec<String> = set.iter().take(excess).cloned().collect();
            for id in stale {
                set.remove(&id);
            }
        }
    }

    /// Whether a JID belongs to a contact saved in the phone's address book.
    pub async fn is_saved_contact(&self, jid: &str) -> bool {
        let Some(phone) = self.resolve_phone(jid).await else {
            return false;
        };
        self.contacts
            .lock()
            .expect("contacts lock")
            .contains_key(&phone)
    }

    /// Whether a chat is the account's own "Message yourself" conversation.
    pub fn is_self_chat(&self, jid: &str) -> bool {
        let user = user_part(jid);
        self.client.pn().is_some_and(|pn| pn.user.as_str() == user)
            || self
                .client
                .lid()
                .is_some_and(|lid| lid.user.as_str() == user)
    }

    /// Pull the whole `contact` app-state collection (the phone's address
    /// book). Linked devices normally only receive deltas after pairing, so a
    /// one-off full sync is how WhatsApp Web fills its contact list.
    pub async fn sync_contacts(&self) -> Result<(), String> {
        use whatsapp_rust::wacore::appstate::hash::HashState;
        use whatsapp_rust::wacore::appstate::patch_decode::WAPatchName;

        // The first sync on a linked device asks for the full address-book
        // snapshot, and rewinds the local version: if this device already
        // advanced past the server's snapshot the sync would fail with a
        // version mismatch and never deliver the contacts. Later calls ask for
        // incremental deltas only.
        let marker = self.contacts_path.with_extension("synced");
        let first_run = !marker.exists();
        if first_run {
            let backend = self.client.persistence_manager().backend();
            backend
                .set_version(
                    WAPatchName::CriticalUnblockLow.as_str(),
                    HashState::default(),
                )
                .await
                .map_err(|e| format!("cannot reset app-state version: {e}"))?;
        }

        self.run_contact_sync(first_run).await;
        if self.contacts_ready() {
            let _ = std::fs::write(&marker, b"1");
            return Ok(());
        }
        // The address book did not arrive. The usual cause is the phone never
        // serving a snapshot (nothing to sync, or the key share still in
        // flight); a snapshot whose aggregate MAC cannot be reproduced is no
        // longer one of them — `wacore-appstate` now applies such a snapshot's
        // individually-authenticated records instead of dropping the whole
        // collection (see the change log).
        tracing::warn!("the phone's address book did not arrive yet");
        Err("the phone's address book did not arrive yet".into())
    }

    fn contacts_ready(&self) -> bool {
        !self.contacts.lock().expect("contacts lock").is_empty()
    }

    async fn run_contact_sync(&self, full_sync: bool) {
        use whatsapp_rust::sync_task::MajorSyncTask;
        use whatsapp_rust::wacore::appstate::patch_decode::WAPatchName;

        self.client
            .process_sync_task(MajorSyncTask::AppStateSync {
                name: WAPatchName::CriticalUnblockLow,
                full_sync,
            })
            .await;
    }

    /// Resolve a LID sender to its phone-number JID (digits only).
    pub async fn resolve_phone(&self, jid: &str) -> Option<String> {
        let jid = jid.parse::<Jid>().ok()?;
        if jid.server == Server::Pn {
            return Some(jid.user.to_string());
        }
        let entry = self.client.get_lid_pn_entry(&jid).await.ok().flatten()?;
        let phone = user_part(&entry.phone_number).to_owned();
        (!phone.is_empty()).then_some(phone)
    }

    /// Every chat JID the session has learned (messages, app-state, receipts).
    pub fn known_chats(&self) -> Vec<String> {
        self.chats
            .lock()
            .expect("chats lock")
            .keys()
            .cloned()
            .collect()
    }

    /// Apply the chat app-state collection (pins, archives, mutes) so those
    /// chats - typically groups, which never appear in the address book - are
    /// announced to the UI.
    pub async fn sync_chat_state(&self) -> Result<(), String> {
        use whatsapp_rust::sync_task::MajorSyncTask;
        use whatsapp_rust::wacore::appstate::patch_decode::WAPatchName;

        self.client
            .process_sync_task(MajorSyncTask::AppStateSync {
                name: WAPatchName::Regular,
                full_sync: false,
            })
            .await;
        Ok(())
    }

    /// The account's push name as stored in the device snapshot.
    pub fn push_name(&self) -> String {
        self.client.push_name()
    }

    /// Resolve peers to their phone/LID identity and verified business name.
    ///
    /// This also makes the protocol layer map the two ids for us: the returned
    /// LID/PN pair is persisted by `is_on_whatsapp`, so later traffic is
    /// canonicalised to the phone-number chat instead of creating a second row.
    pub async fn resolve_peers(&self, chats: &[String]) -> Result<Vec<PeerLookup>, String> {
        let mut requested: Vec<(String, Jid)> = Vec::new();
        for chat in chats {
            let Ok(jid) = self
                .resolve_chat(chat)
                .or_else(|_| chat.parse::<Jid>().map_err(|err| err.to_string()))
            else {
                continue;
            };
            if jid.server != Server::Pn && jid.server != Server::Lid {
                continue;
            }
            requested.push((chat.clone(), jid));
        }

        let mut lookups = Vec::new();
        for chunk in requested.chunks(50) {
            let jids: Vec<Jid> = chunk.iter().map(|(_, jid)| jid.clone()).collect();
            let results = self
                .client
                .contacts()
                .is_on_whatsapp(&jids)
                .await
                .map_err(|err| format!("peer lookup failed: {err}"))?;

            for result in results {
                if !result.is_registered {
                    continue;
                }
                let mut users = vec![result.jid.user.to_string()];
                if let Some(lid) = result.lid.as_ref() {
                    users.push(lid.user.to_string());
                }
                if let Some(pn) = result.pn_jid.as_ref() {
                    users.push(pn.user.to_string());
                }
                let Some((chat, _)) = chunk
                    .iter()
                    .find(|(_, jid)| users.contains(&jid.user.to_string()))
                else {
                    continue;
                };

                let phone = result
                    .pn_jid
                    .as_ref()
                    .filter(|jid| jid.server == Server::Pn)
                    .or_else(|| (result.jid.server == Server::Pn).then_some(&result.jid))
                    .map(|jid| jid.user.to_string());
                let lid = result
                    .lid
                    .as_ref()
                    .or_else(|| (result.jid.server == Server::Lid).then_some(&result.jid))
                    .map(|jid| jid.user.to_string());

                lookups.push(PeerLookup {
                    chat: chat.clone(),
                    phone,
                    lid,
                    name: result
                        .verified_name
                        .as_ref()
                        .and_then(|verified| verified.name.clone())
                        .filter(|name| !name.trim().is_empty()),
                    is_business: result.is_business,
                });
            }
        }
        Ok(lookups)
    }

    /// Save a contact name in the phone's address book (app-state `contact`
    /// mutation) and locally, then return the chat JID for it.
    pub async fn save_contact(&self, phone: &str, name: &str) -> Result<String, String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("the contact needs a name".into());
        }
        let digits = normalize_phone(phone)?;
        let jid = Jid::new(digits.clone(), Server::Pn);

        self.client
            .chat_actions()
            .save_contact(&jid, Some(name.to_owned()), None, false)
            .await
            .map_err(|e| format!("save contact failed: {e}"))?;

        {
            let mut contacts = self.contacts.lock().expect("contacts lock");
            contacts.insert(digits.clone(), name.to_owned());
            save_contacts(&self.contacts_path, &contacts);
        }
        let _ = self.tx.send(UiEvent::ContactName {
            jid: digits.clone(),
            name: name.to_owned(),
        });

        Ok(format!("{digits}@s.whatsapp.net"))
    }

    /// URL of a chat's profile picture, if the contact has one we may see.
    pub async fn contact_photo(&self, chat: &str) -> Result<Option<String>, String> {
        let mut jid = self.resolve_chat(chat)?;
        // Picture IQs are addressed to a user, not a device; a device suffix
        // (`:53`) is rejected with bad-request.
        jid.device = 0;
        jid.agent = 0;
        // The server ignores a PN lookup for our own account; its LID works.
        if self
            .client
            .pn()
            .is_some_and(|own| own.user == jid.user && own.server == jid.server)
            && let Some(mut lid) = self.client.lid()
        {
            lid.device = 0;
            lid.agent = 0;
            jid = lid;
        }

        match self
            .client
            .contacts()
            .get_profile_picture_with_timeout(&jid, true, Some(Duration::from_secs(8)))
            .await
        {
            Ok(Some(picture)) => {
                tracing::debug!(%jid, url = %picture.url, "profile picture");
                Ok(Some(picture.url))
            }
            Ok(None) => {
                tracing::debug!(%jid, "no profile picture");
                Ok(None)
            }
            Err(err) => {
                tracing::warn!(%jid, %err, "profile picture failed");
                Err(format!("profile picture failed: {err}"))
            }
        }
    }

    /// Update the account's display name.
    pub async fn set_push_name(&self, name: &str) -> Result<(), String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("name cannot be empty".into());
        }
        self.client
            .profile()
            .set_push_name(name)
            .await
            .map_err(|e| format!("could not update name: {e}"))
    }

    /// Update the account's "About" text.
    pub async fn set_status_text(&self, text: &str) -> Result<(), String> {
        self.client
            .profile()
            .set_status_text(text.trim())
            .await
            .map_err(|e| format!("could not update about: {e}"))
    }
}
