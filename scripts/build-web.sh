#!/usr/bin/env bash
# Build the WebAssembly version of Ryggattack into dist/web/.
#
# Usage: scripts/build-web.sh [--debug]
#
# Requires the wasm32-unknown-unknown target and a wasm-bindgen CLI whose
# version matches the wasm-bindgen crate in Cargo.lock.

set -euo pipefail

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_root"

profile="release"
for arg in "$@"; do
  case "$arg" in
    --debug) profile="debug" ;;
    *)
      echo "unknown argument: $arg" >&2
      exit 2
      ;;
  esac
done

out_dir="dist/web"
target_dir="target/wasm32-unknown-unknown/$profile"

# Toolchains installed some other way (Nix, a distribution package) ship their
# own target list, so only rustup-managed ones are checked here.
if command -v rustup >/dev/null 2>&1 &&
  ! rustup target list --installed | grep -qx wasm32-unknown-unknown; then
  echo "error: the wasm32-unknown-unknown target is missing." >&2
  echo "       run: rustup target add wasm32-unknown-unknown" >&2
  exit 1
fi

# The generated JavaScript glue only works with the exact wasm-bindgen crate
# version the binary was compiled against, so read it out of Cargo.lock.
expected="$(sed -n '/^name = "wasm-bindgen"$/{n;s/^version = "\(.*\)"$/\1/p;q;}' Cargo.lock)"

if ! command -v wasm-bindgen >/dev/null 2>&1; then
  echo "error: wasm-bindgen was not found on PATH." >&2
  echo "       run: cargo install wasm-bindgen-cli --version $expected" >&2
  exit 1
fi

actual="$(wasm-bindgen --version | awk '{print $2}')"
if [ -n "$expected" ] && [ "$expected" != "$actual" ]; then
  echo "error: wasm-bindgen CLI $actual does not match the wasm-bindgen crate $expected." >&2
  echo "       run: cargo install wasm-bindgen-cli --version $expected --force" >&2
  exit 1
fi

if [ "$profile" = "release" ]; then
  cargo build --release --target wasm32-unknown-unknown
else
  cargo build --target wasm32-unknown-unknown
fi

rm -rf "$out_dir"
mkdir -p "$out_dir"

bindgen_args=(--target web --no-typescript --out-dir "$out_dir")
if [ "$profile" = "release" ]; then
  # The symbol name section is roughly a third of the bundle and only feeds
  # readable JavaScript stack traces, which a release build does not need.
  bindgen_args+=(--remove-name-section --remove-producers-section)
fi

wasm-bindgen "${bindgen_args[@]}" "$target_dir/ryggattack.wasm"

cp web/index.html "$out_dir/index.html"
if [ -d assets ]; then
  cp -R assets "$out_dir/assets"
fi

echo
echo "Built $out_dir. Serve it with:"
echo "  python3 -m http.server 8080 --directory $out_dir"
