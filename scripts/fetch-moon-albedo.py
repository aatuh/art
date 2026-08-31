#!/usr/bin/env python3
"""Fetch and encode the pinned NASA LRO lunar albedo map used by the gallery.

The source URL, byte length, dimensions, and SHA-256 digest are fixed. The browser never runs this
script or contacts NASA; it loads only the committed same-origin WebP generated here.

Requires Pillow with WebP support in the asset-authoring environment.
"""

from __future__ import annotations

import hashlib
import io
import os
import sys
import tempfile
import urllib.error
import urllib.request
from pathlib import Path

try:
    from PIL import Image, features
except ImportError as error:  # pragma: no cover - dependency error is user-facing.
    raise SystemExit(
        "Building the lunar albedo texture requires Pillow. "
        "Install it in the asset-authoring environment and retry."
    ) from error

REPOSITORY_ROOT = Path(__file__).resolve().parent.parent
SOURCE_URL = (
    "https://svs.gsfc.nasa.gov/vis/a000000/a004700/a004720/"
    "lroc_color_poles_2k.tif"
)
SOURCE_SHA256 = "13b797422e8c4b8607ff2b2623ac3a046a6da0132d567c2d272d92fad7052c4a"
SOURCE_BYTES = 3_339_438
SOURCE_DIMENSIONS = (2_048, 1_024)
SOURCE_CACHE = REPOSITORY_ROOT / "target/earth-authoring/lroc_color_poles_2k.tif"
OUTPUT_PATH = REPOSITORY_ROOT / "assets/earth/moon-albedo-2048.webp"


def validate_payload(payload: bytes) -> None:
    if len(payload) != SOURCE_BYTES:
        raise SystemExit(
            f"NASA lunar source has {len(payload)} bytes; expected exactly {SOURCE_BYTES}."
        )
    digest = hashlib.sha256(payload).hexdigest()
    if digest != SOURCE_SHA256:
        raise SystemExit(
            "NASA lunar source SHA-256 mismatch; refusing to use changed or damaged data."
        )


def download_source() -> bytes:
    request = urllib.request.Request(
        SOURCE_URL,
        headers={"User-Agent": "aatuh-art-lunar-asset-builder/1.0"},
    )
    try:
        with urllib.request.urlopen(request, timeout=60) as response:
            if response.geturl() != SOURCE_URL:
                raise SystemExit("NASA lunar source unexpectedly redirected; refusing download.")
            content_length = response.headers.get("Content-Length")
            if content_length is not None:
                try:
                    reported_length = int(content_length)
                except ValueError as error:
                    raise SystemExit(
                        "NASA lunar source returned a malformed Content-Length."
                    ) from error
                if reported_length != SOURCE_BYTES:
                    raise SystemExit(
                        "NASA lunar source Content-Length changed; refusing download."
                    )
            payload = response.read(SOURCE_BYTES + 1)
    except (TimeoutError, urllib.error.URLError) as error:
        raise SystemExit(f"could not fetch the pinned NASA lunar source: {error}") from error

    validate_payload(payload)
    return payload


def load_source() -> bytes:
    if SOURCE_CACHE.is_symlink():
        raise SystemExit(f"refusing symlinked lunar source cache: {SOURCE_CACHE}")
    if SOURCE_CACHE.exists():
        if not SOURCE_CACHE.is_file():
            raise SystemExit(f"lunar source cache is not a regular file: {SOURCE_CACHE}")
        payload = SOURCE_CACHE.read_bytes()
        validate_payload(payload)
        return payload

    payload = download_source()
    SOURCE_CACHE.parent.mkdir(parents=True, exist_ok=True)
    if SOURCE_CACHE.parent.is_symlink():
        raise SystemExit(f"refusing symlinked lunar cache directory: {SOURCE_CACHE.parent}")
    temporary_path: Path | None = None
    try:
        with tempfile.NamedTemporaryFile(
            dir=SOURCE_CACHE.parent,
            prefix=".lroc-color-",
            suffix=".tif",
            delete=False,
        ) as temporary:
            temporary.write(payload)
            temporary_path = Path(temporary.name)
        os.replace(temporary_path, SOURCE_CACHE)
        temporary_path = None
    finally:
        if temporary_path is not None:
            temporary_path.unlink(missing_ok=True)
    return payload


def decode_source(payload: bytes) -> Image.Image:
    try:
        with Image.open(io.BytesIO(payload)) as source:
            if source.format != "TIFF" or source.size != SOURCE_DIMENSIONS:
                raise SystemExit(
                    "NASA lunar source is not the expected 2048x1024 TIFF image."
                )
            source.load()
            return source.convert("RGB")
    except (OSError, Image.DecompressionBombError) as error:
        raise SystemExit(f"could not decode the pinned NASA lunar source: {error}") from error


def encode_webp(image: Image.Image) -> None:
    if not features.check("webp"):
        raise SystemExit("this Pillow build does not support WebP encoding")
    if OUTPUT_PATH.is_symlink():
        raise SystemExit(f"refusing symlinked lunar output path: {OUTPUT_PATH}")
    OUTPUT_PATH.parent.mkdir(parents=True, exist_ok=True)

    temporary_path: Path | None = None
    try:
        with tempfile.NamedTemporaryFile(
            dir=OUTPUT_PATH.parent,
            prefix=".moon-albedo-",
            suffix=".webp",
            delete=False,
        ) as temporary:
            temporary_path = Path(temporary.name)
        image.save(temporary_path, "WEBP", quality=94, method=6)
        with Image.open(temporary_path) as encoded:
            encoded.load()
            if encoded.format != "WEBP" or encoded.size != SOURCE_DIMENSIONS:
                raise SystemExit("generated lunar albedo WebP failed validation")
        temporary_path.chmod(0o644)
        os.replace(temporary_path, OUTPUT_PATH)
        temporary_path = None
    finally:
        if temporary_path is not None:
            temporary_path.unlink(missing_ok=True)


def main() -> None:
    if len(sys.argv) != 1:
        raise SystemExit("Usage: python3 scripts/fetch-moon-albedo.py")
    source = load_source()
    encode_webp(decode_source(source))
    digest = hashlib.sha256(OUTPUT_PATH.read_bytes()).hexdigest()
    print(f"Wrote {OUTPUT_PATH.relative_to(REPOSITORY_ROOT)}")
    print(f"Source SHA-256: {SOURCE_SHA256}")
    print(f"Output SHA-256: {digest}")


if __name__ == "__main__":
    main()
