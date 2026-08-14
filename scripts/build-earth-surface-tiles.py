#!/usr/bin/env python3
"""Build deterministic cube-sphere Earth surface tiles from an equirectangular image.

The generated paths match `planet_tiles::TileAssetRequest` exactly:

    assets/earth/tiles/surface/<face>/<level>/<x>/<y>.webp

This is an offline asset builder. The gallery remains static and has no Python,
NumPy, or Pillow dependency at runtime.
"""

from __future__ import annotations

import argparse
import json
import math
from pathlib import Path

try:
    import numpy as np
    from PIL import Image
except ImportError as error:  # pragma: no cover - dependency error is user-facing.
    raise SystemExit(
        "Building Earth surface tiles requires Pillow and NumPy. "
        "Install them in the asset-build environment and retry."
    ) from error

FACE_NAMES = ("px", "nx", "py", "ny", "pz", "nz")
DEFAULT_TILE_SIZE = 256
DEFAULT_QUALITY = 82
DEFAULT_MIN_LEVEL = 0
DEFAULT_MAX_LEVEL = 4


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("source", type=Path, help="2:1 equirectangular Earth surface image")
    parser.add_argument(
        "--output-root",
        type=Path,
        default=Path("assets/earth/tiles/surface"),
    )
    parser.add_argument("--tile-size", type=int, default=DEFAULT_TILE_SIZE)
    parser.add_argument("--quality", type=int, default=DEFAULT_QUALITY)
    parser.add_argument("--min-level", type=int, default=DEFAULT_MIN_LEVEL)
    parser.add_argument("--max-level", type=int, default=DEFAULT_MAX_LEVEL)
    return parser.parse_args()


def validate_args(args: argparse.Namespace) -> None:
    if not args.source.is_file():
        raise SystemExit(f"surface source does not exist: {args.source}")
    if args.tile_size <= 0:
        raise SystemExit("tile size must be positive")
    if not 0 <= args.quality <= 100:
        raise SystemExit("quality must be in 0..100")
    if args.min_level < 0 or args.max_level < args.min_level or args.max_level > 18:
        raise SystemExit("levels must satisfy 0 <= min-level <= max-level <= 18")


def cube_coordinates(face: str, u: np.ndarray, v: np.ndarray) -> tuple[np.ndarray, ...]:
    ones = np.ones_like(u)
    if face == "px":
        return ones, v, -u
    if face == "nx":
        return -ones, v, u
    if face == "py":
        return u, ones, -v
    if face == "ny":
        return u, -ones, v
    if face == "pz":
        return u, v, ones
    if face == "nz":
        return -u, v, -ones
    raise ValueError(f"unknown cube face: {face}")


def source_coordinates(
    face: str,
    level: int,
    tile_x: int,
    tile_y: int,
    tile_size: int,
    source_width: int,
    source_height: int,
) -> tuple[np.ndarray, np.ndarray]:
    side = 1 << level
    pixels_per_face = side * tile_size
    x = tile_x * tile_size + np.arange(tile_size, dtype=np.float64) + 0.5
    y = tile_y * tile_size + np.arange(tile_size, dtype=np.float64) + 0.5
    u = x / pixels_per_face * 2.0 - 1.0
    v = y / pixels_per_face * 2.0 - 1.0
    grid_u, grid_v = np.meshgrid(u, v)

    cube_x, cube_y, cube_z = cube_coordinates(face, grid_u, grid_v)
    inverse_length = 1.0 / np.sqrt(cube_x**2 + cube_y**2 + cube_z**2)
    direction_x = cube_x * inverse_length
    direction_y = cube_y * inverse_length
    direction_z = cube_z * inverse_length

    longitude = np.arctan2(direction_z, direction_x)
    latitude = np.arcsin(np.clip(direction_y, -1.0, 1.0))
    source_x = (longitude / (2.0 * math.pi) + 0.5) * source_width - 0.5
    source_y = (0.5 - latitude / math.pi) * source_height - 0.5
    return source_x, source_y


def bilinear_sample(source: np.ndarray, source_x: np.ndarray, source_y: np.ndarray) -> np.ndarray:
    height, width, _ = source.shape
    x0_floor = np.floor(source_x)
    y0_floor = np.floor(source_y)
    x_fraction = source_x - x0_floor
    y_fraction = source_y - y0_floor

    x0 = x0_floor.astype(np.int64) % width
    x1 = (x0 + 1) % width
    y0 = np.clip(y0_floor.astype(np.int64), 0, height - 1)
    y1 = np.clip(y0 + 1, 0, height - 1)

    top = source[y0, x0] * (1.0 - x_fraction[..., None]) + source[y0, x1] * x_fraction[..., None]
    bottom = source[y1, x0] * (1.0 - x_fraction[..., None]) + source[y1, x1] * x_fraction[..., None]
    return np.clip(
        top * (1.0 - y_fraction[..., None]) + bottom * y_fraction[..., None],
        0.0,
        255.0,
    ).astype(np.uint8)


def tile_path(root: Path, face: str, level: int, x: int, y: int) -> Path:
    return root / face / str(level) / str(x) / f"{y}.webp"


def build_tiles(args: argparse.Namespace) -> int:
    image = Image.open(args.source).convert("RGB")
    source_width, source_height = image.size
    if source_width != source_height * 2:
        raise SystemExit(
            "Earth surface source must use a 2:1 equirectangular projection; "
            f"got {source_width}x{source_height}."
        )
    source = np.asarray(image, dtype=np.float64)

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
                    pixels = bilinear_sample(source, source_x, source_y)
                    output = tile_path(args.output_root, face, level, tile_x, tile_y)
                    output.parent.mkdir(parents=True, exist_ok=True)
                    Image.fromarray(pixels, "RGB").save(
                        output,
                        "WEBP",
                        quality=args.quality,
                        method=6,
                    )
                    generated += 1

    expected = 6 * sum(4**level for level in range(args.min_level, args.max_level + 1))
    if generated != expected:
        raise RuntimeError(f"generated {generated} tiles, expected {expected}")

    manifest = {
        "projection": "cube-sphere",
        "faces": list(FACE_NAMES),
        "source": args.source.name,
        "source_width": source_width,
        "source_height": source_height,
        "tile_size": args.tile_size,
        "format": "webp",
        "quality": args.quality,
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
        f"Wrote {generated} cube-sphere surface tiles to {args.output_root} "
        f"(levels {args.min_level}..{args.max_level}, {args.tile_size}px)."
    )


if __name__ == "__main__":
    main()
