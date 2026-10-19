//! HTTPS through `curl`, which ships with macOS and Windows 10: the app has no HTTP client.

use std::{
    io::Write,
    path::Path,
    process::{Command, Stdio},
};

use anyhow::Context as _;

fn curl() -> Command {
    let mut command = Command::new("curl");
    #[cfg(windows)]
    std::os::windows::process::CommandExt::creation_flags(&mut command, 0x0800_0000);
    command.arg("-sSL");
    command
}

/// `url`'s body, for `owner`; `headers`, one per line, go on curl's stdin so none shows in a process list.
pub fn get(owner: &str, url: &str, headers: &str) -> anyhow::Result<Vec<u8>> {
    let mut child = curl()
        .args(["--max-time", "15", "-w", "\n%{http_code}", "-H", "@-", url])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("{owner}: cannot run curl"))?;
    child
        .stdin
        .take()
        .with_context(|| format!("{owner}: curl has no stdin"))?
        .write_all(headers.as_bytes())?;
    let output = child.wait_with_output()?;
    anyhow::ensure!(
        output.status.success(),
        "{owner}: curl failed: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    );
    let text = output.stdout;
    let cut = text
        .iter()
        .rposition(|&b| b == b'\n')
        .with_context(|| format!("{owner}: curl gave no status"))?;
    let status = String::from_utf8_lossy(&text[cut + 1..]).trim().to_string();
    anyhow::ensure!(
        status == "200",
        "{owner}: {} answered HTTP {status}",
        host(url)
    );
    Ok(text[..cut].to_vec())
}

/// Saves `url` to `to`, for `owner`, failing on an HTTP error.
pub fn download(owner: &str, url: &str, to: &Path) -> anyhow::Result<()> {
    let output = curl()
        .args(["-f", "--max-time", "900", "-o"])
        .arg(to)
        .arg(url)
        .stdin(Stdio::null())
        .output()
        .with_context(|| format!("{owner}: cannot run curl"))?;
    anyhow::ensure!(
        output.status.success(),
        "{owner}: the download from {} failed: {}",
        host(url),
        String::from_utf8_lossy(&output.stderr).trim()
    );
    Ok(())
}

fn host(url: &str) -> &str {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    rest.split('/').next().unwrap_or(rest)
}
