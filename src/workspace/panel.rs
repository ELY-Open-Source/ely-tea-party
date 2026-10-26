//! The right column: Git, Files, or the shown session's Activity, one at a time.

use ely_gpui_component::{
    i18n::text,
    motion::{self, ease_out_cubic},
    shell::ShellColumn,
    typography::Caption,
};
use gpui::{
    Animation, AnimationExt, Context, Div, IntoElement, ParentElement, StyleRefinement, Styled,
    div, prelude::FluentBuilder,
};

use super::{Side, Workspace};
use crate::settings::PanelTab;

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
            .leading(tabs::tabs((tab, turn), &self.glide, cx))
            .action(end)
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
}
