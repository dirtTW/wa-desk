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

  const optimistic = {
    id: tempId,
    chat,
    sender: null,
    sender_name: null,
    from_me: true,
    is_group: entry.isGroup,
    timestamp: Date.now(),
    kind,
    text: caption,
    pending: true,
  };
  if (kind === "document") {
    // Match the file card an incoming document gets: name, MIME, size.
    optimistic.document = {
      name: file.name,
      mime: file.type || "",
      size: file.size || 0,
      pages: 0,
      thumb: null,
    };
  }
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

    const media = state.media.get(tempId) || { status: "ready", url: localUrl };
    state.media.delete(tempId);
    putMedia(result.id, media);
    optimistic.id = result.id;
    optimistic.pending = false;
    renderChatList();
    renderChat();
    stopTyping();
  } catch (err) {
    optimistic.pending = false;
    optimistic.failed = true;
    state.media.delete(tempId);
    toast(String(err));
    renderChat();
  }
}

