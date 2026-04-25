#!/bin/sh
# Open Photo AI installer — macOS and Linux
# Usage:
#   curl -fsSL https://vegidio.github.io/open-photo-ai/install.sh | sh
#   curl -fsSL https://vegidio.github.io/open-photo-ai/install.sh | OPAI_VERSION=<tag> sh
#
# OPAI_VERSION defaults to 'latest', which is resolved dynamically from
# https://github.com/vegidio/open-photo-ai/releases/latest at run time.

set -eu

REPO="vegidio/open-photo-ai"
OPAI_VERSION="${OPAI_VERSION:-latest}"

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

usage() {
    cat <<EOF
Usage: install.sh [options]

Options:
  --version <tag>  Install a specific version (default: latest)
  -h, --help       Show this help message

Environment variables:
  OPAI_VERSION       release tag         (default: latest)
  OPAI_INSTALL_DIR   install dir         (default: ~/Applications on macOS,
                                                   /usr/local/bin on Linux)
EOF
}

while [ $# -gt 0 ]; do
    case "$1" in
        --version)     shift; [ $# -gt 0 ] || error "--version requires an argument"; OPAI_VERSION="$1" ;;
        --version=*)   OPAI_VERSION="${1#--version=}" ;;
        -h|--help)     usage; exit 0 ;;
        *)             error "unknown option: $1 (try --help)" ;;
    esac
    shift
done

case "$(uname -s)" in
    Darwin) OS=darwin ;;
    Linux)  OS=linux ;;
    *) error "unsupported OS: $(uname -s). This installer supports macOS and Linux. For Windows, use install.ps1." ;;
esac

case "$(uname -m)" in
    arm64|aarch64)  ARCH=arm64 ;;
    x86_64|amd64)   ARCH=amd64 ;;
    *) error "unsupported architecture: $(uname -m)" ;;
esac

if [ "$OS" = darwin ]; then
    INSTALL_DIR="${OPAI_INSTALL_DIR:-$HOME/Applications}"
else
    INSTALL_DIR="${OPAI_INSTALL_DIR:-/usr/local/bin}"
fi

command -v curl  >/dev/null 2>&1 || error "curl is required but not found"
command -v unzip >/dev/null 2>&1 || error "unzip is required but not found"

if [ "$OPAI_VERSION" = "latest" ]; then
    info "resolving latest version..."
    RESOLVED_URL=$(curl -fsSLI -o /dev/null -w '%{url_effective}' "https://github.com/${REPO}/releases/latest") \
        || error "could not reach github.com to resolve the latest version"
    TAG=$(printf '%s' "$RESOLVED_URL" | sed -n 's|.*/tag/\(.*\)$|\1|p')
    [ -n "$TAG" ] || error "could not parse latest version from $RESOLVED_URL"
else
    TAG="$OPAI_VERSION"
fi

info "installing Open Photo AI ${TAG} (${OS}/${ARCH})"

TMP=$(mktemp -d -t opai-install.XXXXXX)
trap 'rm -rf "$TMP"' EXIT INT TERM

download_zip() {
    asset="$1"
    url="https://github.com/${REPO}/releases/download/${TAG}/${asset}"
    info "downloading ${asset}"
    curl -fL --progress-bar -o "$TMP/$asset" "$url" \
        || error "download failed: $url"
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

install_darwin() {
    asset="opai-gui_darwin_${ARCH}.zip"
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

install_linux() {
    asset="opai-gui_linux_${ARCH}.zip"
    download_zip "$asset"
    bin=$(find "$TMP/${asset%.zip}" -maxdepth 3 -name 'OpenPhotoAI' -type f 2>/dev/null | head -n 1)
    [ -n "$bin" ] || error "OpenPhotoAI binary not found inside $asset"
    chmod +x "$bin"

    info "installing OpenPhotoAI to ${INSTALL_DIR}"
    move_in_place "$bin" "${INSTALL_DIR}/OpenPhotoAI"
    info "${GREEN}OpenPhotoAI installed${RESET} at ${INSTALL_DIR}/OpenPhotoAI"
}

case "$OS" in
    darwin) install_darwin ;;
    linux)  install_linux ;;
esac

printf '%s\n' "${GREEN}done.${RESET}" >&2
