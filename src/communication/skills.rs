use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use anyhow::{Context, bail, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::protocol::{APP_VERSION, SKILL, SKILL_VERSION};

const OWNER: &str = "net.toddzheng.ely-tea-party/session-messaging";
const MANIFEST: &str = ".ely-managed.json";

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Missing,
    Synced,
    UpdateAvailable,
    Modified,
    Failed,
}

#[derive(Debug, Clone, Serialize)]
pub struct Status {
    pub agent: String,
    pub path: PathBuf,
    pub state: State,
    pub detail: String,
}

#[derive(Serialize, Deserialize)]
struct Manifest {
    owner: String,
    version: u32,
    hash: String,
    pending: Option<String>,
    app_version: String,
}

pub fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub(super) fn home() -> anyhow::Result<PathBuf> {
    if cfg!(debug_assertions)
        && let Some(path) = std::env::var_os("ELY_TEST_SKILL_HOME")
    {
        let path = PathBuf::from(path);
        ensure!(
            path.is_absolute(),
            "skills: ELY_TEST_SKILL_HOME must be absolute"
        );
        return Ok(path);
    }
    dirs::home_dir().context("skills: no home directory")
}

pub fn targets(home: &Path) -> [(String, PathBuf); 2] {
    [
        (
            "claude".into(),
            home.join(".claude/skills/ely-session-messaging"),
        ),
        (
            "codex".into(),
            home.join(".agents/skills/ely-session-messaging"),
        ),
    ]
}

fn regular(path: &Path) -> anyhow::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(meta) => ensure!(
            !meta.file_type().is_symlink(),
            "skills: refusing symlink {}",
            path.display()
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(error).with_context(|| format!("skills: inspect {}", path.display()));
        }
    }
    Ok(())
}

fn read_manifest(dir: &Path) -> anyhow::Result<Option<Manifest>> {
    regular(dir)?;
    let path = dir.join(MANIFEST);
    regular(&path)?;
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error).with_context(|| format!("skills: read {}", path.display()));
        }
    };
    let manifest: Manifest =
        serde_json::from_slice(&bytes).context("skills: invalid ownership manifest")?;
    ensure!(
        manifest.owner == OWNER,
        "skills: directory belongs to another installer"
    );
    ensure!(
        manifest.version <= SKILL_VERSION,
        "skills: installed skill is newer than this app; use the newer app"
    );
    Ok(Some(manifest))
}

fn contents(dir: &Path) -> anyhow::Result<Option<Vec<u8>>> {
    let path = dir.join("SKILL.md");
    regular(&path)?;
    match fs::read(&path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).with_context(|| format!("skills: read {}", path.display())),
    }
}

fn inspect(dir: &Path) -> anyhow::Result<(State, String)> {
    let manifest = read_manifest(dir)?;
    let bytes = contents(dir)?;
    let Some(manifest) = manifest else {
        return Ok(if bytes.is_some() {
            (
                State::Modified,
                "An unmanaged skill occupies this path; it was preserved.".into(),
            )
        } else {
            (State::Missing, "Not installed".into())
        });
    };
    let Some(bytes) = bytes else {
        return Ok((State::UpdateAvailable, "Managed skill is missing".into()));
    };
    let actual = hash(&bytes);
    if actual != manifest.hash && manifest.pending.as_ref() != Some(&actual) {
        return Ok((State::Modified, "User changes preserved; restore the managed version or move this skill before installing.".into()));
    }
    if bytes == SKILL.as_bytes() && manifest.version == SKILL_VERSION && manifest.pending.is_none()
    {
        Ok((State::Synced, format!("Skill {SKILL_VERSION} · verified")))
    } else {
        Ok((State::UpdateAvailable, "Managed update available".into()))
    }
}

pub fn status(agent: &str, dir: &Path) -> Status {
    let (state, detail) =
        inspect(dir).unwrap_or_else(|error| (State::Failed, format!("{error:#}")));
    Status {
        agent: agent.into(),
        path: dir.to_path_buf(),
        state,
        detail,
    }
}

fn atomic(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    let tmp = path.with_file_name(format!(".ely-write-{}", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut file = OpenOptions::new().write(true).create_new(true).open(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(&tmp, path)?;
        #[cfg(unix)]
        File::open(
            path.parent()
                .context("skills: destination has no directory")?,
        )?
        .sync_all()?;
        ensure!(
            fs::read(path)? == bytes,
            "skills: read-back verification failed for {}",
            path.display()
        );
        Ok(())
    })();
    if tmp.exists() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

fn lock(dir: &Path) -> anyhow::Result<File> {
    regular(dir)?;
    fs::create_dir_all(dir).with_context(|| format!("skills: create {}", dir.display()))?;
    let path = dir.join(".ely-install.lock");
    regular(&path)?;
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)?;
    file.try_lock()
        .context("skills: another installer is updating this skill; try again")?;
    Ok(file)
}

pub fn install(dir: &Path) -> anyhow::Result<()> {
    let _lock = lock(dir)?;
    let (state, detail) = inspect(dir)?;
    ensure!(state != State::Modified, "skills: {detail}");
    if state == State::Synced {
        return Ok(());
    }
    let old = read_manifest(dir)?;
    let wanted = hash(SKILL.as_bytes());
    let mut manifest = Manifest {
        owner: OWNER.into(),
        version: SKILL_VERSION,
        hash: old.map_or_else(|| wanted.clone(), |old| old.hash),
        pending: Some(wanted.clone()),
        app_version: APP_VERSION.into(),
    };
    atomic(&dir.join(MANIFEST), &serde_json::to_vec_pretty(&manifest)?)?;
    atomic(&dir.join("SKILL.md"), SKILL.as_bytes())?;
    manifest.hash = wanted;
    manifest.pending = None;
    atomic(&dir.join(MANIFEST), &serde_json::to_vec_pretty(&manifest)?)?;
    ensure!(
        inspect(dir)?.0 == State::Synced,
        "skills: installation did not verify"
    );
    log::info!(
        "skills: installed version {SKILL_VERSION} in {}",
        dir.display()
    );
    Ok(())
}

pub fn uninstall(dir: &Path) -> anyhow::Result<()> {
    let _lock = lock(dir)?;
    ensure!(
        read_manifest(dir)?.is_some(),
        "skills: no managed installation here"
    );
    let (state, detail) = inspect(dir)?;
    ensure!(state != State::Modified, "skills: {detail}");
    if dir.join("SKILL.md").exists() {
        fs::remove_file(dir.join("SKILL.md"))?;
    }
    fs::remove_file(dir.join(MANIFEST))?;
    log::info!("skills: removed managed entry in {}", dir.display());
    Ok(())
}

pub fn refresh(home: &Path) -> Vec<Status> {
    targets(home)
        .into_iter()
        .map(|(agent, path)| {
            let before = status(&agent, &path);
            if before.state == State::UpdateAvailable
                && let Err(error) = install(&path)
            {
                log::error!("skills: {agent} update failed: {error:#}");
                return Status {
                    state: State::Failed,
                    detail: format!("{error:#}"),
                    ..before
                };
            }
            status(&agent, &path)
        })
        .collect()
}

pub fn cli(args: &[String]) -> anyhow::Result<serde_json::Value> {
    let home = home()?;
    if args == ["status"] || args == ["sync"] {
        let statuses = if args[0] == "sync" {
            refresh(&home)
        } else {
            targets(&home)
                .iter()
                .map(|(agent, path)| status(agent, path))
                .collect()
        };
        let output = serde_json::to_value(&statuses)?;
        if args[0] == "sync"
            && statuses
                .iter()
                .any(|s| matches!(s.state, State::Modified | State::Failed))
        {
            bail!("skills: sync incomplete: {output}");
        }
        return Ok(output);
    }
    if let [action, agent] = args {
        let (_, path) = targets(&home)
            .into_iter()
            .find(|(name, _)| name == agent)
            .context("skills: agent must be claude or codex")?;
        match action.as_str() {
            "install" => install(&path)?,
            "uninstall" => uninstall(&path)?,
            _ => bail!("skills: expected install, uninstall, status or sync"),
        }
        return Ok(serde_json::to_value(status(agent, &path))?);
    }
    bail!("usage: ely-tea-party skills install|uninstall claude|codex; skills status|sync")
}

#[cfg(test)]
mod tests;
