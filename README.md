# Black Cube Gallery

A Rust-to-WebAssembly, static-site personal digital art gallery. Visitors choose an installation
and enter an immersive WebGL artwork in the browser.

## Current artworks

### A World in Light

**A World in Light** is an astronomical-scale Earth–Moon–Sun installation. Celestial dimensions
and distances are expressed in SI units on the CPU, with camera-relative conversion before values
are sent to the GPU. The Earth body uses WGS 84 dimensions; Moon and Sun sizes/distances preserve
real-world scale, while the lightweight solar/lunar ephemerides are intended for the artwork rather
than navigation-grade astronomy.

The current multipass renderer draws a cheap diffuse-space and scale-correct solar-disc/corona
background, a deterministic 1,800-point star catalogue, then only the conservative screen regions
occupied by Earth, Moon, Sun, or atmosphere. NASA Blue Marble surface colour, Black Marble night
lights, and an SVS/LRO lunar map are committed as bounded same-origin assets. Generated material
and weather maps drive stable, distance-filtered terrain detail, physically scaled deep-water wave
bands, terrain-relative cloud volumes, and their shadows. One 2048×1024 height field is sampled by
both WebGL and navigation, so artistic elevation changes the rendered surface and collision
clearance without distance-dependent geometry swaps. Explicit LOD filtering prevents the Pacific
wrap and close procedural detail from selecting unstable or aliased samples. A bounded
Rayleigh/Mie/ozone integration supplies atmosphere and eclipse-aware lighting. The displaced
terrain and procedural detail make close flyovers legible, but they are art-directed rather than
measured ground-level geography. See
[`docs/earth-rendering.md`](docs/earth-rendering.md) and
[`docs/planet-physics.md`](docs/planet-physics.md) for the rendering and physics boundaries.

The installation remains fully static-hostable: it does not require a server-side runtime or fetch
surface/weather data while running.

### Black Cube / White Room

The original gallery room remains a conventional first-person installation with walls, floor,
ceiling, gravity, a solid central cube, and collision-safe movement.

## Controls

Click the installation or choose **Explore room** / **Explore space** to capture the mouse. Press
Escape at any time to release it. **Full screen** expands the installation; the same button exits
full screen when active.

Common controls:

- `W`, `A`, `S`, `D`: move
- Mouse: look
- `Space`: jump in a room / ascend in astronomical flight
- `Ctrl`: crouch in a room / descend in astronomical flight

A World in Light additionally provides:

- **View Earth surface**: move 300 m above the conservative terrain collision surface at a sunlit,
  moderately cloudy land site and look beneath the terrain-relative 1.5 km cloud base; subsequent
  clicks cycle Moon, Sun, and
  whole-Earth views
- **View Moon / Sun / Earth**: move directly to a four-radius inspection view while preserving the
  bodies' real sizes, separations, lighting, and lunar phase
- `Shift`: fast-travel boost while moving
- `Z` / `C`: decrease / increase the camera speed tier
- `X`: brake toward a stop
- `F`: point the camera at Earth
- `R`: reset the astronomical viewpoint
- `T`: cycle simulation time through `1×`, `60×`, `3600×`, and paused

The astronomical navigation controller is a virtual exhibition camera, not a spacecraft or orbital
flight model. Its speed adapts to the nearest Earth, Moon, or Sun surface so interplanetary travel
remains responsive while close inspection of every body stays controllable.

Use **Controls** in an installation to adjust mouse sensitivity, invert vertical mouse look, and
rebind the shared movement actions. Settings are validated and stored only in this browser's local
storage; no visitor data is sent anywhere.

## Commands

The production browser code is compiled from Rust to WebAssembly with `wasm-pack 0.15.0`. The
repository also needs Cargo/Rust 1.85 with the `wasm32-unknown-unknown` target, Node.js for the local
static server, GNU Make, and `glslangValidator` for shader validation. No Node package install is
required.

```sh
make help         # list commands
make build        # compile the static site to dist/
make serve        # build and serve at http://127.0.0.1:8080
make dev          # watch, rebuild, and refresh local browser tabs
make test         # run every workspace crate's focused domain tests
make wasm-check   # check and lint the browser/Wasm adapter
make shader-check # validate the active multipass and generated detailed shaders
make check        # run the final local quality gate
make clean        # remove generated dist/ and target/ directories
```

Open the local URL in a modern desktop browser. A static-hosting provider can publish the generated
`dist/` directory directly; no server process is required in production.

`make dev` is the normal development loop: edit Rust, HTML, CSS, GLSL, static artwork assets, or the
Wasm bootstrap; the server coalesces rapid changes, rebuilds `dist/`, and reloads open local tabs
only after a successful build. If a build fails, the last successful site remains available.

Use `make test` during development and `make check` before handoff.

### Reproduce the planet assets

The checked-in textures are runtime assets, so normal development and deployment do not require
Python or network access. To reproduce them from their pinned NASA authoring sources, install
Pillow with WebP support, NumPy, and SciPy in an asset-authoring environment, then run:

```sh
python3 scripts/fetch-blue-marble.py
python3 scripts/generate-procedural-earth.py
python3 scripts/fetch-black-marble.py
python3 scripts/fetch-moon-albedo.py
cargo test --locked --test earth_assets
```

The generator writes the measured Earth surface plus material, weather, diffuse-starfield, and
shared terrain-height assets; the two remaining fetchers create the night-light and lunar-albedo
WebPs. These authoring commands replace seven runtime files in place, so review the binary diff
before committing it. Source URLs,
input/output SHA-256 values, credits, NASA media-use terms, encoder caveats, and current rendering
limits are recorded in [`docs/earth-rendering.md`](docs/earth-rendering.md).

## Architecture

The Cargo workspace makes artwork ownership a compile-time boundary:

- `crates/gallery-core`: typed artwork, world, installation, presentation, and catalogue contracts;
  it contains no renderer or browser APIs.
- `crates/artwork-black-cube`: the Black Cube descriptor, authored room geometry, spawn and
  colliders, and deterministic first-person physics.
- `crates/artwork-world-in-light`: the planetary descriptor, camera and celestial simulation,
  terrain, LOD and quality rules, shader composition, and artwork-owned GLSL.
- `src/browser/artworks/<artwork>/`: the thin WebGL renderer and browser controller for one
  artwork. A controller owns exactly one visitor state and its optional toolbar controls.
- `src/browser/runtime.rs`: the common installation-runtime port. `controls.rs` handles only raw
  browser events and persisted bindings; it does not switch on concrete artworks.
- `catalogue.rs`, `installation.rs`, `lifecycle.rs`, `dom.rs`, and `storage.rs`: separate gallery
  shell adapters. `browser.rs` is only the Wasm composition root.

`src/browser/artworks/mod.rs` is the explicit composition registry pairing each renderer-free
descriptor with its browser runtime factory. Typed world and installation IDs replace renderer
enums and free-form routing checks.

## Add an artwork

1. Add `crates/artwork-<name>/` with its descriptor, deterministic generation/simulation, and
   focused tests. Depend only on `gallery-core` unless another dependency is genuinely shared.
2. Add `src/browser/artworks/<name>/` with its controller and renderer. Implement the common
   `InstallationRuntime`; keep DOM/WebGL code out of the pure artwork crate.
3. Add the module and one descriptor/factory entry to `src/browser/artworks/mod.rs`.
4. Put bounded, licensed runtime assets under an artwork-specific asset directory, add validation,
   and document any reproducible authoring commands.
5. Run `make check`. Confirm catalogue selection, installation entry, controls, teardown/re-entry,
   renderer failure recovery, and static `dist/` output.

## Security and hosting

The gallery has no credentials, user-provided HTML, or required third-party runtime services. The
only stored value is validated local control preference data; it contains no identity or secret and
can be cleared in browser site settings. A restrictive Content Security Policy permits same-origin
application and artwork assets, including the Earth textures loaded into WebGL. Artwork IDs are
validated before resolving a destination; future assets should continue to be validated for size,
format, provenance, licensing, and safe paths before use.
