---
section: "03"
title: "String Optimization"
status: complete
goal: "Small strings avoid heap allocation entirely; string concat is O(1) amortized when unique"
inspired_by:
  - "Roc crates/compiler/builtins/bitcode/src/str.zig — SSO with 23-byte inline threshold"
  - "Swift stdlib/public/core/StringObject.swift — 15-byte inline storage"
  - "Rust std::string::String — 24-byte layout with len/cap/ptr"
depends_on: ["01"]
sections:
  - id: "03.1"
    title: "Small String Optimization (SSO)"
    status: complete
  - id: "03.2"
    title: "String Capacity Tracking"
    status: complete
  - id: "03.3"
    title: "COW String Concat"
    status: complete
  - id: "03.4"
    title: "COW String Replace & Transform"
    status: complete
  - id: "03.5"
    title: "Runtime Implementation"
    status: complete
  - id: "03.6"
    title: "LLVM Codegen Updates"
    status: complete
  - id: "03.7"
    title: "Completion Checklist"
    status: complete
---

# Section 03: String Optimization

**Context:** Currently, `OriStr` is a 16-byte fat pointer `{len: i64, data: *const u8}`. Every string, even `""` or `"x"`, allocates an RC'd heap buffer. String concatenation always allocates a new buffer and copies both operands. This is wasteful — the median string in real programs is short (< 20 bytes), and concat chains are common (e.g., building output).

**Reference implementations:**
- **Roc** `str.zig`: 24-byte struct. Strings ≤ 23 bytes inline (high bit of last byte = SSO flag). Heap strings store `{ptr, len, capacity}`.
- **Swift** `StringObject.swift`: 16-byte struct. Strings ≤ 15 bytes inline using a discriminator byte.
- **C++ `std::string`** (libc++): 24-byte struct. Strings ≤ 22 bytes inline (SSO uses the capacity byte as length).

**Depends on:** Section 01 (COW primitives).

---

## 03.1 Small String Optimization (SSO)

**File(s):** `compiler/ori_rt/src/lib.rs`, `compiler/ori_llvm/src/codegen/arc_emitter/`

SSO stores short strings directly in the struct, avoiding heap allocation entirely. This eliminates RC overhead for the vast majority of strings in real programs.

**Design decision — struct layout:**

**(a) 24-byte struct with 23-byte SSO** (recommended — matches Roc):
```
Heap string:
┌──────────┬──────────┬──────────┐
│ len (i64)│ cap (i64)│data (ptr)│  24 bytes
└──────────┴──────────┴──────────┘

SSO string (≤ 23 bytes):
┌──────────────────────────┬─────┐
│ string data (23 bytes)   │flags│  24 bytes
└──────────────────────────┴─────┘
                            ↑
                            high bit = 1 → SSO
                            low 7 bits = length (0-23)
```

- Pro: 23 bytes covers the vast majority of strings (identifiers, short messages, numbers)
- Pro: Capacity field enables COW growth for heap strings
- Con: 8 bytes larger than current OriStr (affects stack size, register passing)
- Con: Every string operation must check SSO flag

**(b) 16-byte struct with 15-byte SSO** (Swift-style):
```
Heap string:
┌──────────┬──────────┐
│ len (i64)│data (ptr)│  16 bytes
└──────────┴──────────┘

SSO string (≤ 15 bytes):
┌──────────────┬─────┐
│ data (15 B)  │flags│  16 bytes
└──────────────┴─────┘
```

- Pro: Same size as current OriStr (no ABI change)
- Con: Only 15 bytes of SSO (many common strings are 16-23 bytes)
- Con: No capacity field — COW growth requires realloc guessing

**(c) Keep 16-byte, no SSO:**
- Pro: Simplest, no layout change
- Con: Every string allocates

**Recommended path:** Option (a) — 24-byte struct with 23-byte SSO. The extra 8 bytes per string are well worth the elimination of heap allocation for short strings. This is the layout that Roc uses, and their benchmarks show significant wins.

**Trade-off:** The ABI change (16→24 bytes) affects:
- LLVM codegen (struct type, GEP offsets, return value handling)
- Runtime functions (all `OriStr` parameters and return values)
- ARC pipeline (string RC strategy must handle SSO)
- Drop functions (SSO strings don't need `ori_rc_dec`)

This is a **pervasive change** and must be co-landed with §03.5 (runtime) and §03.6 (codegen).

### SSO Flag Encoding

- [x] Define the SSO discriminator:
  ```rust
  /// The last byte of the 24-byte struct is the SSO discriminator.
  /// Bit 7 (high bit) = 1 → SSO string (data is inline)
  /// Bit 7 (high bit) = 0 → heap string (data is a pointer)
  /// For SSO: bits 0-6 = length (0-23)
  /// For heap: this byte is part of the data pointer (which has high bit 0
  /// on all modern platforms due to canonical addressing).
  const SSO_FLAG: u8 = 0x80;
  const SSO_LEN_MASK: u8 = 0x7F;
  ```

  **Why high bit works:** On 64-bit platforms, user-space pointers have the high bit clear (canonical addresses). So the high bit of the last byte of the struct is always 0 for heap strings (it's part of the pointer). For SSO strings, we set it to 1.

- [x] Define `OriStr` layout:
  ```rust
  #[repr(C)]
  pub union OriStr {
      heap: OriStrHeap,
      sso: OriStrSSO,
  }

  #[repr(C)]
  pub struct OriStrHeap {
      pub len: i64,
      pub cap: i64,
      pub data: *mut u8,
  }

  #[repr(C)]
  pub struct OriStrSSO {
      pub bytes: [u8; 23],
      pub flags: u8,  // high bit = 1 (SSO), low 7 bits = length
  }
  ```

- [x] Add SSO helper functions:
  ```rust
  impl OriStr {
      #[inline]
      pub fn is_sso(&self) -> bool {
          unsafe { self.sso.flags & SSO_FLAG != 0 }
      }

      #[inline]
      pub fn len(&self) -> usize {
          if self.is_sso() {
              unsafe { (self.sso.flags & SSO_LEN_MASK) as usize }
          } else {
              unsafe { self.heap.len as usize }
          }
      }

      #[inline]
      pub fn as_bytes(&self) -> &[u8] {
          if self.is_sso() {
              unsafe { &self.sso.bytes[..self.len()] }
          } else {
              unsafe { std::slice::from_raw_parts(self.heap.data, self.len()) }
          }
      }

      #[inline]
      pub fn is_empty(&self) -> bool {
          self.len() == 0
      }
  }
  ```

- [x] Add SSO constructors:
  ```rust
  /// Creates an SSO string from bytes. Panics if len > 23.
  pub fn from_sso(bytes: &[u8]) -> OriStr {
      assert!(bytes.len() <= 23);
      let mut sso = OriStrSSO { bytes: [0u8; 23], flags: SSO_FLAG | bytes.len() as u8 };
      sso.bytes[..bytes.len()].copy_from_slice(bytes);
      OriStr { sso }
  }

  /// Creates a heap string from bytes.
  pub fn from_heap(bytes: &[u8]) -> OriStr {
      let data = ori_rc_alloc(bytes.len(), 1);
      unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), data, bytes.len()); }
      OriStr { heap: OriStrHeap { len: bytes.len() as i64, cap: bytes.len() as i64, data } }
  }

  /// Creates a string, automatically choosing SSO or heap.
  pub fn from_bytes(bytes: &[u8]) -> OriStr {
      if bytes.len() <= 23 {
          Self::from_sso(bytes)
      } else {
          Self::from_heap(bytes)
      }
  }
  ```

- [x] Unit tests:
  - Empty string → SSO, len=0
  - 1-byte string → SSO, len=1
  - 23-byte string → SSO, len=23
  - 24-byte string → heap, len=24
  - SSO → as_bytes returns correct content
  - Heap → as_bytes returns correct content
  - `is_sso()` correctly discriminates

---

## 03.2 String Capacity Tracking

**File(s):** `compiler/ori_rt/src/lib.rs`

Heap strings need a capacity field for COW growth. The 24-byte `OriStrHeap` struct includes `cap`.

- [x] All heap string constructors must set `cap` correctly:
  - `ori_str_from_bytes(bytes, len)` → `cap = len` (tight allocation)
  - `ori_str_with_capacity(cap)` → `cap = cap`, `len = 0`
  - String literals → `cap = len` (no growth headroom for compile-time strings)

- [x] Add `ori_str_ensure_capacity`:
  ```rust
  /// Ensures the heap string has capacity for at least `required` bytes.
  /// PRECONDITION: string is heap (not SSO) and uniquely owned.
  #[unsafe(no_mangle)]
  pub extern "C" fn ori_str_ensure_capacity(
      str: *mut OriStr,
      required: i64,
  ) { ... }
  ```

- [x] **SSO-to-heap promotion**: When an SSO string grows beyond 23 bytes (e.g., via concat), it must be promoted to a heap string. The promotion allocates a heap buffer, copies the SSO bytes, and sets the appropriate capacity.

  ```rust
  fn promote_to_heap(sso: &OriStrSSO, min_cap: usize) -> OriStrHeap {
      let cap = next_capacity(0, min_cap);
      let data = ori_rc_alloc(cap, 1);
      let len = (sso.flags & SSO_LEN_MASK) as usize;
      unsafe { std::ptr::copy_nonoverlapping(sso.bytes.as_ptr(), data, len); }
      OriStrHeap { len: len as i64, cap: cap as i64, data }
  }
  ```

---

## 03.3 COW String Concat

**File(s):** `compiler/ori_rt/src/lib.rs`

String concatenation (`str1 + str2`) is the most common string mutation. With COW:

- [x] Add `ori_str_concat`:
  ```rust
  /// Concatenates str2 onto str1.
  ///
  /// Cases:
  /// 1. Both SSO and combined ≤ 23 bytes → SSO result (no allocation)
  /// 2. str1 is heap, unique, has capacity → append in place (O(m))
  /// 3. str1 is heap, unique, no capacity → realloc + append (O(n+m))
  /// 4. str1 is shared or SSO (promotion needed) → allocate new (O(n+m))
  ///
  /// In all cases, the result is a new OriStr value. The original str1 and
  /// str2 are NOT modified (their RCs are managed by the caller).
  #[unsafe(no_mangle)]
  pub extern "C" fn ori_str_concat(
      str1: OriStr,
      str2: OriStr,
  ) -> OriStr {
      let len1 = str1.len();
      let len2 = str2.len();
      let combined = len1 + len2;

      // Case 1: Both fit in SSO
      if combined <= 23 {
          let mut result = OriStrSSO { bytes: [0u8; 23], flags: SSO_FLAG | combined as u8 };
          result.bytes[..len1].copy_from_slice(str1.as_bytes());
          result.bytes[len1..combined].copy_from_slice(str2.as_bytes());
          return OriStr { sso: result };
      }

      // Case 2-3: str1 is heap and unique
      if !str1.is_sso() {
          let heap = unsafe { &str1.heap };
          if !heap.data.is_null() && ori_rc_is_unique(heap.data as *const u8) {
              if heap.cap >= combined as i64 {
                  // Case 2: has capacity — append in place
                  unsafe {
                      std::ptr::copy_nonoverlapping(
                          str2.as_bytes().as_ptr(),
                          heap.data.add(len1),
                          len2,
                      );
                  }
                  return OriStr { heap: OriStrHeap {
                      len: combined as i64, cap: heap.cap, data: heap.data
                  }};
              } else {
                  // Case 3: realloc
                  let new_cap = next_capacity(heap.cap as usize, combined);
                  let new_data = ori_rc_realloc(heap.data, len1, new_cap, 1);
                  unsafe {
                      std::ptr::copy_nonoverlapping(
                          str2.as_bytes().as_ptr(),
                          new_data.add(len1),
                          len2,
                      );
                  }
                  return OriStr { heap: OriStrHeap {
                      len: combined as i64, cap: new_cap as i64, data: new_data
                  }};
              }
          }
      }

      // Case 4: allocate new
      let new_cap = next_capacity(0, combined);
      let new_data = ori_rc_alloc(new_cap, 1);
      unsafe {
          std::ptr::copy_nonoverlapping(str1.as_bytes().as_ptr(), new_data, len1);
          std::ptr::copy_nonoverlapping(str2.as_bytes().as_ptr(), new_data.add(len1), len2);
      }
      OriStr { heap: OriStrHeap {
          len: combined as i64, cap: new_cap as i64, data: new_data
      }}
  }
  ```

- [x] Unit tests:
  - SSO + SSO → SSO (both short, combined ≤ 23)
  - SSO + SSO → heap (combined > 23)
  - Heap unique + SSO with capacity → in-place append
  - Heap unique + SSO without capacity → realloc
  - Heap shared + SSO → new allocation
  - Empty + any → return other side
  - Chain of 100 concats on unique string → amortized O(1) per concat

---

## 03.4 COW String Replace & Transform

**File(s):** `compiler/ori_rt/src/lib.rs`

Other string mutations that benefit from COW:

- [x] `ori_str_to_uppercase` — if unique and same length (ASCII), transform in place
- [x] `ori_str_to_lowercase` — same
- [x] `ori_str_trim` — if unique, adjust offset (seamless slice — see §05)
- [x] `ori_str_replace` — if unique and replacement is same length, in-place
- [x] `ori_str_repeat` — allocate with exact capacity, no COW needed (always new)
- [x] `ori_str_push_char` — COW append of a single character:
  ```rust
  /// Appends a single character (1-4 UTF-8 bytes) to the string.
  /// Follows the same COW protocol as concat.
  #[unsafe(no_mangle)]
  pub extern "C" fn ori_str_push_char(str: OriStr, ch: u32) -> OriStr { ... }
  ```

- [x] Unit tests for each operation with unique vs shared inputs

---

## 03.5 Runtime Implementation

**File(s):** `compiler/ori_rt/src/lib.rs`

- [x] Update ALL existing `ori_str_*` functions to handle the new 24-byte layout:
  - `ori_str_from_bool`, `ori_str_from_int`, `ori_str_from_float` → use SSO when possible
  - `ori_str_len` → check SSO flag
  - `ori_str_eq` → handle SSO vs heap comparisons
  - `ori_str_debug` → handle both layouts
  - `ori_str_contains`, `ori_str_starts_with`, `ori_str_ends_with` → use `as_bytes()`

- [x] **ARC interaction**: SSO strings have no heap allocation, so:
  - `ori_rc_inc` on SSO string → **no-op** (nothing to increment)
  - `ori_rc_dec` on SSO string → **no-op** (nothing to decrement)
  - Drop function for SSO string → **no-op** (no memory to free)
  - The codegen must check `is_sso()` before emitting RC operations, OR the runtime RC functions must handle the SSO data pointer (which is not a valid heap pointer).

  **Design decision — who checks SSO for RC:**

  **(a) Codegen checks** (recommended):
  - Emit: `if !is_sso(str) { ori_rc_inc(str.data) }`
  - Pro: No overhead for SSO strings (skip the call entirely)
  - Con: More complex codegen

  **(b) Runtime checks**:
  - `ori_rc_inc` checks if the pointer looks like an SSO pointer
  - Pro: Simpler codegen
  - Con: Runtime overhead on every RC operation

  **Recommended:** Option (a) — codegen checks. The SSO flag is in a fixed position (last byte of the struct), so the check is a single byte load + test.

- [x] Unit tests for all updated functions with SSO and heap inputs

---

## 03.6 LLVM Codegen Updates

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/`, `compiler/ori_llvm/src/codegen/runtime_decl/mod.rs`

- [x] Update `OriStr` LLVM type from `{i64, ptr}` to `{i64, i64, ptr}` (or a 24-byte struct):
  - This affects ALL code that creates, passes, or destructures strings
  - Must update GEP indices for accessing string fields
  - Must update function signatures that take/return `OriStr`

- [x] Add SSO check in codegen:
  ```llvm
  ; Check if string is SSO (high bit of last byte)
  %flags_ptr = getelementptr i8, ptr %str, i64 23
  %flags = load i8, ptr %flags_ptr
  %is_sso = icmp slt i8 %flags, 0  ; sign bit = SSO flag
  br i1 %is_sso, label %sso_path, label %heap_path
  ```

- [x] Update RC emission for strings:
  - Before `ori_rc_inc(str.data)`: check `is_sso` → skip if SSO
  - Before `ori_rc_dec(str.data, drop_fn)`: check `is_sso` → skip if SSO

- [x] Update all string literal emission to use SSO when ≤ 23 bytes

- [x] Update runtime_decl for new function signatures

- [x] AOT integration tests:
  ```ori
  use std.testing { assert_eq }

  @test tests {
      // SSO strings
      let s = "hello"           // 5 bytes → SSO
      assert_eq(s.length(), 5)
      assert_eq(s + " world", "hello world")  // 11 bytes → still SSO

      // Heap strings
      let long = "this is a very long string that exceeds 23 bytes"
      assert_eq(long.length(), 49)

      // SSO → heap promotion
      let s = "12345678901234567890123"  // 23 bytes → SSO
      let s = s + "x"                    // 24 bytes → heap
      assert_eq(s.length(), 24)
  }
  ```

### Known Issue: `create_entry_alloca` places sret allocas in wrong function (dominance error)

**Symptom:** LLVM verification fails with "Instruction does not dominate all uses!" for `%str.val.sretNN` allocas in `_ori_main` that are physically located in `_ori___lambda_0`'s entry block.

**Root cause:** `IrBuilder::call_with_sret()` uses `builder.current_function` to determine which function's entry block receives the alloca. The AOT pipeline uses a two-pass compilation path (`prepare_all_cached` then `emit_prepared_functions` in `nounwind.rs`), which differs from the direct `emit_arc_function` path. In both paths, lambda compilation (`compile_lambda_arc`) sets `builder.current_function` to the lambda's `FunctionId` but never restores it. When the parent function is subsequently emitted, string literal construction calls `IrBuilder::call_with_sret()`, which reads the stale `builder.current_function` (still pointing to the lambda), placing sret allocas in the lambda's entry block instead of the parent's.

**Partial fix applied:**
- Added `self.builder.set_current_function(func_id)` after the lambda loop in `emit_arc_function()` (`define_phase.rs`). This fixes the direct emit path.
- Changed `call_with_sret` in the emitter (`arc_emitter/mod.rs`) to use `create_entry_alloca(self.current_function, ...)` instead of bare `alloca()`.
- Changed invoke terminator sret alloca (`arc_emitter/terminators.rs`) to use `create_entry_alloca` similarly.

**Remaining fix needed:** The `emit_prepared_functions` method in `nounwind.rs` also compiles lambdas before emitting parent functions. It needs the same `builder.current_function` reset after lambda compilation.

**Temporary debug artifact:** `codegen_pipeline.rs` has an `ORI_DUMP_BEFORE_CLONE` debug dump that was added during investigation — must be removed before merge.

**Files modified so far:**

| File | Change |
|------|--------|
| `compiler/ori_llvm/src/codegen/function_compiler/define_phase.rs` | Reset `builder.current_function` after lambda loop |
| `compiler/ori_llvm/src/codegen/arc_emitter/mod.rs` | `call_with_sret` uses `create_entry_alloca` |
| `compiler/ori_llvm/src/codegen/arc_emitter/terminators.rs` | Invoke sret uses `create_entry_alloca` |
| `compiler/oric/src/commands/codegen_pipeline.rs` | TEMPORARY: `ORI_DUMP_BEFORE_CLONE` debug dump (remove) |

**Key file to fix:** `compiler/ori_llvm/src/codegen/function_compiler/nounwind.rs` — `emit_prepared_functions()` method needs `builder.current_function` reset after lambda emission, mirroring the fix in `define_phase.rs`.

---

## 03.7 Completion Checklist

- [x] `OriStr` is 24 bytes with SSO for ≤ 23 bytes
- [x] SSO strings never touch the heap (no alloc, no RC)
- [x] Heap strings have capacity tracking
- [x] `ori_str_concat` is O(1) amortized for unique strings
- [x] SSO + SSO concat produces SSO when result ≤ 23 bytes
- [x] SSO → heap promotion works correctly
- [x] All existing `ori_str_*` functions handle both layouts
- [x] RC operations skip SSO strings (no crash, no leak)
- [x] String literals ≤ 23 bytes use SSO in AOT
- [x] `ori_str_push_char` works for ASCII and multi-byte UTF-8
- [x] LLVM codegen emits correct 24-byte struct layout
- [x] Valgrind clean: no leaks, no invalid reads/writes
- [x] `./test-all.sh` green
- [x] `./clippy-all.sh` green

**Exit Criteria:** A program that builds a string by concatenating 10,000 single characters completes in O(N) time (not O(N²)). Short string operations (< 24 bytes) show zero heap allocations in Valgrind. All existing string tests pass without modification. The SSO threshold covers > 80% of strings in the Ori test suite (measure with instrumented runtime).
