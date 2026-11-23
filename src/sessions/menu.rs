use ely_gpui_component::{
    i18n::text,
    menus::{Menu, MenuItem},
    primitives::IconName,
};
use gpui::{App, ClipboardItem, Entity};

use crate::workspace::Workspace;

/// What a right press on the row of session `id`, in group `own`, offers among `groups`.
pub fn row(
    workspace: &Entity<Workspace>,
    (id, own): (u64, Option<u64>),
    groups: &[(u64, String)],
    cx: &App,
) -> Menu {
    let (duplicate, close, made) = (workspace.clone(), workspace.clone(), workspace.clone());
    let link = workspace.clone();
    let mut menu = Menu::new()
        .item(
            MenuItem::new(text(cx, "et.session.copy_link", &[]))
                .icon(IconName::Link)
                .on_click(move |_, cx| {
                    if let Some(session) = link
                        .read(cx)
                        .sessions()
                        .iter()
                        .find(|session| session.id == id)
                    {
                        cx.write_to_clipboard(ClipboardItem::new_string(session.link_id.url()));
                    }
                }),
        )
        .item(
            MenuItem::new(text(cx, "et.session.duplicate", &[]))
                .icon(IconName::Copy)
                .on_click(move |window, cx| {
                    duplicate.update(cx, |this, cx| this.duplicate(id, window, cx))
                }),
        )
        .separator()
        .item(
            MenuItem::new(text(cx, "et.group.new", &[]))
                .icon(IconName::FolderPlus)
                .on_click(move |window, cx| {
                    made.update(cx, |this, cx| this.new_group(id, window, cx))
                }),
        );
    for (group, name) in groups.iter().filter(|(group, _)| Some(*group) != own) {
        let (workspace, group) = (workspace.clone(), *group);
        menu = menu.item(
            MenuItem::new(text(cx, "et.group.move", &[("name", name)]))
                .icon(IconName::Folder)
                .on_click(move |window, cx| {
                    workspace.update(cx, |this, cx| this.join_group(id, Some(group), window, cx))
                }),
        );
    }
    if own.is_some() {
        let workspace = workspace.clone();
        menu = menu.item(
            MenuItem::new(text(cx, "et.group.leave", &[]))
                .icon(IconName::Minus)
                .on_click(move |window, cx| {
                    workspace.update(cx, |this, cx| this.join_group(id, None, window, cx))
                }),
        );
    }
    menu.separator().item(
        MenuItem::new(text(cx, "et.session.close", &[]))
            .icon(IconName::X)
            .on_click(move |window, cx| {
                close.update(cx, |this, cx| {
                    let found = this.sessions().iter().position(|session| session.id == id);
                    if let Some(index) = found {
                        this.close_asked(index, None, window, cx);
                    }
                })
            }),
    )
}

/// What a right press on group `group`'s header offers.
pub fn group(workspace: &Entity<Workspace>, group: u64, cx: &App) -> Menu {
    let (rename, ungroup) = (workspace.clone(), workspace.clone());
    Menu::new()
        .item(
            MenuItem::new(text(cx, "et.group.rename", &[]))
                .icon(IconName::Pencil)
                .on_click(move |window, cx| {
                    rename.update(cx, |this, cx| this.rename_group(group, window, cx))
                }),
        )
        .item(
            MenuItem::new(text(cx, "et.group.ungroup", &[]))
                .icon(IconName::Minus)
                .on_click(move |window, cx| {
                    ungroup.update(cx, |this, cx| this.ungroup(group, window, cx))
                }),
        )
}
