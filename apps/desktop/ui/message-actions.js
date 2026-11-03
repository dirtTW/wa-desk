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
  const pos = await invoke("ui_clamp_popover", {
    x,
    y,
    width: rect.width,
    height: rect.height,
    viewportW: window.innerWidth,
    viewportH: window.innerHeight,
    margin: 10,
  }).catch(() => ({ left: x, top: y }));
  menu.style.left = `${pos.left}px`;
  menu.style.top = `${pos.top}px`;
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
  // Blank messages have nothing to copy (wa_ui::chat_actions::copy_text).
  const text = await invoke("ui_copy_text", { text: String(message.text || "") }).catch(
    () => null,
  );
  if (!text) return;
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

async function toggleSelection(id) {
  if (!state.selecting || !id) return;
  // Add/remove, the checked state and the "emptied -> leave select mode" rule
  // are decided by wa_ui::actions::toggle_selection.
  const plan = await invoke("ui_toggle_selection", {
    selected: [...state.selected],
    id,
  }).catch(() => null);
  if (!plan) return;
  state.selected = new Set(plan.selected);
  const node = document.querySelector(`#messages [data-id="${CSS.escape(id)}"]`);
  if (node) node.classList.toggle("selected", plan.checked);
  if (plan.exit) {
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
      selfChat: isSelfChat(list[0].chat),
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

async function forwardSelection() {
  const list = selectedMessages();
  if (!list.length) return;
  // Which ids/text a forward carries is wa_ui::chat_actions::forward_payload.
  state.forwardQueue = await invoke("ui_forward_payload", {
    items: list.map(selectionRow),
  }).catch(() => []);
  if (!state.forwardQueue.length) return;
  openForwardPicker(list[0]);
}

/// One selected message reduced to the fields the Rust plans read.
function selectionRow(message) {
  return {
    chat: message.chat || "",
    id: message.id || "",
    from_me: Boolean(message.from_me),
    sender: message.sender || "",
    deleted: Boolean(message.deleted),
    deleted_for_me: Boolean(message.deletedForMe),
    text: message.text || "",
  };
}

async function deleteSelection() {
  const list = selectedMessages();
  if (!list.length) return;
  // wa_ui::chat_actions::delete_targets drops id-less rows and our own author.
  const targets = await invoke("ui_delete_targets", {
    items: list.map(selectionRow),
  }).catch(() => []);
  if (!targets.length) return;
  askDelete(
    targets.map((target) => ({
      chat: target.chat,
      id: target.id,
      fromMe: target.from_me,
      participant: target.participant,
      deleted: false,
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
  // The toggle direction, the participant rule and the toast are
  // wa_ui::chat_actions::star_plan.
  const plan = await invoke("ui_star_plan", {
    id: String(message.id || ""),
    starredNow: state.starred.has(message.id),
    fromMe: Boolean(message.from_me),
    sender: message.sender || "",
  }).catch(() => null);
  if (!plan || !plan.allowed) return;
  try {
    await invoke("chat_star", {
      chat: message.chat,
      id: message.id,
      starred: plan.starred,
      fromMe: Boolean(message.from_me),
      // The wire needs the author for someone else's group message.
      participant: plan.participant,
    });
    applyStar(message.chat, message.id, plan.starred);
    toast(plan.toast);
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

// The menu's dispatch table (including the unsupported rows and the rows that
// need an author) is wa_ui::chat_actions::message_action.
async function applyMessageAction(action, message) {
  if (!message) return;
  const plan = await invoke("ui_message_action", {
    action,
    hasSender: Boolean(message.sender),
  }).catch(() => ({ kind: "none", toast: null, close_menu: false }));
  if (plan.close_menu) closeMessageMenu();
  if (plan.toast) {
    toast(plan.toast);
    return;
  }

  switch (plan.kind) {
    case "edit":
      startEdit(message);
      return;
    case "forward":
      openForwardPicker(message);
      return;
    case "reply":
      startReply(message);
      return;
    case "select":
      startSelection(message);
      return;
    case "star":
      toggleStar(message);
      return;
    case "info":
      showMessageInfo(message);
      return;
    case "copy":
      copyMessageText(message);
      return;
    case "message_sender":
      openChatFromPicker(message.sender, message.sender_name || "");
      return;
    case "reply_privately":
      openChatFromPicker(message.sender, message.sender_name || "");
      startReply(message);
      return;
    case "delete":
      askDelete([
        {
          chat: message.chat,
          id: message.id,
          fromMe: message.from_me,
          participant: message.sender,
          deleted: message.deleted,
        },
      ]);
      return;
    default:
      return;
  }
}

