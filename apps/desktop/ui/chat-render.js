/* WA Desk frontend: chat-render. Loaded in order by index.html; all files share the classic-script global scope. */

let lastRenderedChat = null;
const SCROLL_BOTTOM_THRESHOLD = 120;
let scrollPumpToken = 0;
let scrollLatestUnread = false;
let lastMessageScrollTop = 0;
let messageMediaObserver = null;

function distanceFromBottom(box) {
  return box.scrollHeight - box.scrollTop - box.clientHeight;
}

function setScrollLatestUnread(value) {
  scrollLatestUnread = Boolean(value);
  updateScrollLatestButton();
}

function updateScrollLatestButton() {
  const box = $("messages");
  if (!$("scroll-latest")) return;
  refreshScrollLatestState(box, distanceFromBottom(box));
}

/// The scroll-to-latest state from metrics that were already read, so a repaint
/// does not force a second layout just to show or hide the button.
function refreshScrollLatestState(box, distance) {
  const button = $("scroll-latest");
  if (!button) return;
  const show = box.childElementCount > 0 && distance > 8;
  button.hidden = !show;
  const dot = $("scroll-latest-dot");
  if (dot) dot.hidden = !(show && scrollLatestUnread);
}

/// The scroll-to-latest disc floats just above the composer, so it has to lift
/// clear of the reply / edit / media-draft cards that sit between the messages
/// and the composer — otherwise it lands on top of their close button. The
/// official client lifts it the same way.
function initScrollLatestOffset() {
  const button = $("scroll-latest");
  if (!button) return;
  const parts = ["composer", "reply-banner", "edit-banner", "draft"]
    .map((id) => $(id))
    .filter(Boolean);
  const apply = () => {
    let stack = 0;
    for (const el of parts) {
      if (el.hidden) continue;
      const style = getComputedStyle(el);
      stack +=
        el.offsetHeight +
        parseFloat(style.marginTop || 0) +
        parseFloat(style.marginBottom || 0);
    }
    // 19px keeps the gap the button had above the bare composer.
    button.style.bottom = `${Math.round(stack + 19)}px`;
  };
  // A hidden element reports a zero box, so showing or dismissing a card
  // re-runs the measurement without any scroll-time layout reads.
  const observer = new ResizeObserver(apply);
  for (const el of parts) observer.observe(el);
  apply();
}

function topVisibleMessage(box) {
  const top = box.scrollTop + 1;
  for (const node of box.querySelectorAll("[data-id]")) {
    if (node.offsetTop + node.offsetHeight > top) {
      return { id: node.dataset.id, offset: node.offsetTop - box.scrollTop };
    }
  }
  return null;
}

function pumpStickToBottom(duration = 1200) {
  const box = $("messages");
  const token = ++scrollPumpToken;
  const until = performance.now() + duration;
  const step = () => {
    if (token !== scrollPumpToken || box.dataset.stickBottom !== "1") return;
    const height = box.scrollHeight;
    const client = box.clientHeight;
    box.scrollTop = height;
    lastMessageScrollTop = box.scrollTop;
    refreshScrollLatestState(box, height - lastMessageScrollTop - client);
    if (performance.now() < until) requestAnimationFrame(step);
  };
  requestAnimationFrame(step);
}

function stickToBottomIfNeeded() {
  const box = $("messages");
  if (box.dataset.stickBottom !== "1") return;
  const height = box.scrollHeight;
  const client = box.clientHeight;
  box.scrollTop = height;
  lastMessageScrollTop = box.scrollTop;
  refreshScrollLatestState(box, height - lastMessageScrollTop - client);
}

function bindMessageMediaStick() {
  const box = $("messages");
  for (const img of box.querySelectorAll("img")) {
    if (img.dataset.stickBound) continue;
    img.dataset.stickBound = "1";
    img.addEventListener("load", stickToBottomIfNeeded, { once: true });
  }
  for (const video of box.querySelectorAll("video")) {
    if (video.dataset.stickBound) continue;
    video.dataset.stickBound = "1";
    video.addEventListener("loadedmetadata", stickToBottomIfNeeded, { once: true });
  }
}

function watchMessageMedia() {
  bindMessageMediaStick();
  if (messageMediaObserver) return;
  messageMediaObserver = new MutationObserver(() => {
    bindMessageMediaStick();
    stickToBottomIfNeeded();
  });
  messageMediaObserver.observe($("messages"), { childList: true, subtree: true });
}

function scrollToLatest() {
  const box = $("messages");
  scrollLatestUnread = false;
  box.dataset.stickBottom = "1";
  // Read the fresh metrics once: measuring again inside the button update was a
  // second forced layout on every repaint.
  const height = box.scrollHeight;
  const client = box.clientHeight;
  box.scrollTop = height;
  lastMessageScrollTop = box.scrollTop;
  refreshScrollLatestState(box, height - lastMessageScrollTop - client);
  pumpStickToBottom();
}

// Asynchronous hydration (document metadata/thumbnails, phone lookups) can
// resolve a dozen times in a row for one chat. Each resolution used to rebuild
// every bubble; coalescing keeps that to a single repaint per burst.
let chatRenderTimer = null;

function scheduleChatRender(delay = 60) {
  if (chatRenderTimer) return;
  chatRenderTimer = setTimeout(() => {
    chatRenderTimer = null;
    renderChat();
  }, delay);
}

// The official "You received a view once message…" placeholder body: one dim
// italic line with the round clock icon, and a Learn-more anchor that opens
// the FAQ in the default browser (`target="_blank"` routes it through
// open_link; without the target the WebView2 navigated the app window
// itself). A recovered copy (content kept locally through reply-quoting)
// passes its id: the placeholder becomes clickable and opens the media
// viewer — the official tap-to-view affordance, no rail eye needed. The
// affordance is a hover tooltip so the bubble keeps the official look.
function viewOncePlaceholderBody(openId = null) {
  const cls = openId ? "body view-once openable" : "body view-once";
  const openAttr = openId
    ? ` data-view-once-open="${escapeHtml(openId)}" title="Tap to view"`
    : "";
  return `
      <div class="${cls}"${openAttr}>
        <span class="view-once-icon" aria-hidden="true"><svg viewBox="0 -960 960 960"><path d="m612-292 56-56-148-148v-184h-80v216l172 172ZM480-80q-83 0-156-31.5T197-197q-54-54-85.5-127T80-480q0-83 31.5-156T197-763q54-54 127-85.5T480-880q83 0 156 31.5T763-763q54 54 85.5 127T880-480q0 83-31.5 156T763-197q-54 54-127 85.5T480-80Zm0-400Zm0 320q133 0 226.5-93.5T800-480q0-133-93.5-226.5T480-800q-133 0-226.5 93.5T160-480q0 133 93.5 226.5T480-160Z"/></svg></span>
        You received a view once message. For added privacy, you can only open it on your phone.
        <a class="view-once-more" href="https://faq.whatsapp.com/578442220724722/?cms_platform=android" target="_blank" rel="noreferrer">Learn more</a>
      </div>`;
}

/// A sender's photo can arrive after the bubbles are already painted: swap the
/// initials for the picture in place instead of rebuilding the whole chat.
function patchMessageAvatar(jid) {
  if (!jid) return;
  const url = state.photos.get(jid);
  if (!url) return;
  const box = $("messages");
  if (!box) return;
  const nodes = box.querySelectorAll(`.msg-avatar[data-jid="${CSS.escape(jid)}"]`);
  for (const node of nodes) {
    node.innerHTML = `<img class="avatar-img" src="${escapeHtml(url)}" alt="" onerror="window.__waAvatarError(this)">`;
  }
}

function renderChat() {
  const chat = state.chats.get(state.active);
  const panel = $("chat-panel");
  const placeholder = $("chat-empty");

  if (!chat) {
    panel.hidden = true;
    placeholder.hidden = false;
    return;
  }

  panel.hidden = false;
  placeholder.hidden = true;
  const channel = isChannelJid(chat.jid);
  $("chat-name").textContent = chatDisplayName(chat);
  const cachedPhone = state.phones.get(chat.jid);
  const knownPhone =
    cachedPhone && cachedPhone !== "loading" && cachedPhone !== "missing" ? cachedPhone : "";
  $("chat-sub").textContent = channel
    ? formatFollowers(chat.subscribers)
    : WAChatUtils.chatSubtitle(
        chat.jid,
        chat.isGroup,
        state.meJid,
        state.meLid,
        knownPhone,
      );
  // Channels are broadcast-only: no composer, calls, or per-chat menu. The
  // import button stays: it is the way to backfill posts the phone never synced.
  $("composer").hidden = channel;
  $("chat-voice").hidden = channel;
  $("chat-video").hidden = channel;
  $("chat-menu").hidden = channel;
  $("chat-import").hidden = false;
  $("chat-import").title = channel ? "Import channel posts (.txt)" : "Import chat export";
  if (
    !channel &&
    !chat.isGroup &&
    !knownPhone &&
    chat.jid !== state.meJid &&
    chat.jid !== state.meLid &&
    String(chat.jid).endsWith("@lid")
  ) {
    resolvePhone(chat.jid).then((phone) => {
      if (phone && state.active === chat.jid && !$("chat-panel").hidden) {
        $("chat-sub").textContent = WAChatUtils.chatSubtitle(
          chat.jid,
          chat.isGroup,
          state.meJid,
          state.meLid,
          phone,
        );
      }
    });
  }
  setAvatar($("chat-avatar"), chat.name, chat.jid);
  queuePhoto(chat.jid);

  const box = $("messages");
  const sameChat = lastRenderedChat === chat.jid;
  if (!sameChat) {
    scrollLatestUnread = false;
    lastMessageScrollTop = 0;
  }
  const wasAtBottom = distanceFromBottom(box) <= SCROLL_BOTTOM_THRESHOLD;
  const previousScrollTop = box.scrollTop;
  const anchor = sameChat ? topVisibleMessage(box) : null;
  box.classList.toggle("channel-view", channel);
  // Group chats reserve a left gutter for the sender's profile photo.
  box.classList.toggle("group-view", chat.isGroup && !channel);
  if (chat.messages.length === 0) {
    box.innerHTML = isChannelJid(chat.jid)
      ? '<p class="empty">No posts synced from this channel yet. WhatsApp only shares channel posts received while this device is linked — use the import button in the header to load an exported <em>.txt</em> of its posts.</p>'
      : '<p class="empty">No messages loaded for this chat yet. New messages will appear here.</p>';
    box.dataset.stickBottom = "1";
    lastMessageScrollTop = 0;
    lastRenderedChat = chat.jid;
    updateScrollLatestButton();
    return;
  }

  let lastDay = null;
  let previous = null;
  const html = [];
  const visible = chat.messages.filter((message) =>
    WAChatUtils.isMessageVisible(message, state.showDeleted),
  );

  for (let index = 0; index < visible.length; index += 1) {
    const message = visible[index];

    const day = dayOf(message.timestamp);
    if (day !== lastDay) {
      html.push(`<div class="day">${escapeHtml(day)}</div>`);
      lastDay = day;
      previous = null;
    }

    const classes = ["bubble"];
    if (message.from_me) classes.push("out");
    if (message.forwarded) classes.push("is-forwarded");
    if (state.selecting && state.selected && state.selected.has(message.id)) {
      classes.push("selected");
    }
    // Reaction pills hang off the bottom edge of the bubble (WhatsApp Web), so
    // the bubble needs a little extra room when it carries any.
    if (message.reactions && message.reactions.length) classes.push("has-reactions");

    // WhatsApp only draws the tail on the first bubble of a run, and shows the
    // sender's name and photo there too.
    const sameRun =
      previous &&
      previous.from_me === message.from_me &&
      previous.sender === message.sender &&
      message.timestamp - previous.timestamp < 5 * 60_000;
    if (!sameRun) classes.push("first");
    const firstOfRun = !sameRun;

    // Photos/videos sent in one burst collapse into an album grid.
    const album = collectAlbum(visible, index);
    if (album) {
      const last = album[album.length - 1];
      const albumClasses = ["bubble", "album", `album-${Math.min(album.length, 4)}`];
      if (last.from_me) albumClasses.push("out");
      if (!sameRun) albumClasses.push("first");
      if (last.reactions && last.reactions.length) albumClasses.push("has-reactions");
      if (last.forwarded) albumClasses.push("is-forwarded");

      const cells = album
        .slice(0, 4)        .map((item, cell) => {
          const more =
            cell === 3 && album.length > 4
              ? `<span class="album-more">+${album.length - 4}</span>`
              : "";
          return `<div class="album-cell">${mediaBlock(item)}${more}</div>`;
        })
        .join("");
      const caption = [...album].reverse().find((item) => item.text && item.text.trim());

      html.push(`
        <div class="${albumClasses.join(" ")}" data-id="${escapeHtml(message.id)}">
          ${forwardedHtml(last)}
          ${state.selecting ? '<span class="select-tick"></span>' : ""}
          ${firstOfRun ? messageAvatarHtml(chat, message) : ""}
          ${firstOfRun ? senderHtml(chat, message) : ""}
          ${replyQuoteHtml(message.reply_to)}
          <div class="album-grid album-${Math.min(album.length, 4)}">${cells}</div>
          ${caption ? `<div class="body">${bodyHtml(caption.text, caption.mentions)}</div>` : ""}
          ${reactionHtml(last)}
          <span class="meta">${timeOf(last.timestamp)}${tickHtml(last)}</span>
        </div>`);

      previous = last;
      index += album.length - 1;
      continue;
    }

    const avatar = firstOfRun ? messageAvatarHtml(chat, message) : "";
    let sender = firstOfRun ? senderHtml(chat, message) : "";
    // A status reply/reaction wears WhatsApp's "· Status" card, not a reply quote.
    if (message.status_quote) classes.push("status-interaction");
    const quoted = message.status_quote
      ? statusQuoteHtml(message.reply_to)
      : replyQuoteHtml(message.reply_to);

    let body;
    let docActions = "";
    let docOwnsMeta = false;
    let voiceOwnsMeta = false;
    if (message.deleted) {
      classes.push("deleted");
      const revealed = WAChatUtils.deletedMessageText(message, state.showDeleted);
      body = revealed
        ? `<div class="body">${bodyHtml(revealed)}</div><span class="deleted-tag">deleted</span>`
        : '<div class="body muted">This message was deleted</div>';
    } else if (message.kind === "document") {
      classes.push("has-media", "doc-bubble");
      // The caption and the time/ticks share one flex row, so the float-based
      // timestamp of text bubbles can never collide with the action row.
      const fileName = documentInfo(message).name;
      const captionText = String(message.text || "").trim();
      const caption =
        captionText && captionText !== fileName
          ? `<div class="body doc-caption">${bodyHtml(message.text, message.mentions)}</div>`
          : "";
      body = `${documentCard(message)}
        <div class="doc-footer">${caption}<span class="meta doc-time">${timeOf(
          message.timestamp,
        )}${tickHtml(message)}</span></div>`;
      docActions = documentActions(message);
      docOwnsMeta = true;
    } else if (MEDIA_LABELS[message.kind]) {
      // View-once copies recovered through reply-quoting respect the rail
      // eye: eye off renders the official compact placeholder (still locally
      // cached and tap-openable), eye on shows the kept media with its
      // caption. The placeholder must not wear the media-bubble width cap.
      const hidden = message.once_recovered && !state.showDeleted;
      if (hidden) classes.push("view-once-bubble");
      else {
        classes.push("has-media");
        if (message.kind === "sticker") classes.push("sticker");
      }
      const caption = message.text ? channelCaption(message) : "";
      body = hidden
        ? viewOncePlaceholderBody(message.id)
        : `${mediaBlock(message)}${caption}`;
      // Audio bubbles render "0:17   6:46 am" inside the player footer, so
      // they own the timestamp like a document card does.
      if (message.kind === "audio") {
        classes.push("voice");
        voiceOwnsMeta = true;
      }
    } else if (message.text) {
      // WhatsApp renders emoji-only messages much larger.
      const emojiOnly = isEmojiOnly(message.text);
      if (emojiOnly) classes.push("emoji-only");
      const embedded = message.link;
      const hasEmbedded =
        embedded && (embedded.title || embedded.thumb || embedded.description);
      const previewUrl = hasEmbedded ? embedded.url : WAChatUtils.firstUrl(message.text);
      // Channel/broadcast link posts ship the big preview image as a CDN path
      // (`thumbnailDirectPath`); the card renders the inline thumb at once and
      // swaps in the large image when it lands, like WhatsApp Web.
      const wantsBigThumb =
        hasEmbedded && embedded.thumb_path && isChannelJid(message.chat);
      const thumbAttrs = wantsBigThumb
        ? ` data-thumb-id="${escapeHtml(message.id)}"`
        : "";
      const preview = hasEmbedded
        ? `<div class="link-preview ready"${thumbAttrs}>${previewCard({
            url: embedded.url,
            title: embedded.title,
            description: embedded.description,
            site: WAChatUtils.siteOf(embedded.url),
            image: embedded.thumb,
          })}</div>`
        : previewUrl
          ? `<div class="link-preview" data-preview-url="${escapeHtml(previewUrl)}"></div>`
          : "";
      body = `${preview}${channelCaption(message)}`;
    } else if (message.kind === "view_once") {
      // The linked device never decrypts view-once fanouts (the phone keeps
      // the media); the official UI still shows a placeholder bubble, dim,
      // with the clock icon and no ticks. No content exists to open.
      classes.push("unsupported", "view-once-bubble");
      body = viewOncePlaceholderBody();
    } else {
      classes.push("unsupported");
      body = '<div class="body muted">Unsupported message</div>';
    }

    html.push(`
      <div class="${classes.join(" ")}" data-id="${escapeHtml(message.id)}">
        ${forwardedHtml(message)}
        ${state.selecting ? '<span class="select-tick"></span>' : ""}
        ${avatar}
        ${sender}
        ${quoted}
        ${body}
        ${reactionHtml(message)}
        ${
          docOwnsMeta || voiceOwnsMeta
            ? ""
            : `<span class="meta">${message.edited ? '<span class="edited">edited</span>' : ""}${
                state.starred.has(message.id)
                  ? `<span class="msg-star" title="Starred">${waIcon("star")}</span>`
                  : ""
              }<span>${timeOf(message.timestamp)}</span>${tickHtml(message)}</span>`
        }
        ${docActions}
      </div>`);
    if (isChannelJid(chat.jid)) html.push(channelStatsHtml(message));

    previous = message;
  }

  box.innerHTML = html.join("");
  hydrateMedia(box);
  hydrateDocumentCards(box);
  hydratePreviews(box);
  hydrateLinkThumbs(box);
  hydratePhones(box);
  watchMessageMedia();
  // A chat switch (or a reply while already at the bottom) should land on the
  // newest message. While reading older messages in the same chat, anchor the
  // top visible bubble so new messages/media do not move the viewport.
  const stick = !sameChat || wasAtBottom;
  box.dataset.stickBottom = stick ? "1" : "0";
  if (stick) {
    scrollToLatest();
  } else if (anchor) {
    const node = box.querySelector(`[data-id="${CSS.escape(anchor.id)}"]`);
    box.scrollTop = node ? Math.max(0, node.offsetTop - anchor.offset) : previousScrollTop;
  } else {
    box.scrollTop = previousScrollTop;
  }
  updateScrollLatestButton();
  lastMessageScrollTop = box.scrollTop;
  lastRenderedChat = chat.jid;
  if (!$("chat-search-bar").hidden) runChatSearch($("chat-search-input").value, true);
  scheduleAiContext();
}

function tickHtml(message) {
  if (!message.from_me) return "";
  if (message.pending)
    return `<span class="tick">${waIcon("hourglass_empty")}</span>`;
  // "Message yourself" mirrors the official client: the phone owns the chat,
  // so our own messages are delivered the moment they land (grey double tick).
  if (typeof isSelfJid === "function" && isSelfJid(message.chat)) {
    return `<span class="tick">${waIcon("done_all")}</span>`;
  }
  const status = message.status || "sent";
  if (status === "read" || status === "played") {
    return `<span class="tick read">${waIcon("done_all")}</span>`;
  }
  if (status === "delivered") return `<span class="tick">${waIcon("done_all")}</span>`;
  return `<span class="tick">${waIcon("check")}</span>`;
}

// Channel captions run long; the desktop app truncates them and offers
// "Read more" instead of pushing the next post off screen.
function channelCaption(message) {
  const html = bodyHtml(message.text, message.mentions);
  if (!isChannelJid(message.chat) || String(message.text || "").length < 420) {
    return `<div class="body">${html}</div>`;
  }
  return `<div class="body clamp">${html}</div><button class="channel-more" type="button">Read more</button>`;
}

document.addEventListener("click", (event) => {
  const more = event.target.closest && event.target.closest(".channel-more");
  if (!more) return;
  const bubble = more.closest(".bubble");
  const body = bubble && bubble.querySelector(".body.clamp");
  if (body) body.classList.remove("clamp");
  more.remove();
});

// "↱ Forwarded", drawn above the bubble like WhatsApp Web.
function forwardedHtml(message) {
  if (!message.forwarded) return "";
  return `<span class="forwarded"><svg viewBox="0 -960 960 960" aria-hidden="true"><path d="m640-280-57-56 184-184-184-184 57-56 240 240-240 240ZM80-200v-160q0-83 58.5-141.5T280-560h247L383-704l57-56 240 240-240 240-57-56 144-144H280q-50 0-85 35t-35 85v160H80Z"/></svg>Forwarded</span>`;
}

// "95k" / "1.2m", the way WhatsApp abbreviates counters.
function compactCount(value) {
  const n = Number(value) || 0;
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1).replace(/\.0$/, "")}m`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1).replace(/\.0$/, "")}k`;
  return String(n);
}

// Channel posts carry the server's counters (`views_count`, `forwards_count`,
// `<reactions>`); the desktop app draws them under the card, so the message list
// gets one extra row after the bubble.
function channelStatsHtml(message) {
  const stats = message.stats;
  if (!stats) return "";
  const pills = (stats.reactions || [])
    .slice(0, 3)
    .map(
      (reaction) =>
        `<span class="channel-reaction">${escapeHtml(reaction.code)}<b>${compactCount(
          reaction.count,
        )}</b></span>`,
    )
    .join("");
  const total =
    Number(stats.reaction_total) ||
    (stats.reactions || []).reduce((sum, r) => sum + (Number(r.count) || 0), 0);  const reactions = pills
    ? `<span class="channel-reaction-group" title="Reactions">${pills}${
        total > 0 ? `<span class="channel-reaction-total">${compactCount(total)}</span>` : ""
      }</span>`
    : "";
  const forwards = stats.forwards
    ? `<span class="channel-forward" title="Forwarded"><svg viewBox="0 -960 960 960" aria-hidden="true"><path d="m640-280-57-56 184-184-184-184 57-56 240 240-240 240ZM80-200v-160q0-83 58.5-141.5T280-560h247L383-704l57-56 240 240-240 240-57-56 144-144H280q-50 0-85 35t-35 85v160H80Z"/></svg>${compactCount(
        stats.forwards,
      )}</span>`
    : "";
  const views = stats.views ? `<span class="channel-views" title="Views">${compactCount(stats.views)}</span>` : "";
  if (!reactions && !forwards && !views) return "";
  return `<div class="channel-stats" data-for="${escapeHtml(message.id)}">${reactions}${forwards}${views}</div>`;
}

// Group reactions by emoji, WhatsApp-style: "👍 2 ❤️".
function reactionHtml(message) {
  const counts = new Map();
  for (const reaction of message.reactions || []) {
    if (!reaction || !reaction.emoji) continue;
    counts.set(reaction.emoji, (counts.get(reaction.emoji) || 0) + 1);
  }
  if (counts.size === 0) return "";
  const pills = [...counts.entries()]
    .map(
      ([emoji, count]) =>
        `<span class="reaction-pill">${escapeHtml(emoji)}${count > 1 ? ` ${count}` : ""}</span>`,
    )
    .join("");
  return `<div class="reactions">${pills}</div>`;
}

async function applyReaction(message, emoji) {
  if (!message || !state.active) return;
  const mine = (message.reactions || []).find((reaction) => reaction.by === "me");
  const next = mine && mine.emoji === emoji ? "" : emoji;
  const chat = state.active;
  const chatInfo = state.chats.get(chat);

  const send = (value) =>
    invoke("send_reaction", {
      chat,
      id: message.id,
      fromMe: Boolean(message.from_me),
      // A participant is only part of the key for incoming GROUP messages;
      // DMs address the reaction directly at the chat.
      participant:
        chatInfo?.isGroup && !message.from_me ? message.sender || null : null,
      emoji: value,
    });

  try {
    if (mine && next) {
      // Changing a reaction: revoke the old one first, then send the new emoji.
      // A plain replacement is not applied consistently by the server.
      await send("");
      await send(next);
    } else {
      await send(next);
    }
    message.reactions = (message.reactions || []).filter((reaction) => reaction.by !== "me");
    if (next) message.reactions.push({ by: "me", emoji: next, ts: Date.now() });
    renderChat();
    scheduleSave();
  } catch (err) {
    toast(String(err));
  }
}

// The name WhatsApp prints above a group bubble: the saved contact name, the
// push name with WhatsApp Web's "~" marker for members the address book does
// not know, or the phone number when that is all we have.
function senderDisplayName(chat, message) {
  const saved = contactNameFor(message.sender);
  // History chunks sometimes carry no push name for a sender, so borrow the
  // name another message of the same sender has.
  const sibling = chat.messages.find(
    (entry) => entry.sender === message.sender && entry.sender_name,
  );
  const push = saved ? "" : message.sender_name || (sibling && sibling.sender_name) || "";
  // Never derive a phone number from a LID: only the PN form (resolved by the
  // protocol layer) is a real international number.
  const isLid = Boolean(message.sender && String(message.sender).endsWith("@lid"));
  const phoneSource =
    message.sender_phone ||
    (message.sender && !isLid ? WAChatUtils.phoneFromJid(message.sender) : "");
  const phone = WAChatUtils.formatPhone(phoneSource);
  return saved || (push ? `~ ${push}` : "") || phone || "Unknown";
}

function senderHtml(chat, message) {
  if (!chat.isGroup || message.from_me) return "";
  const name = senderDisplayName(chat, message);
  // Nothing but a LID to go on: the protocol layer may still resolve a phone
  // number, and WhatsApp shows it in place of the name once it lands.
  const unresolved =
    name === "Unknown" && String(message.sender || "").endsWith("@lid")
      ? ` data-phone-jid="${escapeHtml(message.sender)}"`
      : "";
  return `<div class="sender" style="color:${WAChatUtils.senderColor(
    message.sender || name,
  )}"><span class="sender-name"${unresolved}>${escapeHtml(name)}</span></div>`;
}

// WhatsApp puts the sender's profile photo in the left gutter of a group chat,
// next to the first bubble of each sender run.
function messageAvatarHtml(chat, message) {
  if (!chat.isGroup || message.from_me || isChannelJid(chat.jid)) return "";
  if (!message.sender) return "";
  queuePhoto(message.sender);
  return avatarHtml(senderDisplayName(chat, message), message.sender, "avatar msg-avatar");
}

// Resolve a LID to its phone-number JID once, sharing in-flight requests.
const phoneRequests = new Map();

function resolvePhone(jid) {
  const cached = state.phones.get(jid);
  if (cached === "missing") return Promise.resolve(null);
  if (cached && cached !== "loading") return Promise.resolve(cached);
  if (phoneRequests.has(jid)) return phoneRequests.get(jid);

  const request = invoke("resolve_phone", { jid })
    .then((phone) => {
      state.phones.set(jid, phone || "missing");
      while (state.phones.size > 500) {
        state.phones.delete(state.phones.keys().next().value);
      }
      return phone || null;
    })
    .catch(() => {
      state.phones.set(jid, "missing");
      return null;
    })
    .finally(() => phoneRequests.delete(jid));

  phoneRequests.set(jid, request);
  return request;
}

// Old cached group messages have no resolved PN; ask the protocol layer once.
// For a sender we know nothing about, the number becomes the display name.
function hydratePhones(box) {
  for (const node of box.querySelectorAll("[data-phone-jid]")) {
    const jid = node.dataset.phoneJid;
    const isName = node.classList.contains("sender-name");
    const paint = (phone) => {
      const text = WAChatUtils.formatPhone(phone);
      node.innerHTML = isName ? escapeHtml(text) : `${waIcon("call", "phone-glyph")} ${text}`;
    };
    const cached = state.phones.get(jid);
    if (cached === "missing") {
      if (!isName) node.remove();
      continue;
    }
    if (cached && cached !== "loading") {
      paint(cached);
      continue;
    }
    resolvePhone(jid).then((phone) => {
      if (!phone) {
        if (!isName && node.isConnected) node.remove();
        return;
      }
      if (node.isConnected) paint(phone);
    });
  }
}

// Consecutive photos/videos from the same person collapse into one album.
function collectAlbum(list, start) {
  const first = list[start];
  if (!first || first.deleted || (first.kind !== "image" && first.kind !== "video")) {
    return null;
  }
  const items = [first];
  for (let i = start + 1; i < list.length && items.length < 9; i += 1) {
    const next = list[i];
    if (next.deleted || (next.kind !== "image" && next.kind !== "video")) break;
    if (next.from_me !== first.from_me || next.sender !== first.sender) break;
    if (next.timestamp - items[items.length - 1].timestamp > 4000) break;
    items.push(next);
  }
  return items.length >= 2 ? items : null;
}

