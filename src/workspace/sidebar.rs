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

/// A folder's place in the compact list: the named group it sits in, none for the ungrouped, and the folder.
type Spot = (Option<u64>, PathBuf);

#[derive(Default)]
pub(super) struct Sidebar {
    collapsed: BTreeSet<Spot>,
    shown: Option<(u64, Spot)>,
}

impl Sidebar {
    pub(super) fn reveal(&mut self, group: Option<u64>, folder: &Path) {
        if self.collapsed.remove(&(group, folder.to_path_buf())) {
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

    /// The sessions in their rows' order: block by block, and in the compact list folder by folder inside each.
    pub(super) fn session_order(&self, cx: &App) -> Vec<usize> {
        if !self.party && SettingsStore::get(cx).sidebar_style == SidebarStyle::Compact {
            self.blocks()
                .into_iter()
                .flat_map(|(_, places)| {
                    let start = places.start;
                    groups(
                        self.sessions[places]
                            .iter()
                            .map(|session| session.main().folder.as_path()),
                    )
                    .into_values()
                    .flatten()
                    .map(move |index| start + index)
                    .collect::<Vec<_>>()
                })
                .collect()
        } else {
            (0..self.sessions.len()).collect()
        }
    }

    /// The sessions' rows, block by block under each named group's header; the compact list groups each block's rows by folder.
    pub(super) fn session_list(&mut self, now: Instant, cx: &mut Context<Self>) -> AnyElement {
        let home = dirs::home_dir();
        let workspace = cx.entity();
        let named: Vec<(u64, String)> = (self.groups.iter())
            .map(|group| (group.id, group.name.clone()))
            .collect();
        let characters = SettingsStore::get(cx).sidebar_style == SidebarStyle::Characters;
        if !characters {
            self.fold();
        }
        let mut list = div().flex().flex_col().gap_2();
        for (group, places) in self.blocks() {
            let header = group
                .and_then(|id| self.groups.iter().find(|each| each.id == id))
                .map(|found| self.header(found, cx));
            let row = |index: usize| {
                sessions::row(
                    &workspace,
                    index,
                    &self.sessions[index],
                    index == self.active,
                    (home.as_deref(), now),
                    &named,
                    cx,
                )
                .into_any_element()
            };
            let block = if characters {
                List::new()
                    .divided()
                    .children(places.map(row))
                    .into_any_element()
            } else {
                self.folders(group, places, home.as_deref(), &row, cx)
            };
            list = list.child(div().children(header).child(block));
        }
        list.into_any_element()
    }

    /// Keeps the compact list's collapsed folders to those still listed, and opens the shown session's.
    fn fold(&mut self) {
        let spots: BTreeSet<Spot> = (self.sessions.iter())
            .map(|session| (session.group, session.main().folder.clone()))
            .collect();
        self.sidebar.collapsed.retain(|spot| spots.contains(spot));
        if let Some(session) = self.sessions.get(self.active) {
            let shown = (session.id, (session.group, session.main().folder.clone()));
            if self.sidebar.shown.as_ref() != Some(&shown) {
                self.sidebar.reveal(session.group, &session.main().folder);
                self.sidebar.shown = Some(shown);
            }
        }
    }

    /// The rows at `places`, of named group `group`, under a header for each folder they sit in.
    fn folders(
        &self,
        group: Option<u64>,
        places: std::ops::Range<usize>,
        home: Option<&Path>,
        row: &impl Fn(usize) -> AnyElement,
        cx: &Context<Self>,
    ) -> AnyElement {
        let start = places.start;
        let groups = groups(
            self.sessions[places]
                .iter()
                .map(|session| session.main().folder.as_path()),
        );
        let theme = cx.theme();
        div()
            .flex()
            .flex_col()
            .gap_2()
            .children(groups.iter().map(|(folder, indices)| {
                let indices: Vec<usize> = indices.iter().map(|index| start + index).collect();
                let spot = (group, folder.clone());
                let open = !self.sidebar.collapsed.contains(&spot);
                let duplicate = groups
                    .keys()
                    .filter(|other| other.file_name() == folder.file_name())
                    .count()
                    > 1;
                let name = label(folder, home, duplicate);
                let state = indices
                    .iter()
                    .map(|&index| self.sessions[index].task_state(cx))
                    .min();
                let header = div()
                    .id(gpui::SharedString::from(format!(
                        "session-folder:{}:{}",
                        group.unwrap_or(0),
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
                    .tooltip(Tooltip::text(sessions::shown(folder, home)))
                    .on_mouse_down(MouseButton::Left, |_, window, _| window.prevent_default())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        if !this.sidebar.collapsed.remove(&spot) {
                            this.sidebar.collapsed.insert(spot.clone());
                        }
                        log::info!(
                            "sessions: folder {} collapsed={}",
                            spot.1.display(),
                            this.sidebar.collapsed.contains(&spot)
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
                div().child(header).when(open, |block| {
                    block.child(div().pl_4().children(indices.iter().copied().map(row)))
                })
            }))
            .into_any_element()
    }

    /// Whether row `from` may land at row `to`: inside its own group, and in the compact list inside its own folder too.
    pub(crate) fn sidebar_drop_allowed(&self, from: usize, to: usize, cx: &App) -> bool {
        let characters = SettingsStore::get(cx).sidebar_style == SidebarStyle::Characters;
        self.sessions
            .get(from)
            .zip(self.sessions.get(to))
            .is_some_and(|(from, to)| {
                from.group == to.group && (characters || from.main().folder == to.main().folder)
            })
    }

    /// Moves row `from` to row `to`, or below the last of its kind for `usize::MAX`, where the drop is allowed.
    pub(crate) fn move_sidebar_session(&mut self, from: usize, to: usize, cx: &mut Context<Self>) {
        let characters = SettingsStore::get(cx).sidebar_style == SidebarStyle::Characters;
        let Some(session) = self.sessions.get(from) else {
            return;
        };
        let to = if to == usize::MAX {
            self.sessions
                .iter()
                .rposition(|other| {
                    other.group == session.group
                        && (characters || other.main().folder == session.main().folder)
                })
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
        sidebar.collapsed.extend([
            (None, PathBuf::from("/a")),
            (None, PathBuf::from("/b")),
            (Some(1), PathBuf::from("/b")),
        ]);
        sidebar.reveal(None, Path::new("/b"));
        assert_eq!(
            sidebar.collapsed,
            BTreeSet::from([(None, PathBuf::from("/a")), (Some(1), PathBuf::from("/b"))]),
            "the same folder in a named group stays as it was"
        );
    }
}
