# Black Cube Gallery

A Rust-to-WebAssembly, static-site personal digital art gallery. Visitors choose an
installation, then enter a WebGL-rendered white room containing a solid black cube.

## Current artworks

**A World in Light** is a room-scale, Earth-like procedural planet. Its shader generates
terrain, a Pacific-scale ocean with reflective water, lakes, restrained snow-covered mountain
relief, distinct north and south polar glaciers, displaced cloud bodies, atmospheric
scattering, and a day/night terminator at render time. The planet's surface is stable while a
stationary virtual sun supplies the light and a moon orbits the planet. No texture, model, or
network asset is required. The atmosphere is a compact real-time approximation of molecular
(Rayleigh-like) blue scattering and aerosol/cloud (Mie-like) forward scattering, not a climate
simulation.
The Moon and planet also cast geometric directional eclipses on each other; a small amount
of Earthshine remains on the Moon during a lunar eclipse.

## First-person controls

Click the room or choose **Explore room** to capture the mouse. Press Escape at any time
to release it. Use **Full screen** to expand the room; the same button changes to **Exit
full screen** when active.

- `W`, `A`, `S`, `D`: move
- Mouse: look
- `Space`: jump
- `Ctrl`: smoothly crouch

Use **Controls** in the installation to adjust mouse sensitivity, invert vertical mouse
look, and rebind every action. Settings are validated and stored only in this browser’s
local storage; no visitor data is sent anywhere. The physical room has walls, floor,
ceiling, gravity, a solid central cube, and collision-safe movement.

## Commands

The production browser code is compiled from Rust to WebAssembly with `wasm-pack 0.15.0`.
The repository also needs Cargo with the `wasm32-unknown-unknown` target, Node.js for the
local static server, and GNU Make. No Node package install is required.

```sh
make help   # list commands
make build  # compile the static site to dist/
make serve  # build and serve at http://127.0.0.1:8080
make dev    # watch, rebuild, and refresh local browser tabs
make test   # run focused domain tests
make check  # run the final local quality gate
make clean  # remove generated dist/ and target/ directories
```

Open the local URL in a modern desktop browser. A static-hosting provider can publish
the generated `dist/` directory directly; no server process is required in production.

`make dev` is the normal development loop: edit Rust, HTML, CSS, or the Wasm bootstrap;
the server coalesces rapid changes, rebuilds `dist/`, and reloads open local tabs only
after a successful build. If a build fails, the last successful site remains available.

Use `make test` during development and `make check` before handoff.

## Add an artwork

1. Add stable metadata and a destination in a new `src/artworks/<artwork>.rs` module.
2. Register that module in `src/artworks/mod.rs`, including its explicit `ArtworkKind`.
3. Add its self-contained WebGL implementation in `src/browser/<artwork>_renderer.rs` and
   register its `ArtworkKind` in `src/browser/rendering.rs`; visitor controls and persisted
   preferences must remain untouched.
4. Add focused catalog/destination tests in `src/lib.rs`; pure rendering math belongs in
   `src/math.rs` with deterministic tests.

The catalog, destination resolver, artwork kinds, FPS rules, and rendering math are
renderer-independent. Browser DOM construction, local preference storage, and renderer
selection are adapter modules. This remains one Cargo crate intentionally: the gallery is one
static Wasm application with no reusable deployment boundary yet. Internal modules keep the
dependency direction clear without introducing workspace/public-API overhead.

## Security and hosting

The gallery has no network calls, credentials, user-provided HTML, or third-party assets.
The only stored value is validated local control preference data; it contains no identity
or secret and can be cleared in browser site settings. A restrictive meta Content Security
Policy permits only same-origin assets, same-origin WebAssembly loading, and WebAssembly
compilation. Artwork IDs are validated before resolving a destination; assets added later
should be validated for size, format, licensing, and safe paths before use.
