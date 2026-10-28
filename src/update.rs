//! Whether a newer release than this build is out, asked of the site's download store, else GitHub, and on macOS putting it in this app's place.

use std::time::Duration;

use anyhow::Context as _;
use gpui::{App, Global, Task, actions};
use serde::Deserialize;

#[cfg(target_os = "macos")]
mod macos;

use crate::settings::{Language, SettingsStore};

actions!(ely_tea_party, [CheckForUpdates]);

/// The download store's newest release, which the website's `scripts/mirror.mjs` copies there from GitHub.
const MANIFEST: &str = "https://download.elyteaparty.com/latest.json";

/// GitHub's, asked when the store's cannot be read.
const LATEST: &str = "https://api.github.com/repos/ELY-Open-Source/ely-tea-party/releases/latest";

/// How often the app asks on its own.
const EVERY: Duration = Duration::from_secs(6 * 3600);

/// How long the answer to a check asked by hand stays in the title bar.
const SHOWN: Duration = Duration::from_secs(6);

/// The version this build is.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// A published release.
#[derive(Clone, Debug, PartialEq)]
pub struct Release {
    pub version: String,
    /// Where a person downloads it, in each interface language.
    pub pages: Pages,
    /// Its macOS disk image.
    pub image: Option<Image>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Pages {
    pub en: String,
    pub zh: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    pub url: String,
    /// Its SHA-256 in hex, which the download store lists and GitHub does not.
    pub sha256: Option<String>,
}

impl Release {
    /// Its download page in the interface's language.
    pub fn page<'a>(&'a self, cx: &App) -> &'a str {
        match SettingsStore::get(cx).language {
            Language::SimplifiedChinese => &self.pages.zh,
            Language::English => &self.pages.en,
        }
    }
}

#[derive(Deserialize)]
struct Manifest {
    version: String,
    files: Files,
    pages: ManifestPages,
}

#[derive(Deserialize)]
struct Files {
    mac: Option<File>,
}

#[derive(Deserialize)]
struct File {
    url: String,
    sha256: String,
}

#[derive(Deserialize)]
struct ManifestPages {
    download: Localized,
}

#[derive(Deserialize)]
struct Localized {
    en: String,
    zh: String,
}

#[derive(Deserialize)]
struct Answer {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
}

/// The newest release that is neither a draft nor a pre-release, from the download store, else GitHub; it blocks.
pub fn latest() -> anyhow::Result<Release> {
    stored().or_else(|error| {
        log::info!("{error:#}, so GitHub is asked");
        let headers = "Accept: application/vnd.github+json\nUser-Agent: ely-tea-party\n";
        let body = crate::curl::get("update", LATEST, headers)?;
        let answer: Answer =
            serde_json::from_slice(&body).context("update: GitHub's answer does not parse")?;
        release(answer)
    })
}

fn stored() -> anyhow::Result<Release> {
    let body = crate::curl::get("update", MANIFEST, "User-Agent: ely-tea-party\n")?;
    let manifest: Manifest = serde_json::from_slice(&body)
        .context("update: the download store's latest.json does not parse")?;
    manifested(manifest)
}

fn manifested(manifest: Manifest) -> anyhow::Result<Release> {
    numbers(&manifest.version)?;
    let image = manifest.files.mac.map(|file| Image {
        url: file.url,
        sha256: Some(file.sha256.to_ascii_lowercase()),
    });
    anyhow::ensure!(
        image
            .as_ref()
            .is_none_or(|image| image.url.starts_with("https://")),
        "update: the download store's disk image is not on https"
    );
    let Localized { en, zh } = manifest.pages.download;
    Ok(Release {
        version: manifest.version,
        pages: Pages { en, zh },
        image,
    })
}

fn release(answer: Answer) -> anyhow::Result<Release> {
    let version = answer
        .tag_name
        .strip_prefix('v')
        .with_context(|| format!("update: tag {} names no version", answer.tag_name))?
        .to_string();
    let image = format!("ely-tea-party-{version}.dmg");
    let image = answer
        .assets
        .into_iter()
        .find(|asset| asset.name == image)
        .map(|asset| Image {
            url: asset.browser_download_url,
            sha256: None,
        });
    Ok(Release {
        version,
        pages: Pages {
            en: answer.html_url.clone(),
            zh: answer.html_url,
        },
        image,
    })
}

/// `file`'s SHA-256, in hex.
#[cfg(any(target_os = "macos", test))]
pub fn sha256(file: &std::path::Path) -> anyhow::Result<String> {
    use sha2::{Digest, Sha256};
    let mut read = std::fs::File::open(file)
        .with_context(|| format!("update: cannot open {}", file.display()))?;
    let mut hash = Sha256::new();
    std::io::copy(&mut read, &mut hash)
        .with_context(|| format!("update: cannot read {}", file.display()))?;
    Ok(format!("{:x}", hash.finalize()))
}

fn numbers(version: &str) -> anyhow::Result<Vec<u64>> {
    version
        .split('.')
        .map(|part| part.parse::<u64>())
        .collect::<Result<_, _>>()
        .with_context(|| format!("update: {version} is not a version"))
}

/// Whether `version` comes after `than`.
pub fn newer(version: &str, than: &str) -> anyhow::Result<bool> {
    Ok(numbers(version)? > numbers(than)?)
}

/// Where the updater stands, as the title bar shows it.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum State {
    #[default]
    Idle,
    /// A check asked by hand found none newer.
    Current,
    /// A check asked by hand failed.
    Unchecked(String),
    Available(Release),
    Installing(Release),
    /// In place; a restart runs it.
    Ready(Release),
    Failed(Release, String),
}

/// The updater: its state, the check under way, if any, and whether it was asked by hand.
pub struct Updates {
    state: State,
    asking: Option<bool>,
    _check: Option<Task<()>>,
    _install: Option<Task<()>>,
    _clear: Option<Task<()>>,
}

impl Global for Updates {}

/// The updater's state.
pub fn state(cx: &App) -> &State {
    &cx.global::<Updates>().state
}

/// Whether a check asked by hand is under way.
pub fn asked(cx: &App) -> bool {
    cx.global::<Updates>().asking == Some(true)
}

/// Checks at start and every few hours while the settings ask, and binds Check for Updates.
pub fn init(cx: &mut App) {
    cx.set_global(Updates {
        state: State::Idle,
        asking: None,
        _check: None,
        _install: None,
        _clear: None,
    });
    cx.on_action(|_: &CheckForUpdates, cx| check(true, cx));
    cx.spawn(async move |cx| {
        loop {
            cx.update(|cx| {
                if SettingsStore::get(cx).check_updates {
                    check(false, cx);
                }
            });
            cx.background_executor().timer(EVERY).await;
        }
    })
    .detach();
}

fn set(state: State, cx: &mut App) {
    log::info!("update: {state:?}");
    let transient = matches!(state, State::Current | State::Unchecked(_));
    let clear = transient.then(|| {
        let shown = state.clone();
        cx.spawn(async move |cx| {
            cx.background_executor().timer(SHOWN).await;
            cx.update_global::<Updates, _>(|updates, _| {
                if updates.state == shown {
                    log::debug!("update: {shown:?} cleared");
                    updates.state = State::Idle;
                }
            });
        })
    });
    let updates = cx.global_mut::<Updates>();
    updates.state = state;
    updates._clear = clear;
}

/// Asks for the newest release, unless a check or an install is under way.
pub fn check(asked: bool, cx: &mut App) {
    let updates = cx.global::<Updates>();
    let busy = matches!(updates.state, State::Installing(_) | State::Ready(_));
    if busy || updates.asking.is_some() {
        if asked {
            log::info!("update: busy, so no check now");
        }
        return;
    }
    log::debug!("update: checking{}", if asked { ", asked" } else { "" });
    let read = cx.background_executor().spawn(async move {
        let release = latest()?;
        let newer = newer(&release.version, VERSION)?;
        anyhow::Ok(newer.then_some(release))
    });
    let task = cx.spawn(async move |cx| {
        let read = read.await;
        cx.update(|cx| {
            cx.global_mut::<Updates>().asking = None;
            let state = cx.global::<Updates>().state.clone();
            if matches!(state, State::Installing(_) | State::Ready(_)) {
                return;
            }
            match read {
                Ok(Some(release)) if state != State::Available(release.clone()) => {
                    set(State::Available(release), cx)
                }
                Ok(Some(_)) => {}
                Ok(None) if asked => set(State::Current, cx),
                Ok(None) => log::debug!("update: {VERSION} is the newest"),
                Err(error) if asked => set(State::Unchecked(format!("{error:#}")), cx),
                Err(error) => log::info!("{error:#}"),
            }
        });
    });
    let updates = cx.global_mut::<Updates>();
    updates.asking = Some(asked);
    updates._check = Some(task);
}

/// What a press on the title bar's button does: install, restart, or open the release's page.
pub fn press(cx: &mut App) {
    match state(cx).clone() {
        State::Available(release) => install(release, cx),
        State::Ready(release) => restart(release, cx),
        State::Failed(release, _) => {
            cx.open_url(release.page(cx));
            set(State::Available(release), cx);
        }
        State::Current | State::Unchecked(_) => set(State::Idle, cx),
        State::Idle | State::Installing(_) => {}
    }
}

#[cfg(target_os = "macos")]
fn install(release: Release, cx: &mut App) {
    let Some(bundle) = macos::bundle() else {
        log::info!(
            "update: not a release bundle, so {} opens",
            release.page(cx)
        );
        cx.open_url(release.page(cx));
        return;
    };
    set(State::Installing(release.clone()), cx);
    let placed = {
        let release = release.clone();
        cx.background_executor()
            .spawn(async move { macos::install(&release, &bundle) })
    };
    let task = cx.spawn(async move |cx| {
        let placed = placed.await;
        cx.update(|cx| match placed {
            Ok(()) => set(State::Ready(release), cx),
            Err(error) => set(State::Failed(release, format!("{error:#}")), cx),
        });
    });
    cx.global_mut::<Updates>()._install = Some(task);
}

#[cfg(not(target_os = "macos"))]
fn install(release: Release, cx: &mut App) {
    log::info!("update: {} opens", release.page(cx));
    cx.open_url(release.page(cx));
}

#[cfg(target_os = "macos")]
fn restart(release: Release, cx: &mut App) {
    match macos::bundle()
        .context("update: the app left its bundle")
        .and_then(|bundle| macos::relaunch(&bundle))
    {
        Ok(()) => {
            log::info!("update: restarting into {}", release.version);
            cx.quit();
        }
        Err(error) => set(State::Failed(release, format!("{error:#}")), cx),
    }
}

#[cfg(not(target_os = "macos"))]
fn restart(release: Release, cx: &mut App) {
    cx.open_url(release.page(cx));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_by_number() {
        assert!(newer("0.3.4", "0.3.3").expect("versions"));
        assert!(newer("0.10.0", "0.9.9").expect("versions"));
        assert!(newer("1.0", "0.9.9").expect("versions"));
        assert!(!newer("0.3.3", "0.3.3").expect("versions"));
        assert!(!newer("0.3.2", "0.3.3").expect("versions"));
        assert!(newer("0.4.0-beta", "0.3.3").is_err());
    }

    #[test]
    fn a_release_reads_its_version_and_disk_image() {
        let answer: Answer = serde_json::from_str(
            r#"{"tag_name":"v0.3.4","html_url":"https://github.com/ELY-Open-Source/ely-tea-party/releases/tag/v0.3.4","draft":false,"assets":[
                {"name":"ely-tea-party-0.3.4-linux-x86_64.tar.gz","browser_download_url":"https://example.com/linux"},
                {"name":"ely-tea-party-0.3.4.dmg","browser_download_url":"https://example.com/dmg"}
            ]}"#,
        )
        .expect("parses");
        let release = release(answer).expect("a release");
        assert_eq!(release.version, "0.3.4");
        let image = release.image.expect("a disk image");
        assert_eq!(image.url, "https://example.com/dmg");
        assert_eq!(image.sha256, None);
        assert_eq!(release.pages.zh, release.pages.en);
        let bare: Answer =
            serde_json::from_str(r#"{"tag_name":"0.3.4","html_url":"x"}"#).expect("parses");
        assert!(super::release(bare).is_err());
    }

    fn manifest(version: &str, url: &str) -> anyhow::Result<Release> {
        let json = format!(
            r#"{{"version":"{version}","published":"2026-10-28T05:30:59Z","files":{{
                "mac":{{"name":"ely-tea-party-{version}.dmg","url":"{url}","size":1,"sha256":"7BF516A1"}},
                "linux":{{"name":"x","url":"https://example.com/linux","size":1,"sha256":"35ac"}}
            }},"pages":{{"download":{{"zh":"https://elyteaparty.com/#download","en":"https://elyteaparty.com/en/#download"}},
                "notes":{{"zh":"z","en":"e"}},"github":"g"}},"kept":[]}}"#
        );
        manifested(serde_json::from_str(&json).expect("parses"))
    }

    #[test]
    fn the_download_store_names_its_disk_image_hash_and_pages() {
        let release = manifest(
            "0.3.15",
            "https://download.elyteaparty.com/v0.3.15/ely-tea-party-0.3.15.dmg",
        )
        .expect("a release");
        assert_eq!(release.version, "0.3.15");
        let image = release.image.expect("a disk image");
        assert_eq!(image.sha256.as_deref(), Some("7bf516a1"));
        assert_eq!(release.pages.en, "https://elyteaparty.com/en/#download");
        assert_eq!(release.pages.zh, "https://elyteaparty.com/#download");
        assert!(
            manifest("0.3.15", "http://example.com/x.dmg").is_err(),
            "only https"
        );
        assert!(
            manifest("v0.3.15", "https://example.com/x.dmg").is_err(),
            "a bare version"
        );
    }

    #[test]
    fn a_file_hashes_as_sha256_says() {
        let file = std::env::temp_dir().join(format!("ely-update-sha-{}", std::process::id()));
        std::fs::write(&file, "abc").expect("writes");
        assert_eq!(
            sha256(&file).expect("hashes"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        std::fs::remove_file(&file).expect("removes");
    }
}
