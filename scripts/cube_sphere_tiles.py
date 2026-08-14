"""Shared cube-sphere projection helpers for offline Earth asset builders."""

from __future__ import annotations

import math

import numpy as np

FACE_NAMES = ("px", "nx", "py", "ny", "pz", "nz")
MAX_LEVEL = 18


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
    if not 0 <= level <= MAX_LEVEL:
        raise ValueError(f"level must be in 0..{MAX_LEVEL}")
    side = 1 << level
    if not 0 <= tile_x < side or not 0 <= tile_y < side:
        raise ValueError("tile coordinates are outside the requested level")
    if tile_size <= 0 or source_width <= 0 or source_height <= 0:
        raise ValueError("tile and source dimensions must be positive")

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


def bilinear_sample(
    source: np.ndarray,
    source_x: np.ndarray,
    source_y: np.ndarray,
) -> np.ndarray:
    """Bilinearly samples a 2D or channel-last equirectangular array.

    Longitude wraps at the dateline and latitude clamps at the poles.
    The returned dtype is floating point so callers can apply layer-specific
    quantization after projection.
    """
    if source.ndim not in (2, 3):
        raise ValueError("source must be a 2D or channel-last 3D array")
    height, width = source.shape[:2]
    if height <= 0 or width <= 0:
        raise ValueError("source dimensions must be positive")

    x0_floor = np.floor(source_x)
    y0_floor = np.floor(source_y)
    x_fraction = source_x - x0_floor
    y_fraction = source_y - y0_floor

    x0 = x0_floor.astype(np.int64) % width
    x1 = (x0 + 1) % width
    y0 = np.clip(y0_floor.astype(np.int64), 0, height - 1)
    y1 = np.clip(y0 + 1, 0, height - 1)

    if source.ndim == 3:
        x_fraction = x_fraction[..., None]
        y_fraction = y_fraction[..., None]
    top = source[y0, x0] * (1.0 - x_fraction) + source[y0, x1] * x_fraction
    bottom = source[y1, x0] * (1.0 - x_fraction) + source[y1, x1] * x_fraction
    return top * (1.0 - y_fraction) + bottom * y_fraction


def expected_tile_count(min_level: int, max_level: int) -> int:
    if min_level < 0 or max_level < min_level or max_level > MAX_LEVEL:
        raise ValueError(f"levels must satisfy 0 <= min <= max <= {MAX_LEVEL}")
    return 6 * sum(4**level for level in range(min_level, max_level + 1))
