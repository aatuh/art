#!/usr/bin/env python3
"""Build cube-sphere Earth elevation tiles from the official NOAA ETOPO raster.

Tile paths match `planet_tiles::TileAssetRequest` exactly:

    assets/earth/tiles/elevation/<face>/<level>/<x>/<y>.png

Elevations in -11,000..+9,000 metres are normalized to an unsigned 16-bit
integer and packed into the PNG's R (high byte) and G (low byte) channels.
This survives normal browser image decoding while retaining ~0.31 m encoded
vertical resolution. B is reserved for future metadata and currently zero.

The builder is offline-only. It needs GDAL's `gdal_translate`, NumPy and Pillow;
the static gallery has none of those runtime dependencies.
"""

from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import tempfile
from pathlib import Path

try:
    import numpy as np
    from PIL import Image
except ImportError as error:  # pragma: no cover - dependency error is user-facing.
    raise SystemExit(
        "Building Earth elevation tiles requires Pillow and NumPy. "
        "Install them in the asset-build environment and retry."
    ) from error

from cube_sphere_tiles import (
    FACE_NAMES,
    MAX_LEVEL,
    bilinear_sample,
    expected_tile_count,
    source_coordinates,
)

MIN_ELEVATION_M = -11_000.0
MAX_ELEVATION_M = 9_000.0
DEFAULT_TILE_SIZE = 256
DEFAULT_MIN_LEVEL = 0
DEFAULT_MAX_LEVEL = 4
DEFAULT_SAMPLE_WIDTH = 8192


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("source", type=Path, help="ETOPO 2022 global surface GeoTIFF")
    parser.add_argument(
        "--output-root",
        type=Path,
        default=Path("assets/earth/tiles/elevation"),
    )
    parser.add_argument("--tile-size", type=int, default=DEFAULT_TILE_SIZE)
    parser.add_argument("--min-level", type=int, default=DEFAULT_MIN_LEVEL)
    parser.add_argument("--max-level", type=int, default=DEFAULT_MAX_LEVEL)
    parser.add_argument(
        "--sample-width",
        type=int,
        default=DEFAULT_SAMPLE_WIDTH,
        help="width of the temporary 2:1 equirectangular UInt16 raster",
    )
    return parser.parse_args()


def validate_args(args: argparse.Namespace) -> None:
    if not args.source.is_file():
        raise SystemExit(f"ETOPO source does not exist: {args.source}")
    if args.tile_size <= 0:
        raise SystemExit("tile size must be positive")
    if args.min_level < 0 or args.max_level < args.min_level or args.max_level > MAX_LEVEL:
        raise SystemExit(
            f"levels must satisfy 0 <= min-level <= max-level <= {MAX_LEVEL}"
        )
    if args.sample_width <= 0 or args.sample_width % 2:
        raise SystemExit("sample width must be a positive even number")


def require_gdal_translate() -> str:
    executable = shutil.which("gdal_translate")
    if executable is None:
        raise SystemExit(
            "gdal_translate is required to read the ETOPO GeoTIFF. "
            "Install GDAL in the asset-build environment and retry."
        )
    return executable


def build_equirectangular_sample(
    gdal_translate: str,
    source: Path,
    output: Path,
    width: int,
) -> None:
    subprocess.run(
        [
            gdal_translate,
            "-q",
            "-of",
            "PNG",
            "-ot",
            "UInt16",
            "-r",
            "bilinear",
            "-outsize",
            str(width),
            str(width // 2),
            "-scale",
            str(MIN_ELEVATION_M),
            str(MAX_ELEVATION_M),
            "0",
            "65535",
            str(source),
            str(output),
        ],
        check=True,
    )


def tile_path(root: Path, face: str, level: int, x: int, y: int) -> Path:
    return root / face / str(level) / str(x) / f"{y}.png"


def pack_rg16(values: np.ndarray) -> np.ndarray:
    encoded = np.clip(np.rint(values), 0.0, 65535.0).astype(np.uint16)
    packed = np.zeros((*encoded.shape, 3), dtype=np.uint8)
    packed[..., 0] = (encoded >> 8).astype(np.uint8)
    packed[..., 1] = (encoded & 0xFF).astype(np.uint8)
    return packed


def build_tiles(args: argparse.Namespace) -> int:
    gdal_translate = require_gdal_translate()
    with tempfile.TemporaryDirectory(prefix="earth-elevation-tiles-") as directory:
        sample_path = Path(directory) / "etopo-sample.png"
        build_equirectangular_sample(
            gdal_translate,
            args.source,
            sample_path,
            args.sample_width,
        )
        source = np.asarray(Image.open(sample_path), dtype=np.float64)
        if source.ndim != 2:
            raise RuntimeError(f"expected one elevation band, got shape {source.shape}")
        source_height, source_width = source.shape
        if source_width != source_height * 2:
            raise RuntimeError(
                "temporary ETOPO sample is not 2:1 equirectangular: "
                f"{source_width}x{source_height}"
            )

        generated = 0
        for level in range(args.min_level, args.max_level + 1):
            side = 1 << level
            for face in FACE_NAMES:
                for tile_y in range(side):
                    for tile_x in range(side):
                        source_x, source_y = source_coordinates(
                            face,
                            level,
                            tile_x,
                            tile_y,
                            args.tile_size,
                            source_width,
                            source_height,
                        )
                        sampled = bilinear_sample(source, source_x, source_y)
                        output = tile_path(args.output_root, face, level, tile_x, tile_y)
                        output.parent.mkdir(parents=True, exist_ok=True)
                        Image.fromarray(pack_rg16(sampled), "RGB").save(
                            output,
                            "PNG",
                            optimize=True,
                        )
                        generated += 1

    expected = expected_tile_count(args.min_level, args.max_level)
    if generated != expected:
        raise RuntimeError(f"generated {generated} tiles, expected {expected}")

    manifest = {
        "projection": "cube-sphere",
        "faces": list(FACE_NAMES),
        "source": args.source.name,
        "source_dataset": "NOAA ETOPO 2022 Ice Surface",
        "tile_size": args.tile_size,
        "format": "png",
        "encoding": "rg16-unorm",
        "minimum_elevation_m": MIN_ELEVATION_M,
        "maximum_elevation_m": MAX_ELEVATION_M,
        "sample_width": args.sample_width,
        "sample_height": args.sample_width // 2,
        "min_level": args.min_level,
        "max_level": args.max_level,
        "tile_count": generated,
    }
    args.output_root.mkdir(parents=True, exist_ok=True)
    (args.output_root / "tileset.json").write_text(
        json.dumps(manifest, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    return generated


def main() -> None:
    args = parse_args()
    validate_args(args)
    generated = build_tiles(args)
    print(
        f"Wrote {generated} cube-sphere ETOPO tiles to {args.output_root} "
        f"(levels {args.min_level}..{args.max_level}, {args.tile_size}px)."
    )


if __name__ == "__main__":
    main()
