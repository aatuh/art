#!/usr/bin/env python3
"""Fetch and encode the pinned NASA Black Marble night-light map.

The browser never contacts NASA. This authoring command validates a fixed NASA Earth
Observatory JPEG by URL, byte length, SHA-256 and dimensions, then writes the bounded
same-origin WebP used by the artwork.
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
        "Building the Black Marble texture requires Pillow with WebP support."
    ) from error

REPOSITORY_ROOT = Path(__file__).resolve().parent.parent
SOURCE_URL = (
    "https://assets.science.nasa.gov/content/dam/science/esd/eo/images/"
    "imagerecords/144000/144898/BlackMarble_2016_01deg.jpg"
)
SOURCE_SHA256 = "d87de751a264e4f8ff69c68de5dab9606daee87a6f15ae743c93200743bd7ec1"
SOURCE_BYTES = 779_638
SOURCE_DIMENSIONS = (3_600, 1_800)
OUTPUT_DIMENSIONS = (2_048, 1_024)
SOURCE_CACHE = REPOSITORY_ROOT / "target/earth-authoring/BlackMarble_2016_01deg.jpg"
OUTPUT_PATH = REPOSITORY_ROOT / "assets/earth/earth-night-2048.webp"


def validate_payload(payload: bytes) -> None:
    if len(payload) != SOURCE_BYTES:
        raise SystemExit(
            f"NASA Black Marble source has {len(payload)} bytes; expected {SOURCE_BYTES}."
        )
    if hashlib.sha256(payload).hexdigest() != SOURCE_SHA256:
        raise SystemExit("NASA Black Marble SHA-256 mismatch; refusing changed data.")


def download_source() -> bytes:
    request = urllib.request.Request(
        SOURCE_URL,
        headers={"User-Agent": "aatuh-art-black-marble-builder/1.0"},
    )
    try:
        with urllib.request.urlopen(request, timeout=60) as response:
            if response.geturl() != SOURCE_URL:
                raise SystemExit("NASA Black Marble source unexpectedly redirected.")
            reported_length = response.headers.get("Content-Length")
            if reported_length is not None and int(reported_length) != SOURCE_BYTES:
                raise SystemExit("NASA Black Marble Content-Length changed.")
            payload = response.read(SOURCE_BYTES + 1)
    except (TimeoutError, urllib.error.URLError) as error:
        raise SystemExit(f"could not fetch NASA Black Marble: {error}") from error
    validate_payload(payload)
    return payload


def load_source() -> bytes:
    if SOURCE_CACHE.is_symlink():
        raise SystemExit(f"refusing symlinked source cache: {SOURCE_CACHE}")
    if SOURCE_CACHE.exists():
        if not SOURCE_CACHE.is_file():
            raise SystemExit(f"source cache is not a regular file: {SOURCE_CACHE}")
        payload = SOURCE_CACHE.read_bytes()
        validate_payload(payload)
        return payload

    payload = download_source()
    SOURCE_CACHE.parent.mkdir(parents=True, exist_ok=True)
    temporary_path: Path | None = None
    try:
        with tempfile.NamedTemporaryFile(
            dir=SOURCE_CACHE.parent,
            prefix=".black-marble-",
            suffix=".jpg",
            delete=False,
        ) as temporary:
            temporary.write(payload)
            temporary.flush()
            os.fsync(temporary.fileno())
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
            if source.format != "JPEG" or source.size != SOURCE_DIMENSIONS:
                raise SystemExit("NASA Black Marble source is not the pinned 3600x1800 JPEG.")
            source.load()
            return source.convert("RGB").resize(OUTPUT_DIMENSIONS, Image.Resampling.LANCZOS)
    except (OSError, Image.DecompressionBombError) as error:
        raise SystemExit(f"could not decode NASA Black Marble: {error}") from error


def encode_webp(image: Image.Image) -> None:
    if not features.check("webp"):
        raise SystemExit("this Pillow build does not support WebP encoding")
    if OUTPUT_PATH.is_symlink():
        raise SystemExit(f"refusing symlinked output path: {OUTPUT_PATH}")
    OUTPUT_PATH.parent.mkdir(parents=True, exist_ok=True)
    temporary_path: Path | None = None
    try:
        with tempfile.NamedTemporaryFile(
            dir=OUTPUT_PATH.parent,
            prefix=".earth-night-",
            suffix=".webp",
            delete=False,
        ) as temporary:
            temporary_path = Path(temporary.name)
        image.save(temporary_path, "WEBP", quality=92, method=6)
        with Image.open(temporary_path) as encoded:
            encoded.load()
            if encoded.format != "WEBP" or encoded.size != OUTPUT_DIMENSIONS:
                raise SystemExit("generated Black Marble WebP failed validation")
        temporary_path.chmod(0o644)
        os.replace(temporary_path, OUTPUT_PATH)
        temporary_path = None
    finally:
        if temporary_path is not None:
            temporary_path.unlink(missing_ok=True)


def main() -> None:
    if len(sys.argv) != 1:
        raise SystemExit("Usage: python3 scripts/fetch-black-marble.py")
    encode_webp(decode_source(load_source()))
    digest = hashlib.sha256(OUTPUT_PATH.read_bytes()).hexdigest()
    print(f"Wrote {OUTPUT_PATH.relative_to(REPOSITORY_ROOT)}")
    print(f"Source SHA-256: {SOURCE_SHA256}")
    print(f"Output SHA-256: {digest}")


if __name__ == "__main__":
    main()
