//! The characters' animations, built in, beside Ely's own assets.

use std::borrow::Cow;

use gpui::{AssetSource, SharedString};

macro_rules! poses {
    ($($kind:literal),*) => {
        &[$(
            poses!(@one $kind, "working"),
            poses!(@one $kind, "working-research"),
            poses!(@one $kind, "working-special"),
            poses!(@one $kind, "working-slack"),
            poses!(@one $kind, "thinking"),
            poses!(@one $kind, "needs_input"),
            poses!(@one $kind, "idle"),
            poses!(@one $kind, "sleeping"),
        )*]
    };
    (@one $kind:literal, $pose:literal) => {
        (
            concat!("agents/", $kind, "-", $pose, ".webp"),
            include_bytes!(concat!("../../assets/agents/", $kind, "-", $pose, ".webp")),
        )
    };
}

/// Each character's poses as animated WebP, by asset path.
static ART: &[(&str, &[u8])] = poses!(
    "claude", "gpt", "deepseek", "grok", "kimi", "gemini", "pi", "opencode"
);

/// Ely's assets, and the characters under `agents/`.
pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        if path.starts_with("agents/") {
            let found = ART.iter().find(|(name, _)| *name == path);
            let (_, bytes) = found.ok_or_else(|| anyhow::anyhow!("agents: no art at {path}"))?;
            return Ok(Some(Cow::Borrowed(bytes)));
        }
        ely_gpui_component::Assets.load(path)
    }

    fn list(&self, path: &str) -> anyhow::Result<Vec<SharedString>> {
        let mut names = ely_gpui_component::Assets.list(path)?;
        let ours = ART.iter().map(|(name, _)| *name);
        names.extend(ours.filter(|name| name.starts_with(path)).map(Into::into));
        Ok(names)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::{Kind, Pose, art};

    #[test]
    fn every_character_has_every_pose() {
        for kind in Kind::ALL {
            for pose in Pose::ALL {
                let bytes = Assets.load(&art(kind, pose)).expect("built in");
                assert!(bytes.is_some_and(|bytes| bytes.starts_with(b"RIFF")));
            }
        }
        assert_eq!(ART.len(), Kind::ALL.len() * Pose::ALL.len());
    }
}
