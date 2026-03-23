---
section: "10"
title: "Thread-Local Non-Atomic ARC"
status: not-started
reviewed: false
third_party_review:
  status: findings
  updated: 2026-03-23
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
      func: &ArcFunction,
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

**File(s):** `compiler/ori_rt/src/rc/nonatomic.rs` (new file inside `rc/` module)

**Module placement:** Must live inside `rc/` (e.g., `rc/nonatomic.rs` with `mod nonatomic;` in `rc/mod.rs`) to access `call_drop_fn` and `rc_underflow_abort` which are `pub(super)`. Note: `ori_rt` allows `unsafe` (it is NOT in the `#![deny(unsafe_code)]` list). Every `unsafe` block MUST have a `// SAFETY:` comment.

**Risk warning:** Non-atomic RC on a value that IS actually shared across threads causes data races (undefined behavior). The soundness of this entire section depends on §08's escape analysis and §10.1's thread escape analysis being correct. If the analysis is unsound, this creates UB that Valgrind (memcheck) will not catch — only helgrind/TSAN will. This section must be gated on §08 being fully verified first.

- [ ] Implement non-atomic RC operations:
  ```rust
  #[no_mangle]
  pub unsafe extern "C" fn ori_rc_inc_nonatomic(data_ptr: *mut u8) {
      if data_ptr.is_null() { return; }
      // SAFETY: data_ptr was returned by ori_rc_alloc; strong_count is at data_ptr - 8.
      let rc_ptr = data_ptr.sub(8).cast::<i64>();
      let count = *rc_ptr;  // plain load (no atomic)
      if count >= MAX_REFCOUNT {
          std::process::abort();
      }
      *rc_ptr = count + 1;  // plain store (no atomic)
  }

  #[no_mangle]
  pub unsafe extern "C" fn ori_rc_dec_nonatomic(
      data_ptr: *mut u8,
      drop_fn: Option<extern "C" fn(*mut u8)>,
  ) {
      if data_ptr.is_null() { return; }
      // SAFETY: data_ptr was returned by ori_rc_alloc; strong_count is at data_ptr - 8.
      let rc_ptr = data_ptr.sub(8).cast::<i64>();
      let count = *rc_ptr;  // plain load (no atomic)
      // Underflow protection — matches ori_rc_dec (rc/mod.rs).
      // Always-on, not debug-only. Catches double-free bugs.
      if count <= 0 {
          rc_underflow_abort(data_ptr);
      }
      *rc_ptr = count - 1;  // plain store (no atomic)
      if count == 1 {
          // Last reference — drop via abort-on-panic guard.
          // ori_rc_dec_nonatomic is nounwind; unwinding through it is UB.
          if let Some(f) = drop_fn {
              call_drop_fn(f, data_ptr);
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
          // Call ori_rc_inc_$width / ori_rc_dec_$width (§09 width-suffixed)
          // ABI: inc(data_ptr), dec(data_ptr, drop_fn) — matches existing contract
          emit_atomic_rc(width);
      }
      RcStrategy::NonAtomic { width } => {
          // Call ori_rc_inc_nonatomic_$width / ori_rc_dec_nonatomic_$width
          // Same 2-arg dec ABI: dec(data_ptr, drop_fn)
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

**Test matrix for §10 (write failing tests FIRST, verify they fail, then implement):**

| Program pattern | Expected RC variant | Semantic pin |
|---|---|---|
| Single-threaded program with many RC operations | All `ori_rc_inc_nonatomic` / `ori_rc_dec_nonatomic` calls | Yes — zero `ori_rc_inc` (atomic) in LLVM IR |
| Single-threaded program: no `spawn()`, no `channel()` calls | ALL values use non-atomic RC (whole-program optimization) | Yes — zero atomic RC ops in IR |
| Multi-threaded: `spawn()` captures a list | Captured list uses atomic RC; local list in spawned fn uses non-atomic | Yes — split atomic/non-atomic |
| `chan.send(value)` — value crosses channel | `value` promoted to atomic RC before send | Yes — `ori_rc_inc` (atomic) before send |
| Value created AFTER `spawn()` in spawned closure | Non-atomic (thread-local to that thread) | Yes — post-spawn local stays non-atomic |
| Width-specific non-atomic: bounded value + thread-local | `ori_rc_inc_nonatomic_i8` / `ori_rc_dec_nonatomic_i8` | Yes — narrow + non-atomic combined |
| Non-atomic RC correct behavior: single-thread dec to 0 | Value dropped correctly (same semantics as atomic) | Yes — correctness equivalence |

- [ ] Write failing test matrix BEFORE implementation (verify tests fail with current all-atomic codegen)
- [ ] Single-threaded programs: ALL RC operations use `ori_rc_*_nonatomic` variants
- [ ] Multi-threaded programs: only thread-shared values use atomic RC
- [ ] Channel sends correctly mark values as thread-shared
- [ ] Spawn captures correctly mark captured values as thread-shared
- [ ] Width-specific non-atomic variants: `ori_rc_inc_nonatomic_i8`, `ori_rc_dec_nonatomic_i8`, `ori_rc_inc_nonatomic_i16`, `ori_rc_dec_nonatomic_i16` (combines with §09)
- [ ] Add semantic pin test: a single-threaded program produces ZERO atomic RC operations in LLVM IR (all ops are `ori_rc_*_nonatomic`). This test can ONLY pass with thread-local analysis enabled.
- [ ] Non-atomic RC operations are measurably faster (benchmark ≥ 20% improvement in RC-heavy workloads)
- [ ] `./diagnostics/dual-exec-verify.sh` passes — non-atomic RC produces identical behavior to atomic RC
- [ ] Extend `diagnostics/valgrind-aot.sh` to accept an optional `--tool=helgrind` passthrough flag:
  - Add `--helgrind` flag to the script: when present, pass `--tool=helgrind --fair-sched=yes` to valgrind instead of `--tool=memcheck`
  - This is a concrete shell script change, not "invoke manually"
  - File: `diagnostics/valgrind-aot.sh` (modify the valgrind invocation line)
- [ ] Run helgrind on AOT binaries compiled from multi-threaded Ori programs (channel + spawn patterns): `./diagnostics/valgrind-aot.sh --helgrind tests/valgrind/threads/`
- [ ] Create `tests/valgrind/threads/` directory with at minimum:
  - `thread_local_only.ori` — single-threaded program with many RC operations → no helgrind races
  - `channel_send.ori` — program that sends values through a channel → helgrind must find no races
- [ ] `./test-all.sh` green
- [ ] `./clippy-all.sh` green
- [ ] `./diagnostics/dual-exec-verify.sh` passes

**Exit Criteria:** A single-threaded benchmark program shows 0 atomic RC operations in LLVM IR (all `ori_rc_*_nonatomic`). Performance benchmark shows ≥20% improvement in RC-heavy workloads vs. atomic-only baseline.

---

## 10.R Third Party Review Findings

- [ ] `[TPR-10-001][major]` `section-10-thread-local-arc.md:117-148` — **Non-atomic RC has no debug-mode safety net; analysis wrong → silent UB.** The `ori_rc_inc_nonatomic` / `ori_rc_dec_nonatomic` functions use plain loads/stores (lines 120, 124: `*rc_ptr`). If thread escape analysis (§08+§10.1) is unsound for any value, concurrent access produces data race UB. The plan acknowledges this risk (line 111) but proposes no runtime fallback — only helgrind testing as a detection tool. No mechanism exists to verify at runtime that a value classified as thread-local is actually single-threaded. **Action:** Add a debug-mode `#[cfg(debug_assertions)]` per-allocation flag that records the RC mode (atomic vs non-atomic). Assert on mismatched access (e.g., `ori_rc_inc` called on an allocation marked non-atomic). Zero cost in release builds. This catches analysis bugs during development before they become silent data races in production. Consensus: 3/3 reviewers.
