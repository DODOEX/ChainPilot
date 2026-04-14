#!/usr/bin/env bash
set -e

REPO="DODOEX/ChainPilot"
BIN_NAME="chainpilot"
INSTALL_DIR="${HOME}/.chainpilot/bin"
TEMP_DIR=$(mktemp -d)

cleanup() {
    rm -rf "$TEMP_DIR"
}
trap cleanup EXIT

detect_platform() {
    local os=$(uname -s | tr '[:upper:]' '[:lower:]')
    local arch=$(uname -m)

    case "$arch" in
        x86_64) arch="amd64" ;;
        aarch64|arm64) arch="arm64" ;;
        *) echo "Unsupported architecture: $arch" >&2; exit 1 ;;
    esac

    case "$os" in
        linux) os="linux" ;;
        darwin) os="macos" ;;
        *) echo "Unsupported OS: $os" >&2; exit 1 ;;
    esac

    echo "${os}-${arch}"
}

download_release() {
    local version="$1"
    local platform="$2"
    local url="https://github.com/${REPO}/releases/download/${version}/${BIN_NAME}-${platform}.tar.gz"

    echo "Downloading ${BIN_NAME} ${version} for ${platform}..."
    if ! curl -fsSL "$url" -o "${TEMP_DIR}/${BIN_NAME}.tar.gz"; then
        echo "Failed to download from GitHub releases, falling back to source build..." >&2
        return 1
    fi

    mkdir -p "$INSTALL_DIR"
    tar -xzf "${TEMP_DIR}/${BIN_NAME}.tar.gz" -C "$TEMP_DIR"
    mv "${TEMP_DIR}/${BIN_NAME}" "${INSTALL_DIR}/${BIN_NAME}"
    chmod +x "${INSTALL_DIR}/${BIN_NAME}"
    echo "Installed to ${INSTALL_DIR}/${BIN_NAME}"
}

build_from_source() {
    echo "Building ${BIN_NAME} from source..."
    if ! command -v cargo &> /dev/null; then
        echo "Cargo not found. Please install Rust: https://rustup.rs" >&2
        exit 1
    fi

    cd "$(dirname "$0")/.."
    cargo build --release --quiet
    mkdir -p "$INSTALL_DIR"
    cp "target/release/${BIN_NAME}" "${INSTALL_DIR}/${BIN_NAME}"
    echo "Installed to ${INSTALL_DIR}/${BIN_NAME}"
}

main() {
    local version="${1:-latest}"
    local platform=$(detect_platform)

    if [ "$version" = "latest" ]; then
        version=$(curl -s "https://api.github.com/repos/${REPO}/releases/latest" | grep -o '"tag_name": "[^"]*' | cut -d'"' -f4 || echo "")
    fi

    mkdir -p "$INSTALL_DIR"

    if [ -n "$version" ] && [ "$version" != "latest" ]; then
        download_release "$version" "$platform" || build_from_source
    else
        build_from_source
    fi

    if [[ ":$PATH:" != *":${INSTALL_DIR}:"* ]]; then
        echo ""
        echo "Add to PATH (add to ~/.bashrc or ~/.zshrc to persist):"
        echo "  export PATH=\"\${HOME}/.chainpilot/bin:\$PATH\""
    fi
}

main "$@"
