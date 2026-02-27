---
section: "10"
title: "Thread-Local Non-Atomic ARC"
status: not-started
goal: "Use non-atomic (plain load/store) reference counting for values that are provably never shared across threads, eliminating atomic operation overhead"
inspired_by:
  - "Swift isUniquelyReferenced + non-atomic path (stdlib/public/core/ManagedBuffer.swift)"
  - "Rust Rc vs Arc distinction (library/alloc/src/rc.rs vs library/alloc/src/sync.rs)"
  - "CPython GIL-protected refcounting (Objects/object.c)"
depends_on: ["08", "09"]
sections:
  - id: "10.1"
    title: "Thread Escape Analysis"
    status: not-started
  - id: "10.2"
    title: "Non-Atomic RC Runtime"
    status: not-started
  - id: "10.3"
    title: "Migration Fence"
    status: not-started
  - id: "10.4"
    title: "Completion Checklist"
    status: not-started
---

# Section 10: Thread-Local Non-Atomic ARC

**Status:** Not Started
**Goal:** Reference count operations on values that never cross thread boundaries use plain `load`/`store` instead of atomic `fetch_add`/`fetch_sub`. On x86-64, this is 5-20× faster per operation (atomic operations involve cache line synchronization even on single-core). For programs that don't use concurrency, ALL RC operations become non-atomic.

**Context:** Currently, `ori_rt` uses `AtomicI64` with `Relaxed`/`Release`/`Acquire` ordering for all RC operations. This is correct for thread-shared values but wasteful for thread-local ones. Most values in most programs never cross thread boundaries — they're created, used, and freed within a single thread.

Rust solved this by having two types: `Rc` (non-atomic, thread-local) and `Arc` (atomic, thread-safe). Ori doesn't expose this distinction to the programmer — the compiler decides automatically.

**Reference implementations:**
- **Rust** `library/alloc/src/rc.rs` vs `library/alloc/src/sync.rs`: `Rc` uses `Cell<usize>` (non-atomic), `Arc` uses `AtomicUsize`. Programmer chooses.
- **Swift**: All RC is atomic by default, but `isKnownUniquelyReferenced()` enables COW without RC overhead. No automatic non-atomic promotion.
- **CPython**: GIL-protected — all RC is effectively non-atomic because only one thread runs at a time.

**Depends on:** §08 (escape analysis to determine thread-locality), §09 (header compression — non-atomic and atomic headers differ).

---

## 10.1 Thread Escape Analysis

**File(s):** `compiler/ori_repr/src/escape/thread.rs`

Extend escape analysis (§08) to track thread boundaries.

- [ ] Define thread escape:
  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub enum ThreadLocality {
      /// Value never crosses a thread boundary
      ThreadLocal,
      /// Value may be shared across threads
      ThreadShared,
      /// Unknown (conservative: treat as ThreadShared)
      Unknown,
  }
  ```

- [ ] Identify thread boundary operations:
  - `spawn()` — values captured by the spawned closure cross threads
  - `chan.send(value)` — value crosses thread via channel
  - Global mutable state (if Ori adds it) — shared by all threads
  - FFI calls with unknown thread behavior → conservative (ThreadShared)

- [ ] Propagate thread-locality:
  ```rust
  pub fn analyze_thread_locality(
      func: &CanFunction,
      escape_info: &EscapeInfo,
      pool: &Pool,
  ) -> FxHashMap<AllocId, ThreadLocality> {
      let mut locality = FxHashMap::default();

      for alloc in func.allocations() {
          if escape_info.escape_state(alloc) == EscapeState::NoEscape {
              // Non-escaping → definitely thread-local
              locality.insert(alloc, ThreadLocality::ThreadLocal);
              continue;
          }

          // Check if any escape path crosses a thread boundary
          let crosses_thread = escape_info.escape_paths(alloc)
              .any(|path| path.crosses_thread_boundary());

          locality.insert(alloc, if crosses_thread {
              ThreadLocality::ThreadShared
          } else {
              ThreadLocality::ThreadLocal
          });
      }

      locality
  }
  ```

- [ ] Whole-program optimization:
  - If the program has NO `spawn()` calls and NO channel operations → ALL values are ThreadLocal
  - This is detectable with a simple call graph scan
  - Enables ALL RC operations to be non-atomic for single-threaded programs

---

## 10.2 Non-Atomic RC Runtime

**File(s):** `compiler/ori_rt/src/lib.rs`

- [ ] Implement non-atomic RC operations:
  ```rust
  #[no_mangle]
  pub unsafe extern "C" fn ori_rc_inc_nonatomic(data_ptr: *mut u8) {
      if data_ptr.is_null() { return; }
      let rc_ptr = (data_ptr as *mut i64).sub(1);
      let count = *rc_ptr;  // plain load
      if count >= MAX_REFCOUNT {
          std::process::abort();
      }
      *rc_ptr = count + 1;  // plain store
  }

  #[no_mangle]
  pub unsafe extern "C" fn ori_rc_dec_nonatomic(
      data_ptr: *mut u8,
      drop_fn: Option<unsafe extern "C" fn(*mut u8)>,
  ) {
      if data_ptr.is_null() { return; }
      let rc_ptr = (data_ptr as *mut i64).sub(1);
      let count = *rc_ptr;  // plain load
      *rc_ptr = count - 1;  // plain store
      if count == 1 {
          // Last reference — drop
          if let Some(drop_fn) = drop_fn {
              drop_fn(data_ptr);
          }
      }
  }
  ```

- [ ] Also provide width-specific non-atomic variants:
  - `ori_rc_inc_nonatomic_i8`, `ori_rc_dec_nonatomic_i8`
  - `ori_rc_inc_nonatomic_i16`, `ori_rc_dec_nonatomic_i16`
  - Combines with §09 header compression

- [ ] LLVM codegen selects atomic vs. non-atomic based on `ReprPlan::rc_strategy()`:
  ```rust
  match repr_plan.rc_strategy(type_idx) {
      RcStrategy::None => { /* skip RC */ }
      RcStrategy::Atomic { width } => {
          // Call ori_rc_inc / ori_rc_dec (existing)
          emit_atomic_rc(width);
      }
      RcStrategy::NonAtomic { width } => {
          // Call ori_rc_inc_nonatomic / ori_rc_dec_nonatomic
          emit_nonatomic_rc(width);
      }
  }
  ```

---

## 10.3 Migration Fence

**File(s):** `compiler/ori_repr/src/arc_opt/migration.rs`

If a value transitions from thread-local to thread-shared (e.g., sent on a channel), the non-atomic refcount must be migrated to atomic.

- [ ] Design decision: **static vs. dynamic migration**

  **(a) Static (recommended)**: The compiler proves at compile time that a value is either always thread-local or always thread-shared. No runtime migration needed. If uncertain → atomic.

  **(b) Dynamic**: Store a flag in the header indicating atomic/non-atomic. When a value crosses a thread boundary, flip the flag and issue a memory fence. Adds 1 bit of overhead + branching on every RC operation.

  **Recommendation:** Option (a) for initial implementation. It's simpler, has zero runtime overhead, and covers the vast majority of cases. Option (b) is only needed if analysis misses important cases (measure first).

- [ ] If using static migration:
  - At channel send: if value is marked non-atomic → compile error or automatic promotion to atomic at compile time
  - At spawn: closure captures analyzed → all captured values promoted to atomic if needed
  - The promotion happens at compile time, not runtime

---

## 10.4 Completion Checklist

- [ ] Single-threaded programs: ALL RC operations use `ori_rc_*_nonatomic` variants
- [ ] Multi-threaded programs: only thread-shared values use atomic RC
- [ ] Channel sends correctly mark values as thread-shared
- [ ] Spawn captures correctly mark captured values as thread-shared
- [ ] Non-atomic RC operations are measurably faster (benchmark)
- [ ] No data races: `./scripts/valgrind-aot.sh` with `--tool=helgrind` clean
- [ ] `./test-all.sh` green
- [ ] `./scripts/dual-exec-verify.sh` passes

**Exit Criteria:** A single-threaded benchmark program shows 0 atomic RC operations in LLVM IR (all `ori_rc_*_nonatomic`). Performance benchmark shows ≥20% improvement in RC-heavy workloads vs. atomic-only baseline.
