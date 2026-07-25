#!/usr/bin/env bash
# Developer setup script for Ori compiler.
# Verifies every prerequisite the build and test suites need, resolves the
# LLVM 21 prefix for this distribution, and installs the git hooks.
#
# Usage:
#   ./setup.sh            verify prerequisites, install git hooks
#   ./setup.sh --check    verify only; make no changes
#
# Exit codes: 0 = every required prerequisite present; 1 = one or more missing.
set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LLVM_MAJOR=21
LLVM_ENV_FILE="$REPO_ROOT/.llvm-env.sh"
LEFTHOOK_FALLBACK_VERSION="2.1.10"

CHECK_ONLY=0
for arg in "$@"; do
    case "$arg" in
        --check) CHECK_ONLY=1 ;;
        -h|--help) sed -n '2,10p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) printf "${RED}unknown argument: %s${NC}\n" "$arg" >&2; exit 2 ;;
    esac
done

info() { printf "${GREEN}[ok]${NC}   %s\n" "$1"; }
warn() { printf "${YELLOW}[warn]${NC} %s\n" "$1"; }
miss() { printf "${RED}[fail]${NC} %s\n" "$1"; }
step() { printf "\n${BLUE}==>${NC} %s\n" "$1"; }

MISSING_REQUIRED=()
MISSING_OPTIONAL=()

# Emit the install command for this host. Args: <dnf-pkg> <apt-pkg> <other-pkg>
install_hint() {
    if command -v dnf &>/dev/null; then
        printf "sudo dnf install -y %s" "$1"
    elif command -v apt-get &>/dev/null; then
        printf "sudo apt install -y %s" "$2"
    elif command -v pacman &>/dev/null; then
        printf "sudo pacman -S %s" "$3"
    elif command -v brew &>/dev/null; then
        printf "brew install %s" "$3"
    else
        printf "install %s with your system package manager" "$2"
    fi
}

# Args: <command> <required|optional> <purpose> <dnf-pkg> <apt-pkg> <other-pkg>
check_cmd() {
    local cmd="$1" tier="$2" purpose="$3"
    if command -v "$cmd" &>/dev/null; then
        info "$cmd found — $purpose"
        return 0
    fi
    local hint
    hint="$(install_hint "$4" "$5" "$6")"
    if [[ "$tier" == "required" ]]; then
        miss "$cmd NOT found — required for $purpose. Fix: $hint"
        MISSING_REQUIRED+=("$cmd")
    else
        warn "$cmd not found — $purpose unavailable. Fix: $hint"
        MISSING_OPTIONAL+=("$cmd")
    fi
}

step "Rust toolchain"

if command -v cargo &>/dev/null; then
    info "Rust found: $(rustc --version 2>/dev/null || echo 'rustc unavailable')"
    pinned="$(sed -n 's/^channel[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' \
        "$REPO_ROOT/rust-toolchain.toml" 2>/dev/null | head -1)"
    if [[ -n "$pinned" ]]; then
        if command -v rustup &>/dev/null; then
            info "toolchain pin $pinned honored by rustup (rust-toolchain.toml)"
        else
            warn "rust-toolchain.toml pins $pinned but rustup is absent; the pin cannot be applied"
        fi
    fi
else
    miss "Rust NOT found — required to build the compiler. Fix: install from https://rustup.rs"
    MISSING_REQUIRED+=("cargo")
fi

step "Build prerequisites"

check_cmd git       required "cloning and the git hooks"       git                  git                    git
check_cmd cc        required "linking Rust binaries"          gcc                  build-essential        gcc
check_cmd curl      required "downloading the git-hook runner" curl                curl                   curl
check_cmd sha256sum required "verifying downloaded binaries"   coreutils           coreutils              coreutils
check_cmd valgrind optional "the valgrind leak suite"         valgrind             valgrind               valgrind
check_cmd cmake    optional "building the alive2 verifier"    cmake                cmake                  cmake
check_cmd ninja    optional "building the alive2 verifier"    ninja-build          ninja-build            ninja
check_cmd z3       optional "alive2 translation validation"   z3                   z3                     z3

step "LLVM $LLVM_MAJOR"

# ori_llvm pins llvm-sys 211 / inkwell llvm21-1, so the LLVM tools must report
# major version 21. Distributions disagree on binary name and prefix, and a
# bare `llvm-config` / `clang` is frequently a different major version, so
# probe version-qualified names first and accept an unversioned name only when
# it reports the required major itself.
# Args: <candidate>... ; echoes the first path whose --version starts with $LLVM_MAJOR.
resolve_versioned() {
    local candidate resolved
    for candidate in "$@"; do
        resolved=""
        if [[ "$candidate" == /* ]]; then
            [[ -x "$candidate" ]] && resolved="$candidate"
        else
            resolved="$(command -v "$candidate" 2>/dev/null || true)"
        fi
        [[ -z "$resolved" ]] && continue
        if "$resolved" --version 2>/dev/null | grep -qE "(^|[^0-9.])${LLVM_MAJOR}\.[0-9]"; then
            printf '%s' "$resolved"
            return 0
        fi
    done
    return 1
}

llvm_config="$(resolve_versioned \
    "llvm-config-${LLVM_MAJOR}" \
    "llvm-config${LLVM_MAJOR}" \
    "/usr/lib64/llvm${LLVM_MAJOR}/bin/llvm-config" \
    "/usr/lib/llvm${LLVM_MAJOR}/bin/llvm-config" \
    "/usr/lib/llvm-${LLVM_MAJOR}/bin/llvm-config" \
    "/usr/local/opt/llvm@${LLVM_MAJOR}/bin/llvm-config" \
    "/opt/homebrew/opt/llvm@${LLVM_MAJOR}/bin/llvm-config" \
    "llvm-config" || true)"

llvm_prefix=""
if [[ -n "$llvm_config" ]]; then
    llvm_prefix="$("$llvm_config" --prefix 2>/dev/null || true)"
    info "LLVM $("$llvm_config" --version) found: $llvm_config"
    info "LLVM prefix: ${llvm_prefix:-unresolved}"
else
    miss "no llvm-config reporting LLVM $LLVM_MAJOR — required by ori_llvm (llvm-sys 211). Fix: $(install_hint "llvm${LLVM_MAJOR}-devel" "llvm-${LLVM_MAJOR}-dev" "llvm@${LLVM_MAJOR}")"
    MISSING_REQUIRED+=("llvm-config-${LLVM_MAJOR}")
fi

# .cargo/config.toml carries one default LLVM_SYS_211_PREFIX. Its [env] block
# does not set force=true, so a real environment variable takes precedence;
# emit a sourceable override whenever this host's prefix differs.
if [[ -n "$llvm_prefix" ]]; then
    config_prefix="$(sed -n 's/^LLVM_SYS_211_PREFIX[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' \
        "$REPO_ROOT/.cargo/config.toml" 2>/dev/null | head -1)"
    if [[ "$config_prefix" == "$llvm_prefix" ]]; then
        info "LLVM_SYS_211_PREFIX default matches this host"
    elif [[ "$CHECK_ONLY" == "1" ]]; then
        warn "config default (${config_prefix:-unset}) is not this host's prefix"
        printf "         --check makes no changes; a normal run would write %s\n" "$LLVM_ENV_FILE"
        printf "         containing: ${GREEN}export LLVM_SYS_211_PREFIX=\"%s\"${NC}\n" "$llvm_prefix"
    else
        cat > "$LLVM_ENV_FILE" <<EOF
# Generated by setup.sh — LLVM $LLVM_MAJOR prefix for this host.
# The .cargo/config.toml default is "${config_prefix:-unset}", which is not
# this host's layout. Cargo's [env] is unforced, so this export wins.
export LLVM_SYS_211_PREFIX="$llvm_prefix"
EOF
        warn "config default (${config_prefix:-unset}) is not this host's prefix; wrote $LLVM_ENV_FILE"
        printf "         before building run: ${GREEN}source %s${NC}\n" "$LLVM_ENV_FILE"
        printf "         or persist it:       ${GREEN}export LLVM_SYS_211_PREFIX=\"%s\"${NC}\n" "$llvm_prefix"
    fi
fi

clang_bin="$(resolve_versioned \
    "clang-${LLVM_MAJOR}" \
    "clang${LLVM_MAJOR}" \
    "${llvm_prefix:-/nonexistent}/bin/clang" \
    "clang" || true)"
if [[ -n "$clang_bin" ]]; then
    info "clang $LLVM_MAJOR found: $clang_bin"
else
    warn "no clang reporting version $LLVM_MAJOR — the AOT and IR suites are unavailable. Fix: $(install_hint "clang${LLVM_MAJOR}" "clang-${LLVM_MAJOR}" "llvm@${LLVM_MAJOR}")"
    MISSING_OPTIONAL+=("clang-${LLVM_MAJOR}")
fi

step "Git hooks (lefthook)"

if command -v lefthook &>/dev/null; then
    info "lefthook found: $(lefthook version)"
elif [[ "$CHECK_ONLY" == "1" ]]; then
    warn "lefthook not installed (--check makes no changes)"
    MISSING_OPTIONAL+=("lefthook")
else
    warn "lefthook not found, installing..."
    mkdir -p ~/.local/bin

    case "$(uname -s)-$(uname -m)" in
        Linux-x86_64)   asset_arch="Linux_x86_64" ;;
        Linux-aarch64)  asset_arch="Linux_arm64" ;;
        Darwin-x86_64)  asset_arch="macOS_x86_64" ;;
        Darwin-arm64)   asset_arch="macOS_arm64" ;;
        *)
            asset_arch=""
            miss "unsupported platform $(uname -s)-$(uname -m); install lefthook manually: https://github.com/evilmartians/lefthook"
            MISSING_REQUIRED+=("lefthook")
            ;;
    esac

    if [[ -n "$asset_arch" ]]; then
        # Release assets are version-qualified (lefthook_<version>_<os>_<arch>);
        # an unversioned /releases/latest/download/ URL 404s.
        lh_version="$(curl -fsSL https://api.github.com/repos/evilmartians/lefthook/releases/latest 2>/dev/null \
            | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"v\{0,1\}\([^"]*\)".*/\1/p' | head -1)"
        if [[ -z "$lh_version" ]]; then
            warn "could not resolve the latest lefthook version; using $LEFTHOOK_FALLBACK_VERSION"
            lh_version="$LEFTHOOK_FALLBACK_VERSION"
        fi

        base_url="https://github.com/evilmartians/lefthook/releases/download/v${lh_version}"
        asset="lefthook_${lh_version}_${asset_arch}"
        tmp_dir="$(mktemp -d)"
        trap 'rm -rf "$tmp_dir"' EXIT

        # Fail closed: install ONLY on a verified checksum. An unfetchable
        # checksum list, a missing sha256sum, or an absent entry for this asset
        # are all REFUSALS -- an unverifiable download is never installed.
        verify_fail=""
        if ! curl -fsSL "$base_url/$asset" -o "$tmp_dir/lefthook"; then
            verify_fail="download failed: $base_url/$asset"
        elif ! command -v sha256sum &>/dev/null; then
            verify_fail="sha256sum not found, so the download cannot be verified"
        elif ! curl -fsSL "$base_url/lefthook_checksums.txt" -o "$tmp_dir/sums.txt"; then
            verify_fail="checksum list unavailable at $base_url/lefthook_checksums.txt"
        else
            expected="$(awk -v a="$asset" '$2 == a || $2 == "*"a {print $1}' "$tmp_dir/sums.txt" | head -1)"
            actual="$(sha256sum "$tmp_dir/lefthook" | awk '{print $1}')"
            if [[ -z "$expected" ]]; then
                verify_fail="no checksum entry for $asset"
            elif [[ "$expected" != "$actual" ]]; then
                verify_fail="checksum mismatch for $asset (expected $expected, got $actual)"
            fi
        fi

        if [[ -n "$verify_fail" ]]; then
            miss "lefthook NOT installed — $verify_fail"
            printf "         install it manually: https://github.com/evilmartians/lefthook\n"
            MISSING_REQUIRED+=("lefthook")
        else
            info "lefthook checksum verified"
            install -m 0755 "$tmp_dir/lefthook" ~/.local/bin/lefthook
            info "lefthook $lh_version installed to ~/.local/bin"
            if [[ ":$PATH:" != *":$HOME/.local/bin:"* ]]; then
                warn "~/.local/bin is not on PATH; add it to your shell profile"
                export PATH="$HOME/.local/bin:$PATH"
            fi
        fi

        rm -rf "$tmp_dir"
        trap - EXIT
    fi
fi

if [[ "$CHECK_ONLY" != "1" ]] && command -v lefthook &>/dev/null; then
    if (cd "$REPO_ROOT" && lefthook install >/dev/null); then
        info "git hooks installed"
    else
        miss "lefthook install failed"
        MISSING_REQUIRED+=("git hooks")
    fi
fi

step "Summary"

if (( ${#MISSING_OPTIONAL[@]} > 0 )); then
    warn "optional tooling absent: ${MISSING_OPTIONAL[*]}"
fi

if (( ${#MISSING_REQUIRED[@]} > 0 )); then
    miss "missing required prerequisites: ${MISSING_REQUIRED[*]}"
    printf "\nResolve every [fail] line above, then re-run ./setup.sh\n"
    exit 1
fi

printf "\n${GREEN}Setup complete.${NC}\n"
if [[ -f "$LLVM_ENV_FILE" ]]; then
    printf "Run ${GREEN}source %s${NC} before building.\n" "$LLVM_ENV_FILE"
fi
printf "Verify with ${GREEN}./test-all.sh${NC}\n"
