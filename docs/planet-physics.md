# Planet simulation: numerical and physical foundation

This document defines the physical-coordinate, time, ephemeris, and visitor-navigation model for
**A World in Light**. The model preserves useful astronomical scale and lighting relationships; it
is an artwork simulation, not an orbit propagator or scientific observation tool.

## Implemented foundation

The renderer-independent domain code provides:

- SI-unit, double-precision positions for Earth, Moon, Sun, and the visitor;
- WGS 84 dimensions and an oblate Earth ellipsoid;
- real body radii, astronomical-unit scale, and varying Earth–Moon/Sun distances;
- deterministic low-cost apparent geocentric solar and lunar positions from seconds since J2000;
- an IERS-style Earth Rotation Angle with the J2000 phase and sidereal rate;
- camera-relative conversion before values are narrowed for WebGL;
- a shared, bilinearly filtered Earth height field for rendered displacement and navigation;
- six-direction exhibition flight with terrain-clearance-sensitive speed and solid-body
  constraints; and
- deterministic tests for scale ratios, angular sizes, ephemerides, ellipsoid intersections,
  camera precision, movement, recovery, and collision tunnelling.

## Coordinate and precision contract

- Unit: metre.
- Domain and navigation precision: `f64`.
- Origin: Earth's centre.
- Positive `Y`: northward equatorial/celestial direction.
- Earth-fixed longitude: zero on `+X`, increasing toward `+Z`; the celestial-to-terrestrial
  transform subtracts the IERS Earth Rotation Angle from inertial longitude. Positive solar
  declination therefore points north during northern-hemisphere summer.
- Current GPU boundary: subtract the visitor position in `f64`, then cast the resulting
  camera-relative metre vector to `f32` for the active shader's `*_m` uniforms.

Subtracting the camera first is essential. Casting an absolute lunar- or solar-distance position to
`f32` would discard metre- and kilometre-scale detail near a body. The active orbital shader keeps
camera-relative metres and normalizes individual sphere/ellipsoid equations internally where that
improves numerical conditioning; it does not globally divide all uniform positions into Earth-radius
units.

## Time, rotation, and apparent ephemerides

The browser initializes simulation time from its UTC wall clock relative to J2000. A deterministic
clock advances it at `1×`, `60×`, `3600×`, or paused. Negative/non-finite frame progression cannot
rewind the clock. The same simulated interval drives Earth rotation, apparent Sun/Moon positions,
and artistic material animation.

Earth remains at the geocentric origin. The Sun model uses dominant solar mean-anomaly,
eccentricity, ecliptic-longitude, obliquity, and distance terms. The Moon model uses dominant lunar
mean-longitude, anomaly, latitude/inclination, and distance terms. This is enough to preserve
plausible changing angular size, phase, and eclipse geometry, but it omits higher-order
perturbations, light-time iteration, precession/nutation, libration, topocentric parallax, and
barycentric N-body dynamics.

Earth rotation follows the IERS Earth Rotation Angle phase and rate while treating elapsed browser
UTC seconds as approximate UT1. DUT1, leap-second table updates, polar motion, precession, and
nutation are not applied. Consequently the surface orientation is visually grounded but is not a
navigation-grade Greenwich orientation for an exact observation time.

## Recorded constants

| Quantity | Value used |
| --- | ---: |
| WGS 84 Earth equatorial radius | 6,378,137 m |
| WGS 84 Earth polar radius | 6,356,752.314245 m |
| Mean Earth radius | 6,371,008.8 m |
| Renderer atmosphere cutoff | 100,000 m above the ellipsoid |
| Artistic Earth base-height range | 0–10,000 m above the ellipsoid |
| Local procedural relief / collision reserve | 0–200 m |
| Earth surface inspection clearance | 300 m above conservative terrain |
| Earth sidereal rotation period | 86,164.0905 s |
| Mean Earth–Moon distance | 384,400,000 m |
| Moon mean radius | 1,737,400 m |
| Nominal solar radius | 695,700,000 m |
| Astronomical unit | exactly 149,597,870,700 m |
| Speed of light | exactly 299,792,458 m/s |

The 100 km atmosphere top is a finite rendering boundary, not a claim that the real atmosphere has
a hard edge. The solar/lunar equations and constants are placed close to the pure code that consumes
them so tests can enforce scale and angular-size invariants.

## Visitor flight and collision model

The visitor is a virtual exhibition camera, not a spacecraft:

- movement follows camera forward/right/up axes and normalizes diagonal input;
- acceleration and braking use exponential damping, with frame delta bounded to 0.1 s;
- base speed is proportional to non-negative clearance from the nearest terrain-aware Earth, Moon,
  or Sun surface and then adjusted in logarithmic user-controlled notches;
- the normal minimum is 0.25 m/s, fast-travel boost is 12×, and final speed is capped at 0.1 AU/s
  (about 50 times light speed);
- Earth collision searches a swept segment inside a WGS 84 shell, samples the same 2048×1024
  bilinear base-height field as WebGL, and reserves the full 200 m procedural-relief envelope.
  Contact is projected to that conservative terrain radius with a nominal 2 m clearance. Moon and
  Sun retain swept segment-versus-sphere collision with the same clearance;
- the surface-inspection preset chooses the most sun-facing of six fixed, moderately cloudy land
  sites, keeps the physical Earth and current lighting unchanged, and places the visitor 300 m
  above conservative terrain with an 18-degree north-facing view below the terrain-relative 1.5 km
  cloud base;
- contact removes inward velocity, preventing a high-speed step from tunnelling through a body;
  and
- non-finite state or travel beyond 100 AU from the origin resets to the known initial view.

The superluminal cap is intentional and must not be interpreted as physical motion. There is no
gravity, inertia from a spacecraft mass model, orbital insertion, fuel, relativistic time dilation,
or atmospheric drag. Earth collision and speed adaptation include the shared artistic terrain
field and conservative local-relief reserve; oceans remain at the ellipsoid, while clouds and
atmosphere are non-solid. Moon and Sun collision use their reference spheres.

## Relationship to the renderer

The multipass WebGL renderer consumes this frame to preserve finite solar/lunar angular size,
WGS-84 Earth shape, displaced artistic terrain, phase, and mutual shadows. Rendering itself is
bounded and approximate: atmosphere and clouds use small fixed sample counts, eclipse visibility is
a smooth finite-disc calculation, and Earth height is artistic rather than measured local
topography. See
[`earth-rendering.md`](earth-rendering.md) for exact pass order, measured asset provenance, sample
bounds, performance policy, and visual limitations.

Measured ground-scale fidelity would require a licensed high-resolution elevation source, runtime
tiled LOD, and validation against ground/aircraft reference views. The current base field is shared
between CPU and GPU, while collision deliberately reserves the maximum procedural relief instead of
matching each local hill exactly. Navigation-grade astronomy would separately require a maintained
time standard and a high-accuracy ephemeris such as JPL data. Neither capability is currently
shipped.

## Research basis

Primary and foundational references used to choose constants and approximations:

- [NGA, *Department of Defense World Geodetic System 1984*](https://earth-info.nga.mil/php/download.php?file=coord-wgs84)
- [IERS Conventions (2010), Chapter 5: transformation between celestial and terrestrial systems](https://iers-conventions.obspm.fr/content/chapter5/icc5.pdf)
- [NOAA Global Monitoring Laboratory, solar-position equations](https://gml.noaa.gov/grad/solcalc/solareqns.PDF)
- [IAU 2012 Resolution B2, definition of the astronomical unit](https://www.iau.org/static/resolutions/IAU2012_English.pdf)
- [NASA, *Facts About Earth*](https://science.nasa.gov/earth/facts/) and [*Moon Facts*](https://science.nasa.gov/moon/facts/)
- [NASA/JPL Solar System Dynamics, planetary physical parameters](https://ssd.jpl.nasa.gov/planets/phys_par.html)
- [WMO International Cloud Atlas, cloud-level definitions](https://cloudatlas.wmo.int/en/clouds-definitions.html)
- [NASA, *What Are Clouds?*](https://www.nasa.gov/earth/what-are-clouds-grades-5-8/)
- [NASA Earth Observatory, night-side airglow observations](https://earthobservatory.nasa.gov/images/92912/earth-awash-in-lights-of-the-ni)
- Eric Bruneton and Fabrice Neyret, [*Precomputed Atmospheric Scattering* (2008)](https://inria.hal.science/inria-00288758)
- Eric Bruneton, [*Precomputed Atmospheric Scattering: a New Implementation* (2017)](https://inria.hal.science/hal-01520758)

The Bruneton work informs the physical vocabulary and future multiple-scattering path; the shipped
shader uses the cheaper bounded single-scattering model documented in `earth-rendering.md` rather
than that precomputed algorithm.
