use std::{io::Cursor, sync::Arc, time::Duration};

use gpui::RenderImage;
use image::{
    AnimationDecoder, DynamicImage, Frame, ImageDecoder, ImageFormat, Rgba,
    codecs::{gif::GifDecoder, png::PngDecoder, webp::WebPDecoder},
};

pub struct Clip {
    frames: Vec<Arc<RenderImage>>,
    ends: Vec<Duration>,
    pub memory: usize,
}

impl Clip {
    pub fn decode(bytes: &[u8]) -> anyhow::Result<Self> {
        anyhow::ensure!(
            bytes.len() <= super::files::MAX_IMAGE_BYTES,
            "image exceeds 8 MiB"
        );
        let reader = Cursor::new(bytes);
        match image::guess_format(bytes)? {
            ImageFormat::WebP => {
                let mut decoder = WebPDecoder::new(reader)?;
                dimensions(&decoder)?;
                if decoder.has_animation() {
                    decoder.set_background_color(Rgba([0, 0, 0, 0]))?;
                    Self::frames(decoder.into_frames())
                } else {
                    Self::still(decoder)
                }
            }
            ImageFormat::Gif => {
                let decoder = GifDecoder::new(reader)?;
                dimensions(&decoder)?;
                Self::frames(decoder.into_frames())
            }
            ImageFormat::Png => {
                let decoder = PngDecoder::new(reader)?;
                dimensions(&decoder)?;
                if decoder.is_apng()? {
                    Self::frames(decoder.apng()?.into_frames())
                } else {
                    Self::still(decoder)
                }
            }
            _ => anyhow::bail!("use a PNG, GIF or WebP image"),
        }
    }

    fn still(decoder: impl ImageDecoder) -> anyhow::Result<Self> {
        Self::frames(std::iter::once(Ok(Frame::new(
            DynamicImage::from_decoder(decoder)?.into_rgba8(),
        ))))
    }

    fn frames(frames: impl IntoIterator<Item = image::ImageResult<Frame>>) -> anyhow::Result<Self> {
        let mut result = Self {
            frames: Vec::new(),
            ends: Vec::new(),
            memory: 0,
        };
        let mut elapsed = Duration::ZERO;
        let mut decoded = 0usize;
        for (index, frame) in frames.into_iter().enumerate() {
            anyhow::ensure!(index < 240, "animation exceeds 240 frames");
            let frame = frame?;
            decoded += frame.buffer().len();
            anyhow::ensure!(
                decoded <= 128 * 1024 * 1024,
                "animation exceeds 128 MiB before resizing"
            );
            let (n, d) = frame.delay().numer_denom_ms();
            let delay =
                Duration::from_secs_f64((f64::from(n) / f64::from(d.max(1)) / 1000.0).max(0.02));
            elapsed += delay;
            let mut rgba = frame.into_buffer();
            if rgba.width() > 224 || rgba.height() > 224 {
                rgba = DynamicImage::ImageRgba8(rgba)
                    .thumbnail(224, 224)
                    .into_rgba8();
            }
            result.memory += rgba.len();
            for pixel in rgba.chunks_exact_mut(4) {
                pixel.swap(0, 2);
            }
            result
                .frames
                .push(Arc::new(RenderImage::new([Frame::new(rgba)])));
            result.ends.push(elapsed);
        }
        anyhow::ensure!(!result.frames.is_empty(), "image has no frames");
        Ok(result)
    }

    pub fn at(&self, time: Duration, seed: u64) -> Arc<RenderImage> {
        let total = self.ends.last().expect("nonempty").as_nanos();
        let at = (time.as_nanos() + u128::from(seed) * 617_000_000) % total;
        let index = self.ends.partition_point(|end| end.as_nanos() <= at);
        self.frames[index.min(self.frames.len() - 1)].clone()
    }
}

fn dimensions(decoder: &impl ImageDecoder) -> anyhow::Result<()> {
    let (width, height) = decoder.dimensions();
    anyhow::ensure!(
        (1..=2048).contains(&width) && (1..=2048).contains(&height),
        "image dimensions must be between 1 and 2048 pixels"
    );
    Ok(())
}
