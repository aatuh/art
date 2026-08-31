import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const root = dirname(fileURLToPath(new URL("../Cargo.toml", import.meta.url)));
const source = (path) => readFile(join(root, path), "utf8");

test("workspace enforces one renderer-free crate per artwork", async () => {
  const [manifest, coreManifest, cubeManifest, worldManifest] = await Promise.all([
    source("Cargo.toml"),
    source("crates/gallery-core/Cargo.toml"),
    source("crates/artwork-black-cube/Cargo.toml"),
    source("crates/artwork-world-in-light/Cargo.toml")
  ]);

  for (const member of ["gallery-core", "artwork-black-cube", "artwork-world-in-light"]) {
    assert.match(manifest, new RegExp(`crates/${member}`));
  }
  for (const artworkManifest of [cubeManifest, worldManifest]) {
    assert.match(artworkManifest, /gallery-core/);
    assert.doesNotMatch(artworkManifest, /web-sys|wasm-bindgen/);
  }
  assert.doesNotMatch(coreManifest, /web-sys|wasm-bindgen/);
});

test("browser shell depends on the runtime port rather than concrete artwork state", async () => {
  const [controls, runtime, registry, bootstrap] = await Promise.all([
    source("src/browser/controls.rs"),
    source("src/browser/runtime.rs"),
    source("src/browser/artworks/mod.rs"),
    source("bootstrap.js")
  ]);

  assert.match(controls, /InstallationRuntime/);
  assert.doesNotMatch(
    controls,
    /PlayerState|SpaceflightState|CelestialTarget|SceneRenderer|NavigationMode/
  );
  assert.match(runtime, /trait InstallationRuntime/);
  assert.match(registry, /artwork_black_cube::BLACK_CUBE_ROOM/);
  assert.match(registry, /artwork_world_in_light::WORLD_IN_LIGHT/);
  assert.doesNotMatch(registry, /ArtworkKind/);
  assert.doesNotMatch(bootstrap, /orbiting-earth|MutationObserver/);
});

test("planet shader source is owned by its artwork crate", async () => {
  const shaderComposer = await source(
    "crates/artwork-world-in-light/src/planet_shader.rs"
  );
  assert.match(shaderComposer, /include_str!\("\.\.\/shaders\//);
  assert.doesNotMatch(shaderComposer, /src\/browser\/shaders/);
});
