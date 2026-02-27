---
section: "09"
title: "ARC Header Compression"
status: not-started
goal: "Narrow the refcount header from i64 (8 bytes) to i32/i16/i8 based on proven sharing bounds, reducing per-object memory overhead"
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

**Status:** Not Started
**Goal:** The 8-byte refcount header (`i64 strong_count`) is narrowed to 4/2/1 bytes when the compiler can prove that the maximum number of simultaneous references is bounded. Most values in practice have refcount ≤ 3. Using `i8` (max 127 refs) saves 7 bytes per object; for a list of 1M small structs, that's 7MB saved.

**Context:** The current runtime (`ori_rt`) unconditionally uses `i64` for the reference count, with `MAX_REFCOUNT = i64::MAX`. This is maximally safe but wasteful. The ARC pipeline already computes borrow/ownership information — extending this to bound the maximum refcount is a natural next step.

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
  ) -> SharingBound {
      // If value doesn't escape → Unique (refcount always 1)
      if escape_info.escape_state(alloc) == EscapeState::NoEscape {
          return SharingBound::Unique;
      }

      // Count static rc_inc operations on this allocation
      let inc_count = arc_func.count_rc_incs(alloc);

      // If all inc/dec pairs are within the function → bounded
      let dec_count = arc_func.count_rc_decs(alloc);
      if inc_count == dec_count {
          // All sharing is temporary → max refs = inc_count + 1 (initial)
          return SharingBound::Bounded(inc_count as u32 + 1);
      }

      // Value escapes with retained references
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
  // ori_rt additions:
  extern "C" fn ori_rc_inc_i8(data_ptr: *mut u8) { ... }
  extern "C" fn ori_rc_dec_i8(data_ptr: *mut u8, drop_fn: ...) { ... }
  extern "C" fn ori_rc_alloc_i8(size: usize, align: usize) -> *mut u8 { ... }
  // Similar for i16, i32
  ```

---

## 09.3 Runtime Polymorphism

**File(s):** `compiler/ori_rt/src/lib.rs`

The runtime must support multiple header widths without code bloat.

- [ ] Implement generic RC operations parameterized by header width:
  ```rust
  // Macro-generated for each width
  macro_rules! rc_ops {
      ($width:ty, $suffix:ident, $max:expr) => {
          #[no_mangle]
          pub unsafe extern "C" fn paste!(ori_rc_alloc_, $suffix)(
              size: usize, align: usize
          ) -> *mut u8 {
              let header_size = std::mem::size_of::<$width>();
              let total = size + header_size;
              let layout = Layout::from_size_align(total, align.max(header_size)).unwrap();
              let base = alloc(layout);
              // Initialize refcount to 1
              *(base as *mut $width) = 1;
              base.add(header_size)
          }

          #[no_mangle]
          pub unsafe extern "C" fn paste!(ori_rc_inc_, $suffix)(data_ptr: *mut u8) {
              if data_ptr.is_null() { return; }
              let header_size = std::mem::size_of::<$width>();
              let rc_ptr = (data_ptr as *mut $width).sub(1);
              let old = *rc_ptr;
              if old >= $max {
                  // Promote to immortal (never freed)
                  return;
              }
              *rc_ptr = old + 1;
          }
          // ... ori_rc_dec, ori_rc_free similarly
      };
  }

  rc_ops!(i8, i8, i8::MAX);
  rc_ops!(i16, i16, i16::MAX);
  rc_ops!(i32, i32, i32::MAX);
  // i64 is the existing implementation
  ```

- [ ] Atomic variants:
  - For thread-shared values (§10 determines this), use atomic operations
  - For thread-local values, use plain loads/stores (much faster)

---

## 09.4 Completion Checklist

- [ ] Sharing bound analysis computes `Unique` for non-escaping allocations
- [ ] Sharing bound analysis computes `Bounded(N)` for values with limited sharing
- [ ] RC header width matches sharing bound: Unique→none, ≤127→i8, ≤32K→i16
- [ ] Runtime has `ori_rc_alloc_i8`, `ori_rc_inc_i8`, `ori_rc_dec_i8` (and i16, i32)
- [ ] Header overflow in release mode → immortal (never freed, no crash)
- [ ] Header overflow in debug mode → trap with diagnostic
- [ ] `./test-all.sh` green
- [ ] `./scripts/valgrind-aot.sh` clean
- [ ] Memory measurement: per-object overhead reduced from 8 bytes to 1-4 bytes for bounded types

**Exit Criteria:** A program that creates 1M small heap objects with refcount ≤ 2 uses `ori_rc_alloc_i8` (1-byte header), verified by `grep "ori_rc_alloc_i8"` in LLVM IR. Total memory reduced by ~7MB vs. current implementation. Valgrind clean.
