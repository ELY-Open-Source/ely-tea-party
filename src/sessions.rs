use std::{
    path::{MAIN_SEPARATOR, Path, PathBuf},
    time::Instant,
};

use ely_gpui_component::{
    buttons::{ButtonVariant, IconButton},
    lists::ListItem,
    primitives::{Icon, IconName, Tooltip},
    theme::{ActiveTheme, ControlSize, IconSize, Radius},
};
use gpui::{
    AnyElement, App, Entity, ObjectFit, Rems, SharedString, StyledImage, Subscription, div, img,
    prelude::*, rems,
};

use crate::{
    agents::{self, Agent, Pose, Status},
    term::{Kind, Record, Records, State, TerminalView},
    workspace::{Drawer, TITLE, Workspace},
};

mod drag;
pub mod orb;

pub use drag::{Moving, after_move, row};

use orb::Look;

/// A program on its own terminal, as the sessions column lists it.
pub struct Session {
    /// Its hooks report under this.
    pub id: u64,
    /// The coding agent it runs, if any.
    pub agent: Option<Agent>,
    pub terminal: Entity<TerminalView>,
    /// The program's title; empty while it names none.
    pub title: String,
    /// The folder it reports, else the one it started in.
    pub folder: PathBuf,
    /// Its shell has integration, or it reported a folder anyway.
    pub reports: bool,
    /// A bell rang while it was hidden.
    pub bell: bool,
    /// The user's own shell under its terminal, once opened.
    pub drawer: Option<Drawer>,
    /// A line to type at its first prompt: a restored agent's resume.
    pub pending: Option<String>,
    /// It started waiting on the user while hidden.
    pub waiting: bool,
    /// The look its mark shows, and since when.
    pub look: Option<(Look, Instant)>,
    pub _events: Subscription,
}

impl Session {
    /// Notes the look its mark takes `now`; true while the mark moves.
    pub fn note(&mut self, now: Instant, cx: &App) -> bool {
        let view = self.terminal.read(cx);
        let failed = view.ended().is_some_and(|code| code != 0);
        let shows = look(view.records().shown().map(|(_, record)| record), failed);
        if self.look.map(|(shown, _)| shown) != shows {
            self.look = shows.map(|shows| (shows, now));
        }
        self.look
            .is_some_and(|(look, since)| orb::moves(look, now.saturating_duration_since(since)))
    }
}

/// The most records hover lists, and the most characters it shows of each part.
const LISTED: usize = 10;
const SHORT: usize = 120;

/// `folder` as a row shows it: the home folder as `~`, on one line.
pub fn shown(folder: &Path, home: Option<&Path>) -> String {
    let shown = match home.and_then(|home| folder.strip_prefix(home).ok()) {
        Some(rest) if rest.as_os_str().is_empty() => "~".into(),
        Some(rest) => format!("~{MAIN_SEPARATOR}{}", rest.display()),
        None => folder.display().to_string(),
    };
    one_line(&shown)
}

/// A row's folder this many characters long or shorter shows whole.
const WHOLE: usize = 24;

/// `folder` as a row shows it: [`shown`], and past [`WHOLE`] characters each folder but the last cut to its first, as fish's prompt cuts them.
pub fn brief(folder: &Path, home: Option<&Path>) -> String {
    let whole = shown(folder, home);
    if whole.chars().count() <= WHOLE {
        return whole;
    }
    let parts: Vec<&str> = whole.split(MAIN_SEPARATOR).collect();
    let (last, rest) = parts.split_last().expect("split yields a part");
    let cut = rest.iter().map(|part| {
        let mut chars = part.chars();
        match chars.next() {
            Some('.') => chars.next().map_or(".".into(), |c| format!(".{c}")),
            Some(c) if *part != "~" => c.to_string(),
            _ => part.to_string(),
        }
    });
    cut.chain(std::iter::once(last.to_string()))
        .collect::<Vec<_>>()
        .join(std::path::MAIN_SEPARATOR_STR)
}

/// `text` on one line, as gpui shapes labels: each control character as its picture, `␊` for a newline.
pub fn one_line(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            '\0'..='\x1f' => char::from_u32(0x2400 + c as u32).expect("a control picture"),
            '\x7f' => '\u{2421}',
            c => c,
        })
        .collect()
}

/// The session shown once session `closed` of `left + 1` is gone, `active` having been shown.
pub fn after_close(active: usize, closed: usize, left: usize) -> usize {
    if closed < active || active == left {
        active - 1
    } else {
        active
    }
}

/// Its agent waits for an answer.
pub fn waits(session: &Session) -> bool {
    let agent = session.agent.as_ref();
    agent.is_some_and(|agent| agent.status == Status::NeedsInput)
}

/// The character of `agent` in `pose`, on a tile of its color.
pub fn avatar(
    agent: &Agent,
    pose: Pose,
    size: Rems,
    (seed, now): (u64, Instant),
    cx: &App,
) -> impl IntoElement {
    let frame = agents::assets::frame(agent.kind, pose, seed, now);
    div()
        .size(size)
        .rounded(cx.theme().radius(Radius::Lg))
        .bg(agent.kind.color().opacity(0.14))
        .children(frame.map(|frame| img(frame).size_full().object_fit(ObjectFit::Contain)))
}

/// Session `index`'s item: its title and folder, or its agent's character and doing; else the mark of its status and a bell while one rang unseen; a button that closes it.
fn item(
    workspace: &Entity<Workspace>,
    index: usize,
    session: &Session,
    active: bool,
    home: Option<&Path>,
    now: Instant,
    cx: &App,
) -> ListItem {
    let terminal = session.terminal.read(cx);
    let pid = terminal.pid() as usize;
    let title = if session.title.is_empty() {
        TITLE
    } else {
        &session.title
    };
    let folder = brief(&session.folder, home);
    let (select, close) = (workspace.clone(), workspace.clone());
    // A job left running in a drawer out of sight.
    let running = session
        .drawer
        .as_ref()
        .is_some_and(|drawer| drawer.busy && !(active && drawer.open));
    let item = match &session.agent {
        Some(agent) => {
            let pose = agent.pose(now, session.id);
            let since = agents::ago(now.saturating_duration_since(agent.since));
            ListItem::new(("session", pid), agent.kind.name())
                .description(format!("{} · {since}", agent.kind.label(pose)))
                .detail(match agent.bubble(now) {
                    Some((_, line)) => format!("「{line}」"),
                    None => agent.news(pose).map_or(folder, |news| one_line(&news)),
                })
                // The shown session's character stands out.
                .leading(avatar(
                    agent,
                    pose,
                    rems(if active { 5. } else { 3.5 }),
                    (session.id, now),
                    cx,
                ))
        }
        None => ListItem::new(("session", pid), title.to_string()).description(folder),
    };
    let item = item
        .selected(active)
        .strong(session.bell || session.waiting || waits(session))
        .on_click(move |_, window, cx| {
            // The column's own double click opens a session.
            cx.stop_propagation();
            select.update(cx, |this, cx| this.select(index, window, cx))
        })
        .trailing(
            div()
                .flex()
                .items_center()
                .gap_1()
                .children(
                    running
                        .then(|| Icon::new(IconName::Terminal).color(cx.theme().colors.fg_muted)),
                )
                .child(
                    IconButton::new(("close-session", pid), IconName::X)
                        .variant(ButtonVariant::Ghost)
                        .size(ControlSize::Sm)
                        .tooltip("Close session")
                        .on_click(move |_, window, cx| {
                            cx.stop_propagation();
                            close.update(cx, |this, cx| this.close(index, window, cx))
                        }),
                ),
        );
    // A character shows its agent's state itself.
    if session.agent.is_some() {
        return item;
    }
    let mark = mark(session, now, cx);
    let bell = session
        .bell
        .then(|| Icon::new(IconName::Bell).color(cx.theme().colors.warning));
    if mark.is_none() && bell.is_none() {
        return item;
    }
    item.leading(
        div()
            .flex()
            .items_center()
            .gap_1()
            .children(mark)
            .children(bell),
    )
}

/// What a mark shows: the most pressing record's state, else a failed last command.
pub fn look(record: Option<&Record>, failed: bool) -> Option<Look> {
    Some(
        match (record.map(|record| (record.state, record.progress)), failed) {
            (Some((State::Blocked, _)), _) => Look::Waiting,
            (Some((State::Error, _)), _) => Look::Error,
            (Some((State::Working, progress)), _) => Look::Working(progress),
            (Some((State::Done, _)), _) => Look::Done,
            (_, true) => Look::Error,
            (Some((State::Idle, _)), false) => Look::Idle,
            (None, false) => return None,
        },
    )
}

/// The session's mark: its look's dots in its state's color; hover lists the records.
fn mark(session: &Session, now: Instant, cx: &App) -> Option<AnyElement> {
    let (look, since) = session.look?;
    let theme = cx.theme();
    let colors = &theme.colors;
    let color = match look {
        Look::Working(_) => colors.info,
        Look::Waiting => colors.warning,
        Look::Error => colors.danger,
        Look::Done => colors.success,
        Look::Idle => colors.fg_muted,
    };
    let pid = session.terminal.read(cx).pid() as usize;
    // Hover reads the records as they stand, so an open one follows them, and scrolls within the window.
    let watched = session.terminal.downgrade();
    let tip = Tooltip::rich(move |window, cx| {
        let lines = watched.upgrade().map_or_else(Vec::new, |terminal| {
            let view = terminal.read(cx);
            hover(view.records(), view.ended().filter(|&code| code != 0))
        });
        let room = window.viewport_size().height - rems(3.).to_pixels(window.rem_size());
        div()
            .id("session-hover")
            .max_h(room)
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .children(lines)
            .into_any_element()
    });
    // Every mark takes a 24-pixel box, so the title stays put as the state changes.
    Some(
        div()
            .id(("session-status", pid))
            .size(theme.icon_size(IconSize::Xl))
            .flex()
            .items_center()
            .justify_center()
            .hoverable_tooltip(tip)
            .child(orb::lattice(look, color, now, since))
            .into_any_element(),
    )
}

/// What hover shows: a line for each record, ten at most, then a failed command's status.
pub fn hover(records: &Records, failed: Option<i32>) -> Vec<SharedString> {
    let mut lines: Vec<SharedString> = records
        .iter()
        .take(LISTED)
        .map(|(id, record)| line(records, id, record).into())
        .collect();
    let more = records.iter().count().saturating_sub(LISTED);
    if more > 0 {
        lines.push(format!("{more} more").into());
    }
    if let Some(code) = failed {
        lines.push(format!("Last command exited with {code}").into());
    }
    lines
}

/// A record as hover and notifications say it: its state, then its app, title and message.
pub fn line(records: &Records, id: &str, record: &Record) -> String {
    let parts = [
        records.app(id),
        record.title.as_deref(),
        record.msg.as_deref(),
    ];
    let said: Vec<String> = parts.into_iter().flatten().map(plain).collect();
    let state = label(record);
    if said.is_empty() {
        state
    } else {
        format!("{state}: {}", said.join(" · "))
    }
}

/// A record's state, as hover names it.
fn label(record: &Record) -> String {
    match (record.state, record.kind, record.progress) {
        (State::Idle, ..) => "Idle".into(),
        (State::Working, _, Some(value)) => format!("Working, {value}%"),
        (State::Working, ..) => "Working".into(),
        (State::Blocked, Some(Kind::Permission), _) => "Waiting for permission".into(),
        (State::Blocked, Some(Kind::Question), _) => "Waiting for an answer".into(),
        (State::Blocked, Some(Kind::Auth), _) => "Waiting for a login".into(),
        (State::Blocked, None, _) => "Waiting".into(),
        (State::Done, ..) => "Done".into(),
        (State::Error, ..) => "Error".into(),
    }
}

/// A program's text as hover shows it: on one line, 120 characters at most, without what could disguise it.
pub fn plain(text: &str) -> String {
    let kept: String = text.chars().filter(|&c| !invisible(c)).collect();
    let line = one_line(&kept);
    match line.char_indices().nth(SHORT) {
        Some((at, _)) => format!("{}…", &line[..at]),
        None => line,
    }
}

/// C1 controls, direction controls and invisible formatting; joiners stay, as scripts and emoji need them.
fn invisible(c: char) -> bool {
    matches!(
        c,
        '\u{80}'..='\u{9f}'
            | '\u{ad}'
            | '\u{61c}'
            | '\u{180e}'
            | '\u{200b}'
            | '\u{200e}'
            | '\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2060}'..='\u{2064}'
            | '\u{2066}'..='\u{2069}'
            | '\u{feff}'
            | '\u{fff9}'..='\u{fffb}'
            | '\u{e0000}'..='\u{e007f}'
    )
}

#[cfg(test)]
mod tests;
