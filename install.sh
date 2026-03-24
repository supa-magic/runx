#!/bin/sh
# runx installer — https://github.com/supa-magic/runx
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/supa-magic/runx/main/install.sh | sh
#
# Options (via environment variables):
#   RUNX_INSTALL_DIR  — custom install directory (default: /usr/local/bin or ~/.runx/bin)
#   RUNX_VERSION      — install a specific version (default: latest)

set -e

REPO="supa-magic/runx"
BINARY_NAME="runx"

# --- Helpers ---

info() {
    printf '\033[1;34m%s\033[0m\n' "$1"
}

success() {
    printf '\033[1;32m%s\033[0m\n' "$1"
}

error() {
    printf '\033[1;31merror: %s\033[0m\n' "$1" >&2
    exit 1
}

# --- Detect platform ---

detect_os() {
    case "$(uname -s)" in
        Darwin)  echo "apple-darwin" ;;
        Linux)   echo "unknown-linux-gnu" ;;
        *)       error "Unsupported OS: $(uname -s). runx supports macOS and Linux." ;;
    esac
}

detect_arch() {
    case "$(uname -m)" in
        x86_64|amd64)   echo "x86_64" ;;
        aarch64|arm64)  echo "aarch64" ;;
        *)              error "Unsupported architecture: $(uname -m). runx supports x86_64 and aarch64." ;;
    esac
}

# --- Determine install directory ---

detect_install_dir() {
    if [ -n "$RUNX_INSTALL_DIR" ]; then
        echo "$RUNX_INSTALL_DIR"
    elif [ -w /usr/local/bin ]; then
        echo "/usr/local/bin"
    else
        dir="$HOME/.runx/bin"
        mkdir -p "$dir"
        echo "$dir"
    fi
}

# --- Resolve version ---

resolve_version() {
    if [ -n "$RUNX_VERSION" ]; then
        echo "$RUNX_VERSION"
    else
        # Fetch latest release tag from GitHub API
        version=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
            | grep '"tag_name"' \
            | head -1 \
            | sed 's/.*"tag_name": *"//;s/".*//')
        if [ -z "$version" ]; then
            error "Failed to determine latest version. Set RUNX_VERSION manually."
        fi
        echo "$version"
    fi
}

# --- Main ---

main() {
    OS=$(detect_os)
    ARCH=$(detect_arch)
    INSTALL_DIR=$(detect_install_dir)
    VERSION=$(resolve_version)

    TARGET="${ARCH}-${OS}"
    ARCHIVE="runx-${TARGET}.tar.gz"
    URL="https://github.com/${REPO}/releases/download/${VERSION}/${ARCHIVE}"

    info "Installing runx ${VERSION} (${TARGET})"
    info "  from: ${URL}"
    info "  to:   ${INSTALL_DIR}/${BINARY_NAME}"

    # Create temp directory
    TMP_DIR=$(mktemp -d)
    trap 'rm -rf "$TMP_DIR"' EXIT

    # Download
    info "Downloading..."
    if ! curl -fsSL "$URL" -o "${TMP_DIR}/${ARCHIVE}"; then
        error "Download failed. Check that version ${VERSION} exists at:\n  https://github.com/${REPO}/releases"
    fi

    # Extract
    tar xzf "${TMP_DIR}/${ARCHIVE}" -C "$TMP_DIR"

    # Find the binary (may be at top level or in a subdirectory)
    BINARY=$(find "$TMP_DIR" -name "$BINARY_NAME" -type f | head -1)
    if [ -z "$BINARY" ]; then
        error "Binary not found in archive. The release may have a different format."
    fi

    # Install
    chmod +x "$BINARY"
    mkdir -p "$INSTALL_DIR"

    if [ -w "$INSTALL_DIR" ]; then
        mv "$BINARY" "${INSTALL_DIR}/${BINARY_NAME}"
    else
        info "Installing to ${INSTALL_DIR} (requires sudo)"
        sudo mv "$BINARY" "${INSTALL_DIR}/${BINARY_NAME}"
    fi

    # Verify
    if command -v "$BINARY_NAME" >/dev/null 2>&1; then
        INSTALLED_VERSION=$("$BINARY_NAME" --version 2>/dev/null || echo "unknown")
        success ""
        success "runx installed successfully!"
        success "  Version:  ${INSTALLED_VERSION}"
        success "  Location: ${INSTALL_DIR}/${BINARY_NAME}"
        success ""
        success "Get started:"
        success "  runx --with node@22 -- node -v"
        success "  runx --with python@3.12 -- python3 --version"
    else
        success ""
        success "runx installed to ${INSTALL_DIR}/${BINARY_NAME}"
        echo ""
        if [ "$INSTALL_DIR" = "$HOME/.runx/bin" ]; then
            echo "Add to your shell profile:"
            echo "  export PATH=\"\$HOME/.runx/bin:\$PATH\""
            echo ""
        fi
        echo "Then run:"
        echo "  runx --with node@22 -- node -v"
    fi
}

main
