use std::{io, path::PathBuf, sync::Arc};

use alacritty_terminal::{
    event::{OnResize, WindowSize},
    tty::{ChildEvent, EventedPty, EventedReadWrite},
};
use futures::channel::mpsc::UnboundedSender;
use polling::{Event, PollMode, Poller};

use super::{
    marks,
    mouse::MouseMode,
    notify::{self, Kitty},
    prompts::{self, Prompts},
    pty::PtyEvent,
    status,
    titles::{self, Titles},
};

/// The longest OSC body kept; longer ones are dropped whole.
pub(super) const MOST: usize = 4096;

/// A pty whose reader shows each byte to its scanners before the parser.
pub(crate) struct Tapped<P> {
    pub inner: P,
    scanner: Osc,
    mouse: MouseModes,
    prompts: Prompts,
    kitty: Kitty,
    titles: Titles,
    events: UnboundedSender<PtyEvent>,
    pid: u32,
    last: Option<PathBuf>,
}

impl<P> Tapped<P> {
    pub fn new(inner: P, events: UnboundedSender<PtyEvent>, pid: u32) -> Self {
        Self {
            inner,
            scanner: Osc::default(),
            mouse: MouseModes::default(),
            prompts: Prompts::default(),
            kitty: Kitty::default(),
            titles: Titles::default(),
            events,
            pid,
            last: None,
        }
    }
}

impl<P: EventedReadWrite> io::Read for Tapped<P> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        // What a rewrite adds stays inside the buffer, with room for the held sequence's own.
        let room = buf
            .len()
            .saturating_sub(self.prompts.held() + prompts::SLACK);
        let take = room / prompts::GROWTH;
        if take == 0 {
            return Ok(0);
        }
        let count = self.inner.reader().read(&mut buf[..take])?;
        let read = &buf[..count];
        let (events, pid, last) = (&self.events, self.pid, &mut self.last);
        let (kitty, titles) = (&mut self.kitty, &mut self.titles);
        self.scanner.feed(read, &mut |found| {
            report(found, events, pid, last, kitty, titles)
        });
        if let Some(mode) = self.mouse.feed(read) {
            log::info!("pty {pid}: mouse {mode:?}");
            if events.unbounded_send(PtyEvent::Mouse(mode)).is_err() {
                log::debug!("pty {pid}: a mouse mode after its session closed");
            }
        }
        let out = self.prompts.pass(&buf[..count]);
        buf[..out.len()].copy_from_slice(out);
        Ok(out.len())
    }
}

/// Sends the session what the scanner found: a full reset, a title, a mark, a program's status or ask, or a folder.
fn report(
    found: Found,
    events: &UnboundedSender<PtyEvent>,
    pid: u32,
    last: &mut Option<PathBuf>,
    kitty: &mut Kitty,
    titles: &mut Titles,
) {
    let event = match found {
        Found::Reset => {
            log::debug!("pty {pid}: a full reset");
            titles.reset();
            PtyEvent::Reset
        }
        Found::Push => {
            titles.push();
            return;
        }
        Found::Pop => match titles.pop() {
            Some(title) => PtyEvent::Title(title),
            None => return,
        },
        Found::Osc(body, length) => {
            if let Some(mark) = body.strip_prefix(b"133;").and_then(marks::read) {
                log::debug!("pty {pid}: {mark:?}");
                PtyEvent::Mark(mark)
            } else if let Some(status) = body
                .strip_prefix(b"7501;")
                .and_then(|rest| status::read(rest, length))
            {
                log::debug!("pty {pid}: {status:?}");
                PtyEvent::Status(status)
            } else if let Some(ask) = ask(body, kitty) {
                log::debug!("pty {pid}: {ask:?}");
                PtyEvent::Ask(ask)
            } else if let Some(folder) = body.strip_prefix(b"7;").and_then(folder) {
                if last.as_ref() == Some(&folder) {
                    log::debug!("pty {pid}: folder {} again", folder.display());
                } else {
                    log::info!("pty {pid}: folder {}", folder.display());
                    *last = Some(folder.clone());
                }
                PtyEvent::Folder(folder)
            } else if let Some(rest) = body
                .strip_prefix(b"0;")
                .or_else(|| body.strip_prefix(b"2;"))
            {
                PtyEvent::Title(titles.set(rest))
            } else {
                return;
            }
        }
    };
    if events.unbounded_send(event).is_err() {
        log::debug!("pty {pid}: an event after its session closed");
    }
}

/// A notification or progress an OSC 99, 777 or 9 body asks for.
fn ask(body: &[u8], kitty: &mut Kitty) -> Option<notify::Ask> {
    if let Some(rest) = body.strip_prefix(b"99;") {
        kitty.read(rest)
    } else if let Some(rest) = body.strip_prefix(b"777;") {
        notify::rxvt(rest)
    } else {
        body.strip_prefix(b"9;").and_then(notify::nine)
    }
}

impl<P: EventedReadWrite> EventedReadWrite for Tapped<P> {
    type Reader = Self;
    type Writer = P::Writer;

    unsafe fn register(
        &mut self,
        poll: &Arc<Poller>,
        event: Event,
        mode: PollMode,
    ) -> io::Result<()> {
        unsafe { self.inner.register(poll, event, mode) }
    }

    fn reregister(&mut self, poll: &Arc<Poller>, event: Event, mode: PollMode) -> io::Result<()> {
        self.inner.reregister(poll, event, mode)
    }

    fn deregister(&mut self, poll: &Arc<Poller>) -> io::Result<()> {
        self.inner.deregister(poll)
    }

    fn reader(&mut self) -> &mut Self {
        self
    }

    fn writer(&mut self) -> &mut P::Writer {
        self.inner.writer()
    }
}

impl<P: EventedPty> EventedPty for Tapped<P> {
    fn next_child_event(&mut self) -> Option<ChildEvent> {
        self.inner.next_child_event()
    }
}

impl<P: OnResize> OnResize for Tapped<P> {
    fn on_resize(&mut self, size: WindowSize) {
        self.inner.on_resize(size);
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
enum State {
    #[default]
    Ground,
    Escape,
    Osc,
    Csi,
}

/// What the scanner finds: an OSC's body with the sequence's length from OSC through ST, a full reset, or the title stack's push and pop.
pub(crate) enum Found<'a> {
    Osc(&'a [u8], usize),
    Reset,
    Push,
    Pop,
}

/// Finds each OSC body across reads, ended as vte ends it: at BEL, ESC, CAN or SUB, each full reset, and CSI 22 t and 23 t.
#[derive(Default)]
pub(crate) struct Osc {
    state: State,
    body: Vec<u8>,
    /// The sequence's bytes so far, the controls the body leaves out too.
    length: usize,
    overflow: bool,
    /// A CSI's first parameter while it reads as a plain number, and its fields so far.
    first: Option<u32>,
    fields: usize,
}

impl Osc {
    pub fn feed(&mut self, bytes: &[u8], found: &mut impl FnMut(Found)) {
        let mut rest = bytes;
        while !rest.is_empty() {
            if self.state == State::Ground {
                match rest.iter().position(|&byte| byte == 0x1b) {
                    Some(at) => {
                        self.state = State::Escape;
                        rest = &rest[at + 1..];
                    }
                    None => return,
                }
                continue;
            }
            let byte = rest[0];
            rest = &rest[1..];
            self.state = match (self.state, byte) {
                (State::Escape, b']') => {
                    self.body.clear();
                    self.length = 2;
                    self.overflow = false;
                    State::Osc
                }
                (State::Escape, b'c') => {
                    found(Found::Reset);
                    State::Ground
                }
                (State::Escape, b'[') => {
                    self.first = Some(0);
                    self.fields = 1;
                    State::Csi
                }
                // vte runs controls in an escape, passes DEL and high bytes by, and stays there.
                (State::Escape, 0x00..=0x17 | 0x19 | 0x1b..=0x1f | 0x7f..=0xff) => State::Escape,
                (State::Escape, _) => State::Ground,
                (State::Osc, 0x07 | 0x18 | 0x1a) => {
                    self.finish(1, found);
                    State::Ground
                }
                // ESC ends the body, then reads as an escape: `\` makes ST, counted so.
                (State::Osc, 0x1b) => {
                    self.finish(2, found);
                    State::Escape
                }
                (State::Osc, 0x00..=0x06 | 0x08..=0x17 | 0x19 | 0x1c..=0x1f) => {
                    self.length += 1;
                    State::Osc
                }
                (State::Osc, byte) => {
                    self.length += 1;
                    if self.body.len() < self.most() {
                        self.body.push(byte);
                    } else {
                        self.overflow = true;
                    }
                    State::Osc
                }
                // In a CSI, as in vte: CAN and SUB cancel, ESC restarts, controls run, DEL and high bytes pass by.
                (State::Csi, 0x18 | 0x1a) => State::Ground,
                (State::Csi, 0x1b) => State::Escape,
                (State::Csi, 0x00..=0x1f | 0x7f..=0xff) => State::Csi,
                (State::Csi, b'0'..=b'9') => {
                    if self.fields == 1 {
                        self.first = self.first.map(|first| {
                            first
                                .saturating_mul(10)
                                .saturating_add(u32::from(byte - b'0'))
                        });
                    }
                    State::Csi
                }
                (State::Csi, b';' | b':') => {
                    self.fields += 1;
                    State::Csi
                }
                (State::Csi, b't') => {
                    match self.first.filter(|_| self.fields <= PARAMS) {
                        Some(22) => found(Found::Push),
                        Some(23) => found(Found::Pop),
                        _ => {}
                    }
                    State::Ground
                }
                (State::Csi, 0x40..=0x7e) => State::Ground,
                // A private marker or an intermediate makes another sequence.
                (State::Csi, _) => {
                    self.first = None;
                    State::Csi
                }
                (State::Ground, _) => unreachable!("ground scans by position"),
            };
        }
    }

    /// The longest body kept: an OSC 99's holds its payload past its metadata, a title's more.
    fn most(&self) -> usize {
        if self.body.starts_with(b"99;") {
            notify::BODY
        } else if self.body.starts_with(b"0;") || self.body.starts_with(b"2;") {
            titles::LONGEST
        } else {
            MOST
        }
    }

    fn finish(&mut self, end: usize, found: &mut impl FnMut(Found)) {
        if self.overflow {
            log::warn!("pty: an OSC over {} bytes dropped", self.most());
        } else {
            found(Found::Osc(&self.body, self.length + end));
        }
        self.body.clear();
    }
}

/// The most values, subparameters too, one sequence holds before vte drops it.
const PARAMS: usize = 32;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
enum Csi {
    #[default]
    Ground,
    Escape,
    Bracket,
    Private,
}

/// The mouse modes as xterm holds them, read as vte reads; alacritty drops 9.
#[derive(Default)]
pub(crate) struct MouseModes {
    state: Csi,
    params: Vec<u32>,
    /// Values begun, subparameters too.
    fields: usize,
    /// Past a `:`, whose subparameters the parser's mode sets skip.
    sub: bool,
    mode: MouseMode,
}

impl MouseModes {
    /// The mode after `bytes`, when they changed it.
    pub fn feed(&mut self, bytes: &[u8]) -> Option<MouseMode> {
        let was = self.mode;
        let mut rest = bytes;
        while !rest.is_empty() {
            if self.state == Csi::Ground {
                let Some(at) = rest.iter().position(|&byte| byte == 0x1b) else {
                    break;
                };
                self.state = Csi::Escape;
                rest = &rest[at + 1..];
                continue;
            }
            let byte = rest[0];
            rest = &rest[1..];
            self.state = match (self.state, byte) {
                (_, 0x18 | 0x1a) => Csi::Ground,
                (_, 0x1b) => Csi::Escape,
                // Mid-sequence, C0 runs and DEL is skipped, as in vte.
                (state, 0x00..=0x1f | 0x7f..=0xff) => state,
                (Csi::Escape, b'[') => Csi::Bracket,
                (Csi::Escape, b'c') => {
                    self.mode = MouseMode::default();
                    Csi::Ground
                }
                (Csi::Bracket, b'?') => {
                    self.params = vec![0];
                    self.fields = 1;
                    self.sub = false;
                    Csi::Private
                }
                (Csi::Private, b'0'..=b'9') => {
                    if !self.sub {
                        let last = self.params.last_mut().expect("a parameter in progress");
                        *last = last
                            .saturating_mul(10)
                            .saturating_add(u32::from(byte - b'0'));
                    }
                    Csi::Private
                }
                (Csi::Private, b':' | b';') if self.fields < PARAMS => {
                    self.fields += 1;
                    self.sub = byte == b':';
                    if !self.sub {
                        self.params.push(0);
                    }
                    Csi::Private
                }
                (Csi::Private, b'h' | b'l') => {
                    for &mode in &self.params {
                        self.mode.apply(mode, byte == b'h');
                    }
                    Csi::Ground
                }
                _ => Csi::Ground,
            };
        }
        (self.mode != was).then_some(self.mode)
    }
}

/// The local folder an OSC 7 `file://host/path` names; another host's folder is none.
pub(crate) fn folder(body: &[u8]) -> Option<PathBuf> {
    let Some(rest) = body.strip_prefix(b"file://") else {
        log::warn!(
            "pty: OSC 7 without file:// ({})",
            String::from_utf8_lossy(body)
        );
        return None;
    };
    let slash = rest.iter().position(|&byte| byte == b'/')?;
    let host = String::from_utf8_lossy(&rest[..slash]);
    let local = gethostname::gethostname();
    if !(host.is_empty()
        || host.eq_ignore_ascii_case("localhost")
        || local
            .to_str()
            .is_some_and(|name| host.eq_ignore_ascii_case(name)))
    {
        log::debug!("pty: OSC 7 from host {host}, not this one");
        return None;
    }
    let path = match decode(&rest[slash..]) {
        Some(path) => path,
        None => {
            log::warn!("pty: OSC 7 path is not UTF-8 after decoding");
            return None;
        }
    };
    let drive = path.as_bytes().get(1..3).is_some_and(|two| two[1] == b':')
        && path.as_bytes()[1].is_ascii_alphabetic();
    Some(PathBuf::from(if cfg!(windows) && drive {
        &path[1..]
    } else {
        &path[..]
    }))
}

fn decode(bytes: &[u8]) -> Option<String> {
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        let hex = bytes
            .get(at + 1..at + 3)
            .and_then(|two| std::str::from_utf8(two).ok())
            .and_then(|two| u8::from_str_radix(two, 16).ok());
        match (bytes[at], hex) {
            (b'%', Some(value)) => {
                out.push(value);
                at += 3;
            }
            (byte, _) => {
                out.push(byte);
                at += 1;
            }
        }
    }
    String::from_utf8(out).ok()
}

#[cfg(test)]
mod tests;
