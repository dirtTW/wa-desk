/* WA Desk frontend: stickers. Loaded in order by index.html; all files share the classic-script global scope. */

/* ---------------------------------------------------------------- stickers */

const STICKER_RECENT_LIMIT = 80;
const STICKER_LOAD_CONCURRENCY = 16;

let stickerDirty = true;
let stickerLoadToken = 0;
let aiStickerSyncTimer = 0;

function syncAiStickers(immediate = false) {
  clearTimeout(aiStickerSyncTimer);
  const push = () => {
    const ids = recentStickerMessages()
      .filter((message) => state.media.get(message.id)?.status === "ready")
      .slice(0, 80)
      .map((message) => message.id);
    invoke("set_ai_stickers", { ids }).catch(() => {});
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
function syncAiContext(chat) {
  const entry = state.chats.get(chat);
  if (!entry) return;
  const lines = entry.messages
    .filter((message) => !message.deleted && WAChatUtils.isMessageVisible(message, false))
    .slice(-24)
    .map((message) => {
      const text = (message.text || "").trim();
      return {
        id: message.id,
        sender: message.from_me
          ? "Gw"
          : message.sender_name ||
            (message.sender ? WAChatUtils.phoneFromJid(message.sender) : "") ||
            "Mereka",
        text: text || (message.kind !== "text" ? `[${MEDIA_LABELS[message.kind] || "Media"}]` : ""),
        timestamp: message.timestamp || 0,
      };
    })
    .filter((line) => line.id && line.text);
  if (lines.length === 0) return;
  invoke("set_ai_context", { chat, lines }).catch(() => {});
}

// Recent stickers are the sticker messages in the local cache; tapping one
// forwards it, which is how WhatsApp's recent-sticker row behaves.
function recentStickerMessages() {
  const byId = new Map();
  for (const chat of state.chats.values()) {
    for (const message of chat.messages) {
      if (message.kind !== "sticker") continue;
      if (String(message.id).startsWith("local-")) continue;
      const existing = byId.get(message.id);
      if (!existing || existing.timestamp < message.timestamp) byId.set(message.id, message);
    }
  }
  return [...byId.values()]
    .filter((message) => !message.pending)
    .sort((a, b) => b.timestamp - a.timestamp)
    .slice(0, STICKER_RECENT_LIMIT);
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

function stickerCreateCell() {
  return `<button type="button" class="sticker-cell sticker-create" data-sticker-create="1">
    <svg viewBox="0 -960 960 960" aria-hidden="true"><path d="M440-440H200v-80h240v-240h80v240h240v80H520v240h-80v-240Z"/></svg>
    <span>Create</span>
  </button>`;
}

function filterStickerGrid(query) {
  const needle = String(query || "").trim().toLowerCase();
  for (const cell of $("sticker-grid").querySelectorAll(".sticker-cell")) {
    if (cell.dataset.stickerCreate) {
      cell.hidden = Boolean(needle);
      continue;
    }
    const haystack = (
      cell.dataset.search ||
      cell.dataset.favoriteId ||
      cell.dataset.stickerId ||
      ""
    ).toLowerCase();
    cell.hidden = Boolean(needle) && !haystack.includes(needle);
  }
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
  const grid = $("sticker-grid");
  grid.innerHTML = stickerCreateCell() + '<p class="empty">Loading favorite stickers from the linked session...</p>';
  $("sticker-count").textContent = "";
  try {
    const favorites = await invoke("fetch_favorite_stickers");
    if (!favorites.length) {
      grid.innerHTML =
        stickerCreateCell() +
        '<p class="empty">No favorite stickers found. Star stickers on your phone, then try again.</p>';
      return;
    }
    grid.innerHTML =
      stickerCreateCell() +
      favorites
        .map((favorite) => {
          const url = convertFileSrc ? convertFileSrc(favorite.path) : favorite.path;
          return `<div class="sticker-cell" data-favorite-id="${escapeHtml(favorite.id)}" data-search="favorite sticker"><img src="${escapeHtml(url)}" alt="Favorite sticker" loading="lazy" /></div>`;
        })
        .join("");
    $("sticker-count").textContent = `(${favorites.length})`;
    filterStickerGrid($("sticker-search")?.value || "");
  } catch (err) {
    grid.innerHTML = `<p class="empty">Could not load favorites: ${escapeHtml(String(err))}</p>`;
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
  const grid = $("sticker-grid");
  if (!force && !stickerDirty && grid.childElementCount > 0) return;

  const token = ++stickerLoadToken;
  const messages = recentStickerMessages();
  stickerDirty = false;
  $("sticker-count").textContent = messages.length ? `(${messages.length})` : "";

  if (messages.length === 0) {
    grid.innerHTML =
      stickerCreateCell() +
      '<p class="empty">No stickers yet. Received stickers appear here, or add a .webp file.</p>';
    filterStickerGrid($("sticker-search")?.value || "");
    return;
  }

  grid.innerHTML =
    stickerCreateCell() +
    messages
      .map(
        (message) =>
          `<div class="sticker-cell loading" data-sticker-id="${escapeHtml(message.id)}" data-search="recent sticker"></div>`,
      )
      .join("");

  await mapLimit(messages, STICKER_LOAD_CONCURRENCY, async (message) => {
    const url = await stickerUrl(message);
    if (token !== stickerLoadToken) return;
    const cell = grid.querySelector(
      `[data-sticker-id="${CSS.escape(message.id)}"]`,
    );
    if (!url) {
      if (cell) cell.remove();
      return;
    }
    if (cell) {
      cell.outerHTML = `<div class="sticker-cell" data-sticker-id="${escapeHtml(message.id)}" data-search="recent sticker"><img src="${escapeHtml(url)}" alt="Sticker" loading="lazy" /></div>`;
    }
  });

  if (token !== stickerLoadToken) return; // another load superseded this one
  const loaded = grid.querySelectorAll(".sticker-cell img").length;
  $("sticker-count").textContent = loaded ? `(${loaded})` : "";
  syncAiStickers(true);
  if (loaded === 0) {
    grid.innerHTML =
      stickerCreateCell() +
      '<p class="empty">Stickers could not be loaded right now.</p>';
  }
  filterStickerGrid($("sticker-search")?.value || "");
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

