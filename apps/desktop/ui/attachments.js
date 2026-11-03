/* WA Desk frontend: attachments. Loaded in order by index.html; all files share the classic-script global scope. */

/* ---------------------------------------------------------------- attachments */

function fileToBase64(file) {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      const result = String(reader.result || "");
      const comma = result.indexOf(",");
      resolve(comma >= 0 ? result.slice(comma + 1) : "");
    };
    reader.onerror = () => reject(reader.error || new Error("cannot read file"));
    reader.readAsDataURL(file);
  });
}

async function sendFile(file, caption = "") {
  const chat = state.active;
  if (!file || !chat) return;
  // Size limit and media kind are decided by wa-ui::actions.
  const check = await invoke("ui_attachment_check", {
    size: file.size || 0,
    mime: file.type || "",
  }).catch(() => null);
  if (!check || !check.ok) {
    if (check?.error) toast(check.error);
    return;
  }
  const kind = check.kind;

  const entry = state.chats.get(chat);
  if (!entry) return;

  const localUrl = URL.createObjectURL(file);
  const tempId = `local-${++state.localSeq}`;
  rememberMedia(tempId, localUrl);

  // The optimistic bubble (including a document's file card) is shaped by
  // wa_ui::actions::attachment_message.
  const shaped = await invoke("ui_attachment_message", {
    input: {
      temp_id: tempId,
      kind,
      caption: caption || "",
      file_name: file.name || "",
      mime: file.type || "",
      size: file.size || 0,
      is_group: Boolean(entry.isGroup),
      timestamp: Date.now(),
    },
  }).catch(() => null);
  if (!shaped) return;
  const optimistic = { ...shaped, chat };
  entry.messages.push(optimistic);
  entry.last = optimistic;
  entry.lastTs = optimistic.timestamp;
  renderChatList();
  renderChat();

  try {
    const data = await fileToBase64(file);
    const result = await invoke("send_attachment", {
      chat,
      name: file.name,
      mime: file.type || "application/octet-stream",
      caption: caption || null,
      data,
    });

    // The bubble keeps its place and only learns the server id
    // (wa_ui::actions::attachment_result).
    const done = await invoke("ui_attachment_result", {
      tempId,
      serverId: result.id,
      ok: true,
    }).catch(() => ({ id: result.id, pending: false, failed: false, temp_id: tempId }));
    const media = state.media.get(tempId) || { status: "ready", url: localUrl };
    state.media.delete(tempId);
    putMedia(done.id, media);
    optimistic.id = done.id;
    optimistic.pending = done.pending;
    renderChatList();
    renderChat();
    stopTyping();
  } catch (err) {
    const done = await invoke("ui_attachment_result", {
      tempId,
      serverId: "",
      ok: false,
    }).catch(() => ({ id: tempId, pending: false, failed: true, temp_id: tempId }));
    optimistic.pending = done.pending;
    optimistic.failed = done.failed;
    state.media.delete(tempId);
    toast(String(err));
    renderChat();
  }
}

