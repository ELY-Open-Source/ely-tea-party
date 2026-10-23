#!/usr/bin/env python3
"""Builds the app's icon SVGs from a square, full-bleed art image.

    scripts/icon.py PATH/TO/ART.png
    scripts/icon.py --bite PATH/TO/ART.png

The first writes both SVGs; --bite writes ely-tea-party-bite.png, the macOS
icon the Dock now and then wears while the app runs, drawn through the
same shape by examples/icons.rs at 512 pixels.

The art goes in as a JPEG data URI, so each SVG stands alone. On macOS it is
clipped to the shape macOS 26 draws every app icon in: a continuous-corner
square 824 points wide at 100 in from each side of 1024, its corners 215 in
radius, as SwiftUI's RoundedRectangle(style: .continuous) draws them, under
upstream's drop shadow. Elsewhere the same shape fills the canvas.
"""

import base64
import io
import subprocess
import sys
import tempfile
from pathlib import Path

from PIL import Image

ICONS = Path(__file__).resolve().parent.parent / "assets" / "icons"
QUALITY = 92
# SwiftUI's continuous rounded rectangle: (100, 100, 824, 824) at radius 215, and (0, 0, 1024, 1024) at 267.
MACOS = "M924.00 512.00L924.00 595.34C924.00 689.97 924.00 737.29 907.89 788.23C887.65 843.84 843.84 887.65 788.23 907.89C737.29 924.00 689.97 924.00 595.34 924.00L428.66 924.00C334.03 924.00 286.71 924.00 235.77 907.89C180.16 887.65 136.35 843.84 116.11 788.23C100.00 737.29 100.00 689.97 100.00 595.34L100.00 428.66C100.00 334.03 100.00 286.71 116.11 235.77C136.35 180.16 180.16 136.35 235.77 116.11C286.71 100.00 334.03 100.00 428.66 100.00L595.34 100.00C689.97 100.00 737.29 100.00 788.23 116.11C843.84 136.35 887.65 180.16 907.89 235.77C924.00 286.71 924.00 334.03 924.00 428.66Z"
FULL = "M1024.00 512.00L1024.00 615.85C1024.00 733.37 1024.00 792.14 1004.00 855.39C978.86 924.46 924.46 978.86 855.39 1004.00C792.14 1024.00 733.37 1024.00 615.85 1024.00L408.15 1024.00C290.63 1024.00 231.86 1024.00 168.61 1004.00C99.54 978.86 45.14 924.46 20.00 855.39C0.00 792.14 0.00 733.37 0.00 615.85L0.00 408.15C0.00 290.63 0.00 231.86 20.00 168.61C45.14 99.54 99.54 45.14 168.61 20.00C231.86 0.00 290.63 0.00 408.15 0.00L615.85 0.00C733.37 0.00 792.14 0.00 855.39 20.00C924.46 45.14 978.86 99.54 1004.00 168.61C1024.00 231.86 1024.00 290.63 1024.00 408.15Z"


def art(path: Path) -> str:
    image = Image.open(path).convert("RGB").resize((1024, 1024), Image.LANCZOS)
    jpeg = io.BytesIO()
    image.save(jpeg, "JPEG", quality=QUALITY, optimize=True)
    return "data:image/jpeg;base64," + base64.b64encode(jpeg.getvalue()).decode()


def macos(href: str) -> str:
    return f"""<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="1024" viewBox="0 0 1024 1024">
  <defs>
    <clipPath id="body"><path d="{MACOS}"/></clipPath>
    <filter id="shadow" x="-10%" y="-10%" width="120%" height="125%"><feDropShadow dx="0" dy="12" stdDeviation="14" flood-color="#000000" flood-opacity=".35"/></filter>
  </defs>
  <path d="{MACOS}" fill="#0B1938" filter="url(#shadow)"/>
  <image x="100" y="100" width="824" height="824" clip-path="url(#body)" href="{href}"/>
</svg>
"""


def full(href: str) -> str:
    return f"""<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="1024" viewBox="0 0 1024 1024">
  <defs>
    <clipPath id="body"><path d="{FULL}"/></clipPath>
  </defs>
  <image x="0" y="0" width="1024" height="1024" clip-path="url(#body)" href="{href}"/>
</svg>
"""


def bite(path: Path) -> None:
    with tempfile.TemporaryDirectory() as tmp:
        svg = Path(tmp) / "bite.svg"
        svg.write_text(macos(art(path)))
        target = ICONS / "ely-tea-party-bite.png"
        subprocess.run(
            ["cargo", "run", "--quiet", "--example", "icons", "--", "png", str(svg), str(target)],
            cwd=ICONS.parent.parent,
            check=True,
        )
        print(f"{target.name}: {target.stat().st_size // 1024} KB")


def main() -> None:
    if len(sys.argv) == 3 and sys.argv[1] == "--bite":
        return bite(Path(sys.argv[2]))
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    href = art(Path(sys.argv[1]))
    for name, svg in [("ely-tea-party-macos.svg", macos(href)), ("ely-tea-party.svg", full(href))]:
        target = ICONS / name
        target.write_text(svg)
        print(f"{target.name}: {target.stat().st_size // 1024} KB")


if __name__ == "__main__":
    main()
