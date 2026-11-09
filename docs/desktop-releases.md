# Desktop downloads and tag releases

The independent `.forgejo/workflows/desktop-release.yml` builds **Windows x64**
and **Linux x64** graphical clients on the existing Linux `ubuntu-latest` runner
(stock `node:22-bookworm`, no runner infrastructure changes). macOS is deferred.
It does not change the web/main/beta publication workflows.

## Download and run

Every pushed Git tag is eligible, not just `v*` tags. After **both** builds
succeed, its Forgejo release contains:

- `ryggattack-windows-x86_64.zip`
- `ryggattack-linux-x86_64.tar.gz`
- `SHA256SUMS` (both archive SHA-256 digests)

Names intentionally omit the tag: slashes or shell metacharacters in valid Git
tags cannot change output paths. Source SHA, original tag, target and platform
are inside each package's `metadata.json`; `source-sha.txt` holds the full commit.
These are integrity/source markers, **not signed attestations**.

Download the correct archive and `SHA256SUMS`. On Linux, run
`sha256sum --ignore-missing --check SHA256SUMS` in the download directory.
On Windows, compare `Get-FileHash .\ryggattack-windows-x86_64.zip -Algorithm SHA256`
with its line in `SHA256SUMS`. Extract the **entire** archive, including `assets/`.
Do not move only the executable out of that directory.

### Linux

Run `./launch.sh` in the extracted directory. It changes to its own package
directory before executing the game and forwards CLI arguments; invocation from
another working directory and paths containing spaces are supported. Archives
preserve executable permissions (unlike loose CI artifact files).

The current Debian 12 builder sets a **glibc 2.36 baseline**: use Debian 12+ or
an equivalent/newer x86_64 glibc distribution, not Alpine/musl or an older glibc.
This is not a fully static/AppImage package. A graphical X11/Wayland session,
working GPU drivers and Vulkan/OpenGL support are required. On Debian, expected
runtime packages include `libudev1 libwayland-client0 libxkbcommon0 libx11-6
libxcursor1 libxi6 libxrandr2 libvulkan1 libgl1`; install appropriate Mesa/vendor
GPU drivers as well. Actual linked-library and symbol-version inspection of the
real CI executable remains a separate portability gate. NixOS users need an
appropriate runtime environment rather than assuming this glibc archive is a
native Nix package.

### Windows

Extract the ZIP, then open `ryggattack.exe` inside its extracted directory.
The build targets `x86_64-pc-windows-msvc` using pinned `cargo-xwin==0.23.1`
installed from PyPI wheels in a Python virtual environment. LLVM/Clang/lld,
CMake and Ninja are supplied by the stock Debian builder. cargo-xwin downloads
the Microsoft SDK/CRT and uses its upstream SDK license-acceptance behavior;
build operators must accept those upstream SDK licensing terms.

`RUSTFLAGS=-C target-feature=+crt-static` is scoped to the Windows build to prefer
the static MSVC CRT; Linux/host flags are not inherited into that lane. CI logs
`llvm-readobj --coff-imports` for the actual executable. Static CRT intent is not
proof of the complete DLL closure: review those imports before claiming no
Visual C++ Redistributable is required. No third-party DLLs or runtime installer
are bundled. If the executable imports the dynamic MSVC runtime, install the
Microsoft Visual C++ 2015–2022 x64 Redistributable. Windows system DLLs and current
GPU drivers remain required regardless.

Both downloads are **unsigned**; SmartScreen/antivirus may warn. A native Windows
graphical play-test has **not** been performed by these packaging tests.
Cross-compilation, binary header validation and fixture tests do not establish
that a native window, graphics, audio or controllers work. Linux graphical
runtime acceptance is likewise separate from compilation/package verification.

## Manual pre-merge exercise

Use Forgejo Actions **Desktop release → Run workflow**, selecting the feature
branch containing this workflow. `workflow_dispatch` builds both targets and
uploads `desktop-linux` / `desktop-windows` CI artifacts for seven days. Each
contains the archive plus its single-archive `SHA256SUMS`. These artifacts are
available even if publishing is deliberately skipped. Manual runs on branches
**or tags never publish a release**: publishing requires a push tag event, both
matrix lanes succeeding, matching checkout/event/remote-tag commit and matching
artifact metadata/checksums. Tag strings enter shell commands only via quoted
environment variables, never expression interpolation into shell code.

Each new lane sets `CARGO_BUILD_JOBS=1` to limit concurrent Cargo compiler jobs
on the existing capacity-2 / 10 GiB runner. This is not a promise that every
compiler/linker is single-threaded. Desktop caches have separate platform keys,
install zstd before restore, and only save on `refs/heads/main`. Tag/feature
runs restore accessible caches but do not save their own scoped copies. To warm
these desktop-specific caches, manually dispatch on main after merge; there is
no extra main push trigger. Build jobs have no explicit release/package secrets
and checkout does not persist its automatic credentials. Only the publisher
receives `github.token` as `FORGEJO_RELEASE_TOKEN` for Forgejo release writes.

## Publication safety and recovery

The small standard-library Python publisher uses Forgejo's release API:

1. Before any API writes, validate both local archives, checksums and embedded
   source/tag/platform metadata. Check that the remote tag resolves to the exact
   built full commit and refuse any existing release, including drafts.
2. Create a **draft**, upload both archives and the combined checksums, then
   validate the API attachment list: exact names/count, sizes and same-origin
   browser download URLs. Recheck the remote tag and draft release ID/tag/SHA.
   There are **no browser downloads while draft**: on Forgejo 17.0.1 the automatic
   workflow token can read draft API metadata and upload attachments, but draft
   browser/attachment URLs return HTTP 404. No PAT or extra secret is required.
3. Publish the complete draft, read back its identity/source/public state, then
   download all three assets through their browser URLs and compare exact bytes
   (including the combined SHA-256 checksums). Recheck the remote tag afterward.

Partial uploads and prepublication validation failures stay draft. Any exception
in the publication attempt or subsequent verification triggers recovery: read
back the newly-created release at its exact ID, require matching ID/tag/source
SHA, restore it to draft only if public, and read back to confirm draft state.
A failed publish/rollback response may still have applied the write; readback
resolves that uncertainty. A mismatched release is never mutated by recovery.
The original exception is preserved; if draft state cannot be confirmed, its
traceback explicitly includes `ROLLBACK FAILED` and requires operator inspection.
No assets are deleted and no pre-existing release is overwritten or resumed.
Unexpected redirects/cross-origin downloads are rejected rather than forwarding
the token to another origin.

**Publication and byte verification are not atomic.** Assets are temporarily
public during postpublication verification, even if verification later fails.
Rollback cannot revoke bytes already downloaded or cached, and cannot guarantee
recovery after process termination, lost permissions, network failure or a
concurrent release modification. An interrupted run can leave a public release;
inspect its exact ID/tag/SHA and assets. Reruns deliberately refuse existing
releases, including the user's existing drafts: recovery/resume is an operator
operation, not an automatic update/clobber operation. Do not retarget/delete
release tags while publishing; publication is not atomic with a concurrent tag
move. Actual instance permissions, encoded slash-tag API routing, public asset
downloads and real compiler portability still require a real CI/tag run; local
fake-server success does not prove those deployment properties.

## No-build regression tests

```sh
python3 scripts/desktop-workflow.test.py
python3 scripts/desktop-release.test.py
python3 scripts/desktop-publish.test.py
```

The workflow contract requires PyYAML (already installed in the chart job).
Packaging exercises **explicitly synthetic ELF64/PE32+ executable fixtures**, real
ZIP/tar.gz creation, complete assets, selected-file allowlisting, source/tag
metadata, SHA-256, invalid/missing inputs, unsafe tag text and the actual Linux
launcher with spaces/arguments. Publisher integration tests use a local fake
HTTP Forgejo server, not live credentials/releases. No Rust compiler is invoked.
The existing chart CI job runs all three test files.
