#!/bin/sh
# Open Photo AI installer — macOS and Linux
# Usage:
#   curl -fsSL https://vegidio.github.io/open-photo-ai/install.sh | sh
#   curl -fsSL https://vegidio.github.io/open-photo-ai/install.sh | OPAI_VERSION=<tag> sh
#   curl -fsSL https://vegidio.github.io/open-photo-ai/install.sh | OPAI_INSTALL=cli sh
#
# OPAI_VERSION defaults to 'latest', which is resolved dynamically from
# https://github.com/vegidio/open-photo-ai/releases/latest at run time.
# OPAI_INSTALL picks what to install: 'gui', 'cli' or 'both' (the default).

set -eu

REPO="vegidio/open-photo-ai"
OPAI_VERSION="${OPAI_VERSION:-latest}"
OPAI_INSTALL="${OPAI_INSTALL:-both}"

if [ -t 1 ]; then
    BOLD=$(printf '\033[1m')
    RED=$(printf '\033[31m')
    GREEN=$(printf '\033[32m')
    YELLOW=$(printf '\033[33m')
    RESET=$(printf '\033[0m')
else
    BOLD=""; RED=""; GREEN=""; YELLOW=""; RESET=""
fi

info()  { printf '%s==>%s %s\n' "$BOLD" "$RESET" "$*" >&2; }
warn()  { printf '%swarn:%s %s\n' "$YELLOW" "$RESET" "$*" >&2; }
error() { printf '%serror:%s %s\n' "$RED" "$RESET" "$*" >&2; exit 1; }
has()   { command -v "$1" >/dev/null 2>&1; }

usage() {
    cat <<EOF
Usage: install.sh [options]

Options:
  --version <tag>            Install a specific version (default: latest)
  --install <gui|cli|both>   What to install (default: both)
  -h, --help                 Show this help message

Environment variables:
  OPAI_VERSION       release tag              (default: latest)
  OPAI_INSTALL       gui, cli or both         (default: both)
  OPAI_INSTALL_DIR   GUI install dir, macOS   (default: ~/Applications)
  OPAI_BIN_DIR       CLI install dir          (default: ~/.local/bin)

On Linux the GUI is installed system-wide as a .deb or .rpm package, picked
from the distro, so sudo is needed unless the installer runs as root. The CLI
is installed for the current user and doesn't need sudo.
EOF
}

while [ $# -gt 0 ]; do
    case "$1" in
        --version)     shift; [ $# -gt 0 ] || error "--version requires an argument"; OPAI_VERSION="$1" ;;
        --version=*)   OPAI_VERSION="${1#--version=}" ;;
        --install)     shift; [ $# -gt 0 ] || error "--install requires an argument"; OPAI_INSTALL="$1" ;;
        --install=*)   OPAI_INSTALL="${1#--install=}" ;;
        -h|--help)     usage; exit 0 ;;
        *)             error "unknown option: $1 (try --help)" ;;
    esac
    shift
done

case "$OPAI_INSTALL" in
    gui)  WANT_GUI=1; WANT_CLI=0; WHAT=gui ;;
    cli)  WANT_GUI=0; WANT_CLI=1; WHAT=cli ;;
    both) WANT_GUI=1; WANT_CLI=1; WHAT=gui+cli ;;
    *) error "OPAI_INSTALL must be gui, cli or both, got: $OPAI_INSTALL" ;;
esac

case "$(uname -s)" in
    Darwin) OS=macos ;;
    Linux)  OS=linux ;;
    *) error "unsupported OS: $(uname -s). This installer supports macOS and Linux. For Windows, use install.ps1." ;;
esac

case "$(uname -m)" in
    arm64|aarch64)  ARCH=arm64 ;;
    x86_64|amd64)   ARCH=x64 ;;
    *) error "unsupported architecture: $(uname -m)" ;;
esac

INSTALL_DIR="${OPAI_INSTALL_DIR:-$HOME/Applications}"
BIN_DIR="${OPAI_BIN_DIR:-$HOME/.local/bin}"

has curl || error "curl is required but not found"
if [ "$OS" = macos ] || [ "$WANT_CLI" -eq 1 ]; then
    has unzip || error "unzip is required but not found"
fi

if [ "$OPAI_VERSION" = "latest" ]; then
    info "resolving latest version..."
    RESOLVED_URL=$(curl -fsSLI -o /dev/null -w '%{url_effective}' "https://github.com/${REPO}/releases/latest") \
        || error "could not reach github.com to resolve the latest version"
    TAG=$(printf '%s' "$RESOLVED_URL" | sed -n 's|.*/tag/\(.*\)$|\1|p')
    [ -n "$TAG" ] || error "could not parse latest version from $RESOLVED_URL"
else
    TAG="$OPAI_VERSION"
fi

info "installing Open Photo AI ${TAG} (${OS}/${ARCH}, ${WHAT})"

TMP=$(mktemp -d -t opai-install.XXXXXX)
trap 'rm -rf "$TMP"' EXIT INT TERM

download_asset() {
    asset="$1"
    url="https://github.com/${REPO}/releases/download/${TAG}/${asset}"
    info "downloading ${asset}"
    curl -fL --progress-bar -o "$TMP/$asset" "$url" \
        || error "download failed: $url"
}

download_zip() {
    asset="$1"
    download_asset "$asset"
    mkdir -p "$TMP/${asset%.zip}"
    unzip -q -o "$TMP/$asset" -d "$TMP/${asset%.zip}" \
        || error "failed to unzip $asset"
}

move_in_place() {
    src="$1"
    dst="$2"
    dst_dir=$(dirname "$dst")
    if [ -w "$dst_dir" ] || { [ ! -e "$dst_dir" ] && mkdir -p "$dst_dir" 2>/dev/null; }; then
        rm -rf "$dst"
        mv "$src" "$dst"
    else
        info "elevating with sudo to write to ${dst_dir}"
        sudo rm -rf "$dst"
        sudo mv "$src" "$dst"
    fi
}

install_macos() {
    asset="opai-gui_macos_${ARCH}.zip"
    download_zip "$asset"
    app_src=$(find "$TMP/${asset%.zip}" -maxdepth 3 -name '*.app' -type d 2>/dev/null | head -n 1)
    [ -n "$app_src" ] || error ".app bundle not found inside $asset"
    app_name=$(basename "$app_src")

    mkdir -p "$INSTALL_DIR"
    info "installing ${app_name} to ${INSTALL_DIR}"
    move_in_place "$app_src" "${INSTALL_DIR}/${app_name}"
    xattr -dr com.apple.quarantine "${INSTALL_DIR}/${app_name}" 2>/dev/null || true
    info "${GREEN}${app_name} installed${RESET} at ${INSTALL_DIR}/${app_name}"
}

# Prints `deb` or `rpm`: from the distro's os-release when it's one we know, otherwise from whichever package
# manager is on the system.
detect_pkg_format() {
    if [ -r /etc/os-release ]; then
        ids=$(. /etc/os-release && printf '%s %s' "${ID:-}" "${ID_LIKE:-}")
        for id in $ids; do
            case "$id" in
                debian|ubuntu|linuxmint|pop|elementary|raspbian|kali|zorin|neon)
                    echo deb; return ;;
                fedora|rhel|centos|rocky|almalinux|suse|opensuse*|sles|mageia|amzn|ol|nobara)
                    echo rpm; return ;;
            esac
        done
    fi
    if has apt-get || has dpkg; then echo deb; return; fi
    if has dnf || has yum || has zypper || has rpm; then echo rpm; return; fi
    error "unsupported Linux distribution: no apt/dpkg or dnf/yum/zypper/rpm found"
}

# Installing a package is system-wide, so it needs root.
set_sudo() {
    if [ "$(id -u)" -eq 0 ]; then
        SUDO=""
    else
        has sudo || error "sudo is required to install the package (or run this installer as root)"
        SUDO="sudo"
        info "elevating with sudo to install the package"
    fi
}

install_deb() {
    pkg="$1"
    # apt fetches local files as the unprivileged _apt user, which can't read mktemp's 0700 directory.
    chmod 755 "$TMP" && chmod 644 "$pkg"
    if has apt-get; then
        # The path must be absolute (it is, via $TMP) for apt to treat it as a file and resolve its dependencies.
        $SUDO apt-get install -y "$pkg" || error "apt-get failed to install $(basename "$pkg")"
    else
        $SUDO dpkg -i "$pkg" || error "dpkg failed to install $(basename "$pkg")"
    fi
}

install_rpm() {
    pkg="$1"
    if has dnf; then
        $SUDO dnf install -y "$pkg"
    elif has yum; then
        $SUDO yum install -y "$pkg"
    elif has zypper; then
        # The package isn't signed.
        $SUDO zypper --non-interactive install --allow-unsigned-rpm "$pkg"
    else
        $SUDO rpm -Uvh --replacepkgs "$pkg"
    fi || error "failed to install $(basename "$pkg")"
}

# Earlier versions installed an AppImage into ~/Applications with its own menu entry, which would now show up next
# to the package's.
remove_appimage() {
    old_app="${INSTALL_DIR}/OpenPhotoAI.AppImage"
    old_entry="${XDG_DATA_HOME:-$HOME/.local/share}/applications/open-photo-ai.desktop"
    if [ -e "$old_app" ]; then
        rm -f "$old_app" 2>/dev/null || $SUDO rm -f "$old_app" || warn "could not remove ${old_app}"
        info "removed the previous AppImage at ${old_app}"
    fi
    if [ -e "$old_entry" ]; then
        rm -f "$old_entry" && info "removed the previous menu entry at ${old_entry}"
        if has update-desktop-database; then
            update-desktop-database "$(dirname "$old_entry")" >/dev/null 2>&1 || true
        fi
    fi
}

# Copies the menu entry the package installed onto the user's desktop; best-effort.
create_shortcut() {
    fmt="$1"
    name="$2"
    if [ "$fmt" = deb ]; then
        files=$(dpkg -L "$name" 2>/dev/null || true)
    else
        files=$(rpm -ql "$name" 2>/dev/null || true)
    fi
    entry=$(printf '%s\n' "$files" | grep '/applications/.*\.desktop$' | head -n 1 || true)
    if [ -z "$entry" ]; then
        entry=$(grep -l '^Exec=.*OpenPhotoAI' /usr/share/applications/*.desktop 2>/dev/null | head -n 1 || true)
    fi
    if [ -z "$entry" ] || [ ! -r "$entry" ]; then
        warn "could not find the app's menu entry, so no desktop shortcut was created"
        return
    fi

    desktop_dir=""
    has xdg-user-dir && desktop_dir=$(xdg-user-dir DESKTOP 2>/dev/null || true)
    { [ -n "$desktop_dir" ] && [ "$desktop_dir" != "$HOME" ]; } || desktop_dir="$HOME/Desktop"
    if [ ! -d "$desktop_dir" ]; then
        info "no desktop folder at ${desktop_dir}, skipping the desktop shortcut"
        return
    fi

    shortcut="${desktop_dir}/open-photo-ai.desktop"
    if cp "$entry" "$shortcut" 2>/dev/null && chmod +x "$shortcut" 2>/dev/null; then
        # GNOME won't launch a desktop file until it's marked trusted.
        if has gio; then
            gio set "$shortcut" metadata::trusted true >/dev/null 2>&1 || true
        fi
        info "desktop shortcut created at ${shortcut}"
    else
        warn "could not create desktop shortcut at ${shortcut}"
    fi
}

install_linux() {
    fmt=$(detect_pkg_format)
    asset="opai-gui_linux_${ARCH}.${fmt}"
    download_asset "$asset"
    pkg="$TMP/$asset"

    if [ "$fmt" = deb ]; then
        name=$(dpkg-deb -f "$pkg" Package 2>/dev/null || true)
    else
        name=$(rpm -qp --queryformat '%{NAME}' "$pkg" 2>/dev/null || true)
    fi
    name="${name:-open-photo-ai}"

    set_sudo
    info "installing ${asset} (${name})"
    if [ "$fmt" = deb ]; then install_deb "$pkg"; else install_rpm "$pkg"; fi
    info "${GREEN}Open Photo AI installed${RESET} (${fmt} package ${name})"

    remove_appimage
    create_shortcut "$fmt" "$name"
}

# The CLI is a single binary, so it goes to the user's bin dir on both OSes and never needs sudo unless that dir does.
install_cli() {
    asset="opai-cli_${OS}_${ARCH}.zip"
    download_zip "$asset"
    bin_src="$TMP/${asset%.zip}/opai"
    [ -f "$bin_src" ] || error "opai binary not found inside $asset"
    chmod +x "$bin_src"

    mkdir -p "$BIN_DIR" 2>/dev/null || true
    info "installing opai to ${BIN_DIR}"
    move_in_place "$bin_src" "${BIN_DIR}/opai"
    if [ "$OS" = macos ]; then
        xattr -d com.apple.quarantine "${BIN_DIR}/opai" 2>/dev/null || true
    fi
    info "${GREEN}opai installed${RESET} at ${BIN_DIR}/opai"

    case ":${PATH}:" in
        *":${BIN_DIR}:"*) ;;
        *) warn "${BIN_DIR} is not in your PATH; add it to your shell profile to run opai:"
           # shellcheck disable=SC2016 # $PATH is printed literally, for the user to paste.
           printf '    export PATH="%s:$PATH"\n' "$BIN_DIR" >&2 ;;
    esac
}

if [ "$WANT_GUI" -eq 1 ]; then
    case "$OS" in
        macos)  install_macos ;;
        linux)  install_linux ;;
    esac
fi

if [ "$WANT_CLI" -eq 1 ]; then
    install_cli
fi

printf '%s\n' "${GREEN}done.${RESET}" >&2
