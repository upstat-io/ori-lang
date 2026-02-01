#!/bin/sh
# Ori Language Installer
#
# Usage:
#   curl -fsSL https://ori-lang.com/install.sh | sh
#   curl -fsSL https://ori-lang.com/install.sh | sh -s -- --nightly
#   curl -fsSL https://ori-lang.com/install.sh | sh -s -- --version v0.1.0-alpha.2
#
# Options:
#   --nightly       Install latest nightly/alpha release (default during alpha phase)
#   --stable        Install latest stable release only
#   --version VER   Install specific version (e.g., v0.1.0-alpha.2)
#   --help          Show this help message

set -e

REPO="upstat-io/ori-lang"
INSTALL_DIR="${ORI_INSTALL_DIR:-$HOME/.local/bin}"
BINARY_NAME="ori"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

info() {
    printf "${CYAN}info${NC}: %s\n" "$1"
}

warn() {
    printf "${YELLOW}warn${NC}: %s\n" "$1"
}

error() {
    printf "${RED}error${NC}: %s\n" "$1" >&2
    exit 1
}

success() {
    printf "${GREEN}success${NC}: %s\n" "$1"
}

usage() {
    cat << EOF
Ori Language Installer

Usage:
  curl -fsSL https://ori-lang.com/install.sh | sh
  curl -fsSL https://ori-lang.com/install.sh | sh -s -- [OPTIONS]

Options:
  --nightly       Install latest nightly/alpha release (default)
  --stable        Install latest stable release only
  --version VER   Install specific version (e.g., v0.1.0-alpha.2)
  --help          Show this help message

Environment:
  ORI_INSTALL_DIR   Installation directory (default: ~/.local/bin)

Examples:
  # Install latest nightly (default)
  curl -fsSL https://ori-lang.com/install.sh | sh

  # Install specific version
  curl -fsSL https://ori-lang.com/install.sh | sh -s -- --version v0.1.0-alpha.2

  # Install to custom directory
  ORI_INSTALL_DIR=/usr/local/bin curl -fsSL https://ori-lang.com/install.sh | sh
EOF
    exit 0
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

# Download helper that supports both curl and wget
fetch() {
    url="$1"
    if command -v curl >/dev/null 2>&1; then
        curl -fsSL "$url"
    elif command -v wget >/dev/null 2>&1; then
        wget -qO- "$url"
    else
        error "Neither curl nor wget found. Please install one of them."
    fi
}

# Download to file
download() {
    url="$1"
    output="$2"
    if command -v curl >/dev/null 2>&1; then
        curl -fsSL "$url" -o "$output"
    elif command -v wget >/dev/null 2>&1; then
        wget -q "$url" -O "$output"
    else
        error "Neither curl nor wget found. Please install one of them."
    fi
}

# Get the latest release version from GitHub API
# $1: "true" to include pre-releases (nightly), "false" for stable only
get_latest_version() {
    include_prereleases="$1"

    if [ "$include_prereleases" = "true" ]; then
        # Get all releases and take the first (most recent)
        fetch "https://api.github.com/repos/${REPO}/releases" | \
            grep '"tag_name"' | \
            head -1 | \
            sed -E 's/.*"([^"]+)".*/\1/'
    else
        # Get only the latest stable release
        fetch "https://api.github.com/repos/${REPO}/releases/latest" | \
            grep '"tag_name"' | \
            sed -E 's/.*"([^"]+)".*/\1/'
    fi
}

main() {
    # Parse arguments
    VERSION=""
    INCLUDE_PRERELEASES="true"  # Default to nightly during alpha phase

    while [ $# -gt 0 ]; do
        case "$1" in
            --help|-h)
                usage
                ;;
            --nightly)
                INCLUDE_PRERELEASES="true"
                shift
                ;;
            --stable)
                INCLUDE_PRERELEASES="false"
                shift
                ;;
            --version)
                VERSION="$2"
                shift 2
                ;;
            *)
                error "Unknown option: $1. Run with --help for usage."
                ;;
        esac
    done

    echo ""
    printf "${BOLD}Ori Language Installer${NC}\n"
    echo ""

    OS=$(detect_os)
    ARCH=$(detect_arch)

    info "Detected platform: $OS-$ARCH"

    # Get version if not specified
    if [ -z "$VERSION" ]; then
        info "Finding latest release..."
        VERSION=$(get_latest_version "$INCLUDE_PRERELEASES")

        if [ -z "$VERSION" ]; then
            if [ "$INCLUDE_PRERELEASES" = "false" ]; then
                error "No stable release found. Try --nightly for pre-release versions."
            else
                error "Could not determine latest version. Check https://github.com/${REPO}/releases"
            fi
        fi
    fi

    # Ensure version starts with 'v'
    case "$VERSION" in
        v*) ;;
        *)  VERSION="v$VERSION" ;;
    esac

    info "Installing version: $VERSION"

    # Construct download URL
    if [ "$OS" = "windows" ]; then
        ARCHIVE_NAME="ori-${VERSION}-${OS}-${ARCH}.zip"
    else
        ARCHIVE_NAME="ori-${VERSION}-${OS}-${ARCH}.tar.gz"
    fi

    DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${VERSION}/${ARCHIVE_NAME}"

    info "Downloading $ARCHIVE_NAME..."

    # Create temp directory
    TMP_DIR=$(mktemp -d)
    trap 'rm -rf "$TMP_DIR"' EXIT

    ARCHIVE_PATH="${TMP_DIR}/${ARCHIVE_NAME}"

    if ! download "$DOWNLOAD_URL" "$ARCHIVE_PATH"; then
        echo ""
        error "Failed to download from: $DOWNLOAD_URL

This could mean:
  - The version doesn't exist
  - Your platform ($OS-$ARCH) isn't supported yet
  - Network issues

Check available releases at:
  https://github.com/${REPO}/releases"
    fi

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
        error "Binary not found in archive. Please report this issue at:
  https://github.com/${REPO}/issues"
    fi

    mv "$BINARY_PATH" "$INSTALLED_PATH"
    chmod +x "$INSTALLED_PATH"

    success "Installed ori to $INSTALLED_PATH"

    # Check if install directory is in PATH
    case ":$PATH:" in
        *":${INSTALL_DIR}:"*)
            echo ""
            success "Ori $VERSION is ready!"
            echo ""
            echo "  Run: ori --version"
            echo ""
            ;;
        *)
            echo ""
            printf "${YELLOW}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}\n"
            printf "${BOLD}Add Ori to your PATH${NC}\n"
            printf "${YELLOW}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}\n"
            echo ""
            echo "Add this to your shell config:"
            echo ""
            printf "  ${CYAN}export PATH=\"%s:\$PATH\"${NC}\n" "$INSTALL_DIR"
            echo ""
            echo "Shell config files:"
            echo "  bash:  ~/.bashrc or ~/.bash_profile"
            echo "  zsh:   ~/.zshrc"
            echo "  fish:  fish_add_path $INSTALL_DIR"
            echo ""
            echo "Then reload your shell or run:"
            echo ""
            printf "  ${CYAN}source ~/.bashrc${NC}  # or your shell's config\n"
            echo ""
            echo "Or run Ori directly now:"
            echo ""
            printf "  ${CYAN}%s --version${NC}\n" "$INSTALLED_PATH"
            echo ""
            ;;
    esac
}

main "$@"
