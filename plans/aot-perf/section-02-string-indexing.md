---
section: 2
title: "String Indexing Codegen"
status: not-started
reviewed: true
goal: "Implement s[i] string indexing in AOT mode — add TypeInfo::Str handler in apply_protocols.rs and ori_str_index runtime function"
depends_on: []
inspired_by:
  - "Existing list indexing: emit_list_index() in list_builtins.rs + ori_list_get() in ori_rt"
  - "Existing map indexing: emit_map_get() in map_builtins.rs + ori_map_get() in ori_rt"
third_party_review: { status: none, updated: null }
---

# §02 String Indexing Codegen

## Goal

Fix the AOT crash on `s[i]` string indexing. The type checker and ARC lowering already support it. Only the LLVM codegen handler and runtime function are missing.

## Bug Analysis

**Reproducer**: Any `.ori` file with `s[i]` compiled via `ori build`

**Crash trace**:
1. Type checker approves `str[int]` → emits `__index` protocol call
2. ARC lowering (`lower_index()` in `ori_arc/src/lower/collections/mod.rs:160-188`) emits `__index(receiver, index)` correctly
3. LLVM codegen reaches `try_emit_protocol()` in `apply_protocols.rs:79-103`
4. `match &type_info` has `TypeInfo::List` and `TypeInfo::Map` but **no `TypeInfo::Str`** case
5. Falls to `_ =>` wildcard → returns `None` → variable never defined
6. Downstream code tries to use undefined variable → index-out-of-bounds panic

**The fix follows the exact pattern of `emit_list_index()` — the code journey J10 confirms this pattern scores 10/10.**

## Implementation

### Step 1: Add `ori_str_index` Runtime Function

**File**: `compiler/ori_rt/src/string/ops.rs`

Add a function that indexes a string by codepoint position and returns a single-codepoint string:

```rust
/// Index a string by codepoint position.
///
/// Returns a new OriStr containing the single codepoint at position `index`.
/// Panics if `index < 0` or `index >= codepoint_count`.
///
/// # Safety
/// `out_ptr` must point to a valid OriStr-sized allocation.
#[no_mangle]
pub extern "C-unwind" fn ori_str_index(
    str_ptr: *const u8,
    str_len: i64,
    index: i64,
    out_ptr: *mut u8,
) {
    // 1. Bounds check
    // 2. Walk UTF-8 codepoints to find the nth one
    // 3. Extract the codepoint bytes
    // 4. Write an SSO OriStr containing just that codepoint to out_ptr
}
```

**Spec reference**: `s[i]` returns a single-codepoint `str`, not a `char`. The indexing is by codepoint position, not byte offset. (Spec: Clause 7 — Indexing)

**UTF-8 walk**: Use `str_ptr[0..str_len]` as a byte slice, iterate codepoints counting until reaching `index`, extract the codepoint bytes. This is O(n) but correct.

### Step 2: Declare Runtime Function in LLVM Codegen

**File**: `compiler/ori_llvm/src/codegen/runtime_decl/mod.rs` (or the runtime function registry)

Register `ori_str_index` with signature:
- Params: `ptr` (str data), `i64` (str len), `i64` (index), `ptr` (out_ptr)
- Returns: `void`
- Attributes: `nounwind` (panics via `C-unwind`, but from LLVM's perspective it may unwind)

Actually — check whether `ori_str_index` should panic via `ori_panic_cstr` (which is `C-unwind`) or via Rust panic. Follow the pattern of `ori_list_get` for consistency.

### Step 3: Add `TypeInfo::Str` Handler in Protocol Dispatch

**File**: `compiler/ori_llvm/src/codegen/arc_emitter/apply_protocols.rs`

At line 92, add a `TypeInfo::Str` arm alongside `TypeInfo::List` and `TypeInfo::Map`:

```rust
match &type_info {
    TypeInfo::List { element } => self.emit_list_index(recv, idx, *element),
    TypeInfo::Map { key, value } => self.emit_map_get(recv, idx, *key, *value),
    TypeInfo::Str => self.emit_str_index(recv, idx),  // NEW
    _ => {
        tracing::warn!(?type_info, "__index on unsupported type");
        None
    }
}
```

### Step 4: Implement `emit_str_index`

**File**: `compiler/ori_llvm/src/codegen/arc_emitter/builtins/collections/` (new function, or in an existing string builtins file)

Follow the `emit_list_index` pattern:
1. Extract data pointer from `OriStr` (handle SSO vs heap — use `ori_str_data()` runtime call)
2. Extract length (use `ori_str_len()` runtime call)
3. Allocate stack space for the output `OriStr` (24 bytes)
4. Call `ori_str_index(data, len, index, out_ptr)`
5. Load the result `OriStr` from the stack
6. Return it as a `ValueId`

**SSO consideration**: The result is always a 1-4 byte codepoint, which always fits in SSO. The runtime function writes an SSO `OriStr` directly — no heap allocation needed for the result.

## Test Strategy

### Matrix Dimensions

**Type dimension**: `str` (only type being fixed)
**Pattern dimension**:
- ASCII indexing: `"hello"[0]` → `"h"`
- ASCII last: `"hello"[4]` → `"o"`
- Multibyte codepoint: `"héllo"[1]` → `"é"`
- OOB panic: `"hello"[5]` → panic
- Negative index: `"hello"[-1]` → panic (or `#` syntax if supported)
- Empty string: `""[0]` → panic
- Single char: `"x"[0]` → `"x"`

### Semantic Pin Tests

1. **Pin: ASCII indexing returns correct single-char string**: `"hello"[1] == "e"` → true
2. **Pin: UTF-8 indexing counts codepoints not bytes**: `"héllo"[1] == "é"` → true
3. **Pin: OOB panics**: `"hello"[10]` → runtime panic with bounds message

### TDD Ordering

- [ ] Write failing AOT test: `s[0]` on a string literal → currently crashes
- [ ] Write UTF-8 test: `"héllo"[1]` → currently crashes
- [ ] Write OOB test: `"hello"[5]` → should panic cleanly (not crash)
- [ ] Implement `ori_str_index` in `ori_rt`
- [ ] Declare runtime function in LLVM codegen
- [ ] Add `TypeInfo::Str` arm in `apply_protocols.rs`
- [ ] Implement `emit_str_index`
- [ ] Verify all tests pass in debug AND release
- [ ] Re-run bench_string.ori → should compile and run

### Test Files

- `compiler/ori_llvm/tests/aot/strings.rs` — add string indexing AOT tests
- `tests/spec/expressions/index_access.ori` — existing spec tests (lines 206-228) should now pass in LLVM backend

## §02.R Third Party Review Findings

- None.

## Completion Checklist

- [ ] `ori_str_index` runtime function implemented and tested
- [ ] Runtime function declared in LLVM codegen
- [ ] `TypeInfo::Str` arm added to `apply_protocols.rs`
- [ ] `emit_str_index` implemented following `emit_list_index` pattern
- [ ] ASCII, UTF-8, and OOB tests passing
- [ ] `bench_string.ori` compiles and runs correctly
- [ ] `./test-all.sh` passes in debug and release
- [ ] No regressions in existing string AOT tests
- [ ] `/tpr-review` passed — independent Codex review found no critical or major issues (or all findings triaged)
