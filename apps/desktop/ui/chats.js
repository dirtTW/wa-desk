/* WA Desk frontend: chats. Loaded in order by index.html; all files share the classic-script global scope. */

/* Same conversation seen twice (PN vs LID, or a re-sync): merge by name. */

let mergeTimer = null;

function scheduleMerge() {
  if (mergeTimer) return;
  mergeTimer = setTimeout(() => {
    mergeTimer = null;
    mergeDuplicateChats();
  }, 400);
}

function mergeDuplicateChats() {
  const { changed, remap } = WAChatUtils.mergeChats(state.chats);
  if (!changed) return;

  if (state.active && remap.has(state.active)) state.active = remap.get(state.active);
  renderChatList();
  if (state.active) renderChat();
}

/* ------------------------------------------------------------ peer aliases */

// Chats already looked up this session; groups are skipped.
const peerLookups = new Set();
let peerLookupTimer = null;

// Fold `from` into `to`, keeping both message sets. `name` is a verified
// business name, which wins over a numeric placeholder.
function foldChat(from, to, name) {
  if (!from || !to) return false;
  if (from === to) {
    const only = state.chats.get(to);
    if (only && name) {
      if (only.name === name) return false;
      only.name = name;
      return true;
    }
    return false;
  }

  const source = state.chats.get(from);
  const target = state.chats.get(to);
  if (!source) {
    if (target && name && WAChatUtils.isPlaceholderName(target.jid, target.name)) {
      target.name = name;
      return true;
    }
    return false;
  }

  if (!target) {
    source.jid = to;
    state.chats.delete(from);
    state.chats.set(to, source);
    if (name) source.name = name;
    if (state.active === from) state.active = to;
    return true;
  }

  for (const message of source.messages) {
    if (!target.messages.some((candidate) => candidate.id === message.id)) {
      target.messages.push(message);
    }
  }
  target.messages.sort((a, b) => a.timestamp - b.timestamp);
  if ((source.last?.timestamp || 0) > (target.last?.timestamp || 0)) {
    target.last = source.last;
    target.lastTs = source.lastTs;
  }
  target.unread += source.unread;
  if (name) target.name = name;
  state.chats.delete(from);
  if (state.active === from) state.active = to;
  return true;
}

// WhatsApp stores a business/contact under both a LID and a phone number. The
// list can therefore show the same conversation twice, one of them empty.
// Ask the library for the real identity: it also persists the LID↔PN mapping
// so future traffic is canonicalised instead of creating another row.
async function resolvePeers(chats) {
  const pending = chats.filter(
    (chat) =>
      chat &&
      !chat.isGroup &&
      chat.jid !== state.meJid &&
      chat.jid !== state.meLid &&
      !peerLookups.has(chat.jid) &&
      (String(chat.jid).endsWith("@lid") || WAChatUtils.isPlaceholderName(chat.jid, chat.name)),
  );
  if (!pending.length) return;

  const batch = pending.slice(0, 100);
  for (const chat of batch) peerLookups.add(chat.jid);

  let lookups;
  try {
    lookups = await invoke("resolve_peers", { chats: batch.map((chat) => chat.jid) });
  } catch (err) {
    console.warn("peer lookup failed", err);
    return;
  }

  let changed = false;
  for (const info of lookups || []) {
    const phoneJid = info.phone ? `${info.phone}@s.whatsapp.net` : "";
    const lidJid = info.lid ? `${info.lid}@lid` : "";
    const target = phoneJid || info.chat;
    if (!target) continue;

    // The phone-number form is the canonical DM key, so fold the LID row into
    // it (or promote the LID row when the phone row does not exist yet).
    if (phoneJid) {
      for (const key of new Set([info.chat, lidJid])) {
        if (!key || key === target || !state.chats.has(key)) continue;
        if (foldChat(key, target, info.name)) changed = true;
      }
    }

    const entry = state.chats.get(target);
    if (entry && info.name && WAChatUtils.isPlaceholderName(entry.jid, entry.name)) {
      entry.name = info.name;
      changed = true;
    }
  }

  if (changed) {
    scheduleSave();
    renderChatList();
    if (state.active) renderChat();
  }
}

// History sync announces chats in bursts; resolve them once it settles.
function schedulePeerLookups(delay = 4000) {
  if (peerLookupTimer) clearTimeout(peerLookupTimer);
  peerLookupTimer = setTimeout(() => {
    peerLookupTimer = null;
    resolvePeers([...state.chats.values()]);
  }, delay);
}

/* ------------------------------------------------------- clipboard drafts */

const MAX_DRAFT_FILES = 10;
let draftFiles = [];
let draftUrls = [];

function addDraftFiles(files) {
  if (!state.active) {
    toast("Open a chat first.");
    return;
  }

  for (const file of files) {
    if (!file) continue;
    if (draftFiles.length >= MAX_DRAFT_FILES) {
      toast(`You can attach up to ${MAX_DRAFT_FILES} images at once.`);
      break;
    }
    if (!/^image\//.test(file.type)) {
      toast("Only images can be added to a draft.");
      continue;
    }
    draftFiles.push(file);
    draftUrls.push(URL.createObjectURL(file));
  }

  renderDraft();
  if (draftFiles.length > 0) toggleEmoji(false);
}

function renderDraft() {
  const thumbs = $("draft-thumbs");
  thumbs.innerHTML = draftFiles
    .map(
      (file, index) =>
        `<div class="draft-item"><img src="${draftUrls[index]}" alt="Attachment ${index + 1}" /><button type="button" class="draft-item-remove" data-index="${index}" title="Remove">&#215;</button></div>`,
    )
    .join("");

  const open = draftFiles.length > 0;
  $("draft").hidden = !open;
  // The official desktop keeps a single send button: the disc beside the
  // draft replaces the composer's text send while a media draft is open.
  $("send").hidden = open;
  $("draft-more").hidden = draftFiles.length >= MAX_DRAFT_FILES;
  // Keep the text box available: its content becomes the caption.
  $("composer").hidden = false;
}

function clearDraft() {
  for (const url of draftUrls) URL.revokeObjectURL(url);
  draftFiles = [];
  draftUrls = [];
  $("draft-thumbs").innerHTML = "";
  $("draft").hidden = true;
  $("send").hidden = false;
  $("composer").hidden = false;
}

function removeDraftAt(index) {
  const [url] = draftUrls.splice(index, 1);
  if (url) URL.revokeObjectURL(url);
  draftFiles.splice(index, 1);
  if (draftFiles.length === 0) clearDraft();
  else renderDraft();
}

async function sendDraft() {
  const files = draftFiles.slice();
  if (files.length === 0) return;

  // The composer text is the caption, like WhatsApp's media preview.
  const caption = $("input").value.trim();
  $("input").value = "";
  autosize($("input"));
  updateSendState();
  clearDraft();

  for (let index = 0; index < files.length; index += 1) {
    await sendFile(files[index], index === 0 ? caption : "");
  }
}

