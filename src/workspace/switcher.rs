use std::time::Duration;

use ely_gpui_component::{
    forms::{InputEvent, TextInput},
    i18n::text,
};
use gpui::{
    App, AppContext, Context, Entity, FocusHandle, Focusable, KeyBinding, ScrollHandle,
    Subscription, Task, WeakEntity, Window, WindowHandle, actions,
};

use super::Workspace;
use crate::{
    agents::{Kind, icons},
    sessions::{self, task::TaskState},
};

mod search;
mod view;
pub(super) use search::visit;

actions!(ely_tea_party, [SwitcherPrevious, SwitcherNext]);

pub(super) fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("up", SwitcherPrevious, Some("SessionSwitcher > ElyInput")),
        KeyBinding::new("down", SwitcherNext, Some("SessionSwitcher > ElyInput")),
    ]);
}

struct Entry {
    id: u64,
    window: WindowHandle<Workspace>,
    title: String,
    folder: String,
    agent: String,
    kind: Option<Kind>,
    state: TaskState,
}

struct Row {
    entry: Entry,
    matched: search::Match,
}

pub(super) struct Switcher {
    owner: WeakEntity<Workspace>,
    window: WindowHandle<Workspace>,
    current: Option<u64>,
    previous_focus: Option<FocusHandle>,
    focus: FocusHandle,
    input: Entity<TextInput>,
    rows: Vec<Row>,
    selected: Option<u64>,
    scroll: ScrollHandle,
    _input: Subscription,
    _refresh: Task<()>,
}

fn entries(owner: &Workspace, window: WindowHandle<Workspace>, cx: &App) -> Vec<Entry> {
    let mut entries = Vec::new();
    let mut collect = |workspace: &Workspace, handle| {
        entries.extend(workspace.sessions.iter().map(|session| {
            let kind = session.agent.as_ref().map(|agent| agent.kind);
            let agent = session.agent.as_ref().map_or_else(
                || text(cx, "et.switcher.terminal", &[]).to_string(),
                |agent| {
                    let name = icons::name(agent.kind);
                    match &agent.program {
                        Some(program) => format!("{name} · {program}"),
                        None => name.to_owned(),
                    }
                },
            );
            Entry {
                id: session.id,
                window: handle,
                title: sessions::compact::title(session),
                folder: sessions::one_line(&session.folder.to_string_lossy()),
                agent,
                kind,
                state: session.task_state(cx),
            }
        }));
    };
    collect(owner, window);
    for handle in cx
        .windows()
        .into_iter()
        .filter_map(|handle| handle.downcast::<Workspace>())
    {
        if handle != window {
            match handle.read(cx) {
                Ok(workspace) => collect(workspace, handle),
                Err(error) => log::debug!("switcher: window unavailable: {error}"),
            }
        }
    }
    entries
}

impl Workspace {
    pub(super) fn toggle_switcher(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(switcher) = self.switcher.take() {
            if let Some(focus) = switcher.read(cx).previous_focus.clone() {
                window.focus(&focus, cx);
            }
            log::info!("switcher: dismissed");
        } else {
            let handle = window
                .window_handle()
                .downcast::<Workspace>()
                .expect("workspace window");
            let entries = entries(self, handle, cx);
            let owner = cx.entity().downgrade();
            let current = self.sessions.get(self.active).map(|session| session.id);
            self.switcher =
                Some(cx.new(|cx| Switcher::new(owner, handle, current, entries, window, cx)));
            log::info!("switcher: opened");
        }
        cx.notify();
    }
}

impl Switcher {
    fn new(
        owner: WeakEntity<Workspace>,
        handle: WindowHandle<Workspace>,
        current: Option<u64>,
        entries: Vec<Entry>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let previous_focus = window.focused(cx);
        let input = cx.new(|cx| {
            TextInput::new(window, cx).placeholder(text(cx, "et.switcher.placeholder", &[]))
        });
        let subscription =
            cx.subscribe_in(&input, window, |this, _, event, window, cx| match event {
                InputEvent::Changed => {
                    this.selected = None;
                    this.scroll.set_offset(Default::default());
                    this.refresh(cx);
                }
                InputEvent::Submit => this.pick(window, cx),
                _ => {}
            });
        window.focus(&input.focus_handle(cx), cx);
        let refresh = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(500))
                    .await;
                if this.update(cx, |this, cx| this.refresh(cx)).is_err() {
                    break;
                }
            }
        });
        let mut this = Self {
            owner,
            window: handle,
            current,
            previous_focus,
            focus: cx.focus_handle(),
            input,
            rows: Vec::new(),
            selected: None,
            scroll: ScrollHandle::new(),
            _input: subscription,
            _refresh: refresh,
        };
        this.filter(entries, cx);
        this
    }

    fn filter(&mut self, entries: Vec<Entry>, cx: &App) {
        let before = self
            .rows
            .iter()
            .position(|row| Some(row.entry.id) == self.selected);
        let query = self.input.read(cx).text();
        let recent = cx.global::<search::Recent>();
        self.rows = entries
            .into_iter()
            .filter_map(|entry| {
                search::matches(query, [&entry.title, &entry.folder, &entry.agent])
                    .map(|matched| Row { entry, matched })
            })
            .collect();
        self.rows.sort_by_key(|row| {
            search::order(
                query.trim().is_empty(),
                self.current == Some(row.entry.id),
                row.matched.score,
                recent.rank(row.entry.id),
                row.entry.state.priority(),
                row.entry.id,
            )
        });
        let ids: Vec<_> = self.rows.iter().map(|row| row.entry.id).collect();
        self.selected = search::selected(&ids, self.selected);
        if let Some(at) = ids.iter().position(|id| Some(*id) == self.selected)
            && before != Some(at)
        {
            self.scroll.scroll_to_item(at);
        }
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        let Some(owner) = self.owner.upgrade() else {
            return;
        };
        self.filter(entries(owner.read(cx), self.window, cx), cx);
        cx.notify();
    }

    fn step(&mut self, by: isize, cx: &mut Context<Self>) {
        if self.rows.is_empty() {
            return;
        }
        let at = self
            .rows
            .iter()
            .position(|row| Some(row.entry.id) == self.selected)
            .unwrap_or(0);
        let next = (at as isize + by).rem_euclid(self.rows.len() as isize) as usize;
        self.selected = Some(self.rows[next].entry.id);
        self.scroll.scroll_to_item(next);
        cx.notify();
    }

    fn dismiss(&self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(owner) = self.owner.upgrade() {
            owner.update(cx, |this, cx| {
                this.switcher = None;
                if let Some(focus) = &self.previous_focus {
                    window.focus(focus, cx);
                }
                cx.notify();
            });
        }
        log::info!("switcher: dismissed");
    }

    fn pick(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(row) = self
            .rows
            .iter()
            .find(|row| Some(row.entry.id) == self.selected)
        else {
            return;
        };
        let (id, target) = (row.entry.id, row.entry.window);
        let Some(owner) = self.owner.upgrade() else {
            return;
        };
        let select =
            |workspace: &mut Workspace, window: &mut Window, cx: &mut Context<Workspace>| {
                let Some(index) = workspace
                    .sessions
                    .iter()
                    .position(|session| session.id == id)
                else {
                    log::warn!("switcher: session {id} already closed");
                    return false;
                };
                workspace.select(index, window, cx);
                window.activate_window();
                log::info!("switcher: selected session {id}");
                true
            };
        let selected = if target == self.window {
            owner.update(cx, |workspace, cx| select(workspace, window, cx))
        } else {
            match target.update(cx, select) {
                Ok(selected) => selected,
                Err(error) => {
                    log::warn!("switcher: target window closed: {error}");
                    false
                }
            }
        };
        if selected {
            owner.update(cx, |this, cx| {
                this.switcher = None;
                if target != self.window
                    && let Some(focus) = &self.previous_focus
                {
                    window.focus(focus, cx);
                }
                cx.notify();
            });
        } else {
            self.refresh(cx);
        }
    }
}
