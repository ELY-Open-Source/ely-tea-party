use super::Service;
use crate::butler::{
    Config, Credentials,
    process::{self, Cancellation, Event, Purpose, Request},
    tasks::Status,
};
use gpui::{App, Global};
use std::{
    path::PathBuf,
    sync::mpsc,
    time::{Duration, Instant},
};

#[derive(Default)]
struct Organizer {
    probe: Option<Instant>,
    task: Option<gpui::Task<()>>,
    cancel: Option<Cancellation>,
}
impl Global for Organizer {}

pub fn cancel(cx: &mut App) {
    if let Some(service) = cx.try_global::<Service>() {
        service.invalidate();
        let _request = service.request(|store| {
            store
                .db
                .execute("UPDATE jobs SET state='failed' WHERE state='running'", [])?;
            Ok(())
        });
    }
    if cx.has_global::<Organizer>() {
        let organizer = cx.global_mut::<Organizer>();
        organizer.cancel = None;
        organizer.task = None;
    }
}

pub fn poll(folder: PathBuf, config: Config, language: String, cx: &mut App) {
    if !config.memory || !config.memory_ai {
        return;
    }
    cx.default_global::<Organizer>();
    let organizer = cx.global::<Organizer>();
    if organizer.cancel.is_some()
        || organizer
            .probe
            .is_some_and(|at| at.elapsed() < Duration::from_secs(60))
    {
        return;
    }
    cx.global_mut::<Organizer>().probe = Some(Instant::now());
    cx.default_global::<Credentials>();
    let Ok(api_key) = config.api_key(cx.global::<Credentials>()) else {
        return;
    };
    let service = Service::get(cx);
    let epoch = service.revision();
    let claim = service.request(move |store| store.claim(&folder));
    let cancellation = Cancellation::default();
    let flag = cancellation.0.clone();
    cx.global_mut::<Organizer>().cancel = Some(cancellation);
    let task = cx.spawn(async move |cx| {
        let job = match claim.await {
            Ok(Ok(Some(job))) => job,
            other => {
                if let Ok(Err(error)) = other {
                    log::warn!("memory: {error}");
                }
                cx.update(|cx| cx.global_mut::<Organizer>().cancel = None);
                return;
            }
        };
        let request = Request {
            version: 1,
            purpose: Purpose::Remember,
            provider: config.provider,
            model: config.model,
            api_key,
            language,
            persona: String::new(),
            status: Status::Unknown,
            snapshots: vec![],
            memory: Default::default(),
            consolidation: Some(job.clone()),
        };
        let (tx, rx) = mpsc::channel();
        log::info!("memory: one bounded consolidation request starts");
        let result = cx
            .background_executor()
            .spawn(async move {
                process::run(request, flag, tx);
                rx.try_iter().find_map(|event| match event {
                    Event::Done { text, .. } => Some(text),
                    _ => None,
                })
            })
            .await;
        if service.revision() == epoch {
            let save = service.request(move |store| match result {
                Some(text) => match store.finish(&job, &text) {
                    Ok(()) => Ok(()),
                    Err(error) => {
                        store.fail(&job)?;
                        Err(error)
                    }
                },
                None => store.fail(&job),
            });
            if let Ok(Err(error)) = save.await {
                log::warn!("memory: consolidation not applied: {error}");
            }
        } else {
            let _ = service.request(move |store| store.fail(&job)).await;
        }
        cx.update(|cx| cx.global_mut::<Organizer>().cancel = None);
    });
    cx.global_mut::<Organizer>().task = Some(task);
}
