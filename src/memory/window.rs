use super::{Overview, Service, Store};
use crate::settings::SettingsStore;
use ely_gpui_component::{forms::TextInput, i18n::text};
use gpui::{
    App, AppContext, Bounds, Context, Entity, Render, Subscription, Task, Window, WindowBounds,
    WindowOptions, div, prelude::*, px, size,
};
use std::path::PathBuf;

mod render;

pub struct MemoryWindow {
    folder: Option<PathBuf>,
    quote: Option<String>,
    suggestion: Option<String>,
    input: Entity<TextInput>,
    search: Entity<TextInput>,
    kind: String,
    editing: Option<(String, i64)>,
    overview: Overview,
    error: Option<String>,
    busy: bool,
    clear_pending: bool,
    task: Option<Task<()>>,
    settings: Subscription,
}

pub fn open(folder: Option<PathBuf>, feedback: Option<(String, Option<String>)>, cx: &mut App) {
    if let Some(handle) = cx
        .windows()
        .into_iter()
        .find_map(|w| w.downcast::<MemoryWindow>())
    {
        let _ = handle.update(cx, |this, window, cx| {
            if this.busy {
                window.activate_window();
                return;
            }
            this.folder = folder;
            this.quote = feedback.as_ref().map(|f| f.0.clone());
            this.suggestion = feedback.and_then(|f| f.1);
            this.editing = None;
            this.input.update(cx, |input, cx| input.set_text("", cx));
            this.refresh(cx);
            window.activate_window();
        });
        return;
    }
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
            None,
            size(px(760.), px(680.)),
            cx,
        ))),
        ..Default::default()
    };
    if let Err(error) = cx.open_window(options, |window, cx| {
        cx.new(|cx| {
            window.set_window_title(&text(cx, "et.memory.title", &[]));
            let input = cx.new(|cx| TextInput::new(window, cx).multi_line(2, 4).max_len(300));
            let search = cx.new(|cx| {
                TextInput::new(window, cx).max_len(80).placeholder(text(
                    cx,
                    "et.memory.search_hint",
                    &[],
                ))
            });
            let mut this = MemoryWindow {
                folder,
                quote: feedback.as_ref().map(|f| f.0.clone()),
                suggestion: feedback.and_then(|f| f.1),
                input,
                search,
                kind: "preference".into(),
                editing: None,
                overview: Overview::default(),
                error: None,
                busy: false,
                clear_pending: false,
                task: None,
                settings: cx.observe_global::<SettingsStore>(|_, cx| cx.notify()),
            };
            this.refresh(cx);
            this
        })
    }) {
        log::error!("memory: cannot open memory window: {error:#}");
    }
}

impl MemoryWindow {
    fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.busy = true;
        let folder = self.folder.clone();
        let query = self.search.read(cx).text().to_string();
        let request =
            Service::get(cx).request(move |store| store.overview(folder.as_deref(), &query));
        self.task = Some(cx.spawn(async move |this, cx| {
            let result = request.await;
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                match result {
                    Ok(Ok(overview)) => {
                        this.overview = overview;
                        this.error = None;
                    }
                    Ok(Err(error)) => this.error = Some(error),
                    Err(_) => this.error = Some("Memory worker is unavailable or busy".into()),
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn perform(
        &mut self,
        work: impl FnOnce(&mut Store) -> anyhow::Result<()> + Send + 'static,
        cx: &mut Context<Self>,
    ) {
        if self.busy {
            return;
        }
        let service = Service::get(cx);
        service.invalidate();
        self.busy = true;
        let request = service.request(work);
        self.task = Some(cx.spawn(async move |this, cx| {
            let result = request.await;
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                match result {
                    Ok(Ok(())) => {
                        this.editing = None;
                        this.suggestion = None;
                        this.quote = None;
                        this.clear_pending = false;
                        this.input.update(cx, |input, cx| input.set_text("", cx));
                        this.refresh(cx);
                    }
                    Ok(Err(error)) => this.error = Some(error),
                    Err(_) => this.error = Some("Memory worker is unavailable or busy".into()),
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        if !SettingsStore::get(cx).butler.memory {
            return;
        }
        let folder = if self.kind == "preference" {
            None
        } else {
            self.folder.clone()
        };
        let kind = self.kind.clone();
        let body = self.input.read(cx).text().to_string();
        let editing = self.editing.clone();
        let source = self
            .quote
            .as_ref()
            .map(|quote| format!("{body}\nReply to: {quote}"));
        self.perform(
            move |store| {
                store.save(
                    folder.as_deref(),
                    &kind,
                    &body,
                    editing.as_ref().map(|(id, rev)| (id.as_str(), *rev)),
                    source.as_deref(),
                )
            },
            cx,
        );
    }
}

impl Render for MemoryWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let _ = &self.settings;
        div().size_full().child(self.content(window, cx))
    }
}
