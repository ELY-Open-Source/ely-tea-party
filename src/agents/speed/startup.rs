//! A one-time estimate from this Codex process's launch configuration.

use std::{
    io::Read,
    path::{Path, PathBuf},
};
use toml_edit::DocumentMut;

use super::Speed;
use crate::agents::{
    Kind,
    detect::{classify, process::Process},
};

#[derive(Default)]
struct Options {
    overrides: Vec<(String, String)>,
    profile: Option<String>,
    cwd: Option<PathBuf>,
}

fn options(argv: &[String]) -> Option<Options> {
    let mut options = Options::default();
    let mut args = argv.iter().skip(1);
    while let Some(arg) = args.next() {
        let short = ["-c", "-p", "-C"].into_iter().find_map(|key| {
            arg.strip_prefix(key)
                .filter(|tail| !tail.is_empty())
                .map(|tail| (key, Some(tail.strip_prefix('=').unwrap_or(tail))))
        });
        let (key, inline) = short.unwrap_or_else(|| {
            arg.split_once('=')
                .map_or((arg.as_str(), None), |(k, v)| (k, Some(v)))
        });
        match key {
            "--remote" | "--remote-auth-token-env" | "resume" | "fork" | "exec" | "review" => {
                return None;
            }
            "-c" | "--config" => {
                let value = inline.or_else(|| args.next().map(String::as_str))?;
                let (key, value) = value.split_once('=')?;
                options
                    .overrides
                    .push((key.trim().into(), value.trim().into()));
            }
            "-p" | "--profile" => {
                options.profile = Some(inline.or_else(|| args.next().map(String::as_str))?.into())
            }
            "-C" | "--cd" => {
                options.cwd = Some(inline.or_else(|| args.next().map(String::as_str))?.into())
            }
            "--disable" | "--enable" => {
                let name = inline.or_else(|| args.next().map(String::as_str))?;
                options
                    .overrides
                    .push((format!("features.{name}"), (key == "--enable").to_string()));
            }
            "--" => break,
            _ => {}
        }
    }
    Some(options)
}

fn read(path: &Path) -> anyhow::Result<DocumentMut> {
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(DocumentMut::new()),
        Err(e) => return Err(e.into()),
    };
    let mut text = String::new();
    file.take(512 * 1024 + 1).read_to_string(&mut text)?;
    anyhow::ensure!(text.len() <= 512 * 1024, "config is larger than 512 KiB");
    text.parse()
        .map_err(|_| anyhow::anyhow!("config is not valid TOML"))
}

#[derive(Default)]
struct Config {
    tier: Option<String>,
    enabled: Option<bool>,
}

impl Config {
    fn layer(&mut self, doc: &DocumentMut) -> anyhow::Result<()> {
        if let Some(tier) = doc.get("service_tier") {
            self.tier = Some(
                tier.as_str()
                    .ok_or_else(|| anyhow::anyhow!("invalid service_tier"))?
                    .into(),
            );
        }
        if let Some(enabled) = doc.get("features").and_then(|f| f.get("fast_mode")) {
            self.enabled = Some(
                enabled
                    .as_bool()
                    .ok_or_else(|| anyhow::anyhow!("invalid fast_mode"))?,
            );
        }
        Ok(())
    }
}

pub(super) fn resolve(process: &Process, cwd: &Path) -> anyhow::Result<Option<Speed>> {
    if classify(&process.argv) != Some(Kind::Gpt) {
        return Ok(None);
    }
    let Some(options) = options(&process.argv) else {
        return Ok(None);
    };
    let Some(home) = process
        .codex_home
        .clone()
        .or_else(|| process.home.as_ref().map(|h| h.join(".codex")))
    else {
        return Ok(None);
    };
    if !home.is_absolute() {
        return Ok(None);
    }
    let cwd = options.cwd.as_ref().map_or_else(
        || cwd.to_path_buf(),
        |dir| {
            if dir.is_absolute() {
                dir.clone()
            } else {
                cwd.join(dir)
            }
        },
    );
    let user = read(&home.join("config.toml"))?;
    let mut config = Config::default();
    config.layer(&user)?;
    let profile = options
        .profile
        .or_else(|| {
            options
                .overrides
                .iter()
                .rev()
                .find(|(k, _)| k == "profile")
                .map(|(_, v)| scalar(v))
        })
        .or_else(|| {
            user.get("profile")
                .and_then(|p| p.as_str())
                .map(str::to_owned)
        });
    if let Some(profile) = profile {
        if profile.is_empty() || profile.contains(['/', '\\']) || profile == ".." {
            return Ok(None);
        }
        if user.get("profiles").and_then(|p| p.get(&profile)).is_some() {
            return Ok(None);
        }
        config.layer(&read(&home.join(format!("{profile}.config.toml")))?)?;
    }
    let mut parents = Vec::new();
    for dir in cwd.ancestors() {
        parents.push(dir);
        if dir.join(".git").exists() {
            break;
        }
    }
    let trust = parents.iter().find_map(|dir| {
        user.get("projects")?
            .get(dir.to_str()?)?
            .get("trust_level")?
            .as_str()
    });
    for dir in parents.into_iter().rev() {
        let path = dir.join(".codex/config.toml");
        if path == home.join("config.toml") {
            continue;
        }
        let local = read(&path)?;
        let relevant = local.get("service_tier").is_some()
            || local
                .get("features")
                .and_then(|f| f.get("fast_mode"))
                .is_some();
        match trust {
            Some("trusted") => config.layer(&local)?,
            Some("untrusted") => {}
            _ if relevant => return Ok(None),
            _ => {}
        }
    }
    for (key, value) in options.overrides {
        match key.as_str() {
            "service_tier" => config.tier = Some(scalar(&value)),
            "features.fast_mode" => config.enabled = value.parse::<bool>().ok(),
            _ => {}
        }
    }
    if config.enabled == Some(false) {
        return Ok(None);
    }
    Ok(config.tier.as_deref().map(Speed::tier))
}

fn scalar(text: &str) -> String {
    format!("value = {text}")
        .parse::<DocumentMut>()
        .ok()
        .and_then(|doc| doc["value"].as_str().map(str::to_owned))
        .unwrap_or_else(|| text.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn fixture(run: impl FnOnce(Process, &Path, &Path)) {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "ely-speed-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let home = root.join("profile-home");
        let cwd = root.join("project");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(cwd.join(".git")).unwrap();
        let process = Process {
            argv: vec!["codex".into()],
            home: None,
            codex_home: Some(home.clone()),
        };
        run(process, &home, &cwd);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn launch_defaults_work_without_titles_and_profiles_and_cli_win() {
        fixture(|mut process, home, cwd| {
            std::fs::write(
                home.join("config.toml"),
                "service_tier = 'ultrafast'\n[tui]\nterminal_title = []\n",
            )
            .unwrap();
            assert_eq!(resolve(&process, cwd).unwrap(), Some(Speed::Ultrafast));
            std::fs::write(home.join("quick.config.toml"), "service_tier = 'fast'\n").unwrap();
            process.argv.extend(["--profile=quick".into()]);
            assert_eq!(resolve(&process, cwd).unwrap(), Some(Speed::Fast));
            process
                .argv
                .extend(["-c".into(), "service_tier='default'".into()]);
            assert_eq!(resolve(&process, cwd).unwrap(), Some(Speed::Standard));
            process
                .argv
                .extend(["--config=service_tier=ultrafast".into()]);
            assert_eq!(resolve(&process, cwd).unwrap(), Some(Speed::Ultrafast));
            process.argv.push("-cservice_tier=fast".into());
            assert_eq!(resolve(&process, cwd).unwrap(), Some(Speed::Fast));
        });
    }

    #[test]
    fn remote_and_resumed_threads_do_not_inherit_local_defaults() {
        fixture(|mut process, home, cwd| {
            std::fs::write(home.join("config.toml"), "service_tier = 'fast'\n").unwrap();
            for args in [
                ["--remote", "unix://"],
                ["resume", "--last"],
                ["fork", "--last"],
            ] {
                process.argv = ["codex"]
                    .into_iter()
                    .chain(args)
                    .map(str::to_owned)
                    .collect();
                assert_eq!(resolve(&process, cwd).unwrap(), None);
            }
        });
    }

    #[test]
    fn project_tiers_require_known_trust_and_keep_cli_precedence() {
        fixture(|mut process, home, cwd| {
            std::fs::create_dir(cwd.join(".codex")).unwrap();
            std::fs::write(cwd.join(".codex/config.toml"), "service_tier = 'ultrafast'").unwrap();
            std::fs::write(home.join("config.toml"), "service_tier = 'fast'").unwrap();
            assert_eq!(resolve(&process, cwd).unwrap(), None);
            let user = format!(
                "service_tier = 'fast'\n[projects.'{}']\ntrust_level = 'trusted'\n",
                cwd.display()
            );
            std::fs::write(home.join("config.toml"), &user).unwrap();
            assert_eq!(resolve(&process, cwd).unwrap(), Some(Speed::Ultrafast));
            process
                .argv
                .extend(["-c".into(), "service_tier=default".into()]);
            assert_eq!(resolve(&process, cwd).unwrap(), Some(Speed::Standard));
            process.argv.truncate(1);
            std::fs::write(
                home.join("config.toml"),
                user.replace("'trusted'", "'untrusted'"),
            )
            .unwrap();
            assert_eq!(resolve(&process, cwd).unwrap(), Some(Speed::Fast));
        });
    }

    #[cfg(unix)]
    #[test]
    #[ignore = "child fixture launched by startup_reads_the_childs_home_once_and_live_reports_win"]
    fn child_process_fixture() {
        std::thread::sleep(std::time::Duration::from_secs(20));
    }

    #[cfg(unix)]
    #[test]
    fn startup_reads_the_childs_home_once_and_live_reports_win() {
        fixture(|_, home, cwd| {
            let executable = home.join("codex");
            std::os::unix::fs::symlink(std::env::current_exe().unwrap(), &executable).unwrap();
            std::fs::write(home.join("config.toml"), "service_tier = 'ultrafast'").unwrap();
            let mut child = std::process::Command::new(&executable)
                .args([
                    "--exact",
                    "agents::speed::startup::tests::child_process_fixture",
                    "--ignored",
                ])
                .env("CODEX_HOME", home)
                .stdout(std::process::Stdio::null())
                .spawn()
                .unwrap();
            let mut agent = crate::agents::Agent::new(Kind::Gpt, std::time::Instant::now());
            agent.startup_speed(child.id(), cwd);
            let initial = (agent.speed, agent.speed_inferred);
            std::fs::write(home.join("config.toml"), "service_tier = 'fast'").unwrap();
            agent.startup_speed(child.id(), cwd);
            let unchanged = agent.speed;
            agent.reported_speed(Speed::Standard);
            agent.startup_speed(child.id(), cwd);
            let reported = (agent.speed, agent.speed_inferred);
            let _ = child.kill();
            child.wait().unwrap();
            assert_eq!(initial, (Speed::Ultrafast, true));
            assert_eq!(unchanged, Speed::Ultrafast);
            assert_eq!(reported, (Speed::Standard, false));
        });
    }
}
