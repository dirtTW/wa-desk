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
    // The subtitle is wa_ui::labels::group_member_count.
    $("group-info-count").textContent = await invoke("ui_group_member_count", {
      count: info.members.length,
    }).catch(() => `Group - ${info.members.length} members`);
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

async function openMemberMenu(button) {
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

  // The edge maths is wa_ui::menus::member_menu_position.
  const rect = button.getBoundingClientRect();
  const menuRect = menu.getBoundingClientRect();
  const pos = await invoke("ui_member_menu_position", {
    anchor: { x: rect.left, y: rect.top, width: rect.width, height: rect.height },
    menuW: menuRect.width || 210,
    menuH: menuRect.height || 130,
    viewportW: window.innerWidth,
    viewportH: window.innerHeight,
  }).catch(() => ({ left: rect.right - 190, top: rect.bottom + 4 }));
  menu.style.left = `${pos.left}px`;
  menu.style.top = `${pos.top}px`;
}

async function applyMemberAction(action) {
  const target = memberTarget;
  if (!target) return;
  closeMemberMenu();
  // The command, the confirmation copy and the toast are wa_ui::chat_actions.
  const plan = await invoke("ui_member_action", {
    action,
    hasTarget: Boolean(target.jid),
  }).catch(() => null);
  if (!plan || !plan.allowed) return;
  if (plan.confirm && !confirm(plan.confirm)) return;

  const chat = state.active;
  try {
    await invoke(plan.command, { chat, participants: [target.jid] });
    toast(plan.toast);
    if (plan.refresh) await openGroupInfo();
  } catch (err) {
    toast(String(err));
  }
}

