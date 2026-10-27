//! User-owned character art, validated before it replaces the active skin.

use std::{collections::BTreeMap, path::PathBuf, sync::Arc, time::Instant};

use gpui::{App, Global, RenderImage};
use serde::{Deserialize, Serialize};

use crate::agents::{Kind, Pose};

mod decode;
pub mod files;
#[cfg(test)]
mod tests;

pub use decode::Clip;

pub const MASCOT_POSES: &[&str] = &[
    "calm",
    "idle",
    "working",
    "working-special",
    "working-slack",
    "thinking",
    "needs_input",
    "sleeping",
    "walking",
    "walking-left",
    "puzzled-down",
    "puzzled-up",
];
pub const AGENT_POSES: &[&str] = &[
    "idle",
    "working",
    "working-research",
    "working-special",
    "working-slack",
    "thinking",
    "needs_input",
    "sleeping",
];
pub const MAX_TOTAL_MEMORY: usize = 256 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Slot {
    Claude,
    Gpt,
    Deepseek,
    Grok,
    Kimi,
    Gemini,
    Pi,
    Opencode,
    Whale,
}

impl Slot {
    pub const ALL: [Self; 9] = [
        Self::Claude,
        Self::Gpt,
        Self::Deepseek,
        Self::Grok,
        Self::Kimi,
        Self::Gemini,
        Self::Pi,
        Self::Opencode,
        Self::Whale,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Self::Whale => "whale",
            _ => self.kind().expect("agent slot").stem(),
        }
    }

    pub fn kind(self) -> Option<Kind> {
        match self {
            Self::Claude => Some(Kind::Claude),
            Self::Gpt => Some(Kind::Gpt),
            Self::Deepseek => Some(Kind::Deepseek),
            Self::Grok => Some(Kind::Grok),
            Self::Kimi => Some(Kind::Kimi),
            Self::Gemini => Some(Kind::Gemini),
            Self::Pi => Some(Kind::Pi),
            Self::Opencode => Some(Kind::OpenCode),
            Self::Whale => None,
        }
    }

    pub fn agent(kind: Kind) -> Self {
        Self::ALL
            .into_iter()
            .find(|slot| slot.kind() == Some(kind))
            .expect("every agent has a slot")
    }

    pub fn poses(self) -> &'static [&'static str] {
        if self == Self::Whale {
            MASCOT_POSES
        } else {
            AGENT_POSES
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Layout {
    pub scale: f32,
    pub x: f32,
    pub y: f32,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            scale: 1.0,
            x: 0.0,
            y: 0.0,
        }
    }
}

impl Layout {
    pub fn validate(self) -> anyhow::Result<()> {
        anyhow::ensure!(
            (0.5..=1.0).contains(&self.scale),
            "skin: scale must be between 0.5 and 1"
        );
        anyhow::ensure!(
            (-30.0..=30.0).contains(&self.x) && (-30.0..=30.0).contains(&self.y),
            "skin: offsets must be between -30 and 30"
        );
        Ok(())
    }
}

pub struct Skin {
    pub layout: Layout,
    pub bytes: BTreeMap<String, Arc<[u8]>>,
    clips: BTreeMap<String, Arc<Clip>>,
    pub memory: usize,
    start: Instant,
}

impl Skin {
    pub fn new(
        slot: Slot,
        bytes: BTreeMap<String, Arc<[u8]>>,
        layout: Layout,
    ) -> anyhow::Result<Self> {
        layout.validate()?;
        anyhow::ensure!(
            bytes.contains_key("idle"),
            "skin: an idle image is required"
        );
        anyhow::ensure!(
            bytes
                .keys()
                .all(|pose| slot.poses().contains(&pose.as_str())),
            "skin: unknown action for {}",
            slot.id()
        );
        let mut clips = BTreeMap::new();
        let mut memory = 0;
        let mut encoded = 0;
        for (pose, data) in &bytes {
            encoded += data.len();
            anyhow::ensure!(
                encoded <= files::MAX_PACK_BYTES,
                "skin: combined images exceed 32 MiB"
            );
            let clip =
                Arc::new(Clip::decode(data).map_err(|e| anyhow::anyhow!("skin: {pose}: {e:#}"))?);
            memory += clip.memory;
            anyhow::ensure!(
                memory <= 128 * 1024 * 1024,
                "skin: decoded animations exceed 128 MiB"
            );
            clips.insert(pose.clone(), clip);
        }
        Ok(Self {
            bytes,
            layout,
            clips,
            memory,
            start: Instant::now(),
        })
    }

    pub fn frame(&self, pose: &str, seed: u64, now: Instant) -> Arc<RenderImage> {
        let clip = self.clips.get(pose).unwrap_or(&self.clips["idle"]);
        clip.at(now.saturating_duration_since(self.start), seed)
    }

    pub fn with_layout(&self, layout: Layout) -> anyhow::Result<Self> {
        layout.validate()?;
        Ok(Self {
            layout,
            bytes: self.bytes.clone(),
            clips: self.clips.clone(),
            memory: self.memory,
            start: self.start,
        })
    }
}

#[derive(Default)]
pub struct Library {
    pub active: BTreeMap<Slot, Arc<Skin>>,
    pub busy: bool,
    pub error: Option<String>,
    pub notice: Option<&'static str>,
}

impl Global for Library {}

pub fn folder() -> anyhow::Result<PathBuf> {
    Ok(crate::settings::folder()?.join("characters"))
}

pub fn init(cx: &mut App) -> anyhow::Result<()> {
    let folder = folder()?;
    cx.set_global(Library {
        busy: true,
        ..Library::default()
    });
    let loading = cx.background_executor().spawn(async move { load(&folder) });
    cx.spawn(async move |cx| {
        let library = loading.await;
        cx.update(|cx| {
            cx.set_global(library);
            cx.refresh_windows();
        });
    })
    .detach();
    Ok(())
}

fn load(folder: &std::path::Path) -> Library {
    let mut library = Library::default();
    let mut failures = Vec::new();
    for slot in Slot::ALL {
        let path = files::path(folder, slot);
        if !path.exists() {
            continue;
        }
        let loaded = files::read_pack(&path, slot).and_then(|skin| {
            room(&library.active, slot, &skin)?;
            Ok(skin)
        });
        match loaded {
            Ok(skin) => {
                library.active.insert(slot, Arc::new(skin));
            }
            Err(error) => {
                let why = format!("{}: {error:#}", path.display());
                log::error!("characters: {why}");
                failures.push(why);
            }
        }
    }
    if !failures.is_empty() {
        library.error = Some(failures.join("\n"));
    }
    log::info!("characters: loaded {} custom skins", library.active.len());
    library
}

pub fn room(active: &BTreeMap<Slot, Arc<Skin>>, slot: Slot, skin: &Skin) -> anyhow::Result<()> {
    let used: usize = active
        .iter()
        .filter(|(other, _)| **other != slot)
        .map(|(_, skin)| skin.memory)
        .sum();
    anyhow::ensure!(
        used + skin.memory <= MAX_TOTAL_MEMORY,
        "skin: all custom characters exceed 256 MiB of decoded images"
    );
    Ok(())
}

pub fn custom(slot: Slot, cx: &App) -> Option<&Arc<Skin>> {
    cx.try_global::<Library>()?.active.get(&slot)
}

pub fn preview(slot: Slot, pose: &str, now: Instant, cx: &App) -> Option<Arc<RenderImage>> {
    if let Some(skin) = custom(slot, cx) {
        return Some(skin.frame(pose, 0, now));
    }
    match slot.kind() {
        Some(kind) => crate::agents::assets::frame(
            kind,
            Pose::ALL.into_iter().find(|p| p.stem() == pose)?,
            0,
            now,
            cx,
        ),
        None => crate::agents::assets::mascot(pose, now, cx),
    }
}
