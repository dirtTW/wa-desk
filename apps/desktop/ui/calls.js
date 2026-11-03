/* WA Desk frontend: calls. Loaded in order by index.html; all files share the classic-script global scope. */

/* ------------------------------------------------------------------ */
/* protocol events                                                     */
/* ------------------------------------------------------------------ */

/* ------------------------------------------------------------------ calls */

let currentCall = null;
let callMedia = null;
let callTimer = 0;

function callDisplayName(peer) {
  const chat = [...state.chats.values()].find((entry) => entry.jid === peer);
  if (chat) return chat.name;
  const phone = jidUser(peer);
  return phone ? `+${phone}` : peer;
}

function setCallStatus(text) {
  $("call-status").textContent = text;
}

async function showCallOverlay() {
  const call = currentCall;
  if (!call) return;
  const name = callDisplayName(call.peer);
  $("call-name").textContent = name;
  setAvatar($("call-avatar"), name, call.peer);
  $("call-overlay").hidden = false;
  // Buttons, stage visibility and the status line are decided by wa_ui::calls.
  const overlay = await invoke("ui_call_overlay", {
    input: {
      state: call.state || "calling",
      video: Boolean(call.video),
      send_video: Boolean(call.sendVideo),
      muted: Boolean(call.muted),
      started_at: call.startedAt || null,
      now: Date.now(),
    },
  }).catch(() => null);
  if (!overlay) return;
  $("call-video-stage").hidden = !overlay.video_stage;
  // A video call answered without a local camera still shows the peer; the
  // self-view stays hidden instead of painting a permanently black tile.
  $("call-local-video").hidden = !overlay.self_view;
  $("call-accept").hidden = !overlay.accept;
  $("call-reject").hidden = !overlay.reject;
  $("call-mute").hidden = !overlay.mute;
  $("call-hangup").hidden = !overlay.hangup;
  $("call-mute").classList.toggle("muted", overlay.mute_pressed);
  setCallStatus(overlay.status);
}

function clearCallTimer() {
  clearInterval(callTimer);
  callTimer = 0;
}

function startCallTimer() {
  clearCallTimer();
  if (!currentCall) return;
  currentCall.startedAt = Date.now();
  const tick = () => {
    if (!currentCall?.startedAt) return;
    // The "Connected · mm:ss" label is formatted by wa_ui::calls.
    invoke("ui_call_overlay", {
      input: {
        state: currentCall.state || "connected",
        video: Boolean(currentCall.video),
        send_video: Boolean(currentCall.sendVideo),
        muted: Boolean(currentCall.muted),
        started_at: currentCall.startedAt,
        now: Date.now(),
      },
    })
      .then((overlay) => setCallStatus(overlay.status))
      .catch(() => {});
  };
  tick();
  callTimer = setInterval(tick, 1000);
}

function stopCallMedia() {
  clearCallTimer();
  const media = callMedia;
  callMedia = null;
  if (!media) return;
  try { media.reader?.cancel(); } catch {}
  try { media.encoder?.close(); } catch {}
  try { media.decoder?.close(); } catch {}
  try { media.node?.disconnect(); } catch {}
  try { media.source?.disconnect(); } catch {}
  try { media.stream?.getTracks().forEach((track) => track.stop()); } catch {}
  try { media.context?.close(); } catch {}
  $("call-local-video").srcObject = null;
}

function finishCall(reason) {
  stopCallMedia();
  const hadCall = Boolean(currentCall);
  currentCall = null;
  $("call-overlay").hidden = true;
  $("call-video-stage").hidden = true;
  if (hadCall && reason) toast(reason);
}

async function hasVideoInputs() {
  try {
    if (typeof VideoEncoder !== "function") return false;
    const devices = await navigator.mediaDevices.enumerateDevices();
    if (!devices.some((device) => device.kind === "videoinput")) return false;
    const support = await VideoEncoder.isConfigSupported({
      codec: "avc1.42E01F",
      width: 640,
      height: 480,
      bitrate: 700000,
      framerate: 15,
      avc: { format: "annexb" },
    });
    return Boolean(support.supported);
  } catch {
    return false;
  }
}

async function mediaInputsReady(video) {
  try {
    const devices = await navigator.mediaDevices.enumerateDevices();
    if (!devices.some((device) => device.kind === "audioinput")) return false;
    if (!video) return true;
    return await hasVideoInputs();
  } catch {
    return false;
  }
}

async function startOutgoingCall(video) {
  const chat = state.chats.get(state.active);
  if (!chat) return;
  const guard = await invoke("ui_call_guard", {
    isGroup: Boolean(chat.isGroup),
    hasActiveCall: Boolean(currentCall),
  }).catch(() => ({ ok: true }));
  if (!guard.ok) {
    toast(guard.toast);
    return;
  }
  if (!(await mediaInputsReady(video))) {
    const message = await invoke("ui_call_media_toast", { video }).catch(() => "");
    if (message) toast(message);
    return;
  }
  try {
    const info = await invoke("start_call", { chat: chat.jid, video });
    currentCall = {
      callId: info.call_id,
      peer: info.peer,
      video: info.video,
      sendVideo: info.video,
      outgoing: true,
      state: "calling",
    };
    showCallOverlay();
    await startCallMedia(info.call_id, info.video, info.video);
  } catch (err) {
    await invoke("hangup_call").catch(() => {});
    finishCall();
    toast(String(err));
  }
}

async function acceptIncomingCall() {
  const call = currentCall;
  if (!call || call.state !== "ringing") return;
  if (!(await mediaInputsReady(false))) {
    toast("No microphone available; rejecting the call.");
    await rejectIncomingCall();
    return;
  }
  // A video offer must be answered as video even when this machine has no
  // camera: the media is negotiated once for both directions, so dropping the
  // video endpoints would also throw away the caller's frames.
  const offeredVideo = Boolean(call.video);
  let sendVideo = offeredVideo && (await hasVideoInputs());
  if (offeredVideo && !sendVideo) {
    toast("Camera unavailable; you'll see them, but they won't see you.");
  }
  try {
    const info = await invoke("accept_call", { callId: call.callId, video: offeredVideo });
    currentCall = {
      callId: info.call_id,
      peer: info.peer,
      video: info.video,
      sendVideo,
      outgoing: false,
      state: "connecting",
    };
    showCallOverlay();
    await startCallMedia(info.call_id, info.video, sendVideo);
  } catch (err) {
    await invoke("reject_call", { callId: call.callId }).catch(() => {});
    finishCall(String(err));
  }
}

async function rejectIncomingCall() {
  const call = currentCall;
  if (!call) return;
  try { await invoke("reject_call", { callId: call.callId }); } catch {}
  finishCall();
}

async function hangupCall() {
  try { await invoke("hangup_call"); } catch {}
  finishCall();
}

async function toggleCallMute() {
  if (!currentCall) return;
  currentCall.muted = !currentCall.muted;
  try { await invoke("set_call_muted", { muted: currentCall.muted }); } catch {}
  $("call-mute").classList.toggle("muted", currentCall.muted);
  if (callMedia?.micTrack) callMedia.micTrack.enabled = !currentCall.muted;
}

async function startCallMedia(callId, video, sendVideo = video) {
  const audioConstraints = {
    channelCount: 1,
    sampleRate: 16000,
    echoCancellation: true,
    noiseSuppression: true,
    autoGainControl: true,
  };
  const videoConstraints = {
    width: { ideal: 640 },
    height: { ideal: 480 },
    frameRate: { ideal: 15, max: 20 },
  };

  let stream;
  try {
    stream = await navigator.mediaDevices.getUserMedia({
      audio: audioConstraints,
      video: sendVideo ? videoConstraints : false,
    });
  } catch (err) {
    // A camera that refuses to open must not tear down a call that is already
    // negotiated: fall back to audio-only and keep the peer's video coming.
    if (!sendVideo) throw err;
    sendVideo = false;
    if (currentCall) currentCall.sendVideo = false;
    toast("Camera unavailable; continuing without your video.");
    stream = await navigator.mediaDevices.getUserMedia({ audio: audioConstraints, video: false });
  }

  const context = new AudioContext({ sampleRate: 16000 });
  await context.resume();
  await context.audioWorklet.addModule("call-audio-worklet.js");
  const source = context.createMediaStreamSource(stream);
  const node = new AudioWorkletNode(context, "call-mic");
  const silent = context.createGain();
  silent.gain.value = 0;
  source.connect(node);
  node.connect(silent);
  silent.connect(context.destination);

  node.port.onmessage = (event) => {
    const samples = Array.from(new Int16Array(event.data));
    invoke("call_audio_frame", { samples }).catch(() => {});
  };

  callMedia = {
    stream,
    context,
    source,
    node,
    silent,
    nextPlay: 0,
    micTrack: stream.getAudioTracks()[0] || null,
    decoder: null,
    encoder: null,
    reader: null,
    decoderKeySeen: false,
    sendVideo,
  };

  if (video) {
    // Receive first: starting the decoder before the (awaited) encoder setup
    // means the peer's first keyframe is not thrown away.
    startVideoDecode();
    const track = stream.getVideoTracks()[0];
    if (sendVideo && track) {
      const local = $("call-local-video");
      local.srcObject = new MediaStream([track]);
      local.play().catch(() => {});
      await startVideoEncode(track);
    } else {
      $("call-local-video").srcObject = null;
    }
  }
}

async function startVideoEncode(track) {
  if (typeof MediaStreamTrackProcessor !== "function" || typeof VideoEncoder !== "function") {
    toast("This WebView cannot share camera video; continuing without it.");
    if (currentCall) currentCall.sendVideo = false;
    $("call-local-video").hidden = true;
    return;
  }
  const config = {
    codec: "avc1.42E01F",
    width: 640,
    height: 480,
    bitrate: 700000,
    framerate: 15,
    avc: { format: "annexb" },
  };
  const support = await VideoEncoder.isConfigSupported(config);
  if (!support.supported || !callMedia) return;
  const encoder = new VideoEncoder({
    output: (chunk) => {
      const bytes = new Uint8Array(chunk.byteLength);
      chunk.copyTo(bytes);
      invoke("call_video_frame", { data: Array.from(bytes) }).catch(() => {});
    },
    error: () => {},
  });
  encoder.configure(config);
  const processor = new MediaStreamTrackProcessor({ track });
  const reader = processor.readable.getReader();
  callMedia.encoder = encoder;
  callMedia.reader = reader;
  let frameNumber = 0;
  (async () => {
    try {
      while (callMedia && callMedia.reader === reader) {
        const { value: frame, done } = await reader.read();
        if (done) break;
        try {
          encoder.encode(frame, { keyFrame: frameNumber++ % 30 === 0 });
        } finally {
          frame.close();
        }
      }
    } catch {
      /* camera stopped */
    }
  })();
}

function startVideoDecode() {
  const canvas = $("call-remote-canvas");
  if (typeof VideoDecoder !== "function") {
    // Receive-side video needs WebCodecs; keep the call alive without it.
    const draw = canvas.getContext("2d");
    if (draw) {
      draw.fillStyle = "#000";
      draw.fillRect(0, 0, canvas.width, canvas.height);
      draw.fillStyle = "#8696a0";
      draw.font = "16px Segoe UI, Roboto, sans-serif";
      draw.textAlign = "center";
      draw.fillText("Video decoding is not available", canvas.width / 2, canvas.height / 2);
    }
    return;
  }
  const draw = canvas.getContext("2d");
  const decoder = new VideoDecoder({
    output: (frame) => {
      try {
        const scale = Math.min(canvas.width / frame.displayWidth, canvas.height / frame.displayHeight);
        const width = frame.displayWidth * scale;
        const height = frame.displayHeight * scale;
        draw.fillStyle = "#000";
        draw.fillRect(0, 0, canvas.width, canvas.height);
        draw.drawImage(frame, (canvas.width - width) / 2, (canvas.height - height) / 2, width, height);
      } finally {
        frame.close();
      }
    },
    error: () => {},
  });
  decoder.configure({ codec: "avc1.42E01F", avc: { format: "annexb" } });
  if (callMedia) {
    callMedia.decoder = decoder;
    callMedia.decoderKeySeen = false;
  }
}

function playCallAudio(samples) {
  const media = callMedia;
  if (!media?.context || !samples?.length) return;
  const buffer = media.context.createBuffer(1, samples.length, 16000);
  const data = buffer.getChannelData(0);
  for (let i = 0; i < samples.length; i += 1) data[i] = samples[i] / 32768;
  const source = media.context.createBufferSource();
  source.buffer = buffer;
  source.connect(media.context.destination);
  const now = media.context.currentTime;
  if (!media.nextPlay || media.nextPlay < now + 0.02 || media.nextPlay > now + 0.6) {
    media.nextPlay = now + 0.06;
  }
  source.start(media.nextPlay);
  media.nextPlay += buffer.duration;
}

function playCallVideo(data, keyframe) {
  const media = callMedia;
  if (!media?.decoder) return;
  let bytes;
  try {
    bytes = Uint8Array.from(atob(data), (c) => c.charCodeAt(0));
  } catch {
    return;
  }
  if (!keyframe && !media.decoderKeySeen) return;
  media.decoderKeySeen = true;
  try {
    media.decoder.decode(
      new EncodedVideoChunk({
        type: keyframe ? "key" : "delta",
        timestamp: performance.now() * 1000,
        data: bytes,
      }),
    );
  } catch {
    /* malformed access unit */
  }
}

/* --------------------------------------------- pairing QR countdown timer */

// WhatsApp rotates the pairing code every ~20 s and tells us how long the one we
// are showing still has. The bar drains with it so the code never looks stuck,
// and the "rotating" dim makes the swap read as a refresh.
let qrDeadline = 0;
let qrTotalMs = 20_000;
let qrTicker = null;

function stopQrCountdown() {
  clearInterval(qrTicker);
  qrTicker = null;
  qrDeadline = 0;
  $("qr-timer").hidden = true;
  $("qr-box").classList.remove("rotating");
}

function paintQrCountdown() {
  const remaining = Math.max(0, qrDeadline - Date.now());
  const ratio = Math.max(0, Math.min(1, remaining / Math.max(1, qrTotalMs)));
  $("qr-timer-fill").style.transform = `scaleX(${ratio})`;
  if (remaining <= 0) {
    $("qr-hint").textContent = "Refreshing code…";
    $("qr-box").classList.add("rotating");
    return;
  }
  $("qr-box").classList.remove("rotating");
  $("qr-hint").textContent = `Scan this code with your phone — new code in ${Math.ceil(
    remaining / 1000,
  )}s`;
}

function startQrCountdown(timeoutSecs) {
  qrTotalMs = Math.max(1, (Number(timeoutSecs) || 20) * 1000);
  qrDeadline = Date.now() + qrTotalMs;
  $("qr-timer").hidden = false;
  paintQrCountdown();
  clearInterval(qrTicker);
  qrTicker = setInterval(paintQrCountdown, 250);
}

async function requestNewQr() {
  $("qr-refresh").hidden = true;
  $("qr-hint").textContent = "Getting a new code…";
  $("qr-box").innerHTML = '<div class="spinner"></div>';
  try {
    await invoke("restart_session");
  } catch (err) {
    showLoginError(String(err));
  }
}

function handleCallEvent(event) {
  switch (event.type) {
    case "call_incoming": {
      if (currentCall) {
        invoke("reject_call", { callId: event.call_id }).catch(() => {});
        return;
      }
      currentCall = {
        callId: event.call_id,
        peer: event.peer,
        video: Boolean(event.video),
        outgoing: false,
        state: "ringing",
      };
      showCallOverlay();
      break;
    }
    case "call_state": {
      if (!currentCall || currentCall.callId !== event.call_id) return;
      if (event.state === "connected") startCallTimer();
      else if (event.state === "calling") setCallStatus("Calling...");
      break;
    }
    case "call_ended": {
      if (!currentCall || currentCall.callId !== event.call_id) return;
      finishCall(event.reason || "Call ended");
      break;
    }
    case "call_audio":
      if (currentCall && currentCall.callId === event.call_id) playCallAudio(event.samples);
      break;
    case "call_video":
      if (currentCall && currentCall.callId === event.call_id) {
        playCallVideo(event.data, event.keyframe);
      }
      break;
    default:
      break;
  }
}

async function handleEvent(event) {
  if (String(event.type || "").startsWith("call_")) {
    handleCallEvent(event);
    return;
  }
  switch (event.type) {
    case "ready":
      state.ready = true;
      setConnection("waiting for link", "");
      break;

    case "fatal":
      setConnection("failed", "offline");
      showLoginError(event.message || "Could not start the WhatsApp session.");
      break;

    case "qr": {
      const box = $("qr-box");
      box.innerHTML = event.svg || '<div class="spinner"></div>';
      $("login").hidden = false;
      $("qr-refresh").hidden = true;
      // A previous LoggedOut notice must not sit under a fresh, valid code.
      showLoginError("");
      setConnection("waiting for link", "");
      if (event.svg) startQrCountdown(event.timeout_secs);
      else stopQrCountdown();
      break;
    }

    case "qr_expired":
      // The rotation budget is spent: the vendor stops rotating, so offer the
      // restart instead of leaving a dead code on screen.
      stopQrCountdown();
      $("qr-box").innerHTML = '<div class="spinner"></div>';
      $("qr-hint").textContent = "The pairing code expired.";
      $("qr-refresh").hidden = false;
      break;

    case "pair_code": {
      const node = $("pair-code");
      node.textContent = event.code;
      node.hidden = false;
      $("qr-hint").textContent = "Enter the code on your phone: Linked devices -> Link with phone number";
      break;
    }

    case "paired":
      maybeUnlock();
      stopQrCountdown();
      setConnection("linked", "online");
      invoke("get_status")
        .then((status) => {
          applyOwnIdentity(status);
          if (state.me) {
            $("my-avatar").title = state.meJid || state.me;
            refreshMyAvatar();
          }
          renderChatList();
          if (state.active) renderChat();
        })
        .catch(() => {});
      break;

    case "connected":
      maybeUnlock();
      stopConnectionWatch();
      state.connected = true;
      setConnection("connected", "online");
      $("login").hidden = true;
      // Photos requested before the socket was up were refused; retry them.
      state.photoFailed.clear();
      queuePhoto(state.meLid || state.meJid);
      refreshMyAvatar();
      syncContacts();
      // The LID is only known once the session is established; refresh it so
      // the account's own avatar can be fetched.
      invoke("get_status")
        .then((status) => {
          applyOwnIdentity(status);
        })
        .catch(() => {});
      // Media that failed (expired CDN path, message not cached) may succeed
      // after a reconnect or a fresh sync.
      for (const [id, entry] of state.media) {
        if (entry.status === "error") state.media.delete(id);
      }
      renderChatList();
      if (state.active) renderChat();
      if (state.active) requestChatHistory(state.chats.get(state.active));
      schedulePeerLookups(2500);
      maybeSyncChatHistory();
      syncChannelMetadata();
      break;

    case "disconnected":
      state.connected = false;
      setConnection(`reconnecting (${event.reason})`, "offline");
      watchConnection();
      break;

    case "logged_out":
      state.connected = false;
      state.linked = false;
      stopConnectionWatch();
      stopQrCountdown();
      setConnection("logged out", "offline");
      $("login").hidden = false;
      $("btn-logout").hidden = true;
      // The store is wiped and a fresh session opened behind us, so the next
      // event is a brand-new pairing code — say so instead of "session ended",
      // and drop the dead code instead of leaving it dimmed on screen.
      $("qr-box").innerHTML = '<div class="spinner"></div>';
      $("qr-hint").textContent = "Unlinked. Waiting for a new pairing code…";
      $("qr-refresh").hidden = true;
      showLoginError("");
      break;

    case "pair_error":
      showLoginError(event.message);
      break;

    case "message":
      await addIncoming(event);
      break;

    case "history_message":
      await addIncoming(event, true);
      schedulePeerLookups(6000);
      break;

    case "history_sync_progress":
      showSyncProgress(event.done, event.total);
      if (event.done >= event.total) hideSyncProgress(1500);
      break;

    case "chat":
      upsertChat({
        chat: event.jid,
        name: event.name,
        isGroup: event.is_group,
      }).lastTs = event.timestamp || 0;
      if (event.jid === state.meJid || event.jid === state.meLid) {
        state.me = displayNameFromHandle(event.name) || state.me;
        refreshMyAvatar();
      }
      scheduleMerge();
      renderChatList();
      schedulePeerLookups();
      break;

    case "channel_info": {
      const chat = state.chats.get(event.jid) || upsertChat({ chat: event.jid, name: event.name });
      if (event.name) chat.name = event.name;
      chat.subscribers = event.subscribers || 0;
      if (event.picture) state.photos.set(event.jid, event.picture);
      renderChatList();
      if (!$("channels-screen").hidden) renderChannels();
      if (state.active === event.jid) renderChat();
      break;
    }

    case "chat_alias": {
      if (foldChat(event.from, event.to)) {
        scheduleSave();
        renderChatList();
        if (state.active) renderChat();
      }
      break;
    }

    case "status":
      addStatusUpdate(event);
      break;

    case "status_viewed":
      noteStatusView(event);
      break;

    case "chat_name": {
      const chat = state.chats.get(event.jid);
      if (!chat || !event.name) break;
      invoke("ui_placeholder_name", { jid: event.jid, name: event.name })
        .catch(() => false)
        .then((placeholder) => {
          if (placeholder) return;
          chat.name = event.name;
          renderChatList();
          if (state.active === event.jid) renderChat();
        });
      break;
    }

    case "contact_name":
      applyContactName(event.jid, event.name);
      break;

    case "chat_pin": {
      // Pin state from this device or any linked client: the phone and the
      // other desktops feed the same app-state mutation here.
      const chat = state.chats.get(event.chat);
      if (!chat) break;
      chat.pinned = Boolean(event.pinned);
      if (chat.pinned && !chat.pinTs) chat.pinTs = event.timestamp || Date.now();
      if (!chat.pinned) delete chat.pinTs;
      renderChatList();
      scheduleSave();
      break;
    }

    case "chat_archive": {
      const chat = state.chats.get(event.chat);
      if (!chat) break;
      chat.archived = Boolean(event.archived);
      renderChatList();
      if (chat.archived && state.active === event.chat) {
        state.active = null;
        renderChat();
      }
      scheduleSave();
      break;
    }

    case "chat_mute": {
      const chat = state.chats.get(event.chat);
      if (!chat) break;
      if (event.until < 0) {
        chat.muteForever = true;
        delete chat.muteUntil;
      } else if (event.until > 0) {
        chat.muteUntil = event.until;
        delete chat.muteForever;
      } else {
        delete chat.muteUntil;
        delete chat.muteForever;
      }
      renderChatList();
      scheduleSave();
      // The Rust notification pump mirrors mute states; keep it in sync.
      scheduleNotifySync();
      break;
    }

    case "chat_read_mark": {
      const chat = state.chats.get(event.chat);
      if (!chat) break;
      // The debounce-free update: unread 1 for "unread" is how WhatsApp
      // renders the marked row; read clears it like a matching receipt did.
      chat.unread = event.read ? 0 : Math.max(1, chat.unread || 0);
      renderChatList();
      scheduleSave();
      break;
    }

    case "star_message": {
      // Starred/unstarred on this or any linked device.
      applyStar(event.chat, event.id, Boolean(event.starred));
      break;
    }

    case "reaction": {
      const chat = state.chats.get(event.chat);
      if (!chat) break;
      const message = chat.messages.find((m) => m.id === event.id);
      if (!message) break;
      const by = event.from_me
      ? "me"
      : await invoke("ui_phone_from_jid", { jid: event.sender || "" }).catch(() => "");
      const existing = (message.reactions || []).find((r) => r.by === by);
      // Ignore a delayed echo of an older reaction (changing reactions quickly
      // otherwise flips the pill back).
      if (event.timestamp && existing?.ts && event.timestamp < existing.ts) break;
      message.reactions = (message.reactions || []).filter((r) => r.by !== by);
      if (event.emoji) {
        message.reactions.push({
          by,
          emoji: event.emoji,
          ts: event.timestamp || Date.now(),
        });
      }
      if (state.active === event.chat) renderChat();
      scheduleSave();
      break;
    }

    case "sent":
      await addSent(event);
      break;

    case "receipt": {
      const ids = new Set(event.ids || []);
      // Receipts narrate the reader's id, which may be a LID form of the chat
      // we store by phone number. The message ids are unique per conversation,
      // so an unknown chat id falls back to finding them anywhere.
      let entry = state.chats.get(event.chat);
      if (!entry) {
        for (const candidate of state.chats.values()) {
          if ((candidate.messages || []).some((m) => ids.has(m.id))) {
            entry = candidate;
            break;
          }
        }
      }
      if (!entry) break;
      let changed = false;
      const applied = new Set();
      for (const message of entry.messages) {
        if (ids.has(message.id) && message.status !== event.status) {
          const apply = await invoke("ui_receipt_status", {
            current: message.status || "sent",
            incoming: event.status || "",
          }).catch(() => false);
          if (!apply) continue;
          message.status = event.status;
          changed = true;
        }
      }
      if (changed) {
        renderChatList();
        if (state.active === entry.jid) renderChat();
      }
      break;
    }

    case "chat_deleted": {
      const chat = state.chats.get(event.chat);
      if (chat) {
        chat.deleted = true;
        chat.deletedAt = Date.now();
      }
      if (!state.showDeleted && state.active === event.chat) {
        state.active = null;
        renderChat();
      }
      renderChatList();
      scheduleSave();
      break;
    }

    case "chat_cleared": {
      const chat = state.chats.get(event.chat);
      if (chat) {
        chat.cleared = true;
        for (const message of chat.messages) message.cleared = true;
      }
      renderChatList();
      if (state.active === event.chat) renderChat();
      scheduleSave();
      break;
    }

    case "message_deleted": {
      // Revokes arrive with the chat's remote id, which may use the peer's LID
      // while the cached chat is keyed by its phone number (or an old
      // synthetic bubble id) — fall back to scanning for the id anywhere.
      let message = state.chats.get(event.chat)?.messages?.find(
        (candidate) => candidate.id === event.id,
      );
      if (!message) {
        for (const candidate of state.chats.values()) {
          const hit = (candidate.messages || []).find((m) => m.id === event.id);
          if (hit) { message = hit; break; }
        }
      }
      if (message) {
        message.deleted = true;
        renderChatList();
        if (message.chat === state.active) renderChat();
        scheduleSave();
      }
      break;
    }

    case "message_edited": {
      let message = state.chats.get(event.chat)?.messages?.find(
        (candidate) => candidate.id === event.id,
      );
      if (!message) {
        for (const candidate of state.chats.values()) {
          const hit = (candidate.messages || []).find((m) => m.id === event.id);
          if (hit) { message = hit; break; }
        }
      }
      if (message) {
        message.text = event.text;
        message.edited = true;
        renderChatList();
        if (message.chat === state.active) renderChat();
        scheduleSave();
      }
      break;
    }

    default:
      break;
  }
}

