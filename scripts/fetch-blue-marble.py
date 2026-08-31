#!/usr/bin/env python3
"""Fetch the pinned NASA Blue Marble authoring source used by the Earth textures.

The browser never runs this script or contacts NASA. The fixed source is downloaded only into the
ignored target/ authoring directory, validated by byte count, SHA-256, format, and dimensions, and
then consumed by generate-procedural-earth.py.
"""

from __future__ import annotations

import hashlib
import io
import os
import sys
import tempfile
import urllib.request
from pathlib import Path

try:
    from PIL import Image
except ImportError as error:  # pragma: no cover - dependency error is user-facing.
    raise SystemExit("Fetching the Blue Marble source requires Pillow.") from error

SOURCE_URL = (
    "https://assets.science.nasa.gov/content/dam/science/esd/eo/images/"
    "bmng/bmng-base/june/world.200406.3x5400x2700.jpg"
)
SOURCE_SHA256 = "dc059529b8ed1cfcdd59747a7c3f259c93c1322bf88ca1ec8bab1dd71fcc57f5"
OUTPUT = Path("target/earth-authoring/world.200406.3x5400x2700.jpg")
MAXIMUM_DOWNLOAD_BYTES = 3_000_000


def download() -> bytes:
    request = urllib.request.Request(
        SOURCE_URL,
        headers={"User-Agent": "aatuh-art-blue-marble-builder/1.0"},
    )
    with urllib.request.urlopen(request, timeout=60) as response:
        length = response.headers.get("Content-Length")
        if length is not None and int(length) > MAXIMUM_DOWNLOAD_BYTES:
            raise SystemExit("Blue Marble response exceeds the authoring size limit")
        payload = response.read(MAXIMUM_DOWNLOAD_BYTES + 1)
    if len(payload) > MAXIMUM_DOWNLOAD_BYTES:
        raise SystemExit("Blue Marble response exceeds the authoring size limit")
    if hashlib.sha256(payload).hexdigest() != SOURCE_SHA256:
        raise SystemExit("Blue Marble response does not match the pinned SHA-256")
    return payload


def validate_image(payload: bytes) -> None:
    with Image.open(io.BytesIO(payload)) as image:
        if image.format != "JPEG" or image.size != (5400, 2700):
            raise SystemExit(
                f"expected the pinned 5400x2700 JPEG, got {image.format} {image.size}"
            )
        image.verify()


def write_atomically(payload: bytes) -> None:
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    temporary_path: Path | None = None
    try:
        with tempfile.NamedTemporaryFile(dir=OUTPUT.parent, delete=False) as temporary:
            temporary.write(payload)
            temporary.flush()
            os.fsync(temporary.fileno())
            temporary_path = Path(temporary.name)
        temporary_path.replace(OUTPUT)
    finally:
        if temporary_path is not None and temporary_path.exists():
            temporary_path.unlink()


def main() -> None:
    if len(sys.argv) != 1:
        raise SystemExit("Usage: python3 scripts/fetch-blue-marble.py")
    payload = download()
    validate_image(payload)
    write_atomically(payload)
    print(f"Wrote pinned NASA Blue Marble source to {OUTPUT}")


if __name__ == "__main__":
    main()
