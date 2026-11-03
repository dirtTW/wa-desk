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
    names[jidUser(message.sender)] = message.sender_name;
  }
  return names;
}

function sidebarRequest() {
  // Groups that never got their metadata are queued here (the guard inside makes
  // a repeated render cheap).
  for (const chat of state.chats.values()) queueGroupMeta(chat);
  const chats = [];
  for (const chat of state.chats.values()) {
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
      tail: (chat.messages || []).slice(-6).map(sidebarMessage),
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

/* ------------------------------------------- sidebar multi-select ("Select chats") */

// These four were called from the sidebar menu and the row handler but never
// defined, so "Select chats" used to throw. The selection maths is
// wa_ui::actions::toggle_selection; the bulk commands are planned in
// wa_ui::chat_actions::bulk_chat_plan.

async function updateChatSelectBar() {
  const count = state.selectedChats.size;
  const bar = await invoke("ui_selection_bar", { count }).catch(() => ({
    label: `${count} selected`,
    disabled: count === 0,
  }));
  $("chat-select-count").textContent = bar.label;
  for (const id of ["chat-select-read", "chat-select-archive", "chat-select-delete"]) {
    const node = $(id);
    if (node) node.disabled = bar.disabled;
  }
  $("chat-select-bar").hidden = false;
}

function startChatSelection() {
  state.selectingChats = true;
  state.selectedChats = new Set();
  $("chat-list").classList.add("select-mode");
  updateChatSelectBar();
  renderChatList();
}

function exitChatSelection() {
  state.selectingChats = false;
  state.selectedChats = new Set();
  $("chat-list").classList.remove("select-mode");
  $("chat-select-bar").hidden = true;
  renderChatList();
}

async function toggleChatSelection(jid) {
  if (!state.selectingChats || !jid) return;
  // Same add/remove/exit rules as the message selection.
  const plan = await invoke("ui_toggle_selection", {
    selected: [...state.selectedChats],
    id: jid,
  }).catch(() => null);
  if (!plan) return;
  state.selectedChats = new Set(plan.selected);
  if (plan.exit) {
    exitChatSelection();
    return;
  }
  updateChatSelectBar();
  renderChatList();
}

/// "Mark all as read" from the sidebar menu: every chat with an unread badge.
async function markAllChatsRead() {
  const unread = [...state.chats.values()].filter((chat) => (chat.unread || 0) > 0);
  if (!unread.length) {
    toast("No unread chats");
    return;
  }
  for (const chat of unread) await markRead(chat.jid);
  toast(`Marked ${unread.length} chat${unread.length === 1 ? "" : "s"} as read`);
}

/// Run one sidebar bulk action over the selected chats.
async function applyChatSelection(action) {
  const jids = [...state.selectedChats];
  if (!jids.length) return;
  const chats = jids.map((jid) => state.chats.get(jid)).filter(Boolean);
  let plans = [];
  try {
    plans = await invoke("ui_bulk_chat_plan", {
      action,
      chats: chats.map((chat) => ({
        jid: chat.jid,
        pinned: Boolean(chat.pinned),
        archived: Boolean(chat.archived),
        unread: chat.unread || 0,
        is_group: Boolean(chat.isGroup),
      })),
    });
  } catch (err) {
    toast(String(err));
    return;
  }
  const confirmText = plans.find((plan) => plan.confirm)?.confirm;
  if (confirmText && !confirm(confirmText)) return;

  for (const plan of plans) {
    try {
      if (plan.command) await invoke(plan.command, plan.args || {});
      const entry = state.chats.get(plan.args?.chat);
      if (!entry) continue;
      if (plan.archived !== null && plan.archived !== undefined) entry.archived = plan.archived;
      if (plan.unread !== null && plan.unread !== undefined) entry.unread = 0;
      if (plan.delete) entry.deleted = true;
    } catch (err) {
      toast(String(err));
    }
  }
  if (action === "delete" && state.active && jids.includes(state.active)) {
    state.active = null;
    renderChat();
  }
  exitChatSelection();
  scheduleSave();
}



// Starred ids come from the `star` app-state collection (a full sync replays
// every star), so this list matches the phone and every linked client.
/* ------------------------------------------------------ starred messages */

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
          await upsertChat({ chat: jid, name: info.subject, isGroup: true });
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

async function updateRailBadges() {
  // Which chats count (channels, the account's own chat and hidden deleted
  // chats are skipped), the 99 cap and the dot are
  // wa_ui::sidebar::rail_badges.
  const badges = await invoke("ui_rail_badges", {
    chats: [...state.chats.values()].map((chat) => ({
      jid: chat.jid || "",
      unread: chat.unread || 0,
      deleted: Boolean(chat.deleted),
    })),
    showDeleted: Boolean(state.showDeleted),
    statusUnseen: railStatusUnseen,
    meJid: state.meJid || "",
    meLid: state.meLid || "",
  }).catch(() => null);
  if (!badges) return;
  if (badges.key === railBadgeState) return;
  railBadgeState = badges.key;
  setRailBadge($("rail-chats"), badges.chats_text);
  setRailBadge($("btn-status"), badges.status_text);
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

/* -------------------------------------------------------------- group info */

/* -------------------------------------------------------------- group info */

async function openGroupInfo() {
  const chat = state.chats.get(state.active);
  if (!chat || !chat.isGroup) return;

  $("group-info-screen").hidden = false;
  $("settings-screen").hidden = true;
  $("status-screen").hidden = true;
  $("new-chat-screen").hidden = true;
  hideStatusShare();
  $("group-info-name").textContent = chat.name;
  $("group-info-count").textContent = "";
  $("group-info-desc").textContent = "";
  $("group-members").innerHTML = '<p class="new-chat-empty">Loading members...</p>';
  setAvatar($("group-info-avatar"), chat.name, chat.jid);
  queuePhoto(chat.jid);

  try {
    const info = await invoke("group_info", { chat: chat.jid });
    $("group-info-name").textContent = info.subject || chat.name;
    // The subtitle is wa_ui::labels::group_member_count.
    $("group-info-count").textContent = await invoke("ui_group_member_count", {
      count: info.members.length,
    }).catch(() => `Group - ${info.members.length} members`);
    $("group-info-desc").textContent = info.description || "";
    const rendered = await invoke("ui_group_members", {
      members: await Promise.all(
        info.members.map(async (member) => ({
          jid: member.jid,
          phone:
            member.phone ||
            (await invoke("ui_phone_from_jid", { jid: member.jid }).catch(() => "")),
          admin: Boolean(member.admin),
        })),
      ),
      contacts: Object.fromEntries(state.contacts),
      photos: Object.fromEntries(
        [...state.photos].filter(([, url]) => typeof url === "string" && url.startsWith("http")),
      ),
    });
    $("group-members").innerHTML = rendered.html;
    for (const jid of rendered.queue || []) queuePhoto(jid);
  } catch (err) {
    $("group-members").innerHTML = `<p class="new-chat-empty">${escapeHtml(String(err))}</p>`;
  }
}

async function leaveCurrentGroup() {
  const chat = state.chats.get(state.active);
  if (!chat || !chat.isGroup) return;
  // The confirmation copy is wa_ui::chat_actions::leave_group_confirm.
  const confirmation = await invoke("ui_leave_group_confirm", {
    name: chat.name || "",
  }).catch(() => "Leave this group?");
  if (!confirm(confirmation)) return;
  try {
    await invoke("leave_group", { chat: chat.jid });
    state.chats.delete(chat.jid);
    $("group-info-screen").hidden = true;
    state.active = null;
    renderChatList();
    renderChat();
    scheduleSave();
    toast("Left the group");
  } catch (err) {
    toast(String(err));
  }
}

/* ------------------------------------------------- group member actions */

let memberTarget = null;

function closeMemberMenu() {
  closeMenuSession();
  memberTarget = null;
}

async function openMemberMenu(button) {
  const row = button.closest(".group-member");
  if (!row) return;
  memberTarget = {
    jid: row.dataset.memberJid,
    admin: row.dataset.memberAdmin === "true",
  };

  const menu = $("member-menu");
  menu.querySelector('[data-member-action="promote"]').hidden = memberTarget.admin;
  menu.querySelector('[data-member-action="demote"]').hidden = !memberTarget.admin;
  // One popover at a time.
  await openMenuSession("member", memberTarget.jid || "");

  // The edge maths is wa_ui::menus::member_menu_position.
  const rect = button.getBoundingClientRect();
  const menuRect = menu.getBoundingClientRect();
  const pos = await invoke("ui_member_menu_position", {
    anchor: { x: rect.left, y: rect.top, width: rect.width, height: rect.height },
    menuW: menuRect.width || 210,
    menuH: menuRect.height || 130,
    viewportW: window.innerWidth,
    viewportH: window.innerHeight,
  }).catch(() => ({ left: rect.right - 190, top: rect.bottom + 4 }));
  menu.style.left = `${pos.left}px`;
  menu.style.top = `${pos.top}px`;
}

async function applyMemberAction(action) {
  const target = memberTarget;
  if (!target) return;
  closeMemberMenu();
  // The command, the confirmation copy and the toast are wa_ui::chat_actions.
  const plan = await invoke("ui_member_action", {
    action,
    hasTarget: Boolean(target.jid),
  }).catch(() => null);
  if (!plan || !plan.allowed) return;
  if (plan.confirm && !confirm(plan.confirm)) return;

  const chat = state.active;
  try {
    await invoke(plan.command, { chat, participants: [target.jid] });
    toast(plan.toast);
    if (plan.refresh) await openGroupInfo();
  } catch (err) {
    toast(String(err));
  }
}

/* --------------------------------------------------------------- new chat */

/* ---------------------------------------------------------------- new chat */

function openNewChat() {
  $("settings-screen").hidden = true;
  $("status-screen").hidden = true;
  $("channels-discover").hidden = true;
  $("channels-screen").hidden = true;
  closeMessageMenu();
  $("new-chat-screen").hidden = false;
  $("group-info-screen").hidden = true;
  $("group-modal").hidden = true;
  $("new-chat-input").value = "";
  // Leaving forward mode: the same picker doubles as "Forward to".
  forwardMessage = null;
  forwardTargets.clear();
  $("new-chat-title").textContent = "New chat";
  $("forward-bar").hidden = true;
  renderNewChat();
  $("new-chat-input").focus();
  hideStatusShare();
}

/* --------------------------------------------------------------- forwarding */

let forwardMessage = null;
const forwardTargets = new Set();

// WhatsApp Web opens the chat picker with checkboxes and a Send button; we reuse
// the new-chat screen in that mode.
function openForwardPicker(message) {
  if (!message) return;
  openNewChat();
  forwardMessage = message;
  forwardTargets.clear();
  $("new-chat-title").textContent = "Forward to";
  $("forward-bar").hidden = false;
  renderNewChat();
  updateForwardBar();
  $("new-chat-input").focus();
}

function toggleForwardTarget(jid) {
  if (!jid) return;
  if (forwardTargets.has(jid)) forwardTargets.delete(jid);
  else forwardTargets.add(jid);
  renderNewChat();
  updateForwardBar();
}

function updateForwardBar() {
  const count = forwardTargets.size;
  $("forward-count").textContent =
    count === 0 ? "Forward to…" : count === 1 ? "1 chat selected" : `${count} chats selected`;
  $("forward-send").disabled = count === 0;
}

async function sendForward() {
  if (!forwardMessage || forwardTargets.size === 0) return;
  const button = $("forward-send");
  button.disabled = true;
  const source = forwardMessage;
  // Multi-select queues several messages; a single forward has one entry.
  const queue =
    Array.isArray(state.forwardQueue) && state.forwardQueue.length
      ? state.forwardQueue
      : [{ id: source.id, text: source.text || "" }];
  const total = queue.length * forwardTargets.size;
  let placeholders = 0;
  for (let index = 0; index < total; index += 1) {
    const item = queue[index % queue.length];
    const jid = [...forwardTargets][Math.floor(index / queue.length)];
    let entry = state.chats.get(jid);
    if (!entry) {
      const user = await invoke("ui_phone_from_jid", { jid }).catch(() => "");
      entry = {
        jid,
        name:
          state.contacts.get(user) ||
          (await invoke("ui_format_phone", { phone: user }).catch(() => "")) ||
          jid,
        isGroup: jid.endsWith("@g.us"),
        messages: [],
        last: null,
        lastTs: 0,
        unread: 0,
      };
      state.chats.set(jid, entry);
    }
    const pendingId = `local-fwd-${Date.now()}-${placeholders}`;
    placeholders += 1;
    const optimistic = {
      ...source,
      id: pendingId,
      chat: jid,
      chat_name: entry.name,
      from_me: true,
      is_group: entry.isGroup,
      sender: null,
      sender_name: null,
      sender_phone: null,
      timestamp: Date.now() + index,
      pending: true,
      forwarded: true,
      reactions: [],
      reply_to: null,
      stats: null,
      text: item.text,
    };
    entry.messages.push(optimistic);
    entry.last = optimistic;
    entry.lastTs = optimistic.timestamp;
  }
  scheduleSave();
  renderChatList();
  renderChat();
  try {
    let sent = 0;
    for (const item of queue) {
      sent += await invoke("forward_message", {
        chats: [...forwardTargets],
        id: item.id,
        text: item.text || null,
      });
    }
    $("new-chat-screen").hidden = true;
    forwardMessage = null;
    forwardTargets.clear();
    state.forwardQueue = null;
    $("forward-bar").hidden = true;
    if (state.selecting) exitSelection();
    // The copy is wa_ui::picker::forward_toast.
    toast(
      await invoke("ui_forward_toast", { sent }).catch(
        () => `${sent} messages forwarded`,
      ),
    );
  } catch (err) {
    for (const entry of state.chats.values()) {
      entry.messages = entry.messages.filter((m) => !String(m.id).startsWith("local-fwd-"));
      entry.last = entry.messages[entry.messages.length - 1] || null;
    }
    scheduleSave();
    renderChatList();
    renderChat();
    toast(String(err));
    button.disabled = false;
  }
}

function closeNewChat() {
  $("new-chat-screen").hidden = true;
  $("new-chat-input").value = "";
  renderChat();
}

async function knownLids() {
  const lids = new Set();
  const add = async (jid) => {
    const user = await invoke("ui_phone_from_jid", { jid }).catch(() => "");
    if (user) lids.add(user);
  };
  for (const chat of state.chats.values()) {
    if (chat.jid.endsWith("@lid")) await add(chat.jid);
    for (const message of chat.messages.slice(-40)) {
      if (message.sender && String(message.sender).endsWith("@lid")) {
        await add(message.sender);
      }
    }
  }
  return lids;
}

async function pickerRequest() {
  return {
    contacts: Object.fromEntries(state.contacts),
    chats: [...state.chats.values()].map((chat) => ({
      jid: chat.jid,
      name: chat.name || "",
      is_group: Boolean(chat.isGroup),
    })),
    phones: Object.fromEntries(state.phones),
    known_lids: [...(await knownLids())],
    photos: Object.fromEntries(
      [...state.photos].filter(([, url]) => typeof url === "string" && url.startsWith("http")),
    ),
    me_jid: state.meJid || "",
    me_lid: state.meLid || "",
    query: $("new-chat-input").value.trim(),
    forward: Boolean(forwardMessage),
    forward_targets: [...forwardTargets],
  };
}

// The rows, letter sections, raw-number row and forward ticks are built by
// wa-ui::picker; this only injects the markup and queues the avatars.
let pickerSeq = 0;

async function renderNewChat() {
  const seq = ++pickerSeq;
  let out;
  try {
    out = await invoke("ui_picker", { request: await pickerRequest() });
  } catch (err) {
    console.warn("picker render failed", err);
    return;
  }
  if (seq !== pickerSeq) return;
  document.querySelector(".new-chat-actions").hidden = out.actions_hidden;
  $("new-chat-list").innerHTML =
    out.html || '<p class="new-chat-empty">No contacts match that search.</p>';
  const ids = [...$("new-chat-list").querySelectorAll("[data-chat-jid]")]
    .slice(0, 40)
    .map((node) => node.dataset.chatJid);
  for (const jid of ids) queuePhoto(jid);
}

async function openChatFromPicker(jid, name) {
  if (!jid) return;
  if (!state.chats.has(jid)) {
    const user = await invoke("ui_phone_from_jid", { jid }).catch(() => "");
    const label = state.contacts.get(user) || user;
    state.chats.set(jid, {
      jid,
      name: name || label || (await invoke("ui_format_phone", { phone: user }).catch(() => "")),
      isGroup: false,
      messages: [],
      last: null,
      lastTs: 0,
      unread: 0,
    });
  }
  $("new-chat-screen").hidden = true;
  scheduleSave();
  openChat(jid);
}

/* ---------------------------------------------------------------- contacts */

function openContactModal() {
  $("contact-name").value = "";
  $("contact-phone").value = "";
  $("contact-error").hidden = true;
  $("contact-modal").hidden = false;
  $("contact-name").focus();
}

async function saveContact() {
  const name = $("contact-name").value.trim();
  const phone = $("contact-phone").value.trim();
  const error = $("contact-error");
  error.hidden = true;

  if (!name) {
    error.textContent = "Give the contact a name.";
    error.hidden = false;
    return;
  }
  if (phone.replace(/\D/g, "").length < 8) {
    error.textContent = "Enter a full international number, e.g. +62 812 3456 7890.";
    error.hidden = false;
    return;
  }

  $("contact-save").disabled = true;
  try {
    const jid = await invoke("save_contact", { phone, name });
    applyContactName(jid, name);
    $("contact-modal").hidden = true;
    $("new-chat-screen").hidden = true;
    openChatFromPicker(jid, name);
    toast("Contact saved");
  } catch (err) {
    error.textContent = String(err);
    error.hidden = false;
  } finally {
    $("contact-save").disabled = false;
  }
}


/* --------------------------------------------------------------- new group */

/* --------------------------------------------------------------- new group */

let groupSelection = new Set();
let groupModalMode = "create";
let groupTargetChat = null;

async function openGroupModal() {
  groupModalMode = "create";
  groupTargetChat = null;
  groupSelection = new Set();
  // The title, submit label, subject visibility and focus are
  // wa_ui::picker::group_modal.
  await applyGroupModalChrome();
  $("group-subject").value = "";
  $("group-search").value = "";
  $("group-error").hidden = true;
  $("group-modal").hidden = false;
  renderGroupContacts();
  $(groupModalFocus).focus();
}

/// The labels the dialog shows for the current mode.
let groupModalFocus = "group-subject";
let groupModalToast = "Group created";

async function applyGroupModalChrome() {
  const plan = await invoke("ui_group_modal", { mode: groupModalMode }).catch(() => null);
  if (!plan) return;
  $("group-modal-title").textContent = plan.title;
  $("group-create-label").textContent = plan.submit_label;
  $("group-subject-field").hidden = !plan.show_subject;
  groupModalFocus = plan.focus;
  groupModalToast = plan.success_toast;
}

async function openAddMembers() {
  const chat = state.chats.get(state.active);
  if (!chat || !chat.isGroup) return;
  groupModalMode = "add";
  groupTargetChat = chat.jid;
  groupSelection = new Set();
  await applyGroupModalChrome();
  $("group-search").value = "";
  $("group-error").hidden = true;
  $("group-modal").hidden = false;
  renderGroupContacts();
  $(groupModalFocus).focus();
}

// The checklist markup (filtering, formatted numbers, checked state) is built
// by wa-ui::picker; this only sends the selection and injects the HTML.
async function renderGroupContacts() {
  let out;
  try {
    out = await invoke("ui_group_contacts", {
      request: {
        contacts: Object.fromEntries(state.contacts),
        known_lids: [...(await knownLids())],
        photos: Object.fromEntries(
          [...state.photos].filter(([, url]) => typeof url === "string" && url.startsWith("http")),
        ),
        me_jid: state.meJid || "",
        query: $("group-search").value,
        selected: [...groupSelection],
      },
    });
  } catch (err) {
    console.warn("group contacts render failed", err);
    return;
  }
  $("group-contacts").innerHTML = out.html;
  queuePhotos(out.queue || []);
}

async function createGroup() {
  const error = $("group-error");
  error.hidden = true;

  // The form rules are wa_ui::picker::group_form_error.
  const formError = await invoke("ui_group_form_error", {
    mode: groupModalMode,
    subject: $("group-subject").value,
    selected: groupSelection.size,
  }).catch(() => null);
  if (formError) {
    error.textContent = formError;
    error.hidden = false;
    return;
  }

  if (groupModalMode === "add") {
    if (!groupTargetChat) return;
    $("group-create").disabled = true;
    try {
      await invoke("group_add_members", {
        chat: groupTargetChat,
        participants: [...groupSelection],
      });
      $("group-modal").hidden = true;
      toast(groupModalToast);
      if (!$("group-info-screen").hidden) await openGroupInfo();
    } catch (err) {
      error.textContent = String(err);
      error.hidden = false;
    } finally {
      $("group-create").disabled = false;
    }
    return;
  }

  const subject = $("group-subject").value.trim();

  $("group-create").disabled = true;
  try {
    const jid = await invoke("create_group", {
      subject,
      participants: [...groupSelection],
    });
    // The empty chat row is wa_ui::state::created_group_chat.
    const row = await invoke("ui_created_group_chat", {
      jid,
      name: subject,
      now: Date.now(),
    }).catch(() => null);
    if (row) state.chats.set(jid, row);
    $("group-modal").hidden = true;
    scheduleSave();
    openChat(jid);
    toast(groupModalToast);
  } catch (err) {
    error.textContent = String(err);
    error.hidden = false;
  } finally {
    $("group-create").disabled = false;
  }
}

/* --------------------------------------------------------------- settings */

/* ------------------------------------------------------------------ settings */

const SETTINGS_KEY = "wadesk.settings.v1";

let settingsPage = "home";
let settingsPrefs = {
  theme: "system",
  enterSends: true,
  spellcheck: true,
  notifications: true,
  autostart: false,
};

function loadSettingsPrefs() {
  try {
    const parsed = JSON.parse(localStorage.getItem(SETTINGS_KEY) || "null");
    if (parsed && typeof parsed === "object") settingsPrefs = { ...settingsPrefs, ...parsed };
  } catch {
    /* keep defaults */
  }
}

function saveSettingsPrefs() {
  try {
    localStorage.setItem(SETTINGS_KEY, JSON.stringify(settingsPrefs));
  } catch {
    /* quota */
  }
}

/* ------------------------------------------------------------------ theme */

const darkMedia = window.matchMedia("(prefers-color-scheme: dark)");

// Theme icons come from the Material Symbols registry (ui_icon); the toggle
// shows the theme you would switch TO, like the official client.
// Labels and resolution are wa_ui::settings' rules; cached for the synchronous
// call sites (settings rows, theme dialog).
let themeLabels = { light: "Light", dark: "Dark", system: "System default" };
let themeCache = {};
invoke("ui_theme_resolve", { preference: settingsPrefs.theme, systemDark: darkMedia.matches })
  .then((resolved) => {
    themeCache[settingsPrefs.theme] = resolved;
  })
  .catch(() => {});

function themeLabel(value = settingsPrefs.theme) {
  return themeLabels[value] || "System default";
}

function resolvedTheme() {
  const cached = themeCache[settingsPrefs.theme];
  if (cached) return cached;
  if (settingsPrefs.theme === "light" || settingsPrefs.theme === "dark") {
    return settingsPrefs.theme;
  }
  return darkMedia.matches ? "dark" : "light";
}

async function refreshThemeLabels() {
  // The Rust rules are authoritative; fetch both resolutions for the cache.
  const preferences = ["light", "dark", "system", "default"];
  await Promise.all(
    preferences.map(async (preference) => {
      themeLabels[preference] = await invoke("ui_theme_label", { preference }).catch(
        () => themeLabels[preference],
      );
      themeCache[preference] = await invoke("ui_theme_resolve", {
        preference,
        systemDark: darkMedia.matches,
      }).catch(() => themeCache[preference]);
    }),
  );
}

function applyTheme() {
  const resolved = resolvedTheme();
  document.documentElement.dataset.theme = resolved;

  const toggle = $("theme-toggle");
  if (toggle) {
    // The label/icon pair comes from wa_ui::settings::theme_toggle.
    invoke("ui_theme_toggle", { resolved })
      .then(async (next) => {
        toggle.innerHTML = await invoke("ui_icon", { name: next.icon }).catch(
          () => toggle.innerHTML,
        );
        toggle.title = `Switch to ${next.label.toLowerCase()} theme`;
        toggle.setAttribute("aria-label", toggle.title);
      })
      .catch(() => {});
  }

  for (const node of document.querySelectorAll("[data-theme-label]")) {
    node.textContent = themeLabel();
  }
  const dialog = $("theme-dialog");
  if (dialog) {
    const form = $("theme-form");
    if (form) {
      for (const radio of form.querySelectorAll("md-radio")) {
        radio.checked = radio.value === settingsPrefs.theme;
      }
    }
  }
}

function setTheme(value) {
  const allowed = ["light", "dark", "system"];
  settingsPrefs.theme = allowed.includes(value) ? value : "system";
  saveSettingsPrefs();
  applyTheme();
  if (!$("settings-screen").hidden) renderSettings();
}

function openThemeDialog() {
  const dialog = $("theme-dialog");
  if (!dialog || typeof dialog.show !== "function") return;
  applyTheme();
  dialog.returnValue = "";
  dialog.show().catch(() => {});
}

darkMedia.addEventListener("change", () => {
  if (settingsPrefs.theme === "system") applyTheme();
});

function setRailActive(id) {
  for (const btn of document.querySelectorAll("#rail .rail-btn")) {
    btn.classList.toggle("active", btn.id === id);
  }
}

function openSettings(page = "home") {
  settingsPage = page;
  $("channels-discover").hidden = true;
  $("settings-screen").hidden = false;
  $("status-screen").hidden = true;
  $("new-chat-screen").hidden = true;
  $("group-info-screen").hidden = true;
  $("channels-screen").hidden = true;
  closeMessageMenu();
  closeStatusViewer();
  setRailActive("rail-settings");
  renderSettings();
  hideStatusShare();
}

function closeSettings() {
  $("settings-screen").hidden = true;
  settingsPage = "home";
  setRailActive("rail-chats");
  renderChat();
}

async function backFromSettings() {
  // Subpage -> home, home -> close: wa_ui::settings::settings_back.
  const next = await invoke("ui_settings_back", { page: settingsPage }).catch(() => "home");
  if (next === "close") closeSettings();
  else openSettings(next);
}

// Everyone whose work this client stands on. `Help and feedback` lists these
// with links; keep it in step with the Credits section of the README.
const CREDITS = [
  ["whatsapp-rust", "jlucaso1 — the WhatsApp Web protocol client", "https://github.com/jlucaso1/whatsapp-rust"],
  ["libsignal", "Signal — the end-to-end encryption primitives", "https://github.com/signalapp/libsignal"],
  ["whatsmeow / Baileys", "protocol reference implementations", "https://github.com/tulir/whatsmeow"],
  ["Tauri", "the desktop app framework", "https://tauri.app/"],
  ["tokio", "the async runtime", "https://tokio.rs/"],
  ["serde", "serialization", "https://serde.rs/"],
  ["RustCrypto", "aes-gcm, sha2, hmac, hkdf", "https://github.com/RustCrypto"],
  ["ureq", "the blocking HTTP client", "https://github.com/algesten/ureq"],
  ["qrcode", "the pairing QR code", "https://github.com/kennytm/qrcode-rust"],
  ["tracing", "logging", "https://tokio.rs/tokio/tracing"],
  ["Material Symbols", "every icon in this UI", "https://fonts.google.com/icons"],
  ["Material Web", "the settings dialog", "https://github.com/material-components/material-web"],
  ["OpenCode", "the agentic coding environment", "https://opencode.ai"],
  ["DeepSeek & GLM", "the models that wrote the code", "https://deepseek.com"],
];

/* ---------------------------------------------------------------- updates */

// The Account screen's update row: a manual check, then a one-click install.
// `check_for_update`/`install_update` go through tauri-plugin-updater, which
// reads the `latest.json` on the newest GitHub release and verifies the
// installer against the minisign key in `plugins.updater.pubkey`.
let updateState = { checking: false, version: null, status: "" };

async function checkForUpdate() {
  updateState = { checking: true, version: null, status: "Checking for updates..." };
  renderSettings();
  try {
    const result = await invoke("check_for_update");
    updateState = result.version
      ? {
          checking: false,
          version: result.version,
          status: `Version ${result.version} is available — you are on ${result.current}.`,
        }
      : {
          checking: false,
          version: null,
          status: `You are on the newest version (${result.current}).`,
        };
  } catch (err) {
    updateState = {
      checking: false,
      version: null,
      status: `Could not check for updates: ${String(err)}`,
    };
  }
  renderSettings();
}

async function installUpdate() {
  updateState = {
    checking: false,
    version: null,
    status: "Downloading and installing... WA Desk restarts when it is done.",
  };
  renderSettings();
  try {
    await invoke("install_update");
  } catch (err) {
    updateState = {
      checking: false,
      version: null,
      status: `Could not install the update: ${String(err)}`,
    };
    renderSettings();
  }
}

/* ----------------------------------------------- Rust-rendered settings */

// Every row (home, profile, account, privacy, chats, AI, shortcuts, help) is
// built by wa-ui::settings; this only supplies the state and injects the HTML.
function settingsRequest() {
  return {
    page: settingsPage,
    me: state.me || "",
    me_jid: state.meJid || "",
    me_lid: state.meLid || "",
    theme_label: themeLabel(),
    build: buildLabel(),
    prefs: {
      enter_sends: Boolean(settingsPrefs.enterSends),
      spellcheck: Boolean(settingsPrefs.spellcheck),
      notifications: Boolean(settingsPrefs.notifications),
      autostart: Boolean(settingsPrefs.autostart),
    },
    update_state: {
      checking: Boolean(updateState.checking),
      version: updateState.version || null,
      status: updateState.status || "",
    },
    ai: {
      enabled: Boolean(state.ai?.enabled),
      require_trigger: Boolean(state.ai?.requireTrigger),
      base_url: state.ai?.baseUrl || "",
      model: state.ai?.model || "",
      vision_model: state.ai?.visionModel || "",
      system_prompt: state.ai?.systemPrompt || "",
      has_key: Boolean(state.ai?.hasKey),
      key_hint: state.ai?.keyHint || "",
    },
    with_avatar: true,
  };
}

let settingsRenderSeq = 0;

async function renderSettings() {
  const seq = ++settingsRenderSeq;
  let out;
  try {
    out = await invoke("ui_settings", { request: settingsRequest() });
  } catch (err) {
    console.warn("settings render failed", err);
    return;
  }
  if (seq !== settingsRenderSeq) return;
  settingsPage = out.page;
  $("settings-title").textContent = out.title;

  const container = $("settings-body");
  container.innerHTML = out.html;
  const avatar = container.querySelector("[data-avatar]");
  if (avatar) {
    setAvatar(avatar, state.me || "Me", state.meLid || state.meJid);
    queuePhoto(state.meLid || state.meJid);
  }
}

async function loadAiSettings() {
  try {
    state.ai = await invoke("get_ai_settings");
    if (!$("settings-screen").hidden && settingsPage === "ai") renderSettings();
  } catch (err) {
    console.warn("AI settings unavailable", err);
  }
}

async function toggleAiMode() {
  // Enabling needs a stored key (wa_ui::settings::ai_toggle).
  const plan = await invoke("ui_ai_toggle", {
    enabled: Boolean(state.ai.enabled),
    hasKey: Boolean(state.ai.hasKey),
  }).catch(() => null);
  if (!plan) return;
  if (!plan.allow) {
    toast(plan.toast);
    return;
  }
  try {
    state.ai = await invoke("set_ai_enabled", { enabled: plan.next });
    renderSettings();
    toast(plan.toast);
  } catch (err) {
    toast(String(err));
  }
}

async function saveAiSettings() {
  const error = $("ai-error");
  error.hidden = true;
  // Trimming, the URL/model checks and the "blank key keeps the saved key"
  // rule are wa_ui::settings::ai_save_plan.
  const plan = await invoke("ui_ai_save_plan", {
    input: {
      enabled: Boolean(state.ai.enabled),
      require_trigger: Boolean(state.ai.requireTrigger),
      base_url: $("ai-base").value,
      model: $("ai-model").value,
      vision_model: $("ai-vision-model").value,
      system_prompt: $("ai-prompt").value,
      api_key: $("ai-key").value,
    },
  }).catch(() => null);
  if (!plan) return;
  if (!plan.ok) {
    error.textContent = plan.error || "Could not save the AI settings.";
    error.hidden = false;
    return;
  }
  try {
    state.ai = await invoke("save_ai_settings", { settings: plan.settings });
    renderSettings();
    toast(plan.toast);
  } catch (err) {
    error.textContent = String(err);
    error.hidden = false;
  }
}

async function toggleRequireTrigger() {
  const next = !state.ai.requireTrigger;
  try {
    state.ai = await invoke("save_ai_settings", {
      settings: {
        enabled: state.ai.enabled,
        baseUrl: $("ai-base")?.value.trim() || state.ai.baseUrl,
        model: $("ai-model")?.value.trim() || state.ai.model,
        visionModel: $("ai-vision-model")?.value.trim() ?? state.ai.visionModel,
        systemPrompt: $("ai-prompt")?.value.trim() || state.ai.systemPrompt,
        requireTrigger: next,
        apiKey: $("ai-key")?.value.trim() || null,
      },
    });
    renderSettings();
    toast(await invoke("ui_ai_require_toast", { requireTrigger: next }));
  } catch (err) {
    toast(String(err));
  }
}

async function testAiConnection() {
  const button = $("ai-test");
  if (button) button.disabled = true;
  try {
    const reply = await invoke("test_ai");
    const excerpt = await invoke("ui_toast_excerpt", { reply: String(reply), limit: 100 });
    toast(`AI replied: ${excerpt}`);
  } catch (err) {
    toast(String(err));
  } finally {
    if (button && button.isConnected) button.disabled = false;
  }
}

async function previewAiReply() {
  const chat = state.chats.get(state.active);
  // The "open a 1:1 chat first" guard and the sample prompt are wa_ui::settings.
  const plan = await invoke("ui_ai_preview", {
    hasDm: Boolean(chat && !chat.isGroup),
    text: $("input").value,
  }).catch(() => null);
  if (!plan) return;
  if (!plan.ok) {
    if (plan.toast) toast(plan.toast);
    return;
  }
  try {
    const reply = await invoke("ai_draft_reply", { chat: chat.jid, text: plan.text });
    const excerpt = await invoke("ui_toast_excerpt", { reply: String(reply), limit: 120 });
    toast(`AI draft: ${excerpt}`);
  } catch (err) {
    toast(String(err));
  }
}

function openProfileEditor() {
  $("profile-name").value = state.me || "";
  $("profile-about").value = "";
  $("profile-error").hidden = true;
  $("profile-editor").hidden = false;
  $("profile-name").focus();
}

async function saveProfile() {
  const name = $("profile-name").value.trim();
  const about = $("profile-about").value.trim();
  const error = $("profile-error");
  error.hidden = true;

  const nameError = await invoke("ui_profile_name_error", { name }).catch(() => null);
  if (nameError) {
    error.textContent = nameError;
    error.hidden = false;
    return;
  }

  try {
    await invoke("set_push_name", { name });
    if (about) await invoke("set_status_text", { text: about });

    state.me = name;
    $("profile-editor").hidden = true;
    refreshMyAvatar();
    renderSettings();
    toast("Profile updated");
  } catch (err) {
    error.textContent = String(err);
    error.hidden = false;
  }
}

async function doLogout() {
  if (!confirm("Unlink this device? You will need to scan a QR code again.")) return;
  try {
    await invoke("logout");
    localStorage.removeItem(STORE_KEY);
    state.chats.clear();
    state.statuses.clear();
    state.active = null;
    closeSettings();
    renderChatList();
    renderChat();
    $("login").hidden = false;
    $("btn-logout").hidden = true;
    setConnection("logged out", "offline");
  } catch (err) {
    toast(String(err));
  }
}
