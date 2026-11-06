#!/usr/bin/env python3
"""Convert PCX images under a directory to PNG, keeping the directory tree.

For Civilization III palette-based PCX files, the reserved green and magenta
palette entries are written as alpha-transparent pixels in the PNG.

Usage:
    python3 tools/convert_pcx_to_png.py SOURCE_DIR DESTINATION_DIR

Requires Pillow: python3 -m pip install Pillow
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

_CIV3_KEY_COLORS = {
    (0, 255, 0),
    (253, 0, 255),
    (254, 0, 255),
    (255, 0, 255),
}


def rgba_with_palette_transparency(image, Image):
    """Expand a PCX image to RGBA, making Civ3's reserved key indices clear."""
    rgba = image.convert("RGBA")

    if image.mode == "P":
        palette = image.getpalette()
        if palette:
            # Civ3 reserves the final two entries (254 and 255) for green and
            # magenta transparency. Match by palette index, not pixel RGB, so
            # other palette entries with similar-looking colors stay intact.
            transparent = {
                index
                for index in (254, 255)
                if len(palette) >= (index + 1) * 3
                and tuple(palette[index * 3 : index * 3 + 3]) in _CIV3_KEY_COLORS
            }
            if transparent:
                indices = image.tobytes()
                alpha = Image.frombytes(
                    "L",
                    image.size,
                    bytes(0 if index in transparent else 255 for index in indices),
                )
                rgba.putalpha(alpha)
                return rgba

    # Fallback for PCX files decoded to true color or with a nonstandard
    # palette. The Civ3 pack uses the palette-index path above.
    pixels = rgba.load()
    for y in range(rgba.height):
        for x in range(rgba.width):
            if pixels[x, y][:3] in _CIV3_KEY_COLORS:
                pixels[x, y] = (*pixels[x, y][:3], 0)
    return rgba


def convert_tree(source: Path, destination: Path, overwrite: bool = False) -> tuple[int, int]:
    try:
        from PIL import Image
    except ImportError as exc:
        raise RuntimeError(
            "Pillow is required. Install it with: python3 -m pip install Pillow"
        ) from exc

    if not source.is_dir():
        raise ValueError(f"Source directory does not exist: {source}")

    files = sorted(
        path for path in source.rglob("*")
        if path.is_file() and path.suffix.lower() == ".pcx"
    )
    if not files:
        print(f"No PCX files found under {source}")
        return 0, 0

    converted = skipped = 0
    failures: list[str] = []
    for path in files:
        output = (destination / path.relative_to(source)).with_suffix(".png")
        if output.exists() and not overwrite:
            print(f"SKIP (exists): {output}")
            skipped += 1
            continue

        try:
            output.parent.mkdir(parents=True, exist_ok=True)
            with Image.open(path) as image:
                rgba = rgba_with_palette_transparency(image, Image)
                rgba.save(output, format="PNG", optimize=False)
            print(f"OK: {path.relative_to(source)} -> {output}")
            converted += 1
        except Exception as exc:  # report all bad files in one run
            failures.append(f"{path}: {exc}")
            print(f"ERROR: {path}: {exc}", file=sys.stderr)

    print(f"Finished: {converted} converted, {skipped} skipped, {len(failures)} failed")
    return converted, len(failures)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path, help="folder containing PCX images")
    parser.add_argument("destination", type=Path, help="folder to write PNGs into")
    parser.add_argument(
        "--overwrite", action="store_true", help="replace PNGs that already exist"
    )
    args = parser.parse_args()

    try:
        _, failures = convert_tree(args.source.expanduser(), args.destination.expanduser(), args.overwrite)
    except (RuntimeError, ValueError) as exc:
        print(f"Error: {exc}", file=sys.stderr)
        return 2
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
