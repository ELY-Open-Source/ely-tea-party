//! A release's disk image, checked against this app's signer, in this app's bundle's place.

use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use anyhow::Context as _;

use super::Release;
use crate::app::APP_ID;

/// The name the disk image gives the app.
const APP: &str = "Ely Tea Party.app";

/// The bundle this app runs from, when it is a release's, not a dev build's or cargo's.
pub fn bundle() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?.canonicalize().ok()?;
    let bundle = exe.parent()?.parent()?.parent()?;
    if bundle.extension() != Some(OsStr::new("app")) {
        return None;
    }
    let id = plist(bundle, "CFBundleIdentifier").ok()?;
    (id == APP_ID).then(|| bundle.to_path_buf())
}

/// Downloads `release`'s disk image and puts its app in `bundle`'s place, once its hash is the listed one and it is signed as `bundle` is.
pub fn install(release: &Release, bundle: &Path) -> anyhow::Result<()> {
    let image = release
        .image
        .as_ref()
        .with_context(|| format!("update: {} holds no disk image", release.version))?;
    let work = std::env::temp_dir().join(format!("ely-tea-party-update-{}", std::process::id()));
    if work.exists() {
        std::fs::remove_dir_all(&work)
            .with_context(|| format!("update: cannot clear {}", work.display()))?;
    }
    std::fs::create_dir_all(&work)
        .with_context(|| format!("update: cannot make {}", work.display()))?;
    let disk = work.join("update.dmg");
    log::info!("update: downloading {}", image.url);
    crate::curl::download("update", &image.url, &disk)?;
    if let Some(listed) = &image.sha256 {
        let read = super::sha256(&disk)?;
        anyhow::ensure!(
            &read == listed,
            "update: the disk image hashes {read}, the store lists {listed}"
        );
    }
    let mount = work.join("mount");
    run(
        "hdiutil",
        [
            OsStr::new("attach"),
            OsStr::new("-nobrowse"),
            OsStr::new("-readonly"),
            OsStr::new("-noautoopen"),
            OsStr::new("-quiet"),
            OsStr::new("-mountpoint"),
            mount.as_os_str(),
            disk.as_os_str(),
        ],
    )?;
    let new = mount.join(APP);
    let placed = check(&new, bundle, &release.version).and_then(|()| replace(&new, bundle));
    if let Err(error) = run(
        "hdiutil",
        [
            OsStr::new("detach"),
            OsStr::new("-force"),
            mount.as_os_str(),
        ],
    ) {
        log::warn!("{error:#}");
    }
    if let Err(error) = std::fs::remove_dir_all(&work) {
        log::warn!("update: {} stays: {error}", work.display());
    }
    placed?;
    log::info!(
        "update: {} in place at {}",
        release.version,
        bundle.display()
    );
    Ok(())
}

/// Starts `bundle` once this process has quit.
pub fn relaunch(bundle: &Path) -> anyhow::Result<()> {
    let script = r#"while kill -0 "$0" 2>/dev/null; do sleep 0.2; done; exec /usr/bin/open "$1""#;
    Command::new("/bin/sh")
        .arg("-c")
        .arg(script)
        .arg(std::process::id().to_string())
        .arg(bundle)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("update: cannot start the relaunch")?;
    Ok(())
}

/// That `new` is a whole app Gatekeeper accepts, this app's id at `version`, from `current`'s team.
fn check(new: &Path, current: &Path, version: &str) -> anyhow::Result<()> {
    run(
        "codesign",
        [
            OsStr::new("--verify"),
            OsStr::new("--deep"),
            OsStr::new("--strict"),
            new.as_os_str(),
        ],
    )?;
    run(
        "spctl",
        [
            OsStr::new("--assess"),
            OsStr::new("--type"),
            OsStr::new("execute"),
            new.as_os_str(),
        ],
    )?;
    let id = plist(new, "CFBundleIdentifier")?;
    anyhow::ensure!(
        id == APP_ID,
        "update: the disk image holds {id}, not {APP_ID}"
    );
    let shipped = plist(new, "CFBundleShortVersionString")?;
    anyhow::ensure!(
        shipped == version,
        "update: the disk image holds {shipped}, not {version}"
    );
    let ours = team(current)?;
    let theirs = team(new)?;
    anyhow::ensure!(
        theirs == ours,
        "update: the new app is signed by team {theirs}, this one by {ours}"
    );
    Ok(())
}

/// Copies `new` beside `current`, then swaps the two, putting `current` back if the swap fails.
fn replace(new: &Path, current: &Path) -> anyhow::Result<()> {
    let folder = current
        .parent()
        .context("update: the app's bundle has no folder")?;
    let name = current
        .file_name()
        .context("update: the app's bundle has no name")?
        .to_string_lossy();
    let staged = folder.join(format!(".{name}.update"));
    let old = folder.join(format!(".{name}.old"));
    for stale in [&staged, &old] {
        if stale.exists() {
            std::fs::remove_dir_all(stale)
                .with_context(|| format!("update: cannot clear {}", stale.display()))?;
        }
    }
    run("ditto", [new.as_os_str(), staged.as_os_str()])?;
    std::fs::rename(current, &old)
        .with_context(|| format!("update: cannot move {} aside", current.display()))?;
    if let Err(error) = std::fs::rename(&staged, current) {
        std::fs::rename(&old, current)
            .with_context(|| format!("update: cannot put {} back", current.display()))?;
        return Err(error).with_context(|| format!("update: cannot place {}", current.display()));
    }
    if let Err(error) = std::fs::remove_dir_all(&old) {
        log::warn!("update: {} stays: {error}", old.display());
    }
    Ok(())
}

/// The team that signed `app`; an ad hoc signature has none.
fn team(app: &Path) -> anyhow::Result<String> {
    let output = Command::new("codesign")
        .args(["-dv", "--verbose=2"])
        .arg(app)
        .output()
        .context("update: cannot run codesign")?;
    let said = String::from_utf8_lossy(&output.stderr);
    said.lines()
        .find_map(|line| line.strip_prefix("TeamIdentifier="))
        .filter(|team| *team != "not set")
        .map(str::to_string)
        .with_context(|| format!("update: {} is signed by no team", app.display()))
}

fn plist(app: &Path, key: &str) -> anyhow::Result<String> {
    let info = app.join("Contents/Info.plist");
    let output = Command::new("plutil")
        .args(["-extract", key, "raw", "-o", "-"])
        .arg(&info)
        .output()
        .context("update: cannot run plutil")?;
    anyhow::ensure!(
        output.status.success(),
        "update: {} holds no {key}",
        info.display()
    );
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn run<'a>(program: &str, args: impl IntoIterator<Item = &'a OsStr>) -> anyhow::Result<()> {
    let output = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .with_context(|| format!("update: cannot run {program}"))?;
    anyhow::ensure!(
        output.status.success(),
        "update: {program} failed: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    );
    Ok(())
}
