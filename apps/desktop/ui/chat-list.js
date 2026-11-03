/* WA Desk frontend: chat-list. Loaded in order by index.html; all files share the classic-script global scope. */

/* ------------------------------------------------------------------ */
/* rendering                                                           */
/* ------------------------------------------------------------------ */

let historyRenderTimer = null;

// History replay can emit thousands of messages in a burst; coalesce renders.
function scheduleHistoryRender() {
  if (historyRenderTimer) return;
  historyRenderTimer = setTimeout(() => {
    historyRenderTimer = null;
    renderChatList();
    if (state.active) renderChat();
  }, 250);
}

let chatListRenderTimer = null;

/// Coalesce bursty sidebar refreshes (photo arrivals, live message bursts) so
/// they cost one rebuild instead of one per event.
function scheduleChatListRender(delay = 40) {
  if (chatListRenderTimer) return;
  chatListRenderTimer = setTimeout(() => {
    chatListRenderTimer = null;
    renderChatList();
  }, delay);
}

/// A profile photo can arrive for a row that is already painted. Repaint only
/// that avatar instead of rebuilding the whole sidebar.
function patchChatAvatar(jid) {
  if (!jid) return;
  const url = state.photos.get(jid);
  if (!url) return;
  // Search results also carry contact rows, which have no chat entry and use a
  // different data attribute; fall back to a normal (coalesced) rebuild.
  if (state.search) {
    scheduleChatListRender();
    return;
  }
  const row = $("chat-list").querySelector(`li[data-jid="${CSS.escape(jid)}"] .avatar`);
  if (!row) {
    scheduleChatListRender();
    return;
  }
  row.classList.remove("group-avatar");
  row.setAttribute("style", "");
  row.innerHTML = `<img class="avatar-img" src="${escapeHtml(url)}" alt="" loading="lazy" onerror="window.__waAvatarError(this)">`;
}

/* --------------------------------------------------- Rust-rendered sidebar */

// The list markup, pin order, community nesting and search sections live in
// `wa-ui` (crates/wa-ui/src/sidebar.rs). The webview only owns the state and
// the DOM: it sends a request and injects the returned HTML.
let sidebarSeq = 0;

/// The fields the sidebar renderer needs from one message; the raw message
/// object is trimmed so a 200 KB thumbnail never crosses the IPC boundary.
function sidebarMessage(message) {
  if (!message) return null;
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
    document: message.document ? { name: message.document.name || "" } : null,
  };
}

/// Push names already seen in the open chat, so a mention of a group member
/// who is not a saved contact still renders with their name.
function activeSenderNames() {
  const names = {};
  const chat = state.chats.get(state.active);
  for (const message of chat?.messages || []) {
    if (!message.sender || !message.sender_name) continue;
    names[WAChatUtils.phoneFromJid(message.sender)] = message.sender_name;
  }
  return names;
}

function sidebarRequest() {
  const chats = [];
  for (const chat of state.chats.values()) {
    const last = WAChatUtils.lastVisibleMessage(chat, state.showDeleted);
    chats.push({
      jid: chat.jid,
      name: chat.name || "",
      is_group: Boolean(chat.isGroup),
      unread: chat.unread || 0,
      pinned: Boolean(chat.pinned),
      pin_ts: chat.pinTs || 0,
      deleted: Boolean(chat.deleted),
      archived: Boolean(chat.archived),
      cleared: Boolean(chat.cleared),
      mute_until: chat.muteUntil || 0,
      mute_forever: Boolean(chat.muteForever),
      last_ts: chat.last?.timestamp || chat.lastTs || 0,
      last: sidebarMessage(last),
    });
  }
  const self = state.chats.get(state.meJid) || state.chats.get(state.meLid);
  return {
    chats,
    contacts: Object.fromEntries(state.contacts),
    photos: Object.fromEntries(
      [...state.photos].filter(([, url]) => typeof url === "string" && url.startsWith("http")),
    ),
    group_meta: state.groupMeta || {},
    phones: Object.fromEntries(state.phones),
    sender_names: activeSenderNames(),
    me: state.me || "",
    me_jid: state.meJid || "",
    me_lid: state.meLid || "",
    self_handle: self?.name || "",
    active: state.active || "",
    search: state.search || "",
    filter: state.filter || "all",
    show_deleted: Boolean(state.showDeleted),
    selecting: Boolean(state.selectingChats),
    selected: [...state.selectedChats],
    archived_open: Boolean(state.archivedOpen),
    max_search_rows: MAX_SEARCH_ROWS,
    now: Date.now(),
  };
}

/// Repaint the sidebar. Async by nature (Rust builds the markup), so a newer
/// request drops a stale response instead of flashing an old list.
function renderChatList() {
  sidebarSeq += 1;
  const seq = sidebarSeq;
  const list = $("chat-list");
  invoke("ui_sidebar", { request: sidebarRequest() })
    .then((out) => {
      if (seq !== sidebarSeq || !list) return;
      list.innerHTML = out.html;
      $("empty-chats").textContent = out.empty_text;
      $("empty-chats").hidden = out.empty_hidden;
      queuePhotos(out.queue || []);
      scheduleSave();
      if (!$("channels-screen").hidden) renderChannels();
      updateRailBadges();
      afterChatListRender();
    })
    .catch((err) => console.warn("sidebar render failed", err));
}

/// Work that must happen once the (async) list is in the DOM.
function afterChatListRender() {}

/* ------------------------------------------------------ starred messages */

// Starred ids come from the `star` app-state collection (a full sync replays
// every star), so this list matches the phone and every linked client.
function starredEntries() {
  const rows = [];
  for (const chat of state.chats.values()) {
    for (const message of chat.messages) {
      if (state.starred.has(message.id)) rows.push({ chat, message });
    }
  }
  rows.sort((a, b) => (b.message.timestamp || 0) - (a.message.timestamp || 0));
  return rows;
}

function openStarred() {
  state.starredOpen = true;
  $("starred-screen").hidden = false;
  renderStarredScreen();
}

function closeStarred() {
  state.starredOpen = false;
  $("starred-screen").hidden = true;
  renderChatList();
}

function renderStarredScreen() {
  const entries = starredEntries().map(({ chat, message }) => ({
    jid: chat.jid,
    message: sidebarMessage(message),
  }));
  invoke("ui_starred", { request: sidebarRequest(), entries })
    .then((out) => {
      if (!state.starredOpen) return;
      $("starred-body").innerHTML = out.html;
    })
    .catch((err) => console.warn("starred render failed", err));
}

// Open the chat that owns a starred message and flash the message itself.
async function openStarredMessage(jid, id) {
  if (!jid || !id) return;
  closeStarred();
  if (state.active !== jid) await openChat(jid);
  jumpToQuoted(id);
}

/* ----------------------------------------------------------- communities */

// Fetch group metadata lazily (paced, one per ~900ms) to learn the
// community/parent link; cached with the session so restarts stay quiet.
let groupMetaQueue = [];
const groupMetaPending = new Set();
const groupMetaFailures = new Map();
let groupMetaPumpTimer = null;

function queueGroupMeta(chat) {
  if (!state.connected || !chat.isGroup || isChannelJid(chat.jid)) return;
  state.groupMeta = state.groupMeta || {};
  // Metas written before the announce flag existed lack `isAnnounce` — treat
  // them as stale so they re-fetch once and learn it.
  const cached = state.groupMeta[chat.jid];
  if (cached && typeof cached.isAnnounce === "boolean") return;
  if (groupMetaPending.has(chat.jid)) return;
  if ((groupMetaFailures.get(chat.jid) || 0) >= 3) return;
  groupMetaPending.add(chat.jid);
  groupMetaQueue.push(chat.jid);
  pumpGroupMeta();
}

function pumpGroupMeta() {
  if (groupMetaPumpTimer || !groupMetaQueue.length) return;
  groupMetaPumpTimer = setTimeout(async () => {
    groupMetaPumpTimer = null;
    const jid = groupMetaQueue.shift();
    if (!jid) return;
    try {
      const info = await invokeWithTimeout("group_info", { chat: jid }, 15000);
      if (info && typeof info === "object" && info.subject) {
        state.groupMeta = state.groupMeta || {};
        state.groupMeta[jid] = {
          subject: info.subject,
          isCommunity: Boolean(info.is_community),
          parentJid: info.parent_jid || null,
          isAnnounce: Boolean(info.is_announce),
        };
        // The fetched subject is authoritative; it also heals stale or
        // polluted display names.
        const chat = state.chats.get(jid);
        if (chat && chat.name !== info.subject) chat.name = info.subject;
        // A child's parent may be a community that never synced as a chat
        // (the parent itself holds no messages): fetch its subject and seed
        // a chat entry so the official-style community header can render.
        const parentJid = info.parent_jid || null;
        if (parentJid && !state.chats.has(parentJid) && !state.groupMeta[parentJid]) {
          queueParentMeta(parentJid);
        }
        if (!state.chats.has(jid) && state.groupMeta[jid].isCommunity) {
          upsertChat({ chat: jid, name: info.subject, isGroup: true });
        }
        scheduleSave();
        scheduleChatListRender();
      }
    } catch {
      groupMetaFailures.set(jid, (groupMetaFailures.get(jid) || 0) + 1);
    }
    groupMetaPending.delete(jid);
    pumpGroupMeta();
  }, 900);
}

// Communities never appear in history sync when they hold no messages, so
// their parent groups are fetched on demand once a child reveals them.
function queueParentMeta(parentJid) {
  if (groupMetaPending.has(parentJid)) return;
  if ((groupMetaFailures.get(parentJid) || 0) >= 3) return;
  groupMetaPending.add(parentJid);
  groupMetaQueue.push(parentJid);
  pumpGroupMeta();
}

/* ------------------------------------------------- community screen */

// Clicking a community opens the official-style community screen: the
// announcement row, the "Add group" affordance, and the child groups —
// instead of the (always empty) community chat itself.
function openCommunity(jid) {
  state.communityView = jid;
  $("community-screen").hidden = false;
  renderCommunityScreen();
}

function closeCommunity() {
  state.communityView = null;
  $("community-screen").hidden = true;
  renderChatList();
}

function renderCommunityScreen() {
  const jid = state.communityView;
  const chat = state.chats.get(jid);
  if (!chat) {
    closeCommunity();
    return;
  }
  invoke("ui_community", { request: sidebarRequest(), jid })
    .then((out) => {
      if (state.communityView !== jid) return;
      $("community-title").textContent = out.title;
      $("community-body").innerHTML = out.html;
    })
    .catch((err) => console.warn("community render failed", err));
}

/* -------------------------------------------------------------- channels */

// WhatsApp channels are newsletters: broadcast-only chats on @newsletter.
function isChannelJid(jid) {
  return typeof jid === "string" && jid.endsWith("@newsletter");
}

let channelsSyncing = false;

async function syncChannelMetadata() {
  if (channelsSyncing || !state.connected) return;
  channelsSyncing = true;
  try {
    await invoke("sync_channels");
  } catch (err) {
    console.warn("channel metadata unavailable", err);
  } finally {
    channelsSyncing = false;
    if (!$("channels-screen").hidden) renderChannels();
  }
}

function renderChannels() {
  const list = $("channels-list");
  if (!list) return;
  const needle = ($("channels-search")?.value || "").trim();
  invoke("ui_channels", { request: sidebarRequest(), needle })
    .then((out) => {
      list.innerHTML = out.html;
      $("channels-empty").hidden = out.empty_hidden;
      queuePhotos(out.queue || []);
    })
    .catch((err) => console.warn("channels render failed", err));
}

function showChannelsDiscover(show) {
  $("channels-discover").hidden = !show;
  if (show) $("chat-empty").hidden = true;
}

function openChannels() {
  $("status-screen").hidden = true;
  $("settings-screen").hidden = true;
  $("new-chat-screen").hidden = true;
  $("group-info-screen").hidden = true;
  closeStatusViewer();
  $("channels-screen").hidden = false;
  setRailActive("rail-channels");
  renderChannels();
  hideStatusShare();
  // WhatsApp shows the "Discover channels" pane when the panel is opened
  // without a channel selected.
  if (!isChannelJid(state.active)) {
    state.active = null;
    $("chat-panel").hidden = true;
    showChannelsDiscover(true);
  }
  // Channel names/follower counts are not part of chat history; fetch them
  // once per session (and retry if the panel is opened before the first run).
  if (state.connected) syncChannelMetadata();
}

/* Green notification badges on the rail icons, like the official client: the
   chats icon carries a count pill for unread chats and the status icon lights
   with a plain dot while unseen stories exist. */
let railStatusUnseen = 0;
// Cheap change-guard: updateRailBadges runs on every sidebar render, so skip
// the DOM work unless the computed badge values actually changed.
let railBadgeState = "";

// status.js reports how many contacts still have an unseen update.
function setRailStatusUnseen(count) {
  const next = Number(count) || 0;
  if (next === railStatusUnseen) return;
  railStatusUnseen = next;
  updateRailBadges();
}

function updateRailBadges() {
  let unreadChats = 0;
  for (const [, chat] of state.chats) {
    if (isChannelJid(chat.jid) || isSelfChat(chat.jid)) continue;
    if (chat.deleted && !state.showDeleted) continue;
    if (chat.unread > 0) unreadChats += 1;
  }
  const chatsText = unreadChats ? String(Math.min(unreadChats, 99)) : "";
  const statusText = railStatusUnseen ? "dot" : "";
  const key = `${chatsText}|${statusText}`;
  if (key === railBadgeState) return;
  railBadgeState = key;
  setRailBadge($("rail-chats"), chatsText);
  setRailBadge($("btn-status"), statusText);
}

function setRailBadge(btn, text) {
  if (!btn) return;
  let badge = btn.querySelector(".rail-badge");
  if (!text) {
    if (badge) badge.remove();
    return;
  }
  if (!badge) {
    badge = document.createElement("span");
    btn.appendChild(badge);
  }
  badge.className = text === "dot" ? "rail-badge rail-dot" : "rail-badge rail-count";
  badge.textContent = text === "dot" ? "" : text;
}

// Opening a channel keeps the channel list on the left and shows the posts on
// the right, with the composer hidden (channels are read-only).
function openChannel(jid) {
  openChat(jid);
  $("channels-screen").hidden = false;
  setRailActive("rail-channels");
  showChannelsDiscover(false);
  renderChannels();
  // Load the channel's latest posts through the newsletter self-IQ once per
  // session; scrolling to the top pages further back with the server_id cursor.
  requestChannelHistory(state.chats.get(jid)).then(() => {
    if (state.active === jid) renderChat();
  });
}

const MEDIA_LABELS = {
  image: "Photo",
  video: "Video",
  audio: "Audio",
  document: "Document",
  sticker: "Sticker",
};

function putMedia(id, entry) {
  state.media.delete(id);
  state.media.set(id, entry);
  while (state.media.size > MAX_MEDIA_URLS) {
    const oldest = state.media.keys().next().value;
    if (oldest === undefined) break;
    state.media.delete(oldest);
  }
}

function rememberMedia(id, url) {
  putMedia(id, { status: "ready", url });
}
