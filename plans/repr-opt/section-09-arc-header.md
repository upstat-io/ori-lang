---
section: "09"
title: "ARC Header Compression"
status: not-started
reviewed: false
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

- [ ] Generate per-width runtime functions (ABI matches existing i64 functions):
  ```rust
  // ori_rt additions — same 2-arg dec signature as ori_rc_dec:
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
              // Pad header up to payload alignment so that
              // base + padded_header is correctly aligned for the payload.
              // Without this, narrow headers (i8/i16) would misalign
              // payloads requiring 4- or 8-byte alignment.
              let padded_header = header_size.max(align);
              let total = size + padded_header;
              let layout = Layout::from_size_align(
                  total,
                  align.max(std::mem::align_of::<$width>()),
              ).unwrap();
              let base = alloc(layout);
              let data_ptr = base.add(padded_header);
              // Store refcount immediately before the payload.
              // This is where rc_inc/rc_dec will find it via .sub(1).
              let rc_ptr = (data_ptr as *mut $width).sub(1);
              *rc_ptr = 1;
              data_ptr
          }

          #[no_mangle]
          pub unsafe extern "C" fn paste!(ori_rc_inc_, $suffix)(data_ptr: *mut u8) {
              if data_ptr.is_null() { return; }
              let rc_ptr = (data_ptr as *mut $width).sub(1);
              let old = *rc_ptr;
              if old >= $max {
                  // Promote to immortal (never freed)
                  return;
              }
              *rc_ptr = old + 1;
          }
          /// Decrement refcount. On zero, call drop_fn which is responsible
          /// for decrementing child RC fields AND calling ori_rc_free_$suffix.
          ///
          /// CONTRACT (matches existing ori_rc_dec):
          ///   drop_fn does: (1) rc_dec each RC'd child field
          ///                 (2) ori_rc_free_$suffix(data_ptr, size, align)
          ///   rc_dec does NOT call free — that would be a double-free.
          #[no_mangle]
          pub unsafe extern "C" fn paste!(ori_rc_dec_, $suffix)(
              data_ptr: *mut u8,
              drop_fn: Option<extern "C" fn(*mut u8)>,
          ) {
              if data_ptr.is_null() { return; }
              let rc_ptr = (data_ptr as *mut $width).sub(1);
              let old = *rc_ptr;
              // Underflow protection — matches ori_rc_dec (rc/mod.rs).
              // NOT gated behind debug: one branch per dec (~0.5ns, always
              // not-taken). Catches double-free bugs before they corrupt memory.
              if old <= 0 {
                  rc_underflow_abort(data_ptr);
              }
              if old >= $max {
                  return; // immortal — never freed
              }
              let new = old - 1;
              *rc_ptr = new;
              if new == 0 {
                  if let Some(f) = drop_fn {
                      // Route through call_drop_fn (rc/mod.rs) — a
                      // catch_unwind wrapper that aborts on panic.
                      // ori_rc_dec_$suffix is declared nounwind in LLVM
                      // IR, so unwinding through it is UB. This guard
                      // matches the existing ori_rc_dec contract.
                      call_drop_fn(f, data_ptr);
                  }
                  // drop_fn calls ori_rc_free_$suffix internally.
                  // If drop_fn is None, the memory leaks — this should
                  // not happen in well-formed programs (every RC type
                  // must have a drop function).
              }
          }

          /// Free an RC allocation. Called by drop functions (generated by
          /// DropFunctionGenerator), NOT by ori_rc_dec directly.
          ///
          /// Reconstructs the original allocation base by reversing the
          /// padded_header computation from ori_rc_alloc_$suffix.
          #[no_mangle]
          pub unsafe extern "C" fn paste!(ori_rc_free_, $suffix)(
              data_ptr: *mut u8, size: usize, align: usize,
          ) {
              let header_size = std::mem::size_of::<$width>();
              let padded_header = header_size.max(align);
              let base = data_ptr.sub(padded_header);
              let total = size + padded_header;
              let layout = Layout::from_size_align_unchecked(
                  total,
                  align.max(std::mem::align_of::<$width>()),
              );
              dealloc(base, layout);
          }
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
- [ ] `./clippy-all.sh` green
- [ ] `./diagnostics/valgrind-aot.sh` clean
- [ ] Memory measurement: per-object overhead reduced from 8 bytes to 1-4 bytes for bounded types

**Exit Criteria:** A program that creates 1M small heap objects with refcount ≤ 2 uses `ori_rc_alloc_i8` (1-byte header), verified by `grep "ori_rc_alloc_i8"` in LLVM IR. Total memory reduced by ~7MB vs. current implementation. Valgrind clean.
