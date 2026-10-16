use std::fmt::Debug;

use ely_gpui_component::{
    buttons::SegmentedControl,
    feedback::InlineMessage,
    forms::{InputEvent, Switch, TextInput},
    lists::{ListItem, SelectableList},
    primitives::{FocusScope, Icon, IconName, Severity},
    shell::{ColumnShell, ResizeBorder, ShellColumn},
    theme::{ActiveTheme, ControlSize, IconSize},
};
use gpui::{
    App, AppContext, Bounds, Context, Entity, FocusHandle, InteractiveElement, IntoElement,
    KeyBinding, ParentElement, Render, SharedString, StatefulInteractiveElement, Styled,
    Subscription, Task, TitlebarOptions, Window, WindowBounds, WindowDecorations, WindowOptions,
    actions, div, prelude::FluentBuilder, px, size,
};

use super::{Settings, SettingsStore};
use crate::{app::APP_ID, menus, term::BLINK};

mod appearance;
mod terminal;

actions!(ely_tea_party, [OpenSettings, CloseSettings]);

/// The key context of the settings window's content.
const CONTEXT: &str = "Settings";

/// Each category's key, name and icon.
const SECTIONS: [(&str, &str, IconName); 2] = [
    ("appearance", "Appearance", IconName::Palette),
    ("terminal", "Terminal", IconName::Terminal),
];

/// Cmd-, opens the settings, Cmd-W closes them; off macOS Ctrl-, and Ctrl-Shift-W, as Ghostty's.
pub fn bind_keys(cx: &mut App) {
    let (open, close) = if cfg!(target_os = "macos") {
        ("cmd-,", "cmd-w")
    } else {
        ("ctrl-,", "ctrl-shift-w")
    };
    cx.bind_keys([
        KeyBinding::new(open, OpenSettings, None),
        KeyBinding::new(close, CloseSettings, Some(CONTEXT)),
    ]);
    cx.on_action(|_: &OpenSettings, cx| open_window(cx));
}

/// Opens the settings window, or brings the open one forward.
fn open_window(cx: &mut App) {
    let open = cx
        .windows()
        .into_iter()
        .find_map(|window| window.downcast::<SettingsWindow>());
    if let Some(window) = open {
        log::info!("settings: the window comes forward");
        let forward = window.update(cx, |_, window, _| window.activate_window());
        if let Err(error) = forward {
            log::error!("settings: the window did not come forward: {error:#}");
        }
        return;
    }
    let bounds = Bounds::centered(None, size(px(900.0), px(640.0)), cx);
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: Some(TitlebarOptions {
            title: Some("Settings".into()),
            appears_transparent: true,
            traffic_light_position: Some(cx.theme().traffic_light_origin()),
        }),
        // Ely's column draws the controls; only Linux reads this.
        window_decorations: Some(WindowDecorations::Client),
        ..Default::default()
    };
    match cx.open_window(options, |window, cx| {
        window.set_app_id(APP_ID);
        cx.new(|cx| SettingsWindow::new(window, cx))
    }) {
        Ok(_) => log::info!("settings: the window opens"),
        Err(error) => log::error!("settings: the window did not open: {error:#}"),
    }
}

/// The settings window: categories in one column, the open one's rows in the next, each scrolling on its own.
pub struct SettingsWindow {
    focus: FocusHandle,
    section: SharedString,
    shell: Entity<TextInput>,
    /// The shortcut list's search.
    search: Entity<TextInput>,
    /// Why the typed shell is not saved.
    refused: Option<SharedString>,
    /// Why the last change did not save.
    failed: Option<SharedString>,
    /// Refusals so far; a field keyed by it drops its draft and shows the settings again.
    drafts: usize,
    font: Entity<TextInput>,
    /// Whether the cursor preview shows, as it blinks.
    lit: bool,
    _typed: Subscription,
    _settings: Subscription,
    _blink: Task<()>,
}

impl SettingsWindow {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let shell = cx.new(|cx| {
            let mut input = TextInput::new(window, cx).placeholder("Login shell");
            input.set_text(SettingsStore::get(cx).shell.clone(), cx);
            input
        });
        let font = cx.new(|cx| TextInput::new(window, cx));
        let search = cx.new(|cx| TextInput::new(window, cx).placeholder("Search shortcuts"));
        let typed = cx.subscribe(&shell, |this, input, event: &InputEvent, cx| {
            if let InputEvent::Changed = event {
                let shell = input.read(cx).text().to_string();
                this.save_shell(shell, cx);
            }
        });
        // Focus inside, so the window's keys find its context.
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        Self {
            focus,
            section: SECTIONS[0].0.into(),
            shell,
            search,
            refused: None,
            failed: None,
            drafts: 0,
            font,
            lit: true,
            _typed: typed,
            _settings: cx.observe_global::<SettingsStore>(|_, cx| cx.notify()),
            _blink: Self::blink(cx),
        }
    }

    /// Blinks the cursor preview as a terminal blinks its cursor, while it shows.
    fn blink(cx: &mut Context<Self>) -> Task<()> {
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(BLINK).await;
                let ticked = this.update(cx, |this, cx| {
                    if this.section == "appearance" && SettingsStore::get(cx).cursor_blink {
                        this.lit = !this.lit;
                        cx.notify();
                    }
                });
                if ticked.is_err() {
                    break;
                }
            }
        })
    }

    /// Saves `shell` when it names a program; else says why and keeps the saved one.
    fn save_shell(&mut self, shell: String, cx: &mut Context<Self>) {
        self.refused = match SettingsStore::update(cx, |settings| settings.shell = shell.clone()) {
            Ok(()) => {
                log::info!("settings: new sessions run {shell:?}");
                self.failed = None;
                None
            }
            Err(error) => {
                log::debug!("settings: the shell stays: {error:#}");
                Some(format!("{error:#}").into())
            }
        };
        cx.notify();
    }
}

/// The categories; a press, Up and Down, or Enter opens one.
fn sections(open: &SharedString, cx: &Context<SettingsWindow>) -> SelectableList {
    let muted = cx.theme().colors.fg_muted;
    let list = SECTIONS.iter().fold(
        SelectableList::new("settings-section-list").selected([open.clone()]),
        |list, (key, name, icon)| {
            let item = ListItem::new(SharedString::from(format!("settings-section-{key}")), *name)
                .leading(Icon::new(*icon).size(IconSize::Sm).color(muted));
            list.row(*key, item)
        },
    );
    let show = {
        let this = cx.entity();
        move |key: &SharedString, cx: &mut App| {
            log::info!("settings: open {key}");
            this.update(cx, |this, cx| {
                this.section = key.clone();
                cx.notify();
            });
        }
    };
    let enter = show.clone();
    list.on_change(move |keys, _, cx| {
        if let Some(key) = keys.first() {
            show(key, cx);
        }
    })
    .on_activate(move |key, _, cx| enter(key, cx))
}

/// A switch that saves through `put`.
fn switch(
    this: &Entity<SettingsWindow>,
    field: &'static str,
    on: bool,
    put: fn(&mut Settings, bool),
) -> Switch {
    let this = this.clone();
    Switch::new(field, on).on_change(move |on, _, cx| {
        set(&this, format!("{field} is {on}"), cx, |settings| {
            put(settings, on)
        });
    })
}

/// Segments for `choices`, each a value, its key and its name, that save the chosen value through `put`.
fn choice<T: Copy + PartialEq + Debug + 'static>(
    this: &Entity<SettingsWindow>,
    field: &'static str,
    choices: &'static [(T, &'static str, &'static str)],
    now: T,
    put: fn(&mut Settings, T),
) -> SegmentedControl {
    let (_, key, _) = choices
        .iter()
        .find(|(each, ..)| *each == now)
        .expect("every choice is offered");
    let this = this.clone();
    choices
        .iter()
        .fold(
            SegmentedControl::new(field, *key),
            |control, (_, key, name)| control.segment(*key, *name, None),
        )
        .size(ControlSize::Sm)
        .on_change(move |key, _, cx| {
            let (value, ..) = choices
                .iter()
                .find(|(_, each, _)| *each == key.as_ref())
                .expect("a choice the control offers");
            let value = *value;
            set(&this, format!("{field} is {value:?}"), cx, |settings| {
                put(settings, value)
            });
        })
}

/// Saves one change and logs it, telling whether the settings hold it; a refused one changes nothing, and the window says why.
fn set(
    this: &Entity<SettingsWindow>,
    change: String,
    cx: &mut App,
    edit: impl FnOnce(&mut Settings),
) -> bool {
    let mut next = SettingsStore::get(cx).clone();
    edit(&mut next);
    if next == *SettingsStore::get(cx) {
        return true;
    }
    let failed = match SettingsStore::update(cx, |settings| *settings = next) {
        Ok(()) => {
            log::info!("settings: {change}");
            None
        }
        Err(error) => {
            log::error!("settings: refused {change}: {error:#}");
            Some(format!("{error:#}").into())
        }
    };
    let saved = failed.is_none();
    this.update(cx, |this, cx| {
        this.drafts += usize::from(!saved);
        this.failed = failed;
        cx.notify();
    });
    saved
}

impl Render for SettingsWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let page = match self.section.as_ref() {
            "appearance" => self.appearance(window, cx).into_any_element(),
            "terminal" => self.terminal(cx).into_any_element(),
            other => unreachable!("settings: no section {other}"),
        };
        let page = div()
            .flex()
            .flex_col()
            .gap_4()
            .children((self.failed.clone()).map(|why| InlineMessage::new(Severity::Danger, why)))
            .child(page);
        let (_, name, _) = SECTIONS
            .iter()
            .find(|(key, ..)| *key == self.section.as_ref())
            .expect("the open section is listed");
        let list = div()
            .id("settings-list")
            .size_full()
            .overflow_y_scroll()
            .p_2()
            .child(sections(&self.section, cx));
        let body = div()
            .id("settings-body")
            .size_full()
            .overflow_y_scroll()
            .p_6()
            .child(page);
        // The categories and the page scroll apart.
        let shell = ColumnShell::new("settings")
            .column(
                ShellColumn::new("settings-sections")
                    .width(cx.theme().label_width() * 1.25)
                    .title("Settings")
                    .child(list),
            )
            .column(ShellColumn::new("settings-page").title(*name).child(body));
        // The context sits above the focus root, so its keys work wherever focus is.
        div()
            .size_full()
            .key_context(CONTEXT)
            .map(menus::window_actions)
            .on_action(|_: &CloseSettings, window, _| {
                log::info!("settings: the window closes");
                window.remove_window();
            })
            .on_action(|_: &OpenSettings, window, _| {
                log::info!("settings: the window comes forward");
                window.activate_window();
            })
            .child(
                ResizeBorder::new()
                    .child(FocusScope::new(&self.focus).root().size_full().child(shell)),
            )
    }
}
