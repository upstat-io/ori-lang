#!/usr/bin/env bash
# Regression pins for setup.sh prerequisite verification.
#
# Each case runs setup.sh under an isolated HOME + PATH so no real toolchain,
# hook, or ~/.local/bin entry is touched.
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# SETUP_SH_UNDER_TEST lets a reverted-fix falsifier point the pins at a mutated
# copy; unset it targets the real script.
SETUP="${SETUP_SH_UNDER_TEST:-$REPO_ROOT/setup.sh}"
PASS=0
FAIL=0

ok()   { printf '[ok]   %s\n' "$1"; PASS=$((PASS + 1)); }
bad()  { printf '[FAIL] %s\n' "$1"; FAIL=$((FAIL + 1)); }

# Tools setup.sh needs merely to START. Omitting one makes every pin pass on a
# crash instead of on the behavior under test.
BASE_TOOLS=(bash sed awk grep uname mktemp rm cat printf tr cut head sort wc
            dirname basename ln mkdir chmod install env date)

# Real toolchain homes, resolved before any pin overrides HOME. setup.sh runs
# `rustc --version`; under an isolated empty HOME rustup has no toolchain and
# probes the network, which cost 18.7s of the pins' runtime. The pins assert
# about $fake_home/.local/bin and setup.sh's messages, never about rustup
# state, so resolving the toolchain locally removes egress without weakening
# any assertion.
#
# An INHERITED but non-resolving RUSTUP_HOME/CARGO_HOME must not be propagated:
# it would silently restore the bootstrap-over-network path. Fall back to the
# canonical locations whenever the inherited value does not exist, and let
# pin_no_external_network assert the resulting environment actually resolves a
# toolchain rather than merely carrying the right variable names.
# Existence is NOT sufficient: a poisoned RUSTUP_HOME that merely EXISTS (an
# empty dir rustup itself created on a prior run) passes a -d test while
# resolving no toolchain, silently restoring the network bootstrap. Validate by
# behavior -- keep the inherited value only when rustc actually resolves under
# it, else fall back to the canonical location.
_resolve_rustup_home() {
    local inherited="$1" canonical="$2"
    if [[ -n "$inherited" ]] \
       && RUSTUP_HOME="$inherited" rustc --version >/dev/null 2>&1; then
        printf '%s' "$inherited"
    else
        printf '%s' "$canonical"
    fi
}
_resolve_cargo_home() {
    local inherited="$1" canonical="$2"
    if [[ -n "$inherited" && -d "$inherited" ]]; then
        printf '%s' "$inherited"
    else
        printf '%s' "$canonical"
    fi
}
TOOLCHAIN_ENV=(
    "RUSTUP_HOME=$(_resolve_rustup_home "${RUSTUP_HOME:-}" "$HOME/.rustup")"
    "CARGO_HOME=$(_resolve_cargo_home "${CARGO_HOME:-}" "$HOME/.cargo")"
)

# Fixture-serving curl. Staged in place of the real binary so no pin performs
# external network I/O: egress made the pins take 45s-140s+ and exceed the
# 140s test-all action timeout, while asserting nothing about network behavior.
# Serves a release tag, an asset, and a checksum list that MATCHES that asset,
# so a pin with sha256sum present verifies and a pin without it fails closed.
# setup.sh matches a checksum row by the VERSIONED asset filename
# (lefthook_<version>_<os>_<arch>), never a bare "lefthook", so the shim records
# the asset basename it was asked for and names that exact file in the checksum
# list. Without this the success branch is unreachable: every verification would
# take "no checksum entry for $asset".
stage_curl_shim() {
    local bindir="$1"
    local asset_bytes="FAKE-LEFTHOOK-BINARY"
    local asset_sha
    asset_sha="$(printf '%s' "$asset_bytes" | sha256sum | cut -d' ' -f1)"
    cat > "$bindir/curl" <<EOF
#!/usr/bin/env bash
# Fixture curl. Serves a synthetic release tag, an asset, and a checksum list
# naming that asset, so both the verify-success and verify-refusal branches of
# setup.sh are reachable with zero network I/O.
seen="\$(dirname "\$0")/.last-asset"
out=""; url=""
while [ \$# -gt 0 ]; do
  case "\$1" in
    -o) out="\$2"; shift 2 ;;
    -*) shift ;;
    *)  url="\$1"; shift ;;
  esac
done
case "\$url" in
  *lefthook_checksums.txt)
      asset="\$(cat "\$seen" 2>/dev/null)"
      : "\${asset:=lefthook}"
      printf '%s  %s\n' '$asset_sha' "\$asset" > "\${out:-/dev/stdout}"; exit 0 ;;
  *releases/latest)
      printf '{"tag_name": "v0.0.0-hermetic-fixture"}' > "\${out:-/dev/stdout}"; exit 0 ;;
  *)
      basename "\$url" > "\$seen" 2>/dev/null || true
      printf '%s' '$asset_bytes' > "\${out:-/dev/stdout}"; exit 0 ;;
esac
EOF
    chmod +x "$bindir/curl"
}

# Build a PATH containing only BASE_TOOLS plus the named tools.
# A requested `curl` is served by the hermetic shim, never the real binary, so
# a pin cannot reintroduce egress by listing curl among its tools.
# Args: <bindir> <tool>...
stage_tools() {
    local bindir="$1"; shift
    mkdir -p "$bindir"
    local tool src
    for tool in "${BASE_TOOLS[@]}" "$@"; do
        if [[ "$tool" == "curl" ]]; then
            stage_curl_shim "$bindir"
            continue
        fi
        src="$(command -v "$tool" 2>/dev/null)" || continue
        ln -sf "$src" "$bindir/$tool"
    done
}

# Guard the guard: a staged PATH must be able to run setup.sh at all.
assert_harness_sane() {
    local bindir="$1" label="$2" out
    out="$(env "${TOOLCHAIN_ENV[@]}" HOME="$(mktemp -d)" PATH="$bindir" bash "$SETUP" --check 2>&1)"
    if grep -qE "command not found|null directory" <<<"$out"; then
        bad "$label: HARNESS BROKEN — staged PATH cannot run setup.sh"
        return 1
    fi
    return 0
}

# --- Pin 1: checksum unverifiable => REFUSE to install (fail closed) ---------
# Reverted-fix falsifier: the pre-fix script left verified="skipped" and
# installed anyway, so this case asserts non-installation, not just a message.
pin_checksum_fail_closed() {
    local tmp fake_home bindir out rc
    tmp="$(mktemp -d)"; fake_home="$tmp/home"; bindir="$tmp/bin"
    mkdir -p "$fake_home"
    # Everything setup.sh needs EXCEPT sha256sum, and without lefthook so the
    # install path is actually entered.
    stage_tools "$bindir" curl git cc gcc cargo rustc rustup
    assert_harness_sane "$bindir" "checksum-fail-closed" || { rm -rf "$tmp"; return; }

    out="$(env "${TOOLCHAIN_ENV[@]}" HOME="$fake_home" PATH="$bindir" bash "$SETUP" 2>&1)"; rc=$?

    if [[ -e "$fake_home/.local/bin/lefthook" ]]; then
        bad "checksum-fail-closed: lefthook WAS installed without a verified checksum"
    else
        ok "checksum-fail-closed: no lefthook binary installed"
    fi
    if [[ $rc -ne 0 ]]; then
        ok "checksum-fail-closed: exited non-zero ($rc)"
    else
        bad "checksum-fail-closed: exited 0 despite an unverifiable download"
    fi
    if grep -q "cannot be verified\|checksum\|sha256sum" <<<"$out"; then
        ok "checksum-fail-closed: names the verification failure"
    else
        bad "checksum-fail-closed: no verification-failure message"
    fi
    rm -rf "$tmp"
}

# --- Pin 1b: checksum LIST unfetchable while sha256sum present => REFUSE ----
# Exercises the branch pin 1 cannot reach: the download succeeds, sha256sum
# exists, and only the checksum list is unavailable.
pin_checksum_list_unfetchable() {
    local tmp fake_home bindir out rc
    tmp="$(mktemp -d)"; fake_home="$tmp/home"; bindir="$tmp/bin"
    mkdir -p "$fake_home"
    stage_tools "$bindir" git cc gcc cargo rustc rustup sha256sum

    # curl shim: serves the release-tag JSON and the asset, refuses the checksum list.
    cat > "$bindir/curl" <<'EOF'
#!/usr/bin/env bash
out=""; url=""
while [ $# -gt 0 ]; do
  case "$1" in
    -o) out="$2"; shift 2 ;;
    -*) shift ;;
    *)  url="$1"; shift ;;
  esac
done
case "$url" in
  *lefthook_checksums.txt) exit 22 ;;
  *releases/latest)        printf '{"tag_name": "v2.1.10"}' > "${out:-/dev/stdout}"; exit 0 ;;
  *)                       printf 'FAKE-BINARY' > "${out:-/dev/stdout}"; exit 0 ;;
esac
EOF
    chmod +x "$bindir/curl"
    assert_harness_sane "$bindir" "checksum-list-unfetchable" || { rm -rf "$tmp"; return; }

    out="$(env "${TOOLCHAIN_ENV[@]}" HOME="$fake_home" PATH="$bindir" bash "$SETUP" 2>&1)"; rc=$?

    if [[ -e "$fake_home/.local/bin/lefthook" ]]; then
        bad "checksum-list-unfetchable: installed an unverified binary"
    else
        ok "checksum-list-unfetchable: refused to install"
    fi
    if [[ $rc -ne 0 ]]; then
        ok "checksum-list-unfetchable: exited non-zero ($rc)"
    else
        bad "checksum-list-unfetchable: exited 0"
    fi
    rm -rf "$tmp"
}

# --- Pin 2: required prerequisite absent => exit 1 naming it ----------------
pin_missing_required_fails() {
    local tmp fake_home bindir out rc
    tmp="$(mktemp -d)"; fake_home="$tmp/home"; bindir="$tmp/bin"
    mkdir -p "$fake_home"
    # No cc: the linker prerequisite is required.
    stage_tools "$bindir" curl git sha256sum cargo rustc rustup
    assert_harness_sane "$bindir" "missing-required" || { rm -rf "$tmp"; return; }

    out="$(env "${TOOLCHAIN_ENV[@]}" HOME="$fake_home" PATH="$bindir" bash "$SETUP" --check 2>&1)"; rc=$?

    if [[ $rc -eq 1 ]]; then
        ok "missing-required: --check exited 1"
    else
        bad "missing-required: --check exited $rc, expected 1"
    fi
    if grep -q "cc" <<<"$out"; then
        ok "missing-required: names the missing tool"
    else
        bad "missing-required: does not name the missing tool"
    fi
    rm -rf "$tmp"
}

# --- Pin 3: --check makes no changes ---------------------------------------
pin_check_is_read_only() {
    local tmp fake_home bindir
    tmp="$(mktemp -d)"; fake_home="$tmp/home"; bindir="$tmp/bin"
    mkdir -p "$fake_home"
    stage_tools "$bindir" curl git sha256sum cc gcc cargo rustc rustup
    assert_harness_sane "$bindir" "check-read-only" || { rm -rf "$tmp"; return; }

    env "${TOOLCHAIN_ENV[@]}" HOME="$fake_home" PATH="$bindir" bash "$SETUP" --check >/dev/null 2>&1

    if [[ -e "$fake_home/.local/bin/lefthook" ]]; then
        bad "check-read-only: --check installed lefthook"
    else
        ok "check-read-only: --check installed nothing"
    fi
    rm -rf "$tmp"
}

# --- Pin 3b: --check mutates NOTHING in the repo itself ---------------------
# Runs against an isolated copy of the script + the two files it reads, then
# compares the fixture's full file listing before and after. Catches any
# repo-root write (e.g. .llvm-env.sh) that --check must not perform.
pin_check_does_not_touch_repo() {
    local tmp fixture bindir before after
    tmp="$(mktemp -d)"; fixture="$tmp/repo"; bindir="$tmp/bin"
    mkdir -p "$fixture/.cargo" "$tmp/home"
    cp "$SETUP" "$fixture/setup.sh"
    cp "$REPO_ROOT/rust-toolchain.toml" "$fixture/rust-toolchain.toml"
    cp "$REPO_ROOT/.cargo/config.toml" "$fixture/.cargo/config.toml"
    stage_tools "$bindir" curl git sha256sum cc gcc cargo rustc rustup

    before="$(cd "$fixture" && find . | sort)"
    env "${TOOLCHAIN_ENV[@]}" HOME="$tmp/home" PATH="$bindir" bash "$fixture/setup.sh" --check >/dev/null 2>&1
    after="$(cd "$fixture" && find . | sort)"

    if [[ "$before" == "$after" ]]; then
        ok "check-no-repo-writes: --check left the repo fixture unchanged"
    else
        bad "check-no-repo-writes: --check mutated the repo fixture:"
        diff <(printf '%s\n' "$before") <(printf '%s\n' "$after") | sed 's/^/         /'
    fi
    rm -rf "$tmp"
}

# --- Pin 4: version-qualified LLVM selection -------------------------------
# A decoy unversioned `llvm-config` reporting a different major must never be
# selected. setup.sh also probes absolute per-distro prefixes, so on a host with
# a real LLVM 21 the assertion is that 21 is chosen and the decoy is not.
pin_version_qualified_llvm() {
    local tmp bindir out
    tmp="$(mktemp -d)"; bindir="$tmp/bin"
    mkdir -p "$tmp/home" "$bindir"
    stage_tools "$bindir" curl git sha256sum cc gcc cargo rustc rustup
    assert_harness_sane "$bindir" "version-qualified-llvm" || { rm -rf "$tmp"; return; }

    cat > "$bindir/llvm-config" <<'EOF'
#!/usr/bin/env bash
case "$1" in
  --version) echo "22.1.0" ;;
  --prefix)  echo "/nonexistent/llvm22" ;;
esac
EOF
    chmod +x "$bindir/llvm-config"

    out="$(env "${TOOLCHAIN_ENV[@]}" HOME="$tmp/home" PATH="$bindir" bash "$SETUP" --check 2>&1 | sed 's/\x1b\[[0-9;]*m//g')"

    if grep -q "/nonexistent/llvm22" <<<"$out"; then
        bad "version-qualified-llvm: adopted the decoy major-22 toolchain"
    else
        ok "version-qualified-llvm: ignored the decoy major-22 llvm-config"
    fi
    if grep -qE "LLVM 21\.[0-9]+.* found|no llvm-config reporting LLVM 21" <<<"$out"; then
        ok "version-qualified-llvm: reported a definite LLVM-21 verdict"
    else
        bad "version-qualified-llvm: no LLVM-21 verdict in output"
    fi
    rm -rf "$tmp"
}

# --- Pin 5: the pins themselves make no external network call ---------------
# Regression pin for hermeticity. Real egress made these pins take 45s-140s+ and
# blow the 140s test-all action timeout, failing whole runs with
# infrastructure_failed. A staged `curl` MUST therefore be the fixture shim, never
# a symlink to the real binary, and the toolchain homes must resolve locally so
# `rustc --version` cannot make rustup bootstrap over the network.
pin_no_external_network() {
    local tmp bindir tag
    tmp="$(mktemp -d)"; bindir="$tmp/bin"
    stage_tools "$bindir" curl git sha256sum cc gcc cargo rustc rustup

    if [[ -L "$bindir/curl" ]]; then
        bad "no-external-network: staged curl is a symlink to the real binary"
    else
        ok "no-external-network: staged curl is the fixture shim, not the real binary"
    fi

    tag="$(PATH="$bindir" curl -fsSL https://api.github.com/repos/evilmartians/lefthook/releases/latest 2>/dev/null)"
    if [[ "$tag" == *'"tag_name": "v0.0.0-hermetic-fixture"'* ]]; then
        ok "no-external-network: shim serves the release tag from fixture bytes"
    else
        bad "no-external-network: staged curl did not serve fixture bytes (got: ${tag:0:60})"
    fi

    # BEHAVIORAL, not name-shaped: a variable-name check stays green under a
    # poisoned inherited RUSTUP_HOME/CARGO_HOME while rustc silently bootstraps
    # over the network. Assert the pinned environment actually resolves a
    # toolchain under an isolated HOME, which is the invariant that removes the
    # egress.
    if env "${TOOLCHAIN_ENV[@]}" HOME="$tmp/probe-home" PATH="$bindir" \
           rustc --version >/dev/null 2>&1; then
        ok "no-external-network: pinned toolchain homes resolve rustc under an isolated HOME"
    else
        bad "no-external-network: rustc does not resolve under the pinned toolchain homes (rustup would bootstrap over the network)"
    fi
    rm -rf "$tmp"
}

# --- Pin 6: verified checksum => INSTALL (the success branch) ---------------
# Complement to pins 1/1b, which only cover refusal. Nothing previously
# exercised setup.sh's verify-success path, so a regression that refused a
# VALID download would have gone unnoticed. Reachable only because the fixture
# checksum list names the versioned asset setup.sh actually requests.
pin_verified_checksum_installs() {
    local tmp fake_home bindir out rc
    tmp="$(mktemp -d)"; fake_home="$tmp/home"; bindir="$tmp/bin"
    mkdir -p "$fake_home"
    stage_tools "$bindir" curl git cc gcc cargo rustc rustup sha256sum
    assert_harness_sane "$bindir" "verified-checksum-installs" || { rm -rf "$tmp"; return; }

    out="$(env "${TOOLCHAIN_ENV[@]}" HOME="$fake_home" PATH="$bindir" bash "$SETUP" 2>&1)"; rc=$?

    if [[ -e "$fake_home/.local/bin/lefthook" ]]; then
        ok "verified-checksum-installs: installed lefthook after verifying the checksum"
    else
        bad "verified-checksum-installs: refused a VALID download (rc=$rc): $(grep -o 'lefthook NOT installed.*' <<<"$out" | head -1)"
    fi
    if grep -q "checksum verified" <<<"$out"; then
        ok "verified-checksum-installs: names the successful verification"
    else
        bad "verified-checksum-installs: no verification-success message"
    fi
    rm -rf "$tmp"
}

pin_checksum_fail_closed
pin_checksum_list_unfetchable
pin_missing_required_fails
pin_check_is_read_only
pin_check_does_not_touch_repo
pin_version_qualified_llvm
pin_verified_checksum_installs
pin_no_external_network

printf '\n%d passed, %d failed\n' "$PASS" "$FAIL"
[[ $FAIL -eq 0 ]]
