#!/usr/bin/env bash
# INVARIANT: test-all.sh's Phase-1 serial prebuild must warm the EXACT cargo
# selection each Phase-2 leg runs (a mismatched shape recompiles under
# Cargo's feature-unification at run time, racing the shared target/ dir with
# the other legs — the rmeta "os error 2" race that collapses the aot leg).
# Each leg's selection is the single readonly array legs.sh defines,
# dereferenced by BOTH the leg runner and the serial `prebuild_leg_shapes`
# warm; the AOT harness's own per-process builds (`cargo build -p ori_rt` /
# `-p oric --bin ori`) are Phase-2 writers too and must be warmed with those
# exact single-package shapes, never a joint `-p oric -p ori_rt` variant.
#
# Pins:
#  (a) SSOT PIN — legs.sh defines the four per-leg selection arrays with the
#      exact selections the legs run today (workspace / rust_rt / rust_llvm /
#      aot). Pre-cure the arrays do not exist -> fails.
#  (b) BEHAVIORAL — prebuild_leg_shapes warms every leg selection via
#      `cargo test --no-run -q <sel>` (stubbed cargo captures argc+argv;
#      EXACT-LINE match so extra/missing/split argv cannot slip through),
#      cargo-test path (NEXTEST_ACTIVE unset).
#  (c) BEHAVIORAL — with NEXTEST_ACTIVE set, prebuild_leg_shapes ALSO warms
#      each selection's nextest harness binaries via
#      `cargo nextest run --no-run <sel>` (exact-line match).
#  (d) DEREFERENCE PIN — every run_<leg> body and the prebuild_leg_shapes
#      body textually dereference their SSOT array (`${LEG_SEL_*[@]}`), so
#      warm and run consume the SAME definition (arrays stay readonly; a
#      mutation-based sentinel would violate the readonly contract).
#  (e) NEGATIVE PIN — selection arrays carry ONLY cargo selection args: no
#      run-phase flags (--no-fail-fast / --color) and no --no-run; the warm
#      argv carries --no-run and NEVER --no-fail-fast (cargo-nextest rejects
#      the combination).
#  (f) NEGATIVE PIN — the warm is env-clean: prebuild_leg_shapes never
#      exports the aot leg's runtime verification env (run-only, not a build
#      input; the gated env is a locked verdict surface).
#  (g) INTEGRATION SHAPE — the PARALLEL block of test-all.sh (extracted
#      between the `$PARALLEL -eq 1` branch open and the sequential-else)
#      invokes prebuild_leg_shapes GUARDED (`if ! prebuild_leg_shapes`,
#      test-all.sh runs `set -e`); no wide `--no-run` workspace warm without
#      `--exclude` survives in it (orderless check); the harness-exact
#      single-package builds (`-p ori_rt` / `-p oric --bin ori`, debug +
#      release) are present IN THE PARALLEL BLOCK on non-comment lines (a
#      sequential-block or comment mention cannot satisfy this).
#  (h) POSITIVE ENV PIN — run_aot exports the gated verification env
#      (ORI_DISABLE_PREDICATE_STACK_RC=1 ORI_VERIFY_ARC=1 ORI_VERIFY_EACH=1)
#      around its rust_test_leg call, AND the exports stay subshell-scoped
#      (the caller's environment is unchanged after run_aot returns).
#  (i) FAIL-SEMANTICS PIN — a failing warm makes prebuild_leg_shapes return
#      non-zero WITHOUT aborting a `set -e` caller that guards it with
#      `if !`; a SINGLE failing warm (mid-sequence) still lets every later
#      warm run (continue-on-fail per leg, mirroring the wide-warm behavior)
#      and still returns non-zero.
#  (j) SERIAL PIN — prebuild_leg_shapes runs its warms serially: no `&`
#      backgrounding inside the function body (a parallelized warm loop
#      re-introduces the very concurrent-compile race the warm exists to
#      prevent).
#  (k) INTEGRATION SHAPE (sequential) — the SEQUENTIAL (`else`) branch of the
#      same `$PARALLEL -eq 1` compound carries the identical four
#      harness-exact single-package builds (debug + release, `-p ori_rt` /
#      `-p oric --bin ori`) and never a joint `-p oric -p ori_rt` build; the
#      harness's own later per-process builds (run_aot / run_ori_llvm) still
#      recompile against a feature-unification mismatch even with no
#      concurrent dispatch, so the (g) invariant binds this branch too.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LEG_HELPERS="$HERE/../test_all/legs.sh"
TEST_ALL="$HERE/../../test-all.sh"

if [ ! -f "$LEG_HELPERS" ]; then
    echo "FAIL: leg helper file not found at $LEG_HELPERS"
    exit 1
fi
if [ ! -f "$TEST_ALL" ]; then
    echo "FAIL: test-all.sh not found at $TEST_ALL"
    exit 1
fi

# Source the real production helpers under test directly (no sed-extraction:
# legs.sh defines only functions/globals with no top-level side effects, so
# sourcing is faithful to production).
# shellcheck source=/dev/null
. "$LEG_HELPERS"

EXPECTED_WORKSPACE='--workspace --exclude ori_llvm --lib --bins --tests'
EXPECTED_RUST_RT='-p ori_rt'
EXPECTED_RUST_LLVM='-p ori_llvm --lib'
EXPECTED_AOT='-p ori_llvm --test aot'

# (a) SSOT PIN — the four selection arrays exist with the exact leg selections
for pair in \
    "LEG_SEL_WORKSPACE:$EXPECTED_WORKSPACE" \
    "LEG_SEL_RUST_RT:$EXPECTED_RUST_RT" \
    "LEG_SEL_RUST_LLVM:$EXPECTED_RUST_LLVM" \
    "LEG_SEL_AOT:$EXPECTED_AOT"; do
    name="${pair%%:*}"; want="${pair#*:}"
    decl="$(declare -p "$name" 2>/dev/null)" || {
        echo "FAIL: (a) $name not defined in legs.sh (per-leg selection SSOT array missing)"
        exit 1
    }
    case "$decl" in
        "declare -"*r*" $name="*) ;;
        *)
            echo "FAIL: (a) $name is not readonly (declare shows '$decl'; the selection SSOT must carry -r so runners cannot mutate it)"
            exit 1
            ;;
    esac
    got="$(eval "printf '%s ' \"\${${name}[@]}\"")"; got="${got% }"
    if [ "$got" != "$want" ]; then
        echo "FAIL: (a) $name = '$got' (expected '$want')"
        exit 1
    fi
done
echo "OK: (a) four per-leg selection arrays defined with the exact leg selections"

if ! declare -F prebuild_leg_shapes >/dev/null 2>&1; then
    echo "FAIL: (b) prebuild_leg_shapes not defined in legs.sh"
    exit 1
fi

# (d) DEREFERENCE PIN — runners + warm textually consume the SSOT arrays
for pair in \
    "run_rust_workspace:LEG_SEL_WORKSPACE" \
    "run_rust_rt:LEG_SEL_RUST_RT" \
    "run_rust_llvm:LEG_SEL_RUST_LLVM" \
    "run_aot:LEG_SEL_AOT"; do
    fn="${pair%%:*}"; arr="${pair#*:}"
    if ! declare -f "$fn" | grep -qF "\${${arr}[@]}"; then
        echo "FAIL: (d) $fn does not dereference \${${arr}[@]} (runner not consuming the SSOT array)"
        exit 1
    fi
done
for arr in LEG_SEL_WORKSPACE LEG_SEL_RUST_RT LEG_SEL_RUST_LLVM LEG_SEL_AOT; do
    if ! declare -f prebuild_leg_shapes | grep -qF "$arr"; then
        echo "FAIL: (d) prebuild_leg_shapes does not reference $arr (warm not consuming the SSOT array)"
        exit 1
    fi
done
echo "OK: (d) every runner + the warm dereference their SSOT selection arrays"

# Stub cargo to capture warm argc+argv (argc pins element boundaries; a
# space-embedding split would change the count).
CARGO_CAPTURE="$(mktemp)"
# shellcheck disable=SC2317 # invoked indirectly: prebuild_leg_shapes -> cargo_race_retry "$@"
cargo() { printf '%s %s\n' "$#" "$*" >> "$CARGO_CAPTURE"; return 0; }

# (b) cargo-test path warms every selection with --no-run — EXACT-LINE match
: > "$CARGO_CAPTURE"
NEXTEST_ACTIVE="" prebuild_leg_shapes >/dev/null 2>&1 || {
    echo "FAIL: (b) prebuild_leg_shapes returned non-zero with all warms succeeding"
    exit 1
}
# argc = 3 fixed args (test --no-run -q) + selection word count
for spec in \
    "9 $EXPECTED_WORKSPACE" \
    "5 $EXPECTED_RUST_RT" \
    "6 $EXPECTED_RUST_LLVM" \
    "7 $EXPECTED_AOT"; do
    n="${spec%% *}"; want="${spec#* }"
    if ! grep -qxF "$n test --no-run -q $want" "$CARGO_CAPTURE"; then
        echo "FAIL: (b) cargo-test warm not an exact argc+argv match for '$want' (want argc=$n); captured:"
        cat "$CARGO_CAPTURE"
        exit 1
    fi
done
if grep -q "nextest" "$CARGO_CAPTURE"; then
    echo "FAIL: (b) nextest warm ran with NEXTEST_ACTIVE unset"
    exit 1
fi
echo "OK: (b) prebuild_leg_shapes warms all four exact selections via cargo test --no-run (exact argc+argv)"

# (c) nextest path additionally warms each selection's harness binaries
: > "$CARGO_CAPTURE"
NEXTEST_ACTIVE=1 prebuild_leg_shapes >/dev/null 2>&1 || {
    echo "FAIL: (c) prebuild_leg_shapes returned non-zero on the nextest path"
    exit 1
}
for spec in \
    "9 $EXPECTED_WORKSPACE" \
    "5 $EXPECTED_RUST_RT" \
    "6 $EXPECTED_RUST_LLVM" \
    "7 $EXPECTED_AOT"; do
    n="${spec%% *}"; want="${spec#* }"
    if ! grep -qxF "$n nextest run --no-run $want" "$CARGO_CAPTURE"; then
        echo "FAIL: (c) nextest warm not an exact argc+argv match for '$want' (want argc=$n); captured:"
        cat "$CARGO_CAPTURE"
        exit 1
    fi
done
echo "OK: (c) NEXTEST_ACTIVE warms nextest harness binaries for all four selections (exact argc+argv)"

# (e) selection arrays carry ONLY selection args; warm argv is run-flag-free
for name in LEG_SEL_WORKSPACE LEG_SEL_RUST_RT LEG_SEL_RUST_LLVM LEG_SEL_AOT; do
    vals="$(eval "printf '%s ' \"\${${name}[@]}\"")"
    case "$vals" in
        *--no-fail-fast*|*--no-run*|*--color*)
            echo "FAIL: (e) $name carries a run-phase/build-phase flag: '$vals' (selection args only)"
            exit 1
            ;;
    esac
done
if grep -q -- "--no-fail-fast" "$CARGO_CAPTURE"; then
    echo "FAIL: (e) warm argv carries --no-fail-fast (cargo-nextest rejects it with --no-run)"
    exit 1
fi
echo "OK: (e) selection arrays are selection-only; warm argv is run-flag-free"

# (f) the warm is env-clean — no runtime verification env inside the warm body
if declare -f prebuild_leg_shapes | grep -qE 'ORI_DISABLE_PREDICATE_STACK_RC|ORI_VERIFY_ARC|ORI_VERIFY_EACH'; then
    echo "FAIL: (f) prebuild_leg_shapes references the aot leg's runtime env (run-only, never a warm input)"
    exit 1
fi
echo "OK: (f) warm is env-clean (no runtime verification env in prebuild_leg_shapes)"

# (j) SERIAL PIN — no `&` backgrounding inside the warm body (&& and >&
# tokens are not backgrounding; strip them before scanning)
if declare -f prebuild_leg_shapes | sed 's/&&//g; s/[0-9]>&[0-9]//g; s/>&//g' | grep -q '&'; then
    echo "FAIL: (j) prebuild_leg_shapes backgrounds a warm (&) — warms must run serially"
    exit 1
fi
echo "OK: (j) warm loop is serial (no backgrounding in prebuild_leg_shapes)"

# (i) FAIL-SEMANTICS PIN — all-fail returns non-zero under a guarded set -e
# caller; a SINGLE mid-sequence failure still runs later warms AND returns
# non-zero (continue-on-fail per leg)
CARGO_SAVED="$(declare -f cargo)"
# shellcheck disable=SC2317 # invoked indirectly: prebuild_leg_shapes -> cargo_race_retry "$@"
cargo() { return 101; }
prebuild_failed=""
if ! NEXTEST_ACTIVE="" prebuild_leg_shapes >/dev/null 2>&1; then
    prebuild_failed=1
fi
eval "$CARGO_SAVED"
if [ -z "$prebuild_failed" ]; then
    echo "FAIL: (i) prebuild_leg_shapes returned 0 with every warm failing"
    exit 1
fi
# Partial failure: fail ONLY the rust_llvm selection; later warms must still run.
: > "$CARGO_CAPTURE"
cargo() {
    printf '%s %s\n' "$#" "$*" >> "$CARGO_CAPTURE"
    case "$*" in
        *"-p ori_llvm --lib"*) return 101 ;;
    esac
    return 0
}
partial_failed=""
if ! NEXTEST_ACTIVE="" prebuild_leg_shapes >/dev/null 2>&1; then
    partial_failed=1
fi
eval "$CARGO_SAVED"
if [ -z "$partial_failed" ]; then
    echo "FAIL: (i) prebuild_leg_shapes returned 0 with the rust_llvm warm failing"
    exit 1
fi
if ! grep -qxF "7 test --no-run -q $EXPECTED_AOT" "$CARGO_CAPTURE"; then
    echo "FAIL: (i) a mid-sequence warm failure stopped later warms (aot warm never ran); captured:"
    cat "$CARGO_CAPTURE"
    exit 1
fi
echo "OK: (i) failing warms return non-zero without aborting a guarded set -e caller; mid-sequence failure continues to later warms"

# (h) POSITIVE ENV PIN — run_aot exports the gated env around rust_test_leg,
# subshell-scoped (caller env unchanged after return). The (d) argv-parity
# capture rides the same stub pass.
RUN_CAPTURE="$(mktemp)"
ENV_CAPTURE="$(mktemp)"
rust_test_leg() {
    shift
    printf '%s %s\n' "$#" "$*" >> "$RUN_CAPTURE"
    printf '%s|%s|%s|%s\n' \
        "${ORI_DISABLE_PREDICATE_STACK_RC:-}" "${ORI_VERIFY_ARC:-}" \
        "${ORI_VERIFY_EACH:-}" "$*" >> "$ENV_CAPTURE"
    return 0
}
unset ORI_DISABLE_PREDICATE_STACK_RC ORI_VERIFY_ARC ORI_VERIFY_EACH 2>/dev/null || true
RUST_OUTPUT="$(mktemp)"; RUST_RT_OUTPUT="$(mktemp)"; RUST_LLVM_OUTPUT="$(mktemp)"; AOT_OUTPUT="$(mktemp)"
run_rust_workspace >/dev/null
run_rust_rt >/dev/null
run_rust_llvm >/dev/null
run_aot >/dev/null
if [ -n "${ORI_DISABLE_PREDICATE_STACK_RC:-}" ] || [ -n "${ORI_VERIFY_ARC:-}" ] || [ -n "${ORI_VERIFY_EACH:-}" ]; then
    echo "FAIL: (h) run_aot leaked the gated verification env into the caller (exports must stay subshell-scoped)"
    exit 1
fi
expected_runs="$(printf '6 %s\n2 %s\n3 %s\n4 %s\n' "$EXPECTED_WORKSPACE" "$EXPECTED_RUST_RT" "$EXPECTED_RUST_LLVM" "$EXPECTED_AOT")"
actual_runs="$(cat "$RUN_CAPTURE")"
if [ "$actual_runs" != "$expected_runs" ]; then
    echo "FAIL: (d) run_<leg> selection argc+argv diverged from the SSOT arrays:"
    echo "--- expected ---"; printf '%s\n' "$expected_runs"
    echo "--- actual ---"; printf '%s\n' "$actual_runs"
    exit 1
fi
aot_env_line="$(grep -F -- "$EXPECTED_AOT" "$ENV_CAPTURE" | head -1)"
rm -f "$RUN_CAPTURE" "$ENV_CAPTURE" "$RUST_OUTPUT" "$RUST_RT_OUTPUT" "$RUST_LLVM_OUTPUT" "$AOT_OUTPUT" "$CARGO_CAPTURE"
case "$aot_env_line" in
    "1|1|1|"*) ;;
    *)
        echo "FAIL: (h) run_aot did not export the gated verification env around its rust_test_leg call (captured '$aot_env_line'; expected prefix '1|1|1|')"
        exit 1
        ;;
esac
echo "OK: (d)+(h) run argv parity holds; run_aot's gated env is present at the call and subshell-scoped"

# (g) INTEGRATION SHAPE — scoped to the PARALLEL block: guarded warm call,
# no surviving wide --no-run workspace warm, harness-exact builds present.
PARALLEL_BLOCK="$(mktemp)"
awk '/^if \[\[ \$PARALLEL -eq 1 \]\]/{inblk=1} inblk{print} inblk && /^else$/{exit}' "$TEST_ALL" > "$PARALLEL_BLOCK"
if ! [ -s "$PARALLEL_BLOCK" ]; then
    echo "FAIL: (g) could not extract the \$PARALLEL -eq 1 block from test-all.sh"
    exit 1
fi
strip_comments() { grep -vE '^[[:space:]]*#' "$1"; }
if ! strip_comments "$PARALLEL_BLOCK" | grep -qE 'if[[:space:]]+!.*prebuild_leg_shapes'; then
    echo "FAIL: (g) the parallel block has no guarded 'if ! prebuild_leg_shapes' invocation (test-all.sh runs set -e; an unguarded call aborts the suite)"
    rm -f "$PARALLEL_BLOCK"
    exit 1
fi
if strip_comments "$PARALLEL_BLOCK" | grep -E -- '--no-run' | grep -E -- '--workspace' | grep -qv -- '--exclude'; then
    echo "FAIL: (g) the parallel block still carries a wide --no-run --workspace warm without --exclude (the per-leg warms replaced it)"
    rm -f "$PARALLEL_BLOCK"
    exit 1
fi
for build_line in \
    'cargo build -p ori_rt -q' \
    'cargo build -p oric --bin ori -q' \
    'cargo build -p ori_rt --release -q' \
    'cargo build -p oric --bin ori --release -q'; do
    if ! strip_comments "$PARALLEL_BLOCK" | grep -qF "$build_line"; then
        echo "FAIL: (g) parallel block missing harness-exact build '$build_line' on a non-comment line (the AOT harness's per-process cargo builds use these exact single-package shapes)"
        rm -f "$PARALLEL_BLOCK"
        exit 1
    fi
done
# Ordering: every warm (the guarded prebuild_leg_shapes call AND all four
# harness-exact builds) must precede the FIRST backgrounded leg dispatch
# (`timed_leg <name> run_<leg> &`) — a warm after dispatch is no warm at all.
warm_line="$(grep -nE 'if[[:space:]]+!.*prebuild_leg_shapes' "$PARALLEL_BLOCK" | grep -v '^\([0-9]*\):[[:space:]]*#' | head -1 | cut -d: -f1)"
first_dispatch_line="$(grep -nE '^[[:space:]]*timed_leg[[:space:]].*&[[:space:]]*$' "$PARALLEL_BLOCK" | head -1 | cut -d: -f1)"
if [ -z "$first_dispatch_line" ]; then
    echo "FAIL: (g) no backgrounded 'timed_leg ... &' dispatch found in the parallel block (extraction broke?)"
    rm -f "$PARALLEL_BLOCK"
    exit 1
fi
if [ -z "$warm_line" ] || [ "$warm_line" -ge "$first_dispatch_line" ]; then
    echo "FAIL: (g) guarded prebuild_leg_shapes (line ${warm_line:-absent}) does not precede the first leg dispatch (line $first_dispatch_line) — a warm after dispatch is no warm"
    rm -f "$PARALLEL_BLOCK"
    exit 1
fi
for build_line in \
    'cargo build -p ori_rt -q' \
    'cargo build -p oric --bin ori -q' \
    'cargo build -p ori_rt --release -q' \
    'cargo build -p oric --bin ori --release -q'; do
    bl="$(grep -nF "$build_line" "$PARALLEL_BLOCK" | grep -v '^\([0-9]*\):[[:space:]]*#' | tail -1 | cut -d: -f1)"
    if [ -z "$bl" ] || [ "$bl" -ge "$first_dispatch_line" ]; then
        echo "FAIL: (g) harness-exact build '$build_line' (line ${bl:-absent}) does not precede the first leg dispatch (line $first_dispatch_line)"
        rm -f "$PARALLEL_BLOCK"
        exit 1
    fi
done
rm -f "$PARALLEL_BLOCK"
# KEEP-IN-SYNC anchor: the harness-exact build shapes this test asserts
# mirror the AOT harness's own per-process builds. If the harness's build()
# invocation drifts, this cross-check fails so the Phase-1 warm shapes get
# re-synced.
HARNESS_SRC="$HERE/../../compiler/ori_llvm/tests/aot/util/binary.rs"
if [ ! -f "$HARNESS_SRC" ]; then
    echo "FAIL: (g) AOT harness source not found at $HARNESS_SRC (cross-check anchor lost)"
    exit 1
fi
for anchor in \
    '"build", "-p", pkg, "--quiet"' \
    'build("ori_rt", None)' \
    'build("oric", Some("ori"))'; do
    if ! grep -qF "$anchor" "$HARNESS_SRC"; then
        echo "FAIL: (g) AOT harness build invocation drifted — anchor '$anchor' not found in binary.rs; re-sync the Phase-1 harness-exact builds to the harness's actual cargo shapes"
        exit 1
    fi
done
echo "OK: (g) parallel block: guarded prebuild_leg_shapes + harness-exact builds all precede the first leg dispatch; wide workspace warms replaced; harness build shapes cross-checked against binary.rs"

# (k) SEQUENTIAL-BLOCK BUILD PARITY — the sequential (`else`) branch runs the
# same harness-exact per-process builds afterward (run_aot / run_ori_llvm),
# so it must carry the identical four single-package builds and never a
# joint `-p oric -p ori_rt` variant (a feature-unification mismatch the
# harness's own later per-process build would recompile against, tripping
# the AOT identity-gate even without concurrent dispatch).
FULL_IF_BLOCK="$(mktemp)"
awk '/^if \[\[ \$PARALLEL -eq 1 \]\]/{inblk=1} inblk{print} inblk && /^fi$/{exit}' "$TEST_ALL" > "$FULL_IF_BLOCK"
if ! [ -s "$FULL_IF_BLOCK" ]; then
    echo "FAIL: (k) could not extract the full \$PARALLEL -eq 1 if/else/fi block from test-all.sh"
    rm -f "$FULL_IF_BLOCK"
    exit 1
fi
SEQUENTIAL_BLOCK="$(mktemp)"
awk '/^else$/{inblk=1} inblk{print}' "$FULL_IF_BLOCK" > "$SEQUENTIAL_BLOCK"
rm -f "$FULL_IF_BLOCK"
if ! [ -s "$SEQUENTIAL_BLOCK" ]; then
    echo "FAIL: (k) could not extract the sequential (else) branch from the parallel/sequential if-block"
    rm -f "$SEQUENTIAL_BLOCK"
    exit 1
fi
if strip_comments "$SEQUENTIAL_BLOCK" | grep -qE -- '-p[[:space:]]+oric[[:space:]]+-p[[:space:]]+ori_rt|-p[[:space:]]+ori_rt[[:space:]]+-p[[:space:]]+oric'; then
    echo "FAIL: (k) the sequential block still carries a joint '-p oric -p ori_rt' build (must be two separate single-package builds)"
    rm -f "$SEQUENTIAL_BLOCK"
    exit 1
fi
for build_line in \
    'cargo build -p ori_rt -q' \
    'cargo build -p oric --bin ori -q' \
    'cargo build -p ori_rt --release -q' \
    'cargo build -p oric --bin ori --release -q'; do
    if ! strip_comments "$SEQUENTIAL_BLOCK" | grep -qF "$build_line"; then
        echo "FAIL: (k) sequential block missing harness-exact build '$build_line' on a non-comment line"
        rm -f "$SEQUENTIAL_BLOCK"
        exit 1
    fi
done
rm -f "$SEQUENTIAL_BLOCK"
echo "OK: (k) sequential block carries all four harness-exact single-package builds; no joint -p oric -p ori_rt variant"

echo "PASS: test_test_all_prebuild_shape_parity — Phase-1 warm shapes == Phase-2 leg + harness shapes (single-sourced)"
