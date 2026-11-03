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
function stateFingerprint() {
  let hash = 0;
  let messageCount = 0;
  for (const chat of state.chats.values()) {
    let acc = 0;
    for (const message of chat.messages) {
      acc =
        (acc * 31 +
          (message.timestamp || 0) +
          (message.text ? message.text.length : 0) +
          (message.status ? message.status.charCodeAt(0) : 0) +
          (message.edited ? 1 : 0) +
          (message.deleted ? 1 : 0) +
          (message.reactions ? message.reactions.length : 0)) |
        0;
    }
    messageCount += chat.messages.length;
    hash =
      (hash * 31 +
        acc +
        (chat.unread || 0) +
        (chat.name ? chat.name.length : 0) +
        (chat.deleted ? 1 : 0) +
        (chat.cleared ? 1 : 0)) |
      0;
  }
  return `${state.chats.size}:${messageCount}:${state.active || ""}:${state.photos.size}:${state.starred.size}:${hash}`;
}

function scheduleSave() {
  clearTimeout(saveTimer);
  saveTimer = setTimeout(saveNow, 1200);
}

function saveNow() {
  const fingerprint = stateFingerprint();
  if (fingerprint === lastSavedFingerprint) return;
  try {
    const chats = [...state.chats.values()]
      .sort((a, b) => (b.last?.timestamp || b.lastTs || 0) - (a.last?.timestamp || a.lastTs || 0))
      .slice(0, MAX_CHATS)
      .map((chat) => ({
        jid: chat.jid,
        name: chat.name,
        isGroup: chat.isGroup,
        lastTs: chat.last?.timestamp || chat.lastTs || 0,
        unread: chat.unread,
        deleted: Boolean(chat.deleted),
        cleared: Boolean(chat.cleared),
        messages: chat.messages.slice(-MAX_MESSAGES_PER_CHAT).map((message) =>
          message.link?.thumb
            ? { ...message, link: { ...message.link, thumb: null } }
            : message,
        ),
      }));
    localStorage.setItem(
      STORE_KEY,
      JSON.stringify({
        v: 1,
        chats,
        active: state.active,
        groupMeta: state.groupMeta || {},
        starred: [...state.starred],
        photos: Object.fromEntries(
          [...state.photos].filter(([, url]) => typeof url === "string" && url.startsWith("http")),
        ),
      }),
    );
    // Mirror the metadata to disk; the webview store can be cleared or hit its
    // quota, and then the sidebar would otherwise lose every chat.
    persistChatCache();
    lastSavedFingerprint = fingerprint;
  } catch {
    /* quota exceeded: the cache is a convenience, not the source of truth */
  }
}

function restoreCache() {
  let parsed = null;
  try {
    parsed = JSON.parse(localStorage.getItem(STORE_KEY) || "null");
  } catch {
    parsed = null;
  }
  if (parsed && Array.isArray(parsed.chats)) {
    for (const chat of parsed.chats) {
      if (!chat || !chat.jid) continue;
      // Status/broadcast conversations are not chats; channels are kept and
      // shown in their own panel.
      if (chat.jid.endsWith("@broadcast")) continue;
      const messages = (Array.isArray(chat.messages) ? chat.messages : [])
        // Drop protocol/notification stubs written by older builds: they have no
        // text and no media type, and would render as empty bubbles.
        .filter((m) => m && (m.text || (m.kind && m.kind !== "other")))
        .map((m) => ({ ...m, pending: false }));
      state.chats.set(chat.jid, {
        jid: chat.jid,
        name: chat.name || chat.jid.replace(/@.*/, ""),
        isGroup: Boolean(chat.isGroup),
        messages,
        last: messages.length ? messages[messages.length - 1] : null,
        lastTs: chat.lastTs || 0,
        unread: chat.unread || 0,
        deleted: Boolean(chat.deleted),
        cleared: Boolean(chat.cleared),
      });
    }

    // Never restore the "open" chat: after a restart the user is not looking
    // at it, and a restored active chat made incoming messages silently
    // auto-read (unread cleared, read receipts sent) plus suppressed their
    // notifications. The user picks a chat; the list starts clean.
    state.active = null;
    if (parsed.groupMeta && typeof parsed.groupMeta === "object") {
      state.groupMeta = parsed.groupMeta;
    }
    if (Array.isArray(parsed.starred)) {
      for (const id of parsed.starred) {
        if (typeof id === "string" && id) state.starred.add(id);
      }
    }
    if (parsed.photos && typeof parsed.photos === "object") {
      for (const [jid, url] of Object.entries(parsed.photos)) {
        if (typeof url === "string" && url.startsWith("http")) state.photos.set(jid, url);
      }
    }
  }

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

// Deterministic colour per contact, like WhatsApp's default avatar palette.
// Each entry is the official `--WDS-components-profile-photo-surface-*` /
// `-content-*` pair (the dark-theme tinted circle plus its letter colour).
const AVATAR_COLORS = [
  ["#35271E", "#DBA685"], // brown
  ["#092642", "#53A6FD"], // cobalt
  ["#242626", "#BDBDBD"], // gray
  ["#103529", "#25D366"], // green
  ["#35221E", "#FC9775"], // orange
  ["#36192A", "#FF72A1"], // pink
  ["#242447", "#A791FF"], // purple
  ["#321622", "#FB5061"], // red
  ["#092C3D", "#53BDEB"], // sky blue
  ["#092D2F", "#42C7B8"], // teal
  ["#362C1F", "#FFD279"], // yellow
];

function avatarStyle(name) {
  let hash = 0;
  for (const ch of String(name || "?")) hash = (hash * 31 + ch.codePointAt(0)) >>> 0;
  const [bg, fg] = AVATAR_COLORS[hash % AVATAR_COLORS.length];
  return `background:${bg};color:${fg}`;
}

// WhatsApp Web draws a two-person glyph for a group with no photo instead of
// the sender initials it uses for a one-to-one chat.
const GROUP_AVATAR_ICON =
  '<svg class="avatar-icon" viewBox="0 -960 960 960" aria-hidden="true"><path d="M40-160v-112q0-34 17.5-62.5T104-378q62-31 126-46.5T360-440q66 0 130 15.5T616-378q29 15 46.5 43.5T680-272v112H40Zm720 0v-120q0-44-24.5-84.5T666-434q51 6 96 20.5t84 35.5q36 20 55 44.5t19 53.5v120H760ZM247-527q-47-47-47-113t47-113q47-47 113-47t113 47q47 47 47 113t-47 113q-47 47-113 47t-113-47Zm466 0q-47 47-113 47-11 0-28-2.5t-28-5.5q27-32 41.5-71t14.5-81q0-42-14.5-81T544-792q14-5 28-6.5t28-1.5q66 0 113 47t47 113q0 66-47 113ZM120-240h480v-32q0-11-5.5-20T580-306q-54-27-109-40.5T360-360q-56 0-111 13.5T140-306q-9 5-14.5 14t-5.5 20v32Zm296.5-343.5Q440-607 440-640t-23.5-56.5Q393-720 360-720t-56.5 23.5Q280-673 280-640t23.5 56.5Q327-560 360-560t56.5-23.5ZM360-240Zm0-400Z"/></svg>';

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

function mediaIcon(kind) {
  const icons = {
    image:
      '<svg viewBox="0 -960 960 960"><path d="M200-120q-33 0-56.5-23.5T120-200v-560q0-33 23.5-56.5T200-840h560q33 0 56.5 23.5T840-760v560q0 33-23.5 56.5T760-120H200Zm0-80h560v-560H200v560Zm40-80h480L570-480 450-320l-90-120-120 160Zm-40 80v-560 560Z"/></svg>',
    video:
      '<svg viewBox="0 -960 960 960"><path d="M160-160q-33 0-56.5-23.5T80-240v-480q0-33 23.5-56.5T160-800h480q33 0 56.5 23.5T720-720v180l160-160v440L720-420v180q0 33-23.5 56.5T640-160H160Zm0-80h480v-480H160v480Zm0 0v-480 480Z"/></svg>',
    audio:
      '<svg viewBox="0 -960 960 960"><path d="M395-435q-35-35-35-85v-240q0-50 35-85t85-35q50 0 85 35t35 85v240q0 50-35 85t-85 35q-50 0-85-35Zm85-205Zm-40 520v-123q-104-14-172-93t-68-184h80q0 83 58.5 141.5T480-320q83 0 141.5-58.5T680-520h80q0 105-68 184t-172 93v123h-80Zm68.5-371.5Q520-503 520-520v-240q0-17-11.5-28.5T480-800q-17 0-28.5 11.5T440-760v240q0 17 11.5 28.5T480-480q17 0 28.5-11.5Z"/></svg>',
    document:
      '<svg viewBox="0 -960 960 960"><path d="M320-240h320v-80H320v80Zm0-160h320v-80H320v80ZM240-80q-33 0-56.5-23.5T160-160v-640q0-33 23.5-56.5T240-880h320l240 240v480q0 33-23.5 56.5T720-80H240Zm280-520v-200H240v640h480v-440H520ZM240-800v200-200 640-640Z"/></svg>',
    sticker:
      '<svg viewBox="0 -960 960 960"><path d="M460-360q69 0 120-45t60-113l-320 90q26 32 62 50t78 18ZM294-510l106-30q4-28-14-49t-46-21q-25 0-42.5 17.5T280-550q0 11 4 21t10 19Zm240-70 106-30q5-28-13.5-49T580-680q-25 0-42.5 17.5T520-620q0 11 4 21t10 19Zm106 460H200q-33 0-56.5-23.5T120-200v-560q0-33 23.5-56.5T200-840h560q33 0 56.5 23.5T840-760v440L640-120Zm-40-80v-80q0-33 23.5-56.5T680-360h80v-400H200v560h400Zm0 0Zm-400 0v-560 560Z"/></svg>',
  };
  return icons[kind] || icons.document;
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

