//! Renders the icon: `icons iconset SVG FOLDER`, `icons ico SVG FILE` or `icons png SVG FILE`.

use std::path::Path;

use anyhow::{Context as _, bail};
use resvg::{tiny_skia, usvg};

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let form = args
        .next()
        .context("icons: name a form, iconset, ico or png")?;
    let svg = args.next().context("icons: name an SVG to render")?;
    let out = args.next().context("icons: name where to write")?;
    let data = std::fs::read(&svg).with_context(|| format!("icons: cannot read {svg}"))?;
    let tree = usvg::Tree::from_data(&data, &usvg::Options::default())
        .with_context(|| format!("icons: {svg} is not an SVG"))?;
    match form.as_str() {
        "iconset" => iconset(&tree, &out)?,
        "ico" => ico(&tree, &out)?,
        "png" => std::fs::write(&out, draw(&tree, PNG)?)
            .with_context(|| format!("icons: cannot write {out}"))?,
        other => bail!("icons: no form {other}; iconset, ico or png"),
    }
    println!("icons: {svg} drawn into {out}");
    Ok(())
}

/// The side of a lone PNG, as the iconset's 256-point size at 2x.
const PNG: u32 = 512;

/// A macOS iconset: each size at 1x and 2x.
fn iconset(tree: &usvg::Tree, out: &str) -> anyhow::Result<()> {
    std::fs::create_dir_all(out).with_context(|| format!("icons: cannot make {out}"))?;
    for points in [16, 32, 128, 256, 512] {
        for (scale, suffix) in [(1, ""), (2, "@2x")] {
            let png = Path::new(out).join(format!("icon_{points}x{points}{suffix}.png"));
            std::fs::write(&png, draw(tree, points * scale)?)
                .with_context(|| format!("icons: cannot write {}", png.display()))?;
        }
    }
    Ok(())
}

/// A Windows icon: one PNG per size, as Windows reads since Vista.
fn ico(tree: &usvg::Tree, out: &str) -> anyhow::Result<()> {
    let sides = [16, 24, 32, 48, 64, 128, 256];
    let images = sides
        .iter()
        .map(|&side| draw(tree, side))
        .collect::<anyhow::Result<Vec<_>>>()?;
    let mut file = vec![0, 0, 1, 0];
    file.extend((images.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * images.len();
    for (side, png) in sides.iter().zip(&images) {
        // The format writes 256 as 0.
        let edge = (side % 256) as u8;
        file.extend([edge, edge, 0, 0]);
        file.extend(1u16.to_le_bytes());
        file.extend(32u16.to_le_bytes());
        file.extend((png.len() as u32).to_le_bytes());
        file.extend((offset as u32).to_le_bytes());
        offset += png.len();
    }
    for png in &images {
        file.extend(png);
    }
    std::fs::write(out, file).with_context(|| format!("icons: cannot write {out}"))
}

/// The side every size is drawn at before it shrinks.
const FULL: u32 = 1024;

/// The SVG drawn into a square PNG `side` pixels wide: drawn whole, then shrunk with Lanczos, since resvg samples an embedded picture without a filter fit for small sizes.
fn draw(tree: &usvg::Tree, side: u32) -> anyhow::Result<Vec<u8>> {
    let mut full = tiny_skia::Pixmap::new(FULL, FULL).context("icons: no pixmap")?;
    let fit = FULL as f32 / tree.size().width();
    resvg::render(
        tree,
        tiny_skia::Transform::from_scale(fit, fit),
        &mut full.as_mut(),
    );
    if side == FULL {
        return full.encode_png().context("icons: cannot encode a PNG");
    }
    let drawn = image::RgbaImage::from_raw(FULL, FULL, full.take()).context("icons: no image")?;
    let mut shrunk =
        image::imageops::resize(&drawn, side, side, image::imageops::FilterType::Lanczos3)
            .into_raw();
    // Lanczos overshoots; premultiplied color never exceeds its alpha.
    for pixel in shrunk.chunks_exact_mut(4) {
        let alpha = pixel[3];
        pixel[..3]
            .iter_mut()
            .for_each(|color| *color = (*color).min(alpha));
    }
    let size = tiny_skia::IntSize::from_wh(side, side).context("icons: no size")?;
    tiny_skia::Pixmap::from_vec(shrunk, size)
        .context("icons: no pixmap")?
        .encode_png()
        .context("icons: cannot encode a PNG")
}
