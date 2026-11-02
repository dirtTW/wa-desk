/* WA Desk frontend: contacts. Loaded in order by index.html; all files share the classic-script global scope. */

/* --------------------------------------------------------------- new group */

let groupSelection = new Set();
let groupModalMode = "create";
let groupTargetChat = null;

function groupContactEntries() {
  const lids = knownLids();
  const self = WAChatUtils.phoneFromJid(state.meJid || "");
  return [...state.contacts]
    .filter(
      ([key, name]) =>
        name &&
        /^\d+$/.test(key) &&
        key.length >= 8 &&
        key.length <= 13 &&
        !lids.has(key) &&
        key !== self,
    )
    .map(([key, name]) => ({
      jid: `${key}@s.whatsapp.net`,
      name,
      phone: WAChatUtils.formatPhone(key),
    }))
    .sort((a, b) => a.name.localeCompare(b.name));
}

function openGroupModal() {
  groupModalMode = "create";
  groupTargetChat = null;
  groupSelection = new Set();
  $("group-modal-title").textContent = "New group";
  $("group-create-label").textContent = "Create";
  $("group-subject-field").hidden = false;
  $("group-subject").value = "";
  $("group-search").value = "";
  $("group-error").hidden = true;
  $("group-modal").hidden = false;
  renderGroupContacts();
  $("group-subject").focus();
}

function openAddMembers() {
  const chat = state.chats.get(state.active);
  if (!chat || !chat.isGroup) return;
  groupModalMode = "add";
  groupTargetChat = chat.jid;
  groupSelection = new Set();
  $("group-modal-title").textContent = "Add members";
  $("group-create-label").textContent = "Add";
  $("group-subject-field").hidden = true;
  $("group-search").value = "";
  $("group-error").hidden = true;
  $("group-modal").hidden = false;
  renderGroupContacts();
  $("group-search").focus();
}

function renderGroupContacts() {
  const query = $("group-search").value.trim().toLowerCase();
  const digits = query.replace(/\D/g, "");
  const rows = groupContactEntries().filter(
    (entry) =>
      !query ||
      entry.name.toLowerCase().includes(query) ||
      (digits.length >= 3 && entry.phone.replace(/\D/g, "").includes(digits)),
  );

  $("group-contacts").innerHTML =
    rows
      .map(
        (entry) => `
      <label class="group-contact">
        <input type="checkbox" data-group-jid="${escapeHtml(entry.jid)}" ${
          groupSelection.has(entry.jid) ? "checked" : ""
        } />
        ${avatarHtml(entry.name, entry.jid)}
        <span class="group-contact-meta"><strong>${escapeHtml(entry.name)}</strong><span>${escapeHtml(entry.phone)}</span></span>
      </label>`,
      )
      .join("") || '<p class="new-chat-empty">No contacts match.</p>';

  for (const entry of rows.slice(0, 30)) queuePhoto(entry.jid);
}

async function createGroup() {
  const error = $("group-error");
  error.hidden = true;

  if (groupModalMode === "add") {
    if (!groupTargetChat || groupSelection.size === 0) {
      error.textContent = "Pick at least one participant.";
      error.hidden = false;
      return;
    }
    $("group-create").disabled = true;
    try {
      await invoke("group_add_members", {
        chat: groupTargetChat,
        participants: [...groupSelection],
      });
      $("group-modal").hidden = true;
      toast("Members added");
      if (!$("group-info-screen").hidden) await openGroupInfo();
    } catch (err) {
      error.textContent = String(err);
      error.hidden = false;
    } finally {
      $("group-create").disabled = false;
    }
    return;
  }

  const subject = $("group-subject").value.trim();
  if (!subject) {
    error.textContent = "Give the group a name.";
    error.hidden = false;
    return;
  }
  if (groupSelection.size === 0) {
    error.textContent = "Pick at least one participant.";
    error.hidden = false;
    return;
  }

  $("group-create").disabled = true;
  try {
    const jid = await invoke("create_group", {
      subject,
      participants: [...groupSelection],
    });
    state.chats.set(jid, {
      jid,
      name: subject,
      isGroup: true,
      messages: [],
      last: null,
      lastTs: Date.now(),
      unread: 0,
    });
    $("group-modal").hidden = true;
    scheduleSave();
    openChat(jid);
    toast("Group created");
  } catch (err) {
    error.textContent = String(err);
    error.hidden = false;
  } finally {
    $("group-create").disabled = false;
  }
}

