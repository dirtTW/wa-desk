/* WA Desk frontend: main. Loaded in order by index.html; all files share the classic-script global scope. */

async function main() {
  if (!invoke || !listen) {
    showLoginError("Tauri IPC is unavailable. Run the app through `cargo tauri dev`.");
    return;
  }

  wire();
  restoreCache();
  loadStatuses();
  // Cached status replies/reactions stored before the "· Status" card existed
  // heal once both the chats and the statuses are loaded.
  healStatusInteractions();
  loadSettingsPrefs();
  applyTheme();
  syncNotifyState();
  syncAutostart();
  loadAiSettings();
  $("input").spellcheck = Boolean(settingsPrefs.spellcheck);
  updateSendState();

  await listen("wa:event", (e) => handleEvent(e.payload));
  await listen("ai:reply", (e) => {
    if (e.payload && e.payload.chat) {
      pushAiReply(e.payload.chat, e.payload.text, e.payload.id);
    }
  });

  try {
    const status = await invoke("get_status");
    state.version = status.version;
    state.ready = status.ready;
    state.linked = status.linked;
    state.connected = status.connected;
    state.me = status.me || null;
    applyOwnIdentity(status);
    if (status.linked && !status.connected) watchConnection();

    if (state.me || state.meJid) {
      $("my-avatar").title = state.meJid || state.me;
      refreshMyAvatar();
      queuePhoto(state.meLid || state.meJid);
    }

    if (status.linked) {
      $("login").hidden = true;
      $("btn-logout").hidden = false;
      setConnection(
        status.connected ? "connected" : "reconnecting",
        status.connected ? "online" : "offline",
      );
      maybeSyncChatHistory();
      await loadContacts();
      await syncContacts();
      // The cache was restored before the socket state was known, so the photo
      // requests made by that first render were dropped. Ask again now that we
      // know we are connected; otherwise a group's photo stays missing until an
      // unrelated event happens to repaint the list.
      if (status.connected) {
        renderChatList();
        if (state.active) renderChat();
        refreshMyAvatar();
      }
    }
  } catch (err) {
    showLoginError(String(err));
  }
}

main();

