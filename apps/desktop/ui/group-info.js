/* WA Desk frontend: group-info. Loaded in order by index.html; all files share the classic-script global scope. */

/* -------------------------------------------------------------- group info */

async function openGroupInfo() {
  const chat = state.chats.get(state.active);
  if (!chat || !chat.isGroup) return;

  $("group-info-screen").hidden = false;
  $("settings-screen").hidden = true;
  $("status-screen").hidden = true;
  $("new-chat-screen").hidden = true;
  hideStatusShare();
  $("group-info-name").textContent = chat.name;
  $("group-info-count").textContent = "";
  $("group-info-desc").textContent = "";
  $("group-members").innerHTML = '<p class="new-chat-empty">Loading members...</p>';
  setAvatar($("group-info-avatar"), chat.name, chat.jid);
  queuePhoto(chat.jid);

  try {
    const info = await invoke("group_info", { chat: chat.jid });
    $("group-info-name").textContent = info.subject || chat.name;
    $("group-info-count").textContent = `Group - ${info.members.length} members`;
    $("group-info-desc").textContent = info.description || "";
    const rendered = await invoke("ui_group_members", {
      members: await Promise.all(
        info.members.map(async (member) => ({
          jid: member.jid,
          phone:
            member.phone ||
            (await invoke("ui_phone_from_jid", { jid: member.jid }).catch(() => "")),
          admin: Boolean(member.admin),
        })),
      ),
      contacts: Object.fromEntries(state.contacts),
      photos: Object.fromEntries(
        [...state.photos].filter(([, url]) => typeof url === "string" && url.startsWith("http")),
      ),
    });
    $("group-members").innerHTML = rendered.html;
    for (const jid of rendered.queue || []) queuePhoto(jid);
  } catch (err) {
    $("group-members").innerHTML = `<p class="new-chat-empty">${escapeHtml(String(err))}</p>`;
  }
}

async function leaveCurrentGroup() {
  const chat = state.chats.get(state.active);
  if (!chat || !chat.isGroup) return;
  if (!confirm(`Leave "${chat.name}"?`)) return;
  try {
    await invoke("leave_group", { chat: chat.jid });
    state.chats.delete(chat.jid);
    $("group-info-screen").hidden = true;
    state.active = null;
    renderChatList();
    renderChat();
    scheduleSave();
    toast("Left the group");
  } catch (err) {
    toast(String(err));
  }
}

/* ------------------------------------------------- group member actions */

let memberTarget = null;

function closeMemberMenu() {
  $("member-menu").hidden = true;
  memberTarget = null;
}

function openMemberMenu(button) {
  const row = button.closest(".group-member");
  if (!row) return;
  memberTarget = {
    jid: row.dataset.memberJid,
    admin: row.dataset.memberAdmin === "true",
  };

  const menu = $("member-menu");
  menu.querySelector('[data-member-action="promote"]').hidden = memberTarget.admin;
  menu.querySelector('[data-member-action="demote"]').hidden = !memberTarget.admin;
  menu.hidden = false;

  const rect = button.getBoundingClientRect();
  menu.style.left = `${Math.max(8, Math.min(window.innerWidth - 210, rect.right - 190))}px`;
  menu.style.top = `${Math.min(window.innerHeight - 130, rect.bottom + 4)}px`;
}

async function applyMemberAction(action) {
  const target = memberTarget;
  if (!target) return;
  closeMemberMenu();
  if (action === "remove" && !confirm("Remove this member from the group?")) return;

  const chat = state.active;
  try {
    if (action === "promote") {
      await invoke("group_promote_members", { chat, participants: [target.jid] });
      toast("Promoted to group admin");
    } else if (action === "demote") {
      await invoke("group_demote_members", { chat, participants: [target.jid] });
      toast("Admin rights removed");
    } else {
      await invoke("group_remove_members", { chat, participants: [target.jid] });
      toast("Member removed");
    }
    await openGroupInfo();
  } catch (err) {
    toast(String(err));
  }
}

