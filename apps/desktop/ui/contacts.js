/* WA Desk frontend: contacts. Loaded in order by index.html; all files share the classic-script global scope. */

/* --------------------------------------------------------------- new group */

let groupSelection = new Set();
let groupModalMode = "create";
let groupTargetChat = null;

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

// The checklist markup (filtering, formatted numbers, checked state) is built
// by wa-ui::picker; this only sends the selection and injects the HTML.
async function renderGroupContacts() {
  let out;
  try {
    out = await invoke("ui_group_contacts", {
      request: {
        contacts: Object.fromEntries(state.contacts),
        known_lids: [...(await knownLids())],
        photos: Object.fromEntries(
          [...state.photos].filter(([, url]) => typeof url === "string" && url.startsWith("http")),
        ),
        me_jid: state.meJid || "",
        query: $("group-search").value,
        selected: [...groupSelection],
      },
    });
  } catch (err) {
    console.warn("group contacts render failed", err);
    return;
  }
  $("group-contacts").innerHTML = out.html;
  queuePhotos(out.queue || []);
}

async function createGroup() {
  const error = $("group-error");
  error.hidden = true;

  // The form rules are wa_ui::picker::group_form_error.
  const formError = await invoke("ui_group_form_error", {
    mode: groupModalMode,
    subject: $("group-subject").value,
    selected: groupSelection.size,
  }).catch(() => null);
  if (formError) {
    error.textContent = formError;
    error.hidden = false;
    return;
  }

  if (groupModalMode === "add") {
    if (!groupTargetChat) return;
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

