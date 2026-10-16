#!/usr/bin/env python3
"""Turns cmux-todd-ui's APNG avatars into the animated WebPs in assets/agents.

    scripts/agents.py PATH/TO/cmux-todd-ui/sidebar/assets

gpui plays animated WebP and GIF; it shows an APNG's first frame alone.
"""

import sys
from pathlib import Path

from PIL import Image, ImageSequence

SIDE = 112
QUALITY = 75
OUT = Path(__file__).resolve().parent.parent / "assets" / "agents"


def convert(source: Path, target: Path) -> None:
    image = Image.open(source)
    duration = image.info.get("duration")
    if not duration:
        sys.exit(f"agents: {source} names no frame duration")
    frames = [
        frame.convert("RGBA").resize((SIDE, SIDE), Image.LANCZOS)
        for frame in ImageSequence.Iterator(image)
    ]
    frames[0].save(
        target,
        save_all=True,
        append_images=frames[1:],
        duration=int(duration),
        loop=0,
        quality=QUALITY,
        alpha_quality=70,
        method=6,
    )
    print(f"{target.name}: {len(frames)} frames, {target.stat().st_size // 1024} KB")


def main() -> None:
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    sources = sorted(Path(sys.argv[1]).glob("*-*.png"))
    if not sources:
        sys.exit(f"agents: no avatars in {sys.argv[1]}")
    OUT.mkdir(parents=True, exist_ok=True)
    for source in sources:
        convert(source, OUT / f"{source.stem}.webp")


if __name__ == "__main__":
    main()
