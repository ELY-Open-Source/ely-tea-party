use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    time::Instant,
};

use ely_gpui_component::{
    lists::List,
    primitives::{FocusRing, Icon, IconName, Tooltip},
    theme::{ActiveTheme, ControlSize, IconSize, Radius, TextSize},
    typography::Ellipsis,
};
use gpui::{AnyElement, App, Context, MouseButton, Role, Window, div, prelude::*};

use super::Workspace;
use crate::{
    sessions,
    settings::{SettingsStore, SidebarStyle},
};

#[derive(Default)]
pub(super) struct Sidebar {
    collapsed: BTreeSet<PathBuf>,
    shown: Option<(u64, PathBuf)>,
}

impl Sidebar {
    pub(super) fn reveal(&mut self, folder: &Path) {
        if self.collapsed.remove(folder) {
            log::info!("sessions: reveal folder {}", folder.display());
        }
    }
}

fn groups<'a>(folders: impl Iterator<Item = &'a Path>) -> BTreeMap<PathBuf, Vec<usize>> {
    let mut groups = BTreeMap::<PathBuf, Vec<usize>>::new();
    for (index, folder) in folders.enumerate() {
        groups.entry(folder.to_path_buf()).or_default().push(index);
    }
    groups
}

fn label(folder: &Path, home: Option<&Path>, duplicate: bool) -> String {
    if duplicate || Some(folder) == home {
        sessions::shown(folder, home)
    } else {
        folder.file_name().map_or_else(
            || sessions::shown(folder, home),
            |name| sessions::one_line(&name.to_string_lossy()),
        )
    }
}

impl Workspace {
    /// Shows the next session, or the one before, round the ends.
    pub(super) fn step(&mut self, forward: bool, window: &mut Window, cx: &mut Context<Self>) {
        let order = self.session_order(cx);
        let count = order.len();
        let Some(at) = order.iter().position(|&index| index == self.active) else {
            return;
        };
        let by = if forward { 1 } else { count - 1 };
        self.select(order[(at + by) % count], window, cx);
    }

    pub(super) fn session_order(&self, cx: &App) -> Vec<usize> {
        if !self.party && SettingsStore::get(cx).sidebar_style == SidebarStyle::Compact {
            groups(self.sessions.iter().map(|session| session.folder.as_path()))
                .into_values()
                .flatten()
                .collect()
        } else {
            (0..self.sessions.len()).collect()
        }
    }

    pub(super) fn session_list(&mut self, now: Instant, cx: &Context<Self>) -> AnyElement {
        let home = dirs::home_dir();
        let workspace = cx.entity();
        let row = |index| {
            sessions::row(
                &workspace,
                index,
                &self.sessions[index],
                index == self.active,
                (home.as_deref(), now),
                cx,
            )
        };
        if SettingsStore::get(cx).sidebar_style == SidebarStyle::Characters {
            return List::new()
                .divided()
                .children((0..self.sessions.len()).map(row))
                .into_any_element();
        }
        let groups = groups(self.sessions.iter().map(|session| session.folder.as_path()));
        self.sidebar
            .collapsed
            .retain(|folder| groups.contains_key(folder));
        if let Some(session) = self.sessions.get(self.active) {
            let shown = (session.id, session.folder.clone());
            if self.sidebar.shown.as_ref() != Some(&shown) {
                self.sidebar.reveal(&session.folder);
                self.sidebar.shown = Some(shown);
            }
        }
        let theme = cx.theme();
        div()
            .flex()
            .flex_col()
            .gap_2()
            .children(groups.iter().map(|(folder, indices)| {
                let open = !self.sidebar.collapsed.contains(folder);
                let duplicate = groups
                    .keys()
                    .filter(|other| other.file_name() == folder.file_name())
                    .count()
                    > 1;
                let name = label(folder, home.as_deref(), duplicate);
                let path = folder.clone();
                let state = indices
                    .iter()
                    .map(|&index| self.sessions[index].task_state(cx))
                    .min();
                let header = div()
                    .id(gpui::SharedString::from(format!(
                        "session-folder:{}",
                        folder.display()
                    )))
                    .role(Role::Button)
                    .aria_label(name.clone())
                    .aria_expanded(open)
                    .tab_index(0)
                    .min_h(theme.control_height(ControlSize::Md))
                    .px_2()
                    .py_1()
                    .flex()
                    .items_center()
                    .gap_2()
                    .rounded(theme.radius(Radius::Md))
                    .text_size(theme.text_size(TextSize::Sm))
                    .text_color(theme.colors.fg_muted)
                    .cursor_pointer()
                    .hover(|style| style.bg(theme.colors.hover))
                    .focus_ring(cx)
                    .tooltip(Tooltip::text(sessions::shown(folder, home.as_deref())))
                    .on_mouse_down(MouseButton::Left, |_, window, _| window.prevent_default())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        if !this.sidebar.collapsed.remove(&path) {
                            this.sidebar.collapsed.insert(path.clone());
                        }
                        log::info!(
                            "sessions: folder {} collapsed={}",
                            path.display(),
                            this.sidebar.collapsed.contains(&path)
                        );
                        cx.notify();
                    }))
                    .on_drop(|_: &sessions::Moving, _, cx| cx.stop_propagation())
                    .child(
                        Icon::new(if open {
                            IconName::ChevronDown
                        } else {
                            IconName::ChevronRight
                        })
                        .size(IconSize::Xs),
                    )
                    .child(Icon::new(IconName::Folder).size(IconSize::Sm))
                    .child(div().flex_1().min_w_0().child(Ellipsis::new(name)))
                    .children(state.filter(|state| state.needs_attention()).map(|state| {
                        Icon::new(IconName::CircleDot)
                            .size(IconSize::Xs)
                            .color(state.color(&theme.colors))
                    }))
                    .child(indices.len().to_string());
                div().child(header).when(open, |group| {
                    group.child(div().pl_4().children(indices.iter().copied().map(row)))
                })
            }))
            .into_any_element()
    }

    pub(crate) fn sidebar_drop_allowed(&self, from: usize, to: usize, cx: &App) -> bool {
        if SettingsStore::get(cx).sidebar_style == SidebarStyle::Characters {
            return true;
        }
        self.sessions
            .get(from)
            .zip(self.sessions.get(to))
            .is_some_and(|(from, to)| from.folder == to.folder)
    }

    pub(crate) fn move_sidebar_session(&mut self, from: usize, to: usize, cx: &mut Context<Self>) {
        if SettingsStore::get(cx).sidebar_style == SidebarStyle::Characters {
            self.move_session(from, to, cx);
            return;
        }
        let Some(session) = self.sessions.get(from) else {
            return;
        };
        let to = if to == usize::MAX {
            self.sessions
                .iter()
                .rposition(|other| other.folder == session.folder)
                .unwrap_or(from)
        } else {
            to
        };
        if self.sidebar_drop_allowed(from, to, cx) {
            self.move_session(from, to, cx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_nonadjacent_sessions_by_full_path_and_keeps_their_order() {
        let paths = ["/work/api", "/other/api", "/work/api", "/work/web"];
        let grouped = groups(paths.iter().map(Path::new));
        assert_eq!(grouped.len(), 3);
        assert_eq!(grouped[Path::new("/work/api")], [0, 2]);
        assert_eq!(grouped[Path::new("/other/api")], [1]);
        assert_eq!(
            grouped.into_values().flatten().collect::<Vec<_>>(),
            [1, 0, 2, 3]
        );
        assert_eq!(label(Path::new("/work/api"), None, true), "/work/api");
        assert_eq!(label(Path::new("/work/web"), None, false), "web");
    }

    #[test]
    fn home_and_root_have_names_and_empty_lists_have_no_groups() {
        assert_eq!(
            label(Path::new("/home/me"), Some(Path::new("/home/me")), false),
            "~"
        );
        assert!(!label(Path::new("/"), None, false).is_empty());
        assert!(groups(std::iter::empty()).is_empty());
    }

    #[test]
    fn selecting_a_session_reveals_only_its_folder() {
        let mut sidebar = Sidebar::default();
        sidebar
            .collapsed
            .extend([PathBuf::from("/a"), PathBuf::from("/b")]);
        sidebar.reveal(Path::new("/b"));
        assert_eq!(sidebar.collapsed, BTreeSet::from([PathBuf::from("/a")]));
    }
}
