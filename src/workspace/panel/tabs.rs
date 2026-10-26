//! The right column's tabs in its header, each in its own color; the lit one glides to the tab chosen next.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use ely_gpui_component::{
    i18n::text,
    motion::{self, ease_out_cubic, lerp},
    theme::{ActiveTheme, ControlSize, Mix, Palette, Radius, TextSize},
};
use gpui::{
    Animation, AnimationExt, App, Bounds, Div, Hsla, InteractiveElement, MouseButton,
    ParentElement, Pixels, StatefulInteractiveElement, Styled, div, prelude::FluentBuilder, px,
};

use super::super::header::show_tab;
use crate::settings::PanelTab;

const TABS: [(PanelTab, &str, &str); 3] = [
    (PanelTab::Git, "git", "et.panel.git"),
    (PanelTab::Files, "files", "et.panel.files"),
    (PanelTab::Activity, "activity", "et.panel.activity"),
];

/// The tab last shown, the one a glide leaves, how many glides there were, and the tabs' places as last laid out.
#[derive(Default)]
pub struct Glide {
    shown: Cell<Option<PanelTab>>,
    from: Cell<Option<PanelTab>>,
    turn: Cell<usize>,
    places: Rc<RefCell<Vec<Bounds<Pixels>>>>,
}

impl Glide {
    /// Notes `shown`, a glide starting when it changed; gives how many glides there were.
    pub fn note(&self, shown: PanelTab) -> usize {
        if let Some(last) = self.shown.replace(Some(shown))
            && last != shown
        {
            self.from.set(Some(last));
            self.turn.set(self.turn.get() + 1);
        }
        self.turn.get()
    }
}

/// Each tab's own color: Git amber, Files blue, Activity purple.
fn hue(tab: PanelTab, colors: &Palette) -> Hsla {
    match tab {
        PanelTab::Git => colors.warning,
        PanelTab::Files => colors.info,
        PanelTab::Activity => colors.syntax.keyword,
    }
}

/// The tabs, the shown one's name in its color over a wash of it, which glides from the tab shown before.
pub fn tabs((shown, turn): (PanelTab, usize), glide: &Glide, cx: &App) -> Div {
    let theme = cx.theme();
    let colors = &theme.colors;
    let places = glide.places.borrow().clone();
    let laid = places.len() == TABS.len();
    let tab = |(tab, value, label): (PanelTab, &'static str, &'static str)| {
        let on = tab == shown;
        let color = hue(tab, colors);
        div()
            .id(("panel-tab", tab as usize))
            .flex()
            .items_center()
            .px_2()
            .h(theme.control_height(ControlSize::Sm))
            .rounded(theme.radius(Radius::Md))
            .text_size(theme.text_size(TextSize::Sm))
            .map(|tab| match on {
                true => tab.text_color(color),
                false => tab
                    .text_color(colors.fg_muted)
                    .cursor_pointer()
                    .hover(|style| style.bg(colors.hover).text_color(color)),
            })
            // Until the tabs are laid out, the shown one holds its own wash.
            .when(on && !laid, |tab| tab.bg(wash(color)))
            // A press here is the tab's, not the title bar's drag.
            .on_mouse_down(MouseButton::Left, |_, window, _| window.prevent_default())
            .on_click(move |_, _, cx| show_tab(&value.into(), cx))
            .child(text(cx, label, &[]))
    };
    let index = |tab: PanelTab| TABS.iter().position(|(each, ..)| *each == tab);
    let lit = laid.then(|| {
        let to = index(shown).expect("the shown tab is one of the tabs");
        let from = glide.from.get().and_then(index).unwrap_or(to);
        let at = |ix: usize| {
            let place = places[ix];
            (
                f32::from(place.origin.x - places[0].origin.x),
                f32::from(place.size.width),
            )
        };
        let ((from_x, from_w), (to_x, to_w)) = (at(from), at(to));
        let (from_hue, to_hue) = (hue(TABS[from].0, colors), hue(TABS[to].0, colors));
        div()
            .absolute()
            .top_0()
            .bottom_0()
            .rounded(theme.radius(Radius::Md))
            .with_animation(
                ("panel-tab-glide", turn),
                Animation::new(motion::duration(motion::SLOW, cx)).with_easing(ease_out_cubic),
                move |lit, t| {
                    lit.left(px(lerp(from_x, to_x, t)))
                        .w(px(lerp(from_w, to_w, t)))
                        .bg(wash(from_hue.mix(&to_hue, t)))
                },
            )
    });
    let kept = glide.places.clone();
    div()
        .relative()
        .flex()
        .items_center()
        .gap_0p5()
        // The wash first, so the names draw over it.
        .children(lit)
        .children(TABS.map(tab))
        .on_children_prepainted(move |bounds, window, _| {
            let tabs: Vec<Bounds<Pixels>> =
                bounds.into_iter().rev().take(TABS.len()).rev().collect();
            if *kept.borrow() != tabs {
                *kept.borrow_mut() = tabs;
                window.request_animation_frame();
            }
        })
}

/// A tab's color, faint enough to lie under its name.
fn wash(color: Hsla) -> Hsla {
    color.opacity(0.16)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_glide_starts_only_when_the_shown_tab_changes() {
        let glide = Glide::default();
        assert_eq!(
            glide.note(PanelTab::Git),
            0,
            "the first tab shown glides from nowhere"
        );
        assert_eq!(glide.note(PanelTab::Git), 0);
        assert_eq!(glide.note(PanelTab::Activity), 1);
        assert_eq!(glide.from.get(), Some(PanelTab::Git));
        assert_eq!(glide.note(PanelTab::Activity), 1, "a redraw starts none");
    }
}
