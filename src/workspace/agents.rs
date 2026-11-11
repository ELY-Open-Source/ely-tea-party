//! Which agent each session runs and what it is doing, from its foreground job, its output and its hooks.

use std::time::{Duration, Instant};

use gpui::{Context, Task, Window};

use super::Workspace;
use crate::agents::{
    Agent,
    detect::{Foreground, foreground},
};

/// How often the sessions' foreground jobs are read.
const EVERY: Duration = Duration::from_secs(1);

/// How often characters move to their next frame: the art's 12 frames a second.
const FRAME: Duration = Duration::from_millis(83);

/// The foreground polling and animation tasks for this window.
pub(super) fn watch(window: &mut Window, cx: &mut Context<Workspace>) -> [Task<()>; 2] {
    let tick = cx.spawn_in(window, async move |this, cx| {
        loop {
            cx.background_executor().timer(EVERY).await;
            if this.update(cx, |this, cx| this.follow_agents(cx)).is_err() {
                break;
            }
        }
    });
    // Characters move whether or not the window is active, unless the system asks for less motion.
    let frames = cx.spawn(async move |this, cx| {
        loop {
            cx.background_executor().timer(FRAME).await;
            let moved = this.update(cx, |this, cx| {
                if this.sessions.iter().any(|session| session.agent.is_some())
                    && !cx.reduce_motion()
                {
                    cx.notify();
                }
            });
            if moved.is_err() {
                break;
            }
        }
    });
    [tick, frames]
}

#[cfg(unix)]
pub struct Reports {
    _task: Task<()>,
}

#[cfg(unix)]
impl Reports {
    pub fn new(
        mut reports: futures::channel::mpsc::UnboundedReceiver<crate::agents::hooks::Report>,
        cx: &mut gpui::App,
    ) -> Self {
        use futures::StreamExt;
        let task = cx.spawn(async move |cx| {
            while let Some(report) = reports.next().await {
                cx.update(|cx| {
                    for handle in cx
                        .windows()
                        .into_iter()
                        .filter_map(|window| window.downcast::<Workspace>())
                    {
                        let found = handle.update(cx, |workspace, _, cx| {
                            if !workspace
                                .sessions
                                .iter()
                                .any(|session| session.id == report.session)
                            {
                                return false;
                            }
                            workspace.hear(report.clone(), cx);
                            true
                        });
                        match found {
                            Ok(true) => return,
                            Ok(false) => {}
                            Err(error) => log::debug!("agents: report window unavailable: {error}"),
                        }
                    }
                    log::debug!("agents: report for closed session {}", report.session);
                });
            }
        });
        Self { _task: task }
    }
}

#[cfg(unix)]
impl gpui::Global for Reports {}

/// `agent` after the session's foreground is `held`, the program having last drawn at `drew`.
fn follow(agent: &mut Option<Agent>, held: Foreground, drew: Instant, now: Instant) -> bool {
    match (held, agent.as_mut()) {
        (Foreground::Shell, _) => agent.take().is_some(),
        (Foreground::Agent(kind), Some(found)) if found.kind == kind => found.guess(drew, now),
        (Foreground::Agent(kind), _) => {
            *agent = Some(Agent::new(kind, now));
            true
        }
        // Hooks name agents the foreground does not show.
        (Foreground::Other, Some(found)) if found.hooked => found.guess(drew, now),
        (Foreground::Other, _) => agent.take().is_some(),
    }
}

impl Workspace {
    fn follow_agents(&mut self, cx: &mut Context<Self>) {
        let now = Instant::now();
        let mut shown = false;
        for session in &mut self.sessions {
            let view = session.terminal.read(cx);
            let held = foreground(view.pid(), view.foreground());
            let had = session
                .agent
                .as_ref()
                .map(|agent| (agent.kind, agent.status));
            if follow(&mut session.agent, held, view.drew(), now) {
                let has = session
                    .agent
                    .as_ref()
                    .map(|agent| (agent.kind, agent.status));
                log::info!(
                    "agents: session {} went from {had:?} to {has:?}",
                    session.id
                );
            }
            if let Some(agent) = session.agent.as_mut() {
                let pid = view.foreground();
                if agent.instance_pid.is_some() && pid.is_some() && agent.instance_pid != pid {
                    agent.instance = uuid::Uuid::new_v4();
                    log::info!("messages: session {} agent process changed", session.id);
                }
                agent.instance_pid = pid;
            }
            // Hooks name their program; an unhooked Antigravity is known by its command.
            if let Some(agent) = session.agent.as_mut()
                && agent.kind == crate::agents::Kind::Gemini
                && agent.program.is_none()
                && view
                    .foreground()
                    .is_some_and(crate::agents::detect::antigravity)
            {
                agent.program = Some("agy".into());
            }
            if let Some(agent) = session.agent.as_mut()
                && agent.kind == crate::agents::Kind::Gpt
            {
                if let Some(pid) = view.foreground() {
                    agent.startup_speed(pid, &session.folder);
                }
                if agent.speed == crate::agents::speed::Speed::Unknown
                    || had.is_none_or(|(kind, _)| kind != agent.kind)
                {
                    agent.codex_title(&session.title);
                }
                if !agent.hooked
                    && let Some((_, record)) = view.records().iter().find(|(id, record)| {
                        id.is_empty() && record.app.as_deref() == Some("codex")
                    })
                {
                    use crate::{agents::Status, term::State};
                    let status = match record.state {
                        State::Working => Status::Working,
                        State::Blocked => Status::NeedsInput,
                        _ => Status::Idle,
                    };
                    // Read each tick: only a change is a sign of life.
                    if agent.status != status && agent.set(status, now) {
                        log::info!(
                            "agents: session {} native Codex state is {status:?}",
                            session.id
                        );
                    }
                }
            }
            if let Some(drawer) = session.drawer.as_mut() {
                let view = drawer.terminal.read(cx);
                let held = foreground(view.pid(), view.foreground());
                follow(&mut drawer.agent, held, view.drew(), now);
            }
            if let Some(drawer) = session.drawer.as_mut()
                && drawer.check(cx)
            {
                log::debug!(
                    "agents: session {}'s drawer busy: {}",
                    session.id,
                    drawer.busy
                );
            }
            shown |= session.agent.is_some() || session.drawer.is_some();
        }
        self.type_pending(now, cx);
        self.keep();
        self.keep_steps(now, cx);
        self.meter(now, cx);
        // Poses turn and times count up, so rows with agents redraw each tick.
        if shown {
            cx.notify();
        }
    }

    #[cfg(unix)]
    fn hear(&mut self, report: crate::agents::hooks::Report, cx: &mut Context<Self>) {
        use crate::agents::hooks::Change;

        let Some(index) = self.sessions.iter().position(|s| s.id == report.session) else {
            log::debug!("agents: a report for closed session {}", report.session);
            return;
        };
        let Some(change) = report.change() else {
            log::debug!("agents: {} means nothing to a character", report.event);
            return;
        };
        if report.drawer {
            return self.hear_drawer(index, report, change, cx);
        }
        let session = &mut self.sessions[index];
        let now = Instant::now();
        if change == Change::Speed {
            if let Some(agent) = session.agent.as_mut()
                && agent.observe_speed(report.kind(), report.conversation.as_deref(), report.speed)
            {
                log::info!("agents: session {} speed is {:?}", session.id, agent.speed);
                cx.notify();
            }
            return;
        }
        let status = match change {
            Change::Speed => unreachable!(),
            Change::Ended => {
                log::info!("agents: session {}'s {} ended", session.id, report.agent);
                session.agent = None;
                cx.notify();
                return;
            }
            Change::To(status) => status,
        };
        let Some(kind) = report
            .kind()
            .or(session.agent.as_ref().map(|agent| agent.kind))
        else {
            log::error!("agents: no character for agent {:?}", report.agent);
            return;
        };
        if session
            .agent
            .as_ref()
            .is_none_or(|agent| agent.kind != kind)
        {
            session.agent = Some(Agent::new(kind, now));
        }
        let Some(agent) = session.agent.as_mut() else {
            return;
        };
        if report.event == "SessionStart" {
            agent.instance = uuid::Uuid::new_v4();
            agent.instance_pid = None;
        }
        agent.hooked = true;
        agent.program = Some(report.agent.clone());
        if let Some(conversation) = &report.conversation {
            if agent
                .conversation
                .as_ref()
                .is_some_and(|id| id != conversation)
            {
                agent.reported_speed(crate::agents::speed::Speed::Unknown);
                agent.instance_pid = None;
            }
            agent.conversation = Some(conversation.clone());
        }
        if let Some(speed) = report.speed
            && agent.reported_speed(speed)
        {
            log::info!("agents: session {} speed is {speed:?}", session.id);
        }
        if crate::settings::SettingsStore::get(cx).butler.memory {
            let kind = match report.event.as_str() {
                "UserPromptSubmit" => Some("prompt"),
                "Stop" | "agent-turn-complete" => Some("reported"),
                "StopFailure" => Some("failed"),
                "PermissionRequest" => Some("question"),
                _ => None,
            };
            if let (Some(kind), Some(detail)) = (kind, &report.detail) {
                crate::memory::Service::get(cx).record(
                    session.folder.clone(),
                    session.link_id.url(),
                    kind.into(),
                    detail.clone(),
                );
            }
        }
        let (tool, detail) = (report.tool.as_deref(), report.detail.as_deref());
        let said = (tool, detail, report.tool_id.as_deref());
        agent.note(&report.event, said, &mut session.log, now);
        session.log.retain_reply(report.reply.clone());
        if let Some(said) = report.detail.clone() {
            match report.event.as_str() {
                "UserPromptSubmit" => session
                    .terminal
                    .update(cx, |view, cx| view.prompted(said, cx)),
                "Stop" | "agent-turn-complete" => session
                    .terminal
                    .update(cx, |view, cx| view.replied(said, cx)),
                _ => {}
            }
        }
        if agent.set(status, now) {
            log::info!("agents: session {}'s {kind:?} is {status:?}", session.id);
            if status == crate::agents::Status::NeedsInput && index != self.active {
                session.bell = true;
            }
        }
        cx.notify();
    }

    /// What an agent in session `index`'s drawer did: its Activity, leaving the session's character alone.
    #[cfg(unix)]
    fn hear_drawer(
        &mut self,
        index: usize,
        report: crate::agents::hooks::Report,
        change: crate::agents::hooks::Change,
        cx: &mut Context<Self>,
    ) {
        use crate::agents::hooks::Change;

        let Some(drawer) = self.sessions[index].drawer.as_mut() else {
            log::debug!("agents: a report for a closed drawer");
            return;
        };
        let status = match change {
            Change::Speed => return,
            Change::Ended => {
                drawer.agent = None;
                cx.notify();
                return;
            }
            Change::To(status) => status,
        };
        let Some(kind) = report.kind() else {
            log::error!("agents: no character for agent {:?}", report.agent);
            return;
        };
        let now = Instant::now();
        let agent = match drawer.agent.take() {
            Some(agent) if agent.kind == kind => agent,
            _ => Agent::new(kind, now),
        };
        let agent = drawer.agent.insert(agent);
        if report.event == "SessionStart" {
            agent.instance = uuid::Uuid::new_v4();
        }
        agent.hooked = true;
        let said = (
            report.tool.as_deref(),
            report.detail.as_deref(),
            report.tool_id.as_deref(),
        );
        agent.note(&report.event, said, &mut drawer.log, now);
        drawer.log.retain_reply(report.reply);
        agent.set(status, now);
        cx.notify();
    }

    /// Shows the first other session whose agent waits for an answer, else one that rang.
    pub(super) fn jump_to_waiting(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let others = || (0..self.sessions.len()).filter(|index| *index != self.active);
        let found = others()
            .filter(|index| self.sessions[*index].task_state(cx).needs_attention())
            .min_by_key(|index| self.sessions[*index].task_state(cx).priority())
            .or_else(|| others().find(|index| self.sessions[*index].bell));
        match found {
            Some(index) => self.select(index, window, cx),
            None => log::debug!("agents: no session waits"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::Kind;

    #[test]
    fn the_foreground_brings_and_takes_agents() {
        let now = Instant::now();
        let mut agent = None;
        assert!(follow(&mut agent, Foreground::Agent(Kind::Gpt), now, now));
        assert_eq!(agent.as_ref().map(|agent| agent.kind), Some(Kind::Gpt));
        assert!(
            follow(&mut agent, Foreground::Agent(Kind::Claude), now, now),
            "another one"
        );
        assert!(follow(&mut agent, Foreground::Shell, now, now));
        assert!(agent.is_none());
    }

    #[test]
    fn a_hooked_agent_outlives_a_foreground_it_cannot_read() {
        let now = Instant::now();
        let mut hooked = Agent::new(Kind::Claude, now);
        hooked.hooked = true;
        let mut agent = Some(hooked);
        assert!(!follow(&mut agent, Foreground::Other, now, now));
        assert!(agent.is_some());
        let mut guessed = Some(Agent::new(Kind::Claude, now));
        assert!(follow(&mut guessed, Foreground::Other, now, now));
        assert!(guessed.is_none());
    }
}
