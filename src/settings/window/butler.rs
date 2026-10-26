use ely_gpui_component::{
    buttons::{Button, ButtonVariant, SegmentedControl},
    forms::{Choice, Input, PasswordInput, Select, TextInput},
    i18n::text,
    settings::{SettingsRow, SettingsSection},
    theme::{ActiveTheme, ControlSize, Radius},
};
use gpui::{
    AppContext, BorrowAppContext, Context, Div, Entity, ParentElement, Styled, Window, div, rems,
};

use super::{SettingsWindow, set, switch};
use crate::{
    butler::{
        Credentials,
        config::{PROVIDERS, default_persona},
    },
    settings::SettingsStore,
};

pub(super) struct Fields {
    provider: String,
    model: String,
    key: Entity<TextInput>,
    persona: Entity<TextInput>,
}

impl SettingsWindow {
    pub(super) fn butler_page(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        if self.butler_fields.is_none() {
            let config = SettingsStore::get(cx).butler.clone();
            cx.default_global::<Credentials>();
            let key = cx
                .global::<Credentials>()
                .0
                .get(&config.provider)
                .cloned()
                .unwrap_or_default();
            self.butler_fields = Some(Fields {
                provider: config.provider,
                model: config.model,
                key: cx.new(|cx| {
                    let mut input = TextInput::new(window, cx).masked();
                    input.set_text(key, cx);
                    input
                }),
                persona: cx.new(|cx| {
                    let mut input = TextInput::new(window, cx)
                        .multi_line(4, 6)
                        .max_len(2_000)
                        .label(text(cx, "et.butler.persona", &[]))
                        .placeholder(default_persona(SettingsStore::get(cx).language.tag()));
                    let persona = if config.persona.is_empty() {
                        default_persona(SettingsStore::get(cx).language.tag()).to_string()
                    } else {
                        config.persona
                    };
                    input.set_text(persona, cx);
                    input
                }),
            });
        }
        let fields = self.butler_fields.as_ref().expect("fields initialized");
        let this = cx.entity();
        let word = |key| text(cx, key, &[]);
        let enabled = switch(
            &this,
            "butler-enabled",
            SettingsStore::get(cx).butler.enabled,
            |settings, enabled| settings.butler.enabled = enabled,
        );
        let provider = PROVIDERS
            .iter()
            .fold(
                SegmentedControl::new("butler-provider", fields.provider.clone()),
                |control, p| control.segment(p.id.clone(), p.label.clone(), None),
            )
            .size(ControlSize::Sm)
            .on_change({
                let this = this.clone();
                move |provider, _, cx| {
                    this.update(cx, |this, cx| {
                        let fields = this.butler_fields.as_mut().expect("fields initialized");
                        let entry = PROVIDERS
                            .iter()
                            .find(|p| p.id == provider.as_ref())
                            .expect("listed provider");
                        fields.provider = provider.to_string();
                        fields.model = entry.models[0].id.clone();
                        let key = cx
                            .global::<Credentials>()
                            .0
                            .get(provider.as_ref())
                            .cloned()
                            .unwrap_or_default();
                        fields.key.update(cx, |input, cx| input.set_text(key, cx));
                        cx.notify();
                    })
                }
            });
        let entry = PROVIDERS
            .iter()
            .find(|p| p.id == fields.provider)
            .expect("listed provider");
        let listed = entry.models.iter().any(|m| m.id == fields.model);
        let model = Select::new(
            gpui::SharedString::from(format!("butler-model-{}", fields.provider)),
            entry
                .models
                .iter()
                .map(|m| Choice::new(m.id.clone(), m.label.clone())),
        )
        .label(word("et.butler.model"))
        .placeholder(word("et.butler.model_choose"))
        .selected(fields.model.clone())
        .on_change({
            let this = this.clone();
            move |model, _, cx| {
                this.update(cx, |this, cx| {
                    this.butler_fields
                        .as_mut()
                        .expect("fields initialized")
                        .model = model.to_string();
                    cx.notify();
                });
            }
        });
        let save = Button::new("butler-save", word("et.butler.save"))
            .disabled(!listed)
            .on_click(move |_, _, cx| {
                let (provider, model, key) = {
                    let fields = this
                        .read(cx)
                        .butler_fields
                        .as_ref()
                        .expect("fields initialized");
                    (
                        fields.provider.clone(),
                        fields.model.clone(),
                        fields.key.read(cx).text().trim().to_string(),
                    )
                };
                if set(&this, "butler connection updated".into(), cx, |settings| {
                    settings.butler.provider = provider.clone();
                    settings.butler.model = model;
                }) {
                    cx.update_global::<Credentials, _>(|credentials, _| {
                        if key.is_empty() {
                            credentials.0.remove(&provider);
                        } else {
                            credentials.0.insert(provider, key);
                        }
                    });
                }
            });
        let persona = self.persona_card(cx);
        div()
            .flex()
            .flex_col()
            .gap_5()
            .child(
                SettingsSection::new(word("et.butler.title")).row(
                    SettingsRow::new(word("et.butler.enabled"))
                        .description(word("et.butler.enabled_hint"))
                        .control(enabled),
                ),
            )
            .child(persona)
            .child(
                SettingsSection::new(word("et.butler.connection"))
                    .row(SettingsRow::new(word("et.butler.provider")).control(provider))
                    .row(
                        SettingsRow::new(word("et.butler.model"))
                            .description(word(if listed {
                                "et.butler.model_hint"
                            } else {
                                "et.butler.model_retired"
                            }))
                            .control(div().w(rems(18.)).child(model)),
                    )
                    .row(
                        SettingsRow::new(word("et.butler.key"))
                            .description(word("et.butler.key_hint"))
                            .control(div().w(rems(18.)).child(PasswordInput::new(&fields.key))),
                    )
                    .row(
                        SettingsRow::new(word("et.butler.connection"))
                            .description(word("et.butler.connection_hint"))
                            .control(save),
                    ),
            )
    }

    fn persona_card(&self, cx: &Context<Self>) -> Div {
        let fields = self.butler_fields.as_ref().expect("fields initialized");
        let theme = cx.theme();
        let save = Button::new(
            "butler-persona-save",
            text(cx, "et.butler.persona_save", &[]),
        )
        .size(ControlSize::Sm)
        .on_click({
            let this = cx.entity();
            move |_, _, cx| {
                let mut persona = this
                    .read(cx)
                    .butler_fields
                    .as_ref()
                    .expect("fields initialized")
                    .persona
                    .read(cx)
                    .text()
                    .trim()
                    .to_string();
                if persona == default_persona(SettingsStore::get(cx).language.tag()) {
                    persona.clear();
                }
                set(&this, "butler persona updated".into(), cx, |settings| {
                    settings.butler.persona = persona;
                });
            }
        });
        let reset = Button::new(
            "butler-persona-reset",
            text(cx, "et.butler.persona_reset", &[]),
        )
        .size(ControlSize::Sm)
        .variant(ButtonVariant::Ghost)
        .on_click({
            let this = cx.entity();
            move |_, _, cx| {
                if set(&this, "butler persona reset".into(), cx, |settings| {
                    settings.butler.persona.clear();
                }) {
                    let input = this
                        .read(cx)
                        .butler_fields
                        .as_ref()
                        .expect("fields initialized")
                        .persona
                        .clone();
                    input.update(cx, |input, cx| {
                        input.set_text(default_persona(SettingsStore::get(cx).language.tag()), cx)
                    });
                }
            }
        });
        div()
            .flex()
            .flex_col()
            .gap_3()
            .p_4()
            .rounded(theme.radius(Radius::Xl))
            .bg(theme.colors.surface)
            .border_1()
            .border_color(theme.colors.border)
            .child(div().child(text(cx, "et.butler.persona", &[])))
            .child(
                div()
                    .text_sm()
                    .text_color(theme.colors.fg_muted)
                    .child(text(cx, "et.butler.persona_hint", &[])),
            )
            .child(Input::new(&fields.persona))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.colors.fg_muted)
                            .child(text(cx, "et.butler.persona_limit", &[])),
                    )
                    .child(div().flex().gap_2().child(reset).child(save)),
            )
    }
}
