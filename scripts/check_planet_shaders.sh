#!/bin/sh
set -eu

if ! command -v glslangValidator >/dev/null 2>&1; then
    echo "glslangValidator is required to validate the planetary shaders." >&2
    exit 127
fi

fragment_path="${TMPDIR:-/tmp}/black-cube-gallery-planet-$$.frag"
trap 'rm -f "$fragment_path"' EXIT HUP INT TERM

cargo run --quiet --bin emit_planet_shader > "$fragment_path"
glslangValidator -S vert src/browser/shaders/planet.vert.glsl
glslangValidator -S frag "$fragment_path"
