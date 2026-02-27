---
section: "02"
title: Runtime RC Instrumentation
status: not-started
goal: "ORI_TRACE_RC event logging + leak attribution + runtime assertions in ori_rt — zero overhead when disabled"
inspired_by:
  - "Swift swift-inspect (live process RC introspection)"
  - "Koka kklib/src/refcount.c (inline RC assertions)"
  - "Roc ROC_PRINT_IR_AFTER_REFCOUNT (RC pass visibility)"
depends_on: []
sections:
  - id: "02.1"
    title: "RC Event Tracing"
    status: not-started
  - id: "02.2"
    title: "Leak Attribution"
    status: not-started
  - id: "02.3"
    title: "Runtime Assertion Mode"
    status: not-started
  - id: "02.4"
    title: "Release-Mode Underflow Detection"
    status: not-started
  - id: "02.5"
    title: "Completion Checklist"
    status: not-started
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

- [ ] Add `ORI_TRACE_RC` env var check (cached in `OnceLock`, same pattern as `ORI_CHECK_LEAKS`)
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
- [ ] Add trace logging to `ori_rc_alloc()`: print pointer, size, align, initial rc=1, live count
- [ ] Add trace logging to `ori_rc_inc()`: print pointer, new refcount, live count
- [ ] Add trace logging to `ori_rc_dec()`: print pointer, new refcount, "FREE" if reaching 0, live count
- [ ] Add trace logging to `ori_rc_free()`: print pointer, size, align, live count
- [ ] Ensure trace writes are atomic (single `eprintln!` call, not multiple `eprint!`)
- [ ] Add `ORI_TRACE_RC=verbose` mode that also prints a backtrace for each operation (using `std::backtrace::Backtrace`)
- [ ] Verify zero overhead when disabled: `OnceLock` check is a single atomic load after first call
- [ ] Test: run a simple program with `ORI_TRACE_RC=1`, verify the alloc→inc→dec→free sequence is complete and balanced

---

## 02.2 Leak Attribution (Allocation-Site Tracking)

**File(s):** `compiler/ori_rt/src/lib.rs`

When `ORI_CHECK_LEAKS=1` detects leaked allocations, report *which* allocations leaked. Each `ori_rc_alloc` is assigned a monotonic ID, and unfreed IDs are printed at exit.

- [ ] Add `RC_ALLOC_COUNTER: AtomicI64` — monotonic allocation counter
- [ ] Add `RC_ALLOC_REGISTRY: Mutex<HashMap<*mut u8, (i64, usize, usize)>>` — maps data_ptr → (alloc_id, size, align)
  - Only active when `ORI_CHECK_LEAKS=1` (gated by same env check)
  - Behind `#[cfg(debug_assertions)]` to avoid any release overhead
- [ ] On `ori_rc_alloc()`: register (alloc_id, size, align) in registry
- [ ] On `ori_rc_free()`: remove from registry
- [ ] On program exit (in `ori_run_main`): if live_count > 0, iterate registry and print:
  ```
  ori: 2 RC allocation(s) not freed:
    #42: ptr=0x7f1234 size=24 align=8 (unfreed)
    #67: ptr=0x7f5678 size=16 align=8 (unfreed)
  ```
- [ ] Test: write a program with a deliberate leak, verify the attribution report identifies it

---

## 02.3 Runtime Assertion Mode (`ORI_RT_DEBUG`)

**File(s):** `compiler/ori_rt/src/lib.rs`

Add a comprehensive assertion mode that catches bugs at the earliest possible point. When `ORI_RT_DEBUG=1`, the runtime validates every RC operation:

- [ ] Add `ORI_RT_DEBUG` env var check (cached `OnceLock`)
- [ ] **RC header validation**: Before any RC operation, check that the refcount at `data_ptr - 8` is a plausible value (> 0, < 1_000_000)
  - Catches: use-after-free (refcount was overwritten), corruption, misaligned pointers
- [ ] **Null sentinel guard**: COW functions that receive null data_ptr should log a warning (empty list mutation)
- [ ] **Bounds checking for list operations**: Validate index < len in `ori_list_get`, index <= len in `ori_list_insert_cow`
  - Currently these are unchecked in the runtime — only codegen is supposed to validate
- [ ] **Double-free guard**: Track freed pointers in a `HashSet<*mut u8>` (debug only), abort if `ori_rc_free` is called on an already-freed pointer
- [ ] Ensure all guards are gated behind the env var check — zero overhead when disabled
- [ ] Test: deliberately pass a freed pointer to `ori_rc_inc` — assertion mode catches it, normal mode doesn't

---

## 02.4 Release-Mode Underflow Detection

**File(s):** `compiler/ori_rt/src/lib.rs`

The current `debug_assert!(old_count >= 1)` in `ori_rc_dec` only fires in debug builds. In release builds, decrementing a zero refcount silently underflows to `i64::MAX`, creating a phantom "live" allocation that will never be freed. Add a lightweight underflow guard for release mode.

- [ ] In `ori_rc_dec()`, after the atomic decrement: if the previous value was 0, abort with a clear message
  ```rust
  if prev_count == 0 {
      eprintln!("ori: FATAL — ori_rc_dec called on already-freed allocation at {:p}", data_ptr);
      eprintln!("ori: this is a double-free bug in the compiler's RC codegen");
      std::process::abort();
  }
  ```
- [ ] This is NOT gated behind a flag — it's a safety net for all builds (one branch per dec, ~0.5ns overhead)
- [ ] Verify the check is `#[cold]` annotated so the abort path doesn't pollute instruction cache
- [ ] Test: write a Rust unit test that calls `ori_rc_dec` on a zero-refcount allocation — verify abort

---

## 02.5 Completion Checklist

- [ ] `ORI_TRACE_RC=1` produces complete alloc→inc→dec→free traces for simple programs
- [ ] `ORI_TRACE_RC=verbose` adds backtraces to each operation
- [ ] `ORI_CHECK_LEAKS=1` now prints allocation IDs and sizes for unfreed allocations
- [ ] `ORI_RT_DEBUG=1` catches use-after-free and corrupted RC headers
- [ ] Release-mode underflow detection catches `ori_rc_dec` on zero-refcount allocations
- [ ] Zero overhead verified when all flags are disabled (no measurable regression in `perf-baseline.sh`)
- [ ] All new runtime behavior tested with Rust unit tests in `ori_rt/src/tests.rs`
- [ ] `./test-all.sh` green (no interference with existing tests)
- [ ] Update `diagnostics/diagnose-aot.sh` (Section 01) to optionally use `ORI_TRACE_RC` flag

**Exit Criteria:** Running `ORI_TRACE_RC=1 ORI_CHECK_LEAKS=1 ./binary` on the `push(3).reverse()` test case produces an RC event trace that clearly shows whether the data pointer is freed twice or not freed at all. The trace is machine-readable (each line starts with `[RC]`) and human-readable (includes addresses, refcounts, live counts).
