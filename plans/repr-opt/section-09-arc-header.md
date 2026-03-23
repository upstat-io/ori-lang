---
section: "09"
title: "ARC Header Compression"
status: not-started
reviewed: false
goal: "Narrow the refcount header from V5 (32 bytes: data_size + elem_dec_fn + elem_count + strong_count) based on proven sharing bounds, reducing per-object memory overhead"
inspired_by:
  - "Swift refcount encoding (stdlib/public/SwiftShims/RefCount.h — uses bitfields)"
  - "Lean4 RC header layout (src/runtime/object.h)"
  - "CPython refcount (Include/object.h — Py_ssize_t)"
depends_on: ["02", "08"]
sections:
  - id: "09.1"
    title: "Sharing Bound Analysis"
    status: not-started
  - id: "09.2"
    title: "RC Width Selection"
    status: not-started
  - id: "09.3"
    title: "Runtime Polymorphism"
    status: not-started
  - id: "09.4"
    title: "Completion Checklist"
    status: not-started
---

# Section 09: ARC Header Compression

**Context:** The current runtime (`ori_rt`) unconditionally uses `i64` for the reference count, with `MAX_REFCOUNT = i64::MAX`. This is maximally safe but wasteful. The ARC pipeline already computes borrow/ownership information — extending this to bound the maximum refcount is a natural next step.

> **Warning (V5 header, 2026-03-22):** The RC header is now 32 bytes with 4 fields: `data_size` (i64), `elem_dec_fn` (ptr), `elem_count` (i64), `strong_count` (i64). This plan was drafted when the header had only 2 fields (data_size + strong_count). Any narrowing strategy must account for all 4 fields, not just the refcount. The `rc_ops!` macro code in Section 09.3 does not account for `elem_dec_fn` or `elem_count` and must be updated before implementation.

The challenge: RC header width must be a **compile-time** decision, but refcount values are **runtime** quantities. We need static analysis that proves an upper bound on the dynamic refcount.

**Reference implementations:**
- **Swift** `stdlib/public/SwiftShims/RefCount.h`: Encodes refcount + flags in a single 64-bit word using bitfields. Stores strong count, unowned count, and flags (immutable, immortal, deallocating) in one word.
- **Lean4** `src/runtime/object.h`: Uses 32-bit RC + tag in a single word. RC overflow bumps to "immortal" (never freed).
- **CPython**: Uses `Py_ssize_t` (platform word size) — simple but wastes memory.

**Depends on:** §02 (triviality — trivial values need no header), §08 (escape analysis — non-escaping values need no header).

---

## 09.1 Sharing Bound Analysis

**File(s):** `compiler/ori_repr/src/arc_opt/sharing.rs`

Compute an upper bound on the number of simultaneous references to each allocation.

- [ ] Define sharing bound:
  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub enum SharingBound {
      /// Exactly one reference at all times (unique ownership)
      Unique,
      /// At most N references (from call graph analysis)
      Bounded(u32),
      /// Unknown — could be arbitrarily many
      Unbounded,
  }
  ```

- [ ] Implement per-allocation sharing analysis:
  ```rust
  pub fn compute_sharing_bound(
      alloc: AllocId,
      arc_func: &ArcFunction,
      escape_info: &EscapeInfo,
      loop_info: &LoopInfo,
  ) -> SharingBound {
      // If value doesn't escape → Unique (refcount always 1)
      if escape_info.escape_state(alloc) == EscapeState::NoEscape {
          return SharingBound::Unique;
      }

      // Collect all rc_inc instructions for this allocation
      let incs = arc_func.rc_incs_for(alloc);
      let decs = arc_func.rc_decs_for(alloc);

      // SOUNDNESS: Static instruction count ≠ dynamic execution count.
      // A single rc_inc inside a loop body can execute N times, creating
      // up to N simultaneous references. We must bail to Unbounded if
      // any rc_inc is inside a loop or reachable via recursion.
      let any_inc_in_loop = incs.iter().any(|inc| {
          loop_info.is_in_loop(inc.block)
      });
      if any_inc_in_loop {
          // Future: if §03 range analysis can bound the loop iteration
          // count, we could use SharingBound::Bounded(inc_count * max_iters + 1).
          // For now, conservatively bail.
          return SharingBound::Unbounded;
      }

      // Similarly, if this function is (mutually) recursive and the
      // allocation flows into the recursive call, the static count
      // is not a sound bound.
      if arc_func.is_recursive() && arc_func.alloc_flows_to_self_call(alloc) {
          return SharingBound::Unbounded;
      }

      // SOUNDNESS: GlobalEscape means outside code can see this value
      // (returned, stored in global, captured in escaping closure) and may
      // create arbitrarily many additional references. Only ArgEscape
      // (borrowed by callees, never retained) allows a bounded count.
      if escape_info.escape_state(alloc) == EscapeState::GlobalEscape {
          return SharingBound::Unbounded;
      }

      // ArgEscape: value is passed to callees as borrowed only.
      // All rc_inc/dec are in straight-line code (no loops, no recursion).
      // If all inc/dec pairs are within the function → bounded.
      debug_assert_eq!(
          escape_info.escape_state(alloc),
          EscapeState::ArgEscape,
          "only ArgEscape should reach here"
      );
      let inc_count = incs.len();
      let dec_count = decs.len();
      if inc_count == dec_count {
          // All sharing is temporary → max refs = inc_count + 1 (initial)
          return SharingBound::Bounded(inc_count as u32 + 1);
      }

      // Callee retains some references we can't bound
      SharingBound::Unbounded
  }
  ```

- [ ] Interprocedural refinement:
  - If a function's escape summary says a parameter is `ArgEscape` (borrowed), it contributes 0 to the sharing count
  - If a value is passed to N functions as borrowed, sharing bound is still 1
  - If a value is cloned into a collection, sharing bound increases by collection capacity

---

## 09.2 RC Width Selection

**File(s):** `compiler/ori_repr/src/arc_opt/rc_width.rs`

Map sharing bounds to RC header widths.

- [ ] Implement width selection:
  ```rust
  pub fn select_rc_width(bound: SharingBound) -> Option<IntWidth> {
      match bound {
          SharingBound::Unique => None, // no RC header at all!
          SharingBound::Bounded(n) if n <= 127 => Some(IntWidth::I8),
          SharingBound::Bounded(n) if n <= 32_767 => Some(IntWidth::I16),
          SharingBound::Bounded(n) if n <= 2_147_483_647 => Some(IntWidth::I32),
          _ => Some(IntWidth::I64),
      }
  }
  ```

- [ ] Overflow behavior:
  - If at runtime the refcount exceeds the header width → must NOT silently overflow
  - Options:
    - **(a) Trap** (recommended for debug builds): panic on overflow
    - **(b) Promote to immortal** (Lean4 approach): set refcount to MAX → never freed (leaked)
    - **(c) Widen header** (complex): realloc with wider header at runtime
  - Recommendation: (a) for debug, (b) for release. The analysis proving the bound is sound, so overflow should never happen in correct programs. The trap catches bugs; the immortal fallback prevents crashes.

- [ ] Generate per-width runtime functions:
  ```rust
  // ori_rt additions for narrow refcount headers.
  //
  // CRITICAL: The current V5 header is 32 bytes with 4 fields:
  //   data_size (i64), elem_dec_fn (ptr), elem_count (i64), strong_count (i64)
  //
  // Narrowing ONLY applies to strong_count. The other 3 fields
  // (data_size, elem_dec_fn, elem_count) remain at their canonical widths.
  // This means the header size reduction is:
  //   i64 strong_count (8B) → i32 (4B): saves 4 bytes per allocation
  //   i64 strong_count (8B) → i16 (2B): saves 6 bytes per allocation
  //   i64 strong_count (8B) → i8  (1B): saves 7 bytes per allocation
  //
  // The narrow alloc/inc/dec/free functions must still lay out all 4
  // header fields, just with a narrower strong_count.
  extern "C" fn ori_rc_alloc_i8(size: usize, align: usize) -> *mut u8;
  extern "C" fn ori_rc_inc_i8(data_ptr: *mut u8);
  extern "C" fn ori_rc_dec_i8(data_ptr: *mut u8, drop_fn: Option<...>);
  extern "C" fn ori_rc_free_i8(data_ptr: *mut u8, size: usize, align: usize);
  // Similar for i16, i32.
  // drop_fn calls ori_rc_free_$suffix (generated by DropFunctionGenerator).
  ```

---

## 09.3 Runtime Polymorphism

**File(s):** `compiler/ori_rt/src/rc/narrow.rs` (new file inside `rc/` module)

The runtime must support multiple header widths without code bloat.

**Module placement:** The width-specific functions MUST live inside `rc/` (e.g., `rc/narrow.rs` with `mod narrow;` in `rc/mod.rs`). This is required because they call `call_drop_fn` and `rc_underflow_abort`, which are `pub(super)` — visible within `rc/` but not from `lib.rs` or other modules. Tests go in `rc/narrow/tests.rs` (sibling convention) if the file becomes a directory module, or in `rc/tests.rs` if narrow.rs stays as a leaf file and tests are co-located with the existing `rc/` test module.

**Risk warning:** The macro-generated RC operations below use raw pointer arithmetic and `unsafe`. Every `unsafe` block MUST have a `// SAFETY:` comment. The `padded_header` alignment logic is subtle — a bug causes data corruption in EVERY narrow-header allocation. Property-based testing with varying `(size, align)` pairs is essential. Note: `ori_rt` is a crate where `unsafe` IS allowed, so `#![deny(unsafe_code)]` does NOT apply here.

- [ ] **Design the V5-narrow header layout** (BEFORE writing any code — this is a design document, not optional):
  - Write a comment block in `rc/narrow.rs` specifying the exact memory layout for each narrow variant:
    - `V5HeaderI32`: `{ data_size: i64, elem_dec_fn: usize, elem_count: i64, strong_count: i32, _pad: u32 }` — padding after `i32` to maintain 8-byte natural alignment for the payload start
    - `V5HeaderI16`: `{ data_size: i64, elem_dec_fn: usize, elem_count: i64, strong_count: i16, _pad: [u8; 6] }` — 6 bytes padding to align payload to 8 bytes
    - `V5HeaderI8`: `{ data_size: i64, elem_dec_fn: usize, elem_count: i64, strong_count: i8, _pad: [u8; 7] }` — 7 bytes padding to align payload to 8 bytes
  - Verify header sizes: `V5HeaderI32 = 28+4 = 32 bytes`, `V5HeaderI16 = 26+6 = 32 bytes`, `V5HeaderI8 = 25+7 = 32 bytes` — ALL narrow header variants remain 32 bytes to match the canonical V5 header size. This is INTENTIONAL: keeping all headers the same size means the `elem_header.rs` accessors (`store_elem_dec_fn`, etc.) do NOT need to change their hardcoded offsets.
  - Confirm: since all narrow headers are still 32 bytes, `data_ptr.sub(8).cast::<i32/i16/i8>()` correctly locates `strong_count` (at the same offset as the canonical `i64` strong_count, just narrower). No offset changes needed in `elem_header.rs`.
  - Add `static_assert!` macros verifying sizes: `const _: () = assert!(std::mem::size_of::<V5HeaderI32>() == 32);` etc.

- [ ] Implement narrow RC operations for the V5 header layout:

  **CRITICAL DESIGN ISSUE:** The `rc_ops!` macro from the original plan assumes a simple single-field header where the refcount is immediately before the payload (`data_ptr.sub(1)`). This does NOT match the current V5 header layout:

  ```
  V5 Header (32 bytes):
  ┌──────────────┬──────────────┬──────────────┬──────────────┐
  │ data_size    │ elem_dec_fn  │ elem_count   │ strong_count │
  │ (i64)        │ (ptr)        │ (i64)        │ (i64)        │
  └──────────────┴──────────────┴──────────────┴──────────────┘
                                                 ↑ this field narrows
  ```

  The narrow-header approach must:
  1. Keep `data_size`, `elem_dec_fn`, `elem_count` at their current widths (they are semantically different from refcount)
  2. Only narrow `strong_count` (the last field before payload)
  3. The `rc_inc`/`rc_dec` functions locate `strong_count` at a fixed negative offset from `data_ptr` — this offset changes when `strong_count` is narrowed
  4. The V5 header accessor functions in `ori_rt/src/rc/elem_header.rs` (`store_elem_dec_fn`, `load_elem_dec_fn`, etc.) use hardcoded offsets that must be updated or parameterized

  **Implementation approach:**
  - Define a V5-narrow header struct for each width: `V5HeaderI32 { data_size: i64, elem_dec_fn: usize, elem_count: i64, strong_count: i32 }`
  - Generate `ori_rc_alloc_i32`, `ori_rc_inc_i32`, `ori_rc_dec_i32` that use the narrow header struct
  - Update `DropFunctionGenerator` in `ori_llvm` to emit calls to width-specific free functions
  - Add alignment padding between the narrow `strong_count` and payload to maintain payload alignment

  ```rust
  // Simplified sketch — the real implementation must handle V5 header fields:
  rc_narrow_ops!(i8, i8, i8::MAX);    // saves 7 bytes per allocation
  rc_narrow_ops!(i16, i16, i16::MAX); // saves 6 bytes per allocation
  rc_narrow_ops!(i32, i32, i32::MAX); // saves 4 bytes per allocation
  // i64 is the existing V5 implementation (no change)
  ```

- [ ] Atomic variants:
  - For thread-shared values (§10 determines this), use atomic operations
  - For thread-local values, use plain loads/stores (much faster)

---

## 09.4 Completion Checklist

**Test matrix for §09 (write failing tests FIRST, verify they fail, then implement):**

| Allocation pattern | Expected sharing bound | Expected RC width | Semantic pin |
|---|---|---|---|
| Non-escaping stack-promoted value (§08) | `Unique` | `None` (no header) | Yes — zero `ori_rc_alloc` |
| Local value passed as borrowed param, no loop | `Bounded(2)` | `I8` | Yes — `ori_rc_alloc_i8` in IR |
| Value in a loop body (inc inside loop) | `Unbounded` | `I64` | Yes — must NOT use narrow |
| Globally-escaping value (returned) | `Unbounded` | `I64` | Yes — must use standard i64 |
| Value shared with ≤ 127 callers in straight-line code | `Bounded(N ≤ 127)` | `I8` | Yes |
| Recursive function sharing its parameter | `Unbounded` | `I64` | Yes |

- [ ] Design the V5-narrow header layout document in `compiler/ori_rt/src/rc/narrow.rs` BEFORE writing any code:
  - WHERE: write a `// SAFETY:` comment block and `static_assert!` macros as specified in §09.3
  - All `unsafe` blocks in `narrow.rs` MUST have `// SAFETY:` comments per hygiene rules
- [ ] Add `static_assert!` for each narrow header size (`V5HeaderI32 == 32`, `V5HeaderI16 == 32`, `V5HeaderI8 == 32`)
- [ ] Sharing bound analysis computes `Unique` for non-escaping allocations
- [ ] Sharing bound analysis computes `Bounded(N)` for values with limited sharing
- [ ] RC header width matches sharing bound: Unique→none, ≤127→i8, ≤32K→i16
- [ ] Runtime has `ori_rc_alloc_i8`, `ori_rc_inc_i8`, `ori_rc_dec_i8` (and i16, i32) in `compiler/ori_rt/src/rc/narrow.rs`
- [ ] Header overflow in release mode → immortal (never freed, no crash)
- [ ] Header overflow in debug mode → trap with diagnostic
- [ ] `./test-all.sh` green
- [ ] `./clippy-all.sh` green
- [ ] `./diagnostics/valgrind-aot.sh` clean
- [ ] Memory measurement: per-object overhead reduced from 8 bytes to 1-4 bytes for bounded types

**Exit Criteria:** A program that creates 1M small heap objects with refcount ≤ 2 uses `ori_rc_alloc_i8` (1-byte header), verified by `grep "ori_rc_alloc_i8"` in LLVM IR. Total memory reduced by ~7MB vs. current implementation. Valgrind clean.
