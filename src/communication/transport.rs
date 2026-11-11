use std::{
    io::{BufRead, BufReader, Read, Write},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    time::Duration,
};

use anyhow::{Context, ensure};
use futures::channel::mpsc::{Receiver, Sender, channel};

use super::protocol::{MAX_FRAME, Request, Response};

const TIMEOUT: Duration = Duration::from_secs(8);
const MAX_RESPONSE: u64 = 24 * 1024 * 1024;

pub struct Job {
    pub request: Request,
    pub reply: mpsc::SyncSender<Response>,
}

pub struct Server {
    pub endpoint: String,
    #[cfg(unix)]
    directory: std::path::PathBuf,
}

impl Server {
    pub fn close(&self) {
        #[cfg(unix)]
        for result in [
            std::fs::remove_file(self.directory.join("mail.sock")),
            std::fs::remove_dir(&self.directory),
        ] {
            if let Err(error) = result
                && error.kind() != std::io::ErrorKind::NotFound
            {
                log::error!("messages: socket cleanup failed: {error}");
            }
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.close();
    }
}

#[cfg(unix)]
pub fn listen() -> anyhow::Result<(Server, Receiver<Job>)> {
    use std::os::unix::{fs::DirBuilderExt, net::UnixListener};
    let directory = std::path::Path::new("/tmp").join(format!("ely-msg-{}", uuid::Uuid::new_v4()));
    std::fs::DirBuilder::new().mode(0o700).create(&directory)?;
    let path = directory.join("mail.sock");
    let server = Server {
        endpoint: format!("unix:{}", path.display()),
        directory,
    };
    let listener = UnixListener::bind(&path).context("messages: bind local socket")?;
    let (sender, receiver) = channel(16);
    std::thread::Builder::new()
        .name("ely-messages".into())
        .spawn(move || {
            let active = Arc::new(AtomicUsize::new(0));
            for stream in listener.incoming() {
                match stream {
                    Ok(stream) => {
                        if let Err(error) = stream
                            .set_read_timeout(Some(TIMEOUT))
                            .and_then(|_| stream.set_write_timeout(Some(TIMEOUT)))
                        {
                            log::error!("messages: socket timeout setup failed: {error}");
                            continue;
                        }
                        dispatch(stream, sender.clone(), active.clone());
                    }
                    Err(error) => log::error!("messages: accept failed: {error}"),
                }
            }
        })?;
    Ok((server, receiver))
}

#[cfg(windows)]
pub fn listen() -> anyhow::Result<(Server, Receiver<Job>)> {
    let listener = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))?;
    let server = Server {
        endpoint: format!("tcp:{}", listener.local_addr()?),
    };
    let (sender, receiver) = channel(16);
    std::thread::Builder::new()
        .name("ely-messages".into())
        .spawn(move || {
            let active = Arc::new(AtomicUsize::new(0));
            for stream in listener.incoming() {
                match stream {
                    Ok(stream) => {
                        if let Err(error) = stream
                            .set_read_timeout(Some(TIMEOUT))
                            .and_then(|_| stream.set_write_timeout(Some(TIMEOUT)))
                        {
                            log::error!("messages: socket timeout setup failed: {error}");
                            continue;
                        }
                        dispatch(stream, sender.clone(), active.clone());
                    }
                    Err(error) => log::error!("messages: accept failed: {error}"),
                }
            }
        })?;
    Ok((server, receiver))
}

fn dispatch<S: Read + Write + Send + 'static>(
    mut stream: S,
    mut sender: Sender<Job>,
    active: Arc<AtomicUsize>,
) {
    if active.fetch_add(1, Ordering::SeqCst) >= 8 {
        active.fetch_sub(1, Ordering::SeqCst);
        return;
    }
    let count = active.clone();
    let spawned = std::thread::Builder::new()
        .name("ely-message-client".into())
        .spawn(move || {
            let result = (|| {
                let request: Request = serde_json::from_slice(&frame(&mut stream, MAX_FRAME)?)?;
                let (reply, received) = mpsc::sync_channel(1);
                sender.try_send(Job { request, reply }).map_err(|_| {
                    anyhow::anyhow!("messages: app is busy; retry with the same request key")
                })?;
                received
                    .recv_timeout(TIMEOUT)
                    .context("messages: app response timed out; delivery is unconfirmed")
            })();
            let response = result.unwrap_or_else(|error| Response::from_result(Err(error)));
            if let Err(error) = write_response(&mut stream, &response) {
                log::debug!("messages: client reply failed: {error:#}");
            }
            count.fetch_sub(1, Ordering::SeqCst);
        });
    if let Err(error) = spawned {
        active.fetch_sub(1, Ordering::SeqCst);
        log::error!("messages: cannot start client handler: {error}");
    }
}

fn write_response(stream: &mut impl Write, response: &Response) -> anyhow::Result<()> {
    let bytes = serde_json::to_vec(response)?;
    ensure!(
        bytes.len() as u64 <= MAX_RESPONSE,
        "messages: response exceeds size limit"
    );
    stream.write_all(&bytes)?;
    stream.write_all(b"\n")?;
    Ok(())
}

pub fn frame(stream: impl Read, limit: u64) -> anyhow::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    BufReader::new(stream.take(limit + 1)).read_until(b'\n', &mut bytes)?;
    ensure!(
        bytes.len() as u64 <= limit && bytes.last() == Some(&b'\n'),
        "messages: incomplete or oversized frame"
    );
    Ok(bytes)
}

fn exchange(mut stream: impl Read + Write, request: &Request) -> anyhow::Result<Response> {
    let bytes = serde_json::to_vec(request)?;
    ensure!(
        (bytes.len() as u64) < MAX_FRAME,
        "messages: request is too large"
    );
    stream.write_all(&bytes)?;
    stream.write_all(b"\n")?;
    Ok(serde_json::from_slice(&frame(stream, MAX_RESPONSE)?)?)
}

pub fn call(endpoint: &str, request: &Request) -> anyhow::Result<Response> {
    #[cfg(unix)]
    if let Some(path) = endpoint.strip_prefix("unix:") {
        let stream = std::os::unix::net::UnixStream::connect(path)
            .context("messages: cannot connect to this Ely app")?;
        stream.set_read_timeout(Some(TIMEOUT))?;
        stream.set_write_timeout(Some(TIMEOUT))?;
        return exchange(stream, request);
    }
    #[cfg(windows)]
    if let Some(address) = endpoint.strip_prefix("tcp:") {
        let address: std::net::SocketAddr = address.parse()?;
        ensure!(
            address.ip().is_loopback(),
            "messages: only local endpoints are supported"
        );
        let stream = std::net::TcpStream::connect_timeout(&address, TIMEOUT)?;
        stream.set_read_timeout(Some(TIMEOUT))?;
        stream.set_write_timeout(Some(TIMEOUT))?;
        return exchange(stream, request);
    }
    anyhow::bail!("messages: unsupported local endpoint")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn frames_must_be_complete_and_bounded() {
        assert_eq!(frame(&b"{}\ntrailing"[..], 3).unwrap(), b"{}\n");
        assert!(frame(&b"{}"[..], 10).is_err());
        assert!(frame(&b"1234\n"[..], 4).is_err());
    }
}
