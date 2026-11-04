//! channels operations for `Session`.

use super::*;

impl Session {
    /// Load the channels this account follows and publish their names/follower
    /// counts to the UI. Channel messages themselves arrive as normal events;
    /// this only fills in the metadata the sidebar and header need.
    pub async fn sync_channels(&self) -> Result<usize, String> {
        let newsletters = self
            .client
            .newsletter()
            .list_subscribed()
            .await
            .map_err(|err| format!("cannot list channels: {err:?}"))?;

        let mut count = 0usize;
        for mut newsletter in newsletters {
            // `list_subscribed` only carries names on some servers; fetch the
            // per-channel metadata for the avatar and follower count.
            if (newsletter.picture_url.is_none() && newsletter.preview_url.is_none())
                || newsletter.subscriber_count == 0
            {
                match self.client.newsletter().get_metadata(&newsletter.jid).await {
                    Ok(details) => {
                        if newsletter.name.trim().is_empty() {
                            newsletter.name = details.name;
                        }
                        if newsletter.subscriber_count == 0 {
                            newsletter.subscriber_count = details.subscriber_count;
                        }
                        if newsletter.picture_url.is_none() {
                            newsletter.picture_url = details.picture_url;
                        }
                        if newsletter.preview_url.is_none() {
                            newsletter.preview_url = details.preview_url;
                        }
                    }
                    Err(err) => {
                        tracing::debug!(%err, jid = %newsletter.jid, "channel metadata unavailable");
                    }
                }
            }
            let jid = newsletter.jid.to_string();
            if newsletter.name.trim().is_empty() {
                continue;
            }
            self.chats
                .lock()
                .expect("chats lock")
                .insert(jid.clone(), newsletter.jid.clone());
            let _ = self.tx.send(UiEvent::ChatName {
                jid: jid.clone(),
                name: newsletter.name.clone(),
            });
            let _ = self.tx.send(UiEvent::ChannelInfo {
                jid,
                name: newsletter.name,
                subscribers: newsletter.subscriber_count,
                // The server ships channel avatars as CDN paths (`/v/…`,
                // `/m1/v/…`) rather than absolute URLs; the webview needs the
                // host. `pps` serves the profile-photo paths.
                picture: absolute_cdn_url(newsletter.picture_url)
                    .or_else(|| absolute_cdn_url(newsletter.preview_url)),
            });
            count += 1;
        }
        Ok(count)
    }

    /// Load a page of a channel's posts through a direct session IQ.
    ///
    /// The library helper (`newsletter().get_messages`) has no timeout guard and
    /// waits ~75 s before failing, so this sends the same stanza on the session
    /// itself with a short deadline and decodes the reply here. Measured on this
    /// account: the server does not answer the newsletter message IQ for a
    /// linked device (`Timeout` after 6 s), so callers fall back to the export
    /// import path instead of retrying.
    pub async fn fetch_channel_history(
        &self,
        chat: &str,
        name: &str,
        count: u32,
        before: Option<u64>,
    ) -> Result<Option<u64>, String> {
        use std::time::Duration;
        use whatsapp_rust::NodeBuilder;

        let jid: Jid = chat
            .parse()
            .map_err(|err| format!("invalid channel id: {err}"))?;
        if !jid.to_string().ends_with("@newsletter") {
            return Err("not a channel".to_string());
        }
        // WhatsApp Web sends this as a *self* IQ: the stanza goes to the server
        // (`s.whatsapp.net`) and the channel is named inside `<messages>`
        // (`type="jid"`). Addressing the channel JID itself is silently ignored,
        // which is why the earlier attempts timed out.
        // WhatsApp Web caps a page at the `channels_max_messages_batch_pull`
        // AB prop (default 100) — see `WAWebNewsletterGatingUtils`,
        // `getMaxMsgCountFromServer()`, used by `WAWebNewsletterGetMessagesQueryJob`.
        let mut messages = NodeBuilder::new("messages")
            .attr("type", "jid")
            .attr("jid", jid.to_string())
            .attr("count", count.clamp(1, 100));
        if let Some(before_id) = before {
            messages = messages.attr("before", before_id);
        }
        let node = NodeBuilder::new("iq")
            .attr("type", "get")
            .attr("xmlns", "newsletter")
            .attr("to", "s.whatsapp.net")
            .children([messages.build()])
            .build();

        let response = self
            .client
            .send_iq_node(node, Some(Duration::from_secs(6)))
            .await
            .map_err(|err| format!("channel history unavailable: {err:?}"))?;
        let root = response.get();
        let Some(list) = root.get_optional_child("messages") else {
            return Err("channel history: no <messages> in the reply".to_string());
        };
        let page = decode_channel_page(list, &jid, name, self.contacts.as_ref());
        for (view, media, link_post) in page.posts {
            if let Some(body) = media.or(link_post) {
                remember_message(
                    &self.messages,
                    &self.msg_dir,
                    &view.id,
                    body,
                    &jid,
                    view.from_me,
                    None,
                );
            }
            let _ = self.tx.send(UiEvent::HistoryMessage(view));
        }
        tracing::debug!(chat = %jid, ?page.cursor, messages = page.messages, "channel history page");
        // WhatsApp Web treats a page shorter than the requested count as the end
        // of the channel's history (`end: messages.length < count`), so no
        // further `before` request is made from the UI.
        if page.messages < count.clamp(1, 100) as usize {
            Ok(None)
        } else {
            Ok(page.cursor)
        }
    }
}

/// Channel avatars arrive as CDN paths (`/v/t61â€¦`, `/m1/v/t24â€¦`); prefix the
/// profile-photo host so the webview can load them.
fn absolute_cdn_url(url: Option<String>) -> Option<String> {
    let url = url?.trim().to_owned();
    if url.is_empty() {
        return None;
    }
    if url.starts_with("http://") || url.starts_with("https://") {
        return Some(url);
    }
    if url.starts_with('/') {
        return Some(format!("https://pps.whatsapp.net{url}"));
    }
    Some(format!("https://pps.whatsapp.net/{url}"))
}

/// Payload of the first length-delimited field in `data` (wire type 2).
///
/// Newsletter history wraps the real `Message` in a fanout envelope (`<plaintext>`
/// holds one `bytes` field — 101 for media, 106 for text — whose content is the
/// message), so the decoder has to peel those layers before it can read a post.
fn first_length_delimited(data: &[u8]) -> Option<&[u8]> {
    fn varint(data: &[u8], index: &mut usize) -> Option<u64> {
        let mut value = 0u64;
        let mut shift = 0u32;
        while *index < data.len() && shift < 64 {
            let byte = data[*index];
            *index += 1;
            value |= u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return Some(value);
            }
            shift += 7;
        }
        None
    }
    let mut index = 0usize;
    while index < data.len() {
        let key = varint(data, &mut index)?;
        match key & 7 {
            0 => {
                varint(data, &mut index)?;
            }
            1 => index += 8,
            2 => {
                let length = varint(data, &mut index)? as usize;
                let end = index.checked_add(length)?;
                if end > data.len() {
                    return None;
                }
                return Some(&data[index..end]);
            }
            5 => index += 4,
            _ => return None,
        }
    }
    None
}

/// Decode a channel post body, peeling newsletter fanout envelopes until the
/// bytes yield a message with real content (text, caption or media).
pub(crate) fn decode_newsletter_body(bytes: &[u8]) -> Option<wa::Message> {
    use whatsapp_rust::waproto::codec::message_decode;

    let mut layer = bytes.to_vec();
    for _ in 0..4 {
        if let Ok(message) = message_decode(&layer) {
            let base = message.get_base_message();
            let has_text = message_text(base)
                .map(|text| !text.trim().is_empty())
                .unwrap_or(false);
            if has_text || media_kind(base).is_some() {
                return Some(message);
            }
        }
        layer = first_length_delimited(&layer)?.to_vec();
    }
    None
}

/// First descendant (depth-first) carrying `tag`, mirroring WhatsApp Web's
/// `flattenedChildWithTag` helper.
fn descendant<'a>(
    node: &'a whatsapp_rust::wacore_binary::NodeRef<'a>,
    tag: &str,
) -> Option<&'a whatsapp_rust::wacore_binary::NodeRef<'a>> {
    for child in node.children()? {
        if child.tag.as_ref() == tag {
            return Some(child);
        }
        if let Some(found) = descendant(child, tag) {
            return Some(found);
        }
    }
    None
}

/// Counters the server hangs off a channel post: `<views_count count>`,
/// `<forwards_count count>` and `<reactions><reaction code count/></reactions>`.
///
/// The desktop app renders these under the card (reaction pills + share count),
/// so the decoder keeps them on the post. WhatsApp nests them (`flattenedChildWithTag`
/// in `WASmaxInNewslettersNewsletter*CountMixin`), so the lookup walks descendants.
fn channel_stats(node: &whatsapp_rust::wacore_binary::NodeRef<'_>) -> Option<ChannelStats> {
    let count_of = |tag: &str| {
        descendant(node, tag)
            .and_then(|child| child.get_attr("count"))
            .and_then(|value| value.as_str().parse::<u64>().ok())
    };
    let mut stats = ChannelStats {
        views: count_of("views_count").or_else(|| count_of("views")),
        forwards: count_of("forwards_count"),
        reactions: Vec::new(),
        reaction_total: None,
    };
    if let Some(list) = descendant(node, "reactions")
        && let Some(children) = list.children()
    {
        for reaction in children.iter() {
            if reaction.tag.as_ref() != "reaction" {
                continue;
            }
            let (Some(code), Some(count)) = (
                reaction.get_attr("code"),
                reaction
                    .get_attr("count")
                    .and_then(|value| value.as_str().parse::<u32>().ok()),
            ) else {
                continue;
            };
            if code.as_str().is_empty() || count == 0 {
                continue;
            }
            stats.reactions.push(ReactionCount {
                code: code.as_str().into_owned(),
                count,
            });
        }
        stats
            .reactions
            .sort_by_key(|reaction| std::cmp::Reverse(reaction.count));
        // The server sends every emoji and its tally (hundreds of them). Keep the
        // busiest few for the pills plus one total, so the persisted chat cache
        // does not grow by kilobytes per post.
        stats.reaction_total = Some(
            stats
                .reactions
                .iter()
                .map(|reaction| u64::from(reaction.count))
                .sum(),
        );
        stats.reactions.truncate(3);
    }
    (stats.views.is_some() || stats.forwards.is_some() || !stats.reactions.is_empty())
        .then_some(stats)
}

/// Message view + optional body (media post, or a link post with a CDN
/// thumbnail — both cached for downloads / `link_thumbnail`).
pub(crate) type ChannelPost = (
    MessageView,
    Option<Arc<wa::Message>>,
    Option<Arc<wa::Message>>,
);

/// One decoded page of a channel's `<messages>` reply.
pub(crate) struct ChannelPage {
    /// Message view, its protobuf when it carries media, or the body of a link
    /// post with a CDN thumbnail (also cached, for `link_thumbnail`).
    pub posts: Vec<ChannelPost>,
    /// Smallest `server_id` in the page: the `before` cursor for the next one.
    pub cursor: Option<u64>,
    /// `<message>` children the server returned, before any were skipped. A page
    /// shorter than the requested count is the end of the channel's history, the
    /// same rule WhatsApp Web uses (`end: messages.length < count`).
    pub messages: usize,
}

/// Decode `<messages><message id server_id t><plaintext/></message>…` into posts.
///
/// WhatsApp ships the body as a protobuf inside `<plaintext>`; entries without a
/// `server_id` cannot be paged or keyed, so they are skipped like the server
/// helper does.
pub(crate) fn decode_channel_page(
    list: &whatsapp_rust::wacore_binary::NodeRef<'_>,
    jid: &Jid,
    name: &str,
    contacts: &Mutex<HashMap<String, String>>,
) -> ChannelPage {
    let names: Mutex<HashMap<String, String>> = Mutex::new(HashMap::new());
    let aliases: Mutex<HashMap<String, String>> = Mutex::new(HashMap::new());
    let mut posts = Vec::new();
    let mut cursor: Option<u64> = None;
    let mut messages = 0usize;
    let Some(children) = list.children() else {
        return ChannelPage {
            posts,
            cursor,
            messages,
        };
    };
    for node in children
        .iter()
        .filter(|node| node.tag.as_ref() == "message")
    {
        messages += 1;
        let Some(server_id) = node
            .get_attr("server_id")
            .and_then(|value| value.as_str().parse::<u64>().ok())
        else {
            continue;
        };
        cursor = Some(cursor.map_or(server_id, |current: u64| current.min(server_id)));
        let id = node
            .get_attr("id")
            .map(|value| value.as_str().into_owned())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| server_id.to_string());
        let timestamp = node
            .get_attr("t")
            .and_then(|value| value.as_str().parse::<u64>().ok())
            .unwrap_or(0);
        let Some(body) = node
            .get_optional_child("plaintext")
            .and_then(|plain| plain.content_bytes())
            .and_then(decode_newsletter_body)
        else {
            continue;
        };
        // Newsletter history can wrap the post (`editedMessage`,
        // `documentWithCaptionMessage`, view-once, …) exactly like a chat
        // message, so read the unwrapped body. The channel's posts are all
        // `imageMessage` captions; without this they decoded to an empty "text"
        // post and were dropped.
        let base = body.get_base_message();
        let kind = media_kind(base).unwrap_or("text");
        let text = message_text(base).unwrap_or_default();
        if kind == "text" && text.trim().is_empty() {
            continue;
        }
        let view = MessageView {
            chat: jid.to_string(),
            chat_name: name.to_owned(),
            // Channel posts are authored by the channel itself.
            sender: None,
            sender_name: None,
            sender_phone: None,
            id,
            from_me: false,
            is_group: false,
            timestamp: (timestamp as i64).saturating_mul(1000),
            kind: kind.to_owned(),
            text,
            mentions: mentioned_jids(base),
            reply_to: reply_view(base, &names, &aliases, contacts),
            link: link_preview(base),
            document: document_view(base),
            audio: audio_view(base),
            stats: channel_stats(node),
            forwarded: is_forwarded(base),
            once_recovered: false,
            gif: false,
            status_quote: false,
            contact: None,
            poll: None,
        };
        let media = (kind != "text").then(|| Arc::new(body.clone()));
        // Link posts that ship a big CDN thumbnail also hand their body over,
        // so `fetch_channel_history` can cache the protobuf and the UI's
        // `link_thumbnail` can fetch the image later.
        let body_for_cache = (media.is_none()
            && link_preview(base).is_some_and(|link| link.thumb_path.is_some()))
        .then(|| Arc::new(body));
        posts.push((view, media, body_for_cache));
    }
    ChannelPage {
        posts,
        cursor,
        messages,
    }
}
