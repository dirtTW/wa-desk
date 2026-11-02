/* WA Desk frontend: state-updates. Loaded in order by index.html; all files share the classic-script global scope. */

/* ------------------------------------------------------------------ */
/* state updates                                                       */
/* ------------------------------------------------------------------ */

function isPlaceholderName(jid, name) {
  return name === jid.split("@")[0];
}

function contactNameFor(jid) {
  if (!jid) return "";
  return state.contacts.get(WAChatUtils.phoneFromJid(jid)) || "";
}

// Saved-contact names arrive from the phone's address book (app-state sync)
// after the chats were already drawn, so patch chats, senders and the list.
function applyContactName(jid, name) {
  const key = WAChatUtils.phoneFromJid(jid);
  if (!key) return;
  if (name) state.contacts.set(key, name);
  else state.contacts.delete(key);

  let changed = false;
  for (const chat of state.chats.values()) {
    if (chat.jid === state.meJid || chat.jid === state.meLid) continue;
    if (WAChatUtils.phoneFromJid(chat.jid) === key && name) {
      chat.name = name;
      changed = true;
    }
    for (const message of chat.messages) {
      if (message.sender && WAChatUtils.phoneFromJid(message.sender) === key && name) {
        message.sender_name = name;
        changed = true;
      }
    }
  }
  if (changed) scheduleContactRender();
}

// The address book arrives as hundreds of individual events; repaint once.
let contactRenderTimer = null;

function scheduleContactRender() {
  if (contactRenderTimer) return;
  contactRenderTimer = setTimeout(() => {
    contactRenderTimer = null;
    renderChatList();
    if (state.active) renderChat();
    if (!$("new-chat-screen").hidden) renderNewChat();
  }, 150);
}

async function loadContacts() {
  try {
    const map = await invoke("get_contacts");
    for (const [jid, name] of Object.entries(map || {})) applyContactName(jid, name);
  } catch (err) {
    console.warn("contacts unavailable", err);
  }
}

let contactsSyncing = false;

// Ask the phone for its full address book once per connection. WhatsApp only
// pushes contact deltas to linked devices after pairing; this fills the list.
async function syncContacts() {
  if (contactsSyncing || state.contacts.size > 0) return;
  contactsSyncing = true;
  try {
    await invoke("sync_contacts");
    await loadContacts();
  } catch (err) {
    console.warn("contact sync failed", err);
  } finally {
    contactsSyncing = false;
  }
}

function upsertChat({ chat, name, isGroup }) {
  let entry = state.chats.get(chat);
  if (!entry) {
    entry = {
      jid: chat,
      name: name || chat.replace(/@.*/, ""),
      isGroup: Boolean(isGroup),
      messages: [],
      last: null,
      lastTs: 0,
      unread: 0,
    };
    state.chats.set(chat, entry);
    return entry;
  }

  // A bare phone number is only a placeholder until a real name shows up.
  if (name && !isPlaceholderName(chat, name)) entry.name = name;
  if (isGroup !== undefined) entry.isGroup = Boolean(isGroup);
  return entry;
}

function addIncoming(message, history = false) {
  const channel = isChannelJid(message.chat);
  const entry = upsertChat({
    chat: message.chat,
    // A channel's posts are authored by the channel itself; never let a post's
    // sender fallback rename the channel ("Channel" instead of the real name).
    name: channel
      ? message.chat_name
      : message.is_group
        ? message.chat_name
        : contactNameFor(message.chat) || message.sender_name,
    isGroup: message.is_group,
  });

  const existing = entry.messages.find((m) => m.id === message.id);
  // A status reply/reaction used to recover the quoted status as a chat bubble
  // (reply-quote recovery did not know a status is not a chat message). Drop
  // that phantom copy when the real interaction arrives, like the official
  // client which only shows the status inside the "· Status" card.
  if (message.status_quote && message.reply_to?.id) {
    const phantom = entry.messages.findIndex(
      (m) => m.id === message.reply_to.id && m.once_recovered,
    );
    if (phantom >= 0) {
      entry.messages.splice(phantom, 1);
      entry.last = entry.messages.length ? entry.messages[entry.messages.length - 1] : null;
      entry.lastTs = entry.last ? entry.last.timestamp : entry.lastTs;
      scheduleSave();
      if (state.active === entry.jid) scheduleChatRender();
    }
  }
  if (existing) {
    // A key-only history stub can be followed by the body-carrying copy of the
    // same id (the phone answers the placeholder resend with the full node).
    // Upgrade it in place instead of dropping it, otherwise chats whose first
    // sync only delivered keys (e.g. disappearing-message groups) stay blank.
    // A `view_once` placeholder upgrades too when the wire later carries real
    // inline content (multi-device lag) — the copy then renders like a normal
    // kept media bubble on our client.
    const isStub =
      !String(existing.text || "").trim() &&
      (!existing.kind || existing.kind === "text" || existing.kind === "unsupported" || existing.kind === "view_once");
    const hasContent =
      Boolean(String(message.text || "").trim()) ||
      (message.kind && message.kind !== "text" && message.kind !== "unsupported");
    if (isStub && hasContent) {
      Object.assign(existing, message, { pending: false });
      entry.messages.sort((a, b) => a.timestamp - b.timestamp);
      if (!entry.last || message.timestamp >= entry.last.timestamp) {
        entry.last = existing;
        entry.lastTs = existing.timestamp;
      }
      scheduleSave();
      renderChatList();
      if (state.active === entry.jid) renderChat();
    }
    // Channel counters (views/forwards/reactions) only arrive with the history
    // page while the post itself is usually already cached, so fold them in.
    if (message.stats && !existing.stats) {
      existing.stats = message.stats;
      scheduleSave();
      if (state.active === entry.jid) renderChat();
    }
    // A cached copy stored before the forwarded marker existed learns it when
    // the phone replays the same message.
    if (message.forwarded && !existing.forwarded) {
      existing.forwarded = true;
      scheduleSave();
      if (state.active === entry.jid) scheduleChatRender();
    }
    // A cached link preview stored before the CDN thumbnail was decoded gets
    // the fresh one (big channel images ride in `link.thumb_path`).
    if (
      message.link &&
      (!existing.link ||
        (!existing.link.thumb &&
          !existing.link.thumb_path &&
          (message.link.thumb || message.link.thumb_path)))
    ) {
      existing.link = message.link;
      scheduleSave();
      if (state.active === entry.jid) scheduleChatRender();
    }
    return;
  }

  // Our own message echoed back: fold it into the optimistic bubble.
  if (!history && message.from_me) {
    const pending = [...entry.messages]
      .reverse()
      .find(
        (m) =>
          m.pending &&
          m.kind === message.kind &&
          (message.kind !== "text" || m.text === message.text) &&
          message.timestamp - m.timestamp < 120_000,
      );
    if (pending) {
      if (state.media.has(pending.id)) {
        const media = state.media.get(pending.id);
        state.media.delete(pending.id);
        putMedia(message.id, media);
      }
      pending.id = message.id;
      pending.pending = false;
      pending.timestamp = message.timestamp;
      entry.last = pending;
      renderChatList();
      if (state.active === entry.jid) renderChat();
      return;
    }
  }

  if (!message.is_group && message.sender_name && isPlaceholderName(entry.jid, entry.name)) {
    entry.name = message.sender_name;
  }

  entry.messages.push({ ...message, pending: false });
  entry.messages.sort((a, b) => a.timestamp - b.timestamp);
  if (message.kind === "sticker") {
    markStickersDirty();
    syncAiStickers();
    // Stickers are small; fetch them eagerly so the picker and the chat render
    // without a second round-trip later.
    if (!state.media.has(message.id)) {
      invoke("fetch_media", { id: message.id })
        .then((path) => {
          rememberMedia(message.id, convertFileSrc ? convertFileSrc(path) : path);
        })
        .catch(() => {});
    }
  }
  if (entry.messages.length > MAX_MESSAGES_IN_MEMORY) {
    entry.messages.splice(0, entry.messages.length - MAX_MESSAGES_IN_MEMORY);
  }

  // Keep the whole session inside a message budget; bursts of history replay
  // would otherwise grow without bound.
  if (history) {
    if (state.chats.size > 0 && (historyBudget++ & 0x3f) === 0) {
      WAChatUtils.trimMessages(state.chats, {
        perChat: MAX_MESSAGES_IN_MEMORY,
        budget: MAX_MESSAGES_TOTAL,
        keepJid: state.active,
      });
    }
  }

  // History chunks arrive in arbitrary order, so only advance the preview.
  if (!entry.last || message.timestamp >= entry.last.timestamp) {
    entry.last = message;
    entry.lastTs = message.timestamp;
  }

  if (history) {
    scheduleHistoryRender();
    return;
  }

  if (state.active === entry.jid) {
    const wasAway = distanceFromBottom($("messages")) > SCROLL_BOTTOM_THRESHOLD;
    entry.unread = 0;
    renderChat();
    if (wasAway && !message.from_me) setScrollLatestUnread(true);
    else updateScrollLatestButton();
    markRead(entry.jid);
  } else if (message.from_me) {
    // A message we sent from the phone (or another device) is not unread news:
    // no badge bump, no notification, no "new message" affordance.
    entry.unread = 0;
  } else {
    entry.unread += 1;
    // The toast itself fires from the Rust pump; this keeps Rust's mute set
    // fresh (mute expiries) at most once per burst.
    scheduleNotifySync();
  }
  renderChatList();
  maybeUnlock();
}

/* ------------------------------------------------- chat export import */

// WhatsApp's "Export chat" text format:
//   [10/30/26, 12:15:40 PM] You: message
//   [10/30/26, 2:13:18 PM] Alice: reply
// Continuation lines belong to the message above them.
const EXPORT_LINE =
  /^\[(\d{1,2})\/(\d{1,2})\/(\d{2,4}),\s+(\d{1,2}):(\d{2})(?::(\d{2}))?\s*(AM|PM|am|pm)?\]\s*(.*)$/;

function parseChatExport(text, allowNoSender = false) {
  const out = [];
  let current = null;
  for (const raw of String(text || "").split(/\r?\n/)) {
    const line = raw.replace(/[\u200e\u200f\u202a-\u202e]/g, "");
    const match = line.match(EXPORT_LINE);
    if (!match) {
      if (current && line.trim()) current.text += `\n${line.trim()}`;
      continue;
    }
    const [, mo, da, yr, hh, mm, ss, ampm, rest] = match;
    const year = yr.length === 2 ? 2000 + Number(yr) : Number(yr);
    let hour = Number(hh);
    if (ampm) {
      const pm = ampm.toLowerCase() === "pm";
      if (pm && hour < 12) hour += 12;
      if (!pm && hour === 12) hour = 0;
    }
    const timestamp = new Date(
      year,
      Number(mo) - 1,
      Number(da),
      hour,
      Number(mm),
      Number(ss || 0),
    ).getTime();
    if (!Number.isFinite(timestamp)) {
      current = null;
      continue;
    }
    // System entries ("- Alice left") and the empty separator lines are not
    // messages; they only reset continuation.
    const body = rest.trim();
    if (!body || body.startsWith("- ")) {
      current = null;
      continue;
    }
    // A channel export has no sender column — the whole body is the post text,
    // even when it contains a colon of its own.
    if (allowNoSender) {
      current = { sender: "Channel", text: body, timestamp };
      out.push(current);
      continue;
    }
    const sep = body.indexOf(": ");
    if (sep < 0) {
      current = null;
      continue;
    }
    const sender = body.slice(0, sep).trim();
    const messageText = body.slice(sep + 2).trim();
    if (!sender || !messageText || /^Waiting for this message/i.test(messageText)) {
      current = null;
      continue;
    }
    current = { sender, text: messageText, timestamp };
    out.push(current);
  }
  return out;
}

async function importChatExport(file) {
  const chat = state.chats.get(state.active);
  if (!file || !chat) return;
  let text = "";
  try {
    text = await file.text();
  } catch (err) {
    toast(String(err));
    return;
  }
  const parsed = parseChatExport(text, isChannelJid(chat.jid));
  if (!parsed.length) {
    toast("No messages found in that export.");
    return;
  }
  const normal = (value) => String(value || "").replace(/\s+/g, " ").trim();
  // The phone may already have part of the export (the overlap around the
  // link time), so match on time + text rather than generating duplicate ids.
  // A wider window absorbs the phone's second-level rounding.
  const same = (left, right) =>
    Math.abs(left.ts - right.ts) < 120_000 && left.text === right.text;
  chat.messages = chat.messages.filter(
    (message) =>
      !String(message.id).startsWith("export-") ||
      !chat.messages.some(
        (other) => other !== message && !String(other.id).startsWith("export-") &&
          same({ ts: message.timestamp, text: normal(message.text) },
            { ts: other.timestamp, text: normal(other.text) }),
      ),
  );
  const known = chat.messages.map((message) => ({
    ts: message.timestamp,
    text: normal(message.text),
  }));
  let added = 0;
  for (const item of parsed) {
    const value = normal(item.text);
    if (known.some((entry) => same(entry, { ts: item.timestamp, text: value }))) {
      continue;
    }
    const fromMe = /^(you|saya)$/i.test(item.sender);
    addIncoming(
      {
        chat: chat.jid,
        chat_name: chat.name,
        is_group: Boolean(chat.isGroup),
        // Channel posts have no per-post author.
        sender: fromMe || isChannelJid(chat.jid) ? null : item.sender,
        sender_name: fromMe ? null : isChannelJid(chat.jid) ? chat.name : item.sender,
        sender_phone: null,
        id: `export-${item.timestamp}-${added}`,
        from_me: fromMe,
        timestamp: item.timestamp,
        kind: "text",
        text: item.text,
        mentions: [],
        reply_to: null,
        link: null,
      },
      true,
    );
    known.push({ ts: item.timestamp, text: value });
    added += 1;
  }
  if (added) {
    chat.messages.sort((a, b) => a.timestamp - b.timestamp);
    chat.last = chat.messages[chat.messages.length - 1];
    chat.lastTs = chat.last?.timestamp || chat.lastTs;
    scheduleSave();
    renderChatList();
    if (state.active === chat.jid) renderChat();
    toast(`Imported ${added} messages from the export`);
  } else {
    toast("That export is already imported.");
  }
}

/* ------------------------------------------------- notifications + autostart */

// Windows push notifications are owned by the Rust event pump now: it fires
// them even when this webview is hidden, reloading or throttled. The UI only
// syncs the parts Rust cannot see — the notifications preference, the open
// conversation and the currently muted chats.
let notifySyncTimer = null;

function scheduleNotifySync(delay = 400) {
  clearTimeout(notifySyncTimer);
  notifySyncTimer = setTimeout(syncNotifyState, delay);
}

async function syncNotifyState() {
  try {
    const muted = [];
    for (const [, chat] of state.chats) {
      if (WAChatUtils.chatIsMuted(chat)) muted.push(chat.jid);
    }
    await invokeWithTimeout(
      "set_notify_state",
      {
        prefs: {
          enabled: Boolean(settingsPrefs.notifications),
          active: state.active || null,
          muted,
        },
      },
      5000,
    );
  } catch {
    /* best-effort: the Rust side keeps its last known state */
  }
}

async function setNotificationsEnabled(enabled) {
  const api = window.__TAURI__?.notification;
  if (!api || !enabled) return;
  try {
    if (!(await api.isPermissionGranted())) await api.requestPermission();
  } catch {
    /* user declined */
  }
}

async function setAutostartEnabled(enabled) {
  const api = window.__TAURI__?.autostart;
  if (!api) return;
  try {
    if (enabled) await api.enable();
    else await api.disable();
  } catch (err) {
    toast(String(err));
  }
}

async function syncAutostart() {
  const api = window.__TAURI__?.autostart;
  if (!api) return;
  try {
    settingsPrefs.autostart = await api.isEnabled();
    saveSettingsPrefs();
  } catch {
    /* plugin unavailable */
  }
}

function pushAiReply(chat, text, id) {
  const entry = state.chats.get(chat);
  if (!entry || !text) return;
  const now = Date.now();
  // Some accounts also echo the reply back; don't show it twice.
  if (
    entry.messages.some(
      (m) => m.from_me && m.text === text && Math.abs(m.timestamp - now) < 3000,
    )
  ) {
    return;
  }
  const message = {
    // The server's message id when the backend sends it along, so a later
    // revoke (someone deletes the AI reply for everyone) finds this bubble;
    // a synthetic `ai-…` key would never match the REVOKE's id.
    id: id || `ai-${now}-${++state.localSeq}`,
    chat,
    sender: null,
    sender_name: null,
    from_me: true,
    is_group: entry.isGroup,
    timestamp: now,
    kind: "text",
    text,
    pending: false,
  };
  entry.messages.push(message);
  entry.messages.sort((a, b) => a.timestamp - b.timestamp);
  entry.last = message;
  entry.lastTs = now;
  if (entry.messages.length > MAX_MESSAGES_IN_MEMORY) {
    entry.messages.splice(0, entry.messages.length - MAX_MESSAGES_IN_MEMORY);
  }
  renderChatList();
  if (state.active === chat) renderChat();
  scheduleSave();
}

function addSent({ chat, id, timestamp }) {
  const entry = state.chats.get(chat);
  if (!entry) return;

  const pending = [...entry.messages].reverse().find((m) => m.pending);
  if (pending) {
    pending.id = id;
    pending.timestamp = timestamp || pending.timestamp;
    pending.pending = false;
    // The primary phone owns a "Message yourself" chat, so everything sent
    // there is delivered (and read) the moment it lands — the official
    // client shows the grey double tick immediately.
    if (isSelfJid(chat) || isSelfJid(entry.jid)) pending.status = "delivered";
    entry.last = pending;
  }

  renderChatList();
  if (state.active === chat) renderChat();
}

// `true` for the account's own number/LID, so "Message yourself" surfaces
// without re-parsing JIDs everywhere.
function isSelfJid(jid) {
  return (
    state.meJid && String(jid).split("@")[0] === String(state.meJid).split("@")[0]
  ) || (state.meLid && String(jid).split("@")[0] === String(state.meLid).split("@")[0]);
}

function maybeUnlock() {
  if (!state.ready) return;
  state.linked = true;
  $("login").hidden = true;
  $("btn-logout").hidden = false;
}

