use ely_gpui_component::{
    menus::{Menu, MenuItem},
    primitives::IconName,
};
use gpui::{Entity, EntityId};

use crate::workspace::Workspace;

/// What a right press on the row of session `id` offers.
pub fn row(workspace: &Entity<Workspace>, id: EntityId) -> Menu {
    let (duplicate, close) = (workspace.clone(), workspace.clone());
    Menu::new()
        .item(
            MenuItem::new("Duplicate session")
                .icon(IconName::Copy)
                .on_click(move |window, cx| {
                    duplicate.update(cx, |this, cx| this.duplicate(id, window, cx))
                }),
        )
        .separator()
        .item(
            MenuItem::new("Close session")
                .icon(IconName::X)
                .on_click(move |window, cx| {
                    close.update(cx, |this, cx| {
                        let found = this
                            .sessions()
                            .iter()
                            .position(|session| session.terminal.entity_id() == id);
                        if let Some(index) = found {
                            this.close(index, window, cx);
                        }
                    })
                }),
        )
}
