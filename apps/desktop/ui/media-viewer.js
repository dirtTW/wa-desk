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

const VIDEO_ICONS = {
  play: '<svg viewBox="0 -960 960 960" aria-hidden="true"><path d="M320-200v-560l440 280-440 280Zm80-280Zm0 134 210-134-210-134v268Z"/></svg>',
  pause:
    '<svg viewBox="0 -960 960 960" aria-hidden="true"><path d="M520-200v-560h240v560H520Zm-320 0v-560h240v560H200Zm400-80h80v-400h-80v400Zm-320 0h80v-400h-80v400Zm0-400v400-400Zm320 0v400-400Z"/></svg>',
  volume:
    '<svg viewBox="0 -960 960 960" aria-hidden="true"><path d="M560-131v-82q90-26 145-100t55-168q0-94-55-168T560-749v-82q124 28 202 125.5T840-481q0 127-78 224.5T560-131ZM120-360v-240h160l200-200v640L280-360H120Zm440 40v-322q47 22 73.5 66t26.5 96q0 51-26.5 94.5T560-320ZM400-606l-86 86H200v80h114l86 86v-252ZM300-480Z"/></svg>',
  muted:
    '<svg viewBox="0 -960 960 960" aria-hidden="true"><path d="M792-56 671-177q-25 16-53 27.5T560-131v-82q14-5 27.5-10t25.5-12L480-368v208L280-360H120v-240h128L56-792l56-56 736 736-56 56Zm-8-232-58-58q17-31 25.5-65t8.5-70q0-94-55-168T560-749v-82q124 28 202 125.5T840-481q0 53-14.5 102T784-288ZM650-422l-90-90v-130q47 22 73.5 66t26.5 96q0 15-2.5 29.5T650-422ZM480-592 376-696l104-104v208Zm-80 238v-94l-72-72H200v80h114l86 86Zm-36-130Z"/></svg>',
  fullscreen:
    '<svg viewBox="0 -960 960 960" aria-hidden="true"><path d="M120-120v-200h80v120h120v80H120Zm520 0v-80h120v-120h80v200H640ZM120-640v-200h200v80H200v120h-80Zm640 0v-120H640v-80h200v200h-80Z"/></svg>',
  exitFullscreen:
    '<svg viewBox="0 -960 960 960" aria-hidden="true"><path d="M240-120v-120H120v-80h200v200h-80Zm400 0v-200h200v80H720v120h-80ZM120-640v-80h120v-120h80v200H120Zm520 0v-200h80v120h120v80H640Z"/></svg>',
};

function videoPlayerHtml(url, poster) {
  return `<div class="viewer-video" data-viewer-video>
      <video class="viewer-video-el" src="${escapeHtml(url)}"${
        poster ? ` poster="${poster}"` : ""
      } playsinline autoplay preload="metadata"></video>
      <button type="button" class="viewer-big-play" data-video-toggle aria-label="Play video">${VIDEO_ICONS.play}</button>
      <div class="viewer-video-controls">
        <button type="button" class="vv-btn" data-video-toggle aria-label="Play">${VIDEO_ICONS.play}</button>
        <input class="vv-seek" data-video-seek type="range" min="0" max="1000" step="1" value="0" aria-label="Seek" />
        <span class="vv-time"><span data-video-current>0:00</span> / <span data-video-duration>0:00</span></span>
        <button type="button" class="vv-btn vv-speed" data-video-speed aria-label="Playback speed">1.0x</button>
        <button type="button" class="vv-btn" data-video-mute aria-label="Mute">${VIDEO_ICONS.volume}</button>
        <button type="button" class="vv-btn" data-video-fullscreen aria-label="Fullscreen">${VIDEO_ICONS.fullscreen}</button>
      </div>
    </div>`;
}

function setupViewerVideo(container) {
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
    const icon = video.paused ? VIDEO_ICONS.play : VIDEO_ICONS.pause;
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
    mute.innerHTML = muted ? VIDEO_ICONS.muted : VIDEO_ICONS.volume;
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

async function renderMediaViewer() {
  const item = mediaViewerItems[mediaViewerIndex];
  if (!item) {
    closeMediaViewer();
    return;
  }

  const chat = state.chats.get(state.active);
  const token = ++mediaViewerToken;
  $("media-viewer-chat").textContent = chat ? chat.name : "Media";
  $("media-viewer-time").textContent = `${dayOf(item.timestamp)} at ${timeOf(item.timestamp)}`;
  // The official viewer shows the caption under the media, mentions coloured.
  const cap = $("media-viewer-cap");
  const captionText = String(item.text || "").trim();
  if (captionText) {
    cap.hidden = false;
    cap.innerHTML = bodyHtml(captionText, item.mentions);
  } else {
    cap.hidden = true;
    cap.innerHTML = "";
  }
  $("media-viewer-reactbar").hidden = true;
  $("media-viewer-prev").disabled = mediaViewerIndex <= 0;
  $("media-viewer-next").disabled = mediaViewerIndex >= mediaViewerItems.length - 1;
  $("media-viewer-download").disabled = true;

  const stage = $("media-viewer-stage");
  if (!state.media.get(item.id)?.url) {
    stage.innerHTML = '<div class="viewer-loading">Loading media...</div>';
  }

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
      stage.innerHTML = videoPlayerHtml(url, thumbOf(item.id));
      setupViewerVideo(stage.querySelector(".viewer-video"));
      $("media-viewer-zoom-in").disabled = true;
      $("media-viewer-zoom-out").disabled = true;
    } else {
      stage.innerHTML = `<div class="viewer-zoom-layer"><img src="${escapeHtml(url)}" alt="${escapeHtml(item.text || "Photo")}" /></div>`;
      viewerZoom = 1;
      viewerPanX = 0;
      viewerPanY = 0;
      $("media-viewer-zoom-in").disabled = false;
      $("media-viewer-zoom-out").disabled = true;
      applyViewerZoom();
    }
    $("media-viewer-download").disabled = false;
  } catch {
    if (token === mediaViewerToken) {
      stage.innerHTML = '<div class="viewer-loading">Media unavailable</div>';
    }
    $("media-viewer-download").disabled = true;
  }

  renderMediaStrip();
  preloadViewerImages();
}

function stripThumbInner(item) {
  const cached = state.media.get(item.id);
  if (item.kind === "video") {
    ensureThumb(item.id);
    ensureVideoPoster(item.id);
    ensureDuration(item.id);
    captureVideoPoster(item.id);
    const poster = thumbOf(item.id);
    const seconds = state.durations.get(item.id);
    const badge = `<span class="strip-duration" data-duration-id="${escapeHtml(
      item.id,
    )}">${seconds ? formatDuration(seconds) : ""}</span>`;
    const art = poster
      ? `<img src="${poster}" alt="" /><span class="strip-play">${waIcon("play_arrow")}</span>`
      : `<span class="strip-play big">${waIcon("play_arrow")}</span>`;
    return `${art}${badge}`;
  }
  if (cached?.status === "ready" && cached.url) {
    return `<img src="${escapeHtml(cached.url)}" alt="" />`;
  }
  return '<span class="strip-loading"></span>';
}

function renderMediaStrip() {
  const strip = $("media-viewer-strip");
  const start = Math.max(0, mediaViewerIndex - 15);
  const end = Math.min(mediaViewerItems.length, mediaViewerIndex + 16);

  strip.innerHTML = mediaViewerItems
    .slice(start, end)
    .map((item, offset) => {
      const index = start + offset;
      return `<button type="button" class="media-strip-thumb ${
        index === mediaViewerIndex ? "active" : ""
      }" data-index="${index}" data-media-id="${escapeHtml(item.id)}" data-thumb-id="${escapeHtml(
        item.id,
      )}" title="${item.kind === "video" ? "Video" : "Photo"}">${stripThumbInner(item)}</button>`;
    })
    .join("");

  const active = strip.querySelector(".active");
  if (active) active.scrollIntoView({ inline: "center", block: "nearest" });
}

function stepMediaViewer(delta) {
  const next = mediaViewerIndex + delta;
  if (next < 0 || next >= mediaViewerItems.length) return;
  mediaViewerIndex = next;
  renderMediaViewer();
}

