//! Coding agents in sessions, each drawn as its character in a pose that follows its state.

pub mod activity;
pub mod assets;
pub mod detect;
#[cfg(unix)]
pub mod hooks;
#[cfg(unix)]
pub mod install;
pub mod quips;
pub mod speed;
pub mod timeline;

use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

use gpui::Hsla;

use activity::{Activity, Turn};

/// An agent quiet this long while working is thinking.
const THINKING_AFTER: Duration = Duration::from_secs(6);
/// An idle agent untouched this long dozes off.
const SLEEPING_AFTER: Duration = Duration::from_secs(10 * 60);
/// A hooked agent working this long without a word lost its Stop event.
const STALE_AFTER: Duration = Duration::from_secs(15 * 60);
/// Output this recent means an unhooked agent is working.
const BUSY_WITHIN: Duration = Duration::from_secs(2);
/// While working, the pose turns every this long.
const ROTATE: Duration = Duration::from_secs(15);
/// An unhooked agent's working poses in turn; the main one twice, so it shows most.
const WORKING: [Pose; 5] = [
    Pose::Working,
    Pose::Research,
    Pose::Working,
    Pose::Special,
    Pose::Slacking,
];

/// Which character an agent is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Claude,
    Gpt,
    Deepseek,
    Grok,
    Kimi,
    Gemini,
    Pi,
    OpenCode,
}

/// What an agent is doing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Working,
    NeedsInput,
    Idle,
}

/// What its character is drawn doing; each names one animation per character.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Pose {
    Working,
    Research,
    Special,
    Slacking,
    Thinking,
    NeedsInput,
    Idle,
    Sleeping,
}

impl Kind {
    pub const ALL: [Kind; 8] = [
        Kind::Claude,
        Kind::Gpt,
        Kind::Deepseek,
        Kind::Grok,
        Kind::Kimi,
        Kind::Gemini,
        Kind::Pi,
        Kind::OpenCode,
    ];

    /// The agent a program or hook source names, as its command is spelled.
    pub fn named(name: &str) -> Option<Kind> {
        Some(match name.to_ascii_lowercase().as_str() {
            "claude" | "claude-code" => Kind::Claude,
            "codex" => Kind::Gpt,
            "dscode" | "deepseek" => Kind::Deepseek,
            "grok" => Kind::Grok,
            "kimi" => Kind::Kimi,
            "gemini" | "gemini-cli" | "antigravity" | "agy" => Kind::Gemini,
            "pi" | "omp" => Kind::Pi,
            "opencode" | "opencode-ai" => Kind::OpenCode,
            _ => return None,
        })
    }

    /// Its file stem in `assets/agents`.
    pub fn stem(self) -> &'static str {
        match self {
            Kind::Claude => "claude",
            Kind::Gpt => "gpt",
            Kind::Deepseek => "deepseek",
            Kind::Grok => "grok",
            Kind::Kimi => "kimi",
            Kind::Gemini => "gemini",
            Kind::Pi => "pi",
            Kind::OpenCode => "opencode",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Kind::Claude => "Claude学姐",
            Kind::Gpt => "GPT娘",
            Kind::Deepseek => "鲸鱼娘",
            Kind::Grok => "Grok娘",
            Kind::Kimi => "Kimi兔",
            Kind::Gemini => "Gemini星",
            Kind::Pi => "Pi酱",
            Kind::OpenCode => "OpenCode娘",
        }
    }

    /// The character's own color, which tints its tile.
    pub fn color(self) -> Hsla {
        let hex = match self {
            Kind::Claude => 0xD97757,
            Kind::Gpt => 0x10A37F,
            Kind::Deepseek => 0x4D6BFE,
            Kind::Grok => 0xD4D4D4,
            Kind::Kimi => 0x8FA8FF,
            Kind::Gemini => 0xA855F7,
            Kind::Pi => 0xE3342F,
            Kind::OpenCode => 0xFAB283,
        };
        gpui::rgb(hex).into()
    }

    /// What the character is doing in `pose`, in its own words.
    pub fn label(self, pose: Pose) -> &'static str {
        let labels: [&str; 8] = match self {
            Kind::Claude => [
                "丝带多线程",
                "翻魔导书",
                "终曲·彩带炮",
                "偷吃蛋糕（摸鱼）",
                "品茶沉思",
                "等你决定",
                "下午茶",
                "打盹",
            ],
            Kind::Gpt => [
                "科研狂热中",
                "速读文献",
                "实验爆炸了",
                "偷偷打游戏（摸鱼）",
                "……",
                "（举牌）",
                "看书",
                "睡着了",
            ],
            Kind::Deepseek => [
                "边吃边干活",
                "潜入文档海",
                "过热喷水",
                "躺平吃零食（摸鱼）",
                "盯着米饭想",
                "等你投喂",
                "干饭中",
                "吃撑睡着了",
            ],
            Kind::Grok => [
                "电吉他狂飙",
                "坐音箱刷信息流",
                "辣味模式·闪电和弦",
                "鼻尖顶拨片（摸鱼）",
                "嚼拨片憋坏主意",
                "拿琴头指着你喊",
                "弹琴喝果汁",
                "抱着吉他睡着",
            ],
            Kind::Kimi => [
                "读超长卷轴",
                "百卷齐翻",
                "卷轴化星河",
                "荡月亮吃月饼（摸鱼）",
                "望月沉思",
                "递卷轴等你",
                "月下品茶",
                "裹卷轴睡着",
            ],
            Kind::Gemini => [
                "悬浮双键盘",
                "倒挂读书",
                "多模态爆发",
                "吹泡泡（摸鱼）",
                "星星在转",
                "飘过来问你",
                "飘着喝奶茶",
                "悬空睡着",
            ],
            Kind::Pi => [
                "边烤派边敲码",
                "翻食谱",
                "巨型π派出炉！",
                "偷吃樱桃派（摸鱼）",
                "盯着烤箱想",
                "樱桃还是苹果？",
                "下午茶配派",
                "趴在烤箱边睡着",
            ],
            Kind::OpenCode => [
                "敲终端",
                "翻日志",
                "开源派对",
                "打掌机（摸鱼）",
                "盯着光标",
                "摘耳机等你",
                "懒人沙发听歌",
                "趴键盘睡着",
            ],
        };
        labels[pose as usize]
    }
}

impl Pose {
    pub const ALL: [Pose; 8] = [
        Pose::Working,
        Pose::Research,
        Pose::Special,
        Pose::Slacking,
        Pose::Thinking,
        Pose::NeedsInput,
        Pose::Idle,
        Pose::Sleeping,
    ];

    /// Its file suffix in `assets/agents`.
    pub fn stem(self) -> &'static str {
        match self {
            Pose::Working => "working",
            Pose::Research => "working-research",
            Pose::Special => "working-special",
            Pose::Slacking => "working-slack",
            Pose::Thinking => "thinking",
            Pose::NeedsInput => "needs_input",
            Pose::Idle => "idle",
            Pose::Sleeping => "sleeping",
        }
    }
}

/// A new session's id, which its hooks report under.
pub fn next_session() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

/// The variables a session starts with so its agents' hooks reach the app.
#[cfg(unix)]
pub fn env(id: u64) -> anyhow::Result<Vec<(String, String)>> {
    hooks::env(id)
}

#[cfg(windows)]
pub fn env(_: u64) -> anyhow::Result<Vec<(String, String)>> {
    Ok(Vec::new())
}

/// What Claude Code marks its children with; none of it describes a fresh session.
pub const CLAUDE_MARKS: [&str; 9] = [
    "CLAUDECODE",
    "CLAUDE_PID",
    "CLAUDE_CODE_CHILD_SESSION",
    "CLAUDE_CODE_SESSION_ID",
    "CLAUDE_CODE_SESSION_ATTENDED",
    "CLAUDE_CODE_ENTRYPOINT",
    "CLAUDE_CODE_EXECPATH",
    "CLAUDE_CODE_MESSAGING_SOCKET",
    "CLAUDE_CODE_MESSAGING_TOKEN",
];

/// An agent running in a session.
#[derive(Clone, Debug)]
pub struct Agent {
    pub kind: Kind,
    pub status: Status,
    pub speed: speed::Speed,
    pub speed_inferred: bool,
    speed_pid: Option<u32>,
    /// When the status began.
    pub since: Instant,
    /// Its last sign of life: a hook, or output while unhooked.
    pub active: Instant,
    /// Its hooks report its status; else output and bells stand in for them.
    pub hooked: bool,
    /// The tool its hooks last named.
    pub activity: Option<Activity>,
    pub turn: Turn,
    /// Her last word and when she said it.
    pub quip: Option<(quips::Moment, Instant)>,
    /// The command her hooks name, as `codex` or `agy`.
    pub program: Option<String>,
    /// Her conversation's id, which resumes it.
    pub conversation: Option<String>,
}

impl Agent {
    pub fn new(kind: Kind, now: Instant) -> Self {
        Self {
            kind,
            status: Status::Idle,
            speed: speed::Speed::Unknown,
            speed_inferred: false,
            speed_pid: None,
            since: now,
            active: now,
            hooked: false,
            activity: None,
            turn: Turn::default(),
            quip: None,
            program: None,
            conversation: None,
        }
    }

    /// Moves to `status`; returns whether it changed.
    pub fn set(&mut self, status: Status, now: Instant) -> bool {
        self.active = now;
        if self.status == status {
            return false;
        }
        self.status = status;
        self.since = now;
        true
    }

    /// An unhooked agent works while it draws; a bell asks for input until it draws again.
    pub fn guess(&mut self, drew: Instant, now: Instant) -> bool {
        if self.hooked {
            return false;
        }
        let busy = now.saturating_duration_since(drew) < BUSY_WITHIN;
        match self.status {
            Status::NeedsInput if drew > self.since => self.set(Status::Working, drew),
            Status::NeedsInput => false,
            _ if busy => self.set(Status::Working, drew),
            Status::Working => {
                self.status = Status::Idle;
                self.since = drew;
                true
            }
            Status::Idle => false,
        }
    }

    /// The pose at `now`; `seed` staggers sessions so their poses turn apart.
    pub fn pose(&self, now: Instant, seed: u64) -> Pose {
        let quiet = now.saturating_duration_since(self.active);
        match self.status {
            Status::Working if self.hooked && quiet >= STALE_AFTER => Pose::Idle,
            Status::Idle if quiet >= SLEEPING_AFTER => Pose::Sleeping,
            _ if self.hooked => self.hooked_pose(now, quiet, THINKING_AFTER),
            Status::NeedsInput => Pose::NeedsInput,
            Status::Working => {
                let turn = now.saturating_duration_since(self.since).as_secs() / ROTATE.as_secs();
                WORKING[((turn + seed) % WORKING.len() as u64) as usize]
            }
            Status::Idle => Pose::Idle,
        }
    }
}

/// `elapsed` as a row shows it: `12s`, `5m`, `3h`, `2d`.
pub fn ago(elapsed: Duration) -> String {
    match elapsed.as_secs() {
        s if s < 60 => format!("{s}s"),
        s if s < 3600 => format!("{}m", s / 60),
        s if s < 86400 => format!("{}h", s / 3600),
        s => format!("{}d", s / 86400),
    }
}

/// When an agent last showed life, `active`, as its row shows it on the local clock.
pub fn updated(active: Instant, now: Instant) -> String {
    let quiet = chrono::TimeDelta::from_std(now.saturating_duration_since(active))
        .expect("agents: an agent's quiet fits a TimeDelta");
    let today = chrono::Local::now();
    clock(today - quiet, today)
}

/// `at` against `today`: `14:32` the same day, else `10/19`.
pub fn clock<Tz: chrono::TimeZone>(at: chrono::DateTime<Tz>, today: chrono::DateTime<Tz>) -> String
where
    Tz::Offset: std::fmt::Display,
{
    let format = if at.date_naive() == today.date_naive() {
        "%H:%M"
    } else {
        "%-m/%-d"
    };
    at.format(format).to_string()
}

#[cfg(test)]
mod tests;
