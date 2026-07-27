#!/usr/bin/env bash
# Falsifier for sync-version.sh's standalone-manifest omission detector.
#
# The detector enumerates standalone Cargo manifests from DISK and fails when one
# is absent from its handled list. A detector that cannot fail is not a detector,
# so this asserts BOTH directions: a fully-handled tree exits 0 with no MISSING
# line, and adding one unhandled standalone manifest exits nonzero naming it.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

mkdir -p \
    "$WORK/scripts" \
    "$WORK/compiler/ori_llvm" \
    "$WORK/compiler/ori_rt" \
    "$WORK/tools/ori-lsp" \
    "$WORK/tools/ori-rc-remarks" \
    "$WORK/editors/vscode-ori"

# sync-version.sh derives ROOT_DIR from its own location, so the copy makes the
# synthetic tree the root it enumerates.
cp "$ROOT/scripts/sync-version.sh" "$WORK/scripts/"

echo "2026.07.27.1-alpha" > "$WORK/BUILD_NUMBER"
printf '[workspace.package]\nversion = "2026.7.27-alpha.1"\n' > "$WORK/Cargo.toml"
printf '{"version":"2026.7.27"}\n' > "$WORK/editors/vscode-ori/package.json"

# Every standalone manifest the script's handled list names. A standalone manifest
# is one carrying [package] WITHOUT version.workspace, which is what the detector
# scans for.
for pkg in compiler/ori_llvm compiler/ori_rt tools/ori-lsp tools/ori-rc-remarks; do
    printf '[package]\nname = "%s"\nversion = "2026.7.27-alpha.1"\n' \
        "$(basename "$pkg")" > "$WORK/$pkg/Cargo.toml"
done

# --- positive: fully-handled tree is clean -----------------------------------
positive_status=0
positive_output="$(bash "$WORK/scripts/sync-version.sh" --check 2>&1)" || positive_status=$?

if [[ $positive_status -ne 0 ]]; then
    echo "FAIL: fully-handled tree should exit 0, got $positive_status" >&2
    echo "$positive_output" >&2
    exit 1
fi
if grep -q "MISSING:" <<<"$positive_output"; then
    echo "FAIL: fully-handled tree reported MISSING" >&2
    echo "$positive_output" >&2
    exit 1
fi
if ! grep -q "standalone coverage: all on-disk standalone manifests are handled" \
        <<<"$positive_output"; then
    echo "FAIL: coverage check did not report its PASS verdict" >&2
    echo "$positive_output" >&2
    exit 1
fi

# --- negative: one unhandled standalone manifest must be caught --------------
# A silent no-op here is the defect this test exists for: a new standalone crate
# drifting out of version sync with nothing reporting it.
mkdir -p "$WORK/tools/ori-unhandled"
printf '[package]\nname = "ori-unhandled"\nversion = "2026.7.27-alpha.1"\n' \
    > "$WORK/tools/ori-unhandled/Cargo.toml"

negative_status=0
negative_output="$(bash "$WORK/scripts/sync-version.sh" --check 2>&1)" || negative_status=$?

if [[ $negative_status -eq 0 ]]; then
    echo "FAIL: unhandled standalone manifest did not fail the check" >&2
    echo "$negative_output" >&2
    exit 1
fi
if ! grep -q "MISSING: tools/ori-unhandled/Cargo.toml is a standalone manifest not covered" \
        <<<"$negative_output"; then
    echo "FAIL: detector did not name the unhandled manifest" >&2
    echo "$negative_output" >&2
    exit 1
fi

# A standalone manifest under a whitespace-bearing path must be enumerated and
# named, not silently dropped from the scan.
mkdir -p "$WORK/tools/ori spaced"
printf '[package]\nname = "ori-spaced"\nversion = "2026.7.27-alpha.1"\n' \
    > "$WORK/tools/ori spaced/Cargo.toml"

spaced_status=0
spaced_output="$(bash "$WORK/scripts/sync-version.sh" --check 2>&1)" || spaced_status=$?

if [[ $spaced_status -eq 0 ]]; then
    echo "FAIL: standalone manifest under a space-bearing path did not fail the check" >&2
    echo "$spaced_output" >&2
    exit 1
fi
if ! grep -q "MISSING: tools/ori spaced/Cargo.toml is a standalone manifest not covered" \
        <<<"$spaced_output"; then
    echo "FAIL: detector did not name the space-bearing manifest" >&2
    echo "$spaced_output" >&2
    exit 1
fi
rm -rf "$WORK/tools/ori spaced"

# A workspace-versioned manifest is NOT standalone and must not be reported.
mkdir -p "$WORK/compiler/ori_member"
printf '[package]\nname = "ori-member"\nversion.workspace = true\n' \
    > "$WORK/compiler/ori_member/Cargo.toml"

boundary_output="$(bash "$WORK/scripts/sync-version.sh" --check 2>&1)" || true
if grep -q "MISSING: compiler/ori_member/Cargo.toml" <<<"$boundary_output"; then
    echo "FAIL: workspace-versioned manifest wrongly reported as standalone" >&2
    echo "$boundary_output" >&2
    exit 1
fi

# --- single-set: one array drives BOTH synchronization and coverage ------------
# A path added to the single set must be REACHED by the version check, so a
# manifest cannot be covered without also being synchronized.
sed -i 's#^    "tools/ori-rc-remarks/Cargo.toml"#    "tools/ori-rc-remarks/Cargo.toml"\n    "tools/ori-fifth/Cargo.toml"#' \
    "$WORK/scripts/sync-version.sh"

if ! grep -q '"tools/ori-fifth/Cargo.toml"' "$WORK/scripts/sync-version.sh"; then
    echo "FAIL: could not extend STANDALONE_MANIFESTS -- the single-set shape changed" >&2
    exit 1
fi
if [[ $(grep -c '"tools/ori-fifth/Cargo.toml"' "$WORK/scripts/sync-version.sh") -ne 1 ]]; then
    echo "FAIL: the manifest set is declared more than once -- divergence is representable" >&2
    exit 1
fi

mkdir -p "$WORK/tools/ori-fifth"
printf '[package]\nname = "ori-fifth"\nversion = "1.0.0-stale"\n' \
    > "$WORK/tools/ori-fifth/Cargo.toml"

single_set_status=0
single_set_output="$(bash "$WORK/scripts/sync-version.sh" --check 2>&1)" || single_set_status=$?

if [[ $single_set_status -eq 0 ]]; then
    echo "FAIL: a stale manifest in the single set was not reached by the version check" >&2
    echo "$single_set_output" >&2
    exit 1
fi
if ! grep -q "tools/ori-fifth/Cargo.toml has version '1.0.0-stale'" <<<"$single_set_output"; then
    echo "FAIL: version check did not report the stale manifest from the single set" >&2
    echo "$single_set_output" >&2
    exit 1
fi
if grep -q "MISSING: tools/ori-fifth/Cargo.toml" <<<"$single_set_output"; then
    echo "FAIL: a manifest in the single set was also reported as uncovered" >&2
    echo "$single_set_output" >&2
    exit 1
fi

echo "PASS: standalone-coverage detector reports clean, catches an omission, names a"
echo "      space-bearing path, does not misclassify a workspace-versioned manifest,"
echo "      and one manifest set drives both synchronization and coverage"
