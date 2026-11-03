/* WA Desk frontend: stickers. Loaded in order by index.html; all files share the classic-script global scope. */

/* ---------------------------------------------------------------- stickers */

const STICKER_RECENT_LIMIT = 80;
const STICKER_LOAD_CONCURRENCY = 16;

let stickerDirty = true;
let stickerLoadToken = 0;
let aiStickerSyncTimer = 0;

function syncAiStickers(immediate = false) {
  clearTimeout(aiStickerSyncTimer);
  const push = async () => {
    // Recent-sticker selection and the ready filter are decided by
    // wa_ui::context.
    const messages = await recentStickerMessages();
    const ids = messages.map((message) => message.id);
    const ready = ids.filter((id) => state.media.get(id)?.status === "ready");
    const chosen = await invoke("ui_ai_sticker_ids", {
      ids,
      ready,
      limit: 80,
    }).catch(() => []);
    invoke("set_ai_stickers", { ids: chosen }).catch(() => {});
  };
  if (immediate) push();
  else aiStickerSyncTimer = setTimeout(push, 800);
}

function markStickersDirty() {
  stickerDirty = true;
}

let aiContextTimer = 0;

function scheduleAiContext() {
  const chat = state.active;
  clearTimeout(aiContextTimer);
  if (!chat) return;
  aiContextTimer = setTimeout(() => syncAiContext(chat), 600);
}

// Seed the backend with a short, bounded transcript so the AI knows what the
// chat was about even after an app restart.
async function syncAiContext(chat) {
  const entry = state.chats.get(chat);
  if (!entry) return;
  // Visibility, the 24-line window, senders and media placeholders are decided
  // by wa_ui::context.
  const lines = await invoke("ui_ai_context", {
    messages: entry.messages.map((message) => ({
      id: String(message.id || ""),
      text: String(message.text || ""),
      kind: message.kind || "text",
      from_me: Boolean(message.from_me),
      sender_name: message.sender_name || "",
      sender_user: message.sender_user || "",
      timestamp: message.timestamp || 0,
      deleted: Boolean(message.deleted),
      deleted_for_me: Boolean(message.deletedForMe),
      cleared: Boolean(message.cleared),
    })),
    limit: 24,
  }).catch(() => []);
  if (!lines.length) return;
  invoke("set_ai_context", { chat, lines }).catch(() => {});
}

// Recent stickers are the sticker messages in the local cache; tapping one
// forwards it, which is how WhatsApp's recent-sticker row behaves.
async function recentStickerMessages() {
  // Collect the candidates; wa_ui::context dedupes, filters and sorts them.
  const byId = new Map();
  for (const chat of state.chats.values()) {
    for (const message of chat.messages) {
      if (message.kind !== "sticker") continue;
      if (!byId.has(message.id)) byId.set(message.id, message);
    }
  }
  const ids = await invoke("ui_recent_stickers", {
    candidates: [...byId.values()].map((message) => ({
      id: message.id,
      timestamp: message.timestamp || 0,
      pending: Boolean(message.pending),
    })),
    limit: STICKER_RECENT_LIMIT,
  }).catch(() => []);
  return ids.map((id) => byId.get(id)).filter(Boolean);
}

async function mapLimit(items, limit, fn) {
  const results = new Array(items.length);
  let cursor = 0;

  const worker = async () => {
    while (true) {
      const index = cursor++;
      if (index >= items.length) return;
      results[index] = await fn(items[index], index);
    }
  };

  await Promise.all(
    Array.from({ length: Math.min(limit, items.length) }, () => worker()),
  );
  return results;
}

async function stickerUrl(message) {
  const cached = state.media.get(message.id);
  if (cached?.status === "ready" && cached.url) return cached.url;

  try {
    const timeout = new Promise((_, reject) =>
      setTimeout(() => reject(new Error("sticker media timeout")), 1200),
    );
    const path = await Promise.race([
      invoke("fetch_media", { id: message.id }),
      timeout,
    ]);
    const url = convertFileSrc ? convertFileSrc(path) : path;
    rememberMedia(message.id, url);
    return url;
  } catch {
    return null;
  }
}

let stickerTab = "recent";

/* ------------------------------------------------- Rust-rendered sticker grid */

// The Create tile, cells, loading placeholders and empty states are built by
// wa-ui::stickers; this file keeps the downloads and the send actions.
let stickerCells = [];

async function renderStickerGrid(empty = "") {
  let out;
  try {
    out = await invoke("ui_sticker_grid", {
      request: {
        cells: stickerCells,
        query: $("sticker-search")?.value || "",
        empty,
      },
    });
  } catch (err) {
    console.warn("sticker grid render failed", err);
    return null;
  }
  $("sticker-grid").innerHTML = out.html;
  $("sticker-count").textContent = out.loaded ? `(${out.loaded})` : "";
  return out;
}

async function filterStickerGrid() {
  await renderStickerGrid();
}

function reloadStickerView(force = true) {
  if (stickerTab === "favorites" || stickerTab === "packs") loadFavoriteStickers();
  else loadRecentStickers(force);
}

function setStickerTab(tab) {
  if (tab === "create") {
    $("sticker-input").click();
    return;
  }
  stickerTab = tab;
  $("sticker-view")
    .querySelectorAll(".sticker-tab")
    .forEach((node) =>
      node.classList.toggle("active", node.dataset.stickerTab === tab),
    );
  const search = $("sticker-search");
  if (search) search.value = "";
  if (tab === "favorites" || tab === "packs") loadFavoriteStickers();
  else loadRecentStickers(true);
}

// Loads in parallel and skips work when nothing changed since the last open.
async function loadFavoriteStickers() {
  stickerCells = [];
  await renderStickerGrid("Loading favorite stickers from the linked session...");
  try {
    const favorites = await invoke("fetch_favorite_stickers");
    if (!favorites.length) {
      await renderStickerGrid(
        "No favorite stickers found. Star stickers on your phone, then try again.",
      );
      return;
    }
    stickerCells = favorites.map((favorite) => ({
      id: favorite.id,
      url: convertFileSrc ? convertFileSrc(favorite.path) : favorite.path,
      kind: "favorite",
      search: "favorite sticker",
    }));
    await renderStickerGrid();
  } catch (err) {
    stickerCells = [];
    await renderStickerGrid(`Could not load favorites: ${String(err)}`);
  }
}

async function sendFavoriteSticker(id, sourceUrl) {
  const chat = state.active;
  if (!chat) {
    toast("Open a chat first.");
    return;
  }
  const entry = state.chats.get(chat);
  if (!entry) return;

  // Mirror `sendSticker`: show the local favorite immediately, then swap in
  // the server id. Without the optimistic bubble the send succeeds on the wire
  // but the chat looks unchanged because own messages are not echoed back.
  const tempId = `local-${++state.localSeq}`;
  if (sourceUrl) rememberMedia(tempId, sourceUrl);
  const optimistic = {
    id: tempId,
    chat,
    sender: null,
    sender_name: null,
    from_me: true,
    is_group: Boolean(entry.isGroup),
    timestamp: Date.now(),
    kind: "sticker",
    text: "",
    pending: true,
  };
  entry.messages.push(optimistic);
  entry.last = optimistic;
  entry.lastTs = optimistic.timestamp;
  renderChatList();
  renderChat();
  // Close the picker immediately, like WhatsApp Web. Waiting for the upload
  // leaves the grid clickable and lets one tap turn into several sends.
  toggleEmoji(false);

  try {
    const result = await invoke("send_favorite_sticker", { chat, id });
    const media =
      state.media.get(tempId) || (sourceUrl ? { status: "ready", url: sourceUrl } : null);
    if (media) {
      state.media.delete(tempId);
      putMedia(result.id, media);
    }
    optimistic.id = result.id;
    optimistic.pending = false;
    markStickersDirty();
    renderChatList();
    renderChat();
    reloadStickerView();
  } catch (err) {
    optimistic.pending = false;
    optimistic.failed = true;
    if (sourceUrl) state.media.delete(tempId);
    toast(String(err));
    renderChat();
  }
}

async function loadRecentStickers(force = false) {
  if (!force && !stickerDirty && $("sticker-grid").childElementCount > 0) return;

  const token = ++stickerLoadToken;
  const messages = await recentStickerMessages();
  stickerDirty = false;

  if (messages.length === 0) {
    stickerCells = [];
    await renderStickerGrid(
      "No stickers yet. Received stickers appear here, or add a .webp file.",
    );
    return;
  }

  stickerCells = messages.map((message) => ({
    id: message.id,
    url: state.media.get(message.id)?.url || null,
    kind: "recent",
    search: "recent sticker",
  }));
  await renderStickerGrid();

  await mapLimit(messages, STICKER_LOAD_CONCURRENCY, async (message) => {
    const url = await stickerUrl(message);
    if (token !== stickerLoadToken) return;
    const cell = stickerCells.find((entry) => entry.id === message.id);
    if (cell && url) cell.url = url;
  });

  if (token !== stickerLoadToken) return; // another load superseded this one
  const out = await renderStickerGrid("Stickers could not be loaded right now.");
  syncAiStickers(true);
  if (out && !out.loaded) {
    stickerCells = [];
    await renderStickerGrid("Stickers could not be loaded right now.");
  }
}

async function sendStickerFile(file) {
  const chat = state.active;
  if (!file || !chat) return;
  if (!/\.webp$/i.test(file.name) && file.type !== "image/webp") {
    toast("Stickers must be .webp files.");
    return;
  }
  if (file.size > 2 * 1024 * 1024) {
    toast("Stickers must be under 2 MB.");
    return;
  }

  const entry = state.chats.get(chat);
  if (!entry) return;

  const localUrl = URL.createObjectURL(file);
  const tempId = `local-${++state.localSeq}`;
  rememberMedia(tempId, localUrl);

  const optimistic = {
    id: tempId,
    chat,
    sender: null,
    sender_name: null,
    from_me: true,
    is_group: entry.isGroup,
    timestamp: Date.now(),
    kind: "sticker",
    text: "",
    pending: true,
  };
  entry.messages.push(optimistic);
  entry.last = optimistic;
  entry.lastTs = optimistic.timestamp;
  renderChatList();
  renderChat();
  toggleEmoji(false);

  try {
    const data = await fileToBase64(file);
    const result = await invoke("send_sticker", { chat, data });
    const media = state.media.get(tempId) || { status: "ready", url: localUrl };
    state.media.delete(tempId);
    putMedia(result.id, media);
    optimistic.id = result.id;
    optimistic.pending = false;
    markStickersDirty();
    renderChatList();
    renderChat();
    reloadStickerView();
  } catch (err) {
    optimistic.pending = false;
    optimistic.failed = true;
    toast(String(err));
    renderChat();
  }
}

async function forwardSticker(messageId) {
  const chat = state.active;
  if (!chat) {
    toast("Open a chat first.");
    return;
  }

  const entry = state.chats.get(chat);
  const sourceUrl = state.media.get(messageId)?.url;
  const tempId = `local-${++state.localSeq}`;
  if (sourceUrl) rememberMedia(tempId, sourceUrl);

  const optimistic = {
    id: tempId,
    chat,
    sender: null,
    sender_name: null,
    from_me: true,
    is_group: Boolean(entry && entry.isGroup),
    timestamp: Date.now(),
    kind: "sticker",
    text: "",
    pending: true,
  };
  if (entry) {
    entry.messages.push(optimistic);
    entry.last = optimistic;
    entry.lastTs = optimistic.timestamp;
    renderChatList();
    renderChat();
    toggleEmoji(false);
  }

  try {
    await invoke("forward_sticker", { chat, id: messageId });
    optimistic.pending = false;
    markStickersDirty();
    renderChat();
  } catch (err) {
    optimistic.pending = false;
    optimistic.failed = true;
    toast(String(err));
    renderChat();
  }
}


/* ------------------------------------------------------------------ emoji */

/* ------------------------------------------------------------------ */
/* wiring                                                              */
/* ------------------------------------------------------------------ */

const EMOJI_RECENT_KEY = "wadesk.emoji.recents";

let emojiBuilt = false;
let emojiQuery = "";
let emojiActiveGroup = 0;

async function updateSendState() {
  // The "needs real text" rule is wa_ui::emoji::composer_has_text.
  const live = await invoke("ui_composer_has_text", { text: $("input").value }).catch(
    () => $("input").value.trim().length > 0,
  );
  $("send").disabled = !live;
}

function recentEmojis() {
  try {
    const parsed = JSON.parse(localStorage.getItem(EMOJI_RECENT_KEY) || "[]");
    return Array.isArray(parsed) ? parsed.filter((value) => typeof value === "string") : [];
  } catch {
    return [];
  }
}

async function rememberEmoji(char) {
  // The dedupe/unshift/cap is wa_ui::emoji::remember_recent.
  const recents = await invoke("ui_emoji_remember", {
    recents: recentEmojis(),
    char,
  }).catch(() => recentEmojis());
  try {
    localStorage.setItem(EMOJI_RECENT_KEY, JSON.stringify(recents));
  } catch {
    /* storage full: recents are a nicety */
  }
  renderRecentEmojis();
}

function renderRecentEmojis() {
  const node = $("emoji-recents");
  if (!node) return;
  // The row markup comes from wa-ui::emoji; the list itself is local storage.
  invoke("ui_emoji_recents", { chars: recentEmojis() })
    .then((html) => {
      node.innerHTML = html;
    })
    .catch(() => {});
}

// "full" emoji support: every Unicode group, searchable, plus recents.
//
// The catalogue and the picker markup live in Rust (`wa-ui`): the webview only
// injects the HTML fetched at boot and asks Rust for search results.
function buildEmojiPicker() {
  if (emojiBuilt) return;
  const scroll = $("emoji-scroll");
  const cats = $("emoji-cats");
  scroll.innerHTML = uiAssets.emojiPicker || "";
  cats.innerHTML = uiAssets.emojiCategories || "";
  renderRecentEmojis();
  emojiBuilt = true;
}

async function filterEmoji(query) {
  const scroll = $("emoji-scroll");
  const needle = query.trim();
  emojiQuery = needle.toLowerCase();
  const recents = $("emoji-recents");

  if (!needle) {
    scroll.querySelectorAll(".emoji-title, .emoji-grid").forEach((node) => {
      node.hidden = false;
    });
    const stale = scroll.querySelector(".emoji-grid.search-results");
    if (stale) stale.remove();
    recents.hidden = false;
    return;
  }

  recents.hidden = true;
  scroll.querySelectorAll(".emoji-title, .emoji-grid").forEach((node) => {
    node.hidden = true;
  });
  const html = await invoke("ui_emoji_search", { query: needle, limit: 240 });
  if (emojiQuery !== needle.toLowerCase()) return;
  let grid = scroll.querySelector(".emoji-grid.search-results");
  if (!grid) {
    grid = document.createElement("div");
    grid.className = "emoji-grid search-results";
    scroll.appendChild(grid);
  }
  grid.hidden = false;
  grid.innerHTML = html.replace(/^<div[^>]*>|<\/div>$/g, "");
}

function scrollToEmojiGroup(index) {
  const title = $("emoji-scroll").querySelector(`.emoji-title[data-group="${index}"]`);
  if (title) title.scrollIntoView({ block: "start" });
  setActiveEmojiGroup(index);
}

function setActiveEmojiGroup(index) {
  emojiActiveGroup = index;
  $("emoji-cats")
    .querySelectorAll(".emoji-cat")
    .forEach((node) => node.classList.toggle("active", node.dataset.group === String(index)));
}

function bindEmojiScroll() {
  const scroll = $("emoji-scroll");
  let groupScrollQueued = false;
  scroll.addEventListener(
    "scroll",
    () => {
      if (emojiQuery || groupScrollQueued) return;
      // The category maths is wa_ui::emoji::active_group; ask once per frame
      // instead of per scroll event.
      groupScrollQueued = true;
      // setTimeout rather than requestAnimationFrame: rAF is suspended while the
      // window is hidden/occluded, which would leave the flag stuck on.
      setTimeout(async () => {
        groupScrollQueued = false;
        if (emojiQuery) return;
        const titles = [...scroll.querySelectorAll(".emoji-title")];
        const current = await invoke("ui_emoji_active_group", {
          offsets: titles.map((title) => [Number(title.dataset.group), title.offsetTop]),
          scrollTop: scroll.scrollTop,
        }).catch(() => emojiActiveGroup);
        if (current !== emojiActiveGroup) setActiveEmojiGroup(current);
      }, 50);
    },
    { passive: true },
  );
}

let reactionPickMessage = null;

function toggleEmoji(force) {
  const panel = $("emoji-panel");
  const show = force ?? panel.hidden;
  panel.hidden = !show;
  if (!show) {
    reactionPickMessage = null;
    return;
  }

  buildEmojiPicker();
  if (!panel.dataset.bound) {
    panel.dataset.bound = "1";
    bindEmojiScroll();
  }
  $("emoji-scroll").scrollTop = 0;
  if (currentPickerTab() === "stickers") reloadStickerView();
  else if (currentPickerTab() === "emoji") $("emoji-search").focus();
}

function currentPickerTab() {
  const active = $("emoji-panel").querySelector(".picker-tab.active");
  return active ? active.dataset.tab : "emoji";
}

async function setPickerTab(tab) {
  // Which view a tab shows (and its focus/reload behaviour) is
  // wa_ui::emoji::picker_tab.
  const plan = await invoke("ui_picker_tab", { tab }).catch(() => null);
  if (!plan) return;
  $("emoji-panel")
    .querySelectorAll(".picker-tab")
    .forEach((node) => node.classList.toggle("active", node.dataset.tab === tab));
  $("emoji-view").hidden = !plan.emoji_view;
  $("sticker-view").hidden = !plan.sticker_view;
  $("gif-view").hidden = !plan.gif_view;
  if (plan.reload_stickers) reloadStickerView();
  else if (plan.focus) $(plan.focus).focus();
}

async function insertEmoji(emoji) {
  const input = $("input");
  const start = input.selectionStart ?? input.value.length;
  const end = input.selectionEnd ?? start;
  // Replacing the selection and the new caret are wa_ui::emoji::insert_text.
  const plan = await invoke("ui_insert_text", {
    value: input.value,
    start,
    end,
    insert: emoji,
  }).catch(() => null);
  if (plan) {
    input.value = plan.value;
    input.setSelectionRange(plan.caret, plan.caret);
  }
  input.focus();
  autosize(input);
  updateSendState();
  startTyping();
  rememberEmoji(emoji);
}
