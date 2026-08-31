#!/usr/bin/env python3
"""Generate the deterministic, static textures used by A World in Light.

The pinned NASA Blue Marble source supplies measured geography and surface colour. This offline
builder derives a material mask and adds reproducible sub-source relief, bathymetry, polar ice,
organic weather detail, and a diffuse octahedral Milky Way map. With the default dimensions, the
output files are exactly the five assets documented in docs/earth-rendering.md. The browser never
runs Python or contacts NASA.

Requires Pillow, NumPy, and SciPy in the asset-authoring environment.
"""

from __future__ import annotations

import argparse
from pathlib import Path

try:
    import numpy as np
    from PIL import Image
    from scipy.ndimage import distance_transform_edt, gaussian_filter
except ImportError as error:  # pragma: no cover - dependency error is user-facing.
    raise SystemExit(
        "Generating Earth textures requires Pillow, NumPy, and SciPy. "
        "Install them in the asset-authoring environment and retry."
    ) from error

DEFAULT_WIDTH = 2048
DEFAULT_HEIGHT = 1024
DEFAULT_SEED = 0x45524154
SEA_LEVEL = 0.5
MIN_ELEVATION_M = -10_000.0
MAX_ELEVATION_M = 10_000.0
DEFAULT_SURFACE_SOURCE = Path("target/earth-authoring/world.200406.3x5400x2700.jpg")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Generate deterministic gallery-ready Earth material and sky textures."
    )
    parser.add_argument(
        "--surface-source",
        type=Path,
        default=DEFAULT_SURFACE_SOURCE,
        help="pinned 2:1 NASA Blue Marble authoring image",
    )
    parser.add_argument(
        "--output-root",
        type=Path,
        default=Path("assets/earth"),
        help="directory that receives the five generated assets",
    )
    parser.add_argument("--width", type=int, default=DEFAULT_WIDTH, help="surface texture width")
    parser.add_argument("--height", type=int, default=DEFAULT_HEIGHT, help="surface texture height")
    parser.add_argument("--seed", type=int, default=DEFAULT_SEED, help="deterministic random seed")
    return parser.parse_args()


def smoothstep(edge0: float, edge1: float, value: np.ndarray) -> np.ndarray:
    scaled = np.clip((value - edge0) / (edge1 - edge0), 0.0, 1.0)
    return scaled * scaled * (3.0 - 2.0 * scaled)


def resize_field(field: np.ndarray, width: int, height: int) -> np.ndarray:
    image = Image.fromarray(field.astype(np.float32), mode="F")
    resized = image.resize((width, height), Image.Resampling.BICUBIC)
    return np.asarray(resized, dtype=np.float32)


def validate_longitude_periodicity(field: np.ndarray, label: str) -> None:
    """Reject a longitude seam whose curvature is unlike the field interior."""
    if field.ndim not in (2, 3) or field.shape[1] < 4:
        raise ValueError(f"{label} must have at least four longitude samples")

    interior_curvature = np.abs(
        field[:, 2:, ...] - 2.0 * field[:, 1:-1, ...] + field[:, :-2, ...]
    )
    west_curvature = np.abs(
        field[:, 1, ...] - 2.0 * field[:, 0, ...] + field[:, -1, ...]
    )
    east_curvature = np.abs(
        field[:, 0, ...] - 2.0 * field[:, -1, ...] + field[:, -2, ...]
    )
    interior_mean = float(np.mean(interior_curvature))
    seam_mean = 0.5 * float(np.mean(west_curvature) + np.mean(east_curvature))
    tolerance = max(interior_mean * 2.5, float(np.finfo(np.float32).eps) * 16.0)
    if seam_mean > tolerance:
        raise RuntimeError(
            f"{label} has a discontinuous longitude seam "
            f"({seam_mean:.6g} curvature; expected at most {tolerance:.6g})"
        )


def periodic_noise(
    rng: np.random.Generator,
    width: int,
    height: int,
    cells_x: int,
    cells_y: int,
) -> np.ndarray:
    """Return bicubic value noise with a smooth periodic longitude."""
    # Keep the historical draw dimensions so this change does not perturb the
    # generator's shared random stream (and therefore unrelated later assets).
    sampled_grid = rng.random((cells_y + 1, cells_x + 1), dtype=np.float32)
    longitude_period = sampled_grid[:, :cells_x]
    tiled_grid = np.tile(longitude_period, (1, 3))
    image = Image.fromarray(tiled_grid, mode="F")
    expanded = image.resize((width * 3, height), Image.Resampling.BICUBIC)
    noise = np.asarray(expanded, dtype=np.float32)[:, width : width * 2].copy()
    noise = np.clip(noise, 0.0, 1.0)
    validate_longitude_periodicity(noise, "periodic noise")
    return noise


def fbm(
    rng: np.random.Generator,
    width: int,
    height: int,
    base_cells_x: int,
    octaves: int,
) -> np.ndarray:
    value = np.zeros((height, width), dtype=np.float32)
    weight = 0.0
    amplitude = 1.0
    cells_x = base_cells_x
    for _ in range(octaves):
        cells_y = max(2, cells_x // 2)
        value += periodic_noise(rng, width, height, cells_x, cells_y) * amplitude
        weight += amplitude
        amplitude *= 0.5
        cells_x *= 2
    return value / weight


def read_measured_surface(path: Path, width: int, height: int) -> tuple[np.ndarray, np.ndarray]:
    if not path.is_file():
        raise SystemExit(
            f"Blue Marble source does not exist: {path}. "
            "Run python3 scripts/fetch-blue-marble.py first."
        )
    try:
        with Image.open(path) as source:
            source.load()
            if source.width != source.height * 2 or source.width < width * 2:
                raise SystemExit(
                    "Blue Marble source must be 2:1 and at least twice the material width; "
                    f"got {source.width}x{source.height}."
                )
            measured = source.convert("RGB")
            surface = measured.resize((width * 2, height * 2), Image.Resampling.LANCZOS)
            material_source = measured.resize((width, height), Image.Resampling.LANCZOS)
    except (OSError, Image.DecompressionBombError) as error:
        raise SystemExit(f"could not decode Blue Marble source: {error}") from error
    return (
        np.asarray(surface, dtype=np.float32) / 255.0,
        np.asarray(material_source, dtype=np.float32) / 255.0,
    )


def signed_wrapped_distance(mask: np.ndarray) -> np.ndarray:
    """Distance in source pixels with a seamless antimeridian."""
    width = mask.shape[1]
    tiled = np.tile(mask, (1, 3))
    signed = distance_transform_edt(tiled) - distance_transform_edt(~tiled)
    return signed[:, width : width * 2].astype(np.float32)


def longitude_distance(longitude: np.ndarray, center: float) -> np.ndarray:
    return np.abs((longitude - center + 180.0) % 360.0 - 180.0)


def latitude_window(latitude: np.ndarray, low: float, high: float, feather: float = 5.0) -> np.ndarray:
    return smoothstep(low - feather, low + feather, latitude) * (
        1.0 - smoothstep(high - feather, high + feather, latitude)
    )


def mountain_chains(longitude: np.ndarray, latitude: np.ndarray) -> np.ndarray:
    """Broad deterministic relief hints for Earth's major mountain systems."""
    chains = np.zeros_like(longitude, dtype=np.float32)
    definitions = (
        (-72.0, -55.0, 12.0, 4.0, 1.00),  # Andes
        (-116.0, 24.0, 63.0, 6.5, 0.70),  # Rockies
        (82.0, 23.0, 39.0, 9.0, 1.00),  # Himalaya / Tibetan plateau
        (11.0, 39.0, 49.0, 7.0, 0.55),  # Alps
        (39.0, -22.0, 18.0, 6.0, 0.50),  # East African highlands
        (146.0, -44.0, -12.0, 7.0, 0.45),  # Eastern Australia / New Guinea
    )
    for center, low, high, width, strength in definitions:
        ridge = np.exp(-np.square(longitude_distance(longitude, center) / width))
        chains = np.maximum(chains, ridge * latitude_window(latitude, low, high) * strength)
    return chains


def wrapped_longitude_delta(longitude: np.ndarray, center: float) -> np.ndarray:
    """Signed shortest angular distance from `center`, in degrees."""
    return (longitude - center + 180.0) % 360.0 - 180.0


def cyclonic_cloud_system(
    longitude: np.ndarray,
    latitude: np.ndarray,
    distortion: np.ndarray,
    structure: np.ndarray,
    center_longitude: float,
    center_latitude: float,
    radius_degrees: float,
    arm_count: int,
    phase: float,
) -> tuple[np.ndarray, np.ndarray]:
    """Return an asymmetric cloud shield with embedded cyclonic rain bands."""
    delta_longitude = wrapped_longitude_delta(longitude, center_longitude)
    local_x = delta_longitude * np.cos(np.radians(center_latitude))
    local_y = latitude - center_latitude
    # Coherent displacement keeps the analytic circulation from reading as a perfect icon.
    warped_x = local_x + (distortion - 0.5) * radius_degrees * 0.20
    warped_y = local_y + (structure - 0.5) * radius_degrees * 0.16
    radius = np.sqrt(warped_x * warped_x + warped_y * warped_y)
    normalized_radius = radius / radius_degrees
    angle = np.arctan2(warped_y, warped_x)
    handedness = 1.0 if center_latitude >= 0.0 else -1.0
    logarithmic_radius = np.log(np.maximum(normalized_radius, 0.075))
    spiral_phase = (
        float(arm_count) * (angle - handedness * logarithmic_radius * 1.34)
        + phase
        + (distortion - 0.5) * 3.8
        + (structure - 0.5) * 1.7
    )
    arm_wave = 0.5 + 0.5 * np.cos(spiral_phase)
    fragmented_support = smoothstep(
        0.34,
        0.71,
        distortion * 0.63 + structure * 0.37,
    )
    azimuthal_support = 0.58 + 0.42 * (
        0.5 + 0.5 * np.cos(angle - phase * 0.37 + normalized_radius * 1.9)
    )
    arm_ridge = smoothstep(0.48, 0.92, arm_wave)
    arm_ridge *= (0.20 + fragmented_support * 0.80) * azimuthal_support

    envelope = np.exp(-np.square(normalized_radius * 0.94))
    outer_taper = 1.0 - smoothstep(1.05, 1.48, normalized_radius)
    cloud_shield = smoothstep(
        0.37,
        0.70,
        structure * 0.57 + distortion * 0.43,
    )
    cloud_shield *= envelope * (0.48 + azimuthal_support * 0.52)
    central_dense_overcast = np.exp(-np.square(normalized_radius / 0.31))
    clear_eye = smoothstep(0.035, 0.085, normalized_radius)
    eyewall = np.exp(-np.square((normalized_radius - 0.105) / 0.047))
    cloud_texture = 0.42 + distortion * 0.38 + structure * 0.20
    low_cloud = np.clip(
        outer_taper
        * clear_eye
        * (
            envelope * arm_ridge * (0.40 + cloud_texture * 0.38)
            + cloud_shield * 0.46
            + central_dense_overcast * (0.38 + cloud_texture * 0.22)
        )
        + eyewall * (0.14 + distortion * 0.12),
        0.0,
        1.0,
    )

    outflow_envelope = np.exp(-np.square(normalized_radius * 0.68)) * outer_taper
    outflow_wave = 0.5 + 0.5 * np.cos(
        spiral_phase - handedness * normalized_radius * 0.9
    )
    cirrus = np.clip(
        outflow_envelope
        * (
            cloud_shield * 0.32
            + smoothstep(0.57, 0.94, outflow_wave) * fragmented_support * 0.20
        )
        * smoothstep(0.055, 0.19, normalized_radius),
        0.0,
        1.0,
    )
    return low_cloud.astype(np.float32), cirrus.astype(np.float32)


def build_weather_texture(
    rng: np.random.Generator,
    width: int,
    height: int,
) -> np.ndarray:
    """Build coherent coverage, erosion, and cirrus fields for the cloud shader.

    The four fBm calls intentionally retain their historical order and dimensions. That keeps
    the shared generator's random stream stable, so improving weather cannot silently alter the
    independently generated starfield.
    """
    weather_broad = fbm(rng, width, height, 10, 5)
    weather_detail = fbm(rng, width, height, 32, 4)
    weather_cirrus = fbm(rng, width, height, 18, 5)
    weather_cells = fbm(rng, width, height, 6, 4)

    y = np.linspace(0.0, 1.0, height, endpoint=False, dtype=np.float32)[:, None]
    x = np.linspace(0.0, 1.0, width, endpoint=False, dtype=np.float32)[None, :]
    latitude = (90.0 - y * 180.0) + np.zeros((1, width), dtype=np.float32)
    longitude = (x * 360.0 - 180.0) + np.zeros((height, 1), dtype=np.float32)
    longitude_radians = np.radians(longitude)

    synoptic_seed = weather_broad * 0.68 + weather_cells * 0.32
    synoptic = gaussian_filter(
        synoptic_seed,
        sigma=(3.2, 7.5),
        mode=("reflect", "wrap"),
    )
    elongated_detail = gaussian_filter(
        weather_detail,
        sigma=(1.1, 8.0),
        mode=("reflect", "wrap"),
    )

    northern_track_center = 49.0 + 5.5 * np.sin(longitude_radians * 2.0 + 0.7)
    southern_track_center = -51.0 + 6.5 * np.sin(longitude_radians * 3.0 - 0.4)
    northern_track = np.exp(-np.square((latitude - northern_track_center) / 12.0))
    southern_track = np.exp(-np.square((latitude - southern_track_center) / 13.0))
    storm_tracks = np.maximum(northern_track, southern_track)
    storm_modulation = 0.72 + 0.28 * np.sin(longitude_radians * 5.0 + latitude * 0.045)
    storm_support = smoothstep(0.43, 0.60, synoptic * 0.72 + elongated_detail * 0.28)
    storm_tracks *= np.clip(storm_modulation, 0.0, 1.0) * (0.28 + storm_support * 0.72)

    itcz_center = (
        3.5
        + 3.2 * np.sin(longitude_radians - 0.5)
        + 1.4 * np.sin(longitude_radians * 3.0 + 0.8)
    )
    itcz = np.exp(-np.square((latitude - itcz_center) / 7.0))
    itcz_support = smoothstep(0.43, 0.62, weather_cells * 0.68 + synoptic * 0.32)
    itcz *= 0.34 + itcz_support * 0.66

    subtropical_belt = np.exp(-np.square((np.abs(latitude) - 27.0) / 8.5))
    regional_dryness = np.zeros_like(latitude, dtype=np.float32)
    for center_longitude, center_latitude, width_degrees, height_degrees, strength in (
        (18.0, 24.0, 31.0, 11.0, 1.00),
        (52.0, 25.0, 18.0, 9.0, 0.72),
        (134.0, -25.0, 25.0, 12.0, 0.88),
        (-72.0, -23.0, 14.0, 8.0, 0.74),
        (20.0, -23.0, 18.0, 9.0, 0.62),
    ):
        dry_region = np.exp(
            -np.square(
                wrapped_longitude_delta(longitude, center_longitude) / width_degrees
            )
            -np.square((latitude - center_latitude) / height_degrees)
        )
        regional_dryness = np.maximum(regional_dryness, dry_region * strength)
    dryness = np.clip(subtropical_belt * (0.10 + regional_dryness * 0.68), 0.0, 1.0)

    coverage_signal = (
        synoptic * 0.72
        + elongated_detail * 0.16
        + storm_tracks * (0.19 + synoptic * 0.19)
        + itcz * (0.22 + weather_cells * 0.12)
        - dryness * 0.47
    )
    # Earth is cloudy more often than a sparse decorative map suggests. Keep the
    # dry subtropical masks intact while widening ordinary frontal/ITCZ support.
    low_cloud = smoothstep(0.40, 0.68, coverage_signal)

    cyclone_low = np.zeros_like(low_cloud, dtype=np.float32)
    cyclone_cirrus = np.zeros_like(low_cloud, dtype=np.float32)
    for cyclone in (
        (-52.0, 27.0, 17.0, 2, 2.1, 0.84),
        (142.0, 17.0, 18.0, 2, 1.0, 0.91),
        (79.0, -23.0, 17.0, 2, 2.7, 0.82),
    ):
        spiral_low, spiral_cirrus = cyclonic_cloud_system(
            longitude,
            latitude,
            elongated_detail,
            weather_cells,
            cyclone[0],
            cyclone[1],
            cyclone[2],
            cyclone[3],
            cyclone[4],
        )
        cyclone_low = np.maximum(cyclone_low, spiral_low * cyclone[5])
        cyclone_cirrus = np.maximum(cyclone_cirrus, spiral_cirrus * cyclone[5])
    cyclone_low = gaussian_filter(
        cyclone_low,
        sigma=(1.1, 2.2),
        mode=("reflect", "wrap"),
    )
    cyclone_cirrus = gaussian_filter(
        cyclone_cirrus,
        sigma=(1.8, 4.5),
        mode=("reflect", "wrap"),
    )
    low_cloud = np.maximum(low_cloud, cyclone_low * (0.72 + synoptic * 0.16))

    # A synoptic envelope says where clouds can form; the existing higher-frequency
    # weather field supplies condensate variation inside it. Encoding that variation
    # in the map lets the orbital LOD use coherent, filtered detail instead of a sparse
    # screen-sampled 3-D lattice whose cells can read as a repeating stipple pattern.
    condensate_detail = smoothstep(0.28, 0.72, weather_detail)
    low_cloud = np.clip(low_cloud * (0.25 + condensate_detail * 0.95), 0.0, 1.0)

    front_detail = np.abs(weather_detail - elongated_detail)
    erosion = np.clip(
        low_cloud * (0.24 + elongated_detail * 0.54)
        + smoothstep(0.035, 0.16, front_detail) * low_cloud * 0.16
        + cyclone_low * 0.12,
        0.0,
        0.84,
    )

    cirrus_flow = gaussian_filter(
        weather_cirrus,
        sigma=(1.0, 11.0),
        mode=("reflect", "wrap"),
    )
    cirrus_wave = 0.5 + 0.5 * np.sin(
        longitude_radians * 7.0 + latitude * 0.055 + weather_broad * 2.4
    )
    cirrus_signal = cirrus_flow * 0.72 + cirrus_wave * 0.18 + storm_tracks * 0.16
    cirrus = smoothstep(0.65, 0.78, cirrus_signal)
    cirrus *= np.clip(storm_tracks * 0.72 + itcz * 0.26 + 0.10, 0.0, 1.0)
    cirrus = np.maximum(cirrus, cyclone_cirrus * 0.92)

    polar_fade = 1.0 - smoothstep(78.0, 89.0, np.abs(latitude))
    weather = np.stack(
        (
            np.clip(low_cloud * polar_fade, 0.0, 1.0),
            np.clip(erosion * polar_fade, 0.0, 1.0),
            np.clip(cirrus * polar_fade, 0.0, 1.0),
        ),
        axis=-1,
    ).astype(np.float32)
    validate_longitude_periodicity(weather, "weather texture")
    return weather


def build_starfield(rng: np.random.Generator, size: int) -> Image.Image:
    """Generate a subtle diffuse Milky Way; crisp stars are rendered as point sprites."""
    coordinate = np.linspace(-1.0, 1.0, size, endpoint=False, dtype=np.float32)
    octahedral_x = coordinate[None, :] + np.zeros((size, 1), dtype=np.float32)
    octahedral_y = coordinate[:, None] + np.zeros((1, size), dtype=np.float32)
    direction_x = octahedral_x.copy()
    direction_y = octahedral_y.copy()
    direction_z = 1.0 - np.abs(direction_x) - np.abs(direction_y)
    fold = np.clip(-direction_z, 0.0, 1.0)
    direction_x += np.where(direction_x >= 0.0, -fold, fold)
    direction_y += np.where(direction_y >= 0.0, -fold, fold)
    inverse_length = 1.0 / np.sqrt(
        direction_x * direction_x + direction_y * direction_y + direction_z * direction_z
    )
    direction_x *= inverse_length
    direction_y *= inverse_length
    direction_z *= inverse_length
    galactic_axis = np.asarray([0.24, 0.91, -0.34], dtype=np.float32)
    galactic_axis /= np.linalg.norm(galactic_axis)
    plane_distance = np.abs(
        direction_x * galactic_axis[0]
        + direction_y * galactic_axis[1]
        + direction_z * galactic_axis[2]
    )
    dust = fbm(rng, size, size, 8, 5)
    milky_way = np.exp(-np.square(plane_distance * 8.5)) * smoothstep(0.28, 0.80, dust)
    background = milky_way[..., None] * np.asarray([5.0, 7.0, 13.0], dtype=np.float32) / 255.0
    return Image.fromarray(
        (np.clip(background, 0.0, 1.0) * 255.0).astype(np.uint8),
        mode="RGB",
    )


def build_terrain_heightfield(material_rgba: np.ndarray) -> bytes:
    """Encode final shader terrain height as one normalized unsigned byte per texel.

    The input is the exact RGBA8 array written to the material PNG. Decoding those quantized
    red and green channels here keeps the height asset's texel centres consistent with the
    shader's land transition and positive-elevation semantics. WebGL and the CPU can then
    bilinearly filter this final-height field without max-pooling discontinuities.
    """
    if material_rgba.dtype != np.uint8:
        raise ValueError("terrain height input must be the final quantized RGBA8 material")
    if material_rgba.ndim != 3 or material_rgba.shape[2] != 4:
        raise ValueError("terrain height input must have RGBA channels")

    normalized = material_rgba.astype(np.float32) / 255.0
    land_amount = smoothstep(0.42, 0.58, normalized[..., 0])
    elevation_m = np.clip(
        MIN_ELEVATION_M
        + normalized[..., 1] * (MAX_ELEVATION_M - MIN_ELEVATION_M),
        0.0,
        MAX_ELEVATION_M,
    )
    final_height_m = land_amount * elevation_m
    encoded = np.rint(final_height_m * (255.0 / MAX_ELEVATION_M))
    return np.clip(encoded, 0.0, 255.0).astype(np.uint8).tobytes(order="C")


def build_textures(
    surface_source: Path,
    output_root: Path,
    width: int,
    height: int,
    seed: int,
) -> None:
    if (
        width < 512
        or height < 256
        or width != height * 2
    ):
        raise SystemExit("texture dimensions must be 2:1 and at least 512x256")
    if output_root.is_symlink():
        raise SystemExit(f"output root must not be a symlink: {output_root}")
    if output_root.exists() and not output_root.is_dir():
        raise SystemExit(f"output root is not a directory: {output_root}")

    rng = np.random.default_rng(seed)
    surface, measured_material = read_measured_surface(surface_source, width, height)
    red, green, blue = np.moveaxis(measured_material, 2, 0)
    ocean_score = smoothstep(1.06, 1.18, blue / np.maximum(green, 0.006))
    ocean_score *= smoothstep(1.08, 1.26, blue / np.maximum(red, 0.006))
    land_coverage = np.clip(
        1.0 - gaussian_filter(ocean_score, sigma=0.75, mode=("reflect", "wrap")),
        0.0,
        1.0,
    )
    land = land_coverage >= 0.5
    signed_distance = signed_wrapped_distance(land)
    continent_field = signed_distance

    y = np.linspace(0.0, 1.0, height, endpoint=False, dtype=np.float32)[:, None]
    x = np.linspace(0.0, 1.0, width, endpoint=False, dtype=np.float32)[None, :]
    latitude = (90.0 - y * 180.0) + np.zeros((1, width), dtype=np.float32)
    longitude = (x * 360.0 - 180.0) + np.zeros((height, 1), dtype=np.float32)
    latitude_abs = np.abs(latitude) / 90.0

    terrain_noise = fbm(rng, width, height, 12, 6)
    ridge_noise = 1.0 - np.abs(fbm(rng, width, height, 24, 5) * 2.0 - 1.0)
    ridge_noise = np.power(np.clip(ridge_noise, 0.0, 1.0), 4.0)
    interior = smoothstep(0.2, 5.0, np.maximum(continent_field, 0.0))
    chains = mountain_chains(longitude, latitude)
    relief = np.clip(
        0.05
        + terrain_noise * 0.32
        + ridge_noise * (0.18 + interior * 0.25)
        + chains * (0.28 + ridge_noise * 0.52),
        0.0,
        1.0,
    )

    ocean_distance = np.maximum(-continent_field, 0.0)
    ocean_depth = np.clip(0.08 + smoothstep(0.0, 12.0, ocean_distance) * 0.75, 0.0, 1.0)
    ocean_depth *= 0.82 + 0.18 * terrain_noise
    elevation = np.where(land, SEA_LEVEL + relief * 0.46, SEA_LEVEL - ocean_depth * 0.46)

    temperature = np.clip(1.12 - latitude_abs * 1.30 - relief * 0.30, 0.0, 1.0)

    ice_noise = fbm(rng, width, height, 18, 4)
    polar_ice = smoothstep(0.72 + (ice_noise - 0.5) * 0.08, 0.88, latitude_abs)
    alpine_snow = smoothstep(0.73, 0.94, relief) * smoothstep(0.05, 0.55, 0.64 - temperature)
    measured_brightness = measured_material.mean(axis=2)
    measured_chroma = measured_material.max(axis=2) - measured_material.min(axis=2)
    measured_snow = smoothstep(0.48, 0.82, measured_brightness) * (
        1.0 - smoothstep(0.035, 0.15, measured_chroma)
    )
    snow_amount = np.clip(
        np.maximum(np.maximum(polar_ice, alpine_snow), measured_snow) * land_coverage,
        0.0,
        1.0,
    )

    micro = fbm(rng, width, height, 80, 3)

    roughness = np.where(
        land,
        np.clip(0.72 + (micro - 0.5) * 0.18 - snow_amount * 0.18, 0.42, 0.95),
        0.20,
    )
    material = np.stack(
        (
            land_coverage,
            np.clip(elevation, 0.0, 1.0),
            np.clip(roughness, 0.0, 1.0),
            snow_amount,
        ),
        axis=-1,
    )
    material_rgba = (material * 255.0 + 0.5).astype(np.uint8)

    weather = build_weather_texture(rng, width // 2, height // 2)

    output_root.mkdir(parents=True, exist_ok=True)
    surface_path = output_root / f"earth-surface-{width * 2}.webp"
    material_path = output_root / f"earth-material-{width}.png"
    weather_path = output_root / f"earth-weather-{width // 2}.webp"
    stars_path = output_root / f"stars-{width // 2}.png"
    terrain_height_path = output_root / f"earth-terrain-height-{width}x{height}-u8.bin"

    Image.fromarray((surface * 255.0 + 0.5).astype(np.uint8), mode="RGB").save(
        surface_path,
        "WEBP",
        quality=94,
        method=6,
    )
    Image.fromarray(material_rgba, mode="RGBA").save(
        material_path,
        "PNG",
        optimize=True,
    )
    Image.fromarray((weather * 255.0 + 0.5).astype(np.uint8), mode="RGB").save(
        weather_path,
        "WEBP",
        lossless=True,
        quality=100,
        method=6,
    )
    build_starfield(rng, width // 2).save(stars_path, "PNG", optimize=True)
    if terrain_height_path.is_symlink():
        raise SystemExit(f"terrain height output must not be a symlink: {terrain_height_path}")
    terrain_height_path.write_bytes(build_terrain_heightfield(material_rgba))

    print(f"Wrote {surface_path} ({width * 2}x{height * 2})")
    print(f"Wrote {material_path} ({width}x{height})")
    print(f"Wrote {weather_path} ({width // 2}x{height // 2})")
    print(f"Wrote {stars_path} ({width // 2}x{width // 2})")
    print(f"Wrote {terrain_height_path} ({width}x{height}, normalized u8 height)")


def main() -> None:
    args = parse_args()
    build_textures(args.surface_source, args.output_root, args.width, args.height, args.seed)


if __name__ == "__main__":
    main()
