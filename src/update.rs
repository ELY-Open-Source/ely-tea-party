//! Whether GitHub holds a newer release than this build and, on macOS, putting it in this app's place.

use std::time::Duration;

use anyhow::Context as _;
use gpui::{App, Global, Task, actions};
use serde::Deserialize;

#[cfg(target_os = "macos")]
mod macos;

actions!(ely_tea_party, [CheckForUpdates]);

const LATEST: &str = "https://api.github.com/repos/qiz029/ely-tea-party/releases/latest";

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
    /// Its page on GitHub.
    pub page: String,
    /// Its macOS disk image's download.
    pub image: Option<String>,
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

/// The newest release that is neither a draft nor a pre-release; it blocks.
pub fn latest() -> anyhow::Result<Release> {
    let headers = "Accept: application/vnd.github+json\nUser-Agent: ely-tea-party\n";
    let body = crate::curl::get("update", LATEST, headers)?;
    let answer: Answer =
        serde_json::from_slice(&body).context("update: GitHub's answer does not parse")?;
    release(answer)
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
        .map(|asset| asset.browser_download_url);
    Ok(Release {
        version,
        page: answer.html_url,
        image,
    })
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

/// Checks at start and every few hours, and binds Check for Updates.
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
            cx.update(|cx| check(false, cx));
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

/// Asks GitHub for its newest release, unless a check or an install is under way.
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
            cx.open_url(&release.page);
            set(State::Available(release), cx);
        }
        State::Current | State::Unchecked(_) => set(State::Idle, cx),
        State::Idle | State::Installing(_) => {}
    }
}

#[cfg(target_os = "macos")]
fn install(release: Release, cx: &mut App) {
    let Some(bundle) = macos::bundle() else {
        log::info!("update: not a release bundle, so {} opens", release.page);
        cx.open_url(&release.page);
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
    log::info!("update: {} opens", release.page);
    cx.open_url(&release.page);
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
    cx.open_url(&release.page);
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
            r#"{"tag_name":"v0.3.4","html_url":"https://github.com/qiz029/ely-tea-party/releases/tag/v0.3.4","draft":false,"assets":[
                {"name":"ely-tea-party-0.3.4-linux-x86_64.tar.gz","browser_download_url":"https://example.com/linux"},
                {"name":"ely-tea-party-0.3.4.dmg","browser_download_url":"https://example.com/dmg"}
            ]}"#,
        )
        .expect("parses");
        let release = release(answer).expect("a release");
        assert_eq!(release.version, "0.3.4");
        assert_eq!(release.image.as_deref(), Some("https://example.com/dmg"));
        let bare: Answer =
            serde_json::from_str(r#"{"tag_name":"0.3.4","html_url":"x"}"#).expect("parses");
        assert!(super::release(bare).is_err());
    }
}
