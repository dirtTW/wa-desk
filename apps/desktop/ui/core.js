/* WA Desk frontend: core. Loaded in order by index.html; all files share the classic-script global scope. */

/* WA Desk frontend.
 *
 * The Rust side owns all WhatsApp state. This file is a thin view:
 *   - receives normalized events on the `wa:event` channel
 *   - calls a few Tauri commands to send messages / link the account
 * Chats are kept in memory only; the protocol store on disk holds the session.
 */

const tauri = window.__TAURI__;
const invoke = tauri?.core?.invoke;
const listen = tauri?.event?.listen;

const $ = (id) => document.getElementById(id);

// WebView2 scales the entire UI on Ctrl+wheel / Ctrl +/-; a desktop client
// should keep a fixed scale, otherwise the rail and panels drift.
document.addEventListener(
  "wheel",
  (event) => {
    if (event.ctrlKey) event.preventDefault();
  },
  { passive: false },
);
document.addEventListener("keydown", (event) => {
  if (event.ctrlKey && ["+", "-", "=", "0"].includes(event.key)) {
    event.preventDefault();
  }
});

const STORE_KEY = "wadesk.state.v1";
const MAX_MESSAGES_PER_CHAT = 300; // persisted per chat
const MAX_MESSAGES_IN_MEMORY = 250; // per chat
const MAX_MESSAGES_TOTAL = 20000; // whole session
const MAX_MEDIA_URLS = 400; // cached sticker/photo URLs
const MAX_CHATS = 200;
const MAX_SEARCH_ROWS = 40; // per section; keeps the sidebar DOM bounded while typing

const state = {
  ready: false,
  linked: false,
  connected: false,
  me: null,
  meJid: null,
  meLid: null,
  chats: new Map(), // jid -> chat
  active: null,
  search: "",
  filter: "all",
  groupMeta: {}, // group jid -> { subject, isCommunity, parentJid, isAnnounce }
  communityView: null, // community jid while the community screen is open
  showDeleted: false,
  editing: null,
  photos: new Map(), // jid -> profile picture url
  photoQueue: [],
  photosInFlight: 0,
  photoFailed: new Set(),
  media: new Map(), // message id -> { status, url }
  thumbs: new Map(), // message id -> inline JPEG poster (data URI)
  videoPosters: new Map(), // video id -> frame sampled from the clip (data URI)
  thumbPending: new Set(), // ids currently asking the backend for a poster
  durations: new Map(), // video message id -> seconds
  durationPending: new Set(),
  posterPending: new Set(), // videos being sampled for a frame poster
  posterChecking: new Set(), // videos whose on-disk path is being looked up
  previews: new Map(), // url -> { status, data }
  ai: {
    enabled: false,
    baseUrl: "https://opencode.ai/zen/go/v1",
    model: "deepseek-v4.1-flash",
    visionModel: "",
    systemPrompt: "",
    requireTrigger: false,
    hasKey: false,
    keyHint: "",
  },
  contacts: new Map(), // user part -> saved contact name
  phones: new Map(), // LID -> resolved phone number
  starred: new Set(), // message ids starred on this or any linked device
  selectingChats: false, // sidebar multi-select ("Select chats")
  selectedChats: new Set(),
  starredOpen: false, // the "Starred messages" screen is showing
  statuses: new Map(), // sender jid -> { jid, name, updates }
  typingChat: null,
  typingTimer: null,
  localSeq: 0,
};

const convertFileSrc = tauri?.core?.convertFileSrc;

/* Rust-rendered UI assets (the `wa-ui` crate): the emoji catalogue, the picker
   markup and the Material icon set are built in Rust, so this file only holds
   them after one `ui_bootstrap` round-trip at startup. */
const uiAssets = { icons: [], emojiGroups: [], emojiPicker: "", emojiCategories: "" };

async function loadUiAssets() {
  try {
    const boot = await invoke("ui_bootstrap");
    uiAssets.icons = boot.icons || [];
    uiAssets.emojiGroups = boot.emoji_groups || [];
    uiAssets.emojiPicker = boot.emoji_picker || "";
    uiAssets.emojiCategories = boot.emoji_categories || "";
  } catch (err) {
    console.warn("UI assets unavailable", err);
  }
}

/* ------------------------------------------------------------------ */
/* local cache (survives restarts; WhatsApp only replays history once) */
/* ------------------------------------------------------------------ */

let saveTimer = null;
let historyBudget = 0;
let lastSavedFingerprint = null;

// Rendering calls `scheduleSave()` even when nothing changed, and the payload
// is ~1.8 MB of JSON. A cheap fingerprint of everything persisted lets a no-op
// save return immediately instead of re-serialising and rewriting the cache.
// The fingerprint/hash itself is computed by wa_ui::cache; this only exports
// the compact per-message summaries it hashes.
function fingerprintChats() {
  const chats = [];
  for (const chat of state.chats.values()) {
    const messages = [];
    for (const message of chat.messages) {
      messages.push({
        timestamp: message.timestamp || 0,
        text_len: message.text ? message.text.length : 0,
        status: message.status || "",
        edited: Boolean(message.edited),
        deleted: Boolean(message.deleted),
        reactions: message.reactions ? message.reactions.length : 0,
      });
    }
    chats.push({
      name: chat.name || "",
      unread: chat.unread || 0,
      deleted: Boolean(chat.deleted),
      cleared: Boolean(chat.cleared),
      messages,
    });
  }
  return chats;
}

async function stateFingerprint() {
  try {
    return await invoke("ui_cache_fingerprint", {
      chats: fingerprintChats(),
      active: state.active || "",
      photos: state.photos.size,
      starred: state.starred.size,
    });
  } catch {
    return "";
  }
}

function scheduleSave() {
  clearTimeout(saveTimer);
  saveTimer = setTimeout(saveNow, 1200);
}

let saveInFlight = false;

async function saveNow() {
  if (saveInFlight) return;
  saveInFlight = true;
  try {
    const fingerprint = await stateFingerprint();
    if (fingerprint && fingerprint === lastSavedFingerprint) return;
    // wa_ui::cache shapes the payload: newest chats first, the last N messages
    // per chat, link thumbnails stripped, only http photos persisted.
    const payload = await invoke("ui_cache_payload", {
      input: {
        chats: [...state.chats.values()].map((chat) => ({
          jid: chat.jid,
          name: chat.name,
          is_group: Boolean(chat.isGroup),
          last_ts: chat.last?.timestamp || chat.lastTs || 0,
          unread: chat.unread || 0,
          deleted: Boolean(chat.deleted),
          cleared: Boolean(chat.cleared),
          messages: chat.messages,
        })),
        active: state.active || "",
        group_meta: state.groupMeta || {},
        starred: [...state.starred],
        photos: [...state.photos],
        max_chats: MAX_CHATS,
        max_messages_per_chat: MAX_MESSAGES_PER_CHAT,
      },
    });
    localStorage.setItem(STORE_KEY, payload);
    // Mirror the metadata to disk; the webview store can be cleared or hit its
    // quota, and then the sidebar would otherwise lose every chat.
    persistChatCache();
    lastSavedFingerprint = fingerprint;
  } catch {
    /* quota exceeded: the cache is a convenience, not the source of truth */
  } finally {
    saveInFlight = false;
  }
}

async function restoreCache() {
  let raw = null;
  try {
    raw = localStorage.getItem(STORE_KEY);
  } catch {
    raw = null;
  }
  let restored = null;
  if (raw) {
    try {
      // wa_ui::cache filters broadcast rows, message stubs and stale photos.
      restored = await invoke("ui_cache_restore", { raw });
    } catch {
      restored = null;
    }
  }
  if (restored && Array.isArray(restored.chats) && restored.chats.length) {
    for (const chat of restored.chats) {
      if (!chat || !chat.jid) continue;
      const messages = Array.isArray(chat.messages) ? chat.messages : [];
      state.chats.set(chat.jid, {
        jid: chat.jid,
        name: chat.name || chat.jid.replace(/@.*/, ""),
        isGroup: Boolean(chat.is_group),
        messages,
        last: messages.length ? messages[messages.length - 1] : null,
        lastTs: chat.last_ts || 0,
        unread: chat.unread || 0,
        deleted: Boolean(chat.deleted),
        cleared: Boolean(chat.cleared),
      });
    }
    if (restored.group_meta && typeof restored.group_meta === "object") {
      state.groupMeta = restored.group_meta;
    }
    for (const id of restored.starred || []) {
      if (typeof id === "string" && id) state.starred.add(id);
    }
    for (const [jid, url] of restored.photos || []) {
      if (typeof url === "string" && url.startsWith("http")) state.photos.set(jid, url);
    }
  }

  // Never restore the "open" chat: after a restart the user is not looking at
  // it, and a restored active chat made incoming messages silently auto-read
  // (unread cleared, read receipts sent) plus suppressed their notifications.
  state.active = null;

  mergeDuplicateChats();
  renderChatList();
  renderChat();
  // Webview storage can be empty (fresh profile, quota reset); the disk mirror
  // still has the chat list, so a restart does not start from a blank sidebar.
  if (state.chats.size === 0) restoreDiskChatCache();
  requestChatHistory(state.active ? state.chats.get(state.active) : null);
  schedulePeerLookups(1500);
}

/* ------------------------------------------------------------------ */
/* helpers                                                             */
/* ------------------------------------------------------------------ */

// The user part of a JID. Chat state is keyed by phone (the address book's
// key), so the webview needs this one extraction; everything else about ids
// and names is decided by wa-ui.
function jidUser(jid) {
  const text = String(jid || "");
  const at = text.indexOf("@");
  const user = at < 0 ? text : text.slice(0, at);
  const colon = user.indexOf(":");
  return colon < 0 ? user : user.slice(0, colon);
}

function escapeHtml(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#39;");
}

function displayNameFromHandle(name) {
  const clean = String(name || "")
    .replace(/^@/, "")
    .replace(/_/g, " ")
    .trim();
  // The self chat is often labelled "You"; that is not a useful mention name.
  if (!clean || /^(you|me|unknown|whatsapp)$/i.test(clean)) return "";
  return clean.replace(/\b\w/g, (letter) => letter.toUpperCase());
}

// Keep the local identity in sync after pairing. `get_status` can return a
// null push name right after a fresh link, so fall back to the self-chat
// handle (`@alice` -> `Alice`).
function applyOwnIdentity(status) {
  if (status.jid) state.meJid = status.jid;
  if (status.lid) state.meLid = status.lid;
  if (status.me) state.me = status.me;
  if (!state.me) {
    const own = [...state.chats.values()].find(
      (chat) => chat.jid === state.meJid || chat.jid === state.meLid,
    );
    if (own?.name) state.me = displayNameFromHandle(own.name);
  }
  if (state.meJid || state.meLid) refreshMyAvatar();
  // The "Message yourself" chat only ever contains our own messages, so it can
  // never have unread items (older builds counted our own sends).
  for (const chat of state.chats.values()) {
    if (isSelfChat(chat.jid)) chat.unread = 0;
  }
}

/* -------------------------------------------------------------- date labels */

// `toLocaleTimeString` / `toLocaleDateString` are the slowest calls in a
// repaint — profiling showed them at ~47% of a sidebar refresh and ~35% of a
// chat refresh — and the same timestamps are formatted again on every render.
// Memoizing them is therefore the single biggest rendering win. Entries that
// depend on "today" are dropped when the calendar day changes.
const TIME_LABELS = new Map(); // timestamp -> "13:05"
const STAMP_LABELS = new Map(); // timestamp -> sidebar stamp
const DAY_LABELS = new Map(); // local day number -> "Today" / "4 Jul 2026"
const MAX_TIME_LABELS = 4000;
const MAX_STAMP_LABELS = 4000;
const MAX_DAY_LABELS = 400;

let labelDayStart = 0;
let labelDayChecked = 0;

// Cheap guard: the day-dependent caches are only rebuilt when the day may have
// rolled over, so a per-row call costs one numeric comparison.
function ensureDayLabels() {
  const now = Date.now();
  if (now - labelDayChecked < 60_000) return;
  labelDayChecked = now;
  const day = Math.floor(now / 86_400_000); // UTC day; early invalidation is harmless
  if (day !== labelDayStart) {
    labelDayStart = day;
    STAMP_LABELS.clear();
    DAY_LABELS.clear();
  }
}

function dayKey(date) {
  return date.getFullYear() * 10000 + (date.getMonth() + 1) * 100 + date.getDate();
}

function clockLabel(ms) {
  // The official client always renders "8:44 pm", English lowercase with a
  // colon — independent of the OS locale (which uses "20.47" in id-ID).
  return new Date(ms)
    .toLocaleTimeString("en-US", { hour: "numeric", minute: "2-digit", hour12: true })
    .toLowerCase();
}

function initials(name) {
  const clean = String(name || "?").trim();
  const parts = clean.split(/[\s._-]+/).filter(Boolean);
  const letters = parts.slice(0, 2).map((p) => p[0]);
  return (letters.join("") || clean[0] || "?").toUpperCase();
}

function isSelfChat(jid) {
  if (!jid) return false;
  const user = String(jid).split("@")[0].split(":")[0];
  return [state.meJid, state.meLid]
    .filter(Boolean)
    .some((self) => String(self).split("@")[0].split(":")[0] === user);
}

// The official avatar tints come from wa_ui::html::avatar_palette(); the hash
// stays local because setAvatar is synchronous.
let AVATAR_COLORS = [];

async function loadAvatarPalette() {
  try {
    AVATAR_COLORS = await invoke("ui_avatar_palette");
  } catch {
    AVATAR_COLORS = [];
  }
}

function avatarStyle(name) {
  if (!AVATAR_COLORS.length) return "background:#242626;color:#BDBDBD";
  let hash = 0;
  for (const ch of String(name || "?")) hash = (hash * 31 + ch.codePointAt(0)) >>> 0;
  const [bg, fg] = AVATAR_COLORS[hash % AVATAR_COLORS.length];
  return `background:${bg};color:${fg}`;
}

// WhatsApp Web draws a two-person glyph for a group with no photo instead of
// the sender initials it uses for a one-to-one chat.
// The two-person glyph comes from wa_ui::html::GROUP_AVATAR_ICON at boot.
let GROUP_AVATAR_ICON = "";
invoke("ui_group_avatar_icon")
  .then((svg) => {
    GROUP_AVATAR_ICON = svg;
  })
  .catch(() => {});

function isGroupJid(jid) {
  return typeof jid === "string" && jid.endsWith("@g.us");
}

function setAvatar(node, name, jid) {
  node.dataset.jid = jid || "";
  node.dataset.initials = initials(name);
  const url = jid ? state.photos.get(jid) : null;
  if (url) {
    node.classList.remove("group-avatar");
    node.setAttribute("style", "");
    node.innerHTML = `<img class="avatar-img" src="${escapeHtml(url)}" alt="" onerror="window.__waAvatarError(this)">`;
  } else if (isGroupJid(jid)) {
    node.classList.add("group-avatar");
    node.setAttribute("style", "");
    node.innerHTML = GROUP_AVATAR_ICON;
  } else {
    node.classList.remove("group-avatar");
    node.textContent = initials(name);
    node.setAttribute("style", avatarStyle(name));
  }
}

function refreshMyAvatar() {
  const jids = [state.meLid, state.meJid].filter(Boolean);
  if (!jids.length) return;
  // The photo may be cached under either the LID or the PN; prefer whichever
  // has one so the rail avatar does not fall back to initials unnecessarily.
  const jid = jids.find((value) => state.photos.get(value)) || jids[0];
  setAvatar($("my-avatar"), state.me || "me", jid);
  if (state.connected) {
    for (const value of jids) queuePhoto(value);
  }
}

window.__waAvatarError = function (img) {
  const holder = img.parentElement;
  if (!holder) return;
  const jid = holder.dataset.jid;
  if (jid) {
    // The cached URL is dead (expired CDN token): drop it so it is neither
    // reused nor persisted, fall back to the group glyph / initials, and give
    // the lookup another chance instead of hiding the photo for the session.
    state.photos.delete(jid);
    state.photoFailed.add(jid);
    setTimeout(() => {
      state.photoFailed.delete(jid);
      queuePhoto(jid);
    }, PHOTO_RETRY_MS);
  }
  const group = holder.classList.contains("group-avatar") || isGroupJid(jid);
  img.remove();
  if (group) {
    holder.classList.add("group-avatar");
    holder.setAttribute("style", "");
    holder.innerHTML = GROUP_AVATAR_ICON;
  } else {
    holder.textContent = holder.dataset.initials || "?";
    holder.setAttribute("style", avatarStyle(holder.dataset.initials || "?"));
  }
};

/* Profile pictures are fetched lazily, a few at a time, like WhatsApp Web. */

const PHOTO_CONCURRENCY = 3;
// A failed profile photo (expired CDN token, flaky link) is retried instead of
// being hidden for the rest of the session.
const PHOTO_RETRY_MS = 120_000;

// Ask Rust which of these candidates are worth a photo request, then queue
// them through the normal path (which keeps the in-flight bookkeeping).
async function queuePhotos(jids) {
  if (!state.connected || !jids || jids.length === 0) return;
  let allowed = [];
  try {
    allowed = await invoke("ui_photo_queue", {
      candidates: jids.filter(Boolean),
      connected: Boolean(state.connected),
      have: [...state.photos.keys()],
      failed: [...state.photoFailed],
      queued: state.photoQueue,
    });
  } catch {
    allowed = [];
  }
  for (const jid of allowed) queuePhoto(jid);
}

function queuePhoto(jid) {
  if (!jid || !state.connected) return;
  if (jid.startsWith("0@") || jid.endsWith("@broadcast")) return;
  if (state.photos.has(jid) || state.photoFailed.has(jid)) return;
  if (state.photoQueue.includes(jid)) return;
  state.photoQueue.push(jid);
  pumpPhotos();
}

function pumpPhotos() {
  if (!state.connected) return;
  while (state.photosInFlight < PHOTO_CONCURRENCY && state.photoQueue.length > 0) {
    const jid = state.photoQueue.shift();
    state.photosInFlight += 1;
    invoke("contact_photo", { chat: jid })
      .then((url) => {
        // Only real CDN URLs are cached; anything else (empty, a stale data:
        // blob) would be persisted and then fail to paint on the next start.
        if (typeof url === "string" && url.startsWith("http")) {
          state.photos.set(jid, url);
          // A photo only changes one avatar: patch the row (or the open chat
          // header) instead of rebuilding the whole sidebar per arrival.
          patchChatAvatar(jid);
          patchMessageAvatar(jid);
          if (state.active === jid) setAvatar($("chat-avatar"), state.chats.get(jid)?.name || "", jid);
          refreshMyAvatar();
          if (!$("status-screen").hidden) renderStatusList();
        } else {
          state.photoFailed.add(jid);
        }
      })
      .catch(() => state.photoFailed.add(jid))
      .finally(() => {
        state.photosInFlight -= 1;
        pumpPhotos();
      });
  }
}

// The media glyph comes from wa_ui::chat::media_icon.
async function mediaIcon(kind) {
  return invoke("ui_media_icon", { kind: kind || "image" }).catch(() => "");
}

function toast(message) {
  const node = $("toast");
  node.textContent = message;
  node.hidden = false;
  clearTimeout(toast.timer);
  toast.timer = setTimeout(() => {
    node.hidden = true;
  }, 4200);
}

/// `invoke` with a deadline, so a stalled backend call can never leave a
/// disabled Download button (or a silent spinner) behind forever.
function invokeWithTimeout(command, args, timeoutMs) {
  return Promise.race([
    invoke(command, args),
    new Promise((_, reject) => {
      setTimeout(() => reject(new Error("the request is taking too long")), timeoutMs);
    }),
  ]);
}

function buildLabel() {
  // The app version from `tauri.conf.json` (served by `get_status`), not the
  // UI cache-bust number.
  return `${state.version || "dev"} · ${document.title}`;
}

function showLoginError(message) {
  const node = $("login-error");
  if (!message) {
    node.hidden = true;
    node.textContent = "";
    return;
  }
  node.textContent = message;
  node.hidden = false;
}

function setConnection(label, kind) {
  const node = $("conn-state");
  node.textContent = label;
  node.className = `conn ${kind || ""}`;
  // WhatsApp Web only shows the connection line while it is not healthy.
  node.hidden = label === "connected";

  const banner = $("banner");
  if (kind === "offline" && state.linked) {
    banner.textContent = `Connecting... (${label})`;
    banner.hidden = false;
  } else {
    banner.hidden = true;
  }
}

// The transport can reconnect without the webview seeing a new `connected`
// event (the event is emitted before the listener is ready, or a stale
// `disconnected` arrives after the socket recovered). Reconcile with the
// backend while the UI believes it is offline instead of leaving a permanent
// "reconnecting" banner.
let connectionWatch = null;

function stopConnectionWatch() {
  if (connectionWatch) {
    clearInterval(connectionWatch);
    connectionWatch = null;
  }
}

function watchConnection() {
  if (connectionWatch) return;
  connectionWatch = setInterval(async () => {
    try {
      const status = await invoke("get_status");
      state.linked = status.linked;
      state.connected = status.connected;
      if (status.linked && status.connected) {
        stopConnectionWatch();
        setConnection("connected", "online");
        // Photos refused while the socket was down are worth another try.
        state.photoFailed.clear();
        renderChatList();
        if (state.active) renderChat();
        refreshMyAvatar();
        maybeUnlock();
        return;
      }
      if (!status.linked) {
        stopConnectionWatch();
        setConnection("waiting for link", "");
      }
    } catch {
      /* transient IPC failure: keep polling */
    }
  }, 2000);
}

