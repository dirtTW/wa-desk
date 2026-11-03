/* WA Desk frontend: actions. Loaded in order by index.html; all files share the classic-script global scope. */

/* ------------------------------------------------------------------ */
/* actions                                                             */
/* ------------------------------------------------------------------ */

async function markRead(chat) {
  const entry = state.chats.get(chat);
  if (entry) entry.unread = 0;

  // Tell the sender their messages were read (blue ticks), not just synced
  // the unread state to our other devices.
  const ids = entry
    ? entry.messages
        .filter((message) => !message.from_me && !message.read)
        .slice(-200)
        .map((message) => message.id)
    : [];
  if (entry) {
    for (const message of entry.messages) {
      if (!message.from_me) message.read = true;
    }
  }

  try {
    await invoke("mark_read", { chat });
    if (ids.length > 0) await invoke("mark_read_messages", { chat, ids });
  } catch {
    /* read receipts are best effort */
  }
}

const historyRequests = new Map();

let historyGate = Promise.resolve();
let historyGateAt = 0;

// Every history request costs the phone a PDO message. Serialise them and
// leave a quiet gap, so rapid chat switching cannot look like a burst.
function queueHistoryRequest(task) {
  const run = historyGate.then(async () => {
    const wait = Math.max(0, 2000 - (Date.now() - historyGateAt));
    if (wait) await new Promise((resolve) => setTimeout(resolve, wait));
    historyGateAt = Date.now();
    return task();
  });
  historyGate = run.catch(() => {});
  return run;
}

async function requestChatHistory(chat, force = false) {
  if (!state.connected || !chat) return;
  // Channel post history is not served by on-demand requests; the posts arrive
  // live and from the normal history sync instead.
  if (isChannelJid(chat.jid)) return;
  const last = historyRequests.get(chat.jid) || 0;
  if (!force && Date.now() - last < 2 * 60 * 1000) return;
  // WhatsApp Web anchors the on-demand request on the OLDEST message it has
  // stored, so every request pages further back (wa_ui::history::anchor_request).
  // A chat that history sync only announced has no anchor, and the phone answers
  // with that chat's latest window.
  const anchor = await historyAnchorRequest(chat);
  historyRequests.set(chat.jid, Date.now());
  try {
    await queueHistoryRequest(() =>
      invoke("fetch_chat_history", {
        chat: chat.jid,
        oldestId: anchor.oldestId,
        oldestFromMe: anchor.oldestFromMe,
        oldestTimestamp: anchor.oldestTimestamp,
        count: 50,
      }),
    );
  } catch (err) {
    historyRequests.delete(chat.jid);
    toast(String(err));
  }
}

// Scrolling to the top asks the phone for the next older window. Guarded so a
// held scroll position cannot queue a request every frame. Channels page
// through their own `server_id` cursor instead.
const olderPageLocks = new Set();

/* ------------------------------------------------------- channel history */

// Channel posts come from the newsletter self-IQ (`<iq to=s.whatsapp.net
// xmlns=newsletter><messages type=jid jid=…>`); the reply carries the `server_id`
// cursor for the next, older page.
const channelCursors = new Map();
const channelHistoryLocks = new Set();
const channelHistoryFailed = new Set();

async function requestChannelHistory(chat, more = false) {
  if (!state.connected || !chat || !isChannelJid(chat.jid)) return;
  if (channelHistoryLocks.has(chat.jid) || channelHistoryFailed.has(chat.jid)) return;
  const before = more ? channelCursors.get(chat.jid) : null;
  if (more && !before) return;
  channelHistoryLocks.add(chat.jid);
  try {
    const cursor = await invokeWithTimeout(
      "fetch_channel_history",
      {
        chat: chat.jid,
        name: chat.name || "",
        count: 24,
        before: before ?? null,
      },
      15_000,
    );
    if (typeof cursor === "number") channelCursors.set(chat.jid, cursor);
    else if (!more) channelCursors.delete(chat.jid);
  } catch (err) {
    channelHistoryFailed.add(chat.jid);
    console.warn("channel history unavailable", err);
  } finally {
    channelHistoryLocks.delete(chat.jid);
  }
}

function maybePageOlderMessages() {
  const box = $("messages");
  if (!state.active || box.scrollTop > 8) return;
  const chat = state.chats.get(state.active);
  if (!chat || olderPageLocks.has(chat.jid)) return;
  if (isChannelJid(chat.jid)) {
    if (!channelCursors.has(chat.jid)) return;
    olderPageLocks.add(chat.jid);
    requestChannelHistory(chat, true).finally(() => {
      setTimeout(() => olderPageLocks.delete(chat.jid), 2000);
    });
    return;
  }
  olderPageLocks.add(chat.jid);
  requestChatHistory(chat, true).finally(() => {
    setTimeout(() => olderPageLocks.delete(chat.jid), 2500);
  });
}

/* ------------------------------------------------------ history re-sync */

// WhatsApp only pushes history to a linked device when it pairs. Everything
// afterwards arrives live, so a fresh window (new install, cleared storage, or
// a chat that was announced without messages) has to ask the phone again.
// The supported primitive is the per-chat on-demand request, so a re-sync is a
// paced batch of those, never a burst of hundreds at once.
const HISTORY_SYNC_KEY = "wadesk.history.sync";
const HISTORY_SYNC_WINDOW_MS = 24 * 60 * 60 * 1000;
// Deliberately small: every entry is a peer message to the phone. A large
// burst is indistinguishable from spam, so a re-sync asks for a handful and
// the rest load when the user opens them.
const HISTORY_SYNC_BATCH = 8;

function historySyncMeta() {
  try {
    return JSON.parse(localStorage.getItem(HISTORY_SYNC_KEY) || "null");
  } catch {
    return null;
  }
}

function markHistorySynced() {
  try {
    localStorage.setItem(HISTORY_SYNC_KEY, JSON.stringify({ at: Date.now() }));
  } catch {
    /* storage full: the next open simply asks again */
  }
}

let historySyncing = false;

// The re-sync is deliberately slow (3 s between chats), so show how far it got
// instead of leaving the sidebar looking stuck.
let syncProgressHideTimer = null;

function showSyncProgress(done, total) {
  const box = $("sync-progress");
  if (!box) return;
  clearTimeout(syncProgressHideTimer);
  box.hidden = false;
  const pct = total > 0 ? Math.min(100, Math.round((done / total) * 100)) : 0;
  $("sync-progress-fill").style.width = `${pct}%`;
  $("sync-progress-text").textContent =
    total > 0 && done >= total ? "History refreshed" : "Syncing chat history…";
  $("sync-progress-count").textContent = total > 0 ? `${done}/${total}` : "";
}

function hideSyncProgress(delay = 1200) {
  clearTimeout(syncProgressHideTimer);
  syncProgressHideTimer = setTimeout(() => {
    const box = $("sync-progress");
    if (box) box.hidden = true;
  }, delay);
}

// Same anchor rule as requestChatHistory (wa_ui::history): page behind the
// OLDEST locally stored real message, or no anchor for a chat with none.
async function historyAnchorRequest(chat) {
  const anchor = await invoke("ui_history_anchor", {
    chat: {
      jid: chat.jid,
      deleted: Boolean(chat.deleted),
      is_channel: isChannelJid(chat.jid),
      message_count: (chat.messages || []).length,
      last_ts: chat.last?.timestamp || chat.lastTs || 0,
      messages: (chat.messages || []).map((message) => ({
        id: String(message.id || ""),
        timestamp: message.timestamp || 0,
        from_me: Boolean(message.from_me),
        deleted: Boolean(message.deleted),
      })),
    },
  }).catch(() => null);
  if (!anchor) {
    return { chat: chat.jid, oldestId: "", oldestFromMe: false, oldestTimestamp: 0, count: 50 };
  }
  return {
    chat: anchor.chat,
    oldestId: anchor.oldest_id,
    oldestFromMe: anchor.oldest_from_me,
    oldestTimestamp: anchor.oldest_timestamp,
    count: anchor.count,
  };
}

// Chats that never produced a bubble go first; then the most recently active
// (ordered by wa_ui::history::sync_targets).
async function chatsForHistorySync(limit = HISTORY_SYNC_BATCH) {
  const jids = await invoke("ui_history_targets", {
    chats: [...state.chats.values()].map((chat) => ({
      jid: chat.jid,
      deleted: Boolean(chat.deleted),
      is_channel: isChannelJid(chat.jid),
      message_count: (chat.messages || []).length,
      last_ts: chat.last?.timestamp || chat.lastTs || 0,
    })),
    limit,
  }).catch(() => []);
  return jids.map((jid) => state.chats.get(jid)).filter(Boolean);
}

async function syncChatHistoryBatch(chats, force = false) {
  if (historySyncing || !state.connected) return 0;
  const meta = historySyncMeta();
  if (!force && meta && Date.now() - meta.at < HISTORY_SYNC_WINDOW_MS) return 0;

  const targets = (chats && chats.length ? chats : await chatsForHistorySync())
    .filter((chat) => chat && !chat.deleted)
    .slice(0, HISTORY_SYNC_BATCH);
  if (!targets.length) return 0;

  historySyncing = true;
  showSyncProgress(0, targets.length);
  try {
    const requests = await Promise.all(targets.map(historyAnchorRequest));
    const sent = await queueHistoryRequest(() =>
      invoke("sync_chat_history", { requests }),
    );
    if (sent > 0) {
      markHistorySynced();
      toast(`Refreshing ${sent} chat${sent === 1 ? "" : "s"} from your phone…`);
    }
    return sent;
  } catch (err) {
    toast(String(err));
    return 0;
  } finally {
    historySyncing = false;
    hideSyncProgress();
  }
}

// A warm cache needs no phone round-trip. Only a window with no messages at
// all (fresh install / cleared storage) triggers the automatic re-sync, and
// then only a handful of chats; the rest load when opened.
async function maybeSyncChatHistory() {
  if (!state.connected) return;
  const withMessages = [...state.chats.values()].filter(
    (chat) => chat.messages.length > 0,
  ).length;
  if (withMessages > 0) return;
  await seedChatsFromContacts();
  if (!state.chats.size) return;
  renderChatList();
  setTimeout(async () => syncChatHistoryBatch(await chatsForHistorySync(), false), 5000);
}

/* --------------------------------------------------------- disk chat cache */

// The webview cache is convenient but not durable (it can be cleared, and a
// quota error silently drops it). Keep the chat list metadata next to the
// contact store as well so a restart still has a sidebar to sync from.
let chatCacheWriting = false;

async function chatCachePayload() {
  return invoke("ui_disk_cache_payload", {
    chats: [...state.chats.values()].map((chat) => ({
      jid: chat.jid,
      name: chat.name || "",
      is_group: Boolean(chat.isGroup),
      last_ts: chat.last?.timestamp || chat.lastTs || 0,
      unread: chat.unread || 0,
      last_text: chat.last?.text || "",
    })),
    limit: 400,
  }).catch(() => "");
}

async function persistChatCache() {
  if (chatCacheWriting) return;
  chatCacheWriting = true;
  try {
    const json = await chatCachePayload();
    if (!json) return;
    await invoke("save_chat_cache", { json });
  } catch (err) {
    console.warn("chat cache unavailable", err);
  } finally {
    chatCacheWriting = false;
  }
}

async function restoreDiskChatCache() {
  if (state.chats.size > 0) return;
  let raw;
  try {
    raw = await invoke("load_chat_cache");
  } catch {
    return;
  }
  if (!raw || state.chats.size > 0) return;

  // Row shaping (broadcast filtering, fallback names, preview synthesis) is
  // wa_ui::cache::restore_disk.
  const rows = await invoke("ui_restore_disk_cache", { raw }).catch(() => []);
  for (const chat of rows) {
    if (!chat || !chat.jid || state.chats.has(chat.jid)) continue;
    state.chats.set(chat.jid, {
      jid: chat.jid,
      name: chat.name,
      isGroup: Boolean(chat.is_group),
      messages: [],
      last: chat.last_text
        ? { text: chat.last_text, timestamp: chat.last_ts || 0 }
        : null,
      lastTs: chat.last_ts || 0,
      unread: chat.unread || 0,
    });
  }

  mergeDuplicateChats();
  renderChatList();
  if (state.active) renderChat();
  maybeSyncChatHistory();
}

// With no cache at all, the address book is enough to rebuild personal chats;
// history requests then refill them (and business chats return as the phone
// answers with their canonical ids).
async function seedChatsFromContacts() {
  if (state.chats.size > 0 || state.contacts.size === 0) return 0;
  const me = await invoke("ui_phone_from_jid", { jid: state.meJid || "" }).catch(() => "");
  // Which address-book entries become chats is decided by wa_ui::state.
  const seeded = await invoke("ui_seed_chats", {
    contacts: [...state.contacts].map(([phone, name]) => ({ phone, name })),
    mePhone: me,
    existing: [...state.chats.keys()],
  }).catch(() => []);
  for (const chat of seeded) {
    state.chats.set(chat.jid, {
      jid: chat.jid,
      name: chat.name,
      isGroup: false,
      messages: [],
      last: null,
      lastTs: 0,
      unread: 0,
    });
  }
  return seeded.length;
}

async function openChat(jid) {
  if (state.active !== jid) {
    stopTyping();
    closeChatSearch();
  }
  clearDraft();
  closeMessageMenu();
  if (state.editing) cancelEdit();
  if (state.reply) cancelReply();
  $("status-screen").hidden = true;
  $("settings-screen").hidden = true;
  $("channels-discover").hidden = true;
  $("status-viewer").hidden = true;
  $("new-chat-screen").hidden = true;
  $("group-info-screen").hidden = true;
  $("group-modal").hidden = true;
  $("channels-screen").hidden = true;
  $("starred-screen").hidden = true;
  state.starredOpen = false;
  if (!$("media-viewer").hidden) closeMediaViewer();
  setRailActive("rail-chats");
  // Give media that failed earlier another chance (e.g. it has since been
  // downloaded or the connection is back).
  for (const [id, entry] of state.media) {
    if (entry.status === "error") state.media.delete(id);
  }
  state.active = jid;
  const entry = state.chats.get(jid);
  if (entry) {
    entry.unread = 0;
    markRead(jid);
  }
  renderChatList();
  renderChat();
  // A LID row may be the same peer as a phone-number chat; resolve first so the
  // history request targets one canonical row instead of spawning a duplicate.
  resolvePeers(entry ? [entry] : [])
    .catch(() => {})
    .finally(() => requestChatHistory(state.chats.get(state.active)));
  $("input").focus();
  updateSendState();
  toggleEmoji(false);
  // The Rust notification pump suppresses toasts for the open chat.
  scheduleNotifySync();
}

async function sendCurrent() {
  const input = $("input");
  const chat = state.active;
  const text = input.value.trim();
  if (!chat) return;

  // The composer-state decision comes from wa_ui::actions::send_plan.
  const plan = await invoke("ui_send_plan", {
    isChannel: isChannelJid(chat),
    hasDraft: !$("draft").hidden,
    editing: Boolean(state.editing),
    text,
  }).catch(() => ({ action: "send" }));
  if (plan.toast) toast(plan.toast);
  if (plan.action === "blocked" || plan.action === "noop") return;
  if (plan.action === "draft") {
    await sendDraft();
    return;
  }

  // Editing an existing message instead of sending a new one.
  if (plan.action === "edit") {
    const { chat, id } = state.editing;
    if (!text) return;
    cancelEdit();
    try {
      await invoke("edit_message", { chat, id, text });
      const entry = state.chats.get(chat);
      const message = entry && entry.messages.find((candidate) => candidate.id === id);
      if (message) {
        message.text = text;
        message.edited = true;
      }
      renderChatList();
      if (state.active === chat) renderChat();
      scheduleSave();
    } catch (err) {
      toast(String(err));
    }
    return;
  }

  if (!text) return;

  const entry = state.chats.get(chat);
  if (!entry) return;

  const quoted = state.reply && state.reply.chat === chat ? state.reply : null;
  if (quoted) cancelReply(false);
  // The optimistic bubble (including the pending quote card) is shaped by
  // wa_ui::actions::outgoing_message.
  const optimistic = await invoke("ui_outgoing_message", {
    input: {
      temp_id: `local-${++state.localSeq}`,
      chat,
      is_group: Boolean(entry.isGroup),
      timestamp: Date.now(),
      text,
      quote: quoted
        ? {
            id: quoted.id || "",
            text: quoted.text || "",
            sender_name: quoted.sender_name || "",
            from_me: Boolean(quoted.from_me),
            kind: quoted.kind || "text",
          }
        : null,
    },
  }).catch(() => null);
  if (!optimistic) return;

  entry.messages.push(optimistic);
  entry.last = optimistic;
  input.value = "";
  autosize(input);
  updateSendState();
  renderChatList();
  renderChat();

  try {
    await invoke("send_text", {
      chat,
      text,
      replyTo: quoted ? quoted.id : null,
      quotedText: quoted ? quoted.text || "" : null,
    });
    stopTyping(chat);

    // Self-sent messages are not echoed back, so ask the AI directly when the
    // user typed /ai in "Message yourself".
    const ai = await invoke("ui_ai_prompt", { text });
    if (ai && (chat === state.meLid || chat === state.meJid)) {
      invoke("ai_reply_now", { chat, text }).catch((err) => toast(String(err)));
    }
  } catch (err) {
    optimistic.pending = false;
    optimistic.failed = true;
    toast(String(err));
    renderChat();
  }
}

function autosize(textarea) {
  textarea.style.height = "auto";
  textarea.style.height = `${Math.min(textarea.scrollHeight, 140)}px`;
}

function startTyping() {
  if (!state.active) return;
  if (state.typingChat !== state.active) {
    if (state.typingChat) {
      invoke("set_typing", { chat: state.typingChat, typing: false }).catch(() => {});
    }
    state.typingChat = state.active;
    invoke("set_typing", { chat: state.active, typing: true }).catch(() => {});
  }
  clearTimeout(state.typingTimer);
  state.typingTimer = setTimeout(stopTyping, 2500);
}

function stopTyping() {
  clearTimeout(state.typingTimer);
  const chat = state.typingChat;
  state.typingChat = null;
  if (!chat) return;
  invoke("set_typing", { chat, typing: false }).catch(() => {});
}

