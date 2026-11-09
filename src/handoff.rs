//! `elyteaparty://` links reach one app: gpui hands them over on macOS, a socket or pipe elsewhere.

use futures::channel::mpsc::UnboundedSender;

use crate::skins::market::SCHEME;

/// The market links among this process's arguments.
pub fn links() -> Vec<String> {
    std::env::args()
        .skip(1)
        .filter(|arg| is_link(arg))
        .collect()
}

/// Whether a command-line argument is one of the app's links.
pub fn is_link(arg: &str) -> bool {
    arg.starts_with(&format!("{SCHEME}://"))
}

#[cfg(target_os = "linux")]
mod endpoint {
    use std::{
        io::{BufRead, BufReader, Write},
        os::unix::net::{UnixListener, UnixStream},
        path::PathBuf,
    };

    fn path() -> PathBuf {
        dirs::runtime_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join("ely-tea-party-links.sock")
    }

    pub fn send(links: &[String]) -> anyhow::Result<()> {
        let mut stream = UnixStream::connect(path())?;
        stream.write_all(links.join("\n").as_bytes())?;
        Ok(())
    }

    pub fn serve(each: impl Fn(String) -> bool + Send + 'static) -> anyhow::Result<()> {
        let path = path();
        if UnixStream::connect(&path).is_ok() {
            anyhow::bail!("handoff: another app listens on {}", path.display());
        }
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
        let listener = UnixListener::bind(&path)
            .map_err(|error| anyhow::anyhow!("handoff: cannot open {}: {error}", path.display()))?;
        log::info!("handoff: listening on {}", path.display());
        std::thread::Builder::new()
            .name("handoff".into())
            .spawn(move || {
                for stream in listener.incoming().flatten() {
                    for line in BufReader::new(stream).lines().map_while(Result::ok) {
                        if !each(line) {
                            return;
                        }
                    }
                }
            })?;
        Ok(())
    }
}

#[cfg(windows)]
mod endpoint {
    use std::{
        ffi::OsStr,
        io::{Read, Write},
        os::windows::{
            ffi::OsStrExt,
            io::{AsRawHandle, FromRawHandle},
        },
        ptr,
    };

    use windows_sys::Win32::{
        Foundation::{ERROR_PIPE_CONNECTED, GetLastError, INVALID_HANDLE_VALUE},
        Storage::FileSystem::{FILE_FLAG_FIRST_PIPE_INSTANCE, PIPE_ACCESS_INBOUND},
        System::Pipes::{
            ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, PIPE_READMODE_BYTE,
            PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE, PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
        },
    };

    fn name() -> String {
        let user = std::env::var("USERNAME").unwrap_or_default();
        format!(r"\\.\pipe\ely-tea-party-links-{user}")
    }

    pub fn send(links: &[String]) -> anyhow::Result<()> {
        let mut pipe = std::fs::OpenOptions::new().write(true).open(name())?;
        pipe.write_all(links.join("\n").as_bytes())?;
        Ok(())
    }

    fn create(name: &[u16], first: bool) -> anyhow::Result<isize> {
        let flags = PIPE_ACCESS_INBOUND
            | if first {
                FILE_FLAG_FIRST_PIPE_INSTANCE
            } else {
                0
            };
        let mode = PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS;
        // SAFETY: `name` is a NUL-terminated wide string that outlives the call.
        let pipe = unsafe {
            CreateNamedPipeW(
                name.as_ptr(),
                flags,
                mode,
                PIPE_UNLIMITED_INSTANCES,
                0,
                4096,
                0,
                ptr::null(),
            )
        };
        anyhow::ensure!(
            pipe != INVALID_HANDLE_VALUE,
            "handoff: cannot open the pipe: {}",
            std::io::Error::last_os_error()
        );
        Ok(pipe as isize)
    }

    pub fn serve(each: impl Fn(String) -> bool + Send + 'static) -> anyhow::Result<()> {
        let name: Vec<u16> = OsStr::new(&name()).encode_wide().chain([0]).collect();
        let mut pipe = create(&name, true)?;
        log::info!("handoff: listening on the links pipe");
        std::thread::Builder::new()
            .name("handoff".into())
            .spawn(move || {
                loop {
                    // SAFETY: the pipe is open; a client that came first counts as connected.
                    let connected = unsafe { ConnectNamedPipe(pipe as _, ptr::null_mut()) } != 0
                        || unsafe { GetLastError() } == ERROR_PIPE_CONNECTED;
                    // SAFETY: the pipe is open, and the file owns it from here.
                    let file = unsafe { std::fs::File::from_raw_handle(pipe as _) };
                    let mut text = String::new();
                    if connected
                        && let Err(error) = (&file).take(64 * 1024).read_to_string(&mut text)
                    {
                        log::error!("handoff: a link did not arrive: {error}");
                    }
                    // SAFETY: the pipe is open until the file drops.
                    unsafe { DisconnectNamedPipe(file.as_raw_handle() as _) };
                    drop(file);
                    if !text.lines().all(|line| each(line.to_string())) {
                        return;
                    }
                    pipe = match create(&name, false) {
                        Ok(pipe) => pipe,
                        Err(error) => {
                            log::error!("{error:#}");
                            return;
                        }
                    };
                }
            })?;
        Ok(())
    }

    /// Points `elyteaparty://` at this executable for the current user, as an unpacked zip has no installer.
    pub fn register() -> anyhow::Result<()> {
        use windows_sys::Win32::System::Registry::{HKEY_CURRENT_USER, REG_SZ, RegSetKeyValueW};
        let exe = std::env::current_exe()?;
        let wide = |text: &str| -> Vec<u16> { OsStr::new(text).encode_wide().chain([0]).collect() };
        let command = format!("\"{}\" \"%1\"", exe.display());
        for (key, value, data) in [
            (r"Software\Classes\elyteaparty", "", "URL:Ely Tea Party"),
            (r"Software\Classes\elyteaparty", "URL Protocol", ""),
            (
                r"Software\Classes\elyteaparty\shell\open\command",
                "",
                command.as_str(),
            ),
        ] {
            let (key, value, data) = (wide(key), wide(value), wide(data));
            // SAFETY: each string is NUL-terminated and `data`'s size counts its NUL.
            let status = unsafe {
                RegSetKeyValueW(
                    HKEY_CURRENT_USER,
                    key.as_ptr(),
                    if value.len() > 1 {
                        value.as_ptr()
                    } else {
                        ptr::null()
                    },
                    REG_SZ,
                    data.as_ptr().cast(),
                    (data.len() * 2) as u32,
                )
            };
            anyhow::ensure!(
                status == 0,
                "handoff: cannot register the link scheme: error {status}"
            );
        }
        log::info!(
            "handoff: {SCHEME}:// opens {}",
            exe.display(),
            SCHEME = super::SCHEME
        );
        Ok(())
    }
}

/// Hands `links` to the app already running; false when none listens.
#[cfg(any(target_os = "linux", windows))]
pub fn forward(links: &[String]) -> bool {
    endpoint::send(links).is_ok()
}

/// Listens for links later launches hand over; each goes to `sender`.
#[cfg(any(target_os = "linux", windows))]
pub fn listen(sender: UnboundedSender<String>) {
    #[cfg(windows)]
    if let Err(error) = endpoint::register() {
        log::error!("{error:#}");
    }
    let served = endpoint::serve(move |line| {
        let line = line.trim();
        line.is_empty() || sender.unbounded_send(line.to_string()).is_ok()
    });
    if let Err(error) = served {
        log::info!("{error:#}");
    }
}

/// macOS hands every link to gpui, whether the app runs or not.
#[cfg(target_os = "macos")]
pub fn listen(_: UnboundedSender<String>) {}
