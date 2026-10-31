use std::{collections::HashSet, path::PathBuf};

use ely_gpui_component::{
    lists::GitStatus,
    menus::ContextMenu,
    primitives::Disclosure,
    theme::{ActiveTheme, ControlSize, Radius, TextSize},
    typography::Ellipsis,
};
use gpui::{
    Context, FocusHandle, IntoElement, MouseButton, ParentElement, Pixels, Point, Render, Role,
    ScrollStrategy, Styled, UniformListScrollHandle, Window, div, prelude::*, uniform_list,
};

use super::{Files, MenuAsk, key, mark_key};
use crate::{sessions, settings::SettingsStore};

pub(super) struct State {
    pub open: HashSet<PathBuf>,
    pub selected: Option<PathBuf>,
    focus: Option<FocusHandle>,
    scroll: UniformListScrollHandle,
    pub menu: Option<MenuAsk>,
    serial: u64,
}

impl State {
    pub fn new(root: PathBuf) -> Self {
        Self {
            open: HashSet::from([root]),
            selected: None,
            focus: None,
            scroll: UniformListScrollHandle::new(),
            menu: None,
            serial: 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Row {
    pub path: PathBuf,
    pub label: String,
    pub folder: bool,
    pub depth: usize,
    pub mark: Option<GitStatus>,
    pub dirty: bool,
    pub note: Option<&'static str>,
}

impl Files {
    pub(super) fn rows(&self) -> Vec<Row> {
        let home = dirs::home_dir();
        let label = match self.root.file_name() {
            Some(name) if home.as_deref() != Some(self.root.as_path()) => {
                sessions::one_line(&name.to_string_lossy())
            }
            _ => sessions::shown(&self.root, home.as_deref()),
        };
        let mut rows = Vec::new();
        self.walk(self.root.clone(), label, true, 0, false, &mut rows);
        rows
    }

    fn walk(
        &self,
        path: PathBuf,
        label: String,
        folder: bool,
        depth: usize,
        untracked: bool,
        rows: &mut Vec<Row>,
    ) {
        let mark = self
            .marks
            .get(mark_key(&path).as_ref())
            .copied()
            .or(untracked.then_some(GitStatus::Untracked));
        let listing = self.listings.get(&path);
        let note = if folder {
            match listing {
                Some(Err(_)) => Some("unreadable"),
                None if self.tree.open.contains(&path) => Some("loading…"),
                _ => None,
            }
        } else {
            None
        };
        rows.push(Row {
            path: path.clone(),
            label,
            folder,
            depth,
            mark,
            dirty: self.dirty.contains(mark_key(&path).as_ref()),
            note,
        });
        if folder
            && self.tree.open.contains(&path)
            && let Some(Ok(entries)) = listing
        {
            for entry in entries {
                self.walk(
                    path.join(&entry.name),
                    sessions::one_line(&entry.name.to_string_lossy()),
                    entry.folder,
                    depth + 1,
                    mark == Some(GitStatus::Untracked),
                    rows,
                );
            }
        }
    }

    fn toggle(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if self.tree.open.remove(&path) {
            if self
                .tree
                .selected
                .as_ref()
                .is_some_and(|selected| selected.starts_with(&path))
            {
                self.tree.selected = Some(path.clone());
            }
        } else {
            self.tree.open.insert(path.clone());
            self.load(path.clone(), cx);
        }
        log::debug!(
            "files: {} expanded={}",
            path.display(),
            self.tree.open.contains(&path)
        );
        cx.notify();
    }

    fn context(&mut self, path: PathBuf, folder: bool, at: Point<Pixels>, cx: &mut Context<Self>) {
        self.tree.selected = Some(path.clone());
        if !SettingsStore::get(cx).files_menu {
            cx.notify();
            return;
        }
        self.tree.serial += 1;
        log::info!("files: menu for {}", path.display());
        self.tree.menu = Some(MenuAsk {
            n: self.tree.serial,
            at: Some(at),
            path,
            folder,
        });
        cx.notify();
    }

    fn row(&self, row: &Row, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let colors = &theme.colors;
        let (note, ink) = match row.mark {
            Some(GitStatus::Modified) => ("M", colors.warning),
            Some(GitStatus::Added) => ("A", colors.success),
            Some(GitStatus::Deleted) => ("D", colors.danger),
            Some(GitStatus::Untracked) => ("U", colors.success),
            Some(GitStatus::Renamed) => ("R", colors.info),
            Some(GitStatus::Conflicted) => ("!", colors.danger),
            None if row.dirty => ("•", colors.warning),
            None => ("", colors.fg),
        };
        let open = self.tree.open.contains(&row.path);
        let selected = self.tree.selected.as_ref() == Some(&row.path);
        let (toggle, click, context) = (row.clone(), row.clone(), row.clone());
        div()
            .id(key(&row.path))
            .relative()
            .role(Role::TreeItem)
            .aria_label(row.label.clone())
            .aria_level(row.depth + 1)
            .aria_selected(selected)
            .when(row.folder, |el| el.aria_expanded(open))
            .h(theme.control_height(ControlSize::Md))
            .w_full()
            .flex()
            .items_center()
            .gap_1()
            .px_2()
            .rounded(theme.radius(Radius::Sm))
            .text_size(theme.text_size(TextSize::Sm))
            .text_color(ink)
            .when(selected, |el| el.bg(colors.active))
            .when(!selected, |el| el.hover(|style| style.bg(colors.hover)))
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .px_2()
                    .flex()
                    .children((0..row.depth).map(|_| {
                        div()
                            .flex_none()
                            .w(theme.tree_indent())
                            .h_full()
                            .flex()
                            .justify_center()
                            .child(div().h_full().border_l_1().border_color(colors.border))
                    })),
            )
            .child(
                div()
                    .flex_none()
                    .w(theme.tree_indent())
                    .ml(theme.tree_indent() * row.depth as f32)
                    .children(row.folder.then(|| {
                        Disclosure::new(
                            (
                                gpui::ElementId::Name("files-chevron".into()),
                                key(&row.path),
                            ),
                            open,
                        )
                    }))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
                            if toggle.folder {
                                this.toggle(toggle.path.clone(), cx);
                            }
                        }),
                    ),
            )
            .child(super::icons::file(&row.path, row.folder, open, cx))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(Ellipsis::new(row.label.clone())),
            )
            .child(
                div()
                    .flex_none()
                    .text_size(theme.text_size(TextSize::Xs))
                    .text_color(if row.note == Some("unreadable") {
                        colors.danger
                    } else {
                        ink
                    })
                    .child(row.note.unwrap_or(note)),
            )
            .on_click(
                cx.listener(move |this, event: &gpui::ClickEvent, window, cx| {
                    cx.stop_propagation();
                    if let Some(focus) = &this.tree.focus {
                        window.focus(focus, cx);
                    }
                    this.tree.selected = Some(click.path.clone());
                    if event.click_count() == 2 {
                        if click.folder {
                            this.toggle(click.path.clone(), cx);
                        } else {
                            cx.open_with_system(&click.path);
                        }
                    }
                    cx.notify();
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &gpui::MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    window.prevent_default();
                    if let Some(focus) = &this.tree.focus {
                        window.focus(focus, cx);
                    }
                    this.context(context.path.clone(), context.folder, event.position, cx);
                }),
            )
    }

    fn navigate(&mut self, key: &str, cx: &mut Context<Self>) -> bool {
        let rows = self.rows();
        let at = rows
            .iter()
            .position(|row| Some(&row.path) == self.tree.selected.as_ref())
            .unwrap_or(0);
        let row = &rows[at];
        let to = match key {
            "up" => at.saturating_sub(1),
            "down" => (at + 1).min(rows.len() - 1),
            "home" => 0,
            "end" => rows.len() - 1,
            "left" if row.folder && self.tree.open.contains(&row.path) => {
                self.toggle(row.path.clone(), cx);
                at
            }
            "left" => rows
                .iter()
                .position(|parent| Some(parent.path.as_path()) == row.path.parent())
                .unwrap_or(at),
            "right" if row.folder && !self.tree.open.contains(&row.path) => {
                self.toggle(row.path.clone(), cx);
                at
            }
            "right" if row.folder => rows
                .get(at + 1)
                .filter(|next| next.depth > row.depth)
                .map_or(at, |_| at + 1),
            "enter" if row.folder => {
                self.toggle(row.path.clone(), cx);
                at
            }
            "enter" => {
                cx.open_with_system(&row.path);
                at
            }
            _ => return false,
        };
        self.tree.selected = Some(rows[to].path.clone());
        self.tree.scroll.scroll_to_item(to, ScrollStrategy::Top);
        cx.notify();
        true
    }
}

impl Render for Files {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focus = self
            .tree
            .focus
            .get_or_insert_with(|| cx.focus_handle())
            .clone();
        let rows = self.rows();
        let list = uniform_list(
            ("files-rows", self.generation),
            rows.len(),
            cx.processor(move |this, range: std::ops::Range<usize>, _, cx| {
                range
                    .map(|i| this.row(&rows[i], cx).into_any_element())
                    .collect()
            }),
        )
        .track_scroll(&self.tree.scroll)
        .size_full();
        let target = self
            .tree
            .menu
            .as_ref()
            .map(|ask| (ask.path.clone(), ask.folder))
            .unwrap_or_else(|| (self.root.clone(), true));
        let menu = self.menu(target.0, target.1, cx);
        let request = self.request();
        div()
            .size_full()
            .child(
                ContextMenu::new(("files-menu", self.generation), menu)
                    .manual(request)
                    .size_full()
                    .child(
                        div()
                            .id("files-tree")
                            .role(Role::Tree)
                            .track_focus(&focus)
                            .tab_index(0)
                            .size_full()
                            .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, _, cx| {
                                if this.navigate(&event.keystroke.key, cx) {
                                    cx.stop_propagation();
                                }
                            }))
                            .on_mouse_down(
                                MouseButton::Right,
                                cx.listener(|this, event: &gpui::MouseDownEvent, window, cx| {
                                    window.prevent_default();
                                    cx.stop_propagation();
                                    if let Some(focus) = &this.tree.focus {
                                        window.focus(focus, cx);
                                    }
                                    this.context(this.root.clone(), true, event.position, cx);
                                }),
                            )
                            .child(list),
                    ),
            )
            .children(self.creation_dialog(cx))
            .children(self.error_dialog(cx))
    }
}
