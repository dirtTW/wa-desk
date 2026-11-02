/* WA Desk frontend: wiring. Loaded in order by index.html; all files share the classic-script global scope. */

function wire() {
  $("theme-toggle").addEventListener("click", () => {
    setTheme(resolvedTheme() === "dark" ? "light" : "dark");
  });
  const themeDialog = $("theme-dialog");
  for (const button of themeDialog.querySelectorAll("md-text-button")) {
    button.addEventListener("click", () => {
      const value = button.getAttribute("value") || "cancel";
      if (value === "ok") {
        const chosen = [...$("theme-form").querySelectorAll("md-radio")].find(
          (radio) => radio.checked,
        );
        if (chosen) setTheme(chosen.value);
      }
      if (typeof themeDialog.close === "function") themeDialog.close(value).catch(() => {});
    });
  }

  $("chat-list").addEventListener("click", (e) => {
    if (e.target.closest("[data-archived-toggle]")) {
      e.stopPropagation();
      state.archivedOpen = !state.archivedOpen;
      renderChatList();
      return;
    }
    const item = e.target.closest(".chat-item");
    if (!item) return;
    // "Select chats": a click ticks the row instead of opening the chat.
    if (state.selectingChats && item.dataset.jid) {
      toggleChatSelection(item.dataset.jid);
      return;
    }
    if (item.dataset.pickerJid) {
      openChatFromPicker(item.dataset.pickerJid, item.dataset.pickerName);
      return;
    }
    // Communities open their screen (announcement + groups), not the empty
    // parent chat.
    if (item.classList.contains("community")) {
      openCommunity(item.dataset.jid);
      return;
    }
    if (item.dataset.jid) openChat(item.dataset.jid);
  });

  // The sidebar kebab: the official desktop's chat-list options (New group,
  // Starred messages, Select chats, Mark all as read, App lock, Log out).
  $("chat-side-menu").addEventListener("click", (event) => {
    event.stopPropagation();
    toggleSideMenu();
  });
  $("side-menu").addEventListener("click", (event) => {
    const button = event.target.closest("[data-side-action]");
    if (!button) return;
    closeSideMenu();
    const action = button.dataset.sideAction;
    if (action === "new-group") openGroupModal();
    else if (action === "starred") openStarred();
    else if (action === "select-chats") startChatSelection();
    else if (action === "mark-read") markAllChatsRead();
    else if (action === "app-lock") {
      toast("App lock isn't supported by WA Desk yet.");
    } else if (action === "logout") doLogout();
  });
  document.addEventListener("pointerdown", (event) => {
    if (!$("side-menu").hidden && !event.target.closest("#side-menu, #chat-side-menu")) {
      closeSideMenu();
    }
  });

  $("chat-select-close").addEventListener("click", () => exitChatSelection());
  $("chat-select-read").addEventListener("click", () => applyChatSelection("read"));
  $("chat-select-archive").addEventListener("click", () => applyChatSelection("archive"));
  $("chat-select-delete").addEventListener("click", () => applyChatSelection("delete"));

  $("starred-back").addEventListener("click", () => closeStarred());
  $("starred-body").addEventListener("click", (event) => {
    const row = event.target.closest("[data-starred-jid]");
    if (row) openStarredMessage(row.dataset.starredJid, row.dataset.starredId);
  });

  // The official desktop's chat options menu lives in chat-menu.js (pin,
  // archive, mute, mark unread, block, close, clear, delete).
  initChatMenu();

  // Keep the scroll-to-latest disc clear of the reply/edit/draft cards that sit
  // above the composer (it must never cover their close button).
  initScrollLatestOffset();

  // The community screen: children/announcements open their chats, the
  // "Add group" row opens the group composer, and Back returns to the chats
  // list.
  $("community-body").addEventListener("click", (e) => {
    if (e.target.closest("[data-community-add]")) {
      openGroupModal();
      return;
    }
    const item = e.target.closest(".chat-item");
    if (item && item.dataset.jid) openChat(item.dataset.jid);
  });
  $("community-back").addEventListener("click", () => closeCommunity());

  $("filters").addEventListener("click", (e) => {
    const chip = e.target.closest(".chip");
    if (!chip) return;
    state.filter = chip.dataset.filter;
    for (const element of $("filters").querySelectorAll(".chip")) {
      element.classList.toggle("active", element === chip);
    }
    renderChatList();
  });

  // Typing in chat search rebuilds the sidebar; debounce so a burst of
  // keystrokes costs one rebuild, and keep the state in sync immediately so a
  // click that lands before the repaint still sees the current query.
  let searchTimer = null;
  $("search").addEventListener("input", (e) => {
    state.search = e.target.value.trim();
    clearTimeout(searchTimer);
    searchTimer = setTimeout(() => {
      searchTimer = null;
      renderChatList();
    }, 90);
  });

  $("composer").addEventListener("submit", (e) => {
    e.preventDefault();
    sendCurrent();
  });

  $("input").addEventListener("input", (e) => {
    autosize(e.target);
    updateSendState();
    startTyping();
  });

  $("input").addEventListener("keydown", (e) => {
    if (e.key === "Enter" && !e.shiftKey && settingsPrefs.enterSends) {
      e.preventDefault();
      sendCurrent();
    }
  });

  $("btn-emoji").addEventListener("click", (e) => {
    e.stopPropagation();
    toggleEmoji();
  });

  $("btn-attach").addEventListener("click", () => {
    if (!state.active) {
      toast("Open a chat first.");
      return;
    }
    $("file-input").click();
  });

  $("file-input").addEventListener("change", async (e) => {
    const file = e.target.files && e.target.files[0];
    e.target.value = "";
    if (!file) return;
    // With a draft open, picking a file stacks onto it instead of sending.
    if (!$("draft").hidden) addDraftFiles([file]);
    else await sendFile(file);
  });

  $("emoji-panel").addEventListener("click", (e) => {
    const tab = e.target.closest(".picker-tab");
    if (tab) {
      setPickerTab(tab.dataset.tab);
      return;
    }

    const cat = e.target.closest(".emoji-cat");
    if (cat) {
      scrollToEmojiGroup(Number(cat.dataset.group));
      return;
    }

    const emoji = e.target.closest(".emoji");
    if (emoji && emoji.dataset.char) {
      if (reactionPickMessage) {
        const message = reactionPickMessage;
        reactionPickMessage = null;
        toggleEmoji(false);
        applyReaction(message, emoji.dataset.char);
      } else {
        insertEmoji(emoji.dataset.char);
      }
      return;
    }

    const cell = e.target.closest(".sticker-cell");
    if (cell && cell.dataset.stickerId) {
      forwardSticker(cell.dataset.stickerId);
      return;
    }
    if (cell && cell.dataset.favoriteId) {
      sendFavoriteSticker(cell.dataset.favoriteId, cell.querySelector("img")?.src || "");
      return;
    }
  });

  $("emoji-search").addEventListener("input", (e) => filterEmoji(e.target.value));

  $("sticker-view").addEventListener("click", (e) => {
    const tab = e.target.closest(".sticker-tab");
    if (tab) {
      setStickerTab(tab.dataset.stickerTab);
      return;
    }
    if (e.target.closest("[data-sticker-create]")) {
      $("sticker-input").click();
    }
  });
  $("sticker-search").addEventListener("input", (e) => filterStickerGrid(e.target.value));
  $("btn-sticker-file").addEventListener("click", (e) => {
    e.stopPropagation();
    $("sticker-input").click();
  });  $("sticker-input").addEventListener("change", async (e) => {
    const file = e.target.files && e.target.files[0];
    e.target.value = "";
    if (file) await sendStickerFile(file);
  });

  $("btn-status").addEventListener("click", () => openStatus());
  $("btn-eye").addEventListener("click", () => {
    state.showDeleted = !state.showDeleted;
    $("btn-eye").classList.toggle("on", state.showDeleted);
    renderChatList();
    if (state.active) renderChat();
  });

  $("rail-chats").addEventListener("click", () => {
    $("settings-screen").hidden = true;
    $("status-screen").hidden = true;
    $("channels-screen").hidden = true;
    $("community-screen").hidden = true;
    $("starred-screen").hidden = true;
    state.communityView = null;
    state.starredOpen = false;
    $("channels-discover").hidden = true;
    closeStatusViewer();
    hideStatusShare();
    setRailActive("rail-chats");
    renderChatList();
    renderChat();
  });
  $("rail-channels").addEventListener("click", () => openChannels());
  $("channels-list").addEventListener("click", (event) => {
    const row = event.target.closest("[data-jid]");
    if (row) openChannel(row.dataset.jid);
  });
  $("channels-search").addEventListener("input", () => renderChannels());
  $("rail-settings").addEventListener("click", () => openSettings("home"));

  $("btn-compose").addEventListener("click", () => openNewChat());

  $("new-chat-back").addEventListener("click", () => closeNewChat());
  $("new-chat-input").addEventListener("input", () => renderNewChat());
  $("new-chat-list").addEventListener("click", (event) => {
    const forwardRow = event.target.closest("[data-forward-jid]");
    if (forwardRow) {
      toggleForwardTarget(forwardRow.dataset.forwardJid);
      return;
    }
    const row = event.target.closest("[data-chat-jid]");
    if (row) openChatFromPicker(row.dataset.chatJid, row.dataset.chatName);
  });
  $("forward-send").addEventListener("click", () => {
    sendForward();
  });
  document.querySelector(".new-chat-actions").addEventListener("click", (event) => {
    const action = event.target.closest("[data-new-action]");
    if (!action) return;
    if (action.dataset.newAction === "group") {
      openGroupModal();
    } else if (action.dataset.newAction === "contact") {
      openContactModal();
    } else {
      toast("Communities aren't supported yet");
    }
  });

  $("contact-cancel").addEventListener("click", () => {
    $("contact-modal").hidden = true;
  });
  $("contact-save").addEventListener("click", () => saveContact());
  for (const id of ["contact-name", "contact-phone"]) {
    $(id).addEventListener("keydown", (event) => {
      if (event.key === "Enter") saveContact();
    });
  }

  $("group-cancel").addEventListener("click", () => {
    $("group-modal").hidden = true;
  });
  $("group-create").addEventListener("click", () => createGroup());
  $("group-search").addEventListener("input", () => renderGroupContacts());
  $("group-contacts").addEventListener("change", (event) => {
    const box = event.target.closest("input[data-group-jid]");
    if (!box) return;
    if (box.checked) groupSelection.add(box.dataset.groupJid);
    else groupSelection.delete(box.dataset.groupJid);
  });
  $("group-info-back").addEventListener("click", () => {
    $("group-info-screen").hidden = true;
    renderChat();
  });
  $("group-leave").addEventListener("click", () => leaveCurrentGroup());
  $("group-add").addEventListener("click", () => openAddMembers());
  $("group-members").addEventListener("click", (event) => {
    const more = event.target.closest(".member-more");
    if (more) openMemberMenu(more);
  });
  $("member-menu").addEventListener("click", (event) => {
    const button = event.target.closest("button[data-member-action]");
    if (button) applyMemberAction(button.dataset.memberAction);
  });

  $("chat-search").addEventListener("click", () => toggleChatSearch());
  $("chat-search-close").addEventListener("click", () => closeChatSearch());
  $("chat-search-input").addEventListener("input", (event) => runChatSearch(event.target.value));
  $("chat-search-input").addEventListener("keydown", (event) => {
    if (event.key === "Enter") {
      event.preventDefault();
      scrollToChatSearchHit(chatSearchIndex + (event.shiftKey ? -1 : 1));
    } else if (event.key === "Escape") {
      event.preventDefault();
      closeChatSearch();
      $("input").focus();
    }
  });
  $("chat-search-next").addEventListener("click", () => scrollToChatSearchHit(chatSearchIndex + 1));
  $("chat-search-prev").addEventListener("click", () => scrollToChatSearchHit(chatSearchIndex - 1));
  // The rail phone icon mirrors the official client: it starts a voice call in
  // the open chat; a self chat can't call itself.
  $("rail-calls").addEventListener("click", () => {
    const chat = state.chats.get(state.active);
    if (chat && !chat.isGroup && !isSelfChat(chat.jid)) {
      startOutgoingCall(false);
      return;
    }
    toast(
      chat?.isGroup
        ? "Group voice calls start from the group info screen."
        : "Open a chat with a contact to call them.",
    );
  });
  $("chat-voice").addEventListener("click", () => startOutgoingCall(false));
  $("chat-video").addEventListener("click", () => startOutgoingCall(true));
  $("call-accept").addEventListener("click", () => acceptIncomingCall());
  $("call-reject").addEventListener("click", () => rejectIncomingCall());
  $("call-hangup").addEventListener("click", () => hangupCall());
  $("call-mute").addEventListener("click", () => toggleCallMute());
  // The header kebab button (still `#chat-menu`; the sidebar's right-click
  // options menu is `#chat-list-menu`).
  $("chat-menu").addEventListener("click", (event) => {
    event.stopPropagation();
    const dots = $("chat-dots");
    // Label follows the official: a group shows "Group info", a 1:1 shows
    // "Contact info".
    const chat = state.chats.get(state.active);
    $("chat-dots-info-label").textContent = chat?.isGroup ? "Group info" : "Contact info";
    dots.hidden = !dots.hidden;
  });
  $("chat-dots").addEventListener("click", (event) => {
    event.stopPropagation();
    const item = event.target.closest("[data-dots]");
    if (!item) return;
    $("chat-dots").hidden = true;
    if (item.dataset.dots === "info") {
      const chat = state.chats.get(state.active);
      if (chat?.isGroup) openGroupInfo();
      else openSettings("home");
      return;
    }
    if (item.dataset.dots === "voice") startOutgoingCall(false);
    if (item.dataset.dots === "video") startOutgoingCall(true);
    if (item.dataset.dots === "import") $("chat-export-file").click();
  });
  document.addEventListener("click", (event) => {
    if (!event.target.closest("#chat-dots") && !event.target.closest("#chat-menu")) {
      $("chat-dots").hidden = true;
    }
  });
  $("chat-import").addEventListener("click", () => $("chat-export-file").click());
  $("chat-export-file").addEventListener("change", (event) => {
    const file = event.target.files?.[0];
    event.target.value = "";
    importChatExport(file);
  });

  $("my-avatar").addEventListener("click", () => openSettings("home"));
  $("settings-back").addEventListener("click", () => backFromSettings());
  $("settings-body").addEventListener("click", (event) => {
    if (event.target.closest("[data-action='sync-history']")) {
      seedChatsFromContacts();
      renderChatList();
      syncChatHistoryBatch(chatsForHistorySync(), true);
      return;
    }
    if (event.target.closest("[data-ai-toggle]")) {
      toggleAiMode();
      return;
    }
    if (event.target.closest("[data-ai-require]")) {
      toggleRequireTrigger();
      return;
    }
    if (event.target.closest("#ai-save")) {
      saveAiSettings();
      return;
    }
    if (event.target.closest("#ai-test")) {
      testAiConnection();
      return;
    }
    if (event.target.closest("#ai-preview")) {
      previewAiReply();
      return;
    }

    const toggle = event.target.closest("[data-toggle]");
    if (toggle) {
      const key = toggle.dataset.toggle;
      settingsPrefs[key] = !settingsPrefs[key];
      saveSettingsPrefs();
      if (key === "spellcheck") $("input").spellcheck = Boolean(settingsPrefs.spellcheck);
      if (key === "notifications") {
        setNotificationsEnabled(Boolean(settingsPrefs.notifications));
        scheduleNotifySync();
      }
      if (key === "autostart") setAutostartEnabled(Boolean(settingsPrefs.autostart));
      renderSettings();
      return;
    }

    const action = event.target.closest("[data-action]");
    if (action) {
      if (action.dataset.action === "logout") doLogout();
      else if (action.dataset.action === "edit") openProfileEditor();
      else if (action.dataset.action === "theme") openThemeDialog();
      return;
    }

    const page = event.target.closest("[data-page]");
    if (page) openSettings(page.dataset.page);
  });
  $("profile-save").addEventListener("click", () => saveProfile());
  $("profile-cancel").addEventListener("click", () => {
    $("profile-editor").hidden = true;
  });
  $("status-back").addEventListener("click", () => closeStatus());
  $("status-camera").addEventListener("click", () => openStatusComposer());
  $("status-share-text").addEventListener("click", () => openStatusTextComposer());
  $("status-share-media").addEventListener("click", () => {
    openStatusComposer();
    $("status-file").click();
  });
  $("status-menu").addEventListener("click", () =>
    toast("Status privacy follows the phone's status settings."),
  );

  // "My status": the plus badge opens WhatsApp Web's add menu, the row itself
  // opens your own story when you have one (like the reference), and falls back
  // to the menu when there is nothing posted yet.
  $("status-mine").addEventListener("click", (event) => {
    event.stopPropagation();
    const addBadge = event.target.closest(".status-add");
    const mine = [...state.statuses.values()].find(
      (entry) => isMyStatus(entry) && entry.updates.length > 0,
    );
    if (!addBadge && mine) {
      openStatusViewer(mine.jid);
      return;
    }
    openStatusAddMenu();
  });
  $("status-add-menu").addEventListener("click", (event) => {
    const item = event.target.closest("[data-status-add]");
    if (!item) return;
    closeStatusAddMenu();
    if (item.dataset.statusAdd === "text") {
      openStatusTextComposer();
    } else {
      openStatusComposer();
      $("status-file").click();
    }
  });
  document.addEventListener("click", (event) => {
    if ($("status-add-menu").hidden) return;
    if (event.target.closest("#status-add-menu") || event.target.closest("#status-mine")) return;
    closeStatusAddMenu();
  });

  // Full-screen text status composer.
  $("status-text-close").addEventListener("click", () => closeStatusTextComposer());
  $("status-text-input").addEventListener("input", () => {
    resizeStatusTextInput();
    updateStatusTextSend();
  });
  $("status-text-input").addEventListener("keydown", (event) => {
    if (event.key !== "Escape") return;
    event.preventDefault();
    closeStatusTextComposer();
  });
  $("status-text-send").addEventListener("click", () => postTextStatus());
  $("status-text-font").addEventListener("click", () => applyStatusTextFont(statusFontIndex + 1));
  // WhatsApp's palette button switches to the next background colour on each tap.
  $("status-text-colour").addEventListener("click", () => {
    applyStatusTextBackground(statusBgIndex + 1);
    $("status-emoji-panel").hidden = true;
  });
  $("status-text-emoji").addEventListener("click", () => {
    buildStatusEmojiPanel();
    const panel = $("status-emoji-panel");
    panel.hidden = !panel.hidden;
    if (!panel.hidden) $("status-emoji-search").focus();
  });
  $("status-emoji-search").addEventListener("input", (event) =>
    filterStatusEmoji(event.target.value),
  );
  $("status-emoji-cats").addEventListener("click", (event) => {
    const cat = event.target.closest("[data-status-emoji-group]");
    if (!cat) return;
    for (const node of $("status-emoji-cats").querySelectorAll(".emoji-cat")) {
      node.classList.toggle("active", node === cat);
    }
    $("status-emoji-search").value = "";
    renderStatusEmojiGroup(Number(cat.dataset.statusEmojiGroup));
  });
  $("status-emoji-panel").addEventListener("click", (event) => {
    const item = event.target.closest("[data-status-emoji]");
    if (item) insertStatusEmoji(item.dataset.statusEmoji);
  });
  $("status-text-privacy").addEventListener("click", () =>
    toast("Status privacy follows the phone's status settings."),
  );

  // Own story: the eye counter opens the "Viewed by" list.
  $("status-view-count").addEventListener("click", (event) => {
    event.stopPropagation();
    openStatusViews();
  });
  $("status-views-close").addEventListener("click", () => closeStatusViews());
  $("status-views").addEventListener("click", (event) => {
    if (!event.target.closest(".status-views-card")) closeStatusViews();
  });
  document.addEventListener("keydown", (event) => {
    if (event.key !== "Escape") return;
    if (!$("status-views").hidden) {
      closeStatusViews();
      return;
    }
    if (!$("status-text-screen").hidden) {
      closeStatusTextComposer();
      return;
    }
    if ($("status-text-screen").hidden && !$("status-add-menu").hidden) closeStatusAddMenu();
  });

  $("status-cancel").addEventListener("click", () => {
    resetStatusDraft();
    $("status-composer").hidden = true;
  });
  $("status-choose-media").addEventListener("click", () => $("status-file").click());
  $("status-file").addEventListener("change", (event) => {
    const file = event.target.files && event.target.files[0];
    event.target.value = "";
    if (!file) return;
    statusDraftFile = file;
    if (statusDraftUrl) URL.revokeObjectURL(statusDraftUrl);
    statusDraftUrl = URL.createObjectURL(file);
    const preview = $("status-preview");
    preview.hidden = false;
    preview.innerHTML = file.type.startsWith("video/")
      ? `<video src="${statusDraftUrl}" controls></video>`
      : `<img src="${statusDraftUrl}" alt="" />`;
  });
  $("status-post").addEventListener("click", () => postStatus());
  document.querySelectorAll(".status-tab").forEach((tab) => {
    tab.addEventListener("click", () => setStatusTab(tab.dataset.statusTab));
  });
  const openStatusItem = (event) => {
    const item = event.target.closest(".status-item");
    if (item && item.dataset.statusJid) openStatusViewer(item.dataset.statusJid);
  };
  $("status-list").addEventListener("click", openStatusItem);
  $("status-viewed-list").addEventListener("click", openStatusItem);
  $("status-viewer-close").addEventListener("click", () => closeStatusViewer());
  $("status-viewer-pause").addEventListener("click", (event) => {
    event.stopPropagation();
    toggleStatusPause();
  });
  $("status-prev").addEventListener("click", () => stepStatus(-1));
  $("status-next").addEventListener("click", () => stepStatus(1));
  $("status-viewer-body").addEventListener("click", (event) => {
    if (event.target.closest("video")) return;
    stepStatus(1);
  });
  $("status-reply-send").addEventListener("click", () => replyCurrentStatus());
  $("status-reply-input").addEventListener("keydown", (event) => {
    if (event.key === "Enter") {
      event.preventDefault();
      replyCurrentStatus();
    }
  });
  $("status-viewer").querySelectorAll(".status-react").forEach((button) => {
    button.addEventListener("click", () => reactCurrentStatus(button.dataset.statusReact));
  });
  $("status-heart").addEventListener("click", () => reactCurrentStatus("❤️"));
  $("status-viewer-mute").addEventListener("click", () => {
    statusMuted = !statusMuted;
    const video = $("status-viewer-body").querySelector("video");
    if (video) video.muted = statusMuted;
    toast(statusMuted ? "Status muted" : "Status unmuted");
  });
  $("status-viewer-menu").addEventListener("click", () => deleteCurrentStatus());

  $("messages").addEventListener("scroll", () => {
    const box = $("messages");
    const distance = distanceFromBottom(box);
    const top = box.scrollTop;
    const movedUp = top < lastMessageScrollTop - 1;
    lastMessageScrollTop = top;
    if (movedUp) box.dataset.stickBottom = "0";
    else if (distance <= 8) box.dataset.stickBottom = "1";
    if (distance <= 8) setScrollLatestUnread(false);
    updateScrollLatestButton();
    if (box.scrollTop <= 8) maybePageOlderMessages();
  }, { passive: true });

  $("scroll-latest").addEventListener("click", () => scrollToLatest());
  window.addEventListener("resize", updateScrollLatestButton);

  $("messages").addEventListener("click", (event) => {
    // A recovered view-once placeholder opens the kept media directly
    // (official tap-to-view), no rail eye needed. The Learn-more anchor
    // inside the placeholder keeps its default-browser route.
    const onceBody = event.target.closest("[data-view-once-open]");
    if (onceBody && !event.target.closest("a")) {
      openMediaViewer(onceBody.dataset.viewOnceOpen);
      return;
    }
    const image = event.target.closest(".media-image");
    if (image) {
      event.preventDefault();
      if (image.dataset.mediaId) openMediaViewer(image.dataset.mediaId);
      return;
    }
    const video = event.target.closest("video.media-video, .media-video-wrap");
    if (video) {
      const id = video.dataset.mediaId || video.querySelector("video")?.dataset.mediaId;
      if (id) openMediaViewer(id);
    }
  });
  $("media-viewer-close").addEventListener("click", () => closeMediaViewer());
  // Official viewer affordances: reply under the composer banner, react with
  // a quick emoji bar, forward through the chat picker. All reuse the existing
  // message actions on the currently displayed media message.
  $("media-viewer-reply").addEventListener("click", () => {
    const item = mediaViewerItems[mediaViewerIndex];
    if (!item) return;
    closeMediaViewer();
    startReply(item);
  });
  $("media-viewer-react").addEventListener("click", () => {
    if (mediaViewerItems[mediaViewerIndex]) {
      $("media-viewer-reactbar").hidden = !$("media-viewer-reactbar").hidden;
    }
  });
  for (const button of document.querySelectorAll("#media-viewer-reactbar [data-react-emoji]")) {
    button.addEventListener("click", () => {
      const item = mediaViewerItems[mediaViewerIndex];
      if (!item) return;
      $("media-viewer-reactbar").hidden = true;
      applyReaction(item, button.dataset.reactEmoji);
    });
  }
  $("media-viewer-forward").addEventListener("click", () => {
    const item = mediaViewerItems[mediaViewerIndex];
    if (!item) return;
    closeMediaViewer();
    openForwardPicker(item);
  });
  // The filmstrip hides its scrollbar like the official viewer; the mouse
  // wheel keeps it scrollable.
  $("media-viewer-strip").addEventListener("wheel", (event) => {
    if (Math.abs(event.deltaY) > Math.abs(event.deltaX)) {
      $("media-viewer-strip").scrollLeft += event.deltaY;
      event.preventDefault();
    }
  }, { passive: false });
  $("media-viewer-download").addEventListener("click", () => downloadCurrentMedia());
  $("media-viewer-prev").addEventListener("click", () => stepMediaViewer(-1));
  $("media-viewer-next").addEventListener("click", () => stepMediaViewer(1));
  $("media-viewer-zoom-in").addEventListener("click", () => viewerZoomBy(0.5));
  $("media-viewer-zoom-out").addEventListener("click", () => viewerZoomBy(-0.5));
  const viewerStage = $("media-viewer-stage");
  viewerStage.addEventListener("click", (event) => {
    if (event.target === viewerStage && viewerZoom <= 1) closeMediaViewer();
  });
  viewerStage.addEventListener(
    "wheel",
    (event) => {
      if (!document.querySelector(".viewer-zoom-layer")) return;
      event.preventDefault();
      viewerZoomBy(event.deltaY < 0 ? 0.3 : -0.3);
    },
    { passive: false },
  );
  viewerStage.addEventListener("dblclick", (event) => {
    if (!document.querySelector(".viewer-zoom-layer")) return;
    event.preventDefault();
    setViewerZoom(viewerZoom > 1 ? 1 : 2);
  });
  viewerStage.addEventListener("pointerdown", (event) => {
    if (viewerZoom <= 1) return;
    event.preventDefault();
    viewerDragging = true;
    viewerDragStart = {
      x: event.clientX - viewerPanX,
      y: event.clientY - viewerPanY,
    };
    viewerStage.classList.add("dragging");
  });
  window.addEventListener("pointermove", (event) => {
    if (!viewerDragging || !viewerDragStart) return;
    viewerPanX = event.clientX - viewerDragStart.x;
    viewerPanY = event.clientY - viewerDragStart.y;
    applyViewerZoom();
  });
  const endViewerDrag = () => {
    viewerDragging = false;
    viewerDragStart = null;
    viewerStage.classList.remove("dragging");
  };
  window.addEventListener("pointerup", endViewerDrag);
  window.addEventListener("pointercancel", endViewerDrag);
  $("media-viewer-strip").addEventListener("click", (event) => {
    const thumb = event.target.closest("[data-index]");
    if (!thumb) return;
    mediaViewerIndex = Number(thumb.dataset.index);
    renderMediaViewer();
  });

  // Document cards: View opens the in-app preview, Save as writes a copy into
  // Downloads\WA Desk with the sender's own file name. Registered in the
  // capture phase so the bubble/media handlers never swallow the buttons.
  $("messages").addEventListener(
    "click",
    (event) => {
      const action = event.target.closest("[data-doc-action]");
      if (!action) return;
      event.preventDefault();
      event.stopPropagation();
      const id = action.dataset.docId;
      if (!id) return;
      if (action.dataset.docAction === "view") openDocumentViewer(id);
      else saveDocument(id, action.dataset.docName || "");
    },
    true,
  );
  // Media tiles that gave up (CDN 403, timeout) offer a retry instead of
  // staying on the loading spinner forever.
  $("messages").addEventListener("click", (event) => {
    const tile = event.target.closest(".media-box.retry");
    if (!tile) return;
    event.preventDefault();
    event.stopPropagation();
    retryMedia(tile);
  });
  $("doc-viewer-close").addEventListener("click", () => closeDocumentViewer());
  $("document-viewer").addEventListener("click", (event) => {
    if (event.target.closest("[data-doc-close]")) closeDocumentViewer();
  });
  $("doc-viewer-save").addEventListener("click", (event) => {
    const button = event.currentTarget;
    saveDocument(button.dataset.docId, button.dataset.docName || "");
  });
  $("doc-viewer-open").addEventListener("click", (event) => {
    const id = event.currentTarget.dataset.docId;
    if (id) openDocumentExternally(id);
  });
  $("doc-viewer-reveal").addEventListener("click", (event) => {
    const id = event.currentTarget.dataset.docId;
    if (id) revealDocument(id);
  });
  document.addEventListener("keydown", (event) => {
    if (!$("document-viewer").hidden && event.key === "Escape") {
      event.preventDefault();
      closeDocumentViewer();
      return;
    }
    if ($("media-viewer").hidden) return;
    if (event.target.closest("input, textarea, [contenteditable='true']")) return;
    if (event.key === "ArrowLeft") {
      event.preventDefault();
      stepMediaViewer(-1);
    } else if (event.key === "ArrowRight") {
      event.preventDefault();
      stepMediaViewer(1);
    } else if (event.code === "Space" || event.key.toLowerCase() === "k") {
      const video = document.querySelector(".viewer-video-el");
      if (!video) return;
      event.preventDefault();
      if (video.paused) video.play().catch(() => {});
      else video.pause();
    }
  });

  document.addEventListener("paste", (event) => {
    const items = event.clipboardData ? [...event.clipboardData.items] : [];
    const file = WAChatUtils.pickClipboardImage(items);
    if (!file) return;
    event.preventDefault();
    addDraftFiles([file]);
  });

  $("draft-send").addEventListener("click", () => sendDraft());
  $("draft-cancel").addEventListener("click", () => clearDraft());
  $("draft-thumbs").addEventListener("click", (event) => {
    const button = event.target.closest(".draft-item-remove");
    if (button) removeDraftAt(Number(button.dataset.index));
  });
  $("draft-more").addEventListener("click", () => $("file-input").click());

  const messages = $("messages");

  messages.addEventListener("pointerdown", (event) => {
    const bubble = event.target.closest(".bubble");
    const message = bubble && bubbleMessage(bubble);
    if (!message) return;

    cancelPress();
    pressBubble = bubble;
    bubble.classList.add("press");
    pressTimer = setTimeout(() => {
      pressTimer = null;
      if (pressBubble) pressBubble.classList.remove("press");
      openMessageMenu(message, event.clientX, event.clientY);
    }, 450);
  });

  ["pointerup", "pointerleave", "pointermove", "pointercancel"].forEach((type) =>
    messages.addEventListener(type, cancelPress),
  );

  messages.addEventListener("contextmenu", (event) => {
    const bubble = event.target.closest(".bubble");
    const message = bubble && bubbleMessage(bubble);
    if (!message) return;
    event.preventDefault();
    openMessageMenu(message, event.clientX, event.clientY);
  });

  $("msg-menu").addEventListener("click", (event) => {
    const more = event.target.closest("[data-reaction-more]");
    if (more) {
      event.stopPropagation();
      reactionPickMessage = menuMessage;
      closeMessageMenu();
      toggleEmoji(true);
      $("emoji-search").focus();
      return;
    }

    const emojiButton = event.target.closest("button[data-emoji]");
    if (emojiButton) {
      const message = menuMessage;
      closeMessageMenu();
      applyReaction(message, emojiButton.dataset.emoji);
      return;
    }

    const button = event.target.closest("button[data-action]");
    if (!button) return;
    const action = button.dataset.action;
    const message = menuMessage;
    closeMessageMenu();
    applyMessageAction(action, message);
  });

  document.addEventListener("pointerdown", (event) => {
    if (!$("msg-menu").hidden && !event.target.closest("#msg-menu")) closeMessageMenu();
    if (!$("member-menu").hidden && !event.target.closest("#member-menu, .member-more")) {
      closeMemberMenu();
    }
  });

  $("edit-cancel").addEventListener("click", () => cancelEdit());
  $("reply-cancel").addEventListener("click", () => cancelReply());
  $("msg-info-close").addEventListener("click", () => {
    $("msg-info").hidden = true;
  });
  $("msg-info").addEventListener("click", (event) => {
    if (event.target === $("msg-info")) $("msg-info").hidden = true;
  });

  // Tapping a reply's quote jumps to the message it answers, like WhatsApp.
  $("messages").addEventListener("click", (event) => {
    const mention = event.target.closest(".mention[data-mention-jid]");
    if (mention && !state.selecting) {
      event.stopPropagation();
      const jid = mention.dataset.mentionJid;
      // Mentions carry the bare user (sometimes with a device suffix); reuse the
      // conversation the sidebar already has instead of creating a twin.
      const user = String(jid).split("@")[0].split(":")[0];
      const server = String(jid).split("@")[1] || "";
      const existing = [...state.chats.keys()].find((key) => {
        const [keyUser, keyServer] = String(key).split("@");
        return keyServer === server && keyUser.split(":")[0] === user;
      });
      openChatFromPicker(existing || jid, mention.textContent.replace(/^@/, ""));
      return;
    }
    if (!state.selecting) return;
    const bubble = event.target.closest(".bubble");
    if (!bubble || !bubble.dataset.id) return;
    event.preventDefault();
    event.stopPropagation();
    toggleSelection(bubble.dataset.id);
  }, true);

  $("select-close").addEventListener("click", () => exitSelection());
  $("select-forward").addEventListener("click", () => forwardSelection());
  $("select-delete").addEventListener("click", () => deleteSelection());
  $("delete-confirm").addEventListener("click", (event) => {
    const choice = event.target.closest("[data-delete-choice]");
    if (!choice) {
      if (event.target === $("delete-confirm")) closeDeleteConfirm();
      return;
    }
    if (choice.dataset.deleteChoice === "cancel") closeDeleteConfirm();
    else confirmDelete(choice.dataset.deleteChoice === "everyone");
  });
  $("messages").addEventListener("click", (event) => {
    const quote = event.target.closest(".quote[data-quote-id]");
    if (!quote) return;
    event.stopPropagation();
    jumpToQuoted(quote.dataset.quoteId);
  });
  // WebView2 hands `target="_blank"` anchors to the OS window opener, which
  // made body links dead and even flung `asset://…` file paths into the
  // browser. Every external anchor goes through `open_link` (default
  // browser), channel post images follow the official app and open the
  // post's own URL, and internal schemes are simply cancelled here.
  document.addEventListener("click", (event) => {
    const anchor = event.target.closest && event.target.closest('a[target="_blank"]');
    if (!anchor) return;
    const href = anchor.getAttribute("href") || "";
    // WebView2's file proxy (`asset.localhost`) is an INTERNAL url: opening it
    // anywhere outside the media viewer made "nothing happen" (a rejected
    // open_link + cancelled click), so only real http(s) websites go out.
    const isExternalHttp = /^https?:\/\//i.test(href) && !/asset\.localhost/i.test(href);
    if (anchor.classList.contains("media-image") && anchor.closest(".channel-view")) {
      event.preventDefault();
      event.stopPropagation();
      const bubble = anchor.closest("[data-id]");
      const entry = state.chats.get(state.active);
      const message =
        bubble && entry && entry.messages.find((m) => m.id === bubble.dataset.id);
      const postUrl = message && WAChatUtils.firstUrl(message.text || "");
      if (postUrl && isExternalHttp(postUrl)) {
        invoke("open_link", { url: postUrl }).catch((err) => toast(String(err)));
      } else {
        openMediaViewer(anchor.dataset.mediaId);
      }
      return;
    }
    if (isExternalHttp) {
      event.preventDefault();
      event.stopPropagation();
      invoke("open_link", { url: href }).catch((err) => toast(String(err)));
      return;
    }
    // Internal scheme (`asset://…`): the in-app viewer handlers must still
    // receive the click, so only the OS window opener is cancelled.
    event.preventDefault();
  }, true);
  $("messages").addEventListener("keydown", (event) => {
    if (event.key !== "Enter" && event.key !== " ") return;
    const quote = event.target.closest(".quote[data-quote-id]");
    if (!quote) return;
    event.preventDefault();
    jumpToQuoted(quote.dataset.quoteId);
  });

  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape") {
      if (!$("media-viewer").hidden) closeMediaViewer();
      else if (!$("member-menu").hidden) closeMemberMenu();
      else if (!$("msg-menu").hidden) closeMessageMenu();
      else if (!$("profile-editor").hidden) $("profile-editor").hidden = true;
      else if (!$("contact-modal").hidden) $("contact-modal").hidden = true;
      else if (!$("group-modal").hidden) $("group-modal").hidden = true;
      else if (state.selectingChats) exitChatSelection();
      else if (!$("community-screen").hidden) closeCommunity();
      else if (!$("starred-screen").hidden) closeStarred();
      else if (state.editing) cancelEdit();
      else if (!$("draft").hidden) clearDraft();
      else if (!$("emoji-panel").hidden) toggleEmoji(false);
      else if (!$("status-viewer").hidden) closeStatusViewer();
      else if (!$("status-composer").hidden) {
        resetStatusDraft();
        $("status-composer").hidden = true;
      }
      else if (!$("settings-screen").hidden) backFromSettings();
      else if (!$("status-screen").hidden) closeStatus();
      else if (!$("new-chat-screen").hidden) closeNewChat();
      else if (!$("group-info-screen").hidden) {
        $("group-info-screen").hidden = true;
        renderChat();
      }
    }
  });

  document.addEventListener("keydown", (event) => {
    if ($("status-viewer").hidden) return;
    if (event.target.closest("input, textarea, [contenteditable='true']")) return;
    if (event.key === "ArrowLeft") {
      event.preventDefault();
      stepStatus(-1);
    } else if (event.key === "ArrowRight") {
      event.preventDefault();
      stepStatus(1);
    } else if (event.key === " " || event.key === "Spacebar") {
      // Space pauses/resumes the story, like WhatsApp Web.
      event.preventDefault();
      toggleStatusPause();
    }
  });

  document.addEventListener("click", (e) => {
    if (
      !$("emoji-panel").hidden &&
      !e.target.closest("#emoji-panel") &&
      !e.target.closest("#btn-emoji") &&
      !e.target.closest("#msg-menu")
    ) {
      toggleEmoji(false);
    }
  });

  $("btn-pair").addEventListener("click", async () => {
    const phone = $("phone").value.trim();
    if (!phone) {
      showLoginError("Enter your phone number in international format, e.g. 15551234567.");
      return;
    }
    showLoginError("");
    try {
      await invoke("request_pair_code", { phone });
    } catch (err) {
      showLoginError(String(err));
    }
  });

  $("qr-refresh").addEventListener("click", () => requestNewQr());

  $("btn-logout").addEventListener("click", () => doLogout());
}

