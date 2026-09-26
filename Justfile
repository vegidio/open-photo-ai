set windows-shell := ["powershell.exe", "-NoLogo", "-NoProfile", "-Command"]

# Host architecture, as spelled in the release asset names.
host_arch := if arch() == "aarch64" { "arm64" } else if arch() == "x86_64" { "x64" } else { error("unsupported host architecture: " + arch()) }

os_triple := if os() == "macos" { "apple-darwin" } else if os() == "linux" { "unknown-linux-gnu" } else if os() == "windows" { "pc-windows-msvc" } else { error("unsupported OS: " + os()) }

bundle_flags := if os() == "macos" { "--bundles app" } else if os() == "linux" { "--bundles deb,rpm" } else { "--no-bundle" }

build_dir := justfile_directory() / "build"

default:
    @just --list

# Build the GUI for this OS into build/. Pass `arm64` or `x64` to override the host architecture.
gui arch=host_arch: (_compile (if arch == "arm64" { "aarch64" } else if arch == "x64" { "x86_64" } else { error("arch must be arm64 or x64, got: " + arch) }) + "-" + os_triple)

# Build the GUI like `gui`, then package it as build/opai-gui_<os>_<arch>.zip (.deb and .rpm on Linux, unzipped).
package arch=host_arch: (gui arch) (_package "opai-gui_" + os() + "_" + arch)

# Delete build output and all generated build/dev artifacts (target, node_modules, dist, ...).
[unix]
clean:
    rm -rf "{{ build_dir }}" target crates/gui/dist crates/gui/gen/schemas
    find . -path ./.git -prune -o \( -name node_modules -o -name .vite -o -name coverage \) -type d -prune -exec rm -rf {} +
    find . -path ./.git -prune -o -name '*.tsbuildinfo' -type f -exec rm -f {} +

[windows]
clean:
    Remove-Item -Recurse -Force -ErrorAction SilentlyContinue "{{ build_dir }}", target, crates/gui/dist, crates/gui/gen/schemas
    Get-ChildItem -Recurse -Force -Directory -Include node_modules,.vite,coverage | Where-Object { $_.FullName -notmatch '\\node_modules\\.+' } | Remove-Item -Recurse -Force
    Get-ChildItem -Recurse -Force -File -Filter *.tsbuildinfo | Remove-Item -Force

# Run the Rust and frontend tests. Pass `rust` or `node` to run only one of them.
# The frontend is built before the Rust tests because `tauri::generate_context!` embeds crates/gui/dist at compile
# time, and dist is not committed.
test suite="all": (_check-suite suite)
    @just _pnpm install --frozen-lockfile
    {{ if suite != "node" { "just _pnpm build" } else { "" } }}
    {{ if suite != "node" { "cargo test --workspace" } else { "" } }}
    {{ if suite != "rust" { "just _pnpm test" } else { "" } }}

# Run a component in development mode. Currently only `gui` is supported.
dev target: (_check-dev target)
    @just _pnpm tauri dev

# Runs pnpm from inside the GUI rather than with `--dir`: corepack picks the pnpm version from the `packageManager`
# field of the package.json in the *current* directory, and never sees `--dir`, so run from the root it starts
# whatever pnpm is installed globally, which then refuses the GUI's pinned version.
[working-directory: 'crates/gui']
_pnpm +args:
    pnpm {{ args }}

_check-suite suite:
    @{{ if suite =~ '^(all|rust|node)$' { "" } else { error("suite must be rust or node, got: " + suite) } }}

_check-dev target:
    @{{ if target == "gui" { "" } else { error("target must be gui, got: " + target) } }}

_compile triple:
    rustup target add {{ triple }}
    @just _pnpm install --frozen-lockfile
    @just _pnpm tauri build --target {{ triple }} {{ bundle_flags }}
    @just _stage {{ triple }}

[macos]
_stage triple:
    rm -rf "{{ build_dir }}/OpenPhotoAI.app"
    mkdir -p "{{ build_dir }}"
    cp -R "target/{{ triple }}/release/bundle/macos/Open Photo AI.app" "{{ build_dir }}/OpenPhotoAI.app"
    @echo "Built {{ build_dir }}/OpenPhotoAI.app"

[linux]
_stage triple:
    rm -f "{{ build_dir }}/OpenPhotoAI.deb" "{{ build_dir }}/OpenPhotoAI.rpm"
    mkdir -p "{{ build_dir }}"
    cp "$(ls -t target/{{ triple }}/release/bundle/deb/*.deb | head -n 1)" "{{ build_dir }}/OpenPhotoAI.deb"
    cp "$(ls -t target/{{ triple }}/release/bundle/rpm/*.rpm | head -n 1)" "{{ build_dir }}/OpenPhotoAI.rpm"
    @echo "Built {{ build_dir }}/OpenPhotoAI.deb"
    @echo "Built {{ build_dir }}/OpenPhotoAI.rpm"

[windows]
_stage triple:
    New-Item -ItemType Directory -Force -Path "{{ build_dir }}" | Out-Null
    Copy-Item -Force "target/{{ triple }}/release/OpenPhotoAI.exe" "{{ build_dir }}/OpenPhotoAI.exe"
    @echo "Built {{ build_dir }}/OpenPhotoAI.exe"

[macos]
_package base:
    cd "{{ build_dir }}" && rm -f "{{ base }}.zip" && zip -9 -r -y -q "{{ base }}.zip" OpenPhotoAI.app
    @echo "Packaged {{ build_dir }}/{{ base }}.zip"

# The .deb and .rpm are already compressed and are released as they are, so they're only renamed.
[linux]
_package base:
    cp "{{ build_dir }}/OpenPhotoAI.deb" "{{ build_dir }}/{{ base }}.deb"
    cp "{{ build_dir }}/OpenPhotoAI.rpm" "{{ build_dir }}/{{ base }}.rpm"
    @echo "Packaged {{ build_dir }}/{{ base }}.deb"
    @echo "Packaged {{ build_dir }}/{{ base }}.rpm"

# Test-Path rather than -ErrorAction SilentlyContinue: a silenced error still leaves $? false, and
# `powershell -Command` turns that into exit code 1 when the zip doesn't exist yet.
[windows]
_package base:
    if (Test-Path "{{ build_dir }}/{{ base }}.zip") { Remove-Item -Force "{{ build_dir }}/{{ base }}.zip" }
    Compress-Archive -CompressionLevel Optimal -Path "{{ build_dir }}/OpenPhotoAI.exe" -DestinationPath "{{ build_dir }}/{{ base }}.zip"
    @echo "Packaged {{ build_dir }}/{{ base }}.zip"
