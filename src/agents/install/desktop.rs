//! Runs the existing installer with the login shell's environment, including GUI launches.

use std::{
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom},
    os::unix::{fs::OpenOptionsExt, process::CommandExt},
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

use anyhow::Context as _;

pub struct Report {
    pub success: bool,
    pub output: String,
}

pub fn run() -> anyhow::Result<Report> {
    let executable = std::env::current_exe().context("hooks: cannot locate this app")?;
    let shell = crate::term::shell::default_shell()?;
    execute(&shell, &executable, Duration::from_secs(30))
}

fn execute(shell: &str, executable: &Path, timeout: Duration) -> anyhow::Result<Report> {
    let mut output = output_file()?;
    let mut child = Command::new(shell)
        .args(["-ilc", "exec \"$ELY_HOOK_INSTALLER\" hooks install"])
        .env("ELY_HOOK_INSTALLER", executable)
        .stdin(Stdio::null())
        .stdout(output.try_clone()?)
        .stderr(output.try_clone()?)
        .process_group(0)
        .spawn()
        .with_context(|| format!("hooks: cannot start login shell {shell}"))?;
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) if started.elapsed() < timeout => {
                std::thread::sleep(Duration::from_millis(50));
            }
            waiting => {
                // The shell and its children belong to this private process group.
                unsafe { libc::kill(-(child.id() as i32), libc::SIGKILL) };
                let _ = child.wait();
                break match waiting {
                    Err(error) => Err(anyhow::anyhow!("hooks: cannot wait for installer: {error}")),
                    _ => Err(anyhow::anyhow!(
                        "hooks: installation timed out after {} seconds",
                        timeout.as_secs()
                    )),
                };
            }
        }
    };
    output.seek(SeekFrom::Start(0))?;
    let mut bytes = Vec::new();
    output.take(64 * 1024).read_to_end(&mut bytes)?;
    let mut output = String::from_utf8_lossy(&bytes).trim().to_string();
    let success = match status {
        Ok(status) => {
            if !status.success() {
                output.push_str(&format!("\nhooks: installer exited with {status}"));
            }
            status.success()
        }
        Err(error) => {
            output.push_str(&format!("\n{error:#}"));
            false
        }
    };
    Ok(Report { success, output })
}

fn output_file() -> anyhow::Result<File> {
    let path = std::env::temp_dir().join(format!("ely-hooks-{}", uuid::Uuid::new_v4()));
    let file = OpenOptions::new()
        .create_new(true)
        .read(true)
        .write(true)
        .mode(0o600)
        .open(&path)
        .context("hooks: cannot create installation log")?;
    std::fs::remove_file(path).context("hooks: cannot unlink installation log")?;
    Ok(file)
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use super::*;

    #[test]
    fn shell_environment_quoted_executable_errors_and_timeout_reach_the_report() {
        let dir = std::env::temp_dir().join(format!("ely-hooks-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&dir).unwrap();
        let shell = dir.join("shell");
        std::fs::write(&shell, "#!/bin/sh\n[ \"$1\" = -ilc ] || exit 3\nexport ELY_TEST_SHELL=loaded\nexec /bin/sh -c \"$2\"\n").unwrap();
        std::fs::set_permissions(&shell, std::fs::Permissions::from_mode(0o700)).unwrap();
        let executable = dir.join("Ely Tea Party '$app'");
        let script = "#!/bin/sh\n[ \"$ELY_TEST_SHELL\" = loaded ] || exit 4\n[ \"$1 $2\" = 'hooks install' ] || exit 5\necho 'claude installed'\necho 'codex failed' >&2\nexit 1\n";
        std::fs::write(&executable, script).unwrap();
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();
        let run = |timeout| execute(shell.to_str().unwrap(), &executable, timeout).unwrap();
        let report = run(Duration::from_secs(5));
        assert!(!report.success);
        assert!(
            report.output.contains("claude installed"),
            "{}",
            report.output
        );
        assert!(report.output.contains("codex failed"));
        assert!(report.output.contains("installer exited"));
        std::fs::write(&executable, "#!/bin/sh\necho 'up to date'\n").unwrap();
        let report = run(Duration::from_secs(5));
        assert!(report.success);
        assert_eq!(report.output, "up to date");
        std::fs::write(
            &executable,
            "#!/bin/sh\necho 'starting'\nexec /bin/sleep 10\n",
        )
        .unwrap();
        let started = Instant::now();
        let report = run(Duration::from_millis(150));
        assert!(!report.success);
        assert!(report.output.contains("starting"));
        assert!(report.output.contains("timed out"));
        assert!(started.elapsed() < Duration::from_secs(5));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
