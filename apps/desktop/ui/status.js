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

async function syncOwnStatusIds() {
  // Which updates are ours is decided by wa_ui::status::own_update_ids.
  const own = await invoke("ui_status_own_ids", { request: statusRequest() }).catch(
    () => [],
  );
  const ids = own.filter((id) => !syncedStatusIds.has(id));
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
async function healStatusInteractions() {
  const chats = [...state.chats.values()];
  if (!chats.length) return;
  const updateIds = [];
  for (const entry of state.statuses.values()) {
    for (const update of entry.updates) updateIds.push(update.id);
  }
  // Which replies wear the "· Status" card and which phantom bubbles are
  // stories we now track is decided by wa_ui::status::heal_plan.
  const plans = await invoke("ui_status_heal", {
    chats: chats.map((chat) => ({
      messages: chat.messages.map((message) => ({
        id: String(message.id || ""),
        status_quote: Boolean(message.status_quote),
        reply_to_id: String(message.reply_to?.id || ""),
        once_recovered: Boolean(message.once_recovered),
      })),
    })),
    updateIds,
  }).catch(() => []);

  let changed = false;
  plans.forEach((plan, index) => {
    const chat = chats[index];
    if (!chat) return;
    if (plan.quote_ids.length) {
      const quote = new Set(plan.quote_ids);
      for (const message of chat.messages) {
        if (!quote.has(message.id)) continue;
        message.status_quote = true;
        changed = true;
      }
    }
    if (plan.remove_ids.length) {
      const remove = new Set(plan.remove_ids);
      const kept = chat.messages.filter((message) => !remove.has(message.id));
      if (kept.length !== chat.messages.length) {
        chat.messages = kept;
        chat.last = kept.length ? kept[kept.length - 1] : null;
        chat.lastTs = chat.last ? chat.last.timestamp : chat.lastTs;
        changed = true;
      }
    }
  });
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

// The 24 h filter, the group order and the per-group play order are decided
// by wa_ui::status::viewer_groups; this maps the ids back to local updates.
async function statusGroupsPlan() {
  const groups = await invoke("ui_status_groups", { request: statusRequest() }).catch(
    () => [],
  );
  return groups
    .map((group) => {
      const entry = state.statuses.get(group.jid);
      if (!entry) return null;
      const byId = new Map(entry.updates.map((update) => [update.id, update]));
      const updates = group.update_ids.map((id) => byId.get(id)).filter(Boolean);
      if (!updates.length) return null;
      return { ...entry, name: group.name || entry.name, updates };
    })
    .filter(Boolean);
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

/* ------------------------------------------------ Rust-rendered status UI */

// The list rows, viewer shell, media body and "who viewed" markup come from
// wa-ui::status; this file keeps the timers, media fetch and event wiring.
function statusUpdateView(update) {
  return {
    id: update.id || "",
    sender: update.sender || "",
    name: update.name || "",
    kind: update.kind || "text",
    text: update.text || "",
    background: update.background || "",
    timestamp: update.timestamp || 0,
    viewed: Boolean(update.viewed),
    from_me: Boolean(update.from_me),
    viewers: (update.viewers || []).map((viewer) => ({
      jid: viewer.jid || "",
      name: viewer.name || "",
      timestamp: viewer.timestamp || 0,
    })),
  };
}

function statusRequest() {
  return {
    entries: [...state.statuses.values()].map((entry) => ({
      jid: entry.jid,
      name: entry.name || entry.jid,
      updates: (entry.updates || []).map(statusUpdateView),
    })),
    photos: Object.fromEntries(
      [...state.photos].filter(([, url]) => typeof url === "string" && url.startsWith("http")),
    ),
    local_media: Object.fromEntries(statusLocalMedia),
    me: state.me || "",
    me_jid: state.meJid || "",
    me_lid: state.meLid || "",
    now: Date.now(),
  };
}

let statusListSeq = 0;

async function renderStatusList() {
  setAvatar($("status-my-avatar"), state.me || "me", state.meLid || state.meJid);
  const seq = ++statusListSeq;
  let out;
  try {
    out = await invoke("ui_status_list", { request: statusRequest() });
  } catch (err) {
    console.warn("status list render failed", err);
    return;
  }
  if (seq !== statusListSeq) return;

  $("status-my-avatar").hidden = out.mine_has_thumb;
  const thumb = $("status-my-thumb");
  thumb.hidden = !out.mine_has_thumb;
  if (out.mine_has_thumb) {
    thumb.innerHTML = out.mine_thumb_html;
    thumb.setAttribute("style", out.mine_thumb_style);
    thumb.classList.toggle("has-image", out.mine_thumb_has_image);
  }
  $("status-mine").querySelector(".status-meta span").textContent = out.mine_label;

  if (state.connected) {
    queuePhotos(out.queue || []);
  }
  if (typeof setRailStatusUnseen === "function") setRailStatusUnseen(out.unseen);

  $("status-list").innerHTML = out.recent_html;
  $("status-viewed-list").innerHTML = out.viewed_html;
}

function addStatusUpdate(update) {
  const jid = normalizeStatusJid(update.sender);
  let entry = state.statuses.get(jid);
  if (!entry) {
    entry = { jid, name: update.name || jid, updates: [] };
    state.statuses.set(jid, entry);
  }
  if (update.name) {
    // The placeholder check is wa-ui::text::is_placeholder_name.
    invoke("ui_placeholder_name", { jid, name: update.name })
      .catch(() => false)
      .then((placeholder) => {
        if (!placeholder) entry.name = update.name;
      });
  }

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

async function openStatusViewer(jid) {
  statusGroups = await statusGroupsPlan();
  const index = statusGroups.findIndex((entry) => entry.jid === jid);
  if (index < 0) return;

  statusGroupIndex = index;
  statusQueue = statusGroups[index].updates;
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

async function renderStatusViews() {
  const update = statusQueue[statusIndex];
  let out;
  try {
    out = await invoke("ui_status_views", {
      viewers: (update?.viewers || []).map((viewer) => ({
        jid: viewer.jid || "",
        name: viewer.name || "",
        timestamp: viewer.timestamp || 0,
      })),
      request: statusRequest(),
    });
  } catch (err) {
    console.warn("status views render failed", err);
    return;
  }
  $("status-views-title").textContent = out.title;
  $("status-view-count-label").textContent = out.title.replace("Viewed by ", "");
  $("status-views-list").innerHTML = out.html;
  queuePhotos(out.queue || []);
}

function openStatusViews() {
  renderStatusViews();
  $("status-views").hidden = false;
}

/// A status "view" receipt arrived: remember who watched which update.
async function noteStatusView(event) {
  if (!event || !event.id) return;
  for (const entry of state.statuses.values()) {
    const update = entry.updates.find((item) => item.id === event.id);
    if (!update) continue;
    update.viewers = update.viewers || [];
    // The device suffix, the broadcast/newsletter exclusion and the repeat
    // check are decided by wa_ui::status::note_view.
    const note = await invoke("ui_status_note_view", {
      viewers: update.viewers,
      jid: String(event.jid || ""),
      name: String(event.name || ""),
      timestamp: Number(event.timestamp) || Date.now(),
    }).catch(() => ({ accept: false, viewer: null }));
    if (!note.accept || !note.viewer) return;
    update.viewers.push(note.viewer);
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

// The frame timings and capture limits come from wa_ui::status.
let statusTiming = {
  textMs: 10000,
  imageMs: 8000,
  videoFallbackMs: 20000,
  mediaFailedMs: 3000,
  thumbMaxPx: 320,
  posterTimeoutMs: 4000,
};
invoke("ui_status_timing")
  .then((timing) => {
    statusTiming = timing;
  })
  .catch(() => {});

async function statusFrameDuration(update) {
  if (!update) return 0;
  return invoke("ui_status_frame_duration", {
    kind: update.kind || "text",
    text: String(update.text || ""),
  }).catch(() => 0);
}

function currentStatusBar() {
  return $("status-progress").children[statusIndex] || null;
}

// How far into the current frame we are, read from the same clock the advance
// timer uses, so the bar and the frame can never disagree.
async function statusFrameElapsed() {
  return invoke("ui_status_elapsed", {
    total: statusFrameTotal,
    remaining: statusFrameRemaining,
    endsAt: statusFrameEndsAt,
    now: performance.now(),
  }).catch(() => Math.max(0, statusFrameTotal - statusFrameRemaining));
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
async function anchorStatusFrame(total, remaining, token = statusFrameToken) {
  if (token !== statusFrameToken) return;
  const now = performance.now();
  const anchor = await invoke("ui_status_anchor", { total, remaining, now }).catch(() => ({
    total: Math.max(1, Math.round(total)),
    remaining: Math.max(0, Math.round(remaining)),
    ends_at: now + Math.max(0, Math.round(remaining)),
  }));
  if (token !== statusFrameToken) return;
  statusFrameTotal = anchor.total;
  statusFrameRemaining = anchor.remaining;
  statusFrameEndsAt = anchor.ends_at;
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

async function pauseStatusFrame() {
  if (statusPaused || $("status-viewer").hidden) return;
  const update = statusQueue[statusIndex];
  if (!update) return;
  statusPaused = true;
  statusFrameRemaining = await invoke("ui_status_pause_remaining", {
    remaining: statusFrameRemaining,
    endsAt: statusFrameEndsAt,
    now: performance.now(),
  }).catch(() => statusFrameRemaining);
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

async function resumeStatusFrame() {
  if (!statusPaused) return;
  statusPaused = false;
  const [total, remaining] = await invoke("ui_status_resume", {
    total: statusFrameTotal,
    remaining: statusFrameRemaining,
  }).catch(() => [statusFrameTotal || 1, Math.max(1, statusFrameRemaining || 1)]);
  await anchorStatusFrame(total, remaining);
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
  statusFrameTotal = await statusFrameDuration(update);
  statusFrameEndsAt = 0;
  statusFrameRemaining = 0;
  updateStatusPauseButton();

  let shell;
  try {
    shell = await invoke("ui_status_frame", {
      request: {
        queue: statusQueue.map(statusUpdateView),
        index: statusIndex,
        group_name: statusGroups[statusGroupIndex]?.name || "",
        group_jid: statusGroups[statusGroupIndex]?.jid || "",
        photos: Object.fromEntries(
          [...state.photos].filter(([, url]) => typeof url === "string" && url.startsWith("http")),
        ),
        local_media: Object.fromEntries(statusLocalMedia),
        now: Date.now(),
      },
    });
  } catch (err) {
    console.warn("status frame render failed", err);
    return;
  }
  if (!shell || token !== statusFrameToken || $("status-viewer").hidden) return;

  // Past segments are filled by CSS; the active one is painted by the rAF loop.
  $("status-progress").innerHTML = shell.progress_html;
  $("status-viewer-name").textContent = shell.name;
  $("status-viewer-time").textContent = shell.time_label;
  setAvatar($("status-viewer-avatar"), shell.avatar_name, shell.avatar_jid);
  statusReplyVisible(update);
  markStatusViewed(update);
  updateStatusNav();

  const body = $("status-viewer-body");
  $("status-viewer").style.background = shell.background || "";
  body.innerHTML = shell.body_html;

  if (!shell.needs_media) {
    anchorStatusFrame(statusFrameTotal, statusFrameTotal, token);
    return;
  }

  // The bar starts with the frame, not with the media: waiting for a download
  // left the active segment empty (the black line) for as long as it took.
  const placeholder = update.kind === "video" ? statusTiming.videoFallbackMs : statusTiming.imageMs;
  anchorStatusFrame(placeholder, placeholder, token);
  try {
    let url = statusLocalMedia.get(update.id);
    if (!url) {
      const path = await invoke("fetch_media", { id: update.id });
      url = convertFileSrc ? convertFileSrc(path) : path;
    }
    if ($("status-viewer").hidden || token !== statusFrameToken) return;
    const html = await invoke("ui_status_media", {
      url,
      kind: update.kind,
      name: update.name || "",
    });
    if (token !== statusFrameToken) return;
    body.innerHTML = html;
    if (update.kind === "video") {
      const video = body.querySelector("video");
      if (!video) return;
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
      const adoptDuration = async () => {
        if (token !== statusFrameToken || !(video.duration > 0)) return;
        const total = Math.round(video.duration * 1000);
        // Keep the time already spent on the placeholder countdown, so the bar
        // never jumps backwards when the real length lands.
        const spent = await statusFrameElapsed();
        anchorStatusFrame(total, Math.max(1, total - spent), token);
      };
      video.addEventListener("loadedmetadata", adoptDuration, { once: true });
      if (video.readyState >= 1) adoptDuration();
      video.play().catch(() => {
        video.muted = true;
        video.play().catch(() => {});
      });
    }
  } catch {
    if (token !== statusFrameToken) return;
    body.innerHTML = await invoke("ui_status_media_error").catch(
      () => '<div class="text-status">Status media is no longer available.</div>',
    );
    anchorStatusFrame(statusTiming.mediaFailedMs, statusTiming.mediaFailedMs, token);
  }
}

async function replyCurrentStatus() {
  const update = statusQueue[statusIndex];
  const input = $("status-reply-input");
  const text = input.value.trim();
  if (!update) return;
  // Who may reply/react/delete is decided by wa-ui::actions.
  const perms = await invoke("ui_status_permissions", {
    fromMe: Boolean(update.from_me),
  });
  if (!perms.can_reply || !text) return;
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
  if (!update) return;
  const perms = await invoke("ui_status_permissions", {
    fromMe: Boolean(update.from_me),
  });
  if (!perms.can_react) return;
  try {
    await invoke("react_status", { sender: update.sender, id: update.id, emoji });
    toast(`Reacted ${emoji}`);
  } catch (err) {
    toast(String(err));
  }
}

async function deleteCurrentStatus() {
  const update = statusQueue[statusIndex];
  if (!update) return;
  const perms = await invoke("ui_status_permissions", {
    fromMe: Boolean(update.from_me),
  });
  if (!perms.can_delete) {
    toast(perms.delete_reason || "Only your own status can be deleted.");
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
async function openStatusAddMenu() {
  const menu = $("status-add-menu");
  const rect = $("status-mine").getBoundingClientRect();
  menu.hidden = false;
  const width = menu.offsetWidth || 236;
  const height = menu.offsetHeight || 112;
  // The edge maths live in wa_ui::menus::clamp_popover.
  const pos = await invoke("ui_clamp_popover", {
    x: rect.left + 16,
    y: rect.bottom + 6,
    width,
    height,
    viewportW: window.innerWidth,
    viewportH: window.innerHeight,
    margin: 12,
  }).catch(() => ({ left: rect.left + 16, top: rect.bottom + 6 }));
  menu.style.left = `${pos.left}px`;
  menu.style.top = `${pos.top}px`;
}

function closeStatusAddMenu() {
  const menu = $("status-add-menu");
  if (menu) menu.hidden = true;
}

/* -------------------------------------- full-screen text status composer */

// The background palette, fonts and ARGB conversion live in wa_ui::composer;
// they are fetched once and cached here for the cycling buttons.
let statusPaletteCache = null;
const statusPaletteReady = invoke("ui_status_palette")
  .then((palette) => {
    statusPaletteCache = palette;
    return palette;
  })
  .catch(() => ({ backgrounds: [], fonts: [], font_names: [] }));

async function statusPalette() {
  return statusPaletteCache || (await statusPaletteReady);
}

let statusBgIndex = 0;
let statusFontIndex = 0;

async function openStatusTextComposer() {
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
  const palette = await statusPalette();
  await applyStatusTextBackground(
    Math.floor(Math.random() * (palette.backgrounds.length || 1)),
  );
  await applyStatusTextFont(statusFontIndex);
  resizeStatusTextInput();
  updateStatusTextSend();
  input.focus();
}

async function closeStatusTextComposer() {
  const next = await invoke("ui_status_wizard_back", {
    textScreenOpen: !$("status-text-screen").hidden,
    emojiOpen: !$("status-emoji-panel").hidden,
  }).catch(() => "text_screen");
  if (next === "emoji") {
    $("status-emoji-panel").hidden = true;
    return;
  }
  if (next === "text_screen") {
    $("status-text-screen").hidden = true;
    $("status-emoji-panel").hidden = true;
    return;
  }
  $("status-composer").hidden = true;
  $("status-text-screen").hidden = true;
  $("status-emoji-panel").hidden = true;
}

async function applyStatusTextBackground(index) {
  const palette = await statusPalette();
  const total = palette.backgrounds.length || 1;
  statusBgIndex = ((index % total) + total) % total;
  $("status-text-screen").style.setProperty(
    "--status-bg",
    palette.backgrounds[statusBgIndex] || "#243640",
  );
}

async function applyStatusTextFont(index) {
  const palette = await statusPalette();
  const total = palette.fonts.length || 1;
  statusFontIndex = ((index % total) + total) % total;
  $("status-text-screen").style.setProperty(
    "--status-font",
    palette.fonts[statusFontIndex] || "inherit",
  );
}

/// The opaque \`0xAARRGGBB\` integer for a colour (wa_ui::composer::argb).
async function statusArgb(hex) {
  return invoke("ui_status_argb", { hex: String(hex || "") }).catch(() => 0xff00a884);
}

function resizeStatusTextInput() {
  const input = $("status-text-input");
  input.style.height = "auto";
  input.style.height = `${Math.min(input.scrollHeight, Math.round(window.innerHeight * 0.62))}px`;
}

async function updateStatusTextSend() {
  // Enabling needs non-blank text (wa_ui::composer::can_post_text).
  const canSend = await invoke("ui_can_post_text", {
    text: $("status-text-input").value,
  }).catch(() => Boolean($("status-text-input").value.trim()));
  const send = $("status-text-send");
  if (send) send.disabled = !canSend;
}

async function buildStatusEmojiPanel() {
  const panel = $("status-emoji-panel");
  if (panel.dataset.built) return;
  $("status-emoji-cats").innerHTML = await invoke("ui_status_emoji_categories");
  await renderStatusEmojiGroup(0);
  panel.dataset.built = "1";
}

/// Paint one WhatsApp emoji category (or the search results) into the panel.
async function renderStatusEmojiGroup(index, query = "") {
  let panel;
  try {
    panel = await invoke("ui_status_emoji_group", { index, query });
  } catch (err) {
    console.warn("status emoji render failed", err);
    return;
  }
  $("status-emoji-title").textContent = panel.title;
  $("status-emoji-grid").innerHTML = panel.html;
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
  const draft = await invoke("ui_status_post_check", {
    hasFile: false,
    mime: "",
    text,
  });
  if (!draft.ok) return;
  const send = $("status-text-send");
  send.disabled = true;
  // The colour and font travel with the status, so contacts see the same story.
  const palette = await statusPalette();
  const background = palette.backgrounds[statusBgIndex] || "#243640";
  const font = palette.font_names[statusFontIndex] || "system";
  try {
    const serverId = await invoke("post_status_text", {
      text,
      background: await statusArgb(background),
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

// The thumbnail box and JPEG quality come from wa_ui::status::capture_limits.
async function statusCaptureLimits() {
  return invoke("ui_status_capture")
    .then((limits) => limits)
    .catch(() => ({ thumb_max_px: 320, jpeg_quality: 0.72 }));
}

function statusImageThumb(file) {
  return new Promise((resolve) => {
    const url = URL.createObjectURL(file);
    const img = new Image();
    img.onload = async () => {
      const limits = await statusCaptureLimits();
      const size = await invoke("ui_status_thumb_scale", {
        width: img.width,
        height: img.height,
        max: limits.thumb_max_px,
      }).catch(() => ({
        width: Math.max(1, img.width),
        height: Math.max(1, img.height),
      }));
      const canvas = document.createElement("canvas");
      canvas.width = size.width;
      canvas.height = size.height;
      canvas.getContext("2d").drawImage(img, 0, 0, canvas.width, canvas.height);
      URL.revokeObjectURL(url);
      resolve(canvas.toDataURL("image/jpeg", limits.jpeg_quality).split(",")[1] || "");
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
    const timer = setTimeout(
      () => finish("", video.duration || 1),
      statusTiming.posterTimeoutMs,
    );
    video.muted = true;
    video.preload = "metadata";
    video.src = url;
    video.onloadeddata = async () => {
      video.currentTime = await invoke("ui_status_poster_seek", {
        duration: Number(video.duration) || 0,
      }).catch(() => 0.2);
    };
    video.onseeked = async () => {
      clearTimeout(timer);
      const limits = await statusCaptureLimits();
      const size = await invoke("ui_status_thumb_scale", {
        width: video.videoWidth,
        height: video.videoHeight,
        max: limits.thumb_max_px,
      }).catch(() => ({
        width: Math.max(1, video.videoWidth),
        height: Math.max(1, video.videoHeight),
      }));
      const canvas = document.createElement("canvas");
      canvas.width = size.width;
      canvas.height = size.height;
      canvas.getContext("2d").drawImage(video, 0, 0, canvas.width, canvas.height);
      const seconds = await invoke("ui_status_duration_seconds", {
        duration: Number(video.duration) || 0,
      }).catch(() => 1);
      finish(canvas.toDataURL("image/jpeg", limits.jpeg_quality).split(",")[1] || "", seconds);
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
  const draft = await invoke("ui_status_post_check", {
    hasFile: Boolean(file),
    mime: file?.type || "",
    text,
  });
  const error = $("status-error");
  error.hidden = true;
  if (!draft.ok) {
    if (draft.error) {
      error.textContent = draft.error;
      error.hidden = false;
    }
    return;
  }
  const caption = draft.caption;

  try {
    if (file) {
      const isVideo = draft.is_video;
      const data = await fileToBase64(file);
      const meta = isVideo
        ? await statusVideoMeta(file)
        : { thumb: await statusImageThumb(file), duration: 0 };
      await invoke("post_status_media", {
        data,
        mime: file.type || (isVideo ? "video/mp4" : "image/jpeg"),
        caption,
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
        text: caption,
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

