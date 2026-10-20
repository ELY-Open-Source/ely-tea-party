//! Only the arguments and config-home variables needed to identify a local agent.

use std::path::PathBuf;

pub(crate) struct Process {
    pub argv: Vec<String>,
    pub home: Option<PathBuf>,
    pub codex_home: Option<PathBuf>,
}

#[cfg(any(unix, test))]
fn from_parts(argv: Vec<String>, environment: &[u8]) -> Process {
    let variable = |key: &[u8]| {
        environment.split(|b| *b == 0).find_map(|entry| {
            let value = entry.strip_prefix(key)?;
            (!value.is_empty())
                .then(|| std::str::from_utf8(value).ok().map(PathBuf::from))
                .flatten()
        })
    };
    Process {
        argv,
        home: variable(b"HOME="),
        codex_home: variable(b"CODEX_HOME="),
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn read(pid: u32) -> Option<Process> {
    let mut mib = [libc::CTL_KERN, libc::KERN_PROCARGS2, pid as libc::c_int];
    let mut size: libc::size_t = 0;
    let none = std::ptr::null_mut();
    if unsafe { libc::sysctl(mib.as_mut_ptr(), 3, none, &mut size, none, 0) } != 0 {
        return None;
    }
    let mut buffer = vec![0u8; size];
    if unsafe {
        libc::sysctl(
            mib.as_mut_ptr(),
            3,
            buffer.as_mut_ptr().cast(),
            &mut size,
            none,
            0,
        )
    } != 0
    {
        return None;
    }
    buffer.truncate(size);
    mac_args(&buffer)
}

#[cfg(any(target_os = "macos", test))]
fn mac_args(buffer: &[u8]) -> Option<Process> {
    let argc = i32::from_ne_bytes(buffer.get(..4)?.try_into().ok()?);
    if !(1..=65536).contains(&argc) {
        return None;
    }
    let rest = &buffer[4..];
    let end = rest.iter().position(|b| *b == 0)?;
    let start = end + rest[end..].iter().position(|b| *b != 0)?;
    let mut rest = &rest[start..];
    let mut argv = Vec::new();
    for _ in 0..argc {
        let end = rest.iter().position(|b| *b == 0)?;
        argv.push(String::from_utf8_lossy(&rest[..end]).into_owned());
        rest = &rest[end + 1..];
    }
    Some(from_parts(argv, rest))
}

#[cfg(target_os = "linux")]
pub(crate) fn read(pid: u32) -> Option<Process> {
    let line = std::fs::read(format!("/proc/{pid}/cmdline")).ok()?;
    let argv = line
        .split(|b| *b == 0)
        .filter(|arg| !arg.is_empty())
        .map(|arg| String::from_utf8_lossy(arg).into_owned())
        .collect();
    let environment = std::fs::read(format!("/proc/{pid}/environ")).unwrap_or_default();
    Some(from_parts(argv, &environment))
}

#[cfg(windows)]
pub(crate) fn read(_: u32) -> Option<Process> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_arguments_and_config_homes_stay_separate() {
        let mut bytes = 4i32.to_ne_bytes().to_vec();
        bytes.extend(b"/bin/codex\0\0codex\0-c\0service_tier=\"fast\"\0\0HOME=/users/a\0CODEX_HOME=/profiles/b\0OTHER=private\0");
        let process = mac_args(&bytes).unwrap();
        assert_eq!(process.argv, ["codex", "-c", "service_tier=\"fast\"", ""]);
        assert_eq!(process.home, Some("/users/a".into()));
        assert_eq!(process.codex_home, Some("/profiles/b".into()));
        assert!(mac_args(&bytes[..15]).is_none());
        assert!(mac_args(&(-1i32).to_ne_bytes()).is_none());
    }
}
