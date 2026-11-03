/* WA Desk frontend: status. Loaded in order by index.html; all files share the classic-script global scope. */

/* ------------------------------------------------------------------ */
/* status (stories)                                                    */
/* ------------------------------------------------------------------ */

const STATUS_KEY = "wadesk.statuses.v1";
const STATUS_TTL_MS = 24 * 60 * 60 * 1000;

let statusQueue = [];
let statusIndex = 0;
let statusGroups = [];
let statusGroupIndex = 0;
let statusTimer = null;
let statusViewTab = "recent";
let statusDraftFile = null;
let statusDraftUrl = null;
const statusLocalMedia = new Map();
let statusMuted = false;
let lastStatusSync = 0;

function normalizeStatusJid(jid) {
  // Status authors can arrive with a device suffix (`123:98@lid`); status
  // entries and message keys use the bare JID.
  return String(jid || "").replace(/:\d+@/, "@");
}

function loadStatuses() {
  try {
    const parsed = JSON.parse(localStorage.getItem(STATUS_KEY) || "{}");
    for (const [rawJid, entry] of Object.entries(parsed)) {
      if (!entry || !Array.isArray(entry.updates)) continue;
      const jid = normalizeStatusJid(rawJid);
      const updates = entry.updates
        .filter((u) => Date.now() - u.timestamp < STATUS_TTL_MS)
        .map((u) => ({
          ...u,
          sender: normalizeStatusJid(u.sender || jid),
          viewed: Boolean(u.viewed),
          from_me: Boolean(u.from_me),
          // Drop viewer rows a previous build may have stored for the
          // broadcast chat or a newsletter.
          viewers: Array.isArray(u.viewers)
            ? u.viewers.filter(
                (viewer) =>
                  viewer &&
                  viewer.jid &&
                  !viewer.jid.endsWith("@broadcast") &&
                  !viewer.jid.endsWith("@newsletter"),
              )
            : undefined,
        }));
      if (!updates.length) continue;
      const existing = state.statuses.get(jid);
      if (existing) {
        const byId = new Map(existing.updates.map((u) => [u.id, u]));
        for (const update of updates) byId.set(update.id, update);
        existing.updates = [...byId.values()];
        if (entry.name) existing.name = entry.name;
      } else {
        state.statuses.set(jid, { jid, name: entry.name || jid, updates });
      }
    }
  } catch {
    /* ignore a corrupt cache */
  }
  syncOwnStatusIds();
}

/// Push the ids of our own stories to the session, so a view receipt can be
/// attributed to one of them (the session cannot know them otherwise).
let syncedStatusIds = new Set();

function syncOwnStatusIds() {
  const ids = [];
  for (const entry of state.statuses.values()) {
    for (const update of entry.updates) {
      if (update.from_me && update.id && !syncedStatusIds.has(update.id)) ids.push(update.id);
    }
  }
  if (!ids.length) return;
  for (const id of ids) syncedStatusIds.add(id);
  invoke("watch_status_ids", { ids }).catch(() => {
    // The session may not be up yet; drop them so the next sync retries.
    for (const id of ids) syncedStatusIds.delete(id);
  });
}

function saveStatuses() {
  try {
    const payload = {};
    for (const [jid, entry] of state.statuses) payload[jid] = entry;
    localStorage.setItem(STATUS_KEY, JSON.stringify(payload));
  } catch {
    /* quota */
  }
}

function latestStatus(entry) {
  return Math.max(0, ...entry.updates.map((update) => update.timestamp));
}

/// The status update with this id, when we track it.
function statusUpdateById(id) {
  if (!id) return null;
  for (const entry of state.statuses.values()) {
    const update = entry.updates.find((item) => item.id === id);
    if (update) return update;
  }
  return null;
}

/// Replies and reactions to a status used to be stored as plain replies, with
/// the quoted status recovered as a phantom chat bubble (reply-quote recovery
/// did not know a status is not a chat message). Both heal as soon as the quote
/// target is a status update we track: the message wears the official
/// "· Status" card and the phantom bubble disappears.
function healStatusInteractions() {
  let changed = false;
  for (const chat of state.chats.values()) {
    for (const message of chat.messages) {
      if (message.status_quote || !message.reply_to?.id) continue;
      if (!statusUpdateById(message.reply_to.id)) continue;
      message.status_quote = true;
      changed = true;
    }
    const kept = chat.messages.filter(
      (message) => !(message.once_recovered && statusUpdateById(message.id)),
    );
    if (kept.length !== chat.messages.length) {
      chat.messages = kept;
      chat.last = kept.length ? kept[kept.length - 1] : null;
      chat.lastTs = chat.last ? chat.last.timestamp : chat.lastTs;
      changed = true;
    }
  }
  if (!changed) return;
  scheduleSave();
  renderChatList();
  renderChat();
}

function isMyStatus(entry) {
  return (
    entry.updates.some((update) => update.from_me) ||
    entry.jid === state.meJid ||
    entry.jid === state.meLid
  );
}

function setStatusTab(tab) {
  statusViewTab = tab;
  document
    .querySelectorAll(".status-tab")
    .forEach((node) => node.classList.toggle("active", node.dataset.statusTab === tab));
  $("status-recent-section").hidden = tab !== "recent";
  $("status-viewed-section").hidden = tab !== "viewed";
}

function statusEntries() {
  return [...state.statuses.values()]
    .map((entry) => ({
      ...entry,
      updates: entry.updates.filter((update) => Date.now() - update.timestamp < STATUS_TTL_MS),
    }))
    .filter((entry) => entry.updates.length > 0)
    .sort((a, b) => latestStatus(b) - latestStatus(a));
}

function openStatus() {
  $("settings-screen").hidden = true;
  $("channels-discover").hidden = true;
  $("status-screen").hidden = false;
  $("new-chat-screen").hidden = true;
  $("group-info-screen").hidden = true;
  $("channels-screen").hidden = true;
  $("chat-panel").hidden = true;
  $("chat-empty").hidden = true;
  $("status-share").hidden = false;
  closeMessageMenu();
  setRailActive("btn-status");
  setStatusTab("recent");
  renderStatusList();
  // Statuses received while the app was closed live in status@broadcast
  // history. Ask the linked session once per couple of minutes; the response
  // arrives through the normal `status` event path.
  if (Date.now() - lastStatusSync > 120_000) {
    lastStatusSync = Date.now();
    invoke("fetch_status_history")
      .then(() => {
        setTimeout(() => {
          if (!$("status-screen").hidden) renderStatusList();
        }, 1500);
      })
      .catch(() => {});
  }
}

function hideStatusShare() {
  $("status-share").hidden = true;
  $("chat-empty").hidden = false;
  renderChat();
}

function closeStatus() {
  $("status-screen").hidden = true;
  $("status-composer").hidden = true;
  resetStatusDraft();
  closeStatusTextComposer();
  closeStatusAddMenu();
  $("status-share").hidden = true;
  $("chat-empty").hidden = false;
  closeStatusViewer();
  setRailActive("rail-chats");
  renderChat();
}

function statusItemHtml(entry) {
  const latest = latestStatus(entry);
  const viewed = entry.updates.every((update) => update.viewed);
  const count = entry.updates.length;
  return `
    <div class="status-item${viewed ? " viewed" : ""}" data-status-jid="${escapeHtml(entry.jid)}">
      <div class="status-ring${viewed ? " viewed" : ""}">${avatarHtml(entry.name, entry.jid)}</div>
      <div class="status-meta">
        <strong>${escapeHtml(entry.name)}</strong>
        <span>${escapeHtml(dayOf(latest))} at ${timeOf(latest)}${count > 1 ? ` • ${count} updates` : ""}</span>
      </div>
    </div>`;
}

function renderStatusList() {
  setAvatar($("status-my-avatar"), state.me || "me", state.meLid || state.meJid);

  const rows = statusEntries();
  const mine = rows.filter(isMyStatus);
  const others = rows.filter((entry) => !isMyStatus(entry));

  // WhatsApp shows your latest story as the row thumbnail and its time as the
  // subtitle; the empty state keeps the "click to add" hint and your avatar.
  const mineUpdates = mine.flatMap((entry) => entry.updates);
  const latestMine = mineUpdates.sort((a, b) => b.timestamp - a.timestamp)[0] || null;
  const thumb = $("status-my-thumb");
  $("status-my-avatar").hidden = Boolean(latestMine);
  thumb.hidden = !latestMine;
  if (latestMine) {
    if (latestMine.kind === "text") {
      const colour = /^#[0-9a-f]{6}$/i.test(latestMine.background || "")
        ? latestMine.background
        : "#243640";
      thumb.style.background = colour;
      thumb.textContent = String(latestMine.text || "").slice(0, 48);
      thumb.classList.remove("has-image");
    } else {
      const url = statusLocalMedia.get(latestMine.id) || "";
      thumb.innerHTML =
        latestMine.kind === "video" ? waIcon("play_arrow") : "";
      thumb.style.background = url
        ? `center / cover no-repeat url("${url}")`
        : "var(--panel-3)";
      thumb.classList.toggle("has-image", Boolean(url));
    }
  }
  $("status-mine").querySelector(".status-meta span").textContent = latestMine
    ? `${dayOf(latestMine.timestamp)} at ${timeOf(latestMine.timestamp)}`
    : "Click to add status update";

  const recent = others.filter((entry) => entry.updates.some((update) => !update.viewed));
  const viewed = others.filter((entry) => entry.updates.every((update) => update.viewed));

  if (state.connected) {
    for (const entry of rows) queuePhoto(entry.jid);
  }

  // Rail feed: the status icon lights while unseen stories exist.
  if (typeof setRailStatusUnseen === "function") setRailStatusUnseen(recent.length);

  $("status-list").innerHTML = recent.length
    ? recent.map(statusItemHtml).join("")
    : '<p class="empty">No recent status updates.</p>';
  $("status-viewed-list").innerHTML = viewed.length
    ? viewed.map(statusItemHtml).join("")
    : '<p class="empty">No viewed status updates.</p>';
}

function addStatusUpdate(update) {
  const jid = normalizeStatusJid(update.sender);
  let entry = state.statuses.get(jid);
  if (!entry) {
    entry = { jid, name: update.name || jid, updates: [] };
    state.statuses.set(jid, entry);
  }
  if (update.name && !isPlaceholderName(jid, update.name)) entry.name = update.name;

  const normalized = {
    ...update,
    sender: jid,
    from_me: Boolean(update.from_me),
    viewed: Boolean(update.viewed) || Boolean(update.from_me),
  };
  // A re-delivered copy of our own story must not drop the viewers we already
  // recorded from "view" receipts.
  const previous = entry.updates.find((existing) => existing.id === normalized.id);
  if (previous?.viewers?.length) normalized.viewers = previous.viewers;
  entry.updates = entry.updates.filter((existing) => existing.id !== normalized.id);
  entry.updates.push(normalized);
  entry.updates = entry.updates.filter((existing) => Date.now() - existing.timestamp < STATUS_TTL_MS);

  saveStatuses();
  if (!$("status-screen").hidden) renderStatusList();
}

function openStatusViewer(jid) {
  statusGroups = statusEntries().filter((entry) => entry.updates.length);
  const index = statusGroups.findIndex((entry) => entry.jid === jid);
  if (index < 0) return;

  statusGroupIndex = index;
  statusQueue = statusGroups[index].updates
    .filter((update) => Date.now() - update.timestamp < STATUS_TTL_MS)
    .sort((a, b) => a.timestamp - b.timestamp);
  if (statusQueue.length === 0) return;

  statusIndex = statusQueue.findIndex((update) => !update.viewed);
  if (statusIndex < 0) statusIndex = 0;
  statusMuted = false;
  $("status-viewer").hidden = false;
  renderStatusFrame();
}

function loadStatusGroup(index, atEnd = false) {
  const group = statusGroups[index];
  if (!group) return false;
  const updates = group.updates
    .filter((update) => Date.now() - update.timestamp < STATUS_TTL_MS)
    .sort((a, b) => a.timestamp - b.timestamp);
  if (!updates.length) return false;
  statusGroupIndex = index;
  statusQueue = updates;
  statusIndex = atEnd ? updates.length - 1 : 0;
  return true;
}

function closeStatusViewer() {
  clearTimeout(statusTimer);
  statusTimer = null;
  clearTimeout(viewerVideoIdleTimer);
  stopStatusProgress();
  statusFrameToken += 1;
  statusPaused = false;
  statusFrameEndsAt = 0;
  statusFrameRemaining = 0;
  $("status-viewer").hidden = true;
  $("status-viewer").style.background = "";
  $("status-viewer-body").innerHTML = "";
  $("status-view-count").hidden = true;
  closeStatusViews();
  if (!$("status-screen").hidden) renderStatusList();
}

/// WhatsApp only offers the arrow that can actually go somewhere.
function updateStatusNav() {
  const atStart = statusGroupIndex <= 0 && statusIndex <= 0;
  const atEnd =
    statusGroupIndex >= statusGroups.length - 1 && statusIndex >= statusQueue.length - 1;
  $("status-prev").hidden = atStart;
  $("status-next").hidden = atEnd;
}

function stepStatus(delta) {
  clearTimeout(statusTimer);
  statusTimer = null;
  if (delta > 0) {
    if (statusIndex + 1 < statusQueue.length) {
      statusIndex += 1;
      renderStatusFrame();
      return;
    }
    if (loadStatusGroup(statusGroupIndex + 1, false)) {
      renderStatusFrame();
      return;
    }
    closeStatusViewer();
    return;
  }
  if (delta < 0) {
    if (statusIndex > 0) {
      statusIndex -= 1;
      renderStatusFrame();
      return;
    }
    if (loadStatusGroup(statusGroupIndex - 1, true)) renderStatusFrame();
  }
}

function markStatusViewed(update) {
  if (!update || update.from_me || update.viewed) return;
  update.viewed = true;
  saveStatuses();
  invoke("mark_status_viewed", { sender: update.sender, id: update.id }).catch(() => {});
}

function statusReplyVisible(update) {
  const own = Boolean(update?.from_me);
  $("status-reply-input").hidden = own;
  $("status-reply-send").hidden = own;
  $("status-heart").hidden = own;
  $("status-viewer").querySelectorAll(".status-react").forEach((node) => {
    node.hidden = own;
  });
  // Your own story has nothing to reply to: drop the footer bar so the colour
  // covers the whole window like WhatsApp Web, and show the view counter.
  $("status-viewer").classList.toggle("own-story", own);
  // WhatsApp desktop marks your own story with the green add badge.
  $("status-viewer-add").hidden = !own;
  renderStatusViewCount(update);
}

/* ------------------------------------------- who viewed my status */

function renderStatusViewCount(update) {
  const button = $("status-view-count");
  const own = Boolean(update?.from_me);
  button.hidden = !own;
  if (!own) {
    closeStatusViews();
    return;
  }
  const count = (update.viewers || []).length;
  $("status-view-count-label").textContent = String(count);
}

function closeStatusViews() {
  $("status-views").hidden = true;
}

function renderStatusViews() {
  const update = statusQueue[statusIndex];
  const viewers = [...(update?.viewers || [])].sort((a, b) => b.timestamp - a.timestamp);
  $("status-views-title").textContent = `Viewed by ${viewers.length}`;
  $("status-view-count-label").textContent = String(viewers.length);
  $("status-views-list").innerHTML = viewers.length
    ? viewers
        .map(
          (viewer) => `
        <div class="status-view-row">
          ${avatarHtml(viewer.name, viewer.jid)}
          <div class="status-view-meta">
            <strong>${escapeHtml(viewer.name)}</strong>
            <span>${escapeHtml(dayOf(viewer.timestamp))} at ${escapeHtml(timeOf(viewer.timestamp))}</span>
          </div>
        </div>`,
        )
        .join("")
    : '<p class="empty">No views recorded yet. WhatsApp only reports views that arrive while WA Desk is running.</p>';
  for (const viewer of viewers.slice(0, 20)) queuePhoto(viewer.jid);
}

function openStatusViews() {
  renderStatusViews();
  $("status-views").hidden = false;
}

/// A status "view" receipt arrived: remember who watched which update.
function noteStatusView(event) {
  if (!event || !event.id) return;
  for (const entry of state.statuses.values()) {
    const update = entry.updates.find((item) => item.id === event.id);
    if (!update) continue;
    const jid = normalizeStatusJid(event.jid);
    // The broadcast chat itself is not a viewer.
    if (!jid || jid.endsWith("@broadcast") || jid.endsWith("@newsletter")) return;
    update.viewers = update.viewers || [];
    if (update.viewers.some((viewer) => viewer.jid === jid)) return;
    update.viewers.push({
      jid,
      name: event.name || jid,
      timestamp: event.timestamp || Date.now(),
    });
    saveStatuses();
    if (!$("status-screen").hidden) renderStatusList();
    if (update.from_me) syncOwnStatusIds();
    if (!$("status-viewer").hidden) {
      renderStatusViewCount(update);
      if (!$("status-views").hidden) renderStatusViews();
    }
    return;
  }
}

/* ---------------------------------------- frame timing, pause and resume */

let statusPaused = false;
let statusFrameTotal = 0;
let statusFrameEndsAt = 0;
let statusFrameRemaining = 0;
// Bumped for every frame: a media callback that lands late can then tell it is
// talking about a frame the viewer has already left.
let statusFrameToken = 0;
let statusProgressRaf = null;

// WhatsApp gives text stories 5 s, which is short for reading. Ours runs longer
// (and the pause button freezes the bar for as long as you need).
const STATUS_TEXT_MS = 10_000;
const STATUS_IMAGE_MS = 8_000;
// While a status' media is still downloading the bar must keep moving: an empty
// active segment is the "black line" that appears in place of the progress.
// A video whose metadata has not arrived yet gets this placeholder duration,
// and the real length replaces it once `loadedmetadata` lands.
const STATUS_VIDEO_FALLBACK_MS = 20_000;
// The server no longer has the media — still show a moving bar for the frame.
const STATUS_MEDIA_FAILED_MS = 3_000;

function statusFrameDuration(update) {
  if (!update) return 0;
  if (update.kind === "video") return 0; // the video's own duration wins once known
  if (update.kind !== "text") return STATUS_IMAGE_MS;
  // Long stories need more time than a one-liner.
  const extra = Math.min(4000, Math.max(0, String(update.text || "").length - 80) * 40);
  return STATUS_TEXT_MS + extra;
}

function currentStatusBar() {
  return $("status-progress").children[statusIndex] || null;
}

// How far into the current frame we are, read from the same clock the advance
// timer uses, so the bar and the frame can never disagree.
function statusFrameElapsed() {
  if (!statusFrameEndsAt) return Math.max(0, statusFrameTotal - statusFrameRemaining);
  return Math.max(0, statusFrameTotal - Math.max(0, statusFrameEndsAt - performance.now()));
}

// Paint the fill as a scaled layer: sub-pixel, compositor-friendly, and free of
// the width-transition restarts that made the old bar stutter.
function paintStatusProgress(bar, total, remaining) {
  const fill = bar?.querySelector("i");
  if (!fill || !(total > 0)) return;
  const done = Math.max(0, Math.min(1, (total - remaining) / total));
  fill.style.transform = `scaleX(${done})`;
}

function stopStatusProgress() {
  if (statusProgressRaf) cancelAnimationFrame(statusProgressRaf);
  statusProgressRaf = null;
}

// One rAF loop drives the active segment for as long as its frame lasts.
function statusProgressTick() {
  statusProgressRaf = null;
  if (statusPaused || $("status-viewer").hidden || !(statusFrameTotal > 0)) return;
  const remaining = Math.max(0, statusFrameEndsAt - performance.now());
  paintStatusProgress(currentStatusBar(), statusFrameTotal, remaining);
  if (remaining > 0) statusProgressRaf = requestAnimationFrame(statusProgressTick);
}

function startStatusProgress() {
  stopStatusProgress();
  if (statusPaused || $("status-viewer").hidden || !(statusFrameTotal > 0)) return;
  paintStatusProgress(
    currentStatusBar(),
    statusFrameTotal,
    Math.max(0, statusFrameEndsAt - performance.now()),
  );
  statusProgressRaf = requestAnimationFrame(statusProgressTick);
}

// Anchor the active frame: `total` is its full duration, `remaining` how much of
// it is left. The bar, the advance timer and pause/resume all read this anchor.
function anchorStatusFrame(total, remaining, token = statusFrameToken) {
  if (token !== statusFrameToken) return;
  statusFrameTotal = Math.max(1, Math.round(total));
  statusFrameRemaining = Math.max(0, Math.round(remaining));
  statusFrameEndsAt = performance.now() + statusFrameRemaining;
  clearTimeout(statusTimer);
  statusTimer = null;
  if (!statusPaused && !$("status-viewer").hidden) {
    statusTimer = setTimeout(() => stepStatus(1), statusFrameRemaining);
  }
  startStatusProgress();
}

function updateStatusPauseButton() {
  const button = $("status-viewer-pause");
  if (!button) return;
  const frame = statusQueue[statusIndex];
  button.hidden = !frame;
  // Swap the glyph with a class: `hidden` on the SVGs was overridden by the
  // global `svg { display: block }` rule, which left both icons visible.
  button.classList.toggle("paused", statusPaused);
  button.title = statusPaused ? "Play" : "Pause";
}

function pauseStatusFrame() {
  if (statusPaused || $("status-viewer").hidden) return;
  const update = statusQueue[statusIndex];
  if (!update) return;
  statusPaused = true;
  statusFrameRemaining = statusFrameEndsAt
    ? Math.max(0, statusFrameEndsAt - performance.now())
    : statusFrameRemaining;
  statusFrameEndsAt = 0;
  clearTimeout(statusTimer);
  statusTimer = null;
  // The bar is painted from the clock, so freezing the clock freezes the bar
  // exactly where it was — no snapshot of the animated width needed.
  stopStatusProgress();
  const video = document.querySelector("#status-viewer-body video");
  if (video && !video.paused) {
    video.dataset.pausedByUser = "1";
    video.pause();
  }
  updateStatusPauseButton();
}

function resumeStatusFrame() {
  if (!statusPaused) return;
  statusPaused = false;
  const remaining = Math.max(1, statusFrameRemaining || statusFrameTotal || 1);
  anchorStatusFrame(statusFrameTotal || remaining, remaining);
  const video = document.querySelector("#status-viewer-body video");
  if (video?.dataset.pausedByUser) {
    delete video.dataset.pausedByUser;
    video.play().catch(() => {});
  }
  updateStatusPauseButton();
}

function toggleStatusPause() {
  if (statusPaused) resumeStatusFrame();
  else pauseStatusFrame();
}

async function renderStatusFrame() {
  const update = statusQueue[statusIndex];
  if (!update) {
    closeStatusViewer();
    return;
  }

  statusFrameToken += 1;
  const token = statusFrameToken;
  stopStatusProgress();
  clearTimeout(statusTimer);
  statusTimer = null;
  statusPaused = false;
  statusFrameTotal = statusFrameDuration(update);
  statusFrameEndsAt = 0;
  statusFrameRemaining = 0;
  updateStatusPauseButton();

  // Past segments are filled by CSS; the active one is painted by the rAF loop.
  $("status-progress").innerHTML = statusQueue
    .map((item, index) => `<span class="${index < statusIndex ? "done" : ""}"><i></i></span>`)
    .join("");

  $("status-viewer-name").textContent = update.from_me
    ? "You"
    : statusGroups[statusGroupIndex]?.name || update.name || "Status";
  $("status-viewer-time").textContent = `${dayOf(update.timestamp)} at ${timeOf(update.timestamp)}`;
  setAvatar(
    $("status-viewer-avatar"),
    statusGroups[statusGroupIndex]?.name || update.name || "Status",
    statusGroups[statusGroupIndex]?.jid || update.sender,
  );
  statusReplyVisible(update);
  markStatusViewed(update);
  updateStatusNav();

  const body = $("status-viewer-body");
  if (update.kind === "text") {
    // WhatsApp paints a text story across the whole window in the colour that
    // travelled with the message, not as a card on a black backdrop.
    const colour = /^#[0-9a-f]{6}$/i.test(update.background || "")
      ? update.background
      : "#243640";
    $("status-viewer").style.background = colour;
    body.innerHTML = `<div class="text-status">${escapeHtml(update.text || "")}</div>`;
    anchorStatusFrame(statusFrameTotal, statusFrameTotal, token);
    return;
  }
  $("status-viewer").style.background = "";

  body.innerHTML = '<div class="media-box">Loading status...</div>';
  // The bar starts with the frame, not with the media: waiting for a download
  // left the active segment empty (the black line) for as long as it took.
  const placeholder = update.kind === "video" ? STATUS_VIDEO_FALLBACK_MS : STATUS_IMAGE_MS;
  anchorStatusFrame(placeholder, placeholder, token);
  try {
    let url = statusLocalMedia.get(update.id);
    if (!url) {
      const path = await invoke("fetch_media", { id: update.id });
      url = convertFileSrc ? convertFileSrc(path) : path;
    }
    if ($("status-viewer").hidden || token !== statusFrameToken) return;
    if (update.kind === "video") {
      body.innerHTML = `<video src="${escapeHtml(url)}" autoplay playsinline></video>`;
      const video = body.querySelector("video");
      video.muted = statusMuted;
      video.addEventListener(
        "ended",
        () => {
          if (token !== statusFrameToken) return;
          clearTimeout(statusTimer);
          statusTimer = null;
          stepStatus(1);
        },
        { once: true },
      );
      const adoptDuration = () => {
        if (token !== statusFrameToken || !(video.duration > 0)) return;
        const total = Math.round(video.duration * 1000);
        // Keep the time already spent on the placeholder countdown, so the bar
        // never jumps backwards when the real length lands.
        anchorStatusFrame(total, Math.max(1, total - statusFrameElapsed()), token);
      };
      video.addEventListener("loadedmetadata", adoptDuration, { once: true });
      // A blob that is already decoded can have its metadata before the listener
      // is attached; without this the bar would never fill.
      if (video.readyState >= 1) adoptDuration();
      video.play().catch(() => {
        video.muted = true;
        video.play().catch(() => {});
      });
    } else {
      body.innerHTML = `<img src="${escapeHtml(url)}" alt="${escapeHtml(update.name)}" />`;
      // The countdown is already running from the frame's start; an image that
      // decodes late just keeps the same clock instead of restarting the bar.
    }
  } catch {
    if (token !== statusFrameToken) return;
    body.innerHTML = '<div class="text-status">Status media is no longer available.</div>';
    anchorStatusFrame(STATUS_MEDIA_FAILED_MS, STATUS_MEDIA_FAILED_MS, token);
  }
}

async function replyCurrentStatus() {
  const update = statusQueue[statusIndex];
  const input = $("status-reply-input");
  const text = input.value.trim();
  if (!update || !text || update.from_me) return;
  try {
    // The reply carries the status as its quote, like the official client, so
    // the DM shows the "· Status" card instead of a bare line of text.
    await invoke("reply_status", {
      sender: update.sender,
      id: update.id,
      text,
      statusText: update.kind === "text" ? update.text || "" : null,
    });
    input.value = "";
    toast("Reply sent");
  } catch (err) {
    toast(String(err));
  }
}

async function reactCurrentStatus(emoji) {
  const update = statusQueue[statusIndex];
  if (!update || update.from_me) return;
  try {
    await invoke("react_status", { sender: update.sender, id: update.id, emoji });
    toast(`Reacted ${emoji}`);
  } catch (err) {
    toast(String(err));
  }
}

async function deleteCurrentStatus() {
  const update = statusQueue[statusIndex];
  if (!update || !update.from_me) {
    toast("Only your own status can be deleted.");
    return;
  }
  if (!confirm("Delete this status update?")) return;
  try {
    await invoke("delete_status", { id: update.id });
    const entry = state.statuses.get(update.sender);
    if (entry) {
      entry.updates = entry.updates.filter((item) => item.id !== update.id);
      saveStatuses();
    }
    statusLocalMedia.delete(update.id);
    toast("Status deleted");
    stepStatus(1);
  } catch (err) {
    toast(String(err));
  }
}

function resetStatusDraft() {
  statusDraftFile = null;
  if (statusDraftUrl) {
    URL.revokeObjectURL(statusDraftUrl);
    statusDraftUrl = null;
  }
  $("status-preview").hidden = true;
  $("status-preview").innerHTML = "";
  $("status-text").value = "";
  $("status-file").value = "";
}

function openStatusComposer() {
  resetStatusDraft();
  $("status-error").hidden = true;
  $("status-composer").hidden = false;
  $("status-text").focus();
}

/* ------------------------------------------------ "My status" add menu */

// WhatsApp Web opens a two-item menu under the My status row instead of
// jumping straight into the editor.
function openStatusAddMenu() {
  const menu = $("status-add-menu");
  const rect = $("status-mine").getBoundingClientRect();
  menu.hidden = false;
  const width = menu.offsetWidth || 236;
  const height = menu.offsetHeight || 112;
  const left = Math.min(rect.left + 16, window.innerWidth - width - 12);
  const top = Math.min(rect.bottom + 6, window.innerHeight - height - 12);
  menu.style.left = `${Math.max(12, left)}px`;
  menu.style.top = `${Math.max(12, top)}px`;
}

function closeStatusAddMenu() {
  const menu = $("status-add-menu");
  if (menu) menu.hidden = true;
}

/* -------------------------------------- full-screen text status composer */

// The status text backgrounds the official composer cycles through. Read from
// the shipped client: the "Palette" button repaints the composer from this
// list, and the swatch colours match the 20-entry array in the official bundle
// (`#A62C71 … #243640` for dark, the pastel `#FF9485 … #9368CF` set for light).
const STATUS_BACKGROUNDS = [
  "#A62C71",
  "#90A841",
  "#C1A03F",
  "#792138",
  "#AE8774",
  "#F0B330",
  "#B6B327",
  "#C69FCC",
  "#8B6990",
  "#FF8A8C",
  "#54C265",
  "#FF7B6B",
  "#26C4DC",
  "#57C9FF",
  "#74676A",
  "#7E90A3",
  "#5696FF",
  "#6E257E",
  "#7ACBA5",
  "#243640",
];

// The four fonts WhatsApp cycles through with the "Aa" button, in the wire
// names `post_status_text` maps onto the protobuf font enum.
const STATUS_FONTS = [
  "inherit",
  "Georgia, 'Times New Roman', serif",
  "'Segoe Script', 'Comic Sans MS', cursive",
  "Consolas, 'Courier New', monospace",
];
const STATUS_FONT_NAMES = ["system", "serif", "script", "bold"];

let statusBgIndex = 0;
let statusFontIndex = 0;

function openStatusTextComposer() {
  closeStatusAddMenu();
  const screen = $("status-text-screen");
  screen.hidden = false;
  $("status-emoji-panel").hidden = true;
  // Every new story starts on the first emoji category with an empty search.
  $("status-emoji-search").value = "";
  if ($("status-emoji-panel").dataset.built) {
    for (const [index, cat] of $("status-emoji-cats").querySelectorAll(".emoji-cat").entries()) {
      cat.classList.toggle("active", index === 0);
    }
    renderStatusEmojiGroup(0);
  }
  const input = $("status-text-input");
  input.value = "";
  // WhatsApp gives every new text status a random background from its palette;
  // the palette button then cycles to the next colour.
  applyStatusTextBackground(Math.floor(Math.random() * STATUS_BACKGROUNDS.length));
  applyStatusTextFont(statusFontIndex);
  resizeStatusTextInput();
  updateStatusTextSend();
  input.focus();
}

function closeStatusTextComposer() {
  $("status-text-screen").hidden = true;
  $("status-emoji-panel").hidden = true;
}

function applyStatusTextBackground(index) {
  const total = STATUS_BACKGROUNDS.length;
  statusBgIndex = ((index % total) + total) % total;
  $("status-text-screen").style.setProperty("--status-bg", STATUS_BACKGROUNDS[statusBgIndex]);
}

function applyStatusTextFont(index) {
  const total = STATUS_FONTS.length;
  statusFontIndex = ((index % total) + total) % total;
  $("status-text-screen").style.setProperty("--status-font", STATUS_FONTS[statusFontIndex]);
}

/// `#rrggbb` -> the opaque `0xAARRGGBB` integer the status protobuf carries.
function statusArgb(hex) {
  const clean = String(hex || "").replace("#", "");
  if (clean.length !== 6) return 0xff00a884;
  const value = Number.parseInt(clean, 16);
  return Number.isNaN(value) ? 0xff00a884 : (0xff000000 | value) >>> 0;
}

function resizeStatusTextInput() {
  const input = $("status-text-input");
  input.style.height = "auto";
  input.style.height = `${Math.min(input.scrollHeight, Math.round(window.innerHeight * 0.62))}px`;
}

function updateStatusTextSend() {
  $("status-text-send").disabled = !$("status-text-input").value.trim();
}

function buildStatusEmojiPanel() {
  const panel = $("status-emoji-panel");
  if (panel.dataset.built) return;
  const groups = uiAssets.emojiGroups || [];
  $("status-emoji-cats").innerHTML = groups
    .map(
      (group, index) =>
        `<button type="button" class="emoji-cat${index === 0 ? " active" : ""}" data-status-emoji-group="${index}" title="${escapeHtml(
          group.label,
        )}">${group.icon || "*"}</button>`,
    )
    .join("");
  renderStatusEmojiGroup(0);
  panel.dataset.built = "1";
}

/// Paint one WhatsApp emoji category (or the search results) into the panel.
function renderStatusEmojiGroup(index, query = "") {
  const groups = uiAssets.emojiGroups || [];
  const group = groups[index] || groups[0];
  const grid = $("status-emoji-grid");
  const needle = String(query || "").trim().toLowerCase();
  let items;
  let title;
  if (needle) {
    items = groups.flatMap((entry) =>
      entry.items.filter((item) => item.keywords.toLowerCase().includes(needle)),
    );
    title = `Search results`;
  } else {
    items = (group?.items || []).slice();
    // WhatsApp labels the first category "Smileys & People".
    title = index === 0 ? "Smileys & People" : group?.label || "Emoji";
  }
  $("status-emoji-title").textContent = needle && !items.length ? `No emoji match "${query}"` : title;
  grid.innerHTML = items
    .map(
      (item) =>
        `<button type="button" class="emoji" data-status-emoji="${item.char}" title="${escapeHtml(
          item.keywords,
        )}">${item.char}</button>`,
    )
    .join("");
  $("status-emoji-scroll").scrollTop = 0;
}

function filterStatusEmoji(query) {
  const needle = String(query || "").trim();
  if (!needle) {
    const active = $("status-emoji-cats").querySelector(".emoji-cat.active");
    renderStatusEmojiGroup(Number(active?.dataset.statusEmojiGroup || 0));
    return;
  }
  renderStatusEmojiGroup(0, needle);
}

function insertStatusEmoji(character) {
  const input = $("status-text-input");
  const start = input.selectionStart ?? input.value.length;
  const end = input.selectionEnd ?? start;
  input.value = input.value.slice(0, start) + character + input.value.slice(end);
  const caret = start + character.length;
  input.setSelectionRange(caret, caret);
  input.focus();
  resizeStatusTextInput();
  updateStatusTextSend();
}

async function postTextStatus() {
  const text = $("status-text-input").value.trim();
  if (!text) return;
  const send = $("status-text-send");
  send.disabled = true;
  // The colour and font travel with the status, so contacts see the same story.
  const background = STATUS_BACKGROUNDS[statusBgIndex];
  const font = STATUS_FONT_NAMES[statusFontIndex];
  try {
    const serverId = await invoke("post_status_text", {
      text,
      background: statusArgb(background),
      font,
    });
    addStatusUpdate({
      // Keep the server id: WhatsApp can only revoke the update it knows about.
      id: serverId || `local-${Date.now()}`,
      sender: state.meLid || state.meJid || "me",
      name: state.me || "Me",
      kind: "text",
      text,
      background,
      timestamp: Date.now(),
      from_me: true,
      viewed: true,
    });
    closeStatusTextComposer();
    toast("Status posted");
  } catch (err) {
    send.disabled = false;
    toast(String(err));
  }
}

function statusImageThumb(file) {
  return new Promise((resolve) => {
    const url = URL.createObjectURL(file);
    const img = new Image();
    img.onload = () => {
      const scale = Math.min(1, 320 / img.width, 320 / img.height);
      const canvas = document.createElement("canvas");
      canvas.width = Math.max(1, Math.round(img.width * scale));
      canvas.height = Math.max(1, Math.round(img.height * scale));
      canvas.getContext("2d").drawImage(img, 0, 0, canvas.width, canvas.height);
      URL.revokeObjectURL(url);
      resolve(canvas.toDataURL("image/jpeg", 0.72).split(",")[1] || "");
    };
    img.onerror = () => {
      URL.revokeObjectURL(url);
      resolve("");
    };
    img.src = url;
  });
}

function statusVideoMeta(file) {
  return new Promise((resolve) => {
    const url = URL.createObjectURL(file);
    const video = document.createElement("video");
    let settled = false;
    const finish = (thumb, duration) => {
      if (settled) return;
      settled = true;
      URL.revokeObjectURL(url);
      resolve({ thumb, duration: Math.max(1, Math.round(duration || 1)) });
    };
    const timer = setTimeout(() => finish("", video.duration || 1), 4000);
    video.muted = true;
    video.preload = "metadata";
    video.src = url;
    video.onloadeddata = () => {
      video.currentTime = Math.min(0.2, (video.duration || 1) / 2);
    };
    video.onseeked = () => {
      clearTimeout(timer);
      const scale = Math.min(1, 320 / video.videoWidth, 320 / video.videoHeight);
      const canvas = document.createElement("canvas");
      canvas.width = Math.max(1, Math.round(video.videoWidth * scale));
      canvas.height = Math.max(1, Math.round(video.videoHeight * scale));
      canvas.getContext("2d").drawImage(video, 0, 0, canvas.width, canvas.height);
      finish(canvas.toDataURL("image/jpeg", 0.72).split(",")[1] || "", video.duration || 1);
    };
    video.onerror = () => {
      clearTimeout(timer);
      finish("", 1);
    };
  });
}

async function postStatus() {
  const text = $("status-text").value.trim();
  const file = statusDraftFile;
  if (!file && !text) return;

  const error = $("status-error");
  error.hidden = true;

  try {
    if (file) {
      const isVideo = file.type.startsWith("video/");
      const data = await fileToBase64(file);
      const meta = isVideo
        ? await statusVideoMeta(file)
        : { thumb: await statusImageThumb(file), duration: 0 };
      await invoke("post_status_media", {
        data,
        mime: file.type || (isVideo ? "video/mp4" : "image/jpeg"),
        caption: text,
        thumb: meta.thumb,
        duration: meta.duration,
      });
      const localUrl = URL.createObjectURL(file);
      const id = `local-status-${Date.now()}`;
      statusLocalMedia.set(id, localUrl);
      addStatusUpdate({
        id,
        sender: state.meLid || state.meJid || "me",
        name: state.me || "Me",
        kind: isVideo ? "video" : "image",
        text,
        timestamp: Date.now(),
        from_me: true,
        viewed: true,
      });
    } else {
      await invoke("post_status_text", { text });
      addStatusUpdate({
        id: `local-${Date.now()}`,
        sender: state.meLid || state.meJid || "me",
        name: state.me || "Me",
        kind: "text",
        text,
        timestamp: Date.now(),
        from_me: true,
        viewed: true,
      });
    }
    resetStatusDraft();
    $("status-composer").hidden = true;
    toast("Status posted");
  } catch (err) {
    error.textContent = String(err);
    error.hidden = false;
  }
}

