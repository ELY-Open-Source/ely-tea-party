use std::{
    env, fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};

use anyhow::Context as _;
use gpui::{App, Global};

use super::pty::Launch;

/// Each script, by its path under the integration folder.
const SCRIPTS: [(&str, &str); 4] = [
    (
        "zsh/.zshenv",
        include_str!("../../assets/shell-integration/zsh/.zshenv"),
    ),
    (
        "bash/ely-tea-party.bash",
        include_str!("../../assets/shell-integration/bash/ely-tea-party.bash"),
    ),
    (
        "fish/vendor_conf.d/ely-tea-party.fish",
        include_str!("../../assets/shell-integration/fish/vendor_conf.d/ely-tea-party.fish"),
    ),
    (
        "pwsh/ely-tea-party.ps1",
        include_str!("../../assets/shell-integration/pwsh/ely-tea-party.ps1"),
    ),
];

/// The folder the scripts live in, written at start.
pub struct ShellIntegration(pub PathBuf);

impl Global for ShellIntegration {}

impl ShellIntegration {
    /// Writes the scripts into the platform's data folder and keeps its path.
    pub fn init(cx: &mut App) -> anyhow::Result<()> {
        let dir = dirs::data_local_dir()
            .context("shell integration: the platform names no data folder")?
            .join("ely-tea-party")
            .join("shell-integration");
        install(&dir)?;
        cx.set_global(Self(dir));
        Ok(())
    }
}

/// Writes the scripts into `dir`, replacing any that differ.
pub fn install(dir: &Path) -> anyhow::Result<()> {
    for (name, text) in SCRIPTS {
        let path = dir.join(name);
        match fs::read_to_string(&path) {
            Ok(kept) if kept == text => continue,
            Ok(_) => {}
            Err(error) if matches!(error.kind(), ErrorKind::NotFound | ErrorKind::InvalidData) => {}
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("shell integration: cannot read {}", path.display()));
            }
        }
        let folder = path
            .parent()
            .context("shell integration: a script has no folder")?;
        fs::create_dir_all(folder)
            .with_context(|| format!("shell integration: cannot create {}", folder.display()))?;
        fs::write(&path, text)
            .with_context(|| format!("shell integration: cannot write {}", path.display()))?;
        log::info!("shell integration: wrote {}", path.display());
    }
    Ok(())
}

/// How to start `shell`, or the login shell when empty, in `cwd` so it reports its folder.
pub fn launch(shell: &str, cwd: PathBuf, dir: &Path) -> anyhow::Result<Launch> {
    let path = if shell.is_empty() {
        default_shell()?
    } else {
        // The settings checked this path; the session runs no other.
        crate::settings::program_path(shell).context("shell integration")?
    };
    let name = Path::new(&path)
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let mut launch = Launch {
        program: (!shell.is_empty()).then(|| (path.clone(), Vec::new())),
        cwd,
        env: Vec::new(),
        reports: true,
    };
    let script = |name: &str| dir.join(name).display().to_string();
    match name.as_str() {
        "zsh" => {
            if let Ok(zdotdir) = env::var("ZDOTDIR") {
                launch.env.push(("ELY_ZSH_ZDOTDIR".into(), zdotdir));
            }
            launch.env.push(("ZDOTDIR".into(), script("zsh")));
        }
        "bash" => {
            let rcfile = script("bash/ely-tea-party.bash");
            launch.program = Some((path.clone(), vec!["--rcfile".into(), rcfile]));
            if cfg!(target_os = "macos") {
                launch.env.push(("ELY_BASH_LOGIN".into(), "1".into()));
            }
        }
        "fish" => {
            let previous = env::var("XDG_DATA_DIRS").unwrap_or_default();
            let rest = if previous.is_empty() {
                "/usr/local/share:/usr/share"
            } else {
                &previous
            };
            let data = format!("{}:{rest}", dir.display());
            launch.env.push(("XDG_DATA_DIRS".into(), data));
            launch.env.push(("ELY_FISH_XDG_DATA_DIRS".into(), previous));
        }
        "pwsh" | "powershell" => {
            let file = script("pwsh/ely-tea-party.ps1").replace('\'', "''");
            launch.program = Some((
                path.clone(),
                vec![
                    "-NoLogo".into(),
                    "-NoExit".into(),
                    "-ExecutionPolicy".into(),
                    "Bypass".into(),
                    "-Command".into(),
                    format!(
                        "try {{ . '{file}' }} catch {{ [Console]::Error.WriteLine(\"Ely Tea Party: shell integration failed: $_\"); exit 1 }}"
                    ),
                ],
            ));
        }
        _ => {
            launch.reports = false;
            log::info!("shell integration: none for {path}; its folder goes unreported")
        }
    }
    log::info!("shell integration: {path} as {name}");
    Ok(launch)
}

#[cfg(unix)]
fn default_shell() -> anyhow::Result<String> {
    Ok(super::unix::login_shell_path()?)
}

#[cfg(windows)]
fn default_shell() -> anyhow::Result<String> {
    Ok("powershell.exe".into())
}

#[cfg(all(test, unix))]
mod tests;
