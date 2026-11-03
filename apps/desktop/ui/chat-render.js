/* WA Desk frontend: chat-render. Loaded in order by index.html; all files share the classic-script global scope. */

let lastRenderedChat = null;
// Mirrors wa_ui::scroll constants; queried once at boot so the sync scroll
// handler never awaits.
let SCROLL_BOTTOM_THRESHOLD = 120;
let LATEST_BUTTON_GAP_PX = 8;
invoke("ui_scroll_constants")
  .then((constants) => {
    SCROLL_BOTTOM_THRESHOLD = constants.follow_threshold_px;
    LATEST_BUTTON_GAP_PX = constants.latest_button_gap_px;
  })
  .catch(() => {});
let scrollPumpToken = 0;
let scrollLatestUnread = false;
let lastMessageScrollTop = 0;
let messageMediaObserver = null;

function distanceFromBottom(box) {
  // Same arithmetic as wa_ui::scroll::distance_from_bottom (kept sync: this runs
  // on every scroll event and inside repaint decisions).
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
  // The disc rule is wa_ui::scroll::show_latest_button (kept sync for the
  // scroll handler); the thresholds are mirrored from that module.
  const show = box.childElementCount > 0 && distance > LATEST_BUTTON_GAP_PX;
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
  let offsetToken = 0;
  const apply = () => {
    const heights = [];
    for (const el of parts) {
      if (el.hidden) continue;
      const style = getComputedStyle(el);
      heights.push(
        el.offsetHeight +
          parseFloat(style.marginTop || 0) +
          parseFloat(style.marginBottom || 0),
      );
    }
    // The stack sum and the 19px resting gap are wa_ui::scroll's rules.
    const token = ++offsetToken;
    invoke("ui_scroll_latest_offset", { visibleHeights: heights })
      .then((bottom) => {
        if (token === offsetToken) button.style.bottom = `${Math.round(bottom)}px`;
      })
      .catch(() => {});
  };
  // A hidden element reports a zero box, so showing or dismissing a card
  // re-runs the measurement without any scroll-time layout reads.
  const observer = new ResizeObserver(apply);
  for (const el of parts) observer.observe(el);
  apply();
}

/// The first bubble still visible at the top (wa_ui::scroll::visible_anchor),
/// used to keep the viewport still while older messages load.
async function topVisibleMessage(box) {
  const nodes = [...box.querySelectorAll("[data-id]")];
  if (!nodes.length) return null;
  return invoke("ui_scroll_visible_anchor", {
    nodes: nodes.map((node) => ({
      id: node.dataset.id || "",
      offset_top: node.offsetTop,
      offset_height: node.offsetHeight,
    })),
    scrollTop: box.scrollTop,
  }).catch(() => null);
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

/* --------------------------------------------------- Rust-rendered messages */

// The bubble markup (day dividers, albums, quotes, media cards, documents,
// link previews, reactions, ticks) is built by `wa-ui::chat`; this file owns
// the state → request mapping, hydration and scrolling only.
let chatRenderSeq = 0;

/// One message reduced to the fields the renderer reads. Big blobs (media data
/// URIs, link thumbnails) are trimmed so the IPC payload stays small.
function chatMessage(message) {
  const reply = message.reply_to;
  return {
    id: message.id || "",
    text: message.text || "",
    kind: message.kind || "text",
    from_me: Boolean(message.from_me),
    status: message.status || "",
    sender: message.sender || "",
    sender_name: message.sender_name || "",
    sender_phone: message.sender_phone || "",
    deleted: Boolean(message.deleted),
    deleted_for_me: Boolean(message.deletedForMe),
    cleared: Boolean(message.cleared),
    timestamp: message.timestamp || 0,
    forwarded: Boolean(message.forwarded),
    pending: Boolean(message.pending),
    edited: Boolean(message.edited),
    once_recovered: Boolean(message.once_recovered),
    status_quote: Boolean(message.status_quote),
    mentions: Array.isArray(message.mentions) ? message.mentions : [],
    reactions: (message.reactions || []).map((reaction) => ({
      emoji: reaction.emoji || "",
      from_me: reaction.by === "me",
    })),
    reply_to: reply
      ? {
          id: reply.id || "",
          sender: reply.sender || "",
          sender_name: reply.sender_name || "",
          text: reply.text || "",
          kind: reply.kind || "",
          thumb: reply.thumb || null,
        }
      : null,
    link: message.link
      ? {
          url: message.link.url || "",
          title: message.link.title || "",
          description: message.link.description || "",
          thumb: message.link.thumb || null,
          thumb_path: message.link.thumb_path || null,
        }
      : null,
    document: message.document
      ? {
          name: message.document.name || "",
          mime: message.document.mime || "",
          size: message.document.size || 0,
          pages: message.document.pages || 0,
          thumb: message.document.thumb || null,
        }
      : null,
    audio: message.audio
      ? {
          ptt: Boolean(message.audio.ptt),
          seconds: message.audio.seconds || 0,
          waveform: message.audio.waveform || [],
        }
      : null,
    stats: message.stats
      ? {
          reactions: (message.stats.reactions || []).map((reaction) => ({
            code: reaction.code || "",
            count: reaction.count || 0,
          })),
          reaction_total: message.stats.reaction_total || 0,
          forwards: message.stats.forwards || 0,
          views: message.stats.views || 0,
        }
      : null,
  };
}

function chatRequest(chat) {
  const media = {};
  for (const [id, entry] of state.media) {
    media[id] = { status: entry.status || "", url: entry.url || "" };
  }
  const knownPhone = state.phones.get(chat.jid);
  return {
    chat: {
      jid: chat.jid,
      name: chat.name || "",
      is_group: Boolean(chat.isGroup),
      is_channel: isChannelJid(chat.jid),
      subscribers: chat.subscribers || 0,
    },
    messages: chat.messages.map(chatMessage),
    media,
    thumbs: Object.fromEntries(state.thumbs),
    posters: Object.fromEntries(state.videoPosters),
    durations: Object.fromEntries(state.durations),
    doc_thumbs: Object.fromEntries(documentThumbs),
    photos: Object.fromEntries(
      [...state.photos].filter(([, url]) => typeof url === "string" && url.startsWith("http")),
    ),
    contacts: Object.fromEntries(state.contacts),
    phones: Object.fromEntries(state.phones),
    sender_names: activeSenderNames(),
    me: state.me || "",
    me_jid: state.meJid || "",
    me_lid: state.meLid || "",
    self_handle: (state.chats.get(state.meJid) || state.chats.get(state.meLid) || {}).name || "",
    known_phone:
      knownPhone && knownPhone !== "loading" && knownPhone !== "missing" ? knownPhone : "",
    show_deleted: Boolean(state.showDeleted),
    selecting: Boolean(state.selecting),
    selected: [...(state.selected || [])],
    starred: [...state.starred],
    now: Date.now(),
  };
}

async function renderChat() {
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
  const cachedPhone = state.phones.get(chat.jid);
  const knownPhone =
    cachedPhone && cachedPhone !== "loading" && cachedPhone !== "missing" ? cachedPhone : "";
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
        // The header subtitle comes from the renderer: repaint so the resolved
        // number appears.
        scheduleChatRender();
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
  const anchor = sameChat ? await topVisibleMessage(box) : null;
  box.classList.toggle("channel-view", channel);
  // Group chats reserve a left gutter for the sender's profile photo.
  box.classList.toggle("group-view", chat.isGroup && !channel);

  const seq = ++chatRenderSeq;
  let out;
  try {
    out = await invoke("ui_chat", { request: chatRequest(chat) });
  } catch (err) {
    console.warn("chat render failed", err);
    return;
  }
  // A newer repaint (or a chat switch) owns the box now.
  if (seq !== chatRenderSeq || state.active !== chat.jid) return;

  if (out.empty_html) {
    $("chat-name").textContent = out.name;
    $("chat-sub").textContent = out.subtitle;
    box.innerHTML = out.empty_html;
    box.dataset.stickBottom = "1";
    lastMessageScrollTop = 0;
    lastRenderedChat = chat.jid;
    updateScrollLatestButton();
    return;
  }

  $("chat-name").textContent = out.name;
  $("chat-sub").textContent = out.subtitle;
  box.innerHTML = out.html;
  hydrateMedia(box);
  hydrateDocumentCards(box);
  hydratePreviews(box);
  hydrateLinkThumbs(box);
  hydratePhones(box);
  watchMessageMedia();
  queuePhotos(out.queue || []);
  // A chat switch (or a reply while already at the bottom) should land on the
  // newest message. While reading older messages in the same chat, anchor the
  // top visible bubble so new messages/media do not move the viewport.
  const stick = !sameChat || wasAtBottom;
  box.dataset.stickBottom = stick ? "1" : "0";
  if (stick) {
    scrollToLatest();
  } else if (anchor) {
    const node = box.querySelector(`[data-id="${CSS.escape(anchor.id)}"]`);
    box.scrollTop = node
      ? await invoke("ui_scroll_anchor", {
          nodeOffset: node.offsetTop,
          anchorOffset: anchor.offset,
          fallback: previousScrollTop,
        }).catch(() => Math.max(0, node.offsetTop - anchor.offset))
      : previousScrollTop;
  } else {
    box.scrollTop = previousScrollTop;
  }
  updateScrollLatestButton();
  lastMessageScrollTop = box.scrollTop;
  lastRenderedChat = chat.jid;
  if (!$("chat-search-bar").hidden) runChatSearch($("chat-search-input").value, true);
  scheduleAiContext();
}

document.addEventListener("click", (event) => {
  const more = event.target.closest && event.target.closest(".channel-more");
  if (!more) return;
  const bubble = more.closest(".bubble");
  const body = bubble && bubble.querySelector(".body.clamp");
  if (body) body.classList.remove("clamp");
  more.remove();
});

async function applyReaction(message, emoji) {
  if (!message || !state.active) return;
  const chat = state.active;
  const chatInfo = state.chats.get(chat);
  const rows = message.reactions || [];
  // Same emoji removes it, a different one replaces it (revoking first), and
  // nothing yet is a plain add: wa_ui::reactions::toggle.
  const plan = await invoke("ui_reaction_toggle", {
    mine: rows.find((reaction) => reaction.by === "me") || null,
    tapped: emoji,
  }).catch(() => null);
  if (!plan) return;

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
    if (plan.revoke_first) {
      // Changing a reaction: revoke the old one first, then send the new emoji.
      // A plain replacement is not applied consistently by the server.
      await send("");
      await send(plan.value);
    } else {
      await send(plan.value);
    }
    message.reactions = await invoke("ui_reaction_after_send", {
      rows,
      value: plan.value,
      now: Date.now(),
    }).catch(() => rows);
    renderChat();
    scheduleSave();
  } catch (err) {
    toast(String(err));
  }
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
    const paint = async (phone) => {
      const text = await invoke("ui_format_phone", { phone });
      node.innerHTML = isName
        ? escapeHtml(text)
        : `${await invoke("ui_icon", { name: "call", class: "phone-glyph" })} ${text}`;
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

