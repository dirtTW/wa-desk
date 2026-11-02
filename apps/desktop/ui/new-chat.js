/* WA Desk frontend: new-chat. Loaded in order by index.html; all files share the classic-script global scope. */

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
      entry = {
        jid,
        name:
          state.contacts.get(WAChatUtils.phoneFromJid(jid)) ||
          WAChatUtils.formatPhone(WAChatUtils.phoneFromJid(jid)) ||
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
    toast(sent === 1 ? "Message forwarded" : `${sent} messages forwarded`);
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

function knownLids() {
  const lids = new Set();
  for (const chat of state.chats.values()) {
    if (chat.jid.endsWith("@lid")) lids.add(WAChatUtils.phoneFromJid(chat.jid));
    for (const message of chat.messages.slice(-40)) {
      if (message.sender && String(message.sender).endsWith("@lid")) {
        lids.add(WAChatUtils.phoneFromJid(message.sender));
      }
    }
  }
  return lids;
}

function newChatEntries() {
  const entries = [];
  const seen = new Set();
  const selfJid = state.meLid || state.meJid;
  if (selfJid) {
    entries.push({ jid: selfJid, name: "You", subtitle: "Message yourself", self: true });
    seen.add(selfJid);
  }

  const lids = knownLids();
  const byName = new Map();
  const byPhone = new Map();
  for (const chat of state.chats.values()) {
    if (chat.isGroup) continue;
    byName.set(chat.name.trim().toLowerCase(), chat.jid);
    byPhone.set(WAChatUtils.phoneFromJid(chat.jid), chat.jid);
    const resolved = state.phones.get(chat.jid);
    if (typeof resolved === "string" && resolved.length > 5) byPhone.set(resolved, chat.jid);
  }

  for (const [key, name] of state.contacts) {
    if (!name || !/^\d+$/.test(key) || key.length < 8 || key.length > 13) continue;
    if (lids.has(key) || key === WAChatUtils.phoneFromJid(state.meJid || "")) continue;
    // Reuse the existing conversation (it may be keyed by LID) instead of
    // creating an empty PN duplicate.
    const jid =
      byName.get(name.trim().toLowerCase()) || byPhone.get(key) || `${key}@s.whatsapp.net`;
    if (seen.has(jid)) continue;
    entries.push({ jid, name, subtitle: WAChatUtils.formatPhone(key) });
    seen.add(jid);
  }

  for (const chat of state.chats.values()) {
    // The forward picker targets groups too, like WhatsApp's; "new chat" only
    // offers people (a group is created from the action row instead).
    if (chat.isGroup) {
      if (!forwardMessage || seen.has(chat.jid)) continue;
      entries.push({ jid: chat.jid, name: chat.name, subtitle: "Group" });
      seen.add(chat.jid);
      continue;
    }
    if (chat.jid.endsWith("@lid") || seen.has(chat.jid)) continue;
    entries.push({
      jid: chat.jid,
      name: chat.name,
      subtitle: WAChatUtils.formatPhone(WAChatUtils.phoneFromJid(chat.jid)),
    });
    seen.add(chat.jid);
  }

  return entries;
}

function renderNewChat() {
  const query = $("new-chat-input").value.trim();
  const needle = query.toLowerCase();
  const digits = query.replace(/\D/g, "");
  const actions = document.querySelector(".new-chat-actions");
  actions.hidden = Boolean(needle) || Boolean(forwardMessage);

  const entries = newChatEntries()
    .filter((entry) => {
      if (!needle) return true;
      if (entry.name.toLowerCase().includes(needle)) return true;
      return digits.length >= 3 && entry.subtitle.replace(/\D/g, "").includes(digits);
    })
    .sort((a, b) => {
      if (a.self) return -1;
      if (b.self) return 1;
      return a.name.localeCompare(b.name);
    });

  const rows = [];
  if (needle && digits.length >= 7 && /^[+\d][\d\s().-]*$/.test(query)) {
    const jid = `${digits}@s.whatsapp.net`;
    if (!entries.some((entry) => entry.jid === jid)) {
      const label = WAChatUtils.formatPhone(digits);
      rows.push(`<button type="button" class="new-chat-row${
        forwardMessage ? " forward-row" : ""
      }${
        forwardMessage && forwardTargets.has(jid) ? " selected" : ""
      }" ${
        forwardMessage
          ? `data-forward-jid="${escapeHtml(jid)}"`
          : `data-chat-jid="${escapeHtml(jid)}" data-chat-name="${escapeHtml(label)}"`
      }>
        ${forwardMessage ? '<span class="forward-tick" aria-hidden="true"></span>' : ""}
        ${avatarHtml(label, jid)}
        <div class="new-chat-meta"><strong>${escapeHtml(label)}</strong><span>Start a chat with this number</span></div>
      </button>`);
    }
  }

  let lastLetter = "";
  for (const entry of entries) {
    const letter = entry.self
      ? ""
      : /^[a-z]/i.test(entry.name)
        ? entry.name[0].toUpperCase()
        : "#";
    if (letter && letter !== lastLetter) {
      rows.push(`<div class="new-chat-letter">${escapeHtml(letter)}</div>`);
      lastLetter = letter;
    }
    rows.push(`<button type="button" class="new-chat-row${
      forwardMessage ? " forward-row" : ""
    }${
      forwardMessage && forwardTargets.has(entry.jid) ? " selected" : ""
    }" ${
      forwardMessage
        ? `data-forward-jid="${escapeHtml(entry.jid)}"`
        : `data-chat-jid="${escapeHtml(entry.jid)}" data-chat-name="${escapeHtml(entry.name)}"`
    }>
      ${forwardMessage ? '<span class="forward-tick" aria-hidden="true"></span>' : ""}
      ${avatarHtml(entry.name, entry.jid)}
      <div class="new-chat-meta"><strong>${escapeHtml(entry.name)}</strong><span>${escapeHtml(entry.subtitle)}</span></div>
    </button>`);
  }

  const list = $("new-chat-list");
  list.innerHTML = rows.join("") || '<p class="new-chat-empty">No contacts match that search.</p>';
  for (const entry of entries.slice(0, 40)) queuePhoto(entry.jid);
}

function openChatFromPicker(jid, name) {
  if (!jid) return;
  if (!state.chats.has(jid)) {
    state.chats.set(jid, {
      jid,
      name: name || state.contacts.get(WAChatUtils.phoneFromJid(jid)) || WAChatUtils.phoneFromJid(jid),
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

