//! A split session's panes as saved: their layout, programs and folders.

use std::path::{Path, PathBuf};

use gpui::{Axis, EntityId};
use serde::{Deserialize, Serialize};

use crate::sessions::{Layout, Session};

/// A split session's panes: their layout, its first pane's place among them, and the focused one's.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Panes {
    pub layout: SavedLayout,
    pub main: usize,
    pub focused: usize,
}

/// A pane's program and folder, or a split: `down` stacks its parts, the first taking `share`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum SavedLayout {
    Pane {
        shell: String,
        args: Vec<String>,
        folder: PathBuf,
    },
    Split {
        down: bool,
        share: f32,
        first: Box<SavedLayout>,
        second: Box<SavedLayout>,
    },
}

impl SavedLayout {
    /// Its panes, left to right and top to bottom: each program, its arguments and its folder.
    pub fn panes(&self) -> Vec<(&str, &[String], &Path)> {
        match self {
            SavedLayout::Pane {
                shell,
                args,
                folder,
            } => vec![(shell, args, folder)],
            SavedLayout::Split { first, second, .. } => {
                let mut panes = first.panes();
                panes.extend(second.panes());
                panes
            }
        }
    }
}

/// `layout`'s panes as saved; a pane `pane` leaves out goes, and a split losing a part becomes the other.
fn layout_saved(
    layout: &Layout,
    pane: &impl Fn(EntityId) -> Option<SavedLayout>,
) -> Option<SavedLayout> {
    match layout {
        Layout::Pane(id) => pane(*id),
        Layout::Split(axis, share, first, second) => {
            match (layout_saved(first, pane), layout_saved(second, pane)) {
                (Some(first), Some(second)) => Some(SavedLayout::Split {
                    down: *axis == Axis::Vertical,
                    share: *share,
                    first: Box::new(first),
                    second: Box::new(second),
                }),
                (one, other) => one.or(other),
            }
        }
    }
}

/// `saved` with its panes' terminals `ids`, in their order; a pane that did not start is left out.
pub(super) fn layout_of(
    saved: &SavedLayout,
    ids: &mut impl Iterator<Item = Option<EntityId>>,
) -> Option<Layout> {
    match saved {
        SavedLayout::Pane { .. } => ids.next().flatten().map(Layout::Pane),
        SavedLayout::Split {
            down,
            share,
            first,
            second,
        } => {
            let axis = if *down {
                Axis::Vertical
            } else {
                Axis::Horizontal
            };
            match (layout_of(first, ids), layout_of(second, ids)) {
                (Some(first), Some(second)) => Some(Layout::Split(
                    axis,
                    share.clamp(0.0, 1.0),
                    Box::new(first),
                    Box::new(second),
                )),
                (one, other) => one.or(other),
            }
        }
    }
}

/// Session `session`'s panes as saved, while more than its first stay: a pane `-e` started is left out, its first pane never.
pub(super) fn panes_saved(session: &Session) -> Option<Panes> {
    let main = session.main().id();
    let pane = |id: EntityId| {
        let pane = session.panes.iter().find(|pane| pane.id() == id)?;
        (id == main || !pane.program.written).then(|| SavedLayout::Pane {
            shell: pane.program.shell.clone(),
            args: pane.program.args.clone(),
            folder: pane.folder.clone(),
        })
    };
    let layout = layout_saved(&session.layout, &pane)?;
    let kept: Vec<EntityId> = (session.layout.order().into_iter())
        .filter(|id| pane(*id).is_some())
        .collect();
    if kept.len() < 2 {
        return None;
    }
    let at = |id: EntityId| kept.iter().position(|each| *each == id);
    Some(Panes {
        layout,
        main: at(main)?,
        focused: at(session.pane().id()).unwrap_or(0),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pane(folder: &str) -> SavedLayout {
        SavedLayout::Pane {
            shell: "zsh".into(),
            args: Vec::new(),
            folder: folder.into(),
        }
    }

    #[test]
    fn a_layout_saved_opens_as_it_was_less_the_panes_that_did_not_start() {
        let id = |n: u64| EntityId::from(1 << 32 | n);
        let layout = Layout::Split(
            Axis::Horizontal,
            0.3,
            Box::new(Layout::Pane(id(1))),
            Box::new(Layout::Split(
                Axis::Vertical,
                0.6,
                Box::new(Layout::Pane(id(2))),
                Box::new(Layout::Pane(id(3))),
            )),
        );
        let named = |of: EntityId| (of != id(2)).then(|| pane("/"));
        let saved = layout_saved(&layout, &named).expect("two panes stay");
        assert_eq!(saved.panes().len(), 2, "-e's pane is left out");
        let back =
            layout_of(&saved, &mut [Some(id(1)), Some(id(3))].into_iter()).expect("both start");
        let want = Layout::Split(
            Axis::Horizontal,
            0.3,
            Box::new(Layout::Pane(id(1))),
            Box::new(Layout::Pane(id(3))),
        );
        assert_eq!(back, want);
        let one = layout_of(&saved, &mut [None, Some(id(3))].into_iter());
        assert_eq!(one, Some(Layout::Pane(id(3))), "a pane that did not start");
    }
}
