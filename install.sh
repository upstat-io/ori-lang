#!/bin/sh
# Ori installer script
# Usage: curl -sSf https://raw.githubusercontent.com/upstat-io/ori-lang/master/install.sh | sh

set -e

REPO="upstat-io/ori-lang"
INSTALL_DIR="${ORI_INSTALL_DIR:-$HOME/.local/bin}"
BINARY_NAME="ori"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

info() {
    printf "${GREEN}info${NC}: %s\n" "$1"
}

warn() {
    printf "${YELLOW}warn${NC}: %s\n" "$1"
}

error() {
    printf "${RED}error${NC}: %s\n" "$1"
    exit 1
}

# Detect OS
detect_os() {
    case "$(uname -s)" in
        Linux*)     echo "linux" ;;
        Darwin*)    echo "darwin" ;;
        MINGW*|MSYS*|CYGWIN*) echo "windows" ;;
        *)          error "Unsupported operating system: $(uname -s)" ;;
    esac
}

# Detect architecture
detect_arch() {
    case "$(uname -m)" in
        x86_64|amd64)   echo "x86_64" ;;
        aarch64|arm64)  echo "aarch64" ;;
        *)              error "Unsupported architecture: $(uname -m)" ;;
    esac
}

# Get the latest release version from GitHub API
get_latest_version() {
    if command -v curl >/dev/null 2>&1; then
        curl -sSf "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name"' | sed -E 's/.*"([^"]+)".*/\1/'
    elif command -v wget >/dev/null 2>&1; then
        wget -qO- "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name"' | sed -E 's/.*"([^"]+)".*/\1/'
    else
        error "Neither curl nor wget found. Please install one of them."
    fi
}

# Download a file
download() {
    url="$1"
    output="$2"

    if command -v curl >/dev/null 2>&1; then
        curl -sSfL "$url" -o "$output"
    elif command -v wget >/dev/null 2>&1; then
        wget -q "$url" -O "$output"
    else
        error "Neither curl nor wget found. Please install one of them."
    fi
}

main() {
    info "Installing Ori..."

    OS=$(detect_os)
    ARCH=$(detect_arch)

    info "Detected OS: $OS, Architecture: $ARCH"

    # Get latest version
    info "Fetching latest release..."
    VERSION=$(get_latest_version)

    if [ -z "$VERSION" ]; then
        error "Could not determine latest version. Check your internet connection or visit https://github.com/${REPO}/releases"
    fi

    info "Latest version: $VERSION"

    # Construct download URL
    # Binary naming: ori-{version}-{os}-{arch}.tar.gz (or .zip for Windows)
    if [ "$OS" = "windows" ]; then
        ARCHIVE_NAME="ori-${VERSION}-${OS}-${ARCH}.zip"
    else
        ARCHIVE_NAME="ori-${VERSION}-${OS}-${ARCH}.tar.gz"
    fi

    DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${VERSION}/${ARCHIVE_NAME}"

    info "Downloading ${ARCHIVE_NAME}..."

    # Create temp directory
    TMP_DIR=$(mktemp -d)
    trap 'rm -rf "$TMP_DIR"' EXIT

    ARCHIVE_PATH="${TMP_DIR}/${ARCHIVE_NAME}"

    download "$DOWNLOAD_URL" "$ARCHIVE_PATH" || error "Failed to download from ${DOWNLOAD_URL}"

    # Extract archive
    info "Extracting..."
    cd "$TMP_DIR"

    if [ "$OS" = "windows" ]; then
        unzip -q "$ARCHIVE_PATH"
    else
        tar -xzf "$ARCHIVE_PATH"
    fi

    # Create install directory if it doesn't exist
    mkdir -p "$INSTALL_DIR"

    # Install binary
    if [ "$OS" = "windows" ]; then
        BINARY_PATH="${TMP_DIR}/${BINARY_NAME}.exe"
        INSTALLED_PATH="${INSTALL_DIR}/${BINARY_NAME}.exe"
    else
        BINARY_PATH="${TMP_DIR}/${BINARY_NAME}"
        INSTALLED_PATH="${INSTALL_DIR}/${BINARY_NAME}"
    fi

    if [ ! -f "$BINARY_PATH" ]; then
        error "Binary not found in archive. Please report this issue."
    fi

    mv "$BINARY_PATH" "$INSTALLED_PATH"
    chmod +x "$INSTALLED_PATH"

    info "Installed ori to ${INSTALLED_PATH}"

    # Check if install directory is in PATH
    case ":$PATH:" in
        *":${INSTALL_DIR}:"*)
            ;;
        *)
            warn "${INSTALL_DIR} is not in your PATH."
            echo ""
            echo "Add it to your shell profile:"
            echo ""
            echo "  For bash (~/.bashrc):"
            echo "    export PATH=\"\$HOME/.local/bin:\$PATH\""
            echo ""
            echo "  For zsh (~/.zshrc):"
            echo "    export PATH=\"\$HOME/.local/bin:\$PATH\""
            echo ""
            echo "  For fish (~/.config/fish/config.fish):"
            echo "    set -gx PATH \$HOME/.local/bin \$PATH"
            echo ""
            ;;
    esac

    # Verify installation
    if [ -x "$INSTALLED_PATH" ]; then
        echo ""
        info "Ori ${VERSION} installed successfully!"
        echo ""
        echo "Get started:"
        echo "  ori --help"
        echo ""
    else
        error "Installation verification failed"
    fi
}

main
