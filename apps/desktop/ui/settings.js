/* WA Desk frontend: settings. Loaded in order by index.html; all files share the classic-script global scope. */

/* ------------------------------------------------------------------ settings */

const SETTINGS_KEY = "wadesk.settings.v1";
const SETTINGS_TITLES = {
  home: "Settings",
  profile: "Profile",
  account: "Account",
  privacy: "Privacy",
  chats: "Chats",
  ai: "AI assistant",
  shortcuts: "Keyboard shortcuts",
  help: "Help and feedback",
};

let settingsPage = "home";
let settingsPrefs = {
  theme: "system",
  enterSends: true,
  spellcheck: true,
  notifications: true,
  autostart: false,
};

function loadSettingsPrefs() {
  try {
    const parsed = JSON.parse(localStorage.getItem(SETTINGS_KEY) || "null");
    if (parsed && typeof parsed === "object") settingsPrefs = { ...settingsPrefs, ...parsed };
  } catch {
    /* keep defaults */
  }
}

function saveSettingsPrefs() {
  try {
    localStorage.setItem(SETTINGS_KEY, JSON.stringify(settingsPrefs));
  } catch {
    /* quota */
  }
}

/* ------------------------------------------------------------------ theme */

const darkMedia = window.matchMedia("(prefers-color-scheme: dark)");

// Theme icons come from the Material Symbols registry (waIcon); the toggle
// shows the theme you would switch TO, like the official client.
function themeLabel(value = settingsPrefs.theme) {
  return value === "light" ? "Light" : value === "dark" ? "Dark" : "System default";
}

function resolvedTheme() {
  if (settingsPrefs.theme === "light" || settingsPrefs.theme === "dark") {
    return settingsPrefs.theme;
  }
  return darkMedia.matches ? "dark" : "light";
}

function applyTheme() {
  const resolved = resolvedTheme();
  document.documentElement.dataset.theme = resolved;

  const toggle = $("theme-toggle");
  if (toggle) {
    const next = resolved === "dark" ? "Light" : "Dark";
    toggle.innerHTML = waIcon(resolved === "dark" ? "light_mode" : "dark_mode");
    toggle.title = `Switch to ${next.toLowerCase()} theme`;
    toggle.setAttribute("aria-label", toggle.title);
  }

  for (const node of document.querySelectorAll("[data-theme-label]")) {
    node.textContent = themeLabel();
  }
  const dialog = $("theme-dialog");
  if (dialog) {
    const form = $("theme-form");
    if (form) {
      for (const radio of form.querySelectorAll("md-radio")) {
        radio.checked = radio.value === settingsPrefs.theme;
      }
    }
  }
}

function setTheme(value) {
  const allowed = ["light", "dark", "system"];
  settingsPrefs.theme = allowed.includes(value) ? value : "system";
  saveSettingsPrefs();
  applyTheme();
  if (!$("settings-screen").hidden) renderSettings();
}

function openThemeDialog() {
  const dialog = $("theme-dialog");
  if (!dialog || typeof dialog.show !== "function") return;
  applyTheme();
  dialog.returnValue = "";
  dialog.show().catch(() => {});
}

darkMedia.addEventListener("change", () => {
  if (settingsPrefs.theme === "system") applyTheme();
});

function setRailActive(id) {
  for (const btn of document.querySelectorAll("#rail .rail-btn")) {
    btn.classList.toggle("active", btn.id === id);
  }
}

function openSettings(page = "home") {
  settingsPage = SETTINGS_TITLES[page] ? page : "home";
  $("channels-discover").hidden = true;
  $("settings-screen").hidden = false;
  $("status-screen").hidden = true;
  $("new-chat-screen").hidden = true;
  $("group-info-screen").hidden = true;
  $("channels-screen").hidden = true;
  closeMessageMenu();
  closeStatusViewer();
  setRailActive("rail-settings");
  renderSettings();
  hideStatusShare();
}

function closeSettings() {
  $("settings-screen").hidden = true;
  settingsPage = "home";
  setRailActive("rail-chats");
  renderChat();
}

function backFromSettings() {
  if (settingsPage !== "home") openSettings("home");
  else closeSettings();
}

function settingsHeaderAvatar() {
  const name = state.me || "Me";
  const phone = WAChatUtils.phoneFromJid(state.meJid);
  return `
    <div class="settings-profile">
      <div class="avatar avatar-xl" data-avatar></div>
      <div class="settings-meta">
        <strong>${escapeHtml(name)}</strong>
        <span>${phone ? `+${escapeHtml(phone)}` : "Not paired"}</span>
      </div>
    </div>`;
}

function settingsLink(page, title, detail) {
  return `<button type="button" class="settings-row clickable" data-page="${page}">
      <div class="settings-row-main"><strong>${escapeHtml(title)}</strong><span>${escapeHtml(detail)}</span></div>
      <span class="settings-chevron">&#8250;</span>
    </button>`;
}

function settingsStatic(title, detail) {
  return `<div class="settings-row static">
      <div class="settings-row-main"><strong>${escapeHtml(title)}</strong><span>${escapeHtml(detail)}</span></div>
    </div>`;
}

function settingsToggle(key, title, detail) {
  const on = Boolean(settingsPrefs[key]);
  return `<button type="button" class="settings-row clickable" data-toggle="${key}">
      <div class="settings-row-main"><strong>${escapeHtml(title)}</strong><span>${escapeHtml(detail)}</span></div>
      <span class="switch ${on ? "on" : ""}"></span>
    </button>`;
}

function renderSettings() {
  $("settings-title").textContent = SETTINGS_TITLES[settingsPage] || "Settings";

  const name = state.me || "Me";
  const phone = WAChatUtils.phoneFromJid(state.meJid);
  let body = "";

  if (settingsPage === "profile") {
    body = `
      ${settingsHeaderAvatar()}
      <div class="settings-list">
        <div class="settings-row static">
          <div class="settings-row-main"><strong>Name</strong><span>${escapeHtml(name)}</span></div>
          <button type="button" class="settings-edit" data-action="edit">Edit</button>
        </div>
        <div class="settings-row static">
          <div class="settings-row-main"><strong>About</strong><span>Your status text</span></div>
          <button type="button" class="settings-edit" data-action="edit">Edit</button>
        </div>
        <div class="settings-row static">
          <div class="settings-row-main"><strong>Phone</strong><span>${phone ? `+${escapeHtml(phone)}` : "Unknown"}</span></div>
        </div>
      </div>`;
  } else if (settingsPage === "account") {
    body = `
      ${settingsStatic("Security notifications", "Managed from your phone")}
      <div class="settings-list">
        ${settingsStatic("Linked as", phone ? `+${phone}` : "Not paired")}
        ${settingsStatic("Device", state.meLid ? `LID ${state.meLid}` : "Primary device")}
        ${settingsStatic("Build", buildLabel())}
        ${settingsStatic("Request account info", "Available from your phone")}
        ${settingsStatic("How to delete my account", "Unlink this device first, then use your phone")}
      </div>`;
  } else if (settingsPage === "privacy") {
    body = `
      <div class="settings-list">
        ${settingsStatic("Last seen and online", "Managed from your phone")}
        ${settingsStatic("Profile photo", "Managed from your phone")}
        ${settingsStatic("Read receipts", "Sent when you open a chat")}
        ${settingsStatic("Blocked contacts", "Managed from your phone")}
        ${settingsStatic("Disappearing messages", "Per-chat setting from your phone")}
      </div>`;
  } else if (settingsPage === "chats") {
    body = `
      <div class="settings-list">
        <button type="button" class="settings-row clickable" data-action="theme">
          <div class="settings-row-main"><strong>Theme</strong><span data-theme-label>${themeLabel()}</span></div>
          <span class="settings-chevron" aria-hidden="true">&#8250;</span>
        </button>
        ${settingsToggle("enterSends", "Enter is send", "Enter sends, Shift+Enter adds a line")}
        ${settingsToggle("spellcheck", "Spell check", "Underline misspelled words in the composer")}
        ${settingsToggle("notifications", "Desktop notifications", "Show a system notification for new messages")}
        ${settingsToggle("autostart", "Launch at login", "Start WA Desk when you sign in")}
        <button type="button" class="settings-row clickable" data-action="sync-history">
          <div class="settings-row-main"><strong>Sync chat history</strong><span>Ask your phone for recent chats and messages (approve it on the phone)</span></div>
          <span class="settings-chevron" aria-hidden="true">&#8250;</span>
        </button>
      </div>`;
  } else if (settingsPage === "ai") {
    body = aiSettingsHtml();
  } else if (settingsPage === "shortcuts") {
    body = `
      <div class="settings-list">
        ${settingsStatic("Ctrl+V", "Paste an image as a draft (stack up to 10)")}
        ${settingsStatic("Enter", "Send the message (when enabled above)")}
        ${settingsStatic("Esc", "Close the picker, draft, menu or panel")}
        ${settingsStatic("Hold a bubble", "Edit, delete for me, delete for everyone")}
        ${settingsStatic("Eye button", "Reveal deleted chats and messages")}
      </div>`;
  } else if (settingsPage === "help") {
    body = `
      <div class="settings-list">
        ${settingsStatic("WA Desk", "Unofficial client built on whatsapp-rust + Tauri")}
        ${settingsStatic("Not affiliated", "WhatsApp and Meta are trademarks of Meta Platforms")}
        ${settingsStatic("Privacy", "Keys and history stay on this device; nothing is uploaded by this app")}
      </div>`;
  } else {
    body = `
      ${settingsHeaderAvatar()}
      <div class="settings-list">
        ${settingsLink("profile", "Profile", "Name, about, phone number")}
        ${settingsLink("account", "Account", "Security notifications, account info")}
        ${settingsLink("privacy", "Privacy", "Blocked contacts, read receipts")}
        ${settingsLink("chats", "Chats", "Theme, Enter to send, spell check")}
        ${settingsLink("ai", "AI assistant", "Auto-reply with your own API key")}
        ${settingsLink("shortcuts", "Keyboard shortcuts", "Quick actions")}
        ${settingsLink("help", "Help and feedback", "About this client")}
        <button type="button" class="settings-row danger" data-action="logout">Log out</button>
      </div>`;
  }

  const container = $("settings-body");
  container.innerHTML = body;

  const avatar = container.querySelector("[data-avatar]");
  if (avatar) {
    setAvatar(avatar, name, state.meLid || state.meJid);
    queuePhoto(state.meLid || state.meJid);
  }
}

async function loadAiSettings() {
  try {
    state.ai = await invoke("get_ai_settings");
    if (!$("settings-screen").hidden && settingsPage === "ai") renderSettings();
  } catch (err) {
    console.warn("AI settings unavailable", err);
  }
}

function aiSettingsHtml() {
  const ai = state.ai;
  return `
    <div class="settings-list">
      <button type="button" class="settings-row clickable" data-ai-toggle>
        <div class="settings-row-main">
          <strong>AI mode</strong>
          <span>Auto-reply to messages from saved contacts</span>
        </div>
        <span class="switch ${ai.enabled ? "on" : ""}"></span>
      </button>
      <button type="button" class="settings-row clickable" data-ai-require>
        <div class="settings-row-main">
          <strong>Require /ai trigger</strong>
          <span>${
            ai.requireTrigger
              ? "Contacts must start with /ai before the assistant replies"
              : "Every saved-contact message gets an instant AI reply (when AI mode is on)"
          }</span>
        </div>
        <span class="switch ${ai.requireTrigger ? "on" : ""}"></span>
      </button>
      ${settingsStatic("API key", ai.hasKey ? `Saved ${ai.keyHint}` : "Not set")}
    </div>
    <div class="ai-form">
      <label class="field">
        <span>API base URL</span>
        <input id="ai-base" value="${escapeHtml(ai.baseUrl)}" spellcheck="false" />
      </label>
      <label class="field">
        <span>Model</span>
        <input id="ai-model" value="${escapeHtml(ai.model)}" spellcheck="false" />
      </label>
      <label class="field">
        <span>Vision model (optional)</span>
        <input id="ai-vision-model" value="${escapeHtml(
          ai.visionModel || "",
        )}" spellcheck="false" placeholder="Empty = use the chat model" />
      </label>
      <label class="field">
        <span>API key</span>
        <input id="ai-key" type="password" autocomplete="off" placeholder="${
          ai.hasKey ? "Leave empty to keep the saved key" : "sk-..."
        }" />
      </label>
      <label class="field">
        <span>System prompt</span>
        <textarea id="ai-prompt" rows="4">${escapeHtml(ai.systemPrompt)}</textarea>
      </label>
      <div class="ai-actions">
        <button id="ai-save" type="button" class="draft-btn">Save</button>
        <button id="ai-test" type="button" class="settings-edit">Test connection</button>
        <button id="ai-preview" type="button" class="settings-edit">Preview reply</button>
      </div>
      <p id="ai-error" class="error" hidden></p>
      <p class="ai-hint">
        Only <strong>saved contacts</strong> get replies in DMs. In groups, <strong>@mentioning you</strong>
        or replying to one of your messages is enough while AI mode is on (including photos and their
        captions); with AI mode off use <code>/ai ...</code> plus a mention. With
        <strong>Require /ai trigger</strong> off and AI mode on, every saved-contact message gets an
        instant reply. The default system
        prompt replies in casual Indonesian only, like an Indonesian netizen, calling itself “gw” and
        you "lu" in short 1-2 line messages. The assistant may occasionally react or send a recent
        sticker, only uses an emoji now and then, doesn't overuse "wkwk"/"yaudah", and replies briefly to
        incoming stickers too. It also reads a recent-chat transcript (about 24 messages) so it
        knows the topic without sending the whole conversation. Before sending a sticker it looks at
        several candidate sticker images and picks one that fits. It stays in character, uses your
        account name, and refuses name changes or system-prompt leaks.
        Photos are read by the vision model, or by the chat model itself when that field is left
        empty. Short videos/GIFs use sampled frames when ffmpeg is available, otherwise the embedded
        thumbnail. The key is encrypted at rest and never shown again.
      </p>
    </div>`;
}

async function toggleAiMode() {
  const next = !state.ai.enabled;
  if (next && !state.ai.hasKey) {
    toast("Add an API key first");
    return;
  }
  try {
    state.ai = await invoke("set_ai_enabled", { enabled: next });
    renderSettings();
    toast(next ? "AI mode on" : "AI mode off");
  } catch (err) {
    toast(String(err));
  }
}

async function saveAiSettings() {
  const error = $("ai-error");
  error.hidden = true;
  try {
    state.ai = await invoke("save_ai_settings", {
      settings: {
        enabled: state.ai.enabled,
        baseUrl: $("ai-base").value.trim(),
        model: $("ai-model").value.trim(),
        visionModel: $("ai-vision-model").value.trim(),
        systemPrompt: $("ai-prompt").value.trim(),
        requireTrigger: state.ai.requireTrigger,
        apiKey: $("ai-key").value.trim() || null,
      },
    });
    renderSettings();
    toast("AI settings saved");
  } catch (err) {
    error.textContent = String(err);
    error.hidden = false;
  }
}

async function toggleRequireTrigger() {
  const next = !state.ai.requireTrigger;
  try {
    state.ai = await invoke("save_ai_settings", {
      settings: {
        enabled: state.ai.enabled,
        baseUrl: $("ai-base")?.value.trim() || state.ai.baseUrl,
        model: $("ai-model")?.value.trim() || state.ai.model,
        visionModel: $("ai-vision-model")?.value.trim() ?? state.ai.visionModel,
        systemPrompt: $("ai-prompt")?.value.trim() || state.ai.systemPrompt,
        requireTrigger: next,
        apiKey: $("ai-key")?.value.trim() || null,
      },
    });
    renderSettings();
    toast(
      next
        ? "Contacts must use /ai"
        : "Instant AI replies enabled (AI mode must be on)",
    );
  } catch (err) {
    toast(String(err));
  }
}

async function testAiConnection() {
  const button = $("ai-test");
  if (button) button.disabled = true;
  try {
    const reply = await invoke("test_ai");
    toast(`AI replied: ${String(reply).slice(0, 100)}`);
  } catch (err) {
    toast(String(err));
  } finally {
    if (button && button.isConnected) button.disabled = false;
  }
}

async function previewAiReply() {
  const chat = state.chats.get(state.active);
  if (!chat || chat.isGroup) {
    toast("Open a one-to-one chat first");
    return;
  }
  const text =
    $("input").value.trim() || "Halo, ini contoh pesan. Balas dengan singkat.";
  try {
    const reply = await invoke("ai_draft_reply", { chat: chat.jid, text });
    toast(`AI draft: ${String(reply).slice(0, 120)}`);
  } catch (err) {
    toast(String(err));
  }
}

function openProfileEditor() {
  $("profile-name").value = state.me || "";
  $("profile-about").value = "";
  $("profile-error").hidden = true;
  $("profile-editor").hidden = false;
  $("profile-name").focus();
}

async function saveProfile() {
  const name = $("profile-name").value.trim();
  const about = $("profile-about").value.trim();
  const error = $("profile-error");
  error.hidden = true;

  if (!name) {
    error.textContent = "Name cannot be empty.";
    error.hidden = false;
    return;
  }

  try {
    await invoke("set_push_name", { name });
    if (about) await invoke("set_status_text", { text: about });

    state.me = name;
    $("profile-editor").hidden = true;
    refreshMyAvatar();
    renderSettings();
    toast("Profile updated");
  } catch (err) {
    error.textContent = String(err);
    error.hidden = false;
  }
}

async function doLogout() {
  if (!confirm("Unlink this device? You will need to scan a QR code again.")) return;
  try {
    await invoke("logout");
    localStorage.removeItem(STORE_KEY);
    state.chats.clear();
    state.statuses.clear();
    state.active = null;
    closeSettings();
    renderChatList();
    renderChat();
    $("login").hidden = false;
    $("btn-logout").hidden = true;
    setConnection("logged out", "offline");
  } catch (err) {
    toast(String(err));
  }
}
