#!/bin/bash
# Regression matrix for the alive2-verify.sh modellability content-scan.
#
# The --all-codegen sweep must NOT count documented-unmodellable functions
# (RC operations / exception-handling / COW / variadic) as `failed` — Alive2
# cannot model those constructs, so they are classified `suppressed`/
# `unmodellable` instead of handed to alive-tv. The classifier
# (classify_function_modellability) scans a function's pre-opt IR for the
# construct classes per tests/alive2/README.md "Corpus Selection Criteria".
#
# This test exercises the alive-tv-INDEPENDENT `--classify-function` mode against
# synthetic LLVM IR — no alive-tv, no compiler build required. Construct x verdict
# matrix (unmodellable classes + modellable counter-cases + multi-function
# isolation).
#
# Usage: tests/alive2/test-classify-modellability.sh [--verbose]
# Exit: 0 = all cells pass; 1 = one or more cells failed.

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
VERIFY="$ROOT_DIR/diagnostics/alive2-verify.sh"
VERBOSE=0
[[ "${1:-}" == "--verbose" ]] && VERBOSE=1

TMP="$(mktemp -d -t alive2-classify-XXXXXX)"
trap 'rm -rf "$TMP"' EXIT

PASS=0
FAIL=0
CELLS=0

# classify <fixture-ll-text> <func> <expected-verdict> <cell-name>
#   expected-verdict: "modellable" OR "unmodellable:<class>" (eh|rc|cow|variadic).
#   Prefix match — a positive cell asserts the SPECIFIC class so the classifier's
#   reason output is pinned, not just the unmodellable/modellable split.
classify() {
    local ir="$1" func="$2" expect="$3" name="$4"
    CELLS=$((CELLS + 1))
    local llf="$TMP/$name.ll"
    printf '%s\n' "$ir" > "$llf"
    local out rc
    out="$("$VERIFY" --no-color --classify-function "$llf" "$func" 2>&1)"
    rc=$?
    if [[ $rc -ne 0 ]]; then
        echo "FAIL [$name]: --classify-function exited $rc (out: ${out:0:80})"
        FAIL=$((FAIL + 1)); return
    fi
    if [[ "$out" == "$expect"* ]]; then
        [[ $VERBOSE -eq 1 ]] && echo "PASS [$name]: $out"
        PASS=$((PASS + 1))
    else
        echo "FAIL [$name]: expected '$expect*', got '$out'"
        FAIL=$((FAIL + 1))
    fi
}

# ── Positive cells: documented-unmodellable constructs → unmodellable ──

# 1. landingpad (semantic pin — the _ori_elem_dec$N drop-glue bug case)
classify 'define void @"_ori_elem_dec$3"(ptr %x) personality ptr @__gxx_personality_v0 {
entry:
  invoke void @drop(ptr %x) to label %ok unwind label %lpad
ok:
  ret void
lpad:
  %e = landingpad { ptr, i32 } cleanup
  resume { ptr, i32 } %e
}' '_ori_elem_dec$3' 'unmodellable:eh' eh_landingpad

# 2. invoke (the EH-unwind unwind-edge form; a valid invoke requires a landingpad
#    unwind block, so invoke is inherently EH-unmodellable)
classify 'define void @_ori_inv(ptr %x) personality ptr @p {
entry:
  invoke void @f(ptr %x) to label %ok unwind label %u
ok:
  ret void
u:
  %e = landingpad { ptr, i32 } cleanup
  resume { ptr, i32 } %e
}' '_ori_inv' 'unmodellable:eh' eh_invoke

# 3. ori_rc_dec (the RC _ori_main case)
classify 'define void @_ori_main() {
entry:
  call void @ori_rc_dec(ptr null)
  ret void
}' '_ori_main' 'unmodellable:rc' rc_dec

# 4. ori_buffer_rc_dec
classify 'define void @_ori_buf() {
entry:
  call void @ori_buffer_rc_dec(ptr null)
  ret void
}' '_ori_buf' 'unmodellable:rc' rc_buffer

# 5. ori_rc_alloc (the allocation call codegen actually emits — the @ori_alloc
#    symbol is barely used; this pins the real allocation call)
classify 'define ptr @_ori_al() {
entry:
  %p = call ptr @ori_rc_alloc(i64 8)
  ret ptr %p
}' '_ori_al' 'unmodellable:rc' rc_alloc

# 6. ori_panic (the panic token is in the memory-family scan list)
classify 'define void @_ori_p() {
entry:
  call void @ori_panic(ptr null)
  unreachable
}' '_ori_p' 'unmodellable:rc' rc_panic

# 7. is_unique (COW — real runtime symbol ori_rc_is_unique)
classify 'define i1 @_ori_cow(ptr %b) {
entry:
  %u = call i1 @ori_rc_is_unique(ptr %b)
  ret i1 %u
}' '_ori_cow' 'unmodellable:cow' cow_is_unique

# ── Negative cells: modellable functions → MUST NOT be excluded ──

# 8. pure arithmetic (negative pin — don't over-exclude)
classify 'define i64 @_ori_add(i64 %a, i64 %b) {
entry:
  %s = add i64 %a, %b
  %m = mul i64 %s, %b
  ret i64 %m
}' '_ori_add' 'modellable' pure_arith

# 9. pure control flow
classify 'define i64 @_ori_cf(i64 %a) {
entry:
  %c = icmp sgt i64 %a, 0
  br i1 %c, label %pos, label %neg
pos:
  ret i64 1
neg:
  ret i64 0
}' '_ori_cf' 'modellable' pure_control_flow

# 10. indirect call / closure (negative pin — function pointers are valid IR
#     alive-tv handles; closures are NOT an unmodellable class)
classify 'define i64 @_ori_ind(ptr %fn, i64 %a) {
entry:
  %r = call i64 %fn(i64 %a)
  ret i64 %r
}' '_ori_ind' 'modellable' indirect_call

# 11. benign external call not in the scan list (don't exclude on ANY extern)
classify 'define i64 @_ori_ext(i64 %a) {
entry:
  %r = call i64 @llvm.ctpop.i64(i64 %a)
  ret i64 %r
}' '_ori_ext' 'modellable' benign_extern

# 12. resume-only EH residue (still unmodellable — EH class)
classify 'define void @_ori_res(ptr %x) personality ptr @p {
entry:
  br label %lpad
lpad:
  %e = landingpad { ptr, i32 } cleanup
  resume { ptr, i32 } %e
}' '_ori_res' 'unmodellable:eh' eh_resume

# 13. variadic / va_arg (README Exclude: "Variadic functions")
classify 'define i32 @_ori_var(ptr %fmt, ...) {
entry:
  %ap = alloca ptr
  call void @llvm.va_start(ptr %ap)
  %v = va_arg ptr %ap, i32
  call void @llvm.va_end(ptr %ap)
  ret i32 %v
}' '_ori_var' 'unmodellable:variadic' variadic

# 14. multi-function isolation (negative pin — the scan reads ONLY the named
#     function's body, never another function's constructs in the same file).
#     Classifying the PURE function in a file that ALSO holds an EH function
#     must return `modellable`; a whole-file grep would wrongly say unmodellable.
classify 'define i64 @_ori_pure(i64 %a) {
entry:
  %s = add i64 %a, 1
  ret i64 %s
}
define void @_ori_dirty(ptr %x) personality ptr @p {
entry:
  %e = landingpad { ptr, i32 } cleanup
  resume { ptr, i32 } %e
}' '_ori_pure' 'modellable' multi_func_isolation_pure

# 15. user-function call (negative pin — runtime @ori_ vs user @_ori_). A call to
#     another COMPILED function (@_ori_helper, leading underscore) is NOT a runtime
#     call; alive-tv handles it as a consistent opaque callee. MUST be modellable.
classify 'define i64 @_ori_uc(i64 %a) {
entry:
  %r = call i64 @_ori_helper(i64 %a)
  ret i64 %r
}' '_ori_uc' 'modellable' user_call

# 16. pure runtime call (negative pin — pure @ori_ runtime calls are MODELLABLE;
#     alive-tv models ori_compare_int etc. as a consistent uninterpreted function.
#     Only the memory-management family is excluded, not every @ori_ call).
classify 'define i1 @_ori_pc(i64 %a, i64 %b) {
entry:
  %r = call i1 @ori_compare_int(i64 %a, i64 %b)
  ret i1 %r
}' '_ori_pc' 'modellable' pure_runtime_call

# ── RC container families (the real emitted symbols — regression for the
#    str/list/map/set/elem-dec RC ops the @ori_rc_ prefix-only scan missed) ──

# 17. ori_list_rc_inc (list RC)
classify 'define void @_ori_l(ptr %x) {
entry:
  call void @ori_list_rc_inc(ptr %x)
  ret void
}' '_ori_l' 'unmodellable:rc' rc_list

# 18. ori_str_rc_dec (str RC)
classify 'define void @_ori_s(ptr %x) {
entry:
  call void @ori_str_rc_dec(ptr %x)
  ret void
}' '_ori_s' 'unmodellable:rc' rc_str

# 19. ori_map_buffer_rc_dec (map-buffer RC)
classify 'define void @_ori_m(ptr %x) {
entry:
  call void @ori_map_buffer_rc_dec(ptr %x)
  ret void
}' '_ori_m' 'unmodellable:rc' rc_map_buffer

# 20. ori_str_elem_dec (elem-dec drop glue, non-rc_-prefixed)
classify 'define void @_ori_ed(ptr %x) {
entry:
  call void @ori_str_elem_dec(ptr %x)
  ret void
}' '_ori_ed' 'unmodellable:rc' rc_elem_dec

# 21. ori_rc_is_unique_or_null (COW variant — no trailing-( anchor)
classify 'define i1 @_ori_con(ptr %b) {
entry:
  %u = call i1 @ori_rc_is_unique_or_null(ptr %b)
  ret i1 %u
}' '_ori_con' 'unmodellable:cow' cow_or_null

# ── Allocation / free / drop memory family (the symbols codegen ACTUALLY emits
#    most — caught by the alloc/free/drop tokens, not the rc_(inc|dec) infix) ──

# 22. ori_rc_free (the real free)
classify 'define void @_ori_fr(ptr %x) {
entry:
  call void @ori_rc_free(ptr %x)
  ret void
}' '_ori_fr' 'unmodellable:rc' rc_free

# 23. ori_list_alloc_data (list allocation)
classify 'define ptr @_ori_la() {
entry:
  %p = call ptr @ori_list_alloc_data(i64 4, i64 8)
  ret ptr %p
}' '_ori_la' 'unmodellable:rc' rc_list_alloc

# 24. ori_buffer_drop_unique (drop)
classify 'define void @_ori_dr(ptr %x) {
entry:
  call void @ori_buffer_drop_unique(ptr %x)
  ret void
}' '_ori_dr' 'unmodellable:rc' rc_drop_unique

# ── Isolated single-token pins (mutation guards — every other RC alternation
#    token has a cell that matches ONLY via that token; `drop` and `buffer`
#    did not — their only cells (ori_buffer_drop_unique) double-match `buffer`,
#    so dropping either alternation branch from the scan regex went undetected.
#    These two real runtime symbols each match via ONE token, so removing that
#    branch flips the cell to `modellable` and fails the matrix). ──

# 25. ori_iter_drop (drop ALONE — no buffer/alloc/free/rc/elem_dec/panic token)
classify 'define void @_ori_idr(ptr %x) {
entry:
  call void @ori_iter_drop(ptr %x)
  ret void
}' '_ori_idr' 'unmodellable:rc' rc_drop_isolated

# 26. ori_list_reset_buffer (buffer ALONE — no rc/drop/alloc/free/elem_dec/panic token)
classify 'define void @_ori_rbuf(ptr %x) {
entry:
  call void @ori_list_reset_buffer(ptr %x)
  ret void
}' '_ori_rbuf' 'unmodellable:rc' rc_buffer_isolated

# ── Self-verifying count ──
EXPECTED_CELLS=26
if [[ $CELLS -ne $EXPECTED_CELLS ]]; then
    echo "FAIL: ran $CELLS cells, expected $EXPECTED_CELLS (a cell was silently skipped)"
    FAIL=$((FAIL + 1))
fi

echo ""
echo "classify-modellability matrix: $PASS passed, $FAIL failed ($CELLS cells)"
[[ $FAIL -eq 0 ]] && exit 0 || exit 1
