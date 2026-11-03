/* Chat option menu — the official desktop's right-click (and long-press):
   pin, archive, mute, mark unread, block, close, clear and delete all ride
   cross-client app-state/blocklist mutations, so the phone and every other
   desktop follow along. Extracted from wiring.js to keep that file focused on
   keyboard/global wiring. Loaded after chat-utils (needs WAChatUtils) and
   before wiring.js. */

function initChatMenu() {
  const chatListEl = $("chat-list");
  let chatMenuChat = null;

  function chatExists(jid) {
    return state.chats.has(jid);
  }

  // The mute state and the menu rows are decided by wa-ui::menus.
  function chatMenuInput(chat, jid) {
    return {
      jid: jid || chat?.jid || "",
      pinned: Boolean(chat?.pinned),
      mute_until: chat?.muteUntil || 0,
      mute_forever: Boolean(chat?.muteForever),
      now: Date.now(),
      archived: Boolean(chat?.archived),
      unread: chat?.unread || 0,
      is_group: Boolean(chat?.isGroup),
    };
  }

  // Define which rows the official menu shows for the selected chat. The rules
  // live in wa-ui::menus; this only toggles the static rows.
  async function applyLocalChatMenuState() {
    const chat = state.chats.get(chatMenuChat) || {};
    let rows;
    try {
      rows = await invoke("ui_chat_menu", { input: chatMenuInput(chat, chatMenuChat) });
    } catch (err) {
      console.warn("chat menu state failed", err);
      return;
    }
    const menu = $("chat-list-menu");
    for (const button of menu.querySelectorAll("button[data-chat-action]")) {
      const action = button.dataset.chatAction;
      button.hidden = rows[action] === false;
    }
  }

  function closeChatMenu() {
    $("chat-list-menu").hidden = true;
    $("chat-mute-sub").hidden = true;
  }

  // Unhide first so the metrics are real, then clamp inside the window like
  // the message menu does.
  function clampMenu(menu, clientX, clientY) {
    menu.style.left = "8px";
    menu.style.top = "8px";
    const rect = menu.getBoundingClientRect();
    const left = Math.max(8, Math.min(clientX, window.innerWidth - rect.width - 8));
    const top = Math.max(8, Math.min(clientY, window.innerHeight - rect.height - 8));
    menu.style.left = `${left}px`;
    menu.style.top = `${top}px`;
  }

  async function openChatMenu(chatJid, clientX, clientY) {
    if (!chatExists(chatJid)) return;
    chatMenuChat = chatJid;
    const menu = $("chat-list-menu");
    menu.hidden = false;
    await applyLocalChatMenuState();
    clampMenu(menu, clientX, clientY);
  }

  chatListEl.addEventListener("contextmenu", (event) => {
    const item = event.target.closest(".chat-item");
    const jid = item && item.dataset.jid;
    if (!jid) return;
    event.preventDefault();
    openChatMenu(jid, event.clientX, event.clientY);
  });

  // Touch long-press opens the same menu (official desktop parity).
  let chatPressTimer = null;
  chatListEl.addEventListener("pointerdown", (e) => {
    const item = e.target.closest(".chat-item");
    if (!item || !item.dataset.jid || e.pointerType === "mouse") return;
    clearTimeout(chatPressTimer);
    chatPressTimer = setTimeout(() => {
      openChatMenu(item.dataset.jid, e.clientX, e.clientY);
    }, 650);
  });
  for (const type of ["pointerup", "pointerleave", "pointermove", "pointercancel"]) {
    chatListEl.addEventListener(type, () => clearTimeout(chatPressTimer));
  }

  $("chat-list-menu").addEventListener("click", async (event) => {
    const button = event.target.closest("button[data-chat-action]");
    const chatJid = chatMenuChat;
    if (!button || !chatJid) return;
    const action = button.dataset.chatAction;
    if (action === "mute") {
      const submenu = $("chat-mute-sub");
      const rect = button.getBoundingClientRect();
      submenu.hidden = false;
      await applyMuteSubmenu(chatJid);
      clampMenu(submenu, rect.right + 4, rect.top);
      return;
    }
    closeChatMenu();
    if (button.dataset.disabled) {
      toast("Chat lists are not supported yet");
      return;
    }
    runChatAction(action, chatJid);
  });

  // Every action still mirrors the exact same events the official client
  // fires; the chat cache mutates right away so a slow app-state ack never
  // blocks the sidebar (or loses it after another render).
  async function runChatAction(action, chatJid) {
    const entry = state.chats.get(chatJid);
    try {
      switch (action) {
        case "pin":
        case "unpin": {
          const pinned = action === "pin";
          await invoke("chat_pin", { chat: chatJid, pinned });
          applyPin(entry, chatJid, pinned);
          toast(pinned ? "Chat pinned" : "Chat unpinned");
          return;
        }
        case "archive":
        case "unarchive": {
          const archived = action === "archive";
          await invoke("chat_archive", { chat: chatJid, archived });
          if (entry) {
            entry.archived = archived;
            renderChatList();
            scheduleSave();
          }
          toast(archived ? "Chat archived" : "Chat unarchived");
          return;
        }
        case "unread":
        case "read": {
          const unread = action === "unread";
          await invoke("chat_read_mark", { chat: chatJid, unread });
          if (entry) {
            entry.unread = unread ? Math.max(1, entry.unread || 0) : 0;
            renderChatList();
            scheduleSave();
          }
          if (unread) toast("Marked as unread");
          return;
        }
        case "block": {
          const blocked = Boolean(entry && entry.blocked);
          await invoke("chat_block", { chat: chatJid, blocked: !blocked });
          if (entry) {
            entry.blocked = !blocked;
            scheduleSave();
          }
          toast(blocked ? "Unblocked contact" : "Contact blocked");
          return;
        }
        case "close": {
          if (state.active === chatJid) {
            // The official desktop only closes the panel, it keeps the entry.
            state.active = null;
            renderChat();
          }
          return;
        }
        case "clear": {
          if (!confirm("Clear all messages in this chat?")) return;
          await invoke("chat_clear", { chat: chatJid });
          // The chat_cleared event echo discards the history; this renders
          // immediately, matching WhatsApp Web's snappy clear.
          if (entry) {
            entry.messages = [];
            entry.last = null;
            renderChatList();
            renderChat();
            scheduleSave();
          }
          toast("Chat cleared");
          return;
        }
        case "delete": {
          if (!confirm("Delete this chat on all linked devices?")) return;
          await invoke("chat_delete", { chat: chatJid });
          if (entry) {
            entry.deleted = true;
            entry.deletedAt = Date.now();
            if (!state.showDeleted && state.active === chatJid) {
              state.active = null;
              renderChat();
            }
            renderChatList();
            scheduleSave();
          }
          toast("Chat deleted");
          return;
        }
        default:
          return;
      }
    } catch (err) {
      // The conflicting app-state patch right after launch reads as setup
      // noise; give the user the honest short version like the official does.
      toast(String(err).includes("still conflicting")
        ? "WhatsApp is still syncing; try again in a moment."
        : String(err));
      // The event echo may already have landed; keep the UI honest after a
      // failure.
      renderChatList();
      scheduleSave();
    }
  }

  // Apply the pin locally when the background command returns; the
  // chat_pin event echo may race this and apply the same state.
  function applyPin(entry, chatJid, pinned) {
    const chat = entry || state.chats.get(chatJid);
    if (chat) {
      chat.pinned = pinned;
      if (chat.pinned && !chat.pinTs) chat.pinTs = Date.now();
      if (!chat.pinned) delete chat.pinTs;
      renderChatList();
      scheduleSave();
    }
  }

  async function applyMuteSubmenu(chatJid) {
    const chat = state.chats.get(chatJid) || {};
    let rows;
    try {
      rows = await invoke("ui_mute_menu", { input: chatMenuInput(chat, chatJid) });
    } catch (err) {
      console.warn("mute menu state failed", err);
      return;
    }
    const submenu = $("chat-mute-sub");
    for (const button of submenu.querySelectorAll("button[data-mute-choice]")) {
      // The "Unmute" row replaces the durations when the chat is muted.
      button.hidden = button.dataset.muteChoice === "0" ? !rows.unmute : !rows.durations;
    }
  }

  $("chat-mute-sub").addEventListener("click", async (event) => {
    const button = event.target.closest("button[data-mute-choice]");
    const chatJid = chatMenuChat;
    if (!button || !chatJid) return;
    const choice = button.dataset.muteChoice;
    closeChatMenu();
    const until = choice === "0" ? 0 : choice === "-1" ? -1 : Number(choice);
    try {
      await invoke("chat_mute", { chat: chatJid, untilMs: until });
      const entry = state.chats.get(chatJid);
      if (entry) {
        entry.muteUntil = until;
        delete entry.muteForever;
        if (until < 0) entry.muteForever = true;
        renderChatList();
        scheduleSave();
        scheduleNotifySync();
      }
      toast(
        until === 0
          ? "Notifications unmuted"
          : until === -1
            ? "Notifications muted: Always"
            : `Notifications muted for ${until === 28800000 ? "8 hours" : "1 week"}`,
      );
    } catch (err) {
      toast(String(err));
    }
  });

  document.addEventListener("pointerdown", (event) => {
    if (
      !$("chat-list-menu").hidden &&
      !event.target.closest("#chat-list-menu, #chat-mute-sub")
    ) {
      closeChatMenu();
    }
  });
}
