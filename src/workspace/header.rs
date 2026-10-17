//! The title bar's controls: the column toggles and the right column's tab.

use ely_gpui_component::{
    buttons::{ButtonVariant, IconButton},
    primitives::IconName,
    theme::ControlSize,
};
use gpui::{App, Entity, SharedString};

use super::Workspace;
use crate::settings::{PanelTab, Settings, SettingsStore};

/// Shows Git or Files in the right column, as the settings keep it.
pub(super) fn show_tab(value: &SharedString, cx: &mut App) {
    let panel = match value.as_ref() {
        "git" => PanelTab::Git,
        "files" => PanelTab::Files,
        "activity" => PanelTab::Activity,
        other => unreachable!("app: no {other} tab"),
    };
    if SettingsStore::get(cx).panel == panel {
        return;
    }
    match SettingsStore::update(cx, |settings| settings.panel = panel) {
        Ok(()) => log::info!("app: the right column shows {panel:?}"),
        Err(error) => log::error!("app: the right column stays: {error:#}"),
    }
}

/// A header button that hides or shows a column, saved.
pub(super) fn column_toggle(
    workspace: &Entity<Workspace>,
    (id, icon): (&'static str, IconName),
    tip: String,
    flip: fn(&mut Settings),
) -> IconButton {
    let workspace = workspace.clone();
    IconButton::new(id, icon)
        .variant(ButtonVariant::Ghost)
        .size(ControlSize::Sm)
        .tooltip(tip)
        .on_click(move |_, _, cx| workspace.update(cx, |this, cx| this.toggle(flip, cx)))
}
