use std::{
    collections::HashSet,
    path::PathBuf,
    process::ExitStatus,
    rc::Rc,
    time::{Duration, Instant},
};

use alacritty_terminal::{
    event::Event,
    grid::Dimensions,
    term::{ClipboardType, TermMode},
    vte::ansi::CursorShape,
};
use ely_gpui_component::{
    forms::{Copy, Paste, SelectAll},
    theme::ActiveTheme,
};
use futures::StreamExt;
use gpui::{
    Bounds, Context, EventEmitter, FocusHandle, Focusable, Font, FontFeatures, FontId, FontStyle,
    FontWeight, IntoElement, MouseButton, ParentElement, Pixels, Render, Styled, Subscription,
    Task, TextSystem, Window, div, prelude::*, px, size,
};

use super::{
    find::Finder,
    frame::frame,
    ink::Ink,
    input::{self, Find},
    mouse::MouseMode,
    paint::{self, Look},
    pointer::Pointer,
    pty::{Events, Pty, PtyEvent, config, fit},
    zoom,
};
use crate::{
    settings::{Settings, SettingsStore},
    themes,
};

/// The bundled Nerd Font that draws icons any font lacks; gpui loads no font without an `m`.
pub const ICONS: &str = "JetBrainsMono Nerd Font Mono";

/// The bundled code font, the default.
pub const MONO: &str = "JetBrains Mono";

/// How long the cursor shows, then hides, while it blinks.
pub const BLINK: Duration = Duration::from_millis(600);

/// What a terminal tells its owner.
#[derive(Clone, Debug, PartialEq)]
pub enum TerminalEvent {
    Title(String),
    Folder(PathBuf),
    Bell,
    Exited(ExitStatus),
}

/// A program on a pseudo-terminal, drawn cell by cell in the theme and font the settings choose.
pub struct TerminalView {
    pub(super) pty: Pty,
    pub(super) focus: FocusHandle,
    /// Text an input method is composing, drawn at the cursor.
    pub(super) marked: Option<String>,
    /// The cursor's cell in the window, from the last layout.
    pub(super) caret: Bounds<Pixels>,
    pub(super) pointer: Pointer,
    /// The mouse mode, as the tap follows it.
    pub(super) mouse: MouseMode,
    /// Keys whose press this terminal saw, so their releases may go out.
    pub(super) held: HashSet<String>,
    pub(super) finder: Option<Finder>,
    pub(super) size: (usize, usize),
    cell: (u16, u16),
    /// Focused in an active window, as the program last heard.
    focused: bool,
    blinking: bool,
    shown: bool,
    /// When the program last drew.
    drew: Instant,
    _events: Task<()>,
    _blink: Task<()>,
    _focus: [Subscription; 3],
    _settings: Subscription,
    _zoom: Subscription,
}

impl EventEmitter<TerminalEvent> for TerminalView {}

impl Focusable for TerminalView {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.focus.clone()
    }
}

impl TerminalView {
    /// Draws `pty`; the first layout fits its grid to the box.
    pub fn new(pty: Pty, events: Events, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let size = {
            let term = pty.term.lock();
            (term.columns(), term.screen_lines())
        };
        let focus = cx.focus_handle().tab_stop(true);
        let refocus =
            |view: &mut Self, window: &mut Window, cx: &mut Context<Self>| view.refocus(window, cx);
        Self {
            pty,
            _focus: [
                cx.observe_window_activation(window, refocus),
                cx.on_focus(&focus, window, refocus),
                cx.on_blur(&focus, window, refocus),
            ],
            focus,
            marked: None,
            caret: Bounds::default(),
            pointer: Pointer::default(),
            mouse: MouseMode::default(),
            held: HashSet::new(),
            finder: None,
            size,
            cell: (8, 16),
            focused: false,
            blinking: false,
            shown: true,
            drew: Instant::now(),
            _events: Self::pump(events, cx),
            _blink: Self::blink(cx),
            _settings: cx.observe_global::<SettingsStore>(|view, cx| {
                view.pty
                    .term
                    .lock()
                    .set_options(config(SettingsStore::get(cx)));
                view.search(false, cx);
                cx.notify();
            }),
            _zoom: cx.observe_global::<zoom::Enlarged>(|_, cx| cx.notify()),
        }
    }

    pub fn pid(&self) -> u32 {
        self.pty.pid
    }

    /// The process group the program's keys go to.
    pub fn foreground(&self) -> Option<u32> {
        self.pty.foreground()
    }

    /// When the program last drew.
    pub fn drew(&self) -> Instant {
        self.drew
    }

    fn pump(mut events: Events, cx: &mut Context<Self>) -> Task<()> {
        cx.spawn(async move |view, cx| {
            while let Some(first) = events.next().await {
                let mut batch = vec![first];
                while let Ok(more) = events.try_recv() {
                    batch.push(more);
                }
                if view.update(cx, |view, cx| view.handle(batch, cx)).is_err() {
                    break;
                }
            }
        })
    }

    fn blink(cx: &mut Context<Self>) -> Task<()> {
        cx.spawn(async move |view, cx| {
            loop {
                cx.background_executor().timer(BLINK).await;
                let ticked = view.update(cx, |view, cx| {
                    if view.blinking {
                        view.shown = !view.shown;
                        cx.notify();
                    }
                });
                if ticked.is_err() {
                    break;
                }
            }
        })
    }

    /// Fits the grid to `columns` by `lines`; true when the size changed.
    pub(super) fn fit(&mut self, columns: usize, lines: usize, cx: &mut Context<Self>) -> bool {
        let fitted = fit(columns, lines, self.cell);
        let size = (usize::from(fitted.num_cols), usize::from(fitted.num_lines));
        if size == self.size {
            return false;
        }
        self.size = size;
        self.pty.resize(size, self.cell);
        log::debug!("terminal {}: {} by {}", self.pty.pid, size.0, size.1);
        self.search(false, cx);
        true
    }

    /// Tells a program that asked, by mode 1004, when the terminal gains or loses focus.
    fn refocus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let focused = self.focus.is_focused(window) && window.is_window_active();
        if focused != self.focused {
            self.focused = focused;
            let asked = self.pty.term.lock().mode().contains(TermMode::FOCUS_IN_OUT);
            log::debug!(
                "terminal {}: focus {}{}",
                self.pty.pid,
                if focused { "in" } else { "out" },
                if asked { ", reported" } else { "" }
            );
            if asked {
                self.pty.write(if focused {
                    &b"\x1b[I"[..]
                } else {
                    &b"\x1b[O"[..]
                });
            }
        }
        cx.notify();
    }

    fn handle(&mut self, events: Vec<PtyEvent>, cx: &mut Context<Self>) {
        let drawn = events
            .iter()
            .any(|event| matches!(event, PtyEvent::Term(Event::Wakeup)));
        for event in events {
            match event {
                PtyEvent::Folder(folder) => cx.emit(TerminalEvent::Folder(folder)),
                PtyEvent::Term(event) => self.term_event(event, cx),
                PtyEvent::Mouse(mode) => self.mouse = mode,
            }
        }
        // ponytail: a full search per output batch while find is open; search only new lines if it lags.
        if drawn && self.finder.is_some() {
            self.search(false, cx);
        }
        cx.notify();
    }

    fn term_event(&mut self, event: Event, cx: &mut Context<Self>) {
        match event {
            Event::Wakeup => {
                self.shown = true;
                self.drew = Instant::now();
            }
            Event::MouseCursorDirty => {}
            // A reset empties the title and sends only this; setting the options sends the title.
            Event::CursorBlinkingChange => {
                let settings = config(SettingsStore::get(cx));
                self.pty.term.lock().set_options(settings);
            }
            Event::Title(title) => cx.emit(TerminalEvent::Title(title)),
            Event::ResetTitle => cx.emit(TerminalEvent::Title(String::new())),
            Event::Bell => {
                log::debug!("terminal {}: a bell, kept silent", self.pty.pid);
                cx.emit(TerminalEvent::Bell);
            }
            Event::ClipboardStore(kind, text) => {
                log::info!("terminal {}: OSC 52 writes the {kind:?}", self.pty.pid);
                self.copy_text(text, kind == ClipboardType::Selection, cx);
            }
            Event::ClipboardLoad(..) => log::error!(
                "terminal {}: a clipboard read arrived though OSC 52 reads are off",
                self.pty.pid
            ),
            Event::ColorRequest(index, reply) => {
                let theme = themes::active(SettingsStore::get(cx), cx.window_appearance(), cx);
                let term = self.pty.term.lock();
                let rgb = Ink::new(&theme, term.colors()).query(index, term.colors());
                drop(term);
                self.pty.write(reply(rgb).into_bytes());
            }
            Event::TextAreaSizeRequest(reply) => {
                let size = fit(self.size.0, self.size.1, self.cell);
                self.pty.write(reply(size).into_bytes());
            }
            Event::PtyWrite(text) => self.pty.write(text.into_bytes()),
            Event::ChildExit(status) => cx.emit(TerminalEvent::Exited(status)),
            Event::Exit => log::debug!("terminal {}: the grid closed", self.pty.pid),
        }
    }
}

/// `font` as gpui loads it; gpui falls back to another font without a word, and loads none without an `m`.
pub fn load_font(font: &Font, system: &TextSystem) -> anyhow::Result<FontId> {
    let id = system.resolve_font(font);
    let loaded = system.get_font_for_id(id);
    anyhow::ensure!(
        loaded.is_some_and(|loaded| loaded.family == font.family),
        "font_family is {:?}, which the terminal cannot load",
        font.family
    );
    Ok(id)
}

/// Font, cell and metrics at `points` on this window.
fn look(settings: &Settings, points: f32, ink: &Ink, window: &Window) -> anyhow::Result<Look> {
    let font = Font {
        family: settings.font_family.clone().into(),
        features: if settings.ligatures {
            FontFeatures::default()
        } else {
            FontFeatures::disable_ligatures()
        },
        fallbacks: None,
        weight: FontWeight::NORMAL,
        style: FontStyle::Normal,
    };
    let text = px(points);
    let system = window.text_system();
    let id = load_font(&font, system)?;
    let advance = system
        .advance(id, text, 'M')
        .map_err(|error| anyhow::anyhow!("{} cannot measure M: {error}", settings.font_family))?;
    let scale = window.scale_factor();
    let snap = |length: Pixels| px((f32::from(length) * scale).round() / scale);
    let cell = size(
        snap(advance.width).max(px(1.)),
        snap(text * settings.line_height),
    );
    let (ascent, descent) = (system.ascent(id, text), system.descent(id, text).abs());
    let icons = Font {
        family: ICONS.into(),
        features: FontFeatures::default(),
        ..font.clone()
    };
    Ok(Look {
        font_id: id,
        icons,
        baseline: (cell.height - ascent - descent) / 2. + ascent,
        descent,
        rule: snap((text / 14.).max(px(1.))),
        size: text,
        cell,
        font,
        cursor: ink.cursor,
        cursor_text: ink.cursor_text,
        fg: ink.fg,
        bg: ink.bg,
    })
}

impl Render for TerminalView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let settings = SettingsStore::get(cx).clone();
        let theme = themes::active(&settings, window.appearance(), cx);
        let term = self.pty.term.lock();
        let ink = Ink::new(&theme, term.colors());
        let lit = self.finder.as_ref().map_or_else(Vec::new, |finder| {
            finder.lit(&term, cx.theme().colors.warning)
        });
        let shown = frame(&term, &ink, &lit);
        let blinks = term.cursor_style().blinking;
        drop(term);
        let focused = self.focus.is_focused(window) && window.is_window_active();
        self.blinking = focused && blinks;
        if !self.blinking {
            self.shown = true;
        }
        let cursor = shown
            .cursor
            .clone()
            .and_then(|cursor| match (focused, self.shown) {
                (false, _) => Some((cursor, CursorShape::HollowBlock)),
                (true, false) => None,
                (true, true) => {
                    let shape = cursor.shape;
                    Some((cursor, shape))
                }
            });
        let root = div()
            .id(("terminal", cx.entity_id()))
            .key_context(input::CONTEXT)
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::key_down))
            .on_key_up(cx.listener(Self::key_up))
            .on_action(cx.listener(|view, _: &Copy, _, cx| view.copy(cx)))
            .on_action(cx.listener(|view, _: &Paste, _, cx| view.paste(cx)))
            .on_action(cx.listener(|view, _: &SelectAll, _, cx| view.select_all(cx)))
            .on_action(cx.listener(|view, _: &Find, window, cx| view.find(window, cx)))
            .on_any_mouse_down(cx.listener(Self::mouse_down))
            .on_mouse_move(cx.listener(Self::mouse_move))
            .on_scroll_wheel(cx.listener(Self::wheel))
            .size_full()
            .px(px(settings.padding_x))
            .py(px(settings.padding_y))
            // The window's glass grounds the theme's background; a program's own paints over it.
            .when(ink.bg != theme.background, |root| {
                root.bg(ink.bg.alpha(settings.background_opacity))
            });
        let root = [MouseButton::Left, MouseButton::Middle, MouseButton::Right]
            .into_iter()
            .fold(root, |root, button| {
                root.on_mouse_up(button, cx.listener(Self::mouse_up))
                    .on_mouse_up_out(button, cx.listener(Self::mouse_up))
            });
        let root = match look(&settings, zoom::size(cx), &ink, window) {
            Ok(look) => {
                self.cell = (
                    f32::from(look.cell.width).round() as u16,
                    f32::from(look.cell.height).round() as u16,
                );
                let preedit = self.marked.clone();
                root.child(paint::grid(
                    cx.entity(),
                    Rc::new(shown),
                    cursor,
                    preedit,
                    look,
                ))
            }
            Err(error) => {
                log::error!("terminal {}: {error:#}", self.pty.pid);
                root.text_color(ink.fg).child(format!("{error:#}"))
            }
        };
        div().relative().size_full().child(root).children(
            self.find_box(cx)
                .map(|found| div().absolute().top_2().right_4().child(found)),
        )
    }
}
