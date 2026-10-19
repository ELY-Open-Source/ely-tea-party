#!/usr/bin/env python3
"""Keys cmux-todd-ui's whale-girl clips into the mascot's animated WebPs in assets/mascot.

    scripts/mascot.py PATH/TO/cmux-todd-ui

The sessions column's art is 112 px; the mascot is drawn at 112 points, so a
Retina display needs twice that. This runs cmux-todd-ui's own keying, crop and
fade on its 960 px clips at 224 px, writing only into a temporary folder there.
Her calm still is her reference sheet keyed the same way, cut at the knees,
which her window's edge hides, so her head is the size it is in her clips.
"""

import sys
import tempfile
from pathlib import Path

from PIL import Image

SIDE = 224
QUALITY = 80
POSES = ["idle", "working", "working-special", "working-slack", "thinking", "needs_input", "sleeping"]
OUT = Path(__file__).resolve().parent.parent / "assets" / "mascot"
# How much of her standing height, from the top, the calm still keeps.
KEEP = 0.72


def calm(ui: Path) -> Image.Image:
    from animate import cutout

    keyed = cutout(Image.open(ui / "art" / "ref" / "deepseek.png"))
    left, top, right, bottom = keyed.getchannel("A").point(lambda a: 255 if a > 24 else 0).getbbox()
    knees = top + round((bottom - top) * KEEP)
    side = max(right - left, knees - top) * 1.08
    middle = (left + right) / 2
    box = (round(middle - side / 2), round(knees - side), round(middle + side / 2), knees)
    her = keyed.crop(box).resize((round(SIDE * 0.94),) * 2, Image.LANCZOS)
    canvas = Image.new("RGBA", (SIDE, SIDE))
    canvas.alpha_composite(her, ((SIDE - her.width) // 2, SIDE - her.height))
    return canvas


def main() -> None:
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    sys.path.insert(0, str(Path(sys.argv[1]) / "scripts"))
    import video

    OUT.mkdir(parents=True, exist_ok=True)
    still = OUT / "whale-calm.webp"
    calm(Path(sys.argv[1])).save(still, quality=QUALITY, alpha_quality=90, method=6)
    print(f"{still.name}: 1 frame, {still.stat().st_size // 1024} KB")
    with tempfile.TemporaryDirectory() as tmp:
        video.OUT = Path(tmp)
        for pose in POSES:
            frames = video.build(f"deepseek-{pose}", size=SIDE)
            target = OUT / f"whale-{pose}.webp"
            frames[0].save(
                target,
                save_all=True,
                append_images=frames[1:],
                duration=round(1000 / 12),
                loop=0,
                quality=QUALITY,
                alpha_quality=90,
                method=6,
            )
            print(f"{target.name}: {len(frames)} frames, {target.stat().st_size // 1024} KB")


if __name__ == "__main__":
    main()
