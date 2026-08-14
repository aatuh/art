#!/bin/sh
set -eu

if ! command -v glslangValidator >/dev/null 2>&1; then
    echo "glslangValidator is required to validate the planetary shaders." >&2
    exit 127
fi

startup_fragment="${TMPDIR:-/tmp}/black-cube-gallery-planet-startup-$$.frag"
detailed_fragment="${TMPDIR:-/tmp}/black-cube-gallery-planet-detailed-$$.frag"
trap 'rm -f "$startup_fragment" "$detailed_fragment"' EXIT HUP INT TERM

cargo run --quiet --bin emit_planet_shader > "$startup_fragment"
cargo run --quiet --bin emit_detailed_planet_shader > "$detailed_fragment"
glslangValidator -S vert src/browser/shaders/planet.vert.glsl
glslangValidator -S frag "$startup_fragment"
glslangValidator -S frag "$detailed_fragment"
