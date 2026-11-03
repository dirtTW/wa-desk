/* WA Desk frontend: media-viewer. Loaded in order by index.html; all files share the classic-script global scope. */

/* ------------------------------------------------------------ media viewer */

let mediaViewerItems = [];
let mediaViewerIndex = 0;
let mediaViewerToken = 0;
let viewerPendingId = null;
let viewerZoom = 1;
let viewerPanX = 0;
let viewerPanY = 0;
let viewerDragging = false;
let viewerDragStart = null;
let viewerStripTimer = 0;
let viewerVideoIdleTimer = 0;

function applyViewerZoom() {
  const layer = document.querySelector(".viewer-zoom-layer");
  if (layer) {
    layer.style.transform = `translate(${viewerPanX}px, ${viewerPanY}px) scale(${viewerZoom})`;
  }
  $("media-viewer").classList.toggle("zoomed", viewerZoom > 1.01);
  $("media-viewer-zoom-in").disabled = viewerZoom >= 6;
  $("media-viewer-zoom-out").disabled = viewerZoom <= 1;
}

function setViewerZoom(next) {
  viewerZoom = Math.max(1, Math.min(6, Number(next.toFixed(2))));
  if (viewerZoom <= 1.01) {
    viewerZoom = 1;
    viewerPanX = 0;
    viewerPanY = 0;
  }
  applyViewerZoom();
}

function viewerZoomBy(delta) {
  setViewerZoom(viewerZoom + delta);
}

function openMediaViewer(id) {
  const chat = state.chats.get(state.active);
  if (!chat) return;
  mediaViewerItems = chat.messages.filter(
    (message) => !message.deleted && (message.kind === "image" || message.kind === "video"),
  );
  const index = mediaViewerItems.findIndex((message) => message.id === id);
  if (index < 0) return; // stickers/documents are not part of the gallery

  mediaViewerIndex = index;
  viewerZoom = 1;
  viewerPanX = 0;
  viewerPanY = 0;
  $("media-viewer").hidden = false;
  renderMediaViewer();
}

function closeMediaViewer() {
  mediaViewerToken += 1;
  clearTimeout(viewerStripTimer);
  clearTimeout(viewerVideoIdleTimer);
  if (document.fullscreenElement instanceof Element && $("media-viewer").contains(document.fullscreenElement)) {
    document.exitFullscreen().catch(() => {});
  }
  $("media-viewer").hidden = true;
  $("media-viewer").classList.remove("zoomed");
  $("media-viewer-stage").innerHTML = "";
  $("media-viewer-strip").innerHTML = "";
  $("media-viewer-cap").hidden = true;
  $("media-viewer-reactbar").hidden = true;
  $("media-viewer-download").disabled = true;
  mediaViewerItems = [];
  viewerZoom = 1;
  viewerPanX = 0;
  viewerPanY = 0;
}

// WebView2 ignores <a download> for asset.localhost URLs and would navigate the
// whole app to the file, so the Rust side copies the media to Downloads.
async function downloadCurrentMedia() {
  const item = mediaViewerItems[mediaViewerIndex];
  const button = $("media-viewer-download");
  if (!item || button.disabled) return;
  button.disabled = true;
  toast("Saving media\u2026");
  try {
    const path = await invokeWithTimeout("download_media", { id: item.id }, 60000);
    const name = String(path).split(/[\\/]/).pop();
    toast(`Saved ${name} to Downloads\\WA Desk`);
  } catch (err) {
    toast(friendlyMediaError(err));
  } finally {
    if (!$("media-viewer").hidden) button.disabled = false;
  }
}

// Load the rest of the chat's photos in the background so the filmstrip and
// previous/next navigation are ready before the user gets there. Nearby photos
// go first; the count is capped so a huge gallery cannot flood memory or the
// CDN, while still filling the strip ahead of the user.
const VIEWER_PRELOAD_LIMIT = 40;

function refreshStripThumb(item) {
  const cached = state.media.get(item.id);
  if (!cached?.url) return;
  const button = $("media-viewer-strip").querySelector(
    `[data-media-id="${item.id}"]`,
  );
  if (!button) return;
  button.innerHTML = `<img src="${escapeHtml(cached.url)}" alt="" />`;
}

function preloadViewerImages() {
  const token = mediaViewerToken;
  const pending = mediaViewerItems
    .map((item, index) => ({ item, distance: Math.abs(index - mediaViewerIndex) }))
    .filter(({ item }) => item.kind === "image" && !state.media.get(item.id)?.url)
    .sort((a, b) => a.distance - b.distance)
    .slice(0, VIEWER_PRELOAD_LIMIT)
    .map(({ item }) => item);
  if (!pending.length) return;
  mapLimit(pending, 3, async (item) => {
    try {
      await viewerMediaUrl(item);
    } catch {
      return;
    }
    if (token !== mediaViewerToken) return;
    refreshStripThumb(item);
  });
}

async function viewerMediaUrl(item) {
  const cached = state.media.get(item.id);
  if (cached?.status === "ready" && cached.url) return cached.url;
  const path = await invoke("fetch_media", { id: item.id });
  const url = convertFileSrc ? convertFileSrc(path) : path;
  rememberMedia(item.id, url);
  return url;
}

/* ------------------------------------------------------ custom video player */


async function setupViewerVideo(container) {
  // The transport icons are rendered by wa-ui::html::icon.
  const icons = {
    play: await invoke("ui_icon", { name: "play_arrow" }),
    pause: await invoke("ui_icon", { name: "pause" }),
    volume: await invoke("ui_icon", { name: "volume_up" }),
    muted: await invoke("ui_icon", { name: "volume_off" }),
  };
  const video = container.querySelector(".viewer-video-el");
  if (!video) return;
  const controls = container.querySelector(".viewer-video-controls");
  const toggles = container.querySelectorAll("[data-video-toggle]");
  const current = container.querySelector("[data-video-current]");
  const duration = container.querySelector("[data-video-duration]");
  const seek = container.querySelector("[data-video-seek]");
  const mute = container.querySelector("[data-video-mute]");
  const speed = container.querySelector("[data-video-speed]");
  const fullscreen = container.querySelector("[data-video-fullscreen]");
  const speeds = [1, 1.5, 2, 0.5];
  let speedIndex = 0;

  const showControls = () => {
    container.classList.remove("idle");
    clearTimeout(viewerVideoIdleTimer);
    if (!video.paused) {
      viewerVideoIdleTimer = setTimeout(() => {
        if (!video.paused) container.classList.add("idle");
      }, 2400);
    }
  };

  // Keep the transport bar over the bottom edge of the video, matching its
  // width for portrait clips instead of stretching across empty stage space.
  const fitControls = () => {
    const stage = container.getBoundingClientRect();
    const frame = video.getBoundingClientRect();
    if (!frame.width || !frame.height || !controls) return;
    controls.style.width = `${Math.max(300, Math.min(560, frame.width))}px`;
    controls.style.bottom = `${Math.max(10, stage.bottom - frame.bottom + 12)}px`;
  };
  video.addEventListener("loadedmetadata", fitControls);
  if (window.ResizeObserver) new ResizeObserver(fitControls).observe(video);
  setTimeout(fitControls, 60);

  const syncToggle = () => {
    const icon = video.paused ? icons.play : icons.pause;
    toggles.forEach((button) => {
      button.innerHTML = icon;
      button.setAttribute("aria-label", video.paused ? "Play" : "Pause");
    });
    container.classList.toggle("playing", !video.paused);
    if (video.paused) {
      clearTimeout(viewerVideoIdleTimer);
      container.classList.remove("idle");
    } else {
      showControls();
    }
  };

  const syncTime = () => {
    const total = Number.isFinite(video.duration) ? video.duration : 0;
    const ratio = total > 0 ? Math.min(1, video.currentTime / total) : 0;
    current.textContent = formatDuration(Math.floor(video.currentTime));
    duration.textContent = total ? formatDuration(Math.ceil(total)) : "0:00";
    seek.value = String(Math.round(ratio * 1000));
    seek.style.setProperty("--seek", `${(ratio * 100).toFixed(2)}%`);
  };

  const syncMute = () => {
    const muted = video.muted || video.volume === 0;
    mute.innerHTML = muted ? icons.muted : icons.volume;
    mute.setAttribute("aria-label", muted ? "Unmute" : "Mute");
  };

  const togglePlay = () => {
    if (video.paused) {
      video.play().catch(() => {});
    } else {
      video.pause();
    }
    showControls();
  };

  toggles.forEach((button) => button.addEventListener("click", togglePlay));
  video.addEventListener("click", togglePlay);
  video.addEventListener("play", syncToggle);
  video.addEventListener("pause", syncToggle);
  video.addEventListener("loadedmetadata", syncTime);
  video.addEventListener("timeupdate", syncTime);
  video.addEventListener("volumechange", syncMute);
  video.addEventListener("ended", () => {
    container.classList.remove("playing", "idle");
    clearTimeout(viewerVideoIdleTimer);
  });

  seek.addEventListener("input", () => {
    if (!Number.isFinite(video.duration) || video.duration <= 0) return;
    video.currentTime = (Number(seek.value) / 1000) * video.duration;
    syncTime();
    showControls();
  });

  mute.addEventListener("click", () => {
    video.muted = !video.muted;
    syncMute();
    showControls();
  });

  speed.addEventListener("click", () => {
    speedIndex = (speedIndex + 1) % speeds.length;
    video.playbackRate = speeds[speedIndex];
    speed.textContent = `${speeds[speedIndex].toFixed(1)}x`;
    showControls();
  });

  fullscreen.addEventListener("click", () => {
    if (document.fullscreenElement === container) {
      document.exitFullscreen().catch(() => {});
    } else {
      container.requestFullscreen?.().catch(() => {});
    }
    showControls();
  });

  container.addEventListener("mousemove", showControls);
  container.addEventListener("mouseleave", () => {
    if (!video.paused) container.classList.add("idle");
  });

  syncToggle();
  syncTime();
  syncMute();
}


// The viewer chrome (header labels, caption, stage, filmstrip) is built by
// wa-ui::viewer; this keeps the media download, zoom/pan and the player wiring.
let viewerRenderSeq = 0;

function viewerRequest() {
  const media = {};
  for (const [id, entry] of state.media) {
    if (entry?.url) media[id] = entry.url;
  }
  const chat = state.chats.get(state.active);
  return {
    items: mediaViewerItems.map((item) => ({
      id: item.id,
      kind: item.kind,
      text: item.text || "",
      timestamp: item.timestamp || 0,
      mentions: Array.isArray(item.mentions) ? item.mentions : [],
    })),
    index: mediaViewerIndex,
    chat_name: chat ? chat.name : "Media",
    media,
    thumbs: Object.fromEntries(state.thumbs),
    durations: Object.fromEntries(state.durations),
    contacts: Object.fromEntries(state.contacts),
    phones: Object.fromEntries(state.phones),
    sender_names: activeSenderNames(),
    me: state.me || "",
    me_jid: state.meJid || "",
    me_lid: state.meLid || "",
    self_handle: (state.chats.get(state.meJid) || state.chats.get(state.meLid) || {}).name || "",
    now: Date.now(),
  };
}

function paintViewerStrip(html) {
  const strip = $("media-viewer-strip");
  strip.innerHTML = html;
  const active = strip.querySelector(".active");
  if (active) active.scrollIntoView({ inline: "center", block: "nearest" });
}

async function renderMediaViewer() {
  const item = mediaViewerItems[mediaViewerIndex];
  if (!item) {
    closeMediaViewer();
    return;
  }

  const token = ++mediaViewerToken;
  const seq = ++viewerRenderSeq;
  let shell;
  try {
    shell = await invoke("ui_media_viewer", { request: viewerRequest() });
  } catch (err) {
    console.warn("media viewer render failed", err);
    return;
  }
  if (!shell || token !== mediaViewerToken || seq !== viewerRenderSeq) return;

  $("media-viewer-chat").textContent = shell.chat_label;
  $("media-viewer-time").textContent = shell.time_label;
  const cap = $("media-viewer-cap");
  cap.hidden = !shell.has_caption;
  cap.innerHTML = shell.caption_html;
  $("media-viewer-reactbar").hidden = true;
  $("media-viewer-prev").disabled = shell.prev_disabled;
  $("media-viewer-next").disabled = shell.next_disabled;
  $("media-viewer-download").disabled = true;

  const stage = $("media-viewer-stage");
  stage.innerHTML = shell.stage_html;
  paintViewerStrip(shell.strip_html);

  try {
    const url = await viewerMediaUrl(item);
    if (token !== mediaViewerToken || $("media-viewer").hidden) return;
    if (item.kind === "video") {
      ensureThumb(item.id);
      ensureVideoPoster(item.id);
      ensureDuration(item.id);
      captureVideoPoster(item.id);
      clearTimeout(viewerVideoIdleTimer);
      viewerZoom = 1;
      viewerPanX = 0;
      viewerPanY = 0;
      $("media-viewer").classList.remove("zoomed");
      stage.innerHTML = await invoke("ui_viewer_stage", {
        url,
        kind: "video",
        text: item.text || "",
        poster: thumbOf(item.id) || null,
      });
      setupViewerVideo(stage.querySelector(".viewer-video"));
      $("media-viewer-zoom-in").disabled = true;
      $("media-viewer-zoom-out").disabled = true;
    } else {
      stage.innerHTML = await invoke("ui_viewer_stage", {
        url,
        kind: "image",
        text: item.text || "",
      });
      viewerZoom = 1;
      viewerPanX = 0;
      viewerPanY = 0;
      $("media-viewer-zoom-in").disabled = false;
      $("media-viewer-zoom-out").disabled = true;
      applyViewerZoom();
    }
    $("media-viewer-download").disabled = false;
    // Repaint the strip so the active thumb shows its image now that the
    // media cache has the URL.
    if (token === mediaViewerToken) {
      const fresh = await invoke("ui_media_viewer", { request: viewerRequest() }).catch(
        () => null,
      );
      if (fresh && token === mediaViewerToken) paintViewerStrip(fresh.strip_html);
    }
  } catch {
    if (token === mediaViewerToken) {
      stage.innerHTML = await invoke("ui_viewer_placeholder", { kind: "unavailable" });
    }
    $("media-viewer-download").disabled = true;
  }

  preloadViewerImages();
}

function stepMediaViewer(delta) {
  const next = mediaViewerIndex + delta;
  if (next < 0 || next >= mediaViewerItems.length) return;
  mediaViewerIndex = next;
  renderMediaViewer();
}
