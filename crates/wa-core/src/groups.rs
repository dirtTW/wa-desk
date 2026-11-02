//! groups operations for `Session`.

use super::*;

impl Session {
    /// Create a group with the given PN participants and return its JID.
    pub async fn create_group(
        &self,
        subject: &str,
        participants: &[String],
    ) -> Result<String, String> {
        let subject = subject.trim();
        if subject.is_empty() {
            return Err("the group needs a name".into());
        }
        let members: Vec<whatsapp_rust::GroupParticipantOptions> = participants
            .iter()
            .filter_map(|jid| jid.parse::<Jid>().ok())
            .map(whatsapp_rust::GroupParticipantOptions::new)
            .collect();
        if members.is_empty() {
            return Err("pick at least one participant".into());
        }

        let options = whatsapp_rust::GroupCreateOptions::builder()
            .subject(subject.to_owned())
            .participants(members)
            .build();
        let result = self
            .client
            .groups()
            .create_group(options)
            .await
            .map_err(|e| format!("create group failed: {e}"))?;
        Ok(result.metadata.id.to_string())
    }

    /// Group metadata for the info screen.
    pub async fn group_info(&self, chat: &str) -> Result<GroupInfoView, String> {
        let jid = self.resolve_chat(chat)?;
        let metadata = match self.client.groups().get_metadata(&jid).await {
            Ok(metadata) => metadata,
            Err(err) if is_sender_poison(&err) => {
                self.reconnect_safely().await?;
                self.client
                    .groups()
                    .get_metadata(&jid)
                    .await
                    .map_err(|err| format!("group info failed after reconnect: {err}"))?
            }
            Err(err) => return Err(format!("group info failed: {err}")),
        };

        let members = metadata
            .participants
            .iter()
            .map(|participant| {
                let phone = participant
                    .phone_number
                    .as_ref()
                    .filter(|jid| jid.server == Server::Pn)
                    .or_else(|| (participant.jid.server == Server::Pn).then_some(&participant.jid))
                    .map(|jid| jid.user.to_string());
                GroupMemberView {
                    jid: participant.jid.to_string(),
                    phone,
                    admin: participant.is_admin(),
                }
            })
            .collect();

        // The freshly fetched subject beats any stale or polluted display
        // name (a member's push name once renamed whole groups through the
        // names map): re-emit it so the UI chat entry heals on every info
        // fetch.
        self.tx
            .send(UiEvent::ChatName {
                jid: metadata.id.to_string(),
                name: metadata.subject.clone(),
            })
            .ok();

        Ok(GroupInfoView {
            jid: metadata.id.to_string(),
            subject: metadata.subject,
            description: metadata.description,
            members,
            is_community: metadata.is_parent_group,
            parent_jid: metadata.parent_group_jid.map(|parent| parent.to_string()),
            is_announce: metadata.is_default_sub_group,
        })
    }

    /// Leave a group and stop receiving its updates.
    pub async fn leave_group(&self, chat: &str) -> Result<(), String> {
        let jid = self.resolve_chat(chat)?;
        self.client
            .groups()
            .leave(jid)
            .await
            .map_err(|e| format!("leave group failed: {e}"))
    }

    /// Add participants (PN JIDs) to a group we administer.
    pub async fn add_group_members(
        &self,
        chat: &str,
        participants: &[String],
    ) -> Result<(), String> {
        let jid = self.resolve_chat(chat)?;
        let members = parse_jids(participants)?;
        let results = self
            .client
            .groups()
            .add_participants(jid, &members)
            .await
            .map_err(|e| format!("add members failed: {e}"))?;
        group_change_result(results, "add members")
    }

    /// Remove participants from a group we administer.
    pub async fn remove_group_members(
        &self,
        chat: &str,
        participants: &[String],
    ) -> Result<(), String> {
        let jid = self.resolve_chat(chat)?;
        let members = parse_jids(participants)?;
        let results = self
            .client
            .groups()
            .remove_participants(jid, &members)
            .await
            .map_err(|e| format!("remove members failed: {e}"))?;
        group_change_result(results, "remove members")
    }

    /// Promote participants to group admins.
    pub async fn promote_group_members(
        &self,
        chat: &str,
        participants: &[String],
    ) -> Result<(), String> {
        let jid = self.resolve_chat(chat)?;
        let members = parse_jids(participants)?;
        let results = self
            .client
            .groups()
            .promote_participants(jid, &members)
            .await
            .map_err(|e| format!("promote members failed: {e}"))?;
        group_change_result(results, "promote members")
    }

    /// Dismiss participants as group admins.
    pub async fn demote_group_members(
        &self,
        chat: &str,
        participants: &[String],
    ) -> Result<(), String> {
        let jid = self.resolve_chat(chat)?;
        let members = parse_jids(participants)?;
        let results = self
            .client
            .groups()
            .demote_participants(jid, &members)
            .await
            .map_err(|e| format!("demote members failed: {e}"))?;
        group_change_result(results, "demote members")
    }
}
