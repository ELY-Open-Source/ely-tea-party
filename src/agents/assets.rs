//! The characters' animations, built in as animated WebP and decoded frame by frame, so they move whether or not the window is active: gpui's `img` animates only in an active one.

use std::{
    collections::HashMap,
    io::Cursor,
    sync::{Arc, Mutex, OnceLock},
    time::{Duration, Instant},
};

use gpui::RenderImage;
use image::{AnimationDecoder, Rgba, codecs::webp::WebPDecoder};

use super::{Kind, Pose};

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
            concat!($kind, "-", $pose),
            include_bytes!(concat!("../../assets/agents/", $kind, "-", $pose, ".webp")),
        )
    };
}

/// Each character's poses as animated WebP, by `KIND-POSE`.
static ART: &[(&str, &[u8])] = poses!(
    "claude", "gpt", "deepseek", "grok", "kimi", "gemini", "pi", "opencode"
);

/// One pose's loop: a still per frame, and when each frame ends.
struct Strip {
    frames: Vec<Arc<RenderImage>>,
    ends: Vec<Duration>,
}

impl Strip {
    fn decode(bytes: &[u8]) -> anyhow::Result<Strip> {
        let mut decoder = WebPDecoder::new(Cursor::new(bytes))?;
        decoder.set_background_color(Rgba([0, 0, 0, 0]))?;
        let (mut frames, mut ends, mut end) = (Vec::new(), Vec::new(), Duration::ZERO);
        for frame in decoder.into_frames() {
            let mut frame = frame?;
            let (numer, denom) = frame.delay().numer_denom_ms();
            end += Duration::from_secs_f64(f64::from(numer) / f64::from(denom.max(1)) / 1000.0);
            // gpui draws BGRA.
            for pixel in frame.buffer_mut().chunks_exact_mut(4) {
                pixel.swap(0, 2);
            }
            frames.push(Arc::new(RenderImage::new([frame])));
            ends.push(end);
        }
        anyhow::ensure!(!frames.is_empty(), "no frames");
        Ok(Strip { frames, ends })
    }

    /// The frame `at` into the loop.
    fn at(&self, at: Duration) -> Arc<RenderImage> {
        let total = self.ends.last().copied().unwrap_or_default();
        let into = if total.is_zero() {
            Duration::ZERO
        } else {
            Duration::from_nanos((at.as_nanos() % total.as_nanos()) as u64)
        };
        let index = self.ends.partition_point(|end| *end <= into);
        self.frames[index.min(self.frames.len() - 1)].clone()
    }
}

/// The loops decoded so far, each the first time it shows.
type Strips = Mutex<HashMap<(Kind, Pose), Arc<Strip>>>;

fn strips() -> &'static Strips {
    static STRIPS: OnceLock<Strips> = OnceLock::new();
    STRIPS.get_or_init(Default::default)
}

/// `kind` in `pose`, the frame for `now`; `seed` staggers sessions so they move apart.
pub fn frame(kind: Kind, pose: Pose, seed: u64, now: Instant) -> Option<Arc<RenderImage>> {
    static START: OnceLock<Instant> = OnceLock::new();
    let start = *START.get_or_init(|| now);
    let mut strips = strips().lock().ok()?;
    let strip = match strips.get(&(kind, pose)) {
        Some(strip) => strip.clone(),
        None => {
            let name = format!("{}-{}", kind.stem(), pose.stem());
            let (_, bytes) = ART.iter().find(|(art, _)| *art == name)?;
            match Strip::decode(bytes) {
                Ok(strip) => {
                    log::debug!("agents: {name} decoded, {} frames", strip.frames.len());
                    let strip = Arc::new(strip);
                    strips.insert((kind, pose), strip.clone());
                    strip
                }
                Err(error) => {
                    log::error!("agents: {name} does not decode: {error:#}");
                    return None;
                }
            }
        }
    };
    let at = now.saturating_duration_since(start) + Duration::from_millis(seed * 617);
    Some(strip.at(at))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_pose_decodes_into_a_loop() {
        assert_eq!(ART.len(), Kind::ALL.len() * Pose::ALL.len());
        for kind in Kind::ALL {
            for pose in Pose::ALL {
                let name = format!("{}-{}", kind.stem(), pose.stem());
                let (_, bytes) = ART.iter().find(|(art, _)| *art == name).expect("built in");
                let strip = Strip::decode(bytes).expect("decodes");
                assert!(strip.frames.len() > 1, "{name} moves");
            }
        }
    }

    #[test]
    fn frames_follow_the_clock_round_the_loop() {
        let (_, bytes) = ART[0];
        let strip = Strip::decode(bytes).expect("decodes");
        let total = *strip.ends.last().expect("a frame");
        assert_eq!(strip.at(Duration::ZERO).id, strip.frames[0].id);
        assert_eq!(strip.at(strip.ends[0]).id, strip.frames[1].id);
        assert_eq!(strip.at(total).id, strip.frames[0].id, "round again");
    }
}
