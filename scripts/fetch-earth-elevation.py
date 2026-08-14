#!/usr/bin/env python3
"""Fetch a compact measured Earth elevation texture from NOAA ETOPO 2022.

The runtime renderer expects an equirectangular grayscale texture with longitude
ordered west-to-east from -180 to +180 degrees and latitude north-to-south. NOAA's
ERDDAP copy of ETOPO 2022 exposes longitude in the 0..360 convention, so this
builder explicitly reorders it before resampling.

Source dataset:
  NOAA NCEI ETOPO 2022, 60 Arc-Second, Global (Ice Surface)
  https://oceanwatch.pifsc.noaa.gov/erddap/griddap/ETOPO_2022_v1_60s.html

NOAA states that the data may be used and redistributed for free. This script
fetches a strided numeric subset instead of requiring the full global source file.
It uses only the Python standard library.
"""

from __future__ import annotations

import argparse
import base64
import binascii
import csv
import io
import math
import struct
import urllib.parse
import urllib.request
import zlib
from pathlib import Path

MIN_ELEVATION_M = -11_000.0
MAX_ELEVATION_M = 9_000.0
DEFAULT_WIDTH = 512
DEFAULT_HEIGHT = 256
SOURCE_LATITUDE_COUNT = 10_800
SOURCE_LONGITUDE_COUNT = 21_600
DEFAULT_STRIDE = 42
ERDDAP_DATASET = (
    "https://oceanwatch.pifsc.noaa.gov/erddap/griddap/"
    "ETOPO_2022_v1_60s.csv"
)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--output",
        type=Path,
        default=Path("src/browser/earth_elevation_512.png.b64"),
        help="base64 PNG output path",
    )
    parser.add_argument("--width", type=int, default=DEFAULT_WIDTH)
    parser.add_argument("--height", type=int, default=DEFAULT_HEIGHT)
    parser.add_argument(
        "--stride",
        type=int,
        default=DEFAULT_STRIDE,
        help="ETOPO source-grid stride; 42 yields about 515x258 samples",
    )
    return parser.parse_args()


def build_url(stride: int) -> str:
    query = (
        f"z[0:{stride}:{SOURCE_LATITUDE_COUNT - 1}]"
        f"[0:{stride}:{SOURCE_LONGITUDE_COUNT - 1}]"
    )
    return f"{ERDDAP_DATASET}?{urllib.parse.quote(query, safe='[],:')}"


def wrap_longitude(longitude: float) -> float:
    return longitude - 360.0 if longitude > 180.0 else longitude


def download_grid(url: str) -> tuple[list[float], list[float], list[list[float]]]:
    request = urllib.request.Request(
        url,
        headers={"User-Agent": "aatuh-art-etopo-builder/1.0"},
    )
    with urllib.request.urlopen(request, timeout=120) as response:
        text = io.TextIOWrapper(response, encoding="utf-8", newline="")
        reader = csv.DictReader(text)
        samples: dict[tuple[float, float], float] = {}
        latitudes: set[float] = set()
        longitudes: set[float] = set()
        for row in reader:
            try:
                latitude = float(row["latitude"])
                longitude = wrap_longitude(float(row["longitude"]))
                elevation = float(row["z"])
            except (KeyError, TypeError, ValueError):
                # ERDDAP CSV responses include a units row after the header.
                continue
            if not math.isfinite(elevation) or elevation <= -90_000.0:
                raise SystemExit(
                    f"ETOPO response contains an invalid sample at {latitude}, {longitude}"
                )
            latitudes.add(latitude)
            longitudes.add(longitude)
            samples[(latitude, longitude)] = elevation

    ordered_latitudes = sorted(latitudes, reverse=True)
    ordered_longitudes = sorted(longitudes)
    if len(ordered_latitudes) < 2 or len(ordered_longitudes) < 2:
        raise SystemExit("ETOPO response did not contain a usable global grid")

    grid: list[list[float]] = []
    for latitude in ordered_latitudes:
        row_values = []
        for longitude in ordered_longitudes:
            try:
                row_values.append(samples[(latitude, longitude)])
            except KeyError as error:
                raise SystemExit(
                    f"ETOPO response is missing a sample at {latitude}, {longitude}"
                ) from error
        grid.append(row_values)
    return ordered_latitudes, ordered_longitudes, grid


def bilinear_resample(
    grid: list[list[float]],
    width: int,
    height: int,
) -> list[list[float]]:
    source_height = len(grid)
    source_width = len(grid[0])
    output: list[list[float]] = []
    for output_y in range(height):
        source_y = output_y * (source_height - 1) / max(height - 1, 1)
        y0 = int(math.floor(source_y))
        y1 = min(y0 + 1, source_height - 1)
        fy = source_y - y0
        output_row = []
        for output_x in range(width):
            source_x = output_x * (source_width - 1) / max(width - 1, 1)
            x0 = int(math.floor(source_x))
            x1 = min(x0 + 1, source_width - 1)
            fx = source_x - x0
            top = grid[y0][x0] * (1.0 - fx) + grid[y0][x1] * fx
            bottom = grid[y1][x0] * (1.0 - fx) + grid[y1][x1] * fx
            output_row.append(top * (1.0 - fy) + bottom * fy)
        output.append(output_row)
    return output


def encode_elevation(elevation_m: float) -> int:
    normalized = (elevation_m - MIN_ELEVATION_M) / (
        MAX_ELEVATION_M - MIN_ELEVATION_M
    )
    return round(max(0.0, min(1.0, normalized)) * 255.0)


def png_chunk(kind: bytes, payload: bytes) -> bytes:
    checksum = binascii.crc32(kind)
    checksum = binascii.crc32(payload, checksum) & 0xFFFFFFFF
    return struct.pack(">I", len(payload)) + kind + payload + struct.pack(">I", checksum)


def encode_grayscale_png(grid: list[list[float]]) -> bytes:
    height = len(grid)
    width = len(grid[0])
    scanlines = bytearray()
    for row in grid:
        scanlines.append(0)  # PNG filter type: None
        scanlines.extend(encode_elevation(value) for value in row)
    header = struct.pack(">IIBBBBB", width, height, 8, 0, 0, 0, 0)
    return (
        b"\x89PNG\r\n\x1a\n"
        + png_chunk(b"IHDR", header)
        + png_chunk(b"IDAT", zlib.compress(bytes(scanlines), level=9))
        + png_chunk(b"IEND", b"")
    )


def main() -> None:
    args = parse_args()
    if args.width <= 0 or args.height <= 0:
        raise SystemExit("width and height must be positive")
    if args.stride <= 0:
        raise SystemExit("stride must be positive")

    url = build_url(args.stride)
    print(f"Fetching measured ETOPO elevation from {url}")
    latitudes, longitudes, source_grid = download_grid(url)
    print(
        f"Downloaded {len(longitudes)}x{len(latitudes)} source samples; "
        f"resampling to {args.width}x{args.height}."
    )
    output_grid = bilinear_resample(source_grid, args.width, args.height)
    png = encode_grayscale_png(output_grid)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(base64.b64encode(png).decode("ascii") + "\n", encoding="ascii")
    print(
        f"Wrote measured ETOPO elevation texture to {args.output} "
        f"({MIN_ELEVATION_M:.0f}..{MAX_ELEVATION_M:.0f} m encoded as 0..255)."
    )


if __name__ == "__main__":
    main()
