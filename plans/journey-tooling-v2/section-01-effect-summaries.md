---
section: "01"
title: "Runtime Function Effect Summaries"
status: complete
goal: "Eliminate ARC false positives from opaque runtime functions by teaching the scanner which functions allocate (+1) or consume (-1) RC objects"
inspired_by:
  - "Clang RetainCountChecker — function effect summaries for CoreFoundation/Cocoa"
  - "Swift RCIdentityAnalysis (include/swift/SILOptimizer/Analysis/RCIdentityAnalysis.h) — RC identity through projections"
depends_on: []
sections:
  - id: "01.1"
    title: "Effect Summary Table"
    status: complete
  - id: "01.2"
    title: "Integration into arc_metrics.py"
    status: complete
  - id: "01.3"
    title: "Integration into rc_balance.rs"
    status: complete
  - id: "01.4"
    title: "Completion Checklist"
    status: complete
---

# Section 01: Runtime Function Effect Summaries

**Status:** Not Started
**Goal:** Eliminate false positive ARC violations caused by runtime functions that allocate or manage RC objects internally. After this section, J9's `arc_violations` should drop from 9 to ≤2, and J10's from 15 to ≤5.

**Context:** The `arc_metrics.py` scanner counts `ori_rc_inc` and `ori_rc_dec` calls per function and checks if they balance. But `ori_str_from_raw` allocates a string with RC=1 internally — the scanner sees 0 incs and 3 decs, flags it as unbalanced. The Clang Static Analyzer solved this exact problem with `RetainSummaryManager`: a table mapping function names to their RC effects on parameters and return values.

**Reference implementations:**
- **Clang** `clang/lib/StaticAnalyzer/Checkers/RetainCountChecker/RetainCountChecker.cpp`: `RetainSummaryManager` maps CoreFoundation/Cocoa functions to `RetEffect` (returns +1, +0, or owned). This is exactly the pattern we need.
- **Swift** `include/swift/SILOptimizer/Analysis/RCIdentityAnalysis.h`: Traces RC identity through projections (`struct_extract`, `enum_data`). Relevant for understanding when a dec on a field equals a dec on the parent.

**Depends on:** Nothing (foundation section).

---

## 01.1 Effect Summary Table

**File(s):** `.claude/skills/code-journey/effect_summaries.py` (new)

> **WARNING (scalability):** The `RUNTIME_EFFECTS` dict currently has ~45 entries
> (~300 lines of code). Each new `pub extern "C" fn ori_*` in `ori_rt` requires
> a manual entry. The sync test (Section 01.4) will catch missing entries, but
> if the table grows past ~60 entries, consider auto-generating it from `ori_rt`
> function signatures with an `#[rc_effect(...)]` attribute or a build script.

The effect summary table maps runtime function names to their RC effects. Each entry specifies:
- **Return effect**: Does the return value carry a +1 RC? (e.g., `ori_str_from_raw` returns an owned value with RC=1)
- **Parameter effects**: Does any parameter get consumed (-1)? (e.g., `ori_rc_dec` consumes arg 0)

**Terminology**: `PLUS_ONE` (+1) = allocates or returns owned, `MINUS_ONE` (-1) = consumes or decrements, `BORROWED` = no RC change but pointer must remain valid, `NONE` = no RC effect at all. These map to Clang's `RetEffect` concepts: +1 retained, -1 consumed, +0 not-owned.

This is the Python equivalent of Clang's `RetainSummaryManager`, but much simpler -- we have ~137 runtime functions (of which ~40-50 have RC effects), not thousands of Cocoa APIs. Functions with no RC effects (queries, comparisons, printing) are implicit `NONE`/`NONE` and don't need entries -- `get_effect()` returns `None` for them, which callers treat as "no RC impact."

- [ ] Create `effect_summaries.py` with the effect table:
  ```python
  from dataclasses import dataclass
  from enum import Enum

  class RcEffect(Enum):
      """RC effect of a function on a value."""
      NONE = "none"           # No RC effect
      PLUS_ONE = "+1"         # Allocates / returns owned (+1 RC)
      MINUS_ONE = "-1"        # Consumes / decrements (-1 RC)
      BORROWED = "borrowed"   # Borrows (no RC change, must not be freed)

  @dataclass
  class FunctionEffect:
      """RC effects of a runtime function."""
      return_effect: RcEffect       # Effect on the return value
      param_effects: list[RcEffect] # Effect on each parameter (by index)
      is_allocation: bool = False   # True if this function creates a new RC object

  # Verified against compiler/ori_rt/src/ and
  # compiler/ori_llvm/src/codegen/runtime_decl/runtime_functions.rs
  #
  # Note: Struct-returning functions (ori_str_from_raw, ori_str_concat)
  # use sret convention in LLVM IR, so the +1 RC is on the embedded data
  # pointer, not the return value itself.
  RUNTIME_EFFECTS: dict[str, FunctionEffect] = {
      # --- Allocation (return +1) ---
      "ori_rc_alloc": FunctionEffect(
          return_effect=RcEffect.PLUS_ONE,
          param_effects=[RcEffect.NONE, RcEffect.NONE],  # size, align
          is_allocation=True,
      ),
      # Note: ori_str_from_raw returns OriStr struct {len, cap, data} via sret.
      # The +1 RC is on the embedded data pointer, not the struct itself.
      # SSO strings (<=23 bytes) have no heap allocation (data ptr is null).
      "ori_str_from_raw": FunctionEffect(
          return_effect=RcEffect.PLUS_ONE,  # data ptr inside returned struct
          param_effects=[RcEffect.BORROWED, RcEffect.NONE],  # src_ptr, len
          is_allocation=True,
      ),
      # Note: ori_str_concat also returns OriStr struct via sret.
      # Params are *const OriStr (pointers to struct, not to data).
      "ori_str_concat": FunctionEffect(
          return_effect=RcEffect.PLUS_ONE,  # data ptr inside returned struct
          param_effects=[RcEffect.BORROWED, RcEffect.BORROWED],  # a, b
          is_allocation=True,
      ),
      "ori_list_alloc_data": FunctionEffect(
          return_effect=RcEffect.PLUS_ONE,
          param_effects=[RcEffect.NONE, RcEffect.NONE],  # capacity, elem_size
          is_allocation=True,
      ),
      # Note: ori_map_alloc does NOT exist. Map allocation uses
      # ori_map_literal_alloc(count, key_size, val_size, out_cap) -> ptr.
      "ori_map_literal_alloc": FunctionEffect(
          return_effect=RcEffect.PLUS_ONE,
          param_effects=[RcEffect.NONE] * 4,  # count, key_size, val_size, out_cap
          is_allocation=True,
      ),

      # --- Deallocation (param -1) ---
      "ori_rc_dec": FunctionEffect(
          return_effect=RcEffect.NONE,
          param_effects=[RcEffect.MINUS_ONE, RcEffect.NONE],  # data_ptr, drop_fn
      ),
      "ori_rc_free": FunctionEffect(
          return_effect=RcEffect.NONE,
          param_effects=[RcEffect.MINUS_ONE, RcEffect.NONE, RcEffect.NONE],  # data_ptr, size, align
      ),
      # (data, len, cap, elem_size, elem_dec_fn) — 5 params
      "ori_buffer_rc_dec": FunctionEffect(
          return_effect=RcEffect.NONE,
          param_effects=[RcEffect.MINUS_ONE] + [RcEffect.NONE] * 4,
      ),
      "ori_buffer_drop_unique": FunctionEffect(
          return_effect=RcEffect.NONE,
          param_effects=[RcEffect.MINUS_ONE] + [RcEffect.NONE] * 4,
      ),
      # (data, cap, len, key_size, val_size, key_dec_fn, val_dec_fn) — 7 params
      "ori_map_buffer_rc_dec": FunctionEffect(
          return_effect=RcEffect.NONE,
          param_effects=[RcEffect.MINUS_ONE] + [RcEffect.NONE] * 6,
      ),
      "ori_map_buffer_drop_unique": FunctionEffect(
          return_effect=RcEffect.NONE,
          param_effects=[RcEffect.MINUS_ONE] + [RcEffect.NONE] * 6,
      ),

      # --- Increment (+1 on param, no return effect) ---
      "ori_rc_inc": FunctionEffect(
          return_effect=RcEffect.NONE,
          param_effects=[RcEffect.PLUS_ONE],  # data_ptr — increments RC
      ),
      "ori_list_rc_inc": FunctionEffect(
          return_effect=RcEffect.NONE,
          param_effects=[RcEffect.PLUS_ONE, RcEffect.NONE],  # data_ptr, cap
      ),

      # --- COW (may consume input, returns new or same) ---
      # COW functions follow ori_*_cow naming
      # They borrow the input and may return a copy (+1) or the same pointer
      # For balance purposes: input is borrowed, output is +1

      # --- String methods that return new OriStr (possible +1) ---
      # Each returns OriStr struct; +1 on embedded data ptr if non-SSO.
      "ori_str_from_int": FunctionEffect(
          return_effect=RcEffect.PLUS_ONE,
          param_effects=[RcEffect.NONE],  # n: i64
          is_allocation=True,
      ),
      "ori_str_from_bool": FunctionEffect(
          return_effect=RcEffect.PLUS_ONE,
          param_effects=[RcEffect.NONE],  # b: bool
          is_allocation=True,
      ),
      "ori_str_from_float": FunctionEffect(
          return_effect=RcEffect.PLUS_ONE,
          param_effects=[RcEffect.NONE],  # f: f64
          is_allocation=True,
      ),
      "ori_str_substring": FunctionEffect(
          return_effect=RcEffect.PLUS_ONE,
          param_effects=[RcEffect.BORROWED, RcEffect.NONE, RcEffect.NONE],
          is_allocation=True,
      ),
      "ori_str_trim": FunctionEffect(
          return_effect=RcEffect.PLUS_ONE,
          param_effects=[RcEffect.BORROWED],
          is_allocation=True,
      ),
      "ori_str_to_uppercase": FunctionEffect(
          return_effect=RcEffect.PLUS_ONE,
          param_effects=[RcEffect.BORROWED],
          is_allocation=True,
      ),
      "ori_str_to_lowercase": FunctionEffect(
          return_effect=RcEffect.PLUS_ONE,
          param_effects=[RcEffect.BORROWED],
          is_allocation=True,
      ),
      "ori_str_replace": FunctionEffect(
          return_effect=RcEffect.PLUS_ONE,
          param_effects=[RcEffect.BORROWED, RcEffect.BORROWED, RcEffect.BORROWED],
          is_allocation=True,
      ),
      "ori_str_repeat": FunctionEffect(
          return_effect=RcEffect.PLUS_ONE,
          param_effects=[RcEffect.BORROWED, RcEffect.NONE],
          is_allocation=True,
      ),
      "ori_str_push_char": FunctionEffect(
          return_effect=RcEffect.PLUS_ONE,
          param_effects=[RcEffect.BORROWED, RcEffect.NONE],
          is_allocation=True,
      ),

      # --- Format functions (return OriStr, possible +1) ---
      "ori_format_int": FunctionEffect(
          return_effect=RcEffect.PLUS_ONE,
          param_effects=[RcEffect.NONE, RcEffect.BORROWED, RcEffect.NONE],
          is_allocation=True,
      ),
      "ori_format_float": FunctionEffect(
          return_effect=RcEffect.PLUS_ONE,
          param_effects=[RcEffect.NONE, RcEffect.BORROWED, RcEffect.NONE],
          is_allocation=True,
      ),
      "ori_format_str": FunctionEffect(
          return_effect=RcEffect.PLUS_ONE,
          param_effects=[RcEffect.BORROWED, RcEffect.BORROWED, RcEffect.NONE],
          is_allocation=True,
      ),
      "ori_format_bool": FunctionEffect(
          return_effect=RcEffect.PLUS_ONE,
          param_effects=[RcEffect.NONE, RcEffect.BORROWED, RcEffect.NONE],
          is_allocation=True,
      ),
      "ori_format_char": FunctionEffect(
          return_effect=RcEffect.PLUS_ONE,
          param_effects=[RcEffect.NONE, RcEffect.BORROWED, RcEffect.NONE],
          is_allocation=True,
      ),

      # --- Set allocation ---
      "ori_set_literal_alloc": FunctionEffect(
          return_effect=RcEffect.PLUS_ONE,
          param_effects=[RcEffect.NONE, RcEffect.NONE, RcEffect.NONE],  # count, elem_size, out_cap
          is_allocation=True,
      ),
      "ori_set_empty": FunctionEffect(
          return_effect=RcEffect.PLUS_ONE,
          param_effects=[],
          is_allocation=True,
      ),

      # --- List allocation (additional) ---
      "ori_list_empty": FunctionEffect(
          return_effect=RcEffect.PLUS_ONE,
          param_effects=[],
          is_allocation=True,
      ),

      # --- Set RC operations ---
      "ori_set_buffer_rc_dec": FunctionEffect(
          return_effect=RcEffect.NONE,
          param_effects=[RcEffect.MINUS_ONE] + [RcEffect.NONE] * 5,
      ),
      "ori_set_buffer_drop_unique": FunctionEffect(
          return_effect=RcEffect.NONE,
          param_effects=[RcEffect.MINUS_ONE] + [RcEffect.NONE] * 5,
      ),

      # --- Iterator sources (allocate heap-backed iterator state) ---
      "ori_iter_from_list": FunctionEffect(
          return_effect=RcEffect.PLUS_ONE,
          param_effects=[RcEffect.BORROWED] * 3,  # data, len, elem_size
          is_allocation=True,
      ),
      "ori_iter_from_range": FunctionEffect(
          return_effect=RcEffect.PLUS_ONE,
          param_effects=[RcEffect.NONE] * 4,  # start, end, step, inclusive
          is_allocation=True,
      ),
      "ori_iter_from_str": FunctionEffect(
          return_effect=RcEffect.PLUS_ONE,
          param_effects=[RcEffect.BORROWED],
          is_allocation=True,
      ),
      "ori_iter_from_map": FunctionEffect(
          return_effect=RcEffect.PLUS_ONE,
          param_effects=[RcEffect.BORROWED] * 4,
          is_allocation=True,
      ),

      # --- Iterator adapters (consume input iterator, return new) ---
      "ori_iter_map": FunctionEffect(
          return_effect=RcEffect.PLUS_ONE,
          param_effects=[RcEffect.MINUS_ONE, RcEffect.BORROWED, RcEffect.NONE],
          is_allocation=True,
      ),
      "ori_iter_filter": FunctionEffect(
          return_effect=RcEffect.PLUS_ONE,
          param_effects=[RcEffect.MINUS_ONE, RcEffect.BORROWED, RcEffect.NONE],
          is_allocation=True,
      ),
      "ori_iter_take": FunctionEffect(
          return_effect=RcEffect.PLUS_ONE,
          param_effects=[RcEffect.MINUS_ONE, RcEffect.NONE],
          is_allocation=True,
      ),
      "ori_iter_skip": FunctionEffect(
          return_effect=RcEffect.PLUS_ONE,
          param_effects=[RcEffect.MINUS_ONE, RcEffect.NONE],
          is_allocation=True,
      ),
      "ori_iter_enumerate": FunctionEffect(
          return_effect=RcEffect.PLUS_ONE,
          param_effects=[RcEffect.MINUS_ONE],
          is_allocation=True,
      ),
      "ori_iter_zip": FunctionEffect(
          return_effect=RcEffect.PLUS_ONE,
          param_effects=[RcEffect.MINUS_ONE, RcEffect.MINUS_ONE, RcEffect.NONE],
          is_allocation=True,
      ),
      "ori_iter_chain": FunctionEffect(
          return_effect=RcEffect.PLUS_ONE,
          param_effects=[RcEffect.MINUS_ONE, RcEffect.MINUS_ONE],
          is_allocation=True,
      ),

      # --- Iterator consumers (consume input iterator, no new allocation) ---
      "ori_iter_drop": FunctionEffect(
          return_effect=RcEffect.NONE,
          param_effects=[RcEffect.MINUS_ONE],
      ),

      # --- Catch/recover (returns OriStr) ---
      "ori_catch_recover": FunctionEffect(
          return_effect=RcEffect.PLUS_ONE,
          param_effects=[],
          is_allocation=True,
      ),
  }

  def get_effect(func_name: str) -> FunctionEffect | None:
      """Look up the RC effect of a runtime function.

      Returns None for unknown functions (user-defined or unrecognized runtime).
      COW functions (ori_*_cow) are handled by pattern matching.
      """
      if func_name in RUNTIME_EFFECTS:
          return RUNTIME_EFFECTS[func_name]
      # COW pattern: ori_{type}_{op}_cow
      if func_name.startswith("ori_") and func_name.endswith("_cow"):
          return FunctionEffect(
              return_effect=RcEffect.PLUS_ONE,  # COW returns owned
              param_effects=[RcEffect.BORROWED],  # input is borrowed
          )
      return None

  def is_allocation_function(func_name: str) -> bool:
      """Check if a function is known to allocate (return +1)."""
      effect = get_effect(func_name)
      return effect is not None and effect.is_allocation
  ```

- [ ] Verify all entries against `compiler/ori_rt/src/` (read the actual Rust function signatures)
- [ ] Verify all entries against `compiler/ori_llvm/src/codegen/runtime_decl/runtime_functions.rs`
- [ ] Add tests in `.claude/skills/code-journey/tests/test_effect_summaries.py`

**Also missing from `_RC_DEC_RE` in `arc_metrics.py`** (these are included in the effect table above but the current `_RC_DEC_RE` regex does not match them):
- [ ] Add `ori_set_buffer_rc_dec` to `_RC_DEC_RE` — set buffer decrement
- [ ] Add `ori_set_buffer_drop_unique` to `_RC_DEC_RE` — set buffer unique drop

**Verification checklist for effect table completeness:**
- [ ] All iterator sources (`ori_iter_from_list`, `ori_iter_from_range`, etc.) present with +1
- [ ] All iterator adapters (`ori_iter_map`, `ori_iter_filter`, etc.) present with -1 input / +1 output
- [ ] `ori_iter_drop` present with -1
- [ ] All string methods returning `OriStr` (`ori_str_substring`, `ori_str_trim`, etc.) present with +1
- [ ] All format functions (`ori_format_int`, `ori_format_float`, etc.) present with +1
- [ ] `ori_set_literal_alloc`, `ori_set_empty`, `ori_list_empty` present with +1

---

## 01.2 Integration into arc_metrics.py

**File(s):** `.claude/skills/code-journey/arc_metrics.py`

Update the ARC metrics extractor to use effect summaries for balance checking.

- [ ] Fix existing `_RC_DEC_RE` in `arc_metrics.py` to include `ori_map_buffer_rc_dec` (currently missing from the dec pattern, which undercounts map buffer decrements)
- [ ] Import `effect_summaries` and `get_effect` into `arc_metrics.py`
- [ ] In `_count_rc_ops()`, also count allocation functions as implicit +1:
  ```python
  def _count_rc_ops(func: Function) -> tuple[int, int]:
      """Count RC inc and dec operations, including implicit allocations."""
      inc = 0
      dec = 0
      for block in func.blocks:
          for instr in block.instructions:
              # Explicit RC inc
              if _RC_INC_RE.search(instr.text):
                  inc += 1
              # Explicit RC dec
              if _RC_DEC_RE.search(instr.text):
                  dec += 1
              # Implicit allocation (+1) from runtime functions
              callee = _extract_callee(instr.text)
              if callee:
                  effect = get_effect(callee)
                  if effect and effect.return_effect == RcEffect.PLUS_ONE:
                      inc += 1  # Counts as an implicit RC inc
  ```
- [ ] Add `_extract_callee()` helper to pull function name from call/invoke instructions (must handle both `call` and `invoke`, quoted names `@"..."`, and sret calling convention where the first argument is a return pointer):
  ```python
  _CALLEE_RE = re.compile(r'(?:call|invoke)\b[^@]*@(?:"([^"]+)"|(\S+?))\s*\(')

  def _extract_callee(text: str) -> str | None:
      """Extract the callee function name from a call or invoke instruction."""
      m = _CALLEE_RE.search(text)
      if not m:
          return None
      return m.group(1) or m.group(2)
  ```
- [ ] Update tests: J9 should now show balanced RC (the `ori_str_from_raw` +1 balances the 3 `ori_rc_dec` calls minus the 2 other allocations)
- [ ] Preserve backward compatibility: effect summaries ADD information, don't change existing detection

---

## 01.3 Integration into rc_balance.rs (Rust verifier)

**File(s):** `compiler/ori_llvm/src/verify/rc_balance.rs`

The in-pipeline Rust verifier has the same blind spot. Update it to recognize allocation functions beyond just `ori_rc_alloc`.

- [ ] Add the same summary table concept to `rc_balance.rs`:
  ```rust
  /// Runtime functions that allocate RC-managed memory.
  ///
  /// `ori_rc_alloc` and `ori_list_alloc_data` return a `ptr` directly.
  /// `ori_map_literal_alloc` returns a `ptr` to the hash table buffer.
  /// `ori_str_from_raw` and `ori_str_concat` return `OriStr` structs via
  /// sret — the RC-managed data pointer is embedded inside the struct.
  const RC_ALLOCATION_FUNCTIONS: &[&str] = &[
      "ori_rc_alloc",
      "ori_str_from_raw",
      "ori_str_concat",
      "ori_list_alloc_data",
      "ori_map_literal_alloc",
  ];
  ```
- [ ] Update `process_call()` match to check `RC_ALLOCATION_FUNCTIONS` instead of just `"ori_rc_alloc"`

- [ ] **Note**: Only add direct-ptr-returning functions (`ori_list_alloc_data`, `ori_map_literal_alloc`, `ori_set_literal_alloc`) to `RC_ALLOCATION_FUNCTIONS` in rc_balance.rs. Sret-returning functions (`ori_str_from_raw`, `ori_str_concat`, `ori_format_*`) write their return value through an sret pointer parameter (not the call result), requiring different tracking -- defer to a follow-up.
- [ ] Add test in `compiler/ori_llvm/src/verify/tests.rs` for the new allocation recognition
- [ ] Run `cargo test -p ori_llvm -- verify` to validate

---

## 01.4 Completion Checklist

- [ ] `effect_summaries.py` exists with all runtime functions mapped
- [ ] All entries verified against `ori_rt` source and `runtime_functions.rs`
- [ ] `arc_metrics.py` uses effect summaries for balance checking
- [ ] `rc_balance.rs` recognizes all allocation functions
- [ ] J9 (strings): `arc_violations` ≤ 2 (down from 9)
- [ ] J10 (lists): `arc_violations` ≤ 5 (down from 15)
- [ ] All existing tests pass: `python3 -m pytest tests/` and `cargo test -p ori_llvm`
- [ ] No regressions on J1-J4, J6-J8, J11-J12

**Sync test requirement:**
- [ ] Add a test that greps `compiler/ori_rt/src/` for all `pub extern "C" fn ori_*` signatures and verifies each one either:
  1. Has an entry in `RUNTIME_EFFECTS`, OR
  2. Matches the COW wildcard pattern (`ori_*_cow`), OR
  3. Is explicitly listed in a `NO_RC_EFFECT` allowlist (e.g., `ori_str_eq`, `ori_str_len`, `ori_print` — functions with no RC effects)
  This prevents drift when new runtime functions are added.

**Exit Criteria:** Running `extract-metrics.py` on J9's IR produces `arc_violations ≤ 2` and `arc_has_unbalanced: false` for string construction/destruction functions. The remaining violations (if any) are genuine codegen issues, not scanner blind spots.
