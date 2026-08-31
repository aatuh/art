#!/usr/bin/env python3
"""Build a compact research elevation texture from NOAA ETOPO 2022.

The input is the official global Ice Surface GeoTIFF. The output is an 8-bit
PNG encoded as base64 text. Elevation is mapped linearly from -11,000 m to
+9,000 m. The current browser renderer does not consume this artifact; use the
cube-sphere elevation builder for the planned ground-scale terrain pipeline.

Requires GDAL's `gdal_translate` command. Runtime/static hosting has no GDAL
or Python dependency because this script is only an offline asset builder.
"""

from __future__ import annotations

import argparse
import base64
import shutil
import subprocess
import tempfile
from pathlib import Path

MIN_ELEVATION_M = -11_000
MAX_ELEVATION_M = 9_000
DEFAULT_WIDTH = 512
DEFAULT_HEIGHT = 256


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("source", type=Path, help="ETOPO 2022 global surface GeoTIFF")
    parser.add_argument(
        "--output",
        type=Path,
        default=Path("target/earth-authoring/earth-elevation-512.png.b64"),
        help="research base64 PNG output path",
    )
    parser.add_argument("--width", type=int, default=DEFAULT_WIDTH)
    parser.add_argument("--height", type=int, default=DEFAULT_HEIGHT)
    return parser.parse_args()


def require_gdal_translate() -> str:
    executable = shutil.which("gdal_translate")
    if executable is None:
        raise SystemExit(
            "gdal_translate is required to build the Earth elevation texture. "
            "Install GDAL and retry."
        )
    return executable


def build_png(
    gdal_translate: str,
    source: Path,
    output_png: Path,
    width: int,
    height: int,
) -> None:
    subprocess.run(
        [
            gdal_translate,
            "-q",
            "-of",
            "PNG",
            "-ot",
            "Byte",
            "-r",
            "bilinear",
            "-outsize",
            str(width),
            str(height),
            "-scale",
            str(MIN_ELEVATION_M),
            str(MAX_ELEVATION_M),
            "0",
            "255",
            str(source),
            str(output_png),
        ],
        check=True,
    )


def main() -> None:
    args = parse_args()
    if not args.source.is_file():
        raise SystemExit(f"ETOPO source file does not exist: {args.source}")
    if args.width <= 0 or args.height <= 0:
        raise SystemExit("width and height must be positive")

    gdal_translate = require_gdal_translate()
    args.output.parent.mkdir(parents=True, exist_ok=True)

    with tempfile.TemporaryDirectory(prefix="earth-elevation-") as directory:
        png_path = Path(directory) / "earth-elevation.png"
        build_png(gdal_translate, args.source, png_path, args.width, args.height)
        encoded = base64.b64encode(png_path.read_bytes()).decode("ascii")
        args.output.write_text(encoded + "\n", encoding="ascii")

    print(
        f"Wrote {args.width}x{args.height} ETOPO elevation texture to {args.output} "
        f"({MIN_ELEVATION_M}..{MAX_ELEVATION_M} m encoded as 0..255)."
    )


if __name__ == "__main__":
    main()
