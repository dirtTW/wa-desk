/* WA Desk frontend: message-actions. Loaded in order by index.html; all files share the classic-script global scope. */

/* ---------------------------------------------------- message actions */

let menuMessage = null;
let pressTimer = null;
let pressBubble = null;

function bubbleMessage(node) {
  const bubble = node && node.closest ? node.closest(".bubble") : null;
  if (!bubble || !state.active) return null;
  const entry = state.chats.get(state.active);
  if (!entry) return null;
  return entry.messages.find((message) => message.id === bubble.dataset.id) || null;
}

function cancelPress() {
  clearTimeout(pressTimer);
  pressTimer = null;
  if (pressBubble) pressBubble.classList.remove("press");
  pressBubble = null;
}

async function openMessageMenu(message, x, y) {
  cancelPress();
  menuMessage = message;

  const menu = $("msg-menu");
  const chat = state.chats.get(message.chat);
  let rows;
  try {
    rows = await invoke("ui_message_menu", {
      input: {
        from_me: Boolean(message.from_me),
        kind: message.kind || "text",
        deleted: Boolean(message.deleted),
        deleted_for_me: Boolean(message.deletedForMe),
        text: String(message.text || ""),
        sender_label:
          contactNameFor(message.sender) || message.sender_name || "sender",
        chat_is_group: Boolean(chat && chat.isGroup),
        has_sender: Boolean(message.sender),
        starred: state.starred.has(message.id),
      },
    });
  } catch (err) {
    console.warn("message menu state failed", err);
    return;
  }
  if (menuMessage !== message) return;

  menu.querySelector('[data-action="edit"]').hidden = !rows.edit;
  menu.querySelector('[data-action="forward"]').hidden = !rows.forward;
  menu.querySelector('[data-action="reply"]').hidden = !rows.reply;
  menu.querySelector('[data-action="copy"]').hidden = !rows.copy;
  menu.querySelector('[data-action="message-sender"]').hidden = !rows.message_sender;
  menu.querySelector('[data-action="reply-privately"]').hidden = !rows.reply_privately;
  menu.querySelector('[data-action="message-info"]').hidden = !rows.message_info;
  // Starring mirrors the app-state `star` collection, so the row flips between
  // Star/Unstar like the official message menu.
  menu.querySelector('[data-action="star"]').hidden = !rows.star;
  if (rows.star) $("msg-star-label").textContent = rows.star_label;
  if (rows.message_sender) $("message-sender-label").textContent = rows.sender_label;
  menu.hidden = false;

  const rect = menu.getBoundingClientRect();
  const left = Math.max(8, Math.min(x, window.innerWidth - rect.width - 10));
  const top = Math.max(8, Math.min(y, window.innerHeight - rect.height - 10));
  menu.style.left = `${left}px`;
  menu.style.top = `${top}px`;
  clampMessageMenu();
}

function closeMessageMenu() {
  $("msg-menu").hidden = true;
  menuMessage = null;
}

// Quote a message in the composer. WhatsApp shows the author and a one-line
// preview in an inset card ABOVE the input while the reply is pending, with the
// quoted author's colour on the left bar and the name.
async function startReply(message) {
  state.reply = message;
  // Author, preview and participant colour are computed by wa-ui::actions.
  const preview = await invoke("ui_reply_preview", {
    message: {
      from_me: Boolean(message.from_me),
      kind: message.kind || "text",
      text: String(message.text || ""),
      sender: message.sender || "",
      sender_name: message.sender_name || "",
      contact_name: contactNameFor(message.sender) || "",
    },
  });
  const banner = $("reply-banner");
  banner.style.setProperty("--reply-colour", preview.colour);
  $("reply-banner-name").textContent = preview.name;
  $("reply-banner-text").textContent = preview.preview;
  banner.hidden = false;
  const input = $("input");
  if (input) input.focus();
}

function cancelReply() {
  state.reply = null;
  $("reply-banner").hidden = true;
}

async function copyMessageText(message) {
  const text = String(message.text || "");
  if (!text.trim()) return;
  try {
    await navigator.clipboard.writeText(text);
    toast("Copied to clipboard");
  } catch {
    toast("Could not copy the message");
  }
}

/* ------------------------------------------------------- multi-select mode */

function startSelection(message) {
  state.selecting = true;
  state.selected = new Set();
  if (message && message.id) state.selected.add(message.id);
  state.forwardQueue = null;
  $("select-bar").hidden = false;
  $("messages").classList.add("select-mode");
  updateSelectBar();
  renderChat();
}

function exitSelection() {
  state.selecting = false;
  state.selected = new Set();
  state.forwardQueue = null;
  $("select-bar").hidden = true;
  $("messages").classList.remove("select-mode");
  renderChat();
}

function toggleSelection(id) {
  if (!state.selecting || !id) return;
  if (state.selected.has(id)) state.selected.delete(id);
  else state.selected.add(id);
  const node = document.querySelector(`#messages [data-id="${CSS.escape(id)}"]`);
  if (node) node.classList.toggle("selected", state.selected.has(id));
  if (state.selected.size === 0) {
    exitSelection();
    return;
  }
  updateSelectBar();
}

async function updateSelectBar() {
  const bar = await invoke("ui_selection_bar", { count: state.selected.size });
  $("select-count").textContent = bar.label;
  $("select-forward").disabled = bar.disabled;
  $("select-delete").disabled = bar.disabled;
}

/* ------------------------------------------------- delete confirmation */

async function askDelete(items) {
  const list = (items || []).filter(Boolean);
  if (!list.length) return;
  state.deletePending = { items: list };
  let rows;
  try {
    rows = await invoke("ui_delete_confirm", {
      items: list.map((item) => ({
        from_me: Boolean(item.fromMe),
        deleted: Boolean(item.deleted),
        chat: item.chat || "",
      })),
      self_chat: isSelfChat(list[0].chat),
    });
  } catch (err) {
    console.warn("delete confirm failed", err);
    return;
  }
  $("delete-confirm-title").textContent = rows.title;
  $("delete-everyone-ok").hidden = !rows.can_everyone;
  $("delete-confirm").hidden = false;
}

function closeDeleteConfirm() {
  state.deletePending = null;
  $("delete-confirm").hidden = true;
}

async function confirmDelete(everyone) {
  const pending = state.deletePending;
  if (!pending) return;
  closeDeleteConfirm();
  // Mark the bubbles first, exactly like WhatsApp Web: the stanza/app-state
  // mutation goes out in the background and the UI never waits for its ack.
  for (const item of pending.items) {
    const entry = state.chats.get(item.chat);
    const message = entry && entry.messages.find((candidate) => candidate.id === item.id);
    if (message) {
      message.deleted = true;
      if (!everyone) message.deletedForMe = true;
    }
  }
  if (state.selecting) exitSelection();
  renderChatList();
  renderChat();
  scheduleSave();

  // The per-item command and args come from wa_ui::chat_actions.
  let commands = [];
  try {
    commands = await invoke("ui_delete_commands", {
      items: pending.items.map((item) => ({
        chat: item.chat,
        id: item.id,
        from_me: Boolean(item.fromMe),
        participant: item.participant || null,
      })),
      everyone,
    });
  } catch (err) {
    toast(String(err));
    return;
  }
  for (const command of commands) {
    const args = { chat: command.chat, id: command.id };
    if (command.from_me !== null && command.from_me !== undefined) {
      args.fromMe = command.from_me;
      args.participant = command.participant || null;
    }
    invoke(command.command, args).catch((err) => toast(String(err)));
  }
}

function selectedMessages() {
  const chat = state.chats.get(state.active);
  if (!chat) return [];
  return [...state.selected]
    .map((id) => chat.messages.find((message) => message.id === id))
    .filter(Boolean);
}

function forwardSelection() {
  const list = selectedMessages();
  if (!list.length) return;
  state.forwardQueue = list.map((message) => ({
    id: message.id,
    text: message.text || "",
  }));
  openForwardPicker(list[0]);
}

async function deleteSelection() {
  const list = selectedMessages();
  if (!list.length) return;
  askDelete(
    list.map((message) => ({
      chat: message.chat,
      id: message.id,
      fromMe: message.from_me,
      participant: message.sender,
      deleted: message.deleted,
    })),
  );
}

// Scroll to the message a reply quotes and flash it, like WhatsApp Web.
function jumpToQuoted(id) {
  if (!id) return;
  const node = document.querySelector(
    `#messages [data-id="${CSS.escape(String(id))}"]`,
  );
  if (!node) {
    toast("That message isn't loaded in this chat yet.");
    return;
  }
  node.scrollIntoView({ block: "center", behavior: "smooth" });
  node.classList.remove("quote-target");
  // Force a reflow so a repeated jump restarts the highlight animation.
  void node.offsetWidth;
  node.classList.add("quote-target");
  setTimeout(() => node.classList.remove("quote-target"), 1600);
}

// Keep the open menu inside the window: `#msg-menu` is capped at the viewport
// height, so it scrolls instead of overflowing off the bottom edge.
function clampMessageMenu() {
  const menu = $("msg-menu");
  if (!menu || menu.hidden) return;
  const rect = menu.getBoundingClientRect();
  const left = Math.max(8, Math.min(rect.left, window.innerWidth - rect.width - 8));
  const top = Math.max(8, Math.min(rect.top, window.innerHeight - rect.height - 8));
  menu.style.left = `${left}px`;
  menu.style.top = `${top}px`;
}

async function showMessageInfo(message) {
  // Labels, values and rows are built by wa_ui::chat_actions.
  const info = await invoke("ui_message_info", {
    input: {
      kind: message.kind || "text",
      timestamp: message.timestamp || 0,
      from_me: Boolean(message.from_me),
      sender_name: message.sender_name || "",
      sender: message.sender || "",
      id: String(message.id || ""),
      pending: Boolean(message.pending),
      failed: Boolean(message.failed),
      status: message.status || "",
    },
  }).catch(() => null);
  if (!info) return;
  $("msg-info-body").innerHTML = info.html;
  $("msg-info").hidden = false;
}

function startEdit(message) {  state.editing = { chat: message.chat, id: message.id };
  const input = $("input");
  input.value = message.text || "";
  autosize(input);
  updateSendState();
  $("edit-banner").hidden = false;
  input.focus();
  input.setSelectionRange(input.value.length, input.value.length);
}

function cancelEdit() {
  state.editing = null;
  $("edit-banner").hidden = true;
  $("input").value = "";
  autosize($("input"));
  updateSendState();
}

/* ------------------------------------------------------------ starring */

// Starring rides the app-state `star` collection (the phone and every linked
// client show the same list); the local set is applied optimistically so the
// menu flips immediately.
async function toggleStar(message) {
  const starred = !state.starred.has(message.id);
  try {
    await invoke("chat_star", {
      chat: message.chat,
      id: message.id,
      starred,
      fromMe: Boolean(message.from_me),
      // The wire needs the author for someone else's group message.
      participant: message.from_me ? null : message.sender || null,
    });
    applyStar(message.chat, message.id, starred);
    toast(starred ? "Message starred" : "Message unstarred");
  } catch (err) {
    toast(String(err));
  }
}

// Shared by the command's own update and the incoming `star_message` event (a
// full app-state sync replays every star, rebuilding the list on each start).
function applyStar(chatJid, id, starred) {
  if (!id) return;
  if (starred) state.starred.add(id);
  else state.starred.delete(id);
  const chat = state.chats.get(chatJid);
  const message = chat && chat.messages.find((entry) => entry.id === id);
  if (message) message.starred = starred;
  if (state.starredOpen) renderStarredScreen();
  if (state.active === chatJid) renderChat();
  scheduleSave();
}

async function applyMessageAction(action, message) {
  if (!message) return;

  if (action === "edit") {
    startEdit(message);
    return;
  }

  if (action === "forward") {
    closeMessageMenu();
    openForwardPicker(message);
    return;
  }

  if (action === "reply") {
    startReply(message);
    return;
  }

  if (action === "select") {
    startSelection(message);
    return;
  }

  if (action === "star") {
    toggleStar(message);
    return;
  }

  if (action === "ask-ai" || action === "report") {
    toast(
      action === "report"
        ? "Reporting isn't supported by WA Desk yet."
        : "Ask Meta AI isn't available in WA Desk yet.",
    );
    return;
  }

  if (action === "message-info") {
    showMessageInfo(message);
    return;
  }

  if (action === "reply-privately") {
    if (!message.sender) return;
    openChatFromPicker(message.sender, message.sender_name || "");
    startReply(message);
    return;
  }

  if (action === "copy") {
    copyMessageText(message);
    return;
  }

  if (action === "message-sender") {
    openChatFromPicker(message.sender, message.sender_name || "");
    return;
  }

  if (action === "delete") {
    askDelete([
      {
        chat: message.chat,
        id: message.id,
        fromMe: message.from_me,
        participant: message.sender,
        deleted: message.deleted,
      },
    ]);
  }
}

