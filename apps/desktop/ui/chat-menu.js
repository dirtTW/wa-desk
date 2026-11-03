/* Chat option menu — the official desktop's right-click (and long-press):
   pin, archive, mute, mark unread, block, close, clear and delete all ride
   cross-client app-state/blocklist mutations, so the phone and every other
   desktop follow along. Extracted from wiring.js to keep that file focused on
   keyboard/global wiring. Loaded before wiring.js. */

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
  async function clampMenu(menu, clientX, clientY) {
    menu.style.left = "8px";
    menu.style.top = "8px";
    const rect = menu.getBoundingClientRect();
    // The edge maths live in wa_ui::menus::clamp_popover.
    const pos = await invoke("ui_clamp_popover", {
      x: clientX,
      y: clientY,
      width: rect.width,
      height: rect.height,
      viewportW: window.innerWidth,
      viewportH: window.innerHeight,
    }).catch(() => ({ left: clientX, top: clientY }));
    menu.style.left = `${pos.left}px`;
    menu.style.top = `${pos.top}px`;
  }

  async function openChatMenu(chatJid, clientX, clientY) {
    if (!chatExists(chatJid)) return;
    chatMenuChat = chatJid;
    const menu = $("chat-list-menu");
    menu.hidden = false;
    await applyLocalChatMenuState();
    await clampMenu(menu, clientX, clientY);
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
      await clampMenu(submenu, rect.right + 4, rect.top);
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
  // The command, the local mutation, the toast and any confirmation come from
  // wa_ui::chat_actions; this only applies them.
  async function runChatAction(action, chatJid) {
    const entry = state.chats.get(chatJid);
    let plan;
    try {
      plan = await invoke("ui_chat_action", {
        input: {
          action,
          jid: chatJid,
          pinned: Boolean(entry?.pinned),
          archived: Boolean(entry?.archived),
          blocked: Boolean(entry?.blocked),
          is_open: state.active === chatJid,
          is_group: Boolean(entry?.isGroup),
        },
      });
    } catch (err) {
      toast(String(err));
      return;
    }
    if (plan.confirm && !confirm(plan.confirm)) return;

    try {
      if (plan.command) await invoke(plan.command, plan.args || {});
      if (entry) {
        if (plan.pin !== null && plan.pin !== undefined) {
          entry.pinned = plan.pin;
          if (entry.pinned && !entry.pinTs) entry.pinTs = Date.now();
          if (!entry.pinned) delete entry.pinTs;
        }
        if (plan.archived !== null && plan.archived !== undefined) {
          entry.archived = plan.archived;
        }
        if (plan.unread !== null && plan.unread !== undefined) {
          entry.unread = plan.unread ? Math.max(1, entry.unread || 0) : 0;
        }
        if (plan.blocked !== null && plan.blocked !== undefined) {
          entry.blocked = plan.blocked;
        }
        if (plan.clear) {
          entry.messages = [];
          entry.last = null;
        }
        if (plan.delete) {
          entry.deleted = true;
          entry.deletedAt = Date.now();
        }
      }
      if (plan.close) state.active = null;
      if (plan.toast) toast(plan.toast);
    } catch (err) {
      // The conflicting app-state patch right after launch reads as setup
      // noise; give the user the honest short version like the official does.
      toast(
        String(err).includes("still conflicting")
          ? "WhatsApp is still syncing; try again in a moment."
          : String(err),
      );
    }
    renderChatList();
    if (plan.close || plan.clear || plan.delete) renderChat();
    scheduleSave();
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
    // The choice -> duration mapping and the toast are wa_ui's mute_plan.
    const plan = await invoke("ui_mute_plan", { choice }).catch(() => null);
    if (!plan) return;
    const until = plan.until_ms;
    try {
      await invoke("chat_mute", { chat: chatJid, untilMs: until });
      const entry = state.chats.get(chatJid);
      if (entry) {
        entry.muteUntil = until;
        delete entry.muteForever;
        if (plan.forever) entry.muteForever = true;
        renderChatList();
        scheduleSave();
        scheduleNotifySync();
      }
      toast(plan.toast);
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

/* ------------------------------------------------------------ sidebar kebab */

// The sidebar ⋮ menu. Its handlers were referenced by wiring.js but never
// defined, so opening it threw a ReferenceError.
async function openSideMenu() {
  const menu = $("side-menu");
  if (!menu) return;
  // Unhide first so the menu has real metrics, then place it under the kebab.
  menu.hidden = false;
  const anchor = $("chat-side-menu");
  const anchorRect = anchor ? anchor.getBoundingClientRect() : { left: 8, top: 8, width: 40, height: 40 };
  const rect = menu.getBoundingClientRect();
  const pos = await invoke("ui_menu_anchor", {
    anchor: {
      x: anchorRect.left,
      y: anchorRect.top,
      width: anchorRect.width,
      height: anchorRect.height,
    },
    menuW: rect.width || 220,
    menuH: rect.height || 260,
    viewportW: window.innerWidth,
    viewportH: window.innerHeight,
  }).catch(() => ({ left: anchorRect.left, top: anchorRect.bottom }));
  menu.style.left = `${pos.left}px`;
  menu.style.top = `${pos.top}px`;
}

function closeSideMenu() {
  const menu = $("side-menu");
  if (menu) menu.hidden = true;
}

function toggleSideMenu() {
  const menu = $("side-menu");
  if (!menu) return;
  if (menu.hidden) openSideMenu();
  else closeSideMenu();
}
