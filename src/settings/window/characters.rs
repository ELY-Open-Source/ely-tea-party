use std::time::{Duration, Instant};

use ely_gpui_component::{
    buttons::Button,
    feedback::InlineMessage,
    forms::{Choice, Input, NumberInput, Select, TextInput},
    i18n::text,
    primitives::Severity,
    settings::{SettingsRow, SettingsSection},
    theme::{ActiveTheme, ControlSize, Radius},
};
use gpui::{
    App, AppContext, Context, Div, Entity, IntoElement, ObjectFit, ParentElement, Styled,
    StyledImage, Task, Window, div, img, px, rems,
};

use super::SettingsWindow;
use crate::skins::{self, Layout, Library, Meta, Slot};

mod jobs;
use jobs::Action;

pub(super) struct Fields {
    slot: Slot,
    pose: String,
    credits: Option<Credits>,
}

/// The name, author and license an export writes, filled from the shown character.
struct Credits {
    name: Entity<TextInput>,
    author: Entity<TextInput>,
    license: Entity<TextInput>,
    /// What the fields were last filled from.
    filled: Option<(Slot, Option<Meta>)>,
}

impl Default for Fields {
    fn default() -> Self {
        Self {
            slot: Slot::Claude,
            pose: "idle".into(),
            credits: None,
        }
    }
}

pub(super) fn animate(cx: &mut Context<SettingsWindow>) -> Task<()> {
    cx.spawn(async move |this, cx| {
        loop {
            cx.background_executor()
                .timer(Duration::from_millis(83))
                .await;
            if this
                .update(cx, |this, cx| {
                    if this.section == "characters" && !cx.reduce_motion() {
                        cx.notify();
                    }
                })
                .is_err()
            {
                break;
            }
        }
    })
}

impl SettingsWindow {
    pub(super) fn characters_page(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        self.fill_credits(window, cx);
        let slot = self.characters.slot;
        let pose = &self.characters.pose;
        let library = cx.global::<Library>();
        let skin = skins::custom(slot, cx);
        let busy = library.busy;
        let word = |key| text(cx, key, &[]);
        let choices = Slot::ALL.map(|slot| {
            Choice::new(
                slot.id(),
                slot.kind()
                    .map_or_else(|| word("et.mascot.whale"), |kind| kind.name().into()),
            )
        });
        let selected = Select::new("character-slot", choices)
            .selected(slot.id())
            .disabled(busy)
            .on_change({
                let this = cx.entity();
                move |id, _, cx| {
                    this.update(cx, |this, cx| {
                        this.characters.slot = Slot::ALL
                            .into_iter()
                            .find(|slot| slot.id() == id.as_ref())
                            .expect("listed slot");
                        this.characters.pose = "idle".into();
                        cx.notify();
                    })
                }
            });
        let poses = Select::new(
            "character-pose",
            slot.poses()
                .iter()
                .map(|pose| Choice::new(*pose, text(cx, pose_key(pose), &[]))),
        )
        .selected(pose.clone())
        .disabled(busy)
        .on_change({
            let this = cx.entity();
            move |pose, _, cx| {
                this.update(cx, |this, cx| {
                    this.characters.pose = pose.to_string();
                    cx.notify();
                })
            }
        });
        let button = |id, label, action: Action| {
            Button::new(id, word(label))
                .size(ControlSize::Sm)
                .disabled(busy)
                .on_click(move |_, _, cx| jobs::start(slot, action.clone(), cx))
        };
        let actions = div()
            .flex()
            .flex_wrap()
            .gap_2()
            .child(button(
                "character-image",
                "et.characters.image",
                Action::Image,
            ))
            .child(button(
                "character-import",
                "et.characters.import",
                Action::Import,
            ))
            .child(self.export_button(slot, busy, cx))
            .child(button(
                "character-reset",
                "et.characters.reset",
                Action::Reset,
            ));
        let pose_image = button(
            "character-pose-image",
            "et.characters.pose_image",
            Action::Pose(pose.clone()),
        )
        .disabled(busy || skin.is_none());
        let status = if busy {
            "et.characters.busy"
        } else if skin.is_some() {
            "et.characters.custom"
        } else {
            "et.characters.builtin"
        };
        let mut page = div()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().colors.fg_muted)
                    .child(word("et.characters.hint")),
            )
            .child(
                SettingsSection::new(word("et.characters.title")).row(
                    SettingsRow::new(word("et.characters.choose"))
                        .description(word(status))
                        .control(div().w(rems(18.)).child(selected)),
                ),
            )
            .child(self.credits_section(cx))
            .child(actions)
            .children(
                library
                    .error
                    .as_ref()
                    .map(|error| InlineMessage::new(Severity::Danger, error.clone())),
            )
            .children(
                library
                    .notice
                    .map(|key| InlineMessage::new(Severity::Success, word(key))),
            )
            .children((!library.skipped.is_empty()).then(|| {
                let actions = library.skipped.join(", ");
                InlineMessage::new(
                    Severity::Warning,
                    text(cx, "et.characters.skipped", &[("actions", &actions)]),
                )
            }))
            .child(
                div()
                    .flex()
                    .gap_4()
                    .items_center()
                    .child(preview(slot, pose, cx))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_3()
                            .flex_1()
                            .child(poses)
                            .child(pose_image)
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().colors.fg_muted)
                                    .child(word(if skin.is_none() {
                                        "et.characters.pose_hint"
                                    } else if skin
                                        .is_some_and(|skin| !skin.bytes.contains_key(pose))
                                    {
                                        "et.characters.fallback"
                                    } else {
                                        "et.characters.action_hint"
                                    })),
                            ),
                    ),
            );
        if slot == Slot::Whale {
            page = page.child(layout_controls(
                slot,
                skin.map_or(Layout::default(), |skin| skin.layout),
                busy || skin.is_none(),
                cx,
            ));
        }
        page.child(
            div()
                .text_sm()
                .text_color(cx.theme().colors.fg_muted)
                .child(word("et.characters.limits")),
        )
    }
}

impl SettingsWindow {
    /// Fills the credit fields when the shown character or its pack's credits change.
    fn fill_credits(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let credits = self.characters.credits.get_or_insert_with(|| {
            let field = |most, window: &mut Window, cx: &mut Context<Self>| {
                cx.new(|cx| TextInput::new(window, cx).max_len(most))
            };
            Credits {
                name: field(64, window, cx),
                author: field(64, window, cx),
                license: field(64, window, cx),
                filled: None,
            }
        });
        let slot = self.characters.slot;
        let custom = skins::custom(slot, cx);
        let now = (slot, custom.and_then(|skin| skin.meta.clone()));
        if credits.filled.as_ref() == Some(&now) {
            return;
        }
        let (name, author, license) = match &now.1 {
            Some(meta) => (
                meta.name.clone(),
                meta.author.clone(),
                meta.license.clone().unwrap_or_default(),
            ),
            None => (
                character_name(slot, cx),
                if custom.is_some() {
                    String::new()
                } else {
                    "Ely Tea Party".into()
                },
                String::new(),
            ),
        };
        for (input, value) in [
            (&credits.name, name),
            (&credits.author, author),
            (&credits.license, license),
        ] {
            input.update(cx, |input, cx| input.set_text(value, cx));
        }
        credits.filled = Some(now);
    }

    fn credits_section(&self, cx: &Context<Self>) -> SettingsSection {
        let word = |key| text(cx, key, &[]);
        let credits = self.characters.credits.as_ref().expect("credits filled");
        let field = |input: &Entity<TextInput>| div().w(rems(18.)).child(Input::new(input));
        SettingsSection::new(word("et.characters.credits"))
            .row(
                SettingsRow::new(word("et.characters.name"))
                    .description(word("et.characters.credits_hint"))
                    .control(field(&credits.name)),
            )
            .row(SettingsRow::new(word("et.characters.author")).control(field(&credits.author)))
            .row(
                SettingsRow::new(word("et.characters.license"))
                    .description(word("et.characters.license_hint"))
                    .control(field(&credits.license)),
            )
    }

    /// Export writes the typed credits; a pack's own id and links stay while its author does.
    fn export_button(&self, slot: Slot, busy: bool, cx: &Context<Self>) -> Button {
        let credits = self.characters.credits.as_ref().expect("credits filled");
        let fields = [&credits.name, &credits.author, &credits.license].map(Entity::clone);
        Button::new("character-export", text(cx, "et.characters.export", &[]))
            .size(ControlSize::Sm)
            .disabled(busy)
            .on_click(move |_, _, cx| {
                let [name, author, license] = fields
                    .each_ref()
                    .map(|input| input.read(cx).text().trim().to_string());
                let kept = skins::custom(slot, cx)
                    .and_then(|skin| skin.meta.clone())
                    .filter(|meta| meta.author == author);
                let meta = Meta {
                    id: kept.as_ref().and_then(|meta| meta.id.clone()),
                    name,
                    author,
                    description: kept.as_ref().and_then(|meta| meta.description.clone()),
                    license: (!license.is_empty()).then_some(license),
                    homepage: kept.and_then(|meta| meta.homepage),
                };
                jobs::start(slot, Action::Export(meta), cx)
            })
    }
}

fn character_name(slot: Slot, cx: &App) -> String {
    slot.kind().map_or_else(
        || text(cx, "et.mascot.whale", &[]).to_string(),
        |kind| kind.name().to_string(),
    )
}

fn preview(slot: Slot, pose: &str, cx: &App) -> Div {
    let skin = skins::custom(slot, cx);
    let layout = if slot == Slot::Whale {
        skin.map_or(Layout::default(), |skin| skin.layout)
    } else {
        Layout::default()
    };
    let side = 112.0 * layout.scale;
    let frame = skins::preview(slot, pose, Instant::now(), cx);
    let y = if slot == Slot::Whale {
        112.0 - side + layout.y
    } else {
        0.0
    };
    div()
        .p_4()
        .rounded(cx.theme().radius(Radius::Lg))
        .bg(cx.theme().colors.surface)
        .child(
            div()
                .relative()
                .size(px(112.))
                .overflow_hidden()
                .children(frame.map(|frame| {
                    img(frame)
                        .absolute()
                        .left(px((112.0 - side) / 2.0 + layout.x))
                        .top(px(y))
                        .size(px(side))
                        .object_fit(ObjectFit::Contain)
                })),
        )
}

fn layout_controls(slot: Slot, layout: Layout, disabled: bool, cx: &App) -> SettingsSection {
    let mut section = SettingsSection::new(text(cx, "et.characters.placement", &[]));
    for (field, label, value, min, max) in [
        (
            "scale",
            "et.characters.scale",
            layout.scale * 100.0,
            50.0,
            100.0,
        ),
        ("x", "et.characters.x", layout.x, -30.0, 30.0),
        ("y", "et.characters.y", layout.y, -30.0, 30.0),
    ] {
        let control = NumberInput::new(field, f64::from(value))
            .range(min, max)
            .step(1.0)
            .size(ControlSize::Sm)
            .on_commit(move |value, _, cx| {
                if disabled {
                    return;
                }
                let Some(skin) = skins::custom(slot, cx) else {
                    return;
                };
                let mut layout = skin.layout;
                match field {
                    "scale" => layout.scale = value as f32 / 100.0,
                    "x" => layout.x = value as f32,
                    _ => layout.y = value as f32,
                }
                if layout == skin.layout {
                    return;
                }
                jobs::start(slot, Action::Layout(layout), cx);
            });
        let control = if disabled {
            div().child(format!("{value:.0}")).into_any_element()
        } else {
            control.into_any_element()
        };
        section = section.row(SettingsRow::new(text(cx, label, &[])).control(control));
    }
    section
}

fn pose_key(pose: &str) -> &'static str {
    match pose {
        "idle" => "et.characters.idle",
        "calm" => "et.characters.calm",
        "working" => "et.characters.working",
        "working-research" => "et.characters.research",
        "working-special" => "et.characters.special",
        "working-slack" => "et.characters.slack",
        "thinking" => "et.characters.thinking",
        "needs_input" => "et.characters.needs_input",
        "sleeping" => "et.characters.sleeping",
        "walking" => "et.characters.walking",
        "walking-left" => "et.characters.walking_left",
        "puzzled-up" => "et.characters.look_up",
        "puzzled-down" => "et.characters.look_down",
        _ => unreachable!("listed pose"),
    }
}
