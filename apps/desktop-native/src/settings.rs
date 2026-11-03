//! The native app's own settings: theme and the AI form.
//!
//! The rules (theme resolution, the AI form's validation and payload) come from
//! `wa-ui::settings`, so this module only stores values and decides *when* to
//! ask them.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiFields {
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
    /// The key the user typed on this screen; never round-tripped to disk.
    #[serde(skip)]
    pub api_key: String,
    /// Whether a key is already stored (the form shows a masked hint).
    #[serde(default)]
    pub has_key: bool,
}

impl Default for AiFields {
    fn default() -> Self {
        Self {
            enabled: false,
            require_trigger: false,
            base_url: "https://opencode.ai/zen/go/v1".to_string(),
            model: "deepseek-v4.1-flash".to_string(),
            vision_model: String::new(),
            system_prompt: String::new(),
            api_key: String::new(),
            has_key: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeSettings {
    /// `light`, `dark` or `system`.
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default)]
    pub ai: AiFields,
}

fn default_theme() -> String {
    "system".to_string()
}

impl Default for NativeSettings {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            ai: AiFields::default(),
        }
    }
}

impl NativeSettings {
    pub fn load(path: &std::path::Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &std::path::Path) -> Result<(), String> {
        let text = serde_json::to_string_pretty(self).map_err(|err| err.to_string())?;
        std::fs::write(path, text).map_err(|err| err.to_string())
    }

    /// What the window paints: an explicit choice wins, otherwise the OS.
    pub fn resolved_theme(&self, system_dark: bool) -> &'static str {
        wa_ui::settings::resolve_theme(&self.theme, system_dark)
    }

    pub fn theme_label(&self) -> &'static str {
        wa_ui::settings::theme_label(&self.theme)
    }

    /// The AI form as `wa-ui` validates it.
    pub fn ai_plan(&self) -> wa_ui::settings::AiSavePlan {
        wa_ui::settings::ai_save_plan(&wa_ui::settings::AiFormInput {
            enabled: self.ai.enabled,
            require_trigger: self.ai.require_trigger,
            base_url: self.ai.base_url.clone(),
            model: self.ai.model.clone(),
            vision_model: self.ai.vision_model.clone(),
            system_prompt: self.ai.system_prompt.clone(),
            api_key: self.ai.api_key.clone(),
        })
    }

    /// The masked hint the key field shows.
    pub fn key_hint(&self) -> String {
        if self.ai.has_key {
            "•••• (a key is stored)".to_string()
        } else {
            "no key stored".to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn themes_resolve_from_the_choice_and_the_os() {
        let mut settings = NativeSettings::default();
        assert_eq!(settings.theme_label(), "System default");
        assert_eq!(settings.resolved_theme(true), "dark");
        assert_eq!(settings.resolved_theme(false), "light");

        settings.theme = "light".into();
        assert_eq!(settings.theme_label(), "Light");
        assert_eq!(
            settings.resolved_theme(true),
            "light",
            "an explicit choice wins"
        );
    }

    #[test]
    fn the_ai_form_validates_and_keeps_a_blank_key() {
        let mut settings = NativeSettings::default();
        let plan = settings.ai_plan();
        assert!(plan.ok, "the defaults are valid");
        assert!(
            plan.settings["apiKey"].is_null(),
            "a blank key stays stored"
        );

        settings.ai.base_url = "ftp://nope".into();
        assert!(settings.ai_plan().error.unwrap().contains("http"));

        settings.ai.base_url = "https://example.test/v1".into();
        settings.ai.model.clear();
        assert_eq!(
            settings.ai_plan().error.as_deref(),
            Some("Model cannot be empty.")
        );
    }

    #[test]
    fn settings_round_trip_and_never_store_the_key() {
        let dir = std::env::temp_dir().join(format!("wa-native-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join("settings.json");

        let settings = NativeSettings {
            theme: "dark".into(),
            ai: AiFields {
                model: "m1".into(),
                api_key: "sk-secret".into(),
                ..Default::default()
            },
        };
        settings.save(&path).expect("save");

        let text = std::fs::read_to_string(&path).expect("read");
        assert!(!text.contains("sk-secret"), "the key must not hit disk");

        let loaded = NativeSettings::load(&path);
        assert_eq!(loaded.theme, "dark");
        assert_eq!(loaded.ai.model, "m1");
        assert!(loaded.ai.api_key.is_empty());

        let _ = std::fs::remove_file(&path);
    }
}
