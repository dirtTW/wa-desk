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
  const chats = [...state.chats.values()].map((chat) => ({
    jid: chat.jid,
    name: chat.name || "",
    is_group: Boolean(chat.isGroup),
    message_ids: (chat.messages || []).map((message) => message.id),
    unread: chat.unread || 0,
    deleted: Boolean(chat.deleted),
    cleared: Boolean(chat.cleared),
    last_ts: chat.last?.timestamp || chat.lastTs || 0,
  }));
  // The merge decision (which copy survives, which ids move) is made by
  // wa-ui::state; this only applies the plan to the in-memory Map.
  invoke("ui_merge_chats", { chats })
    .then((plan) => {
      if (!plan.changed) return;
      for (const op of plan.ops) {
        const source = state.chats.get(op.source);
        const target = state.chats.get(op.target);
        if (!source || !target) continue;
        const ids = new Set(op.append_ids);
        for (const message of source.messages) {
          if (ids.has(message.id)) target.messages.push(message);
        }
        target.messages.sort((a, b) => a.timestamp - b.timestamp);
        target.unread += op.unread_add;
        target.deleted = op.deleted;
        target.cleared = op.cleared;
        if (op.last_ts !== null && op.last_ts > (target.last?.timestamp || 0)) {
          target.last = source.last;
          target.lastTs = op.last_ts;
        }
        state.chats.delete(op.source);
      }
      for (const [from, to] of plan.remap) {
        if (state.active === from) state.active = to;
      }
      renderChatList();
      if (state.active) renderChat();
    })
    .catch((err) => console.warn("chat merge failed", err));
}

/* ------------------------------------------------------------ peer aliases */

// Chats already looked up this session; groups are skipped.
const peerLookups = new Set();
let peerLookupTimer = null;

// Fold `from` into `to`, keeping both message sets. `name` is a verified
// business name, which wins over a numeric placeholder.
async function foldChat(from, to, name) {
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
  // Which of rename / re-key / merge happens is wa_ui::state::fold_plan.
  const plan = await invoke("ui_fold_plan", {
    input: {
      from,
      to,
      name: name || "",
      source_exists: Boolean(source),
      target_exists: Boolean(target),
      target_name: target?.name || "",
      source_ids: source ? source.messages.map((m) => m.id) : [],
      target_ids: target ? target.messages.map((m) => m.id) : [],
      source_last_ts: source?.last?.timestamp || source?.lastTs || 0,
      target_last_ts: target?.last?.timestamp || target?.lastTs || 0,
      source_unread: source?.unread || 0,
    },
  }).catch(() => null);

  if (plan?.mode === "none" || !plan) return false;
  if (plan.mode === "rename_only") {
    const only = state.chats.get(to);
    if (!only) return false;
    only.name = plan.name;
    return true;
  }
  if (plan.mode === "rekey") {
    if (!source) return false;
    source.jid = to;
    state.chats.delete(from);
    state.chats.set(to, source);
    if (plan.name) source.name = plan.name;
    if (state.active === from) state.active = to;
    return true;
  }
  if (!source || !target) return false;

  const wanted = new Set(plan.append_ids);
  for (const message of source.messages) {
    if (wanted.has(message.id)) target.messages.push(message);
  }
  target.messages.sort((a, b) => a.timestamp - b.timestamp);
  if (plan.last_ts !== null && plan.last_ts !== undefined) {
    target.last = source.last;
    target.lastTs = plan.last_ts;
  }
  target.unread += plan.unread_add;
  if (plan.name) target.name = plan.name;
  state.chats.delete(from);
  if (state.active === from) state.active = to;
  return true;
}

// WhatsApp stores a business/contact under both a LID and a phone number. The
// list can therefore show the same conversation twice, one of them empty.
// Ask the library for the real identity: it also persists the LID↔PN mapping
// so future traffic is canonicalised instead of creating another row.
async function resolvePeers(chats) {
  const pending = [];
  for (const chat of chats) {
    if (
      !chat ||
      chat.isGroup ||
      chat.jid === state.meJid ||
      chat.jid === state.meLid ||
      peerLookups.has(chat.jid)
    ) {
      continue;
    }
    // A LID chat always needs a peer lookup; a PN chat only while its name is
    // still the bare number (decided by wa_ui::text).
    const isLid = String(chat.jid).endsWith("@lid");
    if (!isLid) {
      const placeholder = await invoke("ui_placeholder_name", {
        jid: chat.jid,
        name: chat.name,
      }).catch(() => false);
      if (!placeholder) continue;
    }
    pending.push(chat);
  }
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

  // Which rows fold into which is decided by wa_ui::state::peer_fold_plan.
  const plan = await invoke("ui_alias_plan", {
    lookups: (lookups || []).map((info) => ({
      chat: info.chat,
      phone: info.phone || "",
      lid: info.lid || "",
      name: info.name || "",
    })),
    chats: chats.map((chat) => ({
      jid: chat.jid,
      name: chat.name || "",
      exists: state.chats.has(chat.jid),
    })),
  }).catch(() => ({ changed: false, folds: [], renames: [] }));

  let changed = false;
  for (const fold of plan.folds) {
    if (await foldChat(fold.from, fold.to, fold.name)) changed = true;
  }
  for (const [jid, name] of plan.renames) {
    const entry = state.chats.get(jid);
    if (entry) {
      entry.name = name;
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

let draftFiles = [];
let draftUrls = [];
// The cap itself is wa_ui::actions::MAX_DRAFT_FILES; kept for the UI checks.
const DRAFT_FILE_LIMIT = 10;

async function addDraftFiles(files) {
  if (!state.active) {
    toast("Open a chat first.");
    return;
  }

  const offered = [...files].filter(Boolean);
  if (!offered.length) return;
  // The stack cap and the image-only gate are decided by wa_ui::actions.
  const plan = await invoke("ui_draft_plan", {
    existing: draftFiles.length,
    files: offered.map((file) => ({ name: file.name || "", mime: file.type || "" })),
  }).catch(() => ({ accepted: [], full: false, toast: null }));
  if (plan.toast) toast(plan.toast);
  for (const index of plan.accepted) {
    const file = offered[index];
    if (!file) continue;
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
  $("draft-more").hidden = draftFiles.length >= DRAFT_FILE_LIMIT;
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

  // Only the first file carries the composer text (wa_ui::actions).
  const plan = await invoke("ui_draft_send_plan", {
    count: files.length,
    caption,
  }).catch(() => files.map((_, index) => ({ index, caption: index === 0 ? caption : "" })));
  for (const item of plan) {
    const file = files[item.index];
    if (!file) continue;
    await sendFile(file, item.caption);
  }
}

