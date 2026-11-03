/* WA Desk frontend: settings. Loaded in order by index.html; all files share the classic-script global scope. */

/* ------------------------------------------------------------------ settings */

const SETTINGS_KEY = "wadesk.settings.v1";

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

// Theme icons come from the Material Symbols registry (ui_icon); the toggle
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
    invoke("ui_icon", { name: resolved === "dark" ? "light_mode" : "dark_mode" })
      .then((html) => {
        toggle.innerHTML = html;
      })
      .catch(() => {});
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
  settingsPage = page;
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

// Everyone whose work this client stands on. `Help and feedback` lists these
// with links; keep it in step with the Credits section of the README.
const CREDITS = [
  ["whatsapp-rust", "jlucaso1 — the WhatsApp Web protocol client", "https://github.com/jlucaso1/whatsapp-rust"],
  ["libsignal", "Signal — the end-to-end encryption primitives", "https://github.com/signalapp/libsignal"],
  ["whatsmeow / Baileys", "protocol reference implementations", "https://github.com/tulir/whatsmeow"],
  ["Tauri", "the desktop app framework", "https://tauri.app/"],
  ["tokio", "the async runtime", "https://tokio.rs/"],
  ["serde", "serialization", "https://serde.rs/"],
  ["RustCrypto", "aes-gcm, sha2, hmac, hkdf", "https://github.com/RustCrypto"],
  ["ureq", "the blocking HTTP client", "https://github.com/algesten/ureq"],
  ["qrcode", "the pairing QR code", "https://github.com/kennytm/qrcode-rust"],
  ["tracing", "logging", "https://tokio.rs/tokio/tracing"],
  ["Material Symbols", "every icon in this UI", "https://fonts.google.com/icons"],
  ["Material Web", "the settings dialog", "https://github.com/material-components/material-web"],
  ["OpenCode", "the agentic coding environment", "https://opencode.ai"],
  ["DeepSeek & GLM", "the models that wrote the code", "https://deepseek.com"],
];

/* ---------------------------------------------------------------- updates */

// The Account screen's update row: a manual check, then a one-click install.
// `check_for_update`/`install_update` go through tauri-plugin-updater, which
// reads the `latest.json` on the newest GitHub release and verifies the
// installer against the minisign key in `plugins.updater.pubkey`.
let updateState = { checking: false, version: null, status: "" };

async function checkForUpdate() {
  updateState = { checking: true, version: null, status: "Checking for updates..." };
  renderSettings();
  try {
    const result = await invoke("check_for_update");
    updateState = result.version
      ? {
          checking: false,
          version: result.version,
          status: `Version ${result.version} is available — you are on ${result.current}.`,
        }
      : {
          checking: false,
          version: null,
          status: `You are on the newest version (${result.current}).`,
        };
  } catch (err) {
    updateState = {
      checking: false,
      version: null,
      status: `Could not check for updates: ${String(err)}`,
    };
  }
  renderSettings();
}

async function installUpdate() {
  updateState = {
    checking: false,
    version: null,
    status: "Downloading and installing... WA Desk restarts when it is done.",
  };
  renderSettings();
  try {
    await invoke("install_update");
  } catch (err) {
    updateState = {
      checking: false,
      version: null,
      status: `Could not install the update: ${String(err)}`,
    };
    renderSettings();
  }
}

/* ----------------------------------------------- Rust-rendered settings */

// Every row (home, profile, account, privacy, chats, AI, shortcuts, help) is
// built by wa-ui::settings; this only supplies the state and injects the HTML.
function settingsRequest() {
  return {
    page: settingsPage,
    me: state.me || "",
    me_jid: state.meJid || "",
    me_lid: state.meLid || "",
    theme_label: themeLabel(),
    build: buildLabel(),
    prefs: {
      enter_sends: Boolean(settingsPrefs.enterSends),
      spellcheck: Boolean(settingsPrefs.spellcheck),
      notifications: Boolean(settingsPrefs.notifications),
      autostart: Boolean(settingsPrefs.autostart),
    },
    update_state: {
      checking: Boolean(updateState.checking),
      version: updateState.version || null,
      status: updateState.status || "",
    },
    ai: {
      enabled: Boolean(state.ai?.enabled),
      require_trigger: Boolean(state.ai?.requireTrigger),
      base_url: state.ai?.baseUrl || "",
      model: state.ai?.model || "",
      vision_model: state.ai?.visionModel || "",
      system_prompt: state.ai?.systemPrompt || "",
      has_key: Boolean(state.ai?.hasKey),
      key_hint: state.ai?.keyHint || "",
    },
    with_avatar: true,
  };
}

let settingsRenderSeq = 0;

async function renderSettings() {
  const seq = ++settingsRenderSeq;
  let out;
  try {
    out = await invoke("ui_settings", { request: settingsRequest() });
  } catch (err) {
    console.warn("settings render failed", err);
    return;
  }
  if (seq !== settingsRenderSeq) return;
  settingsPage = out.page;
  $("settings-title").textContent = out.title;

  const container = $("settings-body");
  container.innerHTML = out.html;
  const avatar = container.querySelector("[data-avatar]");
  if (avatar) {
    setAvatar(avatar, state.me || "Me", state.meLid || state.meJid);
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
