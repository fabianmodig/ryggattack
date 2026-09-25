#!/usr/bin/env bash
# Build the WebAssembly version of Ryggattack into dist/web/.
#
# Usage: scripts/build-web.sh [--debug | --profile <name>]
#
# `--profile web-dev` builds without LTO, for hosts short on memory; the
# default release profile is what should be shipped.
#
# Requires the wasm32-unknown-unknown target and a wasm-bindgen CLI whose
# version matches the wasm-bindgen crate in Cargo.lock.

set -euo pipefail

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_root"

profile="release"
while [ $# -gt 0 ]; do
  case "$1" in
    --debug) profile="debug" ;;
    --profile)
      [ $# -ge 2 ] || { echo "--profile needs a name" >&2; exit 2; }
      profile="$2"
      shift
      ;;
    *)
      echo "unknown argument: $1" >&2
      exit 2
      ;;
  esac
  shift
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

expected="$(scripts/wasm-bindgen-version.sh)"

if ! command -v wasm-bindgen >/dev/null 2>&1; then
  echo "error: wasm-bindgen was not found on PATH." >&2
  echo "       run: cargo install wasm-bindgen-cli --version $expected" >&2
  exit 1
fi

actual="$(wasm-bindgen --version | awk '{print $2}')"
if [ "$expected" != "$actual" ]; then
  echo "error: wasm-bindgen CLI $actual does not match the wasm-bindgen crate $expected." >&2
  echo "       run: cargo install wasm-bindgen-cli --version $expected --force" >&2
  exit 1
fi

if [ "$profile" = "release" ]; then
  cargo build --release --target wasm32-unknown-unknown
elif [ "$profile" = "debug" ]; then
  cargo build --target wasm32-unknown-unknown
else
  cargo build --profile "$profile" --target wasm32-unknown-unknown
fi

rm -rf "$out_dir"
mkdir -p "$out_dir"

bindgen_args=(--target web --no-typescript --out-dir "$out_dir")
if [ "$profile" != "debug" ]; then
  # The symbol name section is roughly a third of the bundle and only feeds
  # readable JavaScript stack traces, which an optimised build does not need.
  bindgen_args+=(--remove-name-section --remove-producers-section)
fi

wasm-bindgen "${bindgen_args[@]}" "$target_dir/ryggattack.wasm"

# Optional, because it needs Binaryen rather than anything cargo installs.
# A host that cannot compress the response serves the module at its full size,
# so releases are built where this is available and take the smaller one.
module="$out_dir/ryggattack_bg.wasm"
if [ "$profile" = "release" ] && command -v wasm-opt >/dev/null 2>&1; then
  before="$(wc -c <"$module")"
  # The feature flags have to cover whatever the current rustc emits. They are
  # not a wish list: wasm-opt rejects a module using anything it was not told
  # about, which fails this build rather than shipping a broken one.
  wasm-opt -Oz \
    --enable-bulk-memory \
    --enable-nontrapping-float-to-int \
    -o "$module.opt" "$module"
  mv "$module.opt" "$module"
  after="$(wc -c <"$module")"
  echo "wasm-opt: $((before / 1000000)) MB -> $((after / 1000000)) MB"
elif [ "$profile" = "release" ]; then
  echo "note: wasm-opt was not found, so the module keeps its full size."
fi

cp web/index.html "$out_dir/index.html"
if [ -d assets ]; then
  cp -R assets "$out_dir/assets"
fi

echo
echo "Built $out_dir. Serve it with:"
echo "  python3 -m http.server 8080 --directory $out_dir"
