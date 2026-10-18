use std::{
    path::{MAIN_SEPARATOR, Path, PathBuf},
    time::Instant,
};

use ely_gpui_component::{
    buttons::{ButtonVariant, IconButton},
    lists::ListItem,
    primitives::{Icon, IconName, Tooltip},
    theme::{ActiveTheme, ControlSize, Radius},
};
use gpui::{
    App, AppContext, Context, Entity, InteractiveElement, IntoElement, ObjectFit, ParentElement,
    Rems, Render, SharedString, StatefulInteractiveElement, Styled, StyledImage, Subscription,
    Window, div, img, rems, transparent_black,
};

use crate::{
    agents::{self, Agent, Pose, Status},
    term::TerminalView,
    workspace::{Drawer, TITLE, Workspace},
};

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
    pub _events: Subscription,
}

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

/// The session shown once session `from` moved to `to`, `active` having been shown.
pub fn after_move(active: usize, from: usize, to: usize) -> usize {
    match active {
        _ if active == from => to,
        _ if from < active && active <= to => active - 1,
        _ if to <= active && active < from => active + 1,
        _ => active,
    }
}

/// A session's row on its way to another place.
pub struct Moving {
    pub from: usize,
    title: SharedString,
}

/// What follows the pointer while a row moves: its title.
struct Ghost(SharedString);

impl Render for Ghost {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        div()
            .px_3()
            .py_1()
            .rounded(theme.radius(Radius::Md))
            .bg(theme.colors.overlay)
            .border_1()
            .border_color(theme.colors.border)
            .text_color(theme.colors.fg)
            .text_sm()
            .child(self.0.clone())
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

/// Session `index`'s row, which drags to another session's place.
pub fn row(
    workspace: &Entity<Workspace>,
    index: usize,
    session: &Session,
    active: bool,
    when: (Option<&Path>, Instant),
    cx: &App,
) -> impl IntoElement {
    let pid = session.terminal.read(cx).pid() as usize;
    let title = match &session.agent {
        Some(agent) => agent.kind.name().to_string(),
        None if session.title.is_empty() => TITLE.to_string(),
        None => one_line(&session.title),
    };
    let full = shown(&session.folder, when.0);
    let moving = Moving {
        from: index,
        title: title.into(),
    };
    let workspace = workspace.clone();
    let item = item(&workspace, index, session, active, when, cx);
    div()
        .id(("session-row", pid))
        .rounded(cx.theme().radius(Radius::Lg))
        .border_1()
        .border_color(transparent_black())
        .tooltip(Tooltip::text(full))
        .on_drag(moving, |moving, _, _, cx| {
            cx.new(|_| Ghost(moving.title.clone()))
        })
        .drag_over::<Moving>(move |style, moving, _, cx| {
            if moving.from == index {
                style
            } else {
                style.border_color(cx.theme().colors.focus)
            }
        })
        .on_drop(move |moving: &Moving, _, cx| {
            workspace.update(cx, |this, cx| this.move_session(moving.from, index, cx))
        })
        .child(item)
}

/// Session `index`'s item: its title and folder, or its agent's character and doing; a bell; a close button.
fn item(
    workspace: &Entity<Workspace>,
    index: usize,
    session: &Session,
    active: bool,
    (home, now): (Option<&Path>, Instant),
    cx: &App,
) -> ListItem {
    let pid = session.terminal.read(cx).pid() as usize;
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
        .strong(session.bell || waits(session))
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
    if session.bell && session.agent.is_none() {
        item.leading(Icon::new(IconName::Bell).color(cx.theme().colors.warning))
    } else {
        item
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folders_under_home_read_from_a_tilde() {
        let home = PathBuf::from("/Users/ely");
        let shown = |folder: &str| shown(Path::new(folder), Some(&home));
        assert_eq!(shown("/Users/ely"), "~");
        assert_eq!(
            shown("/Users/ely/src/app"),
            format!("~{MAIN_SEPARATOR}src/app")
        );
        assert_eq!(
            shown("/Users/elyse"),
            "/Users/elyse",
            "a sibling is no child"
        );
        assert_eq!(shown("/tmp"), "/tmp");
        assert_eq!(
            shown("/Users/ely/line\nbreak"),
            format!("~{MAIN_SEPARATOR}line␊break")
        );
    }

    #[test]
    fn long_folders_cut_all_but_the_last_name_to_a_letter() {
        let home = PathBuf::from("/Users/ely");
        let sep = MAIN_SEPARATOR;
        // Windows names folders with either separator, so each test folder is written in its own.
        let brief = |folder: &str| {
            brief(
                Path::new(&folder.replace('/', std::path::MAIN_SEPARATOR_STR)),
                Some(&home),
            )
        };
        assert_eq!(
            brief("/Users/ely/Documents/notes"),
            format!("~{sep}Documents{sep}notes"),
            "short enough"
        );
        assert_eq!(
            brief("/Users/ely/Workspace/other/ely-tea-party/ELYTerminal"),
            format!("~{sep}W{sep}o{sep}e{sep}ELYTerminal")
        );
        assert_eq!(
            brief("/Users/ely/.config/opencode/plugins/x"),
            format!("~{sep}.c{sep}o{sep}p{sep}x"),
            "a hidden folder keeps its dot"
        );
        assert_eq!(
            brief("/opt/homebrew/lib/node_modules/gemini"),
            format!("{sep}o{sep}h{sep}l{sep}n{sep}gemini"),
            "outside home"
        );
    }

    #[test]
    fn control_characters_show_as_their_pictures() {
        assert_eq!(one_line("a\nb\tc\rd\0e\u{7f}"), "a␊b␉c␍d␀e␡");
        assert_eq!(one_line("café ✓"), "café ✓", "printable text stays");
    }

    #[test]
    fn moving_keeps_the_shown_session_shown() {
        assert_eq!(after_move(0, 0, 2), 2, "the shown one moved");
        assert_eq!(after_move(1, 0, 2), 0, "one from before it went past it");
        assert_eq!(after_move(1, 2, 0), 2, "one from after it went before it");
        assert_eq!(after_move(3, 0, 2), 3, "the move happened above it");
    }

    #[test]
    fn closing_shows_the_next_session_or_keeps_the_shown_one() {
        assert_eq!(after_close(2, 0, 3), 1, "one before it went");
        assert_eq!(after_close(1, 1, 3), 1, "it went: the next one");
        assert_eq!(after_close(3, 3, 3), 2, "the last went: the one before");
        assert_eq!(after_close(1, 2, 3), 1, "one after it went");
    }
}
