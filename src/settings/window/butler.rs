use ely_gpui_component::{
    buttons::{Button, ButtonVariant, SegmentedControl},
    forms::{Choice, Input, PasswordInput, Select, TextInput},
    i18n::text,
    settings::{SettingsRow, SettingsSection},
    theme::{ActiveTheme, ControlSize, Radius},
};
use gpui::{App, AppContext, Context, Div, Entity, ParentElement, Styled, Window, div, rems};

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

fn save_connection(this: &Entity<SettingsWindow>, remove: bool, cx: &mut App) {
    let fields = this
        .read(cx)
        .butler_fields
        .as_ref()
        .expect("fields initialized");
    let provider = fields.provider.clone();
    let model = fields.model.clone();
    let draft = fields.key.read(cx).text().to_string();
    let key = draft.trim().to_string();
    let credentials = cx.global::<Credentials>();
    if credentials.saving || !credentials.loaded(&provider) {
        return;
    }
    if !remove && key.is_empty() {
        if set(this, "butler connection updated".into(), cx, |settings| {
            settings.butler.provider = provider;
            settings.butler.model = model;
        }) {
            this.update(cx, |this, cx| {
                this.failed = None;
                cx.notify();
            });
        }
        return;
    }
    let save = Credentials::save(&provider, (!remove).then_some(key), cx);
    let this = this.clone();
    cx.spawn(async move |cx| {
        let result = save.await;
        cx.update(|cx| match result {
            Ok(()) => {
                let saved = if !remove {
                    set(&this, "butler connection updated".into(), cx, |settings| {
                        settings.butler.provider = provider.clone();
                        settings.butler.model = model;
                    })
                } else {
                    true
                };
                this.update(cx, |this, cx| {
                    if saved {
                        this.failed = None;
                    }
                    let fields = this.butler_fields.as_ref().expect("fields initialized");
                    if fields.provider == provider && fields.key.read(cx).text() == draft {
                        fields.key.update(cx, |input, cx| input.set_text("", cx));
                    }
                    cx.notify();
                });
            }
            Err(error) => {
                log::error!("{error:#}");
                this.update(cx, |this, cx| {
                    this.failed = Some(format!("{error:#}").into());
                    cx.notify();
                });
            }
        });
    })
    .detach();
}

impl SettingsWindow {
    pub(super) fn butler_page(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        if self.butler_fields.is_none() {
            let config = SettingsStore::get(cx).butler.clone();
            cx.default_global::<Credentials>();
            self.butler_fields = Some(Fields {
                provider: config.provider,
                model: config.model,
                key: cx.new(|cx| TextInput::new(window, cx).masked()),
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
        let credentials = cx.global::<Credentials>();
        let loaded = credentials.loaded(&fields.provider);
        let saving = credentials.saving;
        let saved_key = credentials.key(&fields.provider);
        let has_key = matches!(saved_key, Ok(Some(_)));
        let key_status = if !loaded {
            text(cx, "et.butler.key_loading", &[])
        } else {
            match saved_key {
                Ok(Some(_)) => text(cx, "et.butler.key_saved", &[]),
                Ok(None) => text(cx, "et.butler.key_missing", &[]),
                Err(error) => format!("{error:#}").into(),
            }
        };
        let this = cx.entity();
        let word = |key| text(cx, key, &[]);
        let enabled = switch(
            &this,
            "butler-enabled",
            SettingsStore::get(cx).butler.enabled,
            |settings, enabled| settings.butler.enabled = enabled,
        );
        let companion = switch(
            &this,
            "butler-companion",
            SettingsStore::get(cx).butler.companion,
            |settings, enabled| settings.butler.companion = enabled,
        );
        let memory = switch(
            &this,
            "butler-memory",
            SettingsStore::get(cx).butler.memory,
            |s, v| s.butler.memory = v,
        );
        let memory_ai = switch(
            &this,
            "butler-memory-ai",
            SettingsStore::get(cx).butler.memory_ai,
            |s, v| s.butler.memory_ai = v,
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
                    if cx.global::<Credentials>().saving {
                        return;
                    }
                    this.update(cx, |this, cx| {
                        let fields = this.butler_fields.as_mut().expect("fields initialized");
                        let entry = PROVIDERS
                            .iter()
                            .find(|p| p.id == provider.as_ref())
                            .expect("listed provider");
                        fields.provider = provider.to_string();
                        fields.model = entry.models[0].id.clone();
                        fields.key.update(cx, |input, cx| input.set_text("", cx));
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
                if cx.global::<Credentials>().saving {
                    return;
                }
                this.update(cx, |this, cx| {
                    this.butler_fields
                        .as_mut()
                        .expect("fields initialized")
                        .model = model.to_string();
                    cx.notify();
                });
            }
        });
        let save = Button::new(
            "butler-save",
            word(if saving {
                "et.butler.key_saving"
            } else {
                "et.butler.save"
            }),
        )
        .disabled(!listed || !loaded || saving)
        .on_click({
            let this = this.clone();
            move |_, _, cx| save_connection(&this, false, cx)
        });
        let remove = Button::new("butler-key-remove", word("et.butler.key_remove"))
            .disabled(!has_key || saving)
            .on_click(move |_, _, cx| save_connection(&this, true, cx));
        let retry_provider = fields.provider.clone();
        let retry = Button::new("butler-key-reload", word("et.butler.key_reload"))
            .disabled(!loaded || saving)
            .on_click(move |_, _, cx| {
                Credentials::load(&retry_provider, cx);
            });
        let persona = self.persona_card(cx);
        div()
            .flex()
            .flex_col()
            .gap_5()
            .child(
                SettingsSection::new(word("et.butler.title"))
                    .row(
                        SettingsRow::new(word("et.butler.enabled"))
                            .description(word("et.butler.enabled_hint"))
                            .control(enabled),
                    )
                    .row(
                        SettingsRow::new(word("et.butler.companion"))
                            .description(word("et.butler.companion_hint"))
                            .control(companion),
                    ),
            )
            .child(
                SettingsSection::new(word("et.memory.title"))
                    .row(
                        SettingsRow::new(word("et.memory.enable"))
                            .description(word("et.memory.setting_hint"))
                            .control(memory),
                    )
                    .row(
                        SettingsRow::new(word("et.memory.organize"))
                            .description(word("et.memory.organize_hint"))
                            .control(memory_ai),
                    )
                    .row(
                        SettingsRow::new(word("et.memory.manage")).control(
                            Button::new("open-memory", word("et.memory.manage"))
                                .on_click(|_, _, cx| crate::memory::window::open(None, None, cx)),
                        ),
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
                        SettingsRow::new(key_status)
                            .control(div().flex().gap_2().child(retry).child(remove)),
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
