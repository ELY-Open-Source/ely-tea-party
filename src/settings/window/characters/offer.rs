//! The pack a market link brought, offered for the character the page shows.

use ely_gpui_component::{
    buttons::Button,
    feedback::InlineMessage,
    i18n::text,
    primitives::Severity,
    settings::{SettingsRow, SettingsSection},
    theme::ControlSize,
};
use gpui::{AnyElement, App, IntoElement, ParentElement, Styled, div};

use super::{character_name, jobs};
use crate::skins::{
    Library, Slot,
    market::{self, Market, Offer},
};

pub(super) fn section(slot: Slot, cx: &App) -> Option<AnyElement> {
    let offer = cx.try_global::<Market>()?.offer.as_ref()?;
    let dismiss = || {
        Button::new("market-dismiss", text(cx, "et.market.dismiss", &[]))
            .size(ControlSize::Sm)
            .on_click(|_, _, cx| market::set(None, cx))
    };
    Some(match offer {
        Offer::Fetching(id) => InlineMessage::new(
            Severity::Info,
            text(cx, "et.market.fetching", &[("id", id)]),
        )
        .into_any_element(),
        Offer::Failed(error) => div()
            .flex()
            .flex_col()
            .gap_2()
            .child(InlineMessage::new(
                Severity::Danger,
                text(cx, "et.market.failed", &[("error", error)]),
            ))
            .child(div().child(dismiss()))
            .into_any_element(),
        Offer::Ready(ready) => {
            let fits = ready.fits(slot);
            let character = character_name(slot, cx);
            let hint = if fits {
                text(cx, "et.market.to", &[("character", &character)])
            } else if ready.target == Slot::Whale.id() {
                text(cx, "et.market.whale_only", &[])
            } else {
                text(cx, "et.market.agent_only", &[])
            };
            let path = ready.path.clone();
            let id = ready.id.clone();
            let install = Button::new("market-install", text(cx, "et.market.install", &[]))
                .primary()
                .size(ControlSize::Sm)
                .disabled(!fits || cx.global::<Library>().busy)
                .on_click(move |_, _, cx| {
                    log::info!("market: installing {id} on {}", slot.id());
                    market::set(None, cx);
                    jobs::start(slot, jobs::Action::Install(path.clone()), cx);
                });
            let title = text(
                cx,
                "et.market.offer",
                &[("name", &ready.name), ("author", &ready.author)],
            );
            SettingsSection::new(text(cx, "et.market.title", &[]))
                .row(
                    SettingsRow::new(title)
                        .description(hint)
                        .control(div().flex().gap_2().child(install).child(dismiss())),
                )
                .into_any_element()
        }
    })
}
