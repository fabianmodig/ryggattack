#!/usr/bin/env bash
# Print the wasm-bindgen version that Cargo.lock pins.
#
# The generated JavaScript glue only works with the CLI of the same version as
# the crate the binary was compiled against, so the build and CI both read the
# version from here instead of hardcoding it.

set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

version="$(sed -n '/^name = "wasm-bindgen"$/{n;s/^version = "\(.*\)"$/\1/p;q;}' Cargo.lock)"

if [ -z "$version" ]; then
  echo "error: no wasm-bindgen entry in Cargo.lock" >&2
  exit 1
fi

echo "$version"
