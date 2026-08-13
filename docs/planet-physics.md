# Planet simulation: numerical and physical foundation

This document describes the physical-coordinate and visitor-navigation foundation for **A World in Light**. It is deliberately renderer-independent so the browser renderer can be replaced without rewriting celestial motion or camera logic.

## Scope of this milestone

The feature branch now establishes:

- SI-unit, double-precision positions for Earth, Moon, Sun, and the visitor;
- WGS 84 Earth dimensions and an oblate Earth ellipsoid;
- real relative radii and mean orbital-distance scale;
- deterministic low-cost apparent solar and lunar positions from seconds since J2000;
- camera-relative conversion before values are narrowed to WebGL-friendly `f32` values;
- logarithmic, unconstrained six-direction flight from surface-detail speeds to interplanetary speeds;
- deterministic tests for scale ratios, apparent angular sizes, ellipsoid intersections, precision, flight direction, and speed limits.

This is not yet the final renderer. The current WebGL artwork remains room-scale until the browser adapter is switched to the new camera and physical frame.

## Coordinate convention

- Unit: metre.
- Precision: `f64` in domain and navigation code.
- Origin: Earth's centre.
- Positive `Y`: north celestial direction.
- GPU conversion: subtract the visitor position in `f64`, then divide the relative vector by Earth's equatorial radius and cast to `f32`.

Subtracting the camera first is non-negotiable. At lunar and solar distances an absolute `f32` position cannot retain the metre- or kilometre-scale precision needed near a body. Camera-relative rendering keeps nearby geometry numerically stable while allowing distant bodies to retain the correct apparent angle.

## Physical constants

The implementation records constants close to the code that consumes them:

- WGS 84 Earth equatorial radius: 6,378,137 m;
- WGS 84 Earth polar radius: 6,356,752.314245 m;
- mean Earth–Moon distance: 384,400 km;
- Moon mean radius: 1,737.4 km;
- nominal solar radius: 695,700 km;
- astronomical unit: exactly 149,597,870,700 m;
- speed of light: exactly 299,792,458 m/s.

The Moon and Sun equations are deliberately low-cost analytical approximations. They include dominant anomaly, eccentricity, inclination, and obliquity terms and are suitable for visual simulation. They are not a substitute for JPL ephemerides, navigation, occultation prediction, or scientific measurement.

## Visitor flight

The flight controller is exhibition-oriented rather than a spacecraft dynamics simulator:

- no collision or gravity is imposed;
- movement follows camera forward/right/up axes;
- diagonal input is normalized;
- acceleration and braking are damped to reduce motion discomfort;
- cruise speed changes in logarithmic notches;
- speed is capped at 0.25 c;
- invalid or runaway state resets to a known orbital view;
- focusing Earth computes an exact camera orientation rather than applying an approximate turn.

Relativistic time dilation and orbital mechanics are intentionally excluded from visitor movement. At the exhibition's highest speeds, movement is a navigation affordance that lets a visitor cross the actual scale, not a claim that the observer is a physically realizable vehicle.

## Rendering roadmap

The next browser-facing stages are:

1. replace the room player with `SpaceflightState` for the planet artwork;
2. render Earth, Moon, and the finite solar disc analytically from camera-relative physical positions;
3. add measured Earth surface datasets and a land/water/material hierarchy;
4. implement atmosphere transmittance, multi-scattering, sky-view, and aerial-perspective lookup tables;
5. add cloud volumes, cloud shadows, and temporal reprojection;
6. add terrain and ocean level-of-detail suitable for descent below orbital altitude;
7. validate reference views from ground level, aircraft altitude, low orbit, lunar distance, and interplanetary space.

## Research basis

Primary references used for this foundation and the following renderer stages:

- NASA Science, *Moon Facts* and *Facts About Earth*;
- NASA/JPL Solar System Dynamics, *Planetary Physical Parameters*;
- National Geospatial-Intelligence Agency, *Department of Defense World Geodetic System 1984*;
- IAU 2012 Resolution B2, definition of the astronomical unit;
- Eric Bruneton and Fabrice Neyret, *Precomputed Atmospheric Scattering*, 2008;
- Eric Bruneton, *Precomputed Atmospheric Scattering: a New Implementation*, 2017.
