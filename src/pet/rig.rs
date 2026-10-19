//! Ely rigged as the films rig her: the icon's paths in layers of one color, each a mask gpui draws with a matrix of its own.

use gpui::{
    App, Bounds, Hsla, Pixels, Point, ScaledPixels, SharedString, TransformationMatrix, Window,
    point, px, radians, rgb, size,
};

use super::{
    WIDTH,
    look::{Eyes, Mark, Mouth, Pose},
};

/// A point or a shift in the window's device pixels.
type Device = (f32, f32);

/// The head's box in the icon's units; the edge she holds runs along its bottom.
const VIEW: [f32; 4] = [110.0, 6.0, 292.0, 280.0];
/// Points a unit draws at.
const UNIT: f32 = WIDTH / VIEW[2];

/// How a layer moves.
#[derive(Clone, Copy, PartialEq)]
enum Bone {
    /// Her neck and collar, which stay on the edge as her head bobs.
    Body,
    /// Bobs, and turns with her head.
    Head,
    /// An open eye's lid and lash, which a blink squashes; left is 0.
    Lid(usize),
    /// An open eye, which looks about as well.
    Iris(usize),
    Arc(usize),
    Lines,
    Brows,
    Blush,
    Mouth(Mouth),
    Lock(usize),
    Ahoge,
    /// Holds the edge; taps.
    Hand(usize),
    Mark(Mark),
}

/// A layer: its name, color and opacity, its box in the icon's units, the point it turns and grows about, how it moves, and its shapes.
struct Part {
    name: &'static str,
    color: u32,
    alpha: f32,
    frame: [f32; 4],
    pivot: (f32, f32),
    bone: Bone,
    shapes: &'static str,
}

/// A part with no pivot of its own.
#[rustfmt::skip]
const fn part(name: &'static str, color: u32, frame: [f32; 4], bone: Bone, shapes: &'static str) -> Part {
    Part { name, color, alpha: 1.0, frame, pivot: (256.0, 236.0), bone, shapes }
}

/// Every layer, back to front.
#[rustfmt::skip]
const PARTS: &[Part] = &[
    part("hair-back", 0xEC86B5, [117.0, 45.0, 278.0, 262.0], Bone::Head, r#"<path d="M256 46C192 46 142 90 136 158C131 214 126 262 118 306L394 306C386 262 381 214 376 158C370 90 320 46 256 46Z"/>"#),
    part("shade-back", 0xD9659A, [193.0, 199.0, 126.0, 78.0], Bone::Head, r#"<path d="M194 200C198 238 214 262 230 276L282 276C298 262 314 238 318 200Z"/>"#),
    part("neck", 0xF7CDBE, [235.0, 221.0, 42.0, 50.0], Bone::Body, r#"<path d="M236 222L276 222L276 266C268 270 244 270 236 266Z"/>"#),
    part("collar", 0xFFFFFF, [197.0, 255.0, 118.0, 30.0], Bone::Body, r#"<path d="M198 284C204 268 222 258 238 256L256 272L274 256C290 258 308 268 314 284Z"/>"#),
    part("bow", 0xF05A97, [221.0, 247.0, 70.0, 32.0], Bone::Body, r#"<path d="M256 264C244 250 226 248 224 258C222 270 236 278 256 264ZM256 264C268 250 286 248 288 258C290 270 276 278 256 264Z"/>"#),
    part("knot", 0xD94683, [247.0, 255.0, 18.0, 18.0], Bone::Body, r#"<circle cx="256" cy="264" r="8"/>"#),
    part("skin", 0xFFE7DC, [177.0, 93.0, 158.0, 152.0], Bone::Head, concat!(r#"<path d=""#, "M178 164C178 116 212 94 256 94C300 94 334 116 334 164C334 202 312 228 284 238C274 242 264 244 256 244C248 244 238 242 228 238C200 228 178 202 178 164Z", r#""/>"#)),
    part("face-shade", 0xF9D8CB, [161.0, 73.0, 190.0, 118.0], Bone::Head, concat!(r#"<defs><clipPath id="face"><path d=""#, "M178 164C178 116 212 94 256 94C300 94 334 116 334 164C334 202 312 228 284 238C274 242 264 244 256 244C248 244 238 242 228 238C200 228 178 202 178 164Z", r#""/></clipPath></defs><g clip-path="url(#face)"><path transform="translate(0 12)" d=""#, "M170 176C162 110 204 62 256 62C308 62 350 110 342 176Q328 152 318 134Q316 156 304 172Q298 150 286 130Q286 156 274 178Q268 150 256 128Q244 150 238 178Q226 156 226 130Q214 150 208 172Q196 156 194 134Q184 152 170 176Z", r#""/></g>"#)),
    eye("iris-l", 0xA24FB9, 222.0, 0, r#"<ellipse cx="222" cy="188" rx="18" ry="23"/>"#),
    eye("ring-l", 0xEE84C4, 222.0, 0, r#"<ellipse cx="222" cy="198" rx="13.5" ry="11.5"/>"#),
    eye("pupil-l", 0x5A2368, 222.0, 0, r#"<ellipse cx="222" cy="190" rx="7.5" ry="10.5"/>"#),
    eye("shine-l", 0xFFFFFF, 222.0, 0, r#"<circle cx="215" cy="178" r="7"/><circle cx="229" cy="199" r="3.2"/>"#),
    Part { name: "lid-l", color: 0x45233F, alpha: 1.0, frame: [193.0, 158.0, 52.0, 26.0], pivot: (222.0, 192.0), bone: Bone::Lid(0), shapes: r#"<path d="M194 182C195 168 208 159 224 159C234 159 241 164 244 172L240 173C236 168 231 165 224 165C214 165 207 172 204 183Z"/>"# },
    Part { name: "lash-l", color: 0xC2506F, alpha: 0.35, frame: [213.0, 210.0, 18.0, 6.0], pivot: (222.0, 192.0), bone: Bone::Lid(0), shapes: r#"<path fill="none" stroke="black" stroke-width="2" stroke-linecap="round" d="M215 212Q222 214 229 212"/>"# },
    eye("iris-r", 0xA24FB9, 290.0, 1, r#"<ellipse cx="290" cy="188" rx="18" ry="23"/>"#),
    eye("ring-r", 0xEE84C4, 290.0, 1, r#"<ellipse cx="290" cy="198" rx="13.5" ry="11.5"/>"#),
    eye("pupil-r", 0x5A2368, 290.0, 1, r#"<ellipse cx="290" cy="190" rx="7.5" ry="10.5"/>"#),
    eye("shine-r", 0xFFFFFF, 290.0, 1, r#"<circle cx="283" cy="178" r="7"/><circle cx="297" cy="199" r="3.2"/>"#),
    Part { name: "lid-r", color: 0x45233F, alpha: 1.0, frame: [267.0, 158.0, 52.0, 26.0], pivot: (290.0, 192.0), bone: Bone::Lid(1), shapes: r#"<path d="M318 182C317 168 304 159 288 159C278 159 271 164 268 172L272 173C276 168 281 165 288 165C298 165 305 172 308 183Z"/>"# },
    Part { name: "lash-r", color: 0xC2506F, alpha: 0.35, frame: [281.0, 210.0, 18.0, 6.0], pivot: (290.0, 192.0), bone: Bone::Lid(1), shapes: r#"<path fill="none" stroke="black" stroke-width="2" stroke-linecap="round" d="M283 212Q290 214 297 212"/>"# },
    Part { name: "arc-l", color: 0x45233F, alpha: 1.0, frame: [200.0, 171.0, 44.0, 31.0], pivot: (222.0, 192.0), bone: Bone::Arc(0), shapes: r#"<path fill="none" stroke="black" stroke-width="7" stroke-linecap="round" d="M205 197Q222 176 239 197"/>"# },
    Part { name: "arc-r", color: 0x45233F, alpha: 1.0, frame: [268.0, 171.0, 44.0, 31.0], pivot: (290.0, 192.0), bone: Bone::Arc(1), shapes: r#"<path fill="none" stroke="black" stroke-width="7" stroke-linecap="round" d="M273 197Q290 176 307 197"/>"# },
    part("lines", 0x45233F, [201.0, 190.0, 110.0, 8.0], Bone::Lines, r#"<path fill="none" stroke="black" stroke-width="6" stroke-linecap="round" d="M205 194H239M273 194H307"/>"#),
    Part { name: "blush", color: 0xFF8DAA, alpha: 0.55, frame: [180.0, 206.0, 152.0, 20.0], pivot: (256.0, 216.0), bone: Bone::Blush, shapes: r#"<ellipse cx="198" cy="216" rx="15" ry="8"/><ellipse cx="314" cy="216" rx="15" ry="8"/>"# },
    mouth("smile", 0xC2506F, [241.0, 219.0, 30.0, 13.0], Mouth::Smile, r#"<path fill="none" stroke="black" stroke-width="3.5" stroke-linecap="round" stroke-linejoin="round" d="M244 222Q250 229 256 223Q262 229 268 222"/>"#),
    mouth("grin", 0xC2506F, [242.0, 218.0, 28.0, 24.0], Mouth::Grin, r#"<path d="M243 219Q256 241 269 219Z"/>"#),
    mouth("tongue", 0xFF8DAA, [248.0, 224.0, 16.0, 13.0], Mouth::Grin, r#"<path d="M249 229Q256 236 263 229Q256 225 249 229Z"/>"#),
    mouth("gasp", 0xC2506F, [249.0, 218.0, 14.0, 16.0], Mouth::Gasp, r#"<ellipse cx="256" cy="226" rx="5.5" ry="7"/>"#),
    mouth("flat", 0xC2506F, [246.0, 222.0, 20.0, 6.0], Mouth::Flat, r#"<path fill="none" stroke="black" stroke-width="3.5" stroke-linecap="round" d="M249 225H263"/>"#),
    mouth("frown", 0xC2506F, [243.0, 217.0, 26.0, 14.0], Mouth::Frown, r#"<path fill="none" stroke="black" stroke-width="3.5" stroke-linecap="round" d="M246 228Q256 220 266 228"/>"#),
    Part { name: "lock-l", color: 0xF7A6C9, alpha: 1.0, frame: [147.0, 117.0, 54.0, 200.0], pivot: (184.0, 118.0), bone: Bone::Lock(0), shapes: r#"<path d="M184 118C164 150 156 204 160 258C162 282 156 300 148 316C164 312 178 300 186 286C190 256 192 214 198 176C200 152 196 132 184 118Z"/>"# },
    Part { name: "lock-r", color: 0xF7A6C9, alpha: 1.0, frame: [311.0, 117.0, 54.0, 200.0], pivot: (328.0, 118.0), bone: Bone::Lock(1), shapes: r#"<path d="M328 118C348 150 356 204 352 258C350 282 356 300 364 316C348 312 334 300 326 286C322 256 320 214 314 176C312 152 316 132 328 118Z"/>"# },
    part("bangs", 0xF7A6C9, [161.0, 61.0, 190.0, 118.0], Bone::Head, concat!(r#"<path d=""#, "M170 176C162 110 204 62 256 62C308 62 350 110 342 176Q328 152 318 134Q316 156 304 172Q298 150 286 130Q286 156 274 178Q268 150 256 128Q244 150 238 178Q226 156 226 130Q214 150 208 172Q196 156 194 134Q184 152 170 176Z", r#""/>"#)),
    part("gloss", 0xFFD1E4, [195.0, 83.0, 122.0, 26.0], Bone::Head, r#"<path d="M196 108C220 84 292 84 316 108C292 92 220 92 196 108Z"/>"#),
    Part { name: "ahoge", color: 0xF7A6C9, alpha: 1.0, frame: [237.0, 17.0, 44.0, 50.0], pivot: (258.0, 64.0), bone: Bone::Ahoge, shapes: r#"<path d="M250 66C238 40 252 18 280 20C264 28 258 44 262 64Z"/>"# },
    part("pin", 0xFFE48A, [303.0, 85.0, 30.0, 30.0], Bone::Head, r#"<path d="M318 86Q320 98 332 100Q320 102 318 114Q316 102 304 100Q316 98 318 86Z"/>"#),
    part("brows", 0x45233F, [202.0, 148.0, 108.0, 16.0], Bone::Brows, r#"<g fill="none" stroke="black" stroke-width="5" stroke-linecap="round"><path d="M206 160L234 152"/><path d="M306 160L278 152"/></g>"#),
    part("hand-l", 0xFFE7DC, [149.0, 261.0, 46.0, 26.0], Bone::Hand(0), r#"<path d="M150 286L150 272C150 264 161 264 161 272C161 262 172 262 172 271C172 262 183 262 183 272C183 264 194 264 194 273L194 286Z"/>"#),
    part("knuckles-l", 0xF2BBA8, [159.0, 271.0, 26.0, 17.0], Bone::Hand(0), r#"<path fill="none" stroke="black" stroke-width="2" stroke-linecap="round" d="M161 274V286M172 273V286M183 274V286"/>"#),
    part("hand-r", 0xFFE7DC, [317.0, 261.0, 46.0, 26.0], Bone::Hand(1), r#"<path d="M318 286L318 273C318 264 329 264 329 272C329 262 340 262 340 271C340 262 351 262 351 272C351 264 362 264 362 272L362 286Z"/>"#),
    part("knuckles-r", 0xF2BBA8, [327.0, 271.0, 26.0, 17.0], Bone::Hand(1), r#"<path fill="none" stroke="black" stroke-width="2" stroke-linecap="round" d="M329 274V286M340 273V286M351 274V286"/>"#),
    mark("bang", 0xFFB020, [368.0, 35.0, 20.0, 75.0], (378.0, 107.0), Mark::Bang, r#"<path d="M371 38L385 38L380 86L376 86Z"/><circle cx="378" cy="100" r="7"/>"#),
    mark("bang-edge", 0x45233F, [368.0, 35.0, 20.0, 75.0], (378.0, 107.0), Mark::Bang, r#"<g fill="none" stroke="black" stroke-width="3"><path d="M371 38L385 38L380 86L376 86Z"/><circle cx="378" cy="100" r="7"/></g>"#),
    mark("z1", 0x9FA3D9, [348.0, 66.0, 28.0, 30.0], (362.0, 81.0), Mark::Z1, r#"<path fill="none" stroke="black" stroke-width="5" stroke-linecap="round" stroke-linejoin="round" d="M352 70H372L352 92H372"/>"#),
    mark("z2", 0x9FA3D9, [376.0, 34.0, 22.0, 24.0], (387.0, 46.0), Mark::Z2, r#"<path fill="none" stroke="black" stroke-width="5" stroke-linecap="round" stroke-linejoin="round" d="M380 38H394L380 54H394"/>"#),
    mark("heart", 0xFF5C93, [349.0, 55.0, 58.0, 46.0], (378.0, 82.0), Mark::Heart, r#"<path d="M378 70C366 56 350 66 356 80C360 90 378 100 378 100C378 100 396 90 400 80C406 66 390 56 378 70Z"/>"#),
    mark("sparkle-l", 0xFFE48A, [123.0, 69.0, 34.0, 34.0], (140.0, 86.0), Mark::SparkleLeft, r#"<path d="M140 70Q143 84 156 86Q143 88 140 102Q137 88 124 86Q137 84 140 70Z"/>"#),
    mark("sparkle-r", 0xFFE48A, [367.0, 51.0, 26.0, 26.0], (380.0, 64.0), Mark::SparkleRight, r#"<path d="M380 52Q382 62 392 64Q382 66 380 76Q378 66 368 64Q378 62 380 52Z"/>"#),
    mark("drop", 0x9CD5FF, [336.0, 126.0, 22.0, 32.0], (346.0, 142.0), Mark::Drop, r#"<path d="M346 128Q356 144 350 152Q340 156 338 146Q338 138 346 128Z"/>"#),
    mark("drop-edge", 0x5AA0D8, [336.0, 126.0, 22.0, 32.0], (346.0, 142.0), Mark::Drop, r#"<path fill="none" stroke="black" stroke-width="2" d="M346 128Q356 144 350 152Q340 156 338 146Q338 138 346 128Z"/>"#),
];

/// A layer of an open eye centred at `x`.
#[rustfmt::skip]
const fn eye(name: &'static str, color: u32, x: f32, side: usize, shapes: &'static str) -> Part {
    let frame = [x - 19.0, 164.0, 38.0, 48.0];
    Part { name, color, alpha: 1.0, frame, pivot: (x, 192.0), bone: Bone::Iris(side), shapes }
}

#[rustfmt::skip]
const fn mouth(name: &'static str, color: u32, frame: [f32; 4], mouth: Mouth, shapes: &'static str) -> Part {
    Part { name, color, alpha: 1.0, frame, pivot: (256.0, 224.0), bone: Bone::Mouth(mouth), shapes }
}

#[rustfmt::skip]
const fn mark(name: &'static str, color: u32, frame: [f32; 4], pivot: (f32, f32), mark: Mark, shapes: &'static str) -> Part {
    Part { name, color, alpha: 1.0, frame, pivot, bone: Bone::Mark(mark), shapes }
}

/// Every layer's key and SVG, made once.
pub struct Rig {
    layers: Vec<(&'static Part, SharedString, Vec<u8>)>,
}

impl Rig {
    pub fn new() -> Self {
        let layers = (PARTS.iter())
            .map(|part| {
                let key = format!("ely/{}.svg", part.name).into();
                (part, key, svg(part).into_bytes())
            })
            .collect();
        Self { layers }
    }

    /// Draws her in `pose`, the head's box from `origin`.
    pub fn paint(&self, origin: Point<Pixels>, pose: &Pose, window: &mut Window, cx: &App) {
        let scale = window.scale_factor();
        let (left, top) = (f32::from(origin.x), f32::from(origin.y));
        let at = |(x, y): (f32, f32)| {
            let x = (left + (x - VIEW[0]) * UNIT) * scale;
            (x, (top + (y - VIEW[1]) * UNIT) * scale)
        };
        let by = |(x, y): (f32, f32)| (x * UNIT * scale, y * UNIT * scale);
        let whole = shift(TransformationMatrix::unit(), by((pose.shake, -pose.lift)));
        let breath = shift(whole, by((0.0, -pose.bob)));
        let head = turn(breath, at((256.0, 236.0)), pose.tilt);
        for (part, key, svg) in &self.layers {
            let Some((matrix, alpha)) = moved(part, pose, [whole, head], &at, &by) else {
                continue;
            };
            let [x, y, w, h] = part.frame;
            let bounds = Bounds::new(
                origin + point(px((x - VIEW[0]) * UNIT), px((y - VIEW[1]) * UNIT)),
                size(px(w * UNIT), px(h * UNIT)),
            );
            let color = Hsla::from(rgb(part.color)).opacity(part.alpha * alpha);
            let drawn =
                window.paint_svg(bounds, key.clone(), Some(svg.as_slice()), matrix, color, cx);
            if let Err(error) = drawn {
                log::error!("pet: Ely's {} does not draw: {error:#}", part.name);
            }
        }
    }
}

/// Where `part` stands in `pose`, and its opacity; none while the pose hides it.
fn moved(
    part: &Part,
    pose: &Pose,
    [whole, head]: [TransformationMatrix; 2],
    at: &impl Fn((f32, f32)) -> Device,
    by: &impl Fn((f32, f32)) -> Device,
) -> Option<(TransformationMatrix, f32)> {
    let pivot = at(part.pivot);
    let open = |side: usize| pose.eyes == Eyes::Open || (pose.eyes == Eyes::Wink && side == 1);
    let shut = |side: usize| pose.eyes == Eyes::Arcs || (pose.eyes == Eyes::Wink && side == 0);
    let squash = |side: usize, wide: f32| grow(head, pivot, (wide, wide * pose.open[side]));
    Some(match part.bone {
        Bone::Body => (whole, 1.0),
        Bone::Head => (head, 1.0),
        Bone::Lid(side) if open(side) => (squash(side, pose.wide), 1.0),
        Bone::Iris(side) if open(side) => (shift(squash(side, pose.wide), by(pose.look)), 1.0),
        Bone::Arc(side) if shut(side) => (squash(side, 1.0), 1.0),
        Bone::Lines if pose.eyes == Eyes::Lines => (head, 1.0),
        Bone::Brows if pose.brows > 0.0 => (head, pose.brows),
        Bone::Blush => (head, 1.0 + 0.6 * pose.flush),
        Bone::Mouth(mouth) if mouth == pose.mouth => (grow(head, pivot, (pose.pop, pose.pop)), 1.0),
        Bone::Lock(side) => {
            let sway = if side == 0 { pose.sway } else { -pose.sway };
            (turn(head, pivot, sway), 1.0)
        }
        Bone::Ahoge => (turn(head, pivot, pose.ahoge), 1.0),
        Bone::Hand(side) => (shift(whole, by((0.0, pose.hands[side]))), 1.0),
        Bone::Mark(mark) => {
            let &(_, motion) = pose.marks.iter().find(|(shown, _)| *shown == mark)?;
            let moved = shift(head, by(motion.at));
            let turned = turn(moved, pivot, motion.turn);
            (
                grow(turned, pivot, (motion.scale, motion.scale)),
                motion.alpha,
            )
        }
        _ => return None,
    })
}

fn shift(matrix: TransformationMatrix, (x, y): Device) -> TransformationMatrix {
    matrix.translate(point(ScaledPixels(x), ScaledPixels(y)))
}

/// `matrix`, after a turn of `degrees` clockwise about `pivot`.
fn turn(matrix: TransformationMatrix, pivot: Device, degrees: f32) -> TransformationMatrix {
    let turned = shift(matrix, pivot).rotate(radians(degrees.to_radians()));
    shift(turned, (-pivot.0, -pivot.1))
}

/// `matrix`, after a growth by `(x, y)` about `pivot`.
fn grow(matrix: TransformationMatrix, pivot: Device, (x, y): (f32, f32)) -> TransformationMatrix {
    shift(shift(matrix, pivot).scale(size(x, y)), (-pivot.0, -pivot.1))
}

/// The SVG of `part`: its shapes in its box.
fn svg(part: &Part) -> String {
    let [x, y, w, h] = part.frame;
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="{x} {y} {w} {h}" width="{w}" height="{h}">{}</svg>"#,
        part.shapes
    )
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use resvg::usvg;

    use super::*;

    #[test]
    fn every_layer_parses_and_stays_in_its_box() {
        let mut names = HashSet::new();
        for part in PARTS {
            assert!(names.insert(part.name), "{} twice", part.name);
            let tree = usvg::Tree::from_str(&svg(part), &usvg::Options::default())
                .unwrap_or_else(|error| panic!("{}: {error}", part.name));
            let drawn = tree.root().abs_stroke_bounding_box();
            let [_, _, width, height] = part.frame;
            assert!(
                drawn.left() >= 0.0
                    && drawn.top() >= 0.0
                    && drawn.right() <= width
                    && drawn.bottom() <= height,
                "{}: {drawn:?} spills out of {width} by {height}",
                part.name
            );
        }
    }

    #[test]
    fn she_is_drawn_from_every_part_the_icon_gives_her() {
        let named = |name: &str| PARTS.iter().any(|part| part.name == name);
        for name in [
            "hair-back",
            "skin",
            "bangs",
            "ahoge",
            "lock-l",
            "lock-r",
            "hand-l",
            "hand-r",
        ] {
            assert!(named(name), "{name}");
        }
        let marks = [
            Mark::Bang,
            Mark::Z1,
            Mark::Z2,
            Mark::Heart,
            Mark::SparkleLeft,
            Mark::SparkleRight,
            Mark::Drop,
        ];
        for mark in marks {
            assert!(
                PARTS.iter().any(|part| part.bone == Bone::Mark(mark)),
                "{mark:?}"
            );
        }
    }
}
