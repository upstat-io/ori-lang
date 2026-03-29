---
section: "11"
title: "Collection Specialization"
status: not-started
reviewed: false
third_party_review:
  status: findings
  updated: 2026-03-23
goal: "Optimize collection representations: small string optimization (SSO), small vector optimization (SVO), packed bool arrays, and narrow-element backing stores"
inspired_by:
  - "C++ std::string SSO (libstdc++ basic_string.h — 15-byte inline buffer)"
  - "Rust SmallVec (smallvec crate — configurable inline capacity)"
  - "Swift Array value semantics with COW (stdlib/public/core/Array.swift)"
  - "Zig packed structs (src/Type.zig)"
depends_on: ["04", "06"]
sections:
  - id: "11.1"
    title: "Small String Optimization (SSO)"
    status: not-started
  - id: "11.2"
    title: "Small Vector Optimization (SVO)"
    status: not-started
  - id: "11.3"
    title: "Packed Bool Arrays"
    status: not-started
  - id: "11.4"
    title: "Narrow-Element Collections"
    status: not-started
  - id: "11.5"
    title: "Completion Checklist"
    status: not-started
---

# Section 11: Collection Specialization

**Context:** Collections remain one of Ori's most common allocation surfaces, but the current baseline is mixed: `str` already uses the 24-byte SSO-capable `OriStr` representation, while `[T]`, `{K:V}`, and `{T}` still use heap-backed 24-byte headers plus RC-managed backing storage with the V5 32-byte RC header. This section therefore starts as an audit/regression-guard pass for the existing string path, then adds new collection optimizations (SVO, packed bool arrays, narrow-element backing stores) on top of that baseline.

**Reference implementations:**
- **C++ libstdc++** `basic_string.h`: 15-byte inline buffer in the string object itself
- **Rust SmallVec**: Configurable inline capacity, heap fallback when full
- **Swift** `Array.swift`: COW with inline storage for small arrays
- **Python** `int` (small int cache): Pre-allocated objects for -5..256

**Depends on:** §04 (element narrowing affects backing store width), §06 (struct layout for inline storage).

---

## 11.1 Small String Optimization (SSO)

**File(s):** `compiler/ori_rt/src/string/` (existing module — SSO already implemented), `compiler/ori_llvm/src/codegen/type_info/info.rs`

**NOTE:** SSO is already implemented in `ori_rt/src/string/` using `OriStrSSO` (inline <=23 bytes) / `OriStrHeap` (heap mode) with `SSO_FLAG` and `SSO_MAX_LEN`. This section is an **audit and completion task**, not greenfield implementation. Remaining work: (1) verify SSO is fully integrated with LLVM codegen, (2) ensure all string operations preserve SSO mode when possible, (3) measure actual SSO hit rates on real programs.

**Verified current coverage (2026-03-23):**
- `compiler/ori_rt/src/tests.rs` already contains extensive SSO runtime coverage
- `compiler/ori_llvm/tests/aot/string_sso.rs` already passes with 45 AOT tests
- `TypeInfo::Str` already lowers to a 24-byte `{ i64, i64, ptr }`-compatible storage shape in `compiler/ori_llvm/src/codegen/type_info/info.rs`

Current `str` representation: SSO-enabled `OriStr` union (24 bytes, inline ≤23 bytes / heap for longer strings).
Previous representation was `{ len: i64, data: *const u8 }` (16 bytes, heap-allocated data).

SSO representation: 24 bytes total, dual-mode (see `ori_rt/src/string/mod.rs`):
- **Inline mode** (len ≤ 23): data stored directly in the struct
- **Heap mode** (len > 23): pointer to heap-allocated buffer

- [ ] Verify existing SSO string layout (already implemented in `ori_rt/src/string/mod.rs`):
  ```
  Inline mode (len ≤ 23, SSO_MAX_LEN = 23):
  ┌──────────────────────────────────┬─────┐
  │  data[0..22] (23 bytes inline)   │flags│
  │  UTF-8 string data               │ 0x80│  high bit=1 → SSO
  └──────────────────────────────────┴─────┘  low 7 bits = length
  24 bytes total (OriStrSSO: bytes[23] + flags u8)

  Heap mode (len > 23):
  ┌────────────┬────────────┬──────────┐
  │  len (i64) │  cap (i64) │data (ptr)│
  │            │            │heap-alloc│
  └────────────┴────────────┴──────────┘
  24 bytes total (OriStrHeap: len i64 + cap i64 + data ptr)
  Discriminator: byte 23 is MSB of data pointer — always 0 on
  64-bit (canonical addressing), so high bit=0 → heap mode.
  ```

- [ ] Verify existing runtime SSO functions (spread across `ori_rt/src/string/ops.rs` and `ori_rt/src/string/methods/mod.rs`):
  ```rust
  // Already implemented — audit for correctness and completeness:
  extern "C" fn ori_str_len(s: *const OriStr) -> i64;
  extern "C" fn ori_str_data(s: *const OriStr) -> *const u8;
  extern "C" fn ori_str_concat(a: *const OriStr, b: *const OriStr) -> OriStr;
  // OriStr::is_sso() method exists on the struct (not an extern "C" fn)
  ```

> **CODEBASE NOTE — TypeInfo::Str (`compiler/ori_llvm/src/codegen/type_info/info.rs:49`):**
> `TypeInfo::Str` already returns a 24-byte `{i64 len, i64 cap, ptr data}`-compatible storage shape. That is layout-compatible with the current `OriStr` union, so this subsection is about preserving and testing that contract, not reintroducing a new 24-byte type from scratch.

- [ ] LLVM codegen changes — verify each is correctly implemented in `ori_llvm`:
  - [ ] **String creation codegen** (`arc_emitter/construction.rs`): when the source is a string literal of known length ≤ 23, emit the inline `OriStrSSO` struct directly (no `ori_rc_alloc` call). When length is unknown or > 23, use the existing heap path.
  - [ ] **String literal embedding** (`arc_emitter/value_emission.rs`): string literals must be emitted as `OriStr` (union of `OriStrSSO` / `OriStrHeap`), not as raw `{ i64, i64, ptr }`. Check that the LLVM struct type matches `OriStr` (24 bytes, not 16).
  - [ ] **String access codegen** (`arc_emitter/`): every string field read must emit a branch on the SSO flag (high bit of byte 23). Verify this is done via `ori_str_len` and `ori_str_data` calls — not raw GEP+load which would bypass SSO mode detection.
  - [ ] **String concat**: verify `ori_str_concat` correctly handles all 4 cases: (inline+inline)→inline, (inline+inline)→heap, (heap+inline)→heap, (heap+heap)→heap. Check that the result fits in SSO when both inputs fit.
  - [ ] **String drop**: `arc_emitter/drop_gen.rs` must check SSO flag before emitting `ori_rc_dec` — inline strings have no RC header and must not call `ori_rc_dec`. Add a test verifying no `ori_rc_dec` call for a function that only uses short string literals.
  - [ ] **TypeInfo LLVM type regression guard**: keep `TypeInfo::Str` at the current 24-byte layout-compatible shape and add/keep tests that fail if it regresses toward the old 16-byte model

- [ ] ARC interaction:
  - Inline strings have NO RC header — they're value types (copy on assignment)
  - Heap strings retain their RC header
  - The SSO flag byte disambiguates at runtime
  - [ ] Verify: assigning a short string literal copies the 24-byte struct (no `ori_rc_inc` emitted). Verify this in LLVM IR using `ORI_DUMP_AFTER_LLVM=1`.

---

## 11.2 Small Vector Optimization (SVO)

**File(s):** `compiler/ori_rt/src/list/` (existing module — add SVO support), `compiler/ori_llvm/src/codegen/type_info/info.rs`

Current `[T]` representation: `{ len: i64, cap: i64, data: *mut u8 }` (24 bytes, heap-allocated data).

SVO representation: same 24-byte size, dual-mode:
- **Inline mode** (elements fit in available inline bytes): stored directly
- **Heap mode**: standard `{ len: i64, cap: i64, data: *mut u8 }` layout

**Design note:** Unlike SSO (which uses the MSB of the data pointer as a discriminator because user-space pointers have MSB=0), SVO needs a different discriminator strategy. Options: (a) use a flag bit in `cap` (e.g., negative cap = inline mode), (b) use a sentinel in `len`, (c) use a separate approach. This must be designed during implementation — the discriminator must not conflict with the existing slice encoding (`SLICE_FLAG` uses bit 63 of cap).

- [ ] Compute inline capacity per element type:
  ```rust
  pub fn svo_inline_capacity(element_size: u32) -> u32 {
      // 23 usable bytes / element size
      if element_size == 0 { return u32::MAX; } // zero-sized: infinite
      23 / element_size
  }
  ```
  - `[int]` (8 bytes each): inline capacity = 2 (16 bytes ≤ 23)
  - `[byte]` (1 byte each): inline capacity = 23
  - `[bool]` (1 byte each): inline capacity = 23
  - `[(int, int)]` (16 bytes each): inline capacity = 1

- [ ] Implement SVO for common cases:
  - Empty list `[]` → zero-cost (len=0, inline mode)
  - Single element `[x]` → inline (no heap alloc for lists with 1-2 ints)
  - Growth: when inline → heap, migrate data + set flag

- [ ] ARC interaction:
  - Inline lists are value types (copy data on copy)
  - Heap lists use RC
  - Element types may themselves be RC'd (must still inc/dec elements)

---

## 11.3 Packed Bool Arrays

**File(s):** `compiler/ori_repr/src/collection/packed.rs`

`[bool]` currently stores 1 byte per bool. With bit packing, it stores 1 bit per bool (8× compression).

- [ ] Define packed bool array:
  ```rust
  /// Backing store for [bool] — 1 bit per element
  pub struct PackedBoolArray {
      len: usize,
      data: *mut u8,  // bit-packed: data[i/8] & (1 << (i%8))
  }
  ```

- [ ] Implement packed operations:
  ```rust
  pub fn get(data: *const u8, index: usize) -> bool {
      let byte = unsafe { *data.add(index / 8) };
      byte & (1 << (index % 8)) != 0
  }

  pub fn set(data: *mut u8, index: usize, value: bool) {
      let byte_ptr = unsafe { data.add(index / 8) };
      let mask = 1u8 << (index % 8);
      if value {
          unsafe { *byte_ptr |= mask; }
      } else {
          unsafe { *byte_ptr &= !mask; }
      }
  }
  ```

- [ ] LLVM codegen for `[bool]`:
  - Index access: emit bit extraction sequence instead of byte load
  - Iteration: iterate bytes, extract bits
  - Length: stored separately (not derivable from byte count)

- [ ] Opt-out: if the user needs byte-addressable bools, provide `[byte]` as alternative

---

## 11.4 Narrow-Element Collections

**File(s):** `compiler/ori_repr/src/collection/narrow.rs`

When §04 narrows an element type (e.g., `int` → `i8`), the collection's backing store should use the narrow type.

- [ ] Specialize list backing stores:
  - `[int]` where all elements are 0..255 → `[i8]` backing store (1 byte/element vs 8)
  - `[int]` where all elements are 0..65535 → `[i16]` backing store (2 bytes/element)
  - `[float]` where all elements are f32-exact → `[f32]` backing store (4 bytes/element)

- [ ] Collection narrowing rules:
  - Narrowing applies when the element type has a bounded range from §04
  - The collection itself tracks the narrow width for correct load/store
  - Widening happens at element access time (load i8 → sext to i64)

- [ ] Map key/value narrowing:
  - `{int: str}` where keys are 0..100 → i8 key array
  - Requires hash function to work on narrow type (hash the canonical value)

---

## 11.5 Completion Checklist

**Test matrix for §11 (write failing tests FIRST, verify they fail, then implement):**

| Input | Expected representation | Semantic pin |
|---|---|---|
| `let s = "hi"` (2 bytes) | SSO inline — 0 `ori_rc_alloc` | Yes |
| `let s = "a" * 23` (23 bytes) | SSO inline — 0 `ori_rc_alloc` | Yes — boundary |
| `let s = "a" * 24` (24 bytes) | Heap — 1 `ori_rc_alloc` | Yes — just over |
| `let list = []` | Inline (zero-cap) — 0 `ori_rc_alloc` | Yes — empty SVO |
| `let list = [1, 2]` | SVO inline (2 ints) | Yes |
| `let list = [1, 2, 3]` | SVO inline (3 ints, 3×8=24 > 23) → Heap | Yes — SVO boundary |
| `let flags = [true, false, true]` | Packed bool | Yes — verify bitcount |
| `[bool]` with 1M elements | ~125KB (vs ~1MB unpacked) | Yes — 8× compression |
| `[int]` where elements are `0..255` | `[i8]` backing store (§04+§11 co-opt) | Yes |

- [ ] Existing strings ≤ 23 bytes remain SSO (no regression from the current baseline)
- [ ] Empty lists use inline mode (no heap allocation)
- [ ] `[bool]` uses 1 bit per element (verified by memory measurement)
- [ ] `[int]` with bounded elements uses narrow backing store
- [ ] SSO strings correctly handle concat, slice, and iteration
- [ ] Existing SSO suites remain green: `compiler/ori_rt` runtime SSO tests and `compiler/ori_llvm/tests/aot/string_sso.rs`
- [ ] SVO lists correctly handle push, pop, and growth
- [ ] `./test-all.sh` green
- [ ] `./clippy-all.sh` green
- [ ] `./diagnostics/valgrind-aot.sh` clean
- [ ] Performance: string-heavy benchmarks show measurable improvement from SSO
- [ ] `/tpr-review` passed — independent Codex review found no critical or major issues (or all findings triaged)

**Exit Criteria:** Creating 10,000 short strings (≤ 23 bytes each) results in ZERO heap allocations for the string data itself, verified by `ori_rc_alloc` call count = 0 in Valgrind output. `[bool]` with 1M elements uses ~125KB instead of ~1MB. SSO string operations (concat, substring, trim, split) correctly handle inline-to-heap transitions.

---

## 11.R Third Party Review Findings

- [ ] `[TPR-11-001][major]` `section-11-collection-spec.md:115` — **SVO discriminator conflicts with `SLICE_FLAG` (bit 63 of cap); solution deferred to implementation.** The plan acknowledges the conflict ("must not conflict with the existing slice encoding") but writes "This must be designed during implementation." `SLICE_FLAG = i64::MIN` (bit 63 of cap) is load-bearing for seamless slices — it's checked in `ori_buffer_rc_dec`, `propagate_elem_header`, `propagate_header`, and `IterState::List` Drop. Using negative cap for SVO inline mode would alias with slice caps (also negative). **Action:** Resolve the discriminator design before implementation. Options: (a) use a sentinel in `len` (e.g., `len = i64::MIN` for inline mode — never a valid length), (b) use a different bit in cap that doesn't conflict with SLICE_FLAG, (c) use a separate tag byte outside the existing 24-byte layout, (d) reserve a specific cap range (e.g., cap in `[i64::MIN+1, -2]` for SVO, `SLICE_FLAG | offset` for slices). Document the chosen approach in the plan. Consensus: 3/3 reviewers.
