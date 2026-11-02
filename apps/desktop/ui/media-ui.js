/* WA Desk frontend: media-ui. Loaded in order by index.html; all files share the classic-script global scope. */

/* --------------------------------------------------------- video posters */

const MAX_THUMBS = 240;
// Sampled video frames are far bigger than the inline JPEGs, so they get their
// own, smaller budget.
const MAX_VIDEO_POSTERS = 120;

// Large link-preview thumbnails currently being fetched (one channel re-render
// must not queue several downloads of the same image).
const linkThumbPending = new Set();

// The poster for a video: the frame sampled from the clip when there is one,
// otherwise WhatsApp's inline JPEG — which is tiny (72 px here) and looks blurry
// once the bubble scales it up to its 420 px box.
function thumbOf(id) {
  if (!id) return "";
  return state.videoPosters.get(id) || state.thumbs.get(id) || "";
}

// Update every on-screen copy of a video poster once the backend returns the
// embedded JPEG: the inline bubble, the filmstrip and the full-screen player.
function applyThumb(id) {
  const poster = thumbOf(id);
  if (!poster) return;
  for (const node of document.querySelectorAll(`[data-thumb-id="${id}"]`)) {
    if (node.tagName === "VIDEO") {
      node.setAttribute("poster", poster);
    } else if (node.classList.contains("media-box")) {
      node.classList.add("has-thumb");
      node.style.backgroundImage = `url("${poster}")`;
    } else if (node.classList.contains("media-strip-thumb")) {
      const item = mediaViewerItems.find((entry) => entry.id === id);
      if (item) node.innerHTML = stripThumbInner(item);
    }
  }
}

function ensureThumb(id) {
  if (!id || state.thumbs.has(id) || state.thumbPending.has(id)) return;
  state.thumbPending.add(id);
  invoke("media_thumbnail", { id })
    .then((data) => {
      if (!data) return;
      state.thumbs.set(id, data);
      while (state.thumbs.size > MAX_THUMBS) {
        const oldest = state.thumbs.keys().next().value;
        if (oldest === undefined) break;
        state.thumbs.delete(oldest);
      }
      applyThumb(id);
    })
    .catch(() => {})
    .finally(() => state.thumbPending.delete(id));
}

function formatDuration(seconds) {
  const total = Math.max(0, Math.round(seconds));
  const hours = Math.floor(total / 3600);
  const minutes = Math.floor((total % 3600) / 60);
  const secs = total % 60;
  return hours
    ? `${hours}:${String(minutes).padStart(2, "0")}:${String(secs).padStart(2, "0")}`
    : `${minutes}:${String(secs).padStart(2, "0")}`;
}

// Video durations live in the file, not in the message, so probe the downloaded
// file once and remember the result for the filmstrip/duration badges.
function ensureDuration(id) {
  if (!id || state.durations.has(id) || state.durationPending.has(id)) return;
  const cached = state.media.get(id);
  if (!cached?.url) return;
  state.durationPending.add(id);
  const probe = document.createElement("video");
  probe.preload = "metadata";
  probe.muted = true;
  probe.src = cached.url;
  const finish = () => {
    state.durationPending.delete(id);
    probe.removeAttribute("src");
    probe.load();
  };
  probe.addEventListener(
    "loadedmetadata",
    () => {
      if (Number.isFinite(probe.duration) && probe.duration > 0) {
        state.durations.set(id, probe.duration);
        applyDuration(id);
      }
      finish();
    },
    { once: true },
  );
  probe.addEventListener("error", finish, { once: true });
}

function applyDuration(id) {
  const seconds = state.durations.get(id);
  if (!seconds) return;
  for (const node of document.querySelectorAll(`[data-duration-id="${id}"]`)) {
    node.textContent = formatDuration(seconds);
  }
}

// WhatsApp's inline JPEG poster is tiny, so a video bubble scaled to its 420 px
// box looks blurry. When the clip is on disk, sample a real frame and keep it as
// the poster (the bubble, filmstrip and player all read it through `thumbOf`).
function captureVideoPoster(id) {
  if (!id || state.videoPosters.has(id) || state.posterPending.has(id)) return;
  const cached = state.media.get(id);
  if (!cached?.url) return;
  state.posterPending.add(id);

  const probe = document.createElement("video");
  probe.muted = true;
  probe.playsInline = true;
  probe.preload = "auto";
  probe.crossOrigin = "anonymous";
  let timer = 0;
  const cleanup = () => {
    clearTimeout(timer);
    state.posterPending.delete(id);
    probe.pause();
    probe.removeAttribute("src");
    probe.load();
  };

  probe.addEventListener(
    "loadedmetadata",
    () => {
      const duration = Number.isFinite(probe.duration) ? probe.duration : 0;
      probe.currentTime = Math.min(1.5, Math.max(0.05, duration * 0.08));
    },
    { once: true },
  );
  probe.addEventListener(
    "seeked",
    () => {
      try {
        // Wide enough for the 420 px chat box and the filmstrip; the clip is
        // local by this point, so one frame costs nothing.
        const width = 480;
        const height = Math.max(
          1,
          Math.round((probe.videoHeight / probe.videoWidth) * width) || 180,
        );
        const canvas = document.createElement("canvas");
        canvas.width = width;
        canvas.height = height;
        const context = canvas.getContext("2d");
        if (!context) throw new Error("no 2d context");
        context.drawImage(probe, 0, 0, width, height);
        const data = canvas.toDataURL("image/jpeg", 0.72);
        if (data && data.startsWith("data:image/")) {
          state.videoPosters.set(id, data);
          while (state.videoPosters.size > MAX_VIDEO_POSTERS) {
            const oldest = state.videoPosters.keys().next().value;
            if (oldest === undefined) break;
            state.videoPosters.delete(oldest);
          }
          applyThumb(id);
        }
      } catch {
        /* the asset scheme may not allow canvas readback; keep the play tile */
      } finally {
        cleanup();
      }
    },
    { once: true },
  );
  probe.addEventListener("error", cleanup, { once: true });
  timer = setTimeout(cleanup, 12000);
  probe.src = cached.url;
}

// If the video is on disk, sample a real frame for the poster; otherwise leave
// the play tile alone until the clip is downloaded.
function ensureVideoPoster(id) {
  if (
    !id ||
    state.videoPosters.has(id) ||
    state.posterPending.has(id) ||
    state.posterChecking.has(id)
  ) {
    return;
  }
  const cached = state.media.get(id);
  if (cached?.url) {
    ensureDuration(id);
    captureVideoPoster(id);
    return;
  }
  if (cached?.status === "error") return;
  state.posterChecking.add(id);
  invoke("cached_media_path", { id })
    .then((path) => {
      if (!path) return;
      const url = convertFileSrc ? convertFileSrc(path) : path;
      rememberMedia(id, url);
      ensureDuration(id);
      captureVideoPoster(id);
    })
    .catch(() => {})
    .finally(() => state.posterChecking.delete(id));
}

function mediaElement(kind, url, id) {
  if (kind === "audio") {
    return audioPlayerElement(id, url, findMessageById(id));
  }
  if (kind === "video") {
    ensureThumb(id);
    // The clip is on disk by the time this renders, so the sampled frame
    // replaces the blurry inline poster as soon as the canvas readback lands.
    ensureVideoPoster(id);
    ensureDuration(id);
    const poster = thumbOf(id);
    const seconds = state.durations.get(id);
    const duration = seconds ? formatDuration(seconds) : "";
    return `<div class="media-video-wrap" data-media-id="${escapeHtml(
      id || "",
    )}" data-media-kind="video" data-thumb-id="${escapeHtml(id || "")}"><video class="media-video" data-media-id="${escapeHtml(
      id || "",
    )}" src="${escapeHtml(url)}"${
      poster ? ` poster="${poster}"` : ""
    } playsinline preload="metadata"></video><span class="media-video-play">${waIcon("play_arrow")}</span><span class="media-video-duration" data-duration-id="${escapeHtml(
      id || "",
    )}">${duration}</span></div>`;
  }
  return `<a class="media-image" href="${escapeHtml(url)}" target="_blank" rel="noreferrer" data-media-id="${escapeHtml(
    id || "",
  )}" data-media-kind="image"><img src="${escapeHtml(url)}" alt="${MEDIA_LABELS[kind] || "Media"}" /></a>`;
}

/* --------------------------------------------------- audio/voice-note player */

const AUDIO_MAX_WAVE_BARS = 64;

// The sender's waveform bytes (0..255 amplitudes) become the bars WhatsApp
// draws behind a voice note's progress; each downsampled peak is one flex bar
// scaled by its relative loudness, capped so clusters render cleanly.
function buildWaveBars(samples) {
  const count = samples.length;
  if (!count) return "";
  const step = Math.max(1, Math.ceil(count / AUDIO_MAX_WAVE_BARS));
  const bars = [];
  for (let i = 0; i < count; i += step) {
    let peak = 0;
    for (let j = i; j < Math.min(i + step, count); j += 1) {
      peak = Math.max(peak, samples[j]);
    }
    bars.push(Math.min(1, Math.max(0.1, peak / 255)));
  }
  return bars.map((h) => `<i style="--h:${Math.round(h * 100)}%"></i>`).join("");
}

// Official-layout audio bubble body: play/pause, a seek line (or the sender's
// waveform for recorded voice notes) and a footer with the duration next to
// the usual time/ticks, like "0:17   6:46 am" on WhatsApp desktop. The hidden
// <audio> does the playback; the delegated media-event handlers at the bottom
// of this file paint progress on every live copy of the bubble.
function audioPlayerElement(id, url, message) {
  const audio = (message && message.audio) || null;
  const ptt = Boolean(audio && audio.ptt);
  const seconds = Number((audio || {}).seconds) || 0;
  const wave = ptt ? buildWaveBars((audio || {}).waveform || []) : "";
  const dim = wave ? `<div class="audio-wave dim">${wave}</div>` : "";
  const lit = wave
    ? `<div class="audio-wave lit" data-audio-wave>${wave}</div>`
    : "";
  const stamp = message
    ? `<span class="meta audio-stamp">${timeOf(message.timestamp)}${tickHtml(message)}</span>`
    : '<span class="meta audio-stamp"></span>';
  return `<div class="audio-player${ptt ? " ptt" : ""}" data-audio-id="${escapeHtml(
    id || "",
  )}">
      <div class="audio-row">
        <button type="button" class="audio-play" aria-label="Play audio">
          <svg class="glyph glyph-play" viewBox="0 -960 960 960"><path d="M320-200v-560l440 280-440 280Zm80-280Zm0 134 210-134-210-134v268Z"/></svg>
          <svg class="glyph glyph-pause" viewBox="0 -960 960 960"><path d="M520-200v-560h240v560H520Zm-320 0v-560h240v560H200Zm400-80h80v-400h-80v400Zm-320 0h80v-400h-80v400Zm0-400v400-400Zm320 0v400-400Z"/></svg>
        </button>
        <div class="audio-track" data-audio-track>
          <div class="audio-line"><span class="audio-fill" data-audio-fill></span></div>
          ${dim}${lit}
          <span class="audio-dot" data-audio-dot></span>
        </div>
        <span class="audio-badge" aria-hidden="true"><svg viewBox="0 -960 960 960"><path d="M360-120H200q-33 0-56.5-23.5T120-200v-280q0-75 28.5-140.5t77-114q48.5-48.5 114-77T480-840q75 0 140.5 28.5t114 77q48.5 48.5 77 114T840-480v280q0 33-23.5 56.5T760-120H600v-320h160v-40q0-117-81.5-198.5T480-760q-117 0-198.5 81.5T200-480v40h160v320Zm-80-240h-80v160h80v-160Zm400 0v160h80v-160h-80Zm-400 0h-80 80Zm400 0h80-80Z"/></svg></span>
      </div>
      <div class="audio-foot">
        <span class="audio-dur" data-audio-dur>${seconds ? formatDuration(seconds) : ""}</span>
        ${stamp}
      </div>
      <audio class="audio-src" src="${escapeHtml(url)}" preload="metadata" data-audio-id="${escapeHtml(
        id || "",
      )}"></audio>
    </div>`;
}

/* The waveforms are drawn twice: the dim copy as the unplayed background and
   the lit copy clipped from the left, which stays permissive when a re-render
   replaces the node (the handlers repaint whatever is on screen). */
function audioApplyProgress(player, ratio) {
  const fill = player.querySelector("[data-audio-fill]");
  if (fill) fill.style.width = `${ratio * 100}%`;
  const wave = player.querySelector("[data-audio-wave]");
  if (wave) wave.style.clipPath = `inset(0 ${(1 - ratio) * 100}% 0 0)`;
  const dot = player.querySelector("[data-audio-dot]");
  if (dot) dot.style.left = `${ratio * 100}%`;
}

// Repaint one <audio> onto its bubble: play/pause glyph, line/waveform
// progress, dot position, and the duration label — which counts down like the
// official client while playing and shows the total length otherwise.
function audioPaint(el) {
  const player = el.closest(".audio-player");
  if (!player) return;
  const duration = Number.isFinite(el.duration) && el.duration > 0 ? el.duration : 0;
  const ratio = duration ? Math.min(1, el.currentTime / duration) : 0;
  const playing = !el.paused && !el.ended;
  player.classList.toggle("playing", playing);
  audioApplyProgress(player, ratio);
  const label = player.querySelector("[data-audio-dur]");
  if (label && duration) {
    label.textContent = formatDuration(playing ? duration - el.currentTime : duration);
  }
  if (!duration) {
    const message = findMessageById(player.dataset.audioId);
    const senderSeconds = Number((message?.audio || {}).seconds) || 0;
    if (senderSeconds && label) label.textContent = formatDuration(senderSeconds);
    else if (el.readyState >= 1 && label) label.textContent = formatDuration(el.duration || 0);
  }
}

// Only one audio bubble plays at a time, like WhatsApp Web.
let activeAudio = null;

function toggleAudio(player) {
  const el = player.querySelector("audio.audio-src");
  if (!el) return;
  if (activeAudio && activeAudio !== el && !activeAudio.paused) activeAudio.pause();
  if (el.paused) {
    if (!Number.isFinite(el.duration) && el.readyState >= 2) {
      // A finished file revived after scrolling: rewind before replaying.
      el.currentTime = 0;
    }
    el.play().catch(() => {});
  } else {
    el.pause();
  }
}

/* Delegated <audio> events (capture phase) so every rendered copy of a bubble
   is painted even when a re-render swapped the node mid-download. */
const AUDIO_EVENT_NAMES = ["play", "playing", "pause", "ended", "timeupdate", "loadedmetadata"];
for (const name of AUDIO_EVENT_NAMES) {
  document.addEventListener(
    name,
    (event) => {
      const el = event.target;
      if (!(el instanceof HTMLAudioElement) || !el.classList.contains("audio-src")) return;
      // Like the official client, a finished clip rewinds: the dot returns to
      // the start of the line and the label shows the full duration again.
      if (event.type === "ended") el.currentTime = 0;
      audioPaint(el);
    },
    true,
  );
}

document.addEventListener("click", (event) => {
  const button = event.target.closest(".audio-player .audio-play");
  if (!button) return;
  const player = button.closest(".audio-player");
  if (player) toggleAudio(player);
});

document.addEventListener("pointerdown", (event) => {
  const track = event.target.closest(".audio-player .audio-track");
  if (!track) return;
  const player = track.closest(".audio-player");
  const el = player?.querySelector("audio.audio-src");
  if (!el) return;
  event.preventDefault();
  const scrub = (ev) => {
    const rect = track.getBoundingClientRect();
    if (!rect.width) return;
    const ratio = Math.min(1, Math.max(0, (ev.clientX - rect.left) / rect.width));
    if (Number.isFinite(el.duration) && el.duration > 0) {
      el.currentTime = ratio * el.duration;
      audioPaint(el);
    }
  };
  scrub(event);
  const move = (ev) => scrub(ev);
  const stop = () => {
    window.removeEventListener("pointermove", move);
    window.removeEventListener("pointerup", stop);
  };
  window.addEventListener("pointermove", move);
  window.addEventListener("pointerup", stop);
});

// A video element takes its layout size from its poster (WhatsApp ships a tiny
// inline JPEG), so the wrapper is reshaped from the real clip dimensions.
// Channel cards take the shape of their clip, like the desktop app: a portrait
// video is tall instead of sitting in black bars inside a 16:10 frame. Chat
// media is bounded by WhatsApp's 420 px box and narrows for a portrait clip.
const CHAT_MEDIA_MAX_PX = 420;

document.addEventListener(
  "loadedmetadata",
  (event) => {
    const video = event.target;
    if (!(video instanceof HTMLVideoElement)) return;
    const wrap = video.closest(".media-video-wrap");
    if (!wrap) return;
    const { videoWidth, videoHeight } = video;
    if (!videoWidth || !videoHeight) return;
    const real = videoWidth / videoHeight;

    if (wrap.closest(".channel-view")) {
      const ratio = Math.min(Math.max(real, 0.5), 1.91);
      wrap.style.aspectRatio = String(ratio);
      // A portrait clip keeps its full height and narrows instead of showing black
      // bars inside a wide frame.
      if (ratio < 1) {
        wrap.style.width = `${Math.round(520 * ratio)}px`;
        wrap.style.marginInline = "auto";
      }
      return;
    }

    // Chats: fit the clip inside the box, keeping its shape.
    let width = Math.min(videoWidth, CHAT_MEDIA_MAX_PX);
    let height = width / real;
    if (height > CHAT_MEDIA_MAX_PX) {
      height = CHAT_MEDIA_MAX_PX;
      width = Math.round(height * real);
    }
    wrap.style.width = `${Math.round(width)}px`;
    wrap.style.aspectRatio = String(real);
  },
  true,
);

function mediaBlock(message) {
  const cached = state.media.get(message.id);
  if (cached?.status === "ready" && cached.url) {
    return mediaElement(message.kind, cached.url, message.id);
  }

  if (cached?.status === "error") {
    // Already failed: render the retry tile instead of starting the download
    // again on every re-render.
    const preview = thumbOf(message.id);
    return `<div class="media media-box error retry ${preview ? "has-thumb" : ""}" data-media-id="${escapeHtml(
      message.id,
    )}" data-media-kind="${escapeHtml(message.kind)}" data-failed="1"${
      preview ? ` style="background-image:url('${preview}')"` : ""
    }>${preview ? "" : mediaIcon(message.kind)}<span class="media-retry-hint">Tap to retry</span></div>`;
  }

  const isVideo = message.kind === "video";
  const isAudio = message.kind === "audio";
  if (isVideo) {
    ensureThumb(message.id);
    ensureVideoPoster(message.id);
  } else if (message.kind === "image") {
    // WhatsApp ships a small JPEG with every photo. Showing it immediately
    // beats an endless "Loading photo..." when the CDN path has expired.
    ensureThumb(message.id);
  }
  const poster = isVideo ? thumbOf(message.id) : "";
  const play = isVideo ? `<span class="media-play">${waIcon("play_arrow")}</span>` : "";
  const hint = isVideo
    ? "Tap to load video"
    : message.kind === "sticker"
      ? "Loading sticker..."
      : isAudio
        ? "Loading audio..."
        : "Loading photo...";
  return `<div class="media media-box ${isVideo ? "video-box" : ""} ${
    poster ? "has-thumb" : ""
  }" data-media-id="${escapeHtml(message.id)}" data-media-kind="${escapeHtml(
    message.kind,
  )}" data-thumb-id="${escapeHtml(message.id)}"${
    poster ? ` style="background-image:url('${poster}')"` : ""
  }>${play}<span class="media-icon">${mediaIcon(message.kind)}</span><span class="media-hint">${hint}</span></div>`;
}

let mediaObserver = null;

/* --------------------------------------------------------- document cards */

// Badge colour per extension, mirroring WhatsApp's file cards.
const DOC_TONES = {
  pdf: "pdf",
  doc: "word",
  docx: "word",
  odt: "word",
  rtf: "word",
  pages: "word",
  xls: "sheet",
  xlsx: "sheet",
  csv: "sheet",
  ods: "sheet",
  ppt: "slides",
  pptx: "slides",
  odp: "slides",
  zip: "archive",
  rar: "archive",
  "7z": "archive",
  gz: "archive",
  tar: "archive",
  png: "image",
  jpg: "image",
  jpeg: "image",
  gif: "image",
  webp: "image",
  bmp: "image",
  heic: "image",
  vsd: "vector",
  vsdx: "vector",
  svg: "vector",
  ai: "vector",
  dwg: "vector",
  dxf: "vector",
  txt: "plain",
  log: "plain",
  md: "plain",
  json: "plain",
  xml: "plain",
  mp3: "audio",
  wav: "audio",
  m4a: "audio",
  ogg: "audio",
  mp4: "video",
  mov: "video",
  mkv: "video",
  avi: "video",
};

const DOC_MIME_EXTENSIONS = {
  "application/pdf": "pdf",
  "image/png": "png",
  "image/jpeg": "jpg",
  "image/webp": "webp",
  "image/gif": "gif",
  "application/msword": "doc",
  "application/vnd.openxmlformats-officedocument.wordprocessingml.document": "docx",
  "application/vnd.ms-excel": "xls",
  "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet": "xlsx",
  "application/vnd.ms-powerpoint": "ppt",
  "application/vnd.openxmlformats-officedocument.presentationml.presentation": "pptx",
  "application/zip": "zip",
  "text/csv": "csv",
  "text/plain": "txt",
};

const DOC_PREVIEWABLE = ["pdf", "png", "jpg", "jpeg", "gif", "webp", "bmp", "txt", "csv", "log", "md", "json", "xml"];

function extensionOf(name) {
  const match = /\.([A-Za-z0-9]{1,8})$/.exec(String(name || "").trim());
  return match ? match[1].toLowerCase() : "";
}

function documentInfo(message) {
  const doc = message.document || {};
  let name = String(doc.name || message.text || "").trim();
  let ext = extensionOf(name);
  if (!ext) {
    ext = DOC_MIME_EXTENSIONS[String(doc.mime || "").toLowerCase()] || "";
    if (ext) name = `${name || "document"}.${ext}`;
  }
  return { name: name || "Document", ext: ext || "" };
}

function formatBytes(bytes) {
  const value = Number(bytes) || 0;
  if (value <= 0) return "";
  const units = ["B", "kB", "MB", "GB"];
  let size = value;
  let unit = 0;
  while (size >= 1024 && unit < units.length - 1) {
    size /= 1024;
    unit += 1;
  }
  return `${size.toFixed(size >= 100 || unit === 0 ? 0 : 1)} ${units[unit]}`;
}

function documentMeta(message) {
  const doc = message.document || {};
  const { ext } = documentInfo(message);
  const parts = [];
  if (doc.pages > 0) parts.push(doc.pages === 1 ? "1 page" : `${doc.pages} pages`);
  if (ext) parts.push(ext.toUpperCase());
  const size = formatBytes(doc.size);
  if (size) parts.push(size);
  return parts.join(" · ");
}

function documentBadge(ext, extra = "") {
  const tone = DOC_TONES[ext] || "plain";
  const label = (ext || "file").toUpperCase().slice(0, 5);
  return `<span class="doc-badge doc-${tone} ${extra}">${escapeHtml(label)}</span>`;
}

/// WhatsApp-Web style file card: a full-width preview thumbnail, the badge,
/// the file name with its extension and the size/pages line. The caption and
/// timestamp live in the footer row rendered by `chat-render.js`.
function documentCard(message) {
  const { name, ext } = documentInfo(message);
  const doc = message.document || {};
  const meta = documentMeta(message);
  // Embedded JPEG first, then a shell-generated preview of a local file.
  const preview = doc.thumb || documentThumbs.get(message.id) || "";
  const thumb = preview
    ? `<img class="doc-thumb" src="${escapeHtml(preview)}" alt="" loading="lazy" />`
    : "";
  const id = escapeHtml(message.id);
  return `${thumb ? `<div class="doc-preview">${thumb}</div>` : ""}
    <div class="doc-card" data-doc-message="${id}" data-doc-name="${escapeHtml(name)}">
      <div class="doc-main">
        ${documentBadge(ext)}
        <div class="doc-text">
          <div class="doc-name" title="${escapeHtml(name)}">${escapeHtml(name)}</div>
          ${meta ? `<div class="doc-meta">${escapeHtml(meta)}</div>` : ""}
        </div>
      </div>
    </div>`;
}

/// View / Save as row; rendered before the timestamp so it sits at the bottom of
/// the bubble like WhatsApp's file card. Types that cannot be previewed in the
/// app get a single Download button, exactly like WhatsApp Web.
function documentActions(message) {
  const { name, ext } = documentInfo(message);
  const id = escapeHtml(message.id);
  const save = `<button type="button" data-doc-action="save" data-doc-id="${id}" data-doc-name="${escapeHtml(
    name,
  )}">`;
  if (!DOC_PREVIEWABLE.includes(ext)) {
    return `<div class="doc-actions">${save}Download</button></div>`;
  }
  return `<div class="doc-actions">
      <button type="button" data-doc-action="view" data-doc-id="${id}">View</button>
      ${save}Save as&hellip;</button>
    </div>`;
}

// Chats cached before the file card existed have no document metadata. Ask the
// Rust side (which still holds the protobuf) once per message.
const documentInfoRequests = new Set();
const documentThumbRequests = new Set();
// Shell-generated previews stay in memory: they are large and can be rebuilt,
// so they must not bloat the persisted chat cache.
const documentThumbs = new Map();

function hydrateDocumentCards(box) {
  for (const node of box.querySelectorAll("[data-doc-message]")) {
    const id = node.dataset.docMessage;
    const entry = state.chats.get(state.active);
    const message = entry?.messages?.find((item) => item.id === id);
    if (!message) continue;

    if (!message.document && !documentInfoRequests.has(id)) {
      documentInfoRequests.add(id);
      invoke("document_info", { id })
        .then((doc) => {
          if (!doc) return;
          message.document = doc;
          scheduleSave();
          if (state.active === message.chat) scheduleChatRender();
        })
        .catch(() => {});
      continue;
    }

    if (message.document?.thumb || documentThumbs.has(id)) continue;
    if (!documentThumbRequests.has(id)) {
      documentThumbRequests.add(id);
      // First the thumbnail shipped with the message, then a preview of a
      // file that is already on disk (Windows shell thumbnail).
      invoke("media_thumbnail", { id })
        .then((thumb) => thumb || invoke("file_thumbnail", { id }))
        .then((thumb) => {
          if (!thumb) return;
          documentThumbs.set(id, thumb);
          if (state.active === message.chat) scheduleChatRender();
        })
        .catch(() => {});
    }
  }
}

let documentViewerMessage = null;

function findMessageById(id) {
  for (const entry of state.chats.values()) {
    const found = (entry.messages || []).find((message) => message.id === id);
    if (found) return found;
  }
  return null;
}

/// Preview a document inside the app. PDFs and photos render in a frame; other
/// types fall back to Open with the default app.
async function openDocumentViewer(id) {
  const message = findMessageById(id);
  if (!message) return;
  const { name, ext } = documentInfo(message);
  const layer = $("document-viewer");
  const body = $("doc-viewer-body");
  documentViewerMessage = message;
  $("doc-viewer-name").textContent = name;
  $("doc-viewer-ext").textContent = (ext || "file").toUpperCase().slice(0, 5);
  $("doc-viewer-ext").className = `doc-badge doc-${DOC_TONES[ext] || "plain"}`;
  $("doc-viewer-save").dataset.docId = message.id;
  $("doc-viewer-save").dataset.docName = name;
  $("doc-viewer-open").dataset.docId = message.id;
  $("doc-viewer-reveal").dataset.docId = message.id;
  body.innerHTML = `<div class="doc-loading">Loading ${escapeHtml(name)}&hellip;</div>`;
  layer.hidden = false;

  // Already in the media cache (a document we sent, or one opened before):
  // preview straight away instead of waiting for another fetch round-trip.
  const cached = state.media.get(message.id);
  if (cached?.status === "ready" && cached.url) {
    showDocumentPreview(cached.url, ext, name);
    return;
  }

  try {
    const path = await invoke("fetch_media", { id: message.id });
    if (documentViewerMessage !== message) return;
    const url = convertFileSrc ? convertFileSrc(path) : path;
    showDocumentPreview(url, ext, name);
  } catch (err) {
    // A file that is still in the local media cache (for example a document we
    // just sent) can preview straight from its blob URL.
    const cached = state.media.get(message.id);
    if (cached?.status === "ready" && cached.url) {
      if (documentViewerMessage === message) showDocumentPreview(cached.url, ext, name);
      return;
    }
    if (documentViewerMessage !== message) return;
    body.innerHTML = `<div class="doc-plain"><p>Could not load this file.</p><p class="muted">${escapeHtml(
      String(err),
    )}</p><p class="muted">Use Open to launch it with the default app, or Save as&hellip; to keep a copy.</p></div>`;
  }
}

/// PDFs and photos render in place; anything else gets the file-card fallback.
function showDocumentPreview(url, ext, name) {
  const body = $("doc-viewer-body");
  if (ext === "pdf") {
    body.innerHTML = `<iframe class="doc-frame" src="${escapeHtml(
      url,
    )}" title="${escapeHtml(name)}"></iframe>`;
  } else if (["png", "jpg", "jpeg", "gif", "webp", "bmp"].includes(ext)) {
    body.innerHTML = `<img class="doc-image" src="${escapeHtml(url)}" alt="${escapeHtml(name)}" />`;
  } else {
    body.innerHTML = `<div class="doc-plain">${documentBadge(ext)}
        <p>This file type cannot be previewed inside WA Desk.</p>
        <p class="muted">Open it with the default app, or save a copy with Save as&hellip;.</p>
      </div>`;
  }
}

function closeDocumentViewer() {
  const layer = $("document-viewer");
  if (!layer || layer.hidden) return;
  documentViewerMessage = null;
  layer.hidden = true;
  $("doc-viewer-body").innerHTML = "";
}

async function saveDocument(id, name) {
  // One write per click: disable every Save button for that message until the
  // copy is on disk, and show immediate feedback (the backend may wait on
  // WhatsApp's re-upload handshake for a few seconds).
  const buttons = [
    ...document.querySelectorAll(`[data-doc-action="save"][data-doc-id="${id}"]`),
  ];
  const label = name || "file";
  for (const button of buttons) {
    button.dataset.label = button.textContent;
    button.textContent = "Downloading\u2026";
    button.disabled = true;
  }
  toast(`Downloading ${label}\u2026`);
  try {
    const path = await invokeWithTimeout(
      "download_media",
      { id, name: name || null },
      45000,
    );
    const saved = String(path).split(/[\\/]/).pop();
    toast(`Saved ${saved} to Downloads\\WA Desk`);
  } catch (err) {
    toast(friendlyMediaError(err));
  } finally {
    for (const button of buttons) {
      button.disabled = false;
      if (button.dataset.label) {
        button.textContent = button.dataset.label;
        delete button.dataset.label;
      }
    }
  }
}

/// WhatsApp expires CDN paths; say so instead of showing a raw status code.
function friendlyMediaError(err) {
  const text = String(err);
  if (/\b(403|404|410)\b/.test(text)) {
    return "WhatsApp no longer serves this file. Ask the sender to resend it.";
  }
  if (/timed out|too long|timeout/i.test(text)) {
    return "The download timed out. Check your connection and try again.";
  }
  return text;
}

async function openDocumentExternally(id) {
  try {
    await invoke("open_media", { id });
  } catch (err) {
    toast(friendlyMediaError(err));
  }
}

async function revealDocument(id) {
  try {
    await invoke("reveal_media", { id });
  } catch (err) {
    toast(friendlyMediaError(err));
  }
}

/* -------------------------------------------------------- media helpers */

// A stalled CDN request should become a retryable tile instead of an endless
// "Loading photo..." spinner.
const MEDIA_LOAD_TIMEOUT_MS = 12000;

// Independent per-message watchdog: even if the invoke promise never settles
// (a 30 s media-reupload wait on WhatsApp's side), the tile turns into a
// retryable state after the timeout.
const mediaWatchdogs = new Map();

function armMediaWatchdog(id, kind) {
  if (mediaWatchdogs.has(id)) return;
  mediaWatchdogs.set(
    id,
    setTimeout(() => {
      mediaWatchdogs.delete(id);
      if (state.media.get(id)?.status !== "loading") return;
      state.media.set(id, { status: "error", message: "the download timed out" });
      markMediaFailed(id, kind);
    }, MEDIA_LOAD_TIMEOUT_MS),
  );
}

function clearMediaWatchdog(id) {
  const timer = mediaWatchdogs.get(id);
  if (timer) clearTimeout(timer);
  mediaWatchdogs.delete(id);
}

function ensureMediaObserver() {
  if (mediaObserver) return mediaObserver;
  mediaObserver = new IntersectionObserver(
    (entries) => {
      for (const entry of entries) {
        if (!entry.isIntersecting) continue;
        const node = entry.target;
        mediaObserver.unobserve(node);
        loadMedia(node.dataset.mediaId, node.dataset.mediaKind, node);
      }
    },
    { root: $("messages"), rootMargin: "300px 0px" },
  );
  return mediaObserver;
}

function hydrateMedia(box) {
  const observer = ensureMediaObserver();
  // Only placeholder boxes need hydrating. Already-rendered <img>/<video>
  // elements carry data-media-id too, and re-hydrating them used to replace a
  // cached video with an <img src="*.mp4"> (a broken "Media" tile).
  for (const node of box.querySelectorAll(".media-box[data-media-id]")) {
    // Failed tiles wait for an explicit tap; never re-fetch them on render.
    if (node.dataset.failed) continue;
    const id = node.dataset.mediaId;
    const kind = node.dataset.mediaKind || "image";
    const cached = state.media.get(id);
    if (cached?.status === "ready" && cached.url) {
      node.outerHTML = mediaElement(kind, cached.url, id);
      if (kind === "video") {
        ensureThumb(id);
        ensureVideoPoster(id);
        ensureDuration(id);
        captureVideoPoster(id);
      }
      continue;
    }
    if (kind === "video") {
      ensureThumb(id);
      // Channel posts behave like the desktop app: the poster and clip load as
      // soon as the card is visible, no tap needed. Chats keep tap-to-play so a
      // long video never downloads by accident.
      if (isChannelJid(state.active)) {
        observer.observe(node);
        continue;
      }
      node.addEventListener("click", () => {
        viewerPendingId = id;
        loadMedia(id, kind, node);
      });
    } else if (node.classList.contains("error")) {
      node.addEventListener("click", () => {
        state.media.delete(id);
        node.classList.remove("error");
        loadMedia(id, kind, node);
      });
    } else {
      observer.observe(node);
    }
  }
}

async function loadMedia(id, kind, node) {
  if (!id || !node || !node.isConnected) return;
  if (state.media.get(id)?.status === "loading") return;
  state.media.set(id, { status: "loading" });
  node.classList.add("loading");
  armMediaWatchdog(id, kind);

  try {
    const path = await Promise.race([
      invoke("fetch_media", { id }),
      new Promise((_, reject) => {
        setTimeout(
          () => reject(new Error("the download is taking too long")),
          MEDIA_LOAD_TIMEOUT_MS,
        );
      }),
    ]);
    clearMediaWatchdog(id);
    const url = convertFileSrc ? convertFileSrc(path) : path;
    rememberMedia(id, url);
    // Update every live copy: a re-render may have replaced the node that
    // started this download.
    for (const tile of document.querySelectorAll(`.media-box[data-media-id="${id}"]`)) {
      tile.outerHTML = mediaElement(kind, url, id);
    }
    if (kind === "video") {
      ensureThumb(id);
      ensureVideoPoster(id);
      ensureDuration(id);
      captureVideoPoster(id);
    }
    if (viewerPendingId === id) {
      viewerPendingId = null;
      openMediaViewer(id);
    }
  } catch (err) {
    clearMediaWatchdog(id);
    state.media.set(id, { status: "error", message: String(err) });
    markMediaFailed(id, kind);
  }
}

/// Paint the failed/timed-out state on every live tile for a message, so a
/// re-render during the download cannot leave an endless "Loading photo...".
function markMediaFailed(id, kind) {
  for (const tile of document.querySelectorAll(`.media-box[data-media-id="${id}"]`)) {
    tile.classList.add("error", "retry");
    tile.classList.remove("loading");
    tile.dataset.failed = "1";
    // Keep the embedded thumbnail visible when there is one; it is still a
    // usable preview of the photo even though the full file expired.
    tile.innerHTML = thumbOf(id)
      ? `<span class="media-retry-hint">Tap to retry</span>`
      : `<span>${MEDIA_LABELS[kind] || "Media"} unavailable</span><span class="media-retry-hint">Tap to retry</span>`;
  }
}

/// Retry a media tile that failed or timed out. The Rust side keeps the
/// protobuf, so another attempt can still succeed once the CDN path is renewed.
function retryMedia(node) {
  const id = node.dataset.mediaId;
  const kind = node.dataset.mediaKind || "image";
  if (!id) return;
  state.media.delete(id);
  node.classList.remove("error", "retry");
  node.removeAttribute("data-failed");
  if (!thumbOf(id)) {
    node.innerHTML = `<span class="media-icon">${mediaIcon(kind)}</span><span class="media-hint">Loading ${
      MEDIA_LABELS[kind] || "media"
    }...</span>`;
  }
  loadMedia(id, kind, node);
}

/* ---------------------------------------------------------- link previews */

function previewCard(data) {
  return `<a class="preview-card" href="${escapeHtml(data.url)}" target="_blank" rel="noreferrer">
      ${
        data.image
          ? `<img class="preview-img" src="${escapeHtml(data.image)}" alt="" loading="lazy" referrerpolicy="no-referrer" onerror="this.remove()" />`
          : ""
      }
      <div class="preview-body">
        <strong class="preview-title">${escapeHtml(data.title || data.url)}</strong>
        ${data.description ? `<span class="preview-desc">${escapeHtml(data.description)}</span>` : ""}
        <span class="preview-site">${escapeHtml(data.site || "Link")}</span>
      </div>
    </a>`;
}

function hydratePreviews(box) {
  for (const node of box.querySelectorAll("[data-preview-url]")) {
    const url = node.dataset.previewUrl;
    const cached = state.previews.get(url);

    if (cached?.status === "ready") {
      // Low-signal cards (a site name without image or description) look wrong;
      // WhatsApp always ships a real title or thumbnail with the message.
      if (!cached.data.image && !cached.data.description) {
        node.remove();
        continue;
      }
      node.innerHTML = previewCard(cached.data);
      node.classList.add("ready");
      continue;
    }
    if (cached?.status === "error") {
      node.remove();
      continue;
    }
    if (cached) continue;

    state.previews.set(url, { status: "loading" });
    invoke("link_preview", { url })
      .then((data) => {
        state.previews.delete(url);
        state.previews.set(url, { status: "ready", data });
        while (state.previews.size > 200) {
          state.previews.delete(state.previews.keys().next().value);
        }
        if (!data.image && !data.description) {
          if (node.isConnected) node.remove();
          return;
        }
        if (node.isConnected) {
          node.innerHTML = previewCard(data);
          node.classList.add("ready");
        }
      })
      .catch(() => {
        state.previews.set(url, { status: "error" });
        if (node.isConnected) node.remove();
      });
  }
}

// Channel/broadcast link posts carry their big preview image as a CDN path
// (`thumbnailDirectPath`); the card shows the small inline thumb first and
// swaps in the large image when it lands, like WhatsApp Web.
function hydrateLinkThumbs(box) {
  for (const node of box.querySelectorAll(".link-preview[data-thumb-id]")) {
    const id = node.dataset.thumbId;
    if (!id || node.dataset.thumbFetched) continue;
    node.dataset.thumbFetched = "1";
    const apply = (data) => {
      if (!data || !node.isConnected) return;
      const card = node.querySelector(".preview-card");
      if (!card) return;
      const img = card.querySelector(".preview-img");
      if (img) {
        // Swap the small inline JPEG for the large copy; `onload` avoids a
        // flash of the broken-image icon if the CDN object is gone.
        const probe = new Image();
        probe.onload = () => {
          img.src = data;
        };
        probe.src = data;
        return;
      }
      const body = card.querySelector(".preview-body");
      const created = document.createElement("img");
      created.className = "preview-img";
      created.alt = "";
      created.loading = "lazy";
      created.addEventListener("error", () => created.remove(), { once: true });
      created.src = data;
      card.insertBefore(created, body);
    };
    // Re-renders reuse the LRU: a second pass never re-downloads the image.
    const cachedThumbnail = state.thumbs.get(`link:${id}`);
    if (cachedThumbnail) {
      apply(cachedThumbnail);
      continue;
    }
    if (linkThumbPending.has(id)) continue;
    linkThumbPending.add(id);
    invoke("link_thumbnail", { id })
      .then((data) => {
        if (data) {
          state.thumbs.set(`link:${id}`, data);
          while (state.thumbs.size > MAX_THUMBS) {
            state.thumbs.delete(state.thumbs.keys().next().value);
          }
          apply(data);
        }
      })
      .catch(() => {})
      .finally(() => linkThumbPending.delete(id));
  }
}

