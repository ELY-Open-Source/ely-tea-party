use std::{collections::HashMap, sync::LazyLock};

use gpui::Global;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub enabled: bool,
    pub companion: bool,
    pub memory: bool,
    pub memory_ai: bool,
    pub provider: String,
    pub model: String,
    pub persona: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: false,
            companion: false,
            memory: false,
            memory_ai: false,
            provider: "deepseek".into(),
            model: "deepseek-flash".into(),
            persona: String::new(),
        }
    }
}

#[derive(Deserialize)]
pub struct ModelChoice {
    pub id: String,
    pub label: String,
}

#[derive(Deserialize)]
pub struct Provider {
    pub id: String,
    pub label: String,
    pub env: String,
    pub models: Vec<ModelChoice>,
}

pub static PROVIDERS: LazyLock<Vec<Provider>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../../butler/models.json"))
        .expect("valid bundled model choices")
});

pub fn default_persona(language: &str) -> &'static str {
    static PERSONAS: LazyLock<HashMap<String, String>> = LazyLock::new(|| {
        serde_json::from_str(include_str!("../../butler/persona.json"))
            .expect("valid bundled personas")
    });
    PERSONAS.get(language).expect("supported UI language")
}

impl Config {
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            PROVIDERS.iter().any(|p| p.id == self.provider),
            "butler: unsupported provider"
        );
        anyhow::ensure!(
            self.persona.chars().count() <= 2_000,
            "butler: persona must be at most 2000 characters"
        );
        anyhow::ensure!(
            !self.model.is_empty()
                && self.model.len() <= 160
                && self
                    .model
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "/-_.:~".contains(c)),
            "butler: invalid model ID"
        );
        Ok(())
    }

    pub fn api_key(&self, credentials: &Credentials) -> anyhow::Result<String> {
        self.validate()?;
        if let Some(key) = credentials
            .0
            .get(&self.provider)
            .filter(|key| !key.is_empty())
        {
            return Ok(key.clone());
        }
        let env = PROVIDERS
            .iter()
            .find(|p| p.id == self.provider)
            .expect("validated provider")
            .env
            .as_str();
        std::env::var(env).ok().filter(|key| !key.trim().is_empty())
            .ok_or_else(|| anyhow::anyhow!("butler: enter an API key in Settings → Agent Butler, or set {env} before launching the app"))
    }
}

#[derive(Default)]
pub struct Credentials(pub HashMap<String, String>);
impl Global for Credentials {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_are_optional_and_cannot_contain_credentials() {
        let old: crate::settings::Settings = serde_json::from_str("{}").unwrap();
        assert!(!old.butler.enabled);
        assert!(!old.butler.companion);
        assert!(!old.butler.memory);
        assert!(!old.butler.memory_ai);
        assert!(serde_json::from_str::<Config>(r#"{"api_key":"example"}"#).is_err());
        let mut credentials = Credentials::default();
        credentials
            .0
            .insert("deepseek".into(), "test-only-secret".into());
        assert_eq!(
            old.butler.api_key(&credentials).unwrap(),
            "test-only-secret"
        );
        assert!(
            !serde_json::to_string(&old)
                .unwrap()
                .contains("test-only-secret")
        );
    }

    #[test]
    fn persona_survives_settings_roundtrip_and_limits_unicode_characters() {
        let mut config: Config = serde_json::from_str(r#"{"enabled":true}"#).unwrap();
        assert!(config.persona.is_empty());
        config.companion = true;
        config.persona = "叫我船长。\n语气活泼，先说需要我处理的事。".into();
        let restored: Config =
            serde_json::from_str(&serde_json::to_string(&config).unwrap()).unwrap();
        assert_eq!(restored, config);
        config.persona = "鲸".repeat(2_000);
        assert!(config.validate().is_ok());
        config.persona.push('鱼');
        assert!(config.validate().is_err());
    }
}
