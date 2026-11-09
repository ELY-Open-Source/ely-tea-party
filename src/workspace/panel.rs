//! The right column: Git, Files, or the shown session's Activity, one at a time.

use ely_gpui_component::{
    buttons::{Button, ButtonVariant},
    i18n::text,
    layout::Collapsible,
    motion::{self, ease_out_cubic},
    primitives::IconName,
    shell::ShellColumn,
    theme::ControlSize,
    typography::Caption,
};
use gpui::{
    Animation, AnimationExt, Context, Div, Focusable, IntoElement, ParentElement, StyleRefinement,
    Styled, div, prelude::FluentBuilder,
};

use super::{Side, Workspace, header::show_tab};
use crate::settings::{PanelTab, SettingsStore};

mod activity;
mod tabs;

pub use tabs::Glide;

impl Workspace {
    /// The right column, ending in `end`, the title bar's last controls.
    pub(super) fn panel(&self, tab: PanelTab, end: Div, cx: &mut Context<Self>) -> ShellColumn {
        let turn = self.glide.note(tab);
        let fading = motion::duration(motion::BASE, cx);
        // The body chosen fades in.
        let fade = |body: Div, name: &'static str| match turn {
            0 => body.into_any_element(),
            _ => body
                .with_animation(
                    (name, turn),
                    Animation::new(fading).with_easing(ease_out_cubic),
                    |body, t| body.opacity(t),
                )
                .into_any_element(),
        };
        let session = self.sessions.get(self.active);
        let quiet = tab != PanelTab::Activity && session.is_some_and(|session| !session.reports);
        let caption = || {
            Caption::new(text(cx, "et.panel.no_folder", &[]))
                .px_3()
                .pt_2()
        };
        // Hidden, not dropped, so each keeps its state.
        let body = |on: bool| {
            div()
                .flex_1()
                .min_h_0()
                .p_2()
                .when(!on, |body| body.hidden())
        };
        // Git's list draws every row, so terminal output must not redraw it.
        let fill = || StyleRefinement::default().size_full();
        self.side_column(Side::Panel, cx)
            .leading(tabs::tabs((tab, turn), &self.glide, &cx.entity(), cx))
            .action(end)
            .child(div().flex_none().child(self.folder_actions(cx)))
            .children(quiet.then(caption))
            .child(fade(
                body(tab == PanelTab::Git).child(self.git.clone().cached(fill())),
                "git-fade",
            ))
            .child(fade(
                body(tab == PanelTab::Files).child(self.files.clone().cached(fill())),
                "files-fade",
            ))
            .children((tab == PanelTab::Activity).then(|| {
                fade(
                    body(true).child(activity::activity(session, cx)),
                    "activity-fade",
                )
            }))
            .children(self.usage(cx))
            .child(super::divider::sidebar(Side::Panel, cx))
    }

    /// Shows Files, then shows or hides its folder row.
    pub(super) fn flip_folder_actions(&mut self, cx: &mut Context<Self>) {
        show_tab(&"files".into(), cx);
        if SettingsStore::get(cx).panel != PanelTab::Files {
            return;
        }
        self.folder_actions = !self.folder_actions;
        let shown = if self.folder_actions {
            "shown"
        } else {
            "hidden"
        };
        log::info!("files: folder actions {shown}");
        cx.notify();
    }

    pub(super) fn folder_actions(&self, cx: &Context<Self>) -> Collapsible {
        let settings = SettingsStore::get(cx);
        let shown = self.folder_actions && settings.files_menu && settings.panel == PanelTab::Files;
        let workspace = cx.entity();
        let choose = Button::new(
            "choose-session-folder",
            text(cx, "et.files.choose_folder", &[]),
        )
        .variant(ButtonVariant::Ghost)
        .size(ControlSize::Sm)
        .icon(IconName::Folder)
        .full_width()
        .disabled(self.choosing_folder)
        .on_click(move |_, window, cx| {
            workspace.update(cx, |this, cx| {
                this.folder_actions = false;
                log::info!("files: folder actions hidden for chooser");
                if let Some(session) = this.sessions.get(this.active) {
                    window.focus(&session.terminal.focus_handle(cx), cx);
                }
                cx.notify();
                this.choose_folder(window, cx);
            });
        });
        Collapsible::new("files-folder-actions", shown).child(div().px_2().pt_2().child(choose))
    }
}
