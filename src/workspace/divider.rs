use ely_gpui_component::theme::{ActiveTheme, Mix};
use gpui::{App, Bounds, IntoElement, Styled, canvas, fill, point, prelude::FluentBuilder, size};

use super::widths::Side;

/// Paints over the edge without intercepting the column's resize handle.
pub(super) fn sidebar(side: Side, cx: &App) -> impl IntoElement {
    let theme = cx.theme();
    let (line, lit) = (color(cx), theme.colors.focus);
    let stroke = theme.hairline() * 2.;
    let grip = theme.grip();
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let color = if bounds.contains(&window.mouse_position()) {
                lit
            } else {
                line
            };
            let width = grip.height.to_pixels(window.rem_size());
            let height = grip
                .width
                .to_pixels(window.rem_size())
                .min(bounds.size.height);
            let x = |width| match side {
                Side::Sessions => bounds.right() - width,
                Side::Panel => bounds.left(),
            };
            window.paint_quad(fill(
                Bounds::new(
                    point(x(stroke), bounds.top()),
                    size(stroke, bounds.size.height),
                ),
                color,
            ));
            window.paint_quad(fill(
                Bounds::new(
                    point(x(width), bounds.center().y - height / 2.),
                    size(width, height),
                ),
                color,
            ));
        },
    )
    .absolute()
    .top(-theme.titlebar_height())
    .bottom_0()
    .map(|edge| match side {
        Side::Sessions => edge.right_0(),
        Side::Panel => edge.left_0(),
    })
    .w(theme.handle_hit())
}

pub(super) fn color(cx: &App) -> gpui::Hsla {
    let colors = &cx.theme().colors;
    colors.border_strong.mix(&colors.fg_muted, 0.35)
}
