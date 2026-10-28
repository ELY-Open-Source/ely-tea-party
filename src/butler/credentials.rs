use std::collections::HashMap;

use anyhow::Context as _;
use gpui::{App, Global, Task};

use super::config::PROVIDERS;

#[cfg(target_os = "macos")]
mod macos;

#[derive(Default)]
pub struct Credentials {
    keys: HashMap<String, Result<Option<String>, String>>,
    pub saving: bool,
}

impl Global for Credentials {}

impl Credentials {
    pub fn init(cx: &mut App) {
        cx.set_global(Self::default());
        for provider in PROVIDERS.iter() {
            Self::load(&provider.id, cx);
        }
    }

    pub fn load(provider: &str, cx: &mut App) {
        let provider = provider.to_string();
        cx.global_mut::<Self>().keys.remove(&provider);
        #[cfg(target_os = "macos")]
        let read = macos::read(&service(&provider), &provider, cx);
        #[cfg(not(target_os = "macos"))]
        let read = cx.read_credentials(&service(&provider));
        cx.spawn(async move |cx| {
            let result = read.await.and_then(decode).map_err(|error| {
                format!(
                    "butler: cannot read {provider} API key from system credential store: {error:#}"
                )
            });
            match &result {
                Ok(_) => log::info!("butler: {provider} credentials loaded"),
                Err(error) => log::error!("{error}"),
            }
            cx.update(|cx| {
                cx.global_mut::<Self>().keys.insert(provider, result);
            });
        })
        .detach();
    }

    pub fn loaded(&self, provider: &str) -> bool {
        self.keys.contains_key(provider)
    }

    pub fn key(&self, provider: &str) -> anyhow::Result<Option<&str>> {
        match self.keys.get(provider) {
            Some(Ok(key)) => Ok(key.as_deref()),
            Some(Err(error)) => anyhow::bail!("{error}"),
            None => anyhow::bail!("butler: loading {provider} credentials; try again shortly"),
        }
    }

    pub fn save(provider: &str, key: Option<String>, cx: &mut App) -> Task<anyhow::Result<()>> {
        if !cx.global::<Self>().loaded(provider) {
            return Task::ready(Err(anyhow::anyhow!(
                "butler: wait for credentials to load before saving"
            )));
        }
        if cx.global::<Self>().saving {
            return Task::ready(Err(anyhow::anyhow!(
                "butler: a credential save is already running"
            )));
        }
        let provider = provider.to_string();
        #[cfg(target_os = "macos")]
        let operation = macos::save(&service(&provider), &provider, key.clone(), cx);
        #[cfg(not(target_os = "macos"))]
        let operation = match &key {
            Some(key) => cx.write_credentials(&service(&provider), &provider, key.as_bytes()),
            None => cx.delete_credentials(&service(&provider)),
        };
        cx.global_mut::<Self>().saving = true;
        cx.spawn(async move |cx| {
            let result = operation.await.with_context(|| {
                format!("butler: cannot save {provider} API key in system credential store")
            });
            cx.update(|cx| {
                cx.global_mut::<Self>().finish_save(provider, key, &result);
            });
            result
        })
    }

    fn finish_save(&mut self, provider: String, key: Option<String>, result: &anyhow::Result<()>) {
        self.saving = false;
        if result.is_ok() {
            log::info!("butler: {provider} credentials saved");
            self.keys.insert(provider, Ok(key));
        }
    }
}

fn service(provider: &str) -> String {
    let mut service = format!("{}.agent-butler.{provider}", crate::app::APP_ID);
    if cfg!(debug_assertions) || option_env!("ELY_DEV").is_some() {
        service.push_str(".dev");
        if let Some(folder) = std::env::var_os("ELY_TEST_CONFIG_DIR") {
            use sha2::{Digest, Sha256};
            service.push_str(&format!(".{:x}", Sha256::digest(folder.as_encoded_bytes())));
        }
    }
    service
}

fn decode(entry: Option<(String, Vec<u8>)>) -> anyhow::Result<Option<String>> {
    entry
        .map(|(_, bytes)| String::from_utf8(bytes).context("stored API key is not UTF-8"))
        .transpose()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_writes_and_deletes_keep_the_working_key() {
        let mut credentials = Credentials::default();
        credentials.finish_save("deepseek".into(), Some("test-old".into()), &Ok(()));
        for next in [Some("test-new".into()), None] {
            credentials.saving = true;
            credentials.finish_save("deepseek".into(), next, &Err(anyhow::anyhow!("locked")));
            assert!(!credentials.saving);
            assert_eq!(credentials.key("deepseek").unwrap(), Some("test-old"));
        }
        credentials.finish_save("deepseek".into(), Some("test-new".into()), &Ok(()));
        assert_eq!(credentials.key("deepseek").unwrap(), Some("test-new"));
        credentials.finish_save("deepseek".into(), None, &Ok(()));
        assert_eq!(credentials.key("deepseek").unwrap(), None);
    }

    #[test]
    fn loading_and_read_errors_are_not_missing_credentials() {
        let mut credentials = Credentials::default();
        assert!(!credentials.loaded("deepseek"));
        assert!(credentials.key("deepseek").is_err());
        credentials
            .keys
            .insert("deepseek".into(), Err("locked".into()));
        assert!(credentials.loaded("deepseek"));
        assert!(
            credentials
                .key("deepseek")
                .unwrap_err()
                .to_string()
                .contains("locked")
        );
        credentials.keys.insert("deepseek".into(), Ok(None));
        assert_eq!(credentials.key("deepseek").unwrap(), None);
    }

    #[test]
    fn restored_keys_are_separate_per_provider_and_not_settings() {
        let mut credentials = Credentials::default();
        for provider in ["deepseek", "openai"] {
            let key = decode(Some((
                provider.into(),
                format!("test-{provider}").into_bytes(),
            )))
            .unwrap();
            credentials.keys.insert(provider.into(), Ok(key));
        }
        assert_eq!(credentials.key("deepseek").unwrap(), Some("test-deepseek"));
        assert_eq!(credentials.key("openai").unwrap(), Some("test-openai"));
        assert_ne!(service("deepseek"), service("openai"));
        let settings = crate::settings::Settings::default();
        assert_eq!(
            settings.butler.api_key(&credentials).unwrap(),
            "test-deepseek"
        );
        assert!(
            !serde_json::to_string(&settings)
                .unwrap()
                .contains("test-deepseek")
        );
    }

    #[test]
    fn invalid_stored_bytes_fail_without_exposing_them() {
        let error = decode(Some(("deepseek".into(), vec![0xff, 0xfe]))).unwrap_err();
        assert_eq!(error.to_string(), "stored API key is not UTF-8");
        assert_eq!(decode(None).unwrap(), None);
    }
}
