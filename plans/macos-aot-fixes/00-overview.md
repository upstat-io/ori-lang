# macOS AOT Fixes

## Context

CI run `23420458239` (PR #88) passed Linux fully but failed on macOS with 2 AOT test failures + Windows had a timeout. The string concat exponential capacity bug is already fixed. These are the remaining issues.

## Checklist

- [ ] **Fix 1**: `arc::test_rc_project_merge_edge_scoped_cleanup` (exit 1)
- [ ] **Fix 2**: `elem_dec_scope::test_trampoline_map_str_identity` (SIGSEGV -139)
- [ ] **Fix 3**: CI cross-platform timeout (10→30 min) — already done in `.github/workflows/ci.yml`
- [ ] All 3 fixes committed, pushed, CI green

---

## Fix 1: Merge-Edge Scoped Cleanup (exit 1)

**Test:** `compiler/ori_llvm/tests/aot/arc.rs:1050`
**Symptom:** Compiled binary returns 1 instead of 0 — wrong branch taken or string comparison fails.

### Reproduce
```bash
ori build plans/macos-aot-fixes/test1_merge_edge.ori -o /tmp/test1
/tmp/test1; echo "Exit: $?"
```

### Diagnose

**If exit 1** — the logic `a.name.len() > b.name.len()` or the final `pick == "alpha..."` is failing.

1. Check string lengths:
```bash
# Create a debug version that prints lengths
cat > /tmp/debug_merge.ori << 'EOF'
type Record = { name: str, data: str }
@make_alpha () -> Record =
    Record { name: "alpha-record-name-over-23-bytes!", data: "alpha-data-payload-over-23-bytes!" };
@make_beta () -> Record =
    Record { name: "beta-record-name-over-23-bytes!", data: "beta-data-payload-over-23-bytes!" };
@main () -> int = {
    let $a = make_alpha();
    let $b = make_beta();
    print(msg: `a.name.len = {a.name.len()}, b.name.len = {b.name.len()}`);
    let $pick = if a.name.len() > b.name.len() then a.name else b.name;
    print(msg: `pick = "{pick}", len = {pick.len()}`);
    print(msg: `eq = {pick == "alpha-record-name-over-23-bytes!"}`);
    if pick == "alpha-record-name-over-23-bytes!" then 0 else 1
}
EOF
ori build /tmp/debug_merge.ori -o /tmp/debug_merge && /tmp/debug_merge
```

2. Dump IR and RC trace:
```bash
ORI_DUMP_AFTER_LLVM=1 ori build plans/macos-aot-fixes/test1_merge_edge.ori -o /tmp/test1 2> /tmp/test1_ir.txt
ORI_CHECK_LEAKS=1 ORI_TRACE_RC=1 /tmp/test1
```

### Likely Root Causes

1. **`str.len()` returns byte_len instead of char count on ARM64** — check if `.len()` dispatches differently. The strings are ASCII so byte_len == char count, but a dispatch issue could return `capacity` or `0`.

2. **Struct field projection ABI mismatch** — `a.name` extracts a field from an sret-returned `Record`. On ARM64 the struct may be returned differently (two registers vs memory). Check if the `name` field extraction GEPs are correct.

3. **String equality (`==`) ABI** — `ori_str_eq` is called with two `OriStr*` pointers. If one pointer is stale (RC freed early due to scoped cleanup edge), the comparison reads garbage.

### Fix Pattern

The fix will be in one of:
- `compiler/ori_llvm/src/codegen/arc_emitter/` — RC emission for merge edges
- `compiler/ori_llvm/src/codegen/function_compiler/` — struct return ABI
- `compiler/ori_rt/src/string/ops.rs` — `ori_str_eq` null handling

---

## Fix 2: Trampoline Map Str Identity (SIGSEGV -139)

**Test:** `compiler/ori_llvm/tests/aot/elem_dec_scope.rs:148`
**Symptom:** Segfault — the compiled binary crashes.

### Reproduce
```bash
ori build plans/macos-aot-fixes/test2_trampoline_str.ori -o /tmp/test2
/tmp/test2; echo "Exit: $?"
```

### Diagnose

**If SIGSEGV:**
```bash
# Get crash location
lldb -- /tmp/test2
# In lldb: run, then bt (backtrace)

# Or with codesign (if needed on macOS):
codesign -s - /tmp/test2
lldb -- /tmp/test2
```

```bash
# Dump IR
ORI_DUMP_AFTER_LLVM=1 ori build plans/macos-aot-fixes/test2_trampoline_str.ori -o /tmp/test2 2> /tmp/test2_ir.txt
# Look for trampoline functions:
grep -A 20 "trampoline\|_ori_elem_dec\|map.*transform" /tmp/test2_ir.txt
```

### Likely Root Cause

**ARM64 ABI mismatch in iterator trampoline.** The trampoline wraps a lambda `s -> s` for the iterator's `map` callback. On x86_64, `OriStr` (24 bytes = `{ i64, i64, ptr }`) fits in 2 registers + 1 pointer register and is returned by value. On ARM64, aggregates >16 bytes are returned via sret (hidden first parameter). If the trampoline doesn't account for this, the caller and callee disagree on parameter layout → stack corruption → SIGSEGV.

**Where to look:**
- `compiler/ori_llvm/src/codegen/arc_emitter/` — search for `trampoline` or `lambda_wrapper`
- `compiler/ori_llvm/src/codegen/function_compiler/` — search for `trampoline`
- Check if the trampoline declaration matches the expected ABI for `map`'s callback

### Fix Pattern

The fix will be in the trampoline generation code — it needs to use the same ABI (sret vs register return) that the iterator runtime expects. Specifically:
1. Find where trampolines are generated (likely `arc_emitter` or `function_compiler`)
2. Check if the trampoline's return type uses sret on ARM64
3. Ensure the trampoline's calling convention matches what `ori_iter_map` expects

---

## Fix 3: CI Timeout (DONE)

Already applied: `.github/workflows/ci.yml` line 198 changed from `timeout-minutes: 10` to `timeout-minutes: 30`.

---

## Files Involved

| File | Role |
|------|------|
| `compiler/ori_llvm/tests/aot/arc.rs:1050` | Test 1 source |
| `compiler/ori_llvm/tests/aot/elem_dec_scope.rs:148` | Test 2 source |
| `compiler/ori_llvm/src/codegen/arc_emitter/` | ARC IR → LLVM emission |
| `compiler/ori_llvm/src/codegen/function_compiler/` | Function ABI + trampolines |
| `compiler/ori_rt/src/string/ops.rs` | String equality + concat |
| `compiler/ori_rt/src/iterator/` | Iterator map callback ABI |
| `.github/workflows/ci.yml:198` | CI timeout (already fixed) |
| `plans/macos-aot-fixes/test1_merge_edge.ori` | Standalone repro file 1 |
| `plans/macos-aot-fixes/test2_trampoline_str.ori` | Standalone repro file 2 |
