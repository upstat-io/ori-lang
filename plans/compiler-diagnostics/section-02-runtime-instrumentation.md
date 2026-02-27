---
section: "02"
title: Runtime RC Instrumentation
status: complete
goal: "ORI_TRACE_RC event logging + leak attribution + runtime assertions in ori_rt — zero overhead when disabled"
inspired_by:
  - "Swift swift-inspect (live process RC introspection)"
  - "Koka kklib/src/refcount.c (inline RC assertions)"
  - "Roc ROC_PRINT_IR_AFTER_REFCOUNT (RC pass visibility)"
depends_on: []
sections:
  - id: "02.1"
    title: "RC Event Tracing"
    status: complete
  - id: "02.2"
    title: "Leak Attribution"
    status: complete
  - id: "02.3"
    title: "Runtime Assertion Mode"
    status: complete
  - id: "02.4"
    title: "Release-Mode Underflow Detection"
    status: complete
  - id: "02.5"
    title: "Completion Checklist"
    status: complete
---

# Section 02: Runtime RC Instrumentation

**Status:** Not Started
**Goal:** Four runtime enhancements in `ori_rt` that provide deep visibility into RC lifecycle — event-level tracing, leak source attribution, assertion-mode guards, and underflow protection — all gated behind environment variables with zero overhead when disabled.

**Context:** Today's RC debugging is blind: `ORI_CHECK_LEAKS` counts the imbalance but can't say *which* allocation leaked or *why*. When `push_cow` internally frees old data and then codegen also emits `ori_rc_dec` on the same pointer, the only way to find the double-free is Valgrind (20-50x slower) or reading C source + LLVM IR by hand. RC event tracing makes the retain/release sequence visible in real-time.

**Reference implementations:**
- **Swift** `tools/swift-inspect/`: Live process introspection of ARC state, metadata caches, conformance tables
- **Koka** `kklib/src/refcount.c`: Inline `kk_assert_internal(kk_block_is_valid(child))` assertions in every RC operation
- **Rust** `Arc::drop`: `debug_assert!(old_count >= 1)` — catches underflow in debug builds

**Depends on:** Nothing (but Section 01 scripts will gain `--rc-trace` flags once this lands).

---

## 02.1 RC Event Tracing (`ORI_TRACE_RC`)

**File(s):** `compiler/ori_rt/src/lib.rs` (modify existing RC functions)

Add optional event logging to every RC operation. When `ORI_TRACE_RC=1`, each `alloc/inc/dec/free` prints a structured line to stderr:

```
[RC] alloc   0x7f1234 size=24 align=8 → rc=1 (live=1)
[RC] inc     0x7f1234 → rc=2 (live=1)
[RC] dec     0x7f1234 → rc=1 (live=1)
[RC] dec     0x7f1234 → rc=0 FREE (live=0)
[RC] free    0x7f1234 size=24 align=8 (live=0)
```

- [x] Add `ORI_TRACE_RC` env var check (cached in `OnceLock`, same pattern as `ORI_CHECK_LEAKS`) (2026-02-27)
  ```rust
  fn rc_trace_enabled() -> bool {
      static ENABLED: OnceLock<bool> = OnceLock::new();
      *ENABLED.get_or_init(|| {
          std::env::var("ORI_TRACE_RC")
              .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
              .unwrap_or(false)
      })
  }
  ```
- [x] Add trace logging to `ori_rc_alloc()`: print pointer, size, align, initial rc=1, live count (2026-02-27)
- [x] Add trace logging to `ori_rc_inc()`: print pointer, new refcount, live count (2026-02-27)
- [x] Add trace logging to `ori_rc_dec()`: print pointer, new refcount, "FREE" if reaching 0, live count (2026-02-27)
- [x] Add trace logging to `ori_rc_free()`: print pointer, size, align, live count (2026-02-27)
- [x] Ensure trace writes are atomic (single `eprintln!` call, not multiple `eprint!`) (2026-02-27)
- [x] Add `ORI_TRACE_RC=verbose` mode that also prints a backtrace for each operation (using `std::backtrace::Backtrace`) (2026-02-27)
- [x] Verify zero overhead when disabled: `OnceLock` check is a single atomic load after first call (2026-02-27)
- [x] Test: run a simple program with `ORI_TRACE_RC=1`, verify the alloc→inc→dec→free sequence is complete and balanced (2026-02-27)

---

## 02.2 Leak Attribution (Allocation-Site Tracking)

**File(s):** `compiler/ori_rt/src/lib.rs`

When `ORI_CHECK_LEAKS=1` detects leaked allocations, report *which* allocations leaked. Each `ori_rc_alloc` is assigned a monotonic ID, and unfreed IDs are printed at exit.

- [x] Add `RC_ALLOC_COUNTER: AtomicI64` — monotonic allocation counter (2026-02-27)
- [x] Add `RC_ALLOC_REGISTRY: Mutex<HashMap<*mut u8, (i64, usize, usize)>>` — maps data_ptr → (alloc_id, size, align) (2026-02-27)
  - Only active when `ORI_CHECK_LEAKS=1` (gated by same env check)
  - Behind `#[cfg(debug_assertions)]` to avoid any release overhead
- [x] On `ori_rc_alloc()`: register (alloc_id, size, align) in registry (2026-02-27)
- [x] On `ori_rc_free()`: remove from registry (2026-02-27)
- [x] On program exit (in `ori_run_main`): if live_count > 0, iterate registry and print: (2026-02-27)
  ```
  ori: 2 RC allocation(s) not freed:
    #42: ptr=0x7f1234 size=24 align=8 (unfreed)
    #67: ptr=0x7f5678 size=16 align=8 (unfreed)
  ```
- [x] Test: write a program with a deliberate leak, verify the attribution report identifies it (2026-02-27)

---

## 02.3 Runtime Assertion Mode (`ORI_RT_DEBUG`)

**File(s):** `compiler/ori_rt/src/lib.rs`

Add a comprehensive assertion mode that catches bugs at the earliest possible point. When `ORI_RT_DEBUG=1`, the runtime validates every RC operation:

- [x] Add `ORI_RT_DEBUG` env var check (cached `OnceLock`) (2026-02-27)
- [x] **RC header validation**: Before any RC operation, check that the refcount at `data_ptr - 8` is a plausible value (> 0, < 1_000_000) (2026-02-27)
  - Catches: use-after-free (refcount was overwritten), corruption, misaligned pointers
- [x] **Null sentinel guard**: COW functions that receive null data_ptr log a warning (empty list mutation) (2026-02-27)
- [x] **Bounds checking for list operations**: Validate index in `ori_list_set_cow`, `ori_list_insert_cow`, `ori_list_remove_cow` (2026-02-27)
  - Note: `ori_list_get` doesn't exist in runtime (indexing done by LLVM GEP). COW functions already had silent bounds checks; debug mode now makes them visible via `rt_debug_bounds_warning`.
- [x] **Double-free guard**: Track freed pointers in a `HashSet<usize>` (debug only), abort if `ori_rc_free` is called on an already-freed pointer (2026-02-27)
- [x] Ensure all guards are gated behind the env var check — zero overhead when disabled (2026-02-27)
- [x] Test: freed-set mechanism detects freed pointers; validation passes on live pointers; 5 new tests in `ori_rt/src/tests.rs` (2026-02-27)

---

## 02.4 Release-Mode Underflow Detection

**File(s):** `compiler/ori_rt/src/lib.rs`

The current `debug_assert!(old_count >= 1)` in `ori_rc_dec` only fires in debug builds. In release builds, decrementing a zero refcount silently underflows to `i64::MAX`, creating a phantom "live" allocation that will never be freed. Add a lightweight underflow guard for release mode.

- [x] In `ori_rc_dec()`, after the atomic decrement: if `prev <= 0`, abort via `rc_underflow_abort()` (2026-02-27)
- [x] This is NOT gated behind a flag — it's a safety net for all builds (one branch per dec, ~0.5ns overhead) (2026-02-27)
- [x] `rc_underflow_abort()` is `#[cold] #[inline(never)]` so the abort path doesn't pollute instruction cache (2026-02-27)
- [x] Test: subprocess test `rc_underflow_aborts_process` writes 0 to refcount header, calls `ori_rc_dec`, verifies SIGABRT (2026-02-27)

---

## 02.5 Completion Checklist

- [x] `ORI_TRACE_RC=1` produces complete alloc→inc→dec→free traces for simple programs (2026-02-27)
- [x] `ORI_TRACE_RC=verbose` adds backtraces to each operation (2026-02-27)
- [x] `ORI_CHECK_LEAKS=1` now prints allocation IDs and sizes for unfreed allocations (2026-02-27)
- [x] `ORI_RT_DEBUG=1` catches use-after-free and corrupted RC headers (2026-02-27)
- [x] Release-mode underflow detection catches `ori_rc_dec` on zero-refcount allocations (2026-02-27)
- [x] Zero overhead: `rt_debug_enabled()` is `#[inline]` OnceLock check (~1ns atomic load); heavy logic in `#[cold]` functions; double-free tracking `#[cfg(debug_assertions)]` only (2026-02-27)
- [x] All new runtime behavior tested with Rust unit tests in `ori_rt/src/tests.rs` — 140 tests (2026-02-27)
- [x] `./test-all.sh` green — 10,372 tests, 0 failures (2026-02-27)
- [x] Updated `diagnostics/diagnose-aot.sh` with `--rc-trace` flag for `ORI_TRACE_RC` (2026-02-27)

**Exit Criteria:** Running `ORI_TRACE_RC=1 ORI_CHECK_LEAKS=1 ./binary` on the `push(3).reverse()` test case produces an RC event trace that clearly shows whether the data pointer is freed twice or not freed at all. The trace is machine-readable (each line starts with `[RC]`) and human-readable (includes addresses, refcounts, live counts).
