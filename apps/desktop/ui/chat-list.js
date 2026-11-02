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

function chatList() {
  const archivedView = Boolean(state.archivedOpen);
  return [...state.chats.values()]
    .filter((chat) => {
      if (isChannelJid(chat.jid)) return false;
      if (!WAChatUtils.isChatVisible(chat, state.showDeleted)) return false;
      // Archived chats live in their own view (the official "Archived" row),
      // so the main list skips them in both directions.
      if (Boolean(chat.archived) !== archivedView) return false;
      if (state.filter === "unread" && chat.unread === 0) return false;
      if (state.filter === "groups" && !chat.isGroup) return false;
      return true;
    })
    // Official rule: pinned chats float to the top in pin order, everything
    // else stays in activity order below them.
    .sort((a, b) => {
      const pa = chatPinScore(a);
      const pb = chatPinScore(b);
      if (pa !== pb) return pb - pa;
      if (pa === 1 && pb === 1) return (a.pinTs || 0) - (b.pinTs || 0);
      return (b.last?.timestamp || b.lastTs || 0) - (a.last?.timestamp || a.lastTs || 0);
    });
}

function archivedChatCount() {
  return [...state.chats.values()].filter(
    (chat) =>
      !isChannelJid(chat.jid) &&
      chat.archived &&
      WAChatUtils.isChatVisible(chat, state.showDeleted),
  ).length;
}

// 0 = unpinned, 1 = pinned (the sort key that lifts pinned chats up).
function chatPinScore(chat) {
  return chat.pinned ? 1 : 0;
}

/// Escape the text and highlight every case-insensitive match (WhatsApp Web
/// paints matches in the accent green, not a yellow marker).
function highlightMatch(text, needle) {
  if (!needle) return escapeHtml(text);
  const lower = text.toLowerCase();
  let html = "";
  let from = 0;
  while (from < text.length) {
    const at = lower.indexOf(needle, from);
    if (at < 0) break;
    html += escapeHtml(text.slice(from, at));
    html += `<mark class="search-hit">${escapeHtml(text.slice(at, at + needle.length))}</mark>`;
    from = at + needle.length;
  }
  html += escapeHtml(text.slice(from));
  return html;
}

/// WhatsApp Web groups chat search into "Chats", "Contacts" and "Groups in
/// common": chat names, saved contact names, and groups where a matching
/// contact recently spoke. Message bodies are not listed here.
function searchSections(needle) {
  const chats = [];
  const contacts = [];
  const groups = [];
  const shown = new Set();

  for (const chat of state.chats.values()) {
    if (isChannelJid(chat.jid)) continue;
    if (!WAChatUtils.isChatVisible(chat, state.showDeleted)) continue;
    if (state.filter === "unread" && chat.unread === 0) continue;
    if (state.filter === "groups" && !chat.isGroup) continue;

    const label = chatDisplayName(chat);
    if (chat.name.toLowerCase().includes(needle) || label.toLowerCase().includes(needle)) {
      chats.push({ chat, label });
      shown.add(chat.jid);
      continue;
    }
    if (!chat.isGroup) continue;
    const member = [...(chat.messages || [])]
      .reverse()
      .find(
        (message) =>
          WAChatUtils.isMessageVisible(message, state.showDeleted) &&
          (message.sender_name || "").toLowerCase().includes(needle),
      );
    if (member) groups.push({ chat, member: member.sender_name });
  }

  const selfPhone = WAChatUtils.phoneFromJid(state.meJid || "");
  for (const [key, name] of state.contacts) {
    if (!name || !/^\d+$/.test(key) || key.length < 8 || key.length > 13) continue;
    if (key === selfPhone) continue;
    if (!name.toLowerCase().includes(needle)) continue;
    const jid = `${key}@s.whatsapp.net`;
    if (shown.has(jid)) continue;
    contacts.push({ jid, name });
  }

  chats.sort((a, b) => (b.chat.last?.timestamp || 0) - (a.chat.last?.timestamp || 0));
  contacts.sort((a, b) => a.name.localeCompare(b.name));
  groups.sort((a, b) => (b.chat.last?.timestamp || 0) - (a.chat.last?.timestamp || 0));
  return { chats, contacts, groups };
}

function chatRowHtml(chat, needle) {
  const last = WAChatUtils.lastVisibleMessage(chat, state.showDeleted);
  const stamp = last ? last.timestamp : chat.lastTs || 0;
  // A mention in the preview shows the display name, and highlightMatch escapes
  // the string itself — escaping here too turned "&" into "&amp;".
  const preview = last
    ? previewOf(last, chat.isGroup, chat.name, state.showDeleted)
    : "Tap to open";
  // Own direct-chat messages get the official delivery tick before the
  // preview: grey until a receipt arrives, blue double tick when read.
  const tick = WAChatUtils.previewTick(last, chat.isGroup);
  // Our own chat only ever holds our own messages, so it is never unread.
  const unread = isSelfChat(chat.jid) ? 0 : chat.unread;
  const selecting = Boolean(state.selectingChats);
  const checked = selecting && state.selectedChats.has(chat.jid);
  return `
        <li class="chat-item ${chat.jid === state.active ? "active" : ""} ${unread > 0 ? "unread" : ""} ${chat.deleted ? "deleted" : ""} ${selecting ? "selectable" : ""} ${checked ? "selected" : ""}" data-jid="${escapeHtml(chat.jid)}">
          ${selecting ? `<span class="chat-check">${checked ? waIcon("check") : ""}</span>` : ""}
          ${avatarHtml(chat.name, chat.jid)}
          <div class="chat-meta">
            <div class="chat-top">
              <strong>${highlightMatch(chatDisplayName(chat), needle)}</strong>
              ${WAChatUtils.chatIsMuted(chat) ? '<span class="chat-muteglyph" title="Muted"><svg viewBox="0 -960 960 960" aria-hidden="true"><path d="M160-200v-80h80v-280q0-33 8.5-65t25.5-61l60 60q-7 16-10.5 32.5T320-560v280h248L56-792l56-56 736 736-56 56-146-144H160Zm560-154-80-80v-126q0-66-47-113t-113-47q-26 0-50 8t-44 24l-58-58q20-16 43-28t49-18v-28q0-25 17.5-42.5T480-880q25 0 42.5 17.5T540-820v28q80 20 130 84.5T720-560v206Zm-276-50Zm36 324q-33 0-56.5-23.5T400-160h160q0 33-23.5 56.5T480-80Zm33-481Z"/></svg></span>' : ""}
              ${chat.pinned ? '<span class="chat-pin" title="Pinned"><svg viewBox="0 -960 960 960" aria-hidden="true"><path d="m640-480 80 80v80H520v240l-40 40-40-40v-240H240v-80l80-80v-280h-40v-80h400v80h-40v280Zm-286 80h252l-46-46v-314H400v314l-46 46Zm126 0Z"/></svg></span>' : ""}
              <time>${stampOf(stamp)}</time>
            </div>
            <div class="chat-preview"><span class="preview-line">${tick}<span class="chat-preview-text">${highlightMatch(preview, needle)}</span></span>
              ${unread > 0 ? `<span class="badge">${unread}</span>` : ""}
            </div>
          </div>
        </li>`;
}

function renderChatList() {
  const list = $("chat-list");
  const needle = (state.search || "").toLowerCase();
  const chats = chatList();
  const searching = Boolean(needle);

  const archived = archivedChatCount();
  let banner = "";
  if (!searching && (archived > 0 || state.archivedOpen)) {
    banner = `
        <button type="button" class="archived-row" data-archived-toggle>
          <span class="archived-arrow">${waIcon(
            state.archivedOpen ? "expand_more" : "chevron_right",
          )}</span>
          Archived
          ${archived > 0 ? `<span class="archived-count">${archived}</span>` : ""}
        </button>`;
  }

  $("empty-chats").textContent = searching
    ? `No chats match "${state.search}".`
    : state.filter === "unread"
      ? "No unread chats."
      : state.filter === "groups"
        ? "No groups yet."
        : "No chats yet. Link your phone, or wait for a message to arrive.";

  if (searching) {
    const sections = searchSections(needle);
    const parts = [];
    if (sections.chats.length) {
      parts.push(`<div class="search-section">Chats</div>`);
      parts.push(sections.chats.slice(0, MAX_SEARCH_ROWS).map((entry) => chatRowHtml(entry.chat, needle)).join(""));
    }
    if (sections.contacts.length) {
      parts.push(`<div class="search-section">Contacts</div>`);
      parts.push(
        sections.contacts
          .slice(0, MAX_SEARCH_ROWS)
          .map(
            (entry) => `
        <li class="chat-item search-contact" data-picker-jid="${escapeHtml(entry.jid)}" data-picker-name="${escapeHtml(entry.name)}">
          ${avatarHtml(entry.name, entry.jid)}
          <div class="chat-meta">
            <div class="chat-top"><strong>${highlightMatch(entry.name, needle)}</strong></div>
            <div class="chat-preview"><span>${escapeHtml(WAChatUtils.formatPhone(WAChatUtils.phoneFromJid(entry.jid)))}</span></div>
          </div>
        </li>`,
          )
          .join(""),
      );
    }
    if (sections.groups.length) {
      parts.push(`<div class="search-section">Groups in common</div>`);
      parts.push(
        sections.groups
          .slice(0, MAX_SEARCH_ROWS)
          .map(
            (entry) => `
        <li class="chat-item" data-jid="${escapeHtml(entry.chat.jid)}">
          ${avatarHtml(entry.chat.name, entry.chat.jid)}
          <div class="chat-meta">
            <div class="chat-top"><strong>${highlightMatch(entry.chat.name, needle)}</strong></div>
            <div class="chat-preview"><span>${highlightMatch(entry.member, needle)} is also in this group</span></div>
          </div>
        </li>`,
          )
          .join(""),
      );
    }
    list.innerHTML = parts.join("");
    const empty = parts.length === 0;
    $("empty-chats").hidden = !empty;
    for (const entry of sections.chats.slice(0, 20)) queuePhoto(entry.chat.jid);
    for (const entry of sections.contacts.slice(0, 20)) queuePhoto(entry.jid);
    for (const entry of sections.groups.slice(0, 20)) queuePhoto(entry.chat.jid);
    scheduleSave();
    if (!$("channels-screen").hidden) renderChannels();
    updateRailBadges();
    return;
  }

  $("empty-chats").hidden = chats.length > 0;

  // Community parity: WhatsApp communities are parent groups whose child
  // groups nest beneath them, like the official chat list. The parent/child
  // link comes from group metadata, fetched lazily (paced, cached) — until a
  // group's metadata lands it renders as a plain row. The community's
  // announcement subgroup is hidden like official; its activity folds into
  // the community row.
  for (const chat of chats) queueGroupMeta(chat);
  const metaOf = (jid) => (state.groupMeta || {})[jid] || null;
  const isAnnounce = (jid) => {
    const meta = metaOf(jid);
    return Boolean(meta && meta.isAnnounce);
  };
  const communities = [];
  const byParent = new Map();
  const top = [];
  for (const chat of chats) {
    const meta = metaOf(chat.jid);
    if (meta && meta.isCommunity) {
      const unit = { chat, children: [], render: [] };
      communities.push(unit);
      byParent.set(chat.jid, unit);
      continue;
    }
  }
  for (const chat of chats) {
    const meta = metaOf(chat.jid);
    if (meta && meta.isCommunity) continue;
    const unit = meta && meta.parentJid ? byParent.get(meta.parentJid) : null;
    if (unit) {
      unit.children.push(chat);
      if (!meta.isAnnounce) unit.render.push(chat);
      continue;
    }
    if (!isAnnounce(chat.jid)) top.push(chat);
  }
  const nested = new Set();
  for (const unit of communities) for (const child of unit.children) nested.add(child.jid);
  const topFlat = top.filter((chat) => !nested.has(chat.jid));

  // The community row sits where its newest child is active, like official;
  // pinned chats stay pinned above everything.
  const units = topFlat.map((chat) => ({
    ts: chat.pinned ? Number.MAX_SAFE_INTEGER : chat.last?.timestamp || chat.lastTs || 0,
    chat,
  }));
  for (const unit of communities) {
    const childTs = Math.max(0, ...unit.children.map((ch) => ch.last?.timestamp || ch.lastTs || 0));
    units.push({
      ts: Math.max(unit.chat.last?.timestamp || unit.chat.lastTs || 0, childTs),
      community: unit,
    });
  }
  units.sort((a, b) => b.ts - a.ts);

  list.innerHTML =
    banner + units.map((u) => (u.community ? communityRowHtml(u.community) : chatRowHtml(u.chat, ""))).join("");

  // Only the first screenful gets a profile-picture request.
  for (const chat of chats.slice(0, 30)) queuePhoto(chat.jid);
  for (const unit of communities) for (const child of unit.children) queuePhoto(child.jid);

  scheduleSave();
  if (!$("channels-screen").hidden) renderChannels();
  updateRailBadges();
}

/* -------------------------------------------------- sidebar kebab menu */

// The official desktop's chat-list options, opened from the sidebar kebab.
function openSideMenu() {
  const menu = $("side-menu");
  const button = $("chat-side-menu");
  menu.hidden = false;
  const anchor = button.getBoundingClientRect();
  const rect = menu.getBoundingClientRect();
  // Right-align under the kebab, clamped inside the window like the other menus.
  const left = Math.max(8, Math.min(anchor.right - rect.width, window.innerWidth - rect.width - 8));
  const top = Math.max(8, Math.min(anchor.bottom + 6, window.innerHeight - rect.height - 8));
  menu.style.left = `${left}px`;
  menu.style.top = `${top}px`;
}

function closeSideMenu() {
  $("side-menu").hidden = true;
}

function toggleSideMenu() {
  if ($("side-menu").hidden) openSideMenu();
  else closeSideMenu();
}

// "Mark all as read": every unread chat gets the same app-state mutation the
// per-chat menu sends, so the phone and other clients follow along.
async function markAllChatsRead() {
  const unread = [...state.chats.values()].filter(
    (chat) => (chat.unread || 0) > 0 && !chat.deleted && !isSelfChat(chat.jid),
  );
  if (!unread.length) {
    toast("No unread chats");
    return;
  }
  let done = 0;
  for (const chat of unread) {
    try {
      await invoke("chat_read_mark", { chat: chat.jid, unread: false });
      chat.unread = 0;
      done += 1;
    } catch (err) {
      toast(String(err));
    }
  }
  renderChatList();
  scheduleSave();
  if (done) toast(done === 1 ? "1 chat marked as read" : `${done} chats marked as read`);
}

/* --------------------------------------------------- chat multi-select */

function startChatSelection() {
  state.selectingChats = true;
  state.selectedChats = new Set();
  $("chat-select-bar").hidden = false;
  $("chat-list").classList.add("chat-select-mode");
  updateChatSelectBar();
  renderChatList();
}

function exitChatSelection() {
  state.selectingChats = false;
  state.selectedChats = new Set();
  $("chat-select-bar").hidden = true;
  $("chat-list").classList.remove("chat-select-mode");
  renderChatList();
}

function toggleChatSelection(jid) {
  if (!state.selectingChats || !jid) return;
  const checked = !state.selectedChats.has(jid);
  if (checked) state.selectedChats.add(jid);
  else state.selectedChats.delete(jid);
  // Patch the one row instead of rebuilding the list, so the sidebar keeps its
  // scroll position while several chats are ticked.
  const node = document.querySelector(`#chat-list .chat-item[data-jid="${CSS.escape(jid)}"]`);
  if (node) {
    node.classList.toggle("selected", checked);
    const check = node.querySelector(".chat-check");
    if (check) check.innerHTML = checked ? waIcon("check") : "";
  }
  updateChatSelectBar();
}

function updateChatSelectBar() {
  const count = state.selectedChats.size;
  $("chat-select-count").textContent = count === 1 ? "1 selected" : `${count} selected`;
  $("chat-select-read").disabled = count === 0;
  $("chat-select-archive").disabled = count === 0;
  $("chat-select-delete").disabled = count === 0;
}

// Run one bulk action over the ticked chats. Each row rides the same per-chat
// command the right-click menu uses, so nothing new reaches the wire.
async function applyChatSelection(action) {
  const targets = [...state.selectedChats];
  if (!targets.length) return;
  if (action === "delete" && !confirm(`Delete ${targets.length === 1 ? "this chat" : `these ${targets.length} chats`} on all linked devices?`)) {
    return;
  }
  exitChatSelection();
  let done = 0;
  for (const jid of targets) {
    const chat = state.chats.get(jid);
    try {
      if (action === "read") {
        await invoke("chat_read_mark", { chat: jid, unread: false });
        if (chat) chat.unread = 0;
      } else if (action === "archive") {
        await invoke("chat_archive", { chat: jid, archived: true });
        if (chat) chat.archived = true;
      } else if (action === "delete") {
        await invoke("chat_delete", { chat: jid });
        if (chat) {
          chat.deleted = true;
          chat.deletedAt = Date.now();
        }
      }
      done += 1;
    } catch (err) {
      toast(String(err));
    }
  }
  if (action === "delete" && state.active && targets.includes(state.active)) {
    state.active = null;
    renderChat();
  }
  renderChatList();
  scheduleSave();
  if (!done) return;
  const label = action === "read" ? "marked as read" : action === "archive" ? "archived" : "deleted";
  toast(done === 1 ? `Chat ${label}` : `${done} chats ${label}`);
}

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
  const rows = starredEntries();
  $("starred-body").innerHTML = rows.length
    ? rows
        .map(({ chat, message }) => {
          const preview = WAChatUtils.previewOf(message, chat.isGroup, chat.name, state.showDeleted);
          return `
        <button type="button" class="starred-row" data-starred-jid="${escapeHtml(chat.jid)}" data-starred-id="${escapeHtml(message.id)}">
          <div class="starred-top">
            <strong>${escapeHtml(chatDisplayName(chat))}</strong>
            <time>${stampOf(message.timestamp)}</time>
          </div>
          <div class="starred-text"><span class="starred-icon">${waIcon("star")}</span>${escapeHtml(preview)}</div>
        </button>`;
        })
        .join("")
    : '<p class="empty">No starred messages yet. Right-click a message and pick Star message.</p>';
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

// The community header row plus its nested child groups. The header shows no
// timestamp (like official); the announcement subgroup never renders — its
// activity still drives the header's sort position.
function communityRowHtml(unit) {
  const label = unit.render.length
    ? `Community · ${unit.render.length} group${unit.render.length === 1 ? "" : "s"}`
    : "Community";
  // The row's stamp is the community's newest activity (children and the
  // announcement group included), like official.
  const ts = Math.max(0, ...unit.children.map((ch) => ch.last?.timestamp || ch.lastTs || 0));
  return `
        <li class="chat-item community" data-jid="${escapeHtml(unit.chat.jid)}">
          ${avatarHtml(unit.chat.name, unit.chat.jid)}
          <div class="chat-meta">
            <div class="chat-top"><strong>${escapeHtml(chatDisplayName(unit.chat))}</strong>${
              ts ? `<time>${stampOf(ts)}</time>` : ""
            }</div>
            <div class="chat-preview"><span class="preview-line"><span class="chat-preview-text">${escapeHtml(label)}</span></span></div>
          </div>
        </li>${unit.render
          .map((child) => chatRowHtml(child, "").replace('class="chat-item ', 'class="chat-item nested '))
          .join("")}`;
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
  $("community-title").textContent = chatDisplayName(chat);

  // Children and the announcement subgroup, from the cached metadata.
  const children = [];
  let announce = null;
  for (const [, other] of state.chats) {
    if (other.jid === jid) continue;
    const meta = (state.groupMeta || {})[other.jid];
    if (!meta || meta.parentJid !== jid) continue;
    if (meta.isAnnounce) announce = other;
    else children.push(other);
  }
  children.sort(
    (a, b) => (b.last?.timestamp || b.lastTs || 0) - (a.last?.timestamp || a.lastTs || 0),
  );

  const row = (child) => chatRowHtml(child, "");
  // Official labels the announcement subgroup "Announcements" regardless of
  // its (community-named) subject.
  const announceRow = announce
    ? row(announce)
        .replace('class="chat-item ', 'class="chat-item community-announce ')
        .replace(/<strong>[\s\S]*?<\/strong>/, "<strong>Announcements</strong>")
    : "";

  $("community-body").innerHTML = `
        ${announceRow}
        <div class="community-section">Groups you're in</div>
        <button type="button" class="chat-item community-add" data-community-add>
          <span class="community-add-badge"><svg viewBox="0 -960 960 960" aria-hidden="true"><path d="M440-440H200v-80h240v-240h80v240h240v80H520v240h-80v-240Z"/></svg></span>
          <div class="chat-meta"><div class="chat-top"><strong>Add group</strong></div></div>
        </button>
        ${children.map(row).join("")}
        ${children.length ? "" : '<p class="empty">No groups in this community yet.</p>'}
  `;
}

/* -------------------------------------------------------------- channels */

// WhatsApp channels are newsletters: broadcast-only chats on @newsletter.
function isChannelJid(jid) {
  return typeof jid === "string" && jid.endsWith("@newsletter");
}

// 7_100_000 -> "7.1m followers", 84_300 -> "84.3k followers".
function formatFollowers(count) {
  const n = Number(count) || 0;
  if (n <= 0) return "Channel";
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1).replace(/\.0$/, "")}m followers`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1).replace(/\.0$/, "")}k followers`;
  return `${n} follower${n === 1 ? "" : "s"}`;
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

function channelChats() {
  return [...state.chats.values()]
    .filter(
      (chat) =>
        isChannelJid(chat.jid) && WAChatUtils.isChatVisible(chat, state.showDeleted),
    )
    .sort((a, b) => (b.last?.timestamp || b.lastTs || 0) - (a.last?.timestamp || a.lastTs || 0));
}

function renderChannels() {
  const list = $("channels-list");
  if (!list) return;
  const needle = ($("channels-search")?.value || "").trim().toLowerCase();
  const channels = channelChats().filter(
    (chat) => !needle || String(chat.name || "").toLowerCase().includes(needle),
  );
  $("channels-empty").hidden = channels.length > 0;
  list.innerHTML = channels
    .map((chat) => {
      const last = WAChatUtils.lastVisibleMessage(chat, state.showDeleted);
      const stamp = last ? last.timestamp : chat.lastTs || 0;
      // WhatsApp marks media posts with a small glyph in the preview line.
      const kind = last && last.kind !== "text" ? last.kind : null;
      const icon = kind ? `<span class="channel-kind">${mediaIcon(kind)}</span>` : "";
      return `
        <button type="button" class="channel-row ${chat.jid === state.active ? "active" : ""}" data-jid="${escapeHtml(chat.jid)}">
          ${avatarHtml(chat.name, chat.jid)}
          <span class="channel-meta">
            <span class="channel-top">
              <strong>${escapeHtml(chat.name)}</strong>
              <time>${stampOf(stamp)}</time>
            </span>
            <span class="channel-preview">${last ? `${icon}${escapeHtml(previewOf(last, false, chat.name, state.showDeleted))}` : "Channel"}</span>
          </span>
          ${chat.unread > 0 ? `<span class="badge">${chat.unread}</span>` : ""}
        </button>`;
    })
    .join("");
  for (const chat of channels.slice(0, 20)) queuePhoto(chat.jid);
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

function isEmojiOnly(text) {
  const trimmed = text.trim();
  if (!trimmed || trimmed.length > 12) return false;
  if (/[\p{L}\p{N}]/u.test(trimmed)) return false;
  return /[\p{Extended_Pictographic}\u200d\uFE0F]/u.test(trimmed);
}

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
