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

function pickerRequest() {
  return {
    contacts: Object.fromEntries(state.contacts),
    chats: [...state.chats.values()].map((chat) => ({
      jid: chat.jid,
      name: chat.name || "",
      is_group: Boolean(chat.isGroup),
    })),
    phones: Object.fromEntries(state.phones),
    known_lids: [...knownLids()],
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
    out = await invoke("ui_picker", { request: pickerRequest() });
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

