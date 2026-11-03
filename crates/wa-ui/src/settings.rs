//! The Settings screen: the page title and body for every page, including the
//! update row, the AI assistant form and the credits list.

use serde::{Deserialize, Serialize};

use crate::html::escape;
use crate::text::phone_from_jid;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PrefsView {
    #[serde(default)]
    pub enter_sends: bool,
    #[serde(default)]
    pub spellcheck: bool,
    #[serde(default)]
    pub notifications: bool,
    #[serde(default)]
    pub autostart: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct UpdateView {
    #[serde(default)]
    pub checking: bool,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub status: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct AiView {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub require_trigger: bool,
    #[serde(default)]
    pub base_url: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub vision_model: String,
    #[serde(default)]
    pub system_prompt: String,
    #[serde(default)]
    pub has_key: bool,
    #[serde(default)]
    pub key_hint: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SettingsRequest {
    #[serde(default)]
    pub page: String,
    #[serde(default)]
    pub me: String,
    #[serde(default)]
    pub me_jid: String,
    #[serde(default)]
    pub me_lid: String,
    #[serde(default)]
    pub theme_label: String,
    #[serde(default)]
    pub build: String,
    #[serde(default)]
    pub prefs: PrefsView,
    #[serde(default)]
    pub update_state: UpdateView,
    #[serde(default)]
    pub ai: AiView,
    /// Marks the avatar element so the webview can paint the cached photo.
    #[serde(default = "default_true")]
    pub with_avatar: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Serialize)]
pub struct SettingsOutput {
    pub title: String,
    /// The canonical page after falling back to `home`.
    pub page: String,
    pub html: String,
}

pub const SOURCE_URL: &str = "https://github.com/dirtTW/wa-desk";
pub const LICENSE_URL: &str = "https://github.com/dirtTW/wa-desk/blob/main/LICENSE.md";

/// Everyone whose work this client stands on.
const CREDITS: [(&str, &str, &str); 14] = [
    (
        "whatsapp-rust",
        "jlucaso1 — the WhatsApp Web protocol client",
        "https://github.com/jlucaso1/whatsapp-rust",
    ),
    (
        "libsignal",
        "Signal — the end-to-end encryption primitives",
        "https://github.com/signalapp/libsignal",
    ),
    (
        "whatsmeow / Baileys",
        "protocol reference implementations",
        "https://github.com/tulir/whatsmeow",
    ),
    ("Tauri", "the desktop app framework", "https://tauri.app/"),
    ("tokio", "the async runtime", "https://tokio.rs/"),
    ("serde", "serialization", "https://serde.rs/"),
    (
        "RustCrypto",
        "aes-gcm, sha2, hmac, hkdf",
        "https://github.com/RustCrypto",
    ),
    (
        "ureq",
        "the blocking HTTP client",
        "https://github.com/algesten/ureq",
    ),
    (
        "qrcode",
        "the pairing QR code",
        "https://github.com/kennytm/qrcode-rust",
    ),
    ("tracing", "logging", "https://tokio.rs/tokio/tracing"),
    (
        "Material Symbols",
        "every icon in this UI",
        "https://fonts.google.com/icons",
    ),
    (
        "Material Web",
        "the settings dialog",
        "https://github.com/material-components/material-web",
    ),
    (
        "OpenCode",
        "the agentic coding environment",
        "https://opencode.ai",
    ),
    (
        "DeepSeek & GLM",
        "the models that wrote the code",
        "https://deepseek.com",
    ),
];

/// The page title, or `Settings` for an unknown page.
pub fn title(page: &str) -> &'static str {
    match page {
        "profile" => "Profile",
        "account" => "Account",
        "privacy" => "Privacy",
        "chats" => "Chats",
        "ai" => "AI assistant",
        "shortcuts" => "Keyboard shortcuts",
        "help" => "Help and feedback",
        _ => "Settings",
    }
}

pub fn is_known_page(page: &str) -> bool {
    matches!(
        page,
        "home" | "profile" | "account" | "privacy" | "chats" | "ai" | "shortcuts" | "help"
    )
}

fn header_avatar(req: &SettingsRequest) -> String {
    let name = if req.me.is_empty() { "Me" } else { &req.me };
    let phone = phone_from_jid(&req.me_jid);
    format!(
        r#"
    <div class="settings-profile">
      <div class="avatar avatar-xl" data-avatar></div>
      <div class="settings-meta">
        <strong>{}</strong>
        <span>{}</span>
      </div>
    </div>"#,
        escape(name),
        if phone.is_empty() {
            "Not paired".to_string()
        } else {
            format!("+{}", escape(&phone))
        }
    )
}

fn link(page: &str, title: &str, detail: &str) -> String {
    format!(
        r#"<button type="button" class="settings-row clickable" data-page="{page}">
      <div class="settings-row-main"><strong>{}</strong><span>{}</span></div>
      <span class="settings-chevron">&#8250;</span>
    </button>"#,
        escape(title),
        escape(detail)
    )
}

fn static_row(title: &str, detail: &str) -> String {
    format!(
        r#"<div class="settings-row static">
      <div class="settings-row-main"><strong>{}</strong><span>{}</span></div>
    </div>"#,
        escape(title),
        escape(detail)
    )
}

fn external_row(title: &str, detail: &str, href: &str) -> String {
    format!(
        r#"<a class="settings-row static" href="{href}" target="_blank" rel="noreferrer noopener">
      <div class="settings-row-main"><strong>{}</strong><span>{}</span></div>
      <span class="settings-chevron">&#8250;</span>
    </a>"#,
        escape(title),
        escape(detail)
    )
}

fn credits() -> String {
    let rows: String = CREDITS
        .iter()
        .map(|(name, what, url)| external_row(name, what, url))
        .collect();
    format!(
        r#"<div class="settings-section">Special thanks</div>
    <div class="settings-list">{rows}</div>"#
    )
}

fn toggle(key: &str, title: &str, detail: &str, on: bool) -> String {
    format!(
        r#"<button type="button" class="settings-row clickable" data-toggle="{key}">
      <div class="settings-row-main"><strong>{}</strong><span>{}</span></div>
      <span class="switch {}"></span>
    </button>"#,
        escape(title),
        escape(detail),
        if on { "on" } else { "" }
    )
}

fn updates_row(state: &UpdateView) -> String {
    let status = if state.status.is_empty() {
        "Check whether a newer version of WA Desk has been published."
    } else {
        &state.status
    };
    let action = match state
        .version
        .as_deref()
        .filter(|version| !version.is_empty())
    {
        Some(version) => format!(
            r#"<button type="button" class="settings-edit" data-update="install">Update to {}</button>"#,
            escape(version)
        ),
        None => format!(
            r#"<button type="button" class="settings-edit" data-update="check"{disabled}>{label}</button>"#,
            disabled = if state.checking { " disabled" } else { "" },
            label = if state.checking {
                "Checking..."
            } else {
                "Check for updates"
            }
        ),
    };
    format!(
        r#"<div class="settings-list">
      <div class="settings-row static">
        <div class="settings-row-main"><strong>Updates</strong><span id="update-status">{}</span></div>
        {action}
      </div>
    </div>"#,
        escape(status)
    )
}

fn ai_form(ai: &AiView) -> String {
    let key_state = if ai.has_key {
        format!("Saved {}", ai.key_hint)
    } else {
        "Not set".to_string()
    };
    let require_detail = if ai.require_trigger {
        "Contacts must start with /ai before the assistant replies"
    } else {
        "Every saved-contact message gets an instant AI reply (when AI mode is on)"
    };
    let key_placeholder = if ai.has_key {
        "Leave empty to keep the saved key"
    } else {
        "sk-..."
    };
    format!(
        r#"
    <div class="settings-list">
      <button type="button" class="settings-row clickable" data-ai-toggle>
        <div class="settings-row-main">
          <strong>AI mode</strong>
          <span>Auto-reply to messages from saved contacts</span>
        </div>
        <span class="switch {enabled}"></span>
      </button>
      <button type="button" class="settings-row clickable" data-ai-require>
        <div class="settings-row-main">
          <strong>Require /ai trigger</strong>
          <span>{require_detail}</span>
        </div>
        <span class="switch {required}"></span>
      </button>
      {key_row}
    </div>
    <div class="ai-form">
      <label class="field">
        <span>API base URL</span>
        <input id="ai-base" value="{base_url}" spellcheck="false" />
      </label>
      <label class="field">
        <span>Model</span>
        <input id="ai-model" value="{model}" spellcheck="false" />
      </label>
      <label class="field">
        <span>Vision model (optional)</span>
        <input id="ai-vision-model" value="{vision}" spellcheck="false" placeholder="Empty = use the chat model" />
      </label>
      <label class="field">
        <span>API key</span>
        <input id="ai-key" type="password" autocomplete="off" placeholder="{key_placeholder}" />
      </label>
      <label class="field">
        <span>System prompt</span>
        <textarea id="ai-prompt" rows="4">{prompt}</textarea>
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
    </div>"#,
        enabled = if ai.enabled { "on" } else { "" },
        required = if ai.require_trigger { "on" } else { "" },
        key_row = static_row("API key", &key_state),
        base_url = escape(&ai.base_url),
        model = escape(&ai.model),
        vision = escape(&ai.vision_model),
        prompt = escape(&ai.system_prompt),
        key_placeholder = key_placeholder,
        require_detail = require_detail,
    )
}

/// Render the settings screen for one page.
pub fn render(req: &SettingsRequest) -> SettingsOutput {
    let page = if is_known_page(&req.page) {
        req.page.as_str()
    } else {
        "home"
    };
    let name = if req.me.is_empty() { "Me" } else { &req.me };
    let phone = phone_from_jid(&req.me_jid);

    let body = match page {
        "profile" => format!(
            r#"
      {}
      <div class="settings-list">
        <div class="settings-row static">
          <div class="settings-row-main"><strong>Name</strong><span>{}</span></div>
          <button type="button" class="settings-edit" data-action="edit">Edit</button>
        </div>
        <div class="settings-row static">
          <div class="settings-row-main"><strong>About</strong><span>Your status text</span></div>
          <button type="button" class="settings-edit" data-action="edit">Edit</button>
        </div>
        <div class="settings-row static">
          <div class="settings-row-main"><strong>Phone</strong><span>{}</span></div>
        </div>
      </div>"#,
            header_avatar(req),
            escape(name),
            if phone.is_empty() {
                "Unknown".to_string()
            } else {
                format!("+{}", escape(&phone))
            }
        ),
        "account" => format!(
            r#"
      {}
      <div class="settings-list">
        {}
        {}
        {}
        {}
        {}
      </div>
      {}"#,
            static_row("Security notifications", "Managed from your phone"),
            static_row(
                "Linked as",
                &if phone.is_empty() {
                    "Not paired".to_string()
                } else {
                    format!("+{phone}")
                }
            ),
            static_row(
                "Device",
                &if req.me_lid.is_empty() {
                    "Primary device".to_string()
                } else {
                    format!("LID {}", req.me_lid)
                }
            ),
            static_row("Build", &req.build),
            static_row("Request account info", "Available from your phone"),
            static_row(
                "How to delete my account",
                "Unlink this device first, then use your phone"
            ),
            updates_row(&req.update_state),
        ),
        "privacy" => format!(
            r#"
      <div class="settings-list">
        {}
        {}
        {}
        {}
        {}
      </div>"#,
            static_row("Last seen and online", "Managed from your phone"),
            static_row("Profile photo", "Managed from your phone"),
            static_row("Read receipts", "Sent when you open a chat"),
            static_row("Blocked contacts", "Managed from your phone"),
            static_row("Disappearing messages", "Per-chat setting from your phone"),
        ),
        "chats" => format!(
            r#"
      <div class="settings-list">
        <button type="button" class="settings-row clickable" data-action="theme">
          <div class="settings-row-main"><strong>Theme</strong><span data-theme-label>{}</span></div>
          <span class="settings-chevron" aria-hidden="true">&#8250;</span>
        </button>
        {}
        {}
        {}
        {}
        <button type="button" class="settings-row clickable" data-action="sync-history">
          <div class="settings-row-main"><strong>Sync chat history</strong><span>Ask your phone for recent chats and messages (approve it on the phone)</span></div>
          <span class="settings-chevron" aria-hidden="true">&#8250;</span>
        </button>
      </div>"#,
            escape(&req.theme_label),
            toggle(
                "enterSends",
                "Enter is send",
                "Enter sends, Shift+Enter adds a line",
                req.prefs.enter_sends
            ),
            toggle(
                "spellcheck",
                "Spell check",
                "Underline misspelled words in the composer",
                req.prefs.spellcheck
            ),
            toggle(
                "notifications",
                "Desktop notifications",
                "Show a system notification for new messages",
                req.prefs.notifications
            ),
            toggle(
                "autostart",
                "Launch at login",
                "Start WA Desk when you sign in",
                req.prefs.autostart
            ),
        ),
        "ai" => ai_form(&req.ai),
        "shortcuts" => format!(
            r#"
      <div class="settings-list">
        {}
        {}
        {}
        {}
        {}
      </div>"#,
            static_row("Ctrl+V", "Paste an image as a draft (stack up to 10)"),
            static_row("Enter", "Send the message (when enabled above)"),
            static_row("Esc", "Close the picker, draft, menu or panel"),
            static_row("Hold a bubble", "Edit, delete for me, delete for everyone"),
            static_row("Eye button", "Reveal deleted chats and messages"),
        ),
        "help" => format!(
            r#"
      <div class="settings-list">
        {}
        {}
        {}
      </div>
      <div class="settings-list">
        {}
        {}
        {}
      </div>
      {}"#,
            static_row(
                "WA Desk",
                "Unofficial client built on whatsapp-rust + Tauri"
            ),
            static_row(
                "Not affiliated",
                "WhatsApp and Meta are trademarks of Meta Platforms"
            ),
            static_row(
                "Privacy",
                "Keys and history stay on this device; nothing is uploaded by this app"
            ),
            external_row("Source code", "github.com/dirtTW/wa-desk", SOURCE_URL),
            external_row(
                "License",
                "AGPL-3.0-only — the binary links Signal's AGPL code",
                LICENSE_URL
            ),
            static_row("Version", &req.build),
            credits(),
        ),
        _ => format!(
            r#"
      {}
      <div class="settings-list">
        {}
        {}
        {}
        {}
        {}
        {}
        {}
        <button type="button" class="settings-row danger" data-action="logout">Log out</button>
      </div>"#,
            header_avatar(req),
            link("profile", "Profile", "Name, about, phone number"),
            link("account", "Account", "Security notifications, account info"),
            link("privacy", "Privacy", "Blocked contacts, read receipts"),
            link("chats", "Chats", "Theme, Enter to send, spell check"),
            link("ai", "AI assistant", "Auto-reply with your own API key"),
            link("shortcuts", "Keyboard shortcuts", "Quick actions"),
            link("help", "Help and feedback", "About this client"),
        ),
    };

    SettingsOutput {
        title: title(page).to_string(),
        page: page.to_string(),
        html: body,
    }
}

/// The label for a stored theme preference.
pub fn theme_label(preference: &str) -> &'static str {
    match preference {
        "light" => "Light",
        "dark" => "Dark",
        _ => "System default",
    }
}

/// The theme actually painted: an explicit choice wins, otherwise the OS.
pub fn resolve_theme(preference: &str, system_dark: bool) -> &'static str {
    match preference {
        "light" => "light",
        "dark" => "dark",
        _ => {
            if system_dark {
                "dark"
            } else {
                "light"
            }
        }
    }
}

/// The toggle shows the theme you would switch **to**, like the official client.
pub fn theme_toggle_label(resolved: &str) -> &'static str {
    if resolved == "dark" { "Light" } else { "Dark" }
}

/// The Material icon the toggle shows.
pub fn theme_toggle_icon(resolved: &str) -> &'static str {
    if resolved == "dark" {
        "light_mode"
    } else {
        "dark_mode"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(page: &str) -> SettingsRequest {
        SettingsRequest {
            page: page.to_string(),
            me: "Ada".into(),
            me_jid: "6281234567890@s.whatsapp.net".into(),
            me_lid: String::new(),
            theme_label: "System default".into(),
            build: "0.2.0-rc.3 · WA Desk".into(),
            prefs: PrefsView {
                enter_sends: true,
                spellcheck: false,
                notifications: true,
                autostart: false,
            },
            update_state: UpdateView {
                checking: false,
                version: None,
                status: String::new(),
            },
            ai: AiView {
                enabled: true,
                require_trigger: false,
                base_url: "https://opencode.ai/zen/go/v1".into(),
                model: "deepseek-v4.1-flash".into(),
                vision_model: String::new(),
                system_prompt: "you are <me>".into(),
                has_key: true,
                key_hint: "••••1234".into(),
            },
            with_avatar: true,
        }
    }

    #[test]
    fn theme_resolution_and_toggle_labels() {
        assert_eq!(theme_label("light"), "Light");
        assert_eq!(theme_label("dark"), "Dark");
        assert_eq!(theme_label("system"), "System default");
        assert_eq!(theme_label(""), "System default");

        assert_eq!(
            resolve_theme("light", true),
            "light",
            "an explicit choice wins"
        );
        assert_eq!(resolve_theme("dark", false), "dark");
        assert_eq!(resolve_theme("system", true), "dark");
        assert_eq!(resolve_theme("system", false), "light");

        assert_eq!(theme_toggle_label("dark"), "Light");
        assert_eq!(theme_toggle_label("light"), "Dark");
        assert_eq!(theme_toggle_icon("dark"), "light_mode");
        assert_eq!(theme_toggle_icon("light"), "dark_mode");
    }

    #[test]
    fn home_lists_every_page_and_hides_the_rest() {
        let out = render(&request("home"));
        assert_eq!(out.title, "Settings");
        for page in [
            "profile",
            "account",
            "privacy",
            "chats",
            "ai",
            "shortcuts",
            "help",
        ] {
            assert!(
                out.html.contains(&format!(r#"data-page="{page}""#)),
                "{page}"
            );
        }
        assert!(out.html.contains("Log out"));
        assert!(out.html.contains("+6281234567890"));
    }

    #[test]
    fn unknown_pages_fall_back_to_home() {
        let out = render(&request("nope"));
        assert_eq!(out.title, "Settings");
        assert!(out.html.contains("data-page=\"profile\""));
    }

    #[test]
    fn account_page_has_build_phone_and_updates() {
        let out = render(&request("account"));
        assert_eq!(out.title, "Account");
        assert!(out.html.contains("6281234567890"));
        assert!(out.html.contains("0.2.0-rc.3"));
        assert!(out.html.contains("Primary device"));
        assert!(out.html.contains("Check for updates"));

        let mut req = request("account");
        req.update_state = UpdateView {
            checking: false,
            version: Some("0.2.0-rc.4".into()),
            status: "Version 0.2.0-rc.4 is available".into(),
        };
        let out = render(&req);
        assert!(out.html.contains("Update to 0.2.0-rc.4"));
        assert!(out.html.contains("is available"));

        req.update_state.checking = true;
        req.update_state.version = None;
        let out = render(&req);
        assert!(out.html.contains("Checking..."));
        assert!(out.html.contains("disabled"));
    }

    #[test]
    fn chats_page_reflects_preferences() {
        let out = render(&request("chats"));
        assert_eq!(out.title, "Chats");
        assert!(out.html.contains("System default"));
        assert!(out.html.contains(r#"data-toggle="enterSends""#));
        assert!(out.html.contains(r#"class="switch on"></span>"#));
        assert!(out.html.contains(r#"data-toggle="spellcheck""#));
        assert!(out.html.contains("Sync chat history"));
        // spellcheck is off, so at least one switch is not "on"
        assert!(out.html.matches(r#"class="switch "></span>"#).count() >= 1);
    }

    #[test]
    fn ai_page_escapes_and_mirrors_state() {
        let out = render(&request("ai"));
        assert_eq!(out.title, "AI assistant");
        assert!(
            out.html
                .contains(r#"value="https://opencode.ai/zen/go/v1""#)
        );
        assert!(out.html.contains("deepseek-v4.1-flash"));
        assert!(out.html.contains("Saved ••••1234"));
        assert!(out.html.contains("you are &lt;me&gt;"));
        assert!(out.html.contains("Leave empty to keep the saved key"));
        assert!(out.html.contains("data-ai-toggle"));
        assert!(out.html.contains(r#"class="switch on"></span>"#));
    }

    #[test]
    fn help_page_lists_credits_and_links() {
        let out = render(&request("help"));
        assert_eq!(out.title, "Help and feedback");
        assert!(out.html.contains(SOURCE_URL));
        assert!(out.html.contains(LICENSE_URL));
        assert!(out.html.contains("Special thanks"));
        assert!(out.html.contains("whatsapp-rust"));
        assert!(out.html.contains("DeepSeek &amp; GLM"));
        assert!(out.html.contains("AGPL-3.0-only"));
    }

    #[test]
    fn profile_and_privacy_and_shortcuts() {
        let profile = render(&request("profile"));
        assert_eq!(profile.title, "Profile");
        assert!(profile.html.contains("data-avatar"));
        assert!(profile.html.contains("+6281234567890"));

        let privacy = render(&request("privacy"));
        assert!(privacy.html.contains("Read receipts"));

        let shortcuts = render(&request("shortcuts"));
        assert!(shortcuts.html.contains("Ctrl+V"));
        assert_eq!(shortcuts.title, "Keyboard shortcuts");
    }
}
