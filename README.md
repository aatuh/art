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

The current Earth renderer combines a bundled global surface image with distance-dependent surface
detail, animated ocean reflection, cloud shadows, a ray-marched cloud shell with deterministic
formation/dissipation, atmospheric scattering, day/night illumination, eclipses, city-light-like
night detail, stars, and tone mapping. The global imagery is appropriate for orbital/approach views;
measured tiled elevation and ground-scale terrain LOD remain a later milestone. See
[`docs/earth-rendering.md`](docs/earth-rendering.md) and
[`docs/planet-physics.md`](docs/planet-physics.md) for the rendering and physics boundaries.

The installation remains fully static-hostable: it does not require a server-side runtime or fetch
surface/weather data while running.

### Black Cube / White Room

The original gallery room remains a conventional first-person installation with walls, floor,
ceiling, gravity, a solid central cube, and collision-safe movement.

## Controls

Click the installation or choose **Explore room** to capture the mouse. Press Escape at any time to
release it. **Full screen** expands the installation; the same button exits full screen when active.

Common controls:

- `W`, `A`, `S`, `D`: move
- Mouse: look
- `Space`: jump in a room / ascend in astronomical flight
- `Ctrl`: crouch in a room / descend in astronomical flight

A World in Light additionally provides:

- `Shift`: fast-travel boost while moving
- `Z` / `C`: decrease / increase the camera speed tier
- `X`: brake toward a stop
- `F`: point the camera at Earth
- `R`: reset the astronomical viewpoint
- `T`: cycle simulation time through `1×`, `60×`, `3600×`, and paused

The astronomical navigation controller is a virtual exhibition camera, not a spacecraft or orbital
flight model. Its speed adapts to distance from the Earth surface so orbital motion is visibly
responsive while close inspection remains controllable.

Use **Controls** in an installation to adjust mouse sensitivity, invert vertical mouse look, and
rebind the shared movement actions. Settings are validated and stored only in this browser's local
storage; no visitor data is sent anywhere.

## Commands

The production browser code is compiled from Rust to WebAssembly with `wasm-pack 0.15.0`. The
repository also needs Cargo/Rust 1.85 with the `wasm32-unknown-unknown` target, Node.js for the local
static server, and GNU Make. No Node package install is required.

```sh
make help   # list commands
make build  # compile the static site to dist/
make serve  # build and serve at http://127.0.0.1:8080
make dev    # watch, rebuild, and refresh local browser tabs
make test   # run focused domain tests
make check  # run the final local quality gate
make clean  # remove generated dist/ and target/ directories
```

Open the local URL in a modern desktop browser. A static-hosting provider can publish the generated
`dist/` directory directly; no server process is required in production.

`make dev` is the normal development loop: edit Rust, HTML, CSS, GLSL, or the Wasm bootstrap; the
server coalesces rapid changes, rebuilds `dist/`, and reloads open local tabs only after a successful
build. If a build fails, the last successful site remains available.

Use `make test` during development and `make check` before handoff.

## Add an artwork

1. Add stable metadata and a destination in a new `src/artworks/<artwork>.rs` module.
2. Register that module in `src/artworks/mod.rs`, including its explicit `ArtworkKind`.
3. Add its self-contained WebGL implementation in `src/browser/<artwork>_renderer.rs` and register
   its `ArtworkKind` in `src/browser/rendering.rs`; visitor controls and persisted preferences must
   remain independent from artwork rendering.
4. Add focused catalog/destination tests in `src/lib.rs`; pure rendering/physics math belongs in
   renderer-independent modules with deterministic tests.

The catalog, destination resolver, artwork kinds, room rules, astronomical camera rules, and
rendering math are renderer-independent. Browser DOM construction, local preference storage, and
renderer selection are adapter modules. This remains one Cargo crate intentionally: the gallery is
one static Wasm application with no reusable deployment boundary yet.

## Security and hosting

The gallery has no credentials, user-provided HTML, or required third-party runtime services. The
only stored value is validated local control preference data; it contains no identity or secret and
can be cleared in browser site settings. A restrictive Content Security Policy permits same-origin
application assets and the locally embedded Earth image data used to initialize the WebGL texture.
Artwork IDs are validated before resolving a destination; future assets should continue to be
validated for size, format, provenance, licensing, and safe paths before use.
