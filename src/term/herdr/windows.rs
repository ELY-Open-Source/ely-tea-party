use std::{
    collections::HashMap,
    ffi::{OsStr, OsString},
    fs::{File, OpenOptions},
    io,
    os::windows::{
        ffi::{OsStrExt, OsStringExt},
        fs::OpenOptionsExt,
        io::{AsRawHandle, FromRawHandle, OwnedHandle},
    },
    path::{Path, PathBuf},
};

use anyhow::Context as _;
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
use windows_sys::Win32::{
    Foundation::{ERROR_NO_MORE_FILES, ERROR_PIPE_BUSY, INVALID_HANDLE_VALUE},
    Storage::FileSystem::SECURITY_IDENTIFICATION,
    System::{
        Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
            TH32CS_SNAPPROCESS,
        },
        Pipes::WaitNamedPipeW,
    },
};

use super::{Invocation, WAIT, follows};
use crate::term::Pty;

pub(super) type Pid = u32;

/// The herdr processes at or under the session's shell, nearest first.
pub(super) type Program = Vec<Pid>;

/// herdr's executable.
const HERDR: &str = "herdr.exe";

/// The session's shell: Windows has no foreground group, so herdr is sought under it.
pub(super) fn leader(pty: &Pty) -> io::Result<Pid> {
    Ok(pty.pid)
}

pub(super) fn running(shell: Pid) -> io::Result<Program> {
    Ok(herdrs(&processes()?, shell))
}

/// The herdr processes at or under `root`, breadth first; what a herdr starts, its server among them, is its own.
pub(super) fn herdrs(processes: &[(Pid, Pid, String)], root: Pid) -> Vec<Pid> {
    let herdr = |pid: Pid| {
        processes
            .iter()
            .any(|(id, _, name)| *id == pid && name.eq_ignore_ascii_case(HERDR))
    };
    let (mut under, mut found, mut at) = (vec![root], Vec::new(), 0);
    while let Some(&pid) = under.get(at) {
        at += 1;
        if herdr(pid) {
            found.push(pid);
            continue;
        }
        for (child, ..) in processes.iter().filter(|(_, of, _)| *of == pid) {
            // A parent id outlives its process, so a stale one may lead back up.
            if !under.contains(child) {
                under.push(*child);
            }
        }
    }
    found
}

/// Every process: its id, its parent's, and its executable's name.
pub(super) fn processes() -> io::Result<Vec<(Pid, Pid, String)>> {
    // SAFETY: a plain call; its handle is owned below.
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: the handle is new and ours.
    let snapshot = unsafe { OwnedHandle::from_raw_handle(snapshot) };
    let mut entry = PROCESSENTRY32W {
        dwSize: size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    let mut processes = Vec::new();
    // SAFETY: `entry` holds the size it names.
    let mut read = unsafe { Process32FirstW(snapshot.as_raw_handle(), &mut entry) };
    while read != 0 {
        let name = entry.szExeFile.split(|unit| *unit == 0).next();
        processes.push((
            entry.th32ProcessID,
            entry.th32ParentProcessID,
            String::from_utf16_lossy(name.unwrap_or_default()),
        ));
        // SAFETY: as above.
        read = unsafe { Process32NextW(snapshot.as_raw_handle(), &mut entry) };
    }
    let error = io::Error::last_os_error();
    // The list ends with ERROR_NO_MORE_FILES.
    if error.raw_os_error() == Some(ERROR_NO_MORE_FILES as i32) {
        Ok(processes)
    } else {
        Err(error)
    }
}

/// The socket of the herdr server whose terminal client is among `herdrs`; a server or a command is passed by.
pub(super) fn client(herdrs: Program) -> anyhow::Result<Option<PathBuf>> {
    pick(herdrs.into_iter().map(|pid| (pid, launch(pid))))
}

/// The first terminal client's socket in `reads`; with none, a failed read is the error, so it is tried again.
pub(super) fn pick(
    reads: impl IntoIterator<Item = (Pid, io::Result<Invocation>)>,
) -> anyhow::Result<Option<PathBuf>> {
    let mut unread = None;
    for (pid, read) in reads {
        match read.with_context(|| format!("herdr: process {pid} does not read")) {
            // herdr names its pipe after the socket's path as written, relative or not.
            Ok((args, env)) => {
                if let Some(socket) = follows(&args, &env)? {
                    return Ok(Some(socket));
                }
            }
            Err(error) => {
                unread.get_or_insert(error);
            }
        }
    }
    unread.map_or(Ok(None), Err)
}

/// The arguments and variables of process `pid`; names fold to upper case, as Windows matches them.
pub(super) fn launch(pid: Pid) -> io::Result<Invocation> {
    let id = sysinfo::Pid::from_u32(pid);
    let mut system = System::new();
    let read = ProcessRefreshKind::nothing()
        .with_cmd(UpdateKind::Always)
        .with_environ(UpdateKind::Always);
    system.refresh_processes_specifics(ProcessesToUpdate::Some(&[id]), true, read);
    let process = system
        .process(id)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "the process is gone"))?;
    // sysinfo leaves a list empty when it does not read; every process has both.
    if process.cmd().is_empty() || process.environ().is_empty() {
        return Err(io::Error::other(
            "its command line or variables do not read",
        ));
    }
    let args = process
        .cmd()
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    let env = process
        .environ()
        .iter()
        .filter_map(|pair| variable(pair))
        .collect();
    Ok((args, env))
}

/// A `NAME=value` variable, its name in upper case; its value keeps its units.
fn variable(pair: &OsStr) -> Option<(String, OsString)> {
    let pair: Vec<u16> = pair.encode_wide().collect();
    let at = pair.iter().position(|unit| *unit == u16::from(b'='))?;
    Some((
        String::from_utf16_lossy(&pair[..at]).to_ascii_uppercase(),
        OsString::from_wide(&pair[at + 1..]),
    ))
}

/// herdr's folder without `XDG_CONFIG_HOME`, as herdr finds it on Windows.
pub(super) fn home(env: &HashMap<String, OsString>) -> anyhow::Result<PathBuf> {
    if let Some(data) = env.get("APPDATA") {
        return Ok(PathBuf::from(data).join("herdr"));
    }
    if let Some(profile) = env.get("USERPROFILE") {
        return Ok(PathBuf::from(profile).join(r"AppData\Roaming\herdr"));
    }
    let home = env
        .get("HOME")
        .context("herdr: its client has no APPDATA, USERPROFILE or HOME")?;
    Ok(PathBuf::from(home).join(".config/herdr"))
}

/// The pipe herdr names after its socket's path; a busy one is waited for, as herdr's client waits.
pub(super) fn connect(socket: &Path) -> io::Result<File> {
    let pipe = format!(r"\\.\pipe\{}", socket.to_string_lossy());
    // Identification only: the pipe's server may not act as this user.
    let open = || {
        OpenOptions::new()
            .read(true)
            .write(true)
            .security_qos_flags(SECURITY_IDENTIFICATION)
            .open(&pipe)
    };
    match open() {
        Err(error) if error.raw_os_error() == Some(ERROR_PIPE_BUSY as i32) => {
            let name: Vec<u16> = OsStr::new(&pipe).encode_wide().chain([0]).collect();
            // SAFETY: `name` ends with a NUL.
            if unsafe { WaitNamedPipeW(name.as_ptr(), WAIT.as_millis() as u32) } == 0 {
                return Err(io::Error::last_os_error());
            }
            open()
        }
        opened => opened,
    }
}
