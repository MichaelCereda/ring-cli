#!/bin/sh
set -e

# install.sh — platform-detecting installer for ring-cli
# Usage: curl -fsSL https://raw.githubusercontent.com/MichaelCereda/ring-cli/master/install.sh | sh
# Or:    INSTALL_DIR=/usr/local/bin sh install.sh

REPO="MichaelCereda/ring-cli"
BINARY="ring-cli"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

info()  { printf '[info]  %s\n' "$*"; }
warn()  { printf '[warn]  %s\n' "$*" >&2; }
error() { printf '[error] %s\n' "$*" >&2; exit 1; }

need_cmd() {
    if ! command -v "$1" > /dev/null 2>&1; then
        error "Required command not found: $1"
    fi
}

# ---------------------------------------------------------------------------
# Platform detection
# ---------------------------------------------------------------------------

detect_os() {
    _os="$(uname -s)"
    case "$_os" in
        Linux)  printf 'Linux'  ;;
        Darwin) printf 'Darwin' ;;
        *)      error "Unsupported operating system: $_os. Only Linux and macOS are supported." ;;
    esac
}

detect_arch() {
    _arch="$(uname -m)"
    case "$_arch" in
        x86_64 | amd64)         printf 'x86_64'  ;;
        aarch64 | arm64)        printf 'aarch64' ;;
        armv7l | armv6l | arm*) printf 'arm'     ;;
        *)                      error "Unsupported architecture: $_arch. Supported: x86_64, aarch64, armv7l." ;;
    esac
}

# ---------------------------------------------------------------------------
# Archive name resolution
# Mirrors the naming in .github/workflows/ci.yml
# ---------------------------------------------------------------------------

resolve_archive_name() {
    _os="$1"
    _arch="$2"

    case "${_os}-${_arch}" in
        Linux-x86_64)  printf 'ring-cli-Linux-x86_64-musl.tar.gz'  ;;
        Linux-aarch64) printf 'ring-cli-Linux-aarch64-musl.tar.gz' ;;
        Linux-arm)     printf 'ring-cli-Linux-arm-musl.tar.gz'     ;;
        Darwin-x86_64) printf 'ring-cli-Darwin-x86_64.tar.gz'      ;;
        Darwin-aarch64)printf 'ring-cli-Darwin-aarch64.tar.gz'     ;;
        *)             error "No release archive available for ${_os}-${_arch}." ;;
    esac
}

# ---------------------------------------------------------------------------
# Latest release tag from GitHub API
# ---------------------------------------------------------------------------

get_latest_tag() {
    _api_url="https://api.github.com/repos/${REPO}/releases/latest"

    if command -v curl > /dev/null 2>&1; then
        _tag="$(curl -fsSL "$_api_url" | \
            tr ',' '\n' | \
            grep '"tag_name"' | \
            head -n 1 | \
            sed 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/')"
    elif command -v wget > /dev/null 2>&1; then
        _tag="$(wget -qO- "$_api_url" | \
            tr ',' '\n' | \
            grep '"tag_name"' | \
            head -n 1 | \
            sed 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/')"
    else
        error "Neither curl nor wget found. Please install one of them and try again."
    fi

    if [ -z "$_tag" ]; then
        error "Could not determine the latest release tag from GitHub. Check your internet connection."
    fi

    printf '%s' "$_tag"
}

# ---------------------------------------------------------------------------
# Download helper (curl with wget fallback)
# ---------------------------------------------------------------------------

download() {
    _url="$1"
    _dest="$2"

    if command -v curl > /dev/null 2>&1; then
        curl -fsSL --progress-bar -o "$_dest" "$_url"
    elif command -v wget > /dev/null 2>&1; then
        wget -q --show-progress -O "$_dest" "$_url"
    else
        error "Neither curl nor wget found. Please install one of them and try again."
    fi
}

# ---------------------------------------------------------------------------
# PATH check
# ---------------------------------------------------------------------------

check_path() {
    _dir="$1"

    # Walk the colon-separated PATH to see if _dir is already present.
    _found=0
    _old_ifs="$IFS"
    IFS=':'
    for _p in $PATH; do
        if [ "$_p" = "$_dir" ]; then
            _found=1
            break
        fi
    done
    IFS="$_old_ifs"

    if [ "$_found" -eq 0 ]; then
        warn "$_dir is not in your PATH."
        warn "Add the following line to your shell profile (~/.profile, ~/.bashrc, ~/.zshrc, etc.):"
        warn "  export PATH=\"\$PATH:$_dir\""
    fi
}

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

main() {
    need_cmd uname
    need_cmd tar

    OS="$(detect_os)"
    ARCH="$(detect_arch)"
    ARCHIVE="$(resolve_archive_name "$OS" "$ARCH")"

    info "Detected platform: ${OS}-${ARCH}"
    info "Fetching latest release tag..."

    TAG="$(get_latest_tag)"
    info "Latest release: $TAG"

    DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${TAG}/${ARCHIVE}"
    info "Downloading: $DOWNLOAD_URL"

    # Use a temporary directory that is cleaned up on exit regardless of success.
    TMP_DIR="$(mktemp -d)"
    trap 'rm -rf "$TMP_DIR"' EXIT INT TERM

    TMP_ARCHIVE="${TMP_DIR}/${ARCHIVE}"
    download "$DOWNLOAD_URL" "$TMP_ARCHIVE"

    info "Extracting archive..."
    tar -xzf "$TMP_ARCHIVE" -C "$TMP_DIR"

    # Ensure the install directory exists.
    if [ ! -d "$INSTALL_DIR" ]; then
        info "Creating install directory: $INSTALL_DIR"
        mkdir -p "$INSTALL_DIR"
    fi

    # Move the binary into place.
    if [ ! -f "${TMP_DIR}/${BINARY}" ]; then
        error "Binary '${BINARY}' not found in the extracted archive."
    fi

    cp "${TMP_DIR}/${BINARY}" "${INSTALL_DIR}/${BINARY}"
    chmod +x "${INSTALL_DIR}/${BINARY}"

    info "ring-cli ${TAG} installed to ${INSTALL_DIR}/${BINARY}"

    check_path "$INSTALL_DIR"

    info "Installation complete. Run 'ring-cli --help' to get started."
}

main "$@"
