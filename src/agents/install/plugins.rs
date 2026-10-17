//! Agents that take plugins, not hook commands: OpenCode, Pi and oh-my-pi each load one file of Ely Tea Party's.

use std::path::{Path, PathBuf};

use super::{Installed, MARK, files, folder};

const OPENCODE: &str = include_str!("../../../assets/hooks/opencode.js");
const PI: &str = include_str!("../../../assets/hooks/pi.ts");

pub fn opencode(home: &Path) -> anyhow::Result<Installed> {
    let xdg = folder("XDG_CONFIG_HOME", home.join(".config")).join("opencode");
    let config = folder("OPENCODE_CONFIG_DIR", xdg);
    place(config.join("plugins").join(format!("{MARK}.js")), OPENCODE)
}

pub fn pi(home: &Path) -> anyhow::Result<Installed> {
    let agent = folder("PI_CODING_AGENT_DIR", home.join(".pi/agent"));
    place(extension(&agent), &PI.replace("__AGENT__", "pi"))
}

pub fn omp(home: &Path) -> anyhow::Result<Installed> {
    let agent = folder("OMP_AGENT_DIR", home.join(".omp/agent"));
    place(extension(&agent), &PI.replace("__AGENT__", "omp"))
}

fn extension(agent: &Path) -> PathBuf {
    agent.join("extensions").join(format!("{MARK}.ts"))
}

/// Writes `source` at `path`, unless a file there is someone else's.
fn place(path: PathBuf, source: &str) -> anyhow::Result<Installed> {
    let old = files::read(&path)?;
    if let Some(old) = &old {
        anyhow::ensure!(
            old.starts_with(&format!("// {MARK}:")),
            "{} is not Ely Tea Party's; move it away to install",
            path.display()
        );
    }
    let wrote = files::write(&path, old.as_deref(), source)?;
    Ok(Installed {
        path,
        added: usize::from(wrote),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plugin_is_ours_or_left_alone() {
        let home = std::env::temp_dir().join(format!("ely-plugins-{}", std::process::id()));
        let path = home.join("x/ely-tea-party.ts");
        let source = PI.replace("__AGENT__", "omp");
        assert_eq!(place(path.clone(), &source).expect("placed").added, 1);
        assert_eq!(place(path.clone(), &source).expect("again").added, 0);
        std::fs::write(&path, "// mine\n").expect("written");
        assert!(place(path, &source).is_err(), "someone else's file");
        assert!(!source.contains("__AGENT__"));
        assert!(OPENCODE.starts_with(&format!("// {MARK}:")));
        std::fs::remove_dir_all(&home).expect("cleaned");
    }
}
