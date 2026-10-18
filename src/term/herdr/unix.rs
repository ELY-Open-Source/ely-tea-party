use std::{
    collections::HashMap,
    ffi::OsString,
    io,
    os::unix::{ffi::OsStringExt, net::UnixStream},
    path::{Path, PathBuf},
};

use anyhow::Context as _;

use super::{Invocation, WAIT, follows};
use crate::term::Pty;

pub(super) type Pid = i32;

/// The foreground's group and its leader's executable: an exec keeps the group and swaps the program.
pub(super) type Program = (Pid, Option<PathBuf>);

/// herdr's executable.
const HERDR: &str = "herdr";

/// The process group leading the session's foreground.
pub(super) fn leader(pty: &Pty) -> io::Result<Pid> {
    pty.foreground()
}

pub(super) fn running(group: Pid) -> io::Result<Program> {
    Ok((group, path(group).ok()))
}

/// The socket of the herdr server whose terminal client leads process group `group`, if it is one.
pub(super) fn client((group, exe): Program) -> anyhow::Result<Option<PathBuf>> {
    let Some(exe) = exe else {
        log::debug!("herdr: process {group}, another user's or gone, does not read");
        return Ok(None);
    };
    if !exe
        .file_name()
        .is_some_and(|name| name.eq_ignore_ascii_case(HERDR))
    {
        return Ok(None);
    }
    let (args, env) =
        launch(group).with_context(|| format!("herdr: process {group} does not read"))?;
    let Some(socket) = follows(&args, &env)? else {
        return Ok(None);
    };
    if socket.is_absolute() {
        return Ok(Some(socket));
    }
    // herdr reads a relative socket from its client's folder.
    let folder =
        cwd(group).with_context(|| format!("herdr: process {group}'s folder does not read"))?;
    Ok(Some(folder.join(socket)))
}

/// herdr's folder without `XDG_CONFIG_HOME`.
pub(super) fn home(env: &HashMap<String, OsString>) -> anyhow::Result<PathBuf> {
    let home = env.get("HOME").context("herdr: its client has no HOME")?;
    Ok(PathBuf::from(home).join(".config/herdr"))
}

/// herdr's socket, which must answer within `WAIT`.
pub(super) fn connect(socket: &Path) -> io::Result<UnixStream> {
    let stream = UnixStream::connect(socket)?;
    stream.set_read_timeout(Some(WAIT))?;
    stream.set_write_timeout(Some(WAIT))?;
    Ok(stream)
}

#[cfg(target_os = "linux")]
pub(super) fn path(pid: Pid) -> io::Result<PathBuf> {
    std::fs::read_link(format!("/proc/{pid}/exe")).map(installed)
}

/// The path of an executable the kernel marks ` (deleted)`, as an update that replaced it leaves it.
#[cfg(target_os = "linux")]
pub(super) fn installed(path: PathBuf) -> PathBuf {
    use std::{ffi::OsStr, os::unix::ffi::OsStrExt};
    match path.as_os_str().as_bytes().strip_suffix(b" (deleted)") {
        Some(kept) => PathBuf::from(OsStr::from_bytes(kept)),
        None => path,
    }
}

#[cfg(target_os = "linux")]
pub(super) fn launch(pid: Pid) -> io::Result<Invocation> {
    let read = |name: &str| std::fs::read(format!("/proc/{pid}/{name}"));
    let (args, env) = (read("cmdline")?, read("environ")?);
    let args = strings(&args).map(lossy).collect();
    let env = strings(&env).filter_map(variable).collect();
    Ok((args, env))
}

#[cfg(target_os = "linux")]
pub(super) fn cwd(pid: Pid) -> io::Result<PathBuf> {
    std::fs::read_link(format!("/proc/{pid}/cwd"))
}

/// Each NUL-ended string in `bytes`.
#[cfg(target_os = "linux")]
fn strings(bytes: &[u8]) -> impl Iterator<Item = &[u8]> {
    bytes
        .strip_suffix(b"\0")
        .unwrap_or(bytes)
        .split(|byte| *byte == 0)
}

#[cfg(target_os = "macos")]
pub(super) fn path(pid: Pid) -> io::Result<PathBuf> {
    let mut buffer = vec![0u8; libc::PROC_PIDPATHINFO_MAXSIZE as usize];
    // SAFETY: the buffer holds the size given.
    let length =
        unsafe { libc::proc_pidpath(pid, buffer.as_mut_ptr().cast(), buffer.len() as u32) };
    if length <= 0 {
        return Err(io::Error::last_os_error());
    }
    buffer.truncate(length as usize);
    Ok(PathBuf::from(OsString::from_vec(buffer)))
}

#[cfg(target_os = "macos")]
pub(super) fn launch(pid: Pid) -> io::Result<Invocation> {
    let mut name = [libc::CTL_KERN, libc::KERN_PROCARGS2, pid];
    let mut ask = |buffer: &mut Vec<u8>| {
        let mut size = buffer.len();
        let into = if buffer.is_empty() {
            std::ptr::null_mut()
        } else {
            buffer.as_mut_ptr().cast()
        };
        // SAFETY: `into` holds `size` bytes, or is null to ask for the size.
        let done = unsafe {
            libc::sysctl(
                name.as_mut_ptr(),
                3,
                into,
                &mut size,
                std::ptr::null_mut(),
                0,
            )
        };
        if done != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(size)
    };
    let mut buffer = Vec::new();
    let size = ask(&mut buffer)?;
    buffer.resize(size, 0);
    let size = ask(&mut buffer)?;
    buffer.truncate(size);
    procargs(&buffer)
}

#[cfg(target_os = "macos")]
pub(super) fn cwd(pid: Pid) -> io::Result<PathBuf> {
    // SAFETY: the struct is plain C data, valid zeroed.
    let mut info: libc::proc_vnodepathinfo = unsafe { std::mem::zeroed() };
    let size = std::mem::size_of::<libc::proc_vnodepathinfo>() as libc::c_int;
    // SAFETY: `info` holds the size given.
    let read = unsafe {
        libc::proc_pidinfo(
            pid,
            libc::PROC_PIDVNODEPATHINFO,
            0,
            (&raw mut info).cast(),
            size,
        )
    };
    if read <= 0 {
        return Err(io::Error::last_os_error());
    }
    if read != size {
        return Err(io::Error::other(format!(
            "proc_pidinfo read {read} of {size} bytes"
        )));
    }
    let path = info
        .pvi_cdir
        .vip_path
        .iter()
        .flatten()
        .map(|byte| *byte as u8);
    Ok(PathBuf::from(OsString::from_vec(
        path.take_while(|byte| *byte != 0).collect(),
    )))
}

/// `KERN_PROCARGS2`'s layout: argc, the program's path, NUL padding, the arguments, the variables.
#[cfg(target_os = "macos")]
pub(super) fn procargs(buffer: &[u8]) -> io::Result<Invocation> {
    let invalid =
        |what: &str| io::Error::new(io::ErrorKind::InvalidData, format!("KERN_PROCARGS2 {what}"));
    let (count, rest) = buffer
        .split_first_chunk::<4>()
        .ok_or_else(|| invalid("holds no argument count"))?;
    let count = usize::try_from(i32::from_ne_bytes(*count))
        .map_err(|_| invalid("holds a negative argument count"))?;
    let mut parts = rest
        .split(|byte| *byte == 0)
        .skip(1)
        .skip_while(|part| part.is_empty());
    let args = parts.by_ref().take(count).map(lossy).collect();
    let env = parts
        .take_while(|part| !part.is_empty())
        .filter_map(variable)
        .collect();
    Ok((args, env))
}

fn lossy(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// A `NAME=value` variable; its value keeps its bytes.
fn variable(pair: &[u8]) -> Option<(String, OsString)> {
    let at = pair.iter().position(|byte| *byte == b'=')?;
    Some((
        lossy(&pair[..at]),
        OsString::from_vec(pair[at + 1..].to_vec()),
    ))
}
