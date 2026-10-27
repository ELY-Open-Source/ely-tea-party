use ely_gpui_component::{
    buttons::{ButtonVariant, IconButton},
    i18n::text,
    primitives::{Icon, IconName},
    theme::{ActiveTheme, ControlSize, Elevation, IconSize, Radius},
};
use gpui::{AnyElement, Context, FontWeight, MouseButton, Window, div, prelude::*, px, rems};

use crate::workspace::Workspace;

impl Workspace {
    pub(in crate::workspace) fn companion_bubble(
        &self,
        window: &Window,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let reply = &self.companion.reply;
        if !reply.visible {
            return None;
        }
        let theme = cx.theme();
        let anchor = reply.anchor;
        let placed = anchor.place(
            window.viewport_size().width.into(),
            window.viewport_size().height.into(),
            window.rem_size().into(),
        );
        Some(
            div()
                .id("whale-companion")
                .occlude()
                .absolute()
                .left(px(placed.left))
                .w(px(placed.width.min(f32::from(window.rem_size()) * 23.)))
                .when(anchor.bottom, |bubble| bubble.bottom(px(placed.offset)))
                .when(!anchor.bottom, |bubble| bubble.top(px(placed.offset)))
                .p_3()
                .flex()
                .flex_col()
                .gap_2()
                .rounded(theme.radius(Radius::Xl))
                .bg(theme.colors.overlay)
                .border_1()
                .border_color(theme.colors.border)
                .shadow(theme.elevation(Elevation::Floating))
                .text_color(theme.colors.fg)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(Icon::new(IconName::Sparkles).size(IconSize::Sm))
                        .child(
                            div()
                                .flex_1()
                                .text_xs()
                                .font_weight(FontWeight::MEDIUM)
                                .child(text(cx, "et.butler.idea_title", &[])),
                        )
                        .child(
                            IconButton::new("companion-close", IconName::X)
                                .size(ControlSize::Sm)
                                .variant(ButtonVariant::Ghost)
                                .tooltip(text(cx, "et.butler.close", &[]))
                                .on_click(cx.listener(|this, _, _, cx| this.quiet_companion(cx))),
                        ),
                )
                .child(
                    div()
                        .text_sm()
                        .line_height(rems(1.4))
                        .max_h(px(placed.content_height.max(0.) * 0.6))
                        .id("companion-text")
                        .overflow_y_scroll()
                        .child(reply.text.clone()),
                )
                .child(
                    div()
                        .id("companion-feedback")
                        .text_xs()
                        .cursor_pointer()
                        .child(text(cx, "et.memory.reply", &[]))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _, _, cx| {
                                let folder =
                                    this.sessions.get(this.active).map(|s| s.folder.clone());
                                let feedback = Some((
                                    this.companion.reply.text.clone(),
                                    this.companion.suggestion.clone(),
                                ));
                                crate::memory::window::open(folder, feedback, cx);
                            }),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .text_xs()
                        .text_color(theme.colors.fg_muted)
                        .children(reply.tokens.map(|count| {
                            text(cx, "et.butler.tokens", &[("count", &count.to_string())])
                        }))
                        .child(
                            div()
                                .id("companion-mute")
                                .cursor_pointer()
                                .child(text(cx, "et.butler.idea_mute", &[]))
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|this, _, _, cx| {
                                        this.companion.muted = true;
                                        this.quiet_companion(cx);
                                        log::info!("companion: muted until this window closes");
                                    }),
                                ),
                        ),
                )
                .into_any_element(),
        )
    }
}
