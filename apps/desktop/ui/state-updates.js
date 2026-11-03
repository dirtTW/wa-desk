/* WA Desk frontend: state-updates. Loaded in order by index.html; all files share the classic-script global scope. */

/* ------------------------------------------------------------------ */
/* state updates                                                       */
/* ------------------------------------------------------------------ */

function contactNameFor(jid) {
  if (!jid) return "";
  return state.contacts.get(jidUser(jid)) || "";
}

// Saved-contact names arrive from the phone's address book (app-state sync)
// after the chats were already drawn, so patch chats, senders and the list.
// Which chats and sender bubbles take a saved name is decided by
// wa_ui::state::contact_rename_plan; this applies the plan.
function contactChatViews() {
  const views = [];
  for (const chat of state.chats.values()) {
    const senders = new Set();
    for (const message of chat.messages) {
      if (message.sender) senders.add(message.sender);
    }
    views.push({
      jid: chat.jid,
      name: chat.name || "",
      is_self: chat.jid === state.meJid || chat.jid === state.meLid,
      senders: [...senders],
    });
  }
  return views;
}

function applyContactPlan(plan) {
  if (!plan || !plan.changed) return false;
  for (const [jid, name] of plan.chat_renames) {
    const chat = state.chats.get(jid);
    if (chat) chat.name = name;
  }
  for (const [jid, sender, name] of plan.sender_renames) {
    const chat = state.chats.get(jid);
    if (!chat) continue;
    for (const message of chat.messages) {
      if (message.sender === sender) message.sender_name = name;
    }
  }
  return true;
}

// The address book replays hundreds of events; collect them and ask Rust for
// one rename plan per burst.
const pendingContactUpdates = new Map();
let contactPlanTimer = null;

function flushContactPlan() {
  contactPlanTimer = null;
  const updates = [...pendingContactUpdates];
  pendingContactUpdates.clear();
  if (!updates.length) return;
  invoke("ui_contact_plan", { updates, chats: contactChatViews() })
    .then((plan) => {
      if (applyContactPlan(plan)) scheduleContactRender();
    })
    .catch(() => {});
}

function applyContactName(jid, name) {
  const key = jidUser(jid);
  if (!key) return;
  if (name) state.contacts.set(key, name);
  else state.contacts.delete(key);
  pendingContactUpdates.set(key, name || "");
  if (!contactPlanTimer) contactPlanTimer = setTimeout(flushContactPlan, 150);
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
    const updates = Object.entries(map || {}).map(([jid, name]) => [jidUser(jid), name]);
    for (const [key, name] of updates) {
      if (!key) continue;
      if (name) state.contacts.set(key, name);
      else state.contacts.delete(key);
    }
    const plan = await invoke("ui_contact_plan", {
      updates,
      chats: contactChatViews(),
    }).catch(() => null);
    if (applyContactPlan(plan)) scheduleContactRender();
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

// Create-or-update the chat row. Which of the two happens, the fallback name
// and whether a placeholder may be renamed are decided by
// wa_ui::state::upsert_plan.
async function upsertChat({ chat, name, isGroup }) {
  const existing = state.chats.get(chat);
  const plan = await invoke("ui_upsert_plan", {
    input: {
      jid: chat,
      name: name || "",
      is_group: isGroup === undefined ? null : Boolean(isGroup),
      exists: Boolean(existing),
      current_name: existing?.name || "",
    },
  }).catch(() => null);
  if (!plan) {
    // Offline fallback: keep the previous behaviour so nothing is lost.
    if (existing) return existing;
    const entry = {
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

  if (plan.create) {
    const entry = {
      jid: chat,
      name: plan.name,
      isGroup: plan.is_group,
      messages: [],
      last: null,
      lastTs: 0,
      unread: 0,
    };
    state.chats.set(chat, entry);
    return entry;
  }
  if (plan.rename) existing.name = plan.name;
  if (plan.set_group) existing.isGroup = plan.is_group;
  return existing;
}

async function addIncoming(message, history = false) {
  const channel = isChannelJid(message.chat);
  const entry = await upsertChat({
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
  // The duplicate/upgrade/echo/unread rules are decided by wa_ui::incoming.
  const incomingView = {
    kind: message.kind || "text",
    text: String(message.text || ""),
    from_me: Boolean(message.from_me),
    timestamp: message.timestamp || 0,
    has_stats: Boolean(message.stats),
    forwarded: Boolean(message.forwarded),
    has_link_thumb: Boolean(message.link && (message.link.thumb || message.link.thumb_path)),
  };
  let echoIndex = null;
  if (!existing && !history && message.from_me) {
    const pendingMessages = entry.messages.filter((m) => m.pending);
    echoIndex = await invoke("ui_pending_echo", {
      candidates: pendingMessages.map((m) => ({
        kind: m.kind || "text",
        text: String(m.text || ""),
        timestamp: m.timestamp || 0,
      })),
      message: incomingView,
    }).catch(() => null);
  }
  const decision = await invoke("ui_incoming_classify", {
    existing: existing
      ? {
          text: String(existing.text || ""),
          kind: existing.kind || "",
          has_stats: Boolean(existing.stats),
          forwarded: Boolean(existing.forwarded),
          has_link: Boolean(existing.link),
          has_link_thumb: Boolean(
            existing.link && (existing.link.thumb || existing.link.thumb_path),
          ),
        }
      : null,
    message: incomingView,
    echo: echoIndex,
  }).catch(() => ({ kind: "append" }));

  if (existing && decision.kind === "duplicate") {
    const plan = decision;
    if (plan.upgrade) {
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
    if (plan.fold_stats) {
      existing.stats = message.stats;
      scheduleSave();
      if (state.active === entry.jid) renderChat();
    }
    // A cached copy stored before the forwarded marker existed learns it when
    // the phone replays the same message.
    if (plan.fold_forwarded) {
      existing.forwarded = true;
      scheduleSave();
      if (state.active === entry.jid) scheduleChatRender();
    }
    // A cached link preview stored before the CDN thumbnail was decoded gets
    // the fresh one (big channel images ride in `link.thumb_path`).
    if (plan.fold_link) {
      existing.link = message.link;
      scheduleSave();
      if (state.active === entry.jid) scheduleChatRender();
    }
    return;
  }

  // Our own message echoed back: fold it into the optimistic bubble.
  if (!existing && decision.kind === "echo_pending") {
    const pendingList = entry.messages.filter((m) => m.pending);
    const pending = pendingList[decision.index];
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

  if (!message.is_group && message.sender_name) {
    invoke("ui_placeholder_name", { jid: entry.jid, name: entry.name })
      .catch(() => false)
      .then((placeholder) => {
        if (!placeholder) return;
        entry.name = message.sender_name;
        scheduleChatListRender();
        scheduleSave();
      });
  }

  // Where the bubble belongs is wa_ui::state::insert_index (stable for equal
  // timestamps, so an echo keeps its arrival order).
  const stored = { ...message, pending: false };
  const index = await invoke("ui_insert_index", {
    timestamps: entry.messages.map((item) => item.timestamp || 0),
    timestamp: stored.timestamp || 0,
  }).catch(() => entry.messages.length);
  entry.messages.splice(index, 0, stored);
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
      // wa-ui::state decides which messages to drop; this applies the plan.
      invoke("ui_trim_messages", {
        chats: [...state.chats.values()].map((chat) => ({
          jid: chat.jid,
          len: (chat.messages || []).length,
          last_ts: chat.last?.timestamp || chat.lastTs || 0,
        })),
        perChat: MAX_MESSAGES_IN_MEMORY,
        budget: MAX_MESSAGES_TOTAL,
        keepJid: state.active || "",
      })
        .then((plan) => {
          for (const op of plan.ops) {
            const chat = state.chats.get(op.jid);
            if (chat && op.drop > 0) chat.messages.splice(0, op.drop);
          }
        })
        .catch(() => {});
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

  const active = state.active === entry.jid;
  const rule = await invoke("ui_unread_rule", {
    active,
    fromMe: Boolean(message.from_me),
  }).catch(() => ({ clear: active, increment: !active }));
  if (rule.clear) entry.unread = 0;
  if (rule.increment) entry.unread += 1;

  if (active) {
    const wasAway = distanceFromBottom($("messages")) > SCROLL_BOTTOM_THRESHOLD;
    renderChat();
    if (wasAway && !message.from_me) setScrollLatestUnread(true);
    else updateScrollLatestButton();
    markRead(entry.jid);
  } else if (rule.increment) {
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
  // Parsing and the duplicate/supersede rules live in wa-ui::export.
  const parsed = await invoke("ui_parse_export", {
    text,
    allowNoSender: isChannelJid(chat.jid),
  });
  if (!parsed.length) {
    toast("No messages found in that export.");
    return;
  }
  const plan = await invoke("ui_import_plan", {
    parsed,
    existing: chat.messages.map((message) => ({
      id: String(message.id || ""),
      timestamp: message.timestamp || 0,
      text: String(message.text || ""),
    })),
  });
  if (plan.remove_export_ids.length) {
    const remove = new Set(plan.remove_export_ids);
    chat.messages = chat.messages.filter((message) => !remove.has(message.id));
  }
  let added = 0;
  for (const item of plan.add) {
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
    // The mute rule lives in wa_ui::menus; this sends the raw mute fields.
    const muted = await invoke("ui_muted_chats", {
      chats: [...state.chats.values()].map((chat) => ({
        jid: chat.jid,
        pinned: Boolean(chat.pinned),
        mute_until: chat.muteUntil || 0,
        mute_forever: Boolean(chat.muteForever),
        now: Date.now(),
        archived: Boolean(chat.archived),
        unread: chat.unread || 0,
        is_group: Boolean(chat.isGroup),
      })),
    }).catch(() => []);
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

async function pushAiReply(chat, text, id) {
  const entry = state.chats.get(chat);
  if (!entry || !text) return;
  const now = Date.now();
  // Some accounts also echo the reply back; don't show it twice
  // (wa_ui::incoming::is_ai_echo).
  const echo = await invoke("ui_ai_echo", {
    messages: entry.messages.map((m) => [Boolean(m.from_me), String(m.text || ""), m.timestamp || 0]),
    text,
    now,
  }).catch(() => false);
  if (echo) return;
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

async function addSent({ chat, id, timestamp }) {
  const entry = state.chats.get(chat);
  if (!entry) return;

  const index = await invoke("ui_newest_pending", {
    flags: entry.messages.map((m) => Boolean(m.pending)),
  }).catch(() => null);
  const pending = index === null || index === undefined ? null : entry.messages[index];
  if (pending) {
    pending.id = id;
    pending.timestamp = timestamp || pending.timestamp;
    pending.pending = false;
    // The primary phone owns a "Message yourself" chat, so everything sent
    // there is delivered the moment it lands (grey double tick).
    const status = await invoke("ui_self_chat_status", {
      isSelfChat: isSelfJid(chat) || isSelfJid(entry.jid),
    }).catch(() => "sent");
    pending.status = status;
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

