set windows-shell := ["powershell.exe", "-NoLogo", "-NoProfile", "-Command"]

# Host architecture, in Go-style names.
host_arch := if arch() == "aarch64" { "arm64" } else if arch() == "x86_64" { "amd64" } else { error("unsupported host architecture: " + arch()) }

os_triple := if os() == "macos" { "apple-darwin" } else if os() == "linux" { "unknown-linux-gnu" } else if os() == "windows" { "pc-windows-msvc" } else { error("unsupported OS: " + os()) }

bundle_flags := if os() == "macos" { "--bundles app" } else if os() == "linux" { "--bundles appimage" } else { "--no-bundle" }

build_dir := justfile_directory() / "build"

default:
    @just --list

# Build the GUI for this OS into build/. Pass `arm64` or `amd64` to override the host architecture.
gui arch=host_arch: (_compile (if arch == "arm64" { "aarch64" } else if arch == "amd64" { "x86_64" } else { error("arch must be arm64 or amd64, got: " + arch) }) + "-" + os_triple)

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
test suite="all": (_check-suite suite)
    {{ if suite != "node" { "cargo test --workspace" } else { "" } }}
    {{ if suite != "rust" { "pnpm --dir crates/gui install --frozen-lockfile" } else { "" } }}
    {{ if suite != "rust" { "pnpm --dir crates/gui test" } else { "" } }}

_check-suite suite:
    @{{ if suite =~ '^(all|rust|node)$' { "" } else { error("suite must be rust or node, got: " + suite) } }}

_compile triple:
    rustup target add {{ triple }}
    cd crates/gui && pnpm install --frozen-lockfile
    cd crates/gui && pnpm tauri build --target {{ triple }} {{ bundle_flags }}
    @just _package {{ triple }}

[macos]
_package triple:
    rm -rf "{{ build_dir }}/OpenPhotoAI.app"
    mkdir -p "{{ build_dir }}"
    cp -R "target/{{ triple }}/release/bundle/macos/Open Photo AI.app" "{{ build_dir }}/OpenPhotoAI.app"
    @echo "Built {{ build_dir }}/OpenPhotoAI.app"

[linux]
_package triple:
    rm -f "{{ build_dir }}/OpenPhotoAI.AppImage"
    mkdir -p "{{ build_dir }}"
    cp "$(ls -t target/{{ triple }}/release/bundle/appimage/*.AppImage | head -n 1)" "{{ build_dir }}/OpenPhotoAI.AppImage"
    chmod +x "{{ build_dir }}/OpenPhotoAI.AppImage"
    @echo "Built {{ build_dir }}/OpenPhotoAI.AppImage"

[windows]
_package triple:
    New-Item -ItemType Directory -Force -Path "{{ build_dir }}" | Out-Null
    Copy-Item -Force "target/{{ triple }}/release/OpenPhotoAI.exe" "{{ build_dir }}/OpenPhotoAI.exe"
    @echo "Built {{ build_dir }}/OpenPhotoAI.exe"
