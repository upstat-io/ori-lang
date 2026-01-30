#!/usr/bin/env bash
# Developer setup script for Ori compiler
set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

info() { printf "${GREEN}✓${NC} %s\n" "$1"; }
warn() { printf "${YELLOW}!${NC} %s\n" "$1"; }
error() { printf "${RED}✗${NC} %s\n" "$1"; exit 1; }

echo "Setting up Ori development environment..."
echo ""

# Check for Rust
if command -v cargo &>/dev/null; then
    info "Rust found: $(rustc --version)"
else
    error "Rust not found. Install from https://rustup.rs"
fi

# Install lefthook for git hooks
if command -v lefthook &>/dev/null; then
    info "Lefthook found: $(lefthook version)"
else
    warn "Lefthook not found, installing..."

    mkdir -p ~/.local/bin

    OS=$(uname -s)
    ARCH=$(uname -m)

    case "$OS-$ARCH" in
        Linux-x86_64)   BINARY="lefthook_Linux_x86_64" ;;
        Linux-aarch64)  BINARY="lefthook_Linux_arm64" ;;
        Darwin-x86_64)  BINARY="lefthook_macOS_x86_64" ;;
        Darwin-arm64)   BINARY="lefthook_macOS_arm64" ;;
        *) error "Unsupported platform: $OS-$ARCH. Install lefthook manually: https://github.com/evilmartians/lefthook" ;;
    esac

    curl -fsSL "https://github.com/evilmartians/lefthook/releases/latest/download/$BINARY" -o ~/.local/bin/lefthook
    chmod +x ~/.local/bin/lefthook

    if [[ ":$PATH:" != *":$HOME/.local/bin:"* ]]; then
        warn "Add ~/.local/bin to your PATH"
        export PATH="$HOME/.local/bin:$PATH"
    fi

    info "Lefthook installed"
fi

# Install git hooks
lefthook install
info "Git hooks installed"

echo ""
echo "Setup complete! Run ./test-all to verify everything works."
