## Implementation Hygiene Review: `ori_canon::const_fold` (commit 57631c21)

**Scope:** 2 boundaries reviewed (ori_ir→ori_canon, ori_canon→ori_eval/ori_llvm), ~3 findings (1 leak, 2 waste, 1 note)

**Commit:** `feat(canon): Duration/Size constant folding in ori_canon`

---

### Boundary: `const_fold` internal invariant (classify → extract)

**Interface types:** `Constness`, `CanExpr`, `ConstValue`
**Entry point:** `try_fold()` → `classify()` → `extract_const_value()`

**Invariant:** If `classify(arena, id) == Constness::Const`, then `extract_const_value(arena, constants, id)` MUST return `Some(...)` for every possible child of Binary/Unary. Otherwise the fold silently fails — `classify` says "yes" but extraction says "no".

1. **[LEAK]** `const_fold.rs:54` + `const_fold.rs:219` — **`classify()` / `extract_const_value()` mismatch for `CanExpr::Const`**

   `classify()` at line 54 treats `CanExpr::Const(_)` (named `$` constants like `$PI`) as `Constness::Const`. But `extract_const_value()` has no arm for `CanExpr::Const(_)` — it falls through to the `_ => None` catch-all at line 219.

   **Effect:** For `$MAX * 2`, classify says "this is foldable" → try_fold enters the Binary path → `extract_const_value($MAX)` returns `None` → fold silently fails → expression remains un-folded. Wasted work, violated invariant, and the caller (`lower.rs`) is misled about why folding didn't happen.

   **Fix:** Remove `CanExpr::Const(_)` from the `Constness::Const` arm in `classify()`. Named constant references cannot be folded until their values are resolved (which happens at a later stage). The classification is aspirationally correct (these ARE constants) but practically wrong (the value is not available yet).

   **Pre-existing:** Yes — this arm existed before the commit. But it's a live invariant violation in the const_fold module being extended.

   ```rust
   // const_fold.rs:54 — BEFORE
   | CanExpr::Const(_) => Constness::Const,

   // const_fold.rs:54 — AFTER
   // Remove this arm; CanExpr::Const falls through to _ => Constness::Runtime
   ```

---

### Boundary: `ori_ir` → `ori_canon` (normalization functions called by const_fold)

**Interface types:** `DurationUnit::to_nanos()`, `SizeUnit::to_bytes()`
**Data flow:** `ConstValue::Duration { value: u64, unit }` → `unit.to_nanos(value)` → `i64` arithmetic → `result.cast_unsigned()` → `ConstValue::Duration { value: u64, unit: Nanoseconds }`

2. **[WASTE]** `token.rs:1242-1246` — **Unchecked overflow in `DurationUnit::to_nanos()`**

   `to_nanos()` does `value * self.nanos_multiplier()` without overflow checking. The const_fold code carefully uses `checked_add/sub/mul/div` for all arithmetic, but the normalization step before that arithmetic is unchecked.

   In practice, lexer literals are small (e.g., `500ms` = `500 * 1_000_000` = safe). But the const_fold module is called after lowering, where values could theoretically be large folded constants fed back as children.

   For folded results, the unit is always `Nanoseconds` (multiplier = 1), so overflow cannot happen on the second pass. This makes the issue **theoretical only** for the current architecture.

   **Fix (optional):** Add `checked_to_nanos()` → `Option<i64>` alongside `to_nanos()`, or add a `debug_assert!` to `to_nanos()` guarding against overflow. Low priority since the current data flow prevents the dangerous case.

   **Pre-existing:** Yes.

3. **[WASTE]** `token.rs:1300-1305` — **Unchecked overflow in `SizeUnit::to_bytes()`**

   Same pattern as Duration. `value * self.bytes_multiplier()` is unchecked. Same mitigating factor: folded Size results use `SizeUnit::Bytes` (multiplier = 1).

   **Pre-existing:** Yes.

---

### Boundary: `ori_canon` → `ori_eval` / `ori_llvm` (ConstValue consumption)

**Interface types:** `ConstValue::Duration { value: u64, unit: DurationUnit }`, `ConstValue::Size { value: u64, unit: SizeUnit }`

4. **[NOTE]** Duration `value: u64` stores semantically signed data — **intentional, consistent**

   The const_fold code stores negative Duration results (from subtraction or negation) as `i64_result.cast_unsigned()`. Downstream consumers reconstruct the signed value via `to_nanos()` which calls `cast_signed()`. The round-trip `i64 → cast_unsigned → u64 → [stored] → to_nanos() → cast_signed → i64` is correct due to two's complement.

   All consumers (ori_eval `const_to_value()` at `can_eval.rs:1327`, ori_llvm `lower_duration()` at `lower_literals.rs:67`) use the same `to_nanos()` path. Since folded constants always have `unit: Nanoseconds` (multiplier = 1), the conversion is effectively just `cast_signed()`.

   **Not a bug.** The encoding is a deliberate design choice. However, a `// NOTE: negative durations stored as wrapping u64` comment in the fold_binary Duration arms would improve clarity.

---

### Summary Table

| # | Category | File:Line | Introduced? | Severity | Fix |
|---|----------|-----------|-------------|----------|-----|
| 1 | LEAK | const_fold.rs:54 | Pre-existing | Medium | Remove `CanExpr::Const` from Const classification |
| 2 | WASTE | token.rs:1242 | Pre-existing | Low | Add overflow guard to `to_nanos()` (optional) |
| 3 | WASTE | token.rs:1300 | Pre-existing | Low | Add overflow guard to `to_bytes()` (optional) |
| 4 | NOTE | const_fold.rs:337+ | N/A | — | Add clarifying comment (optional) |

---

### Execution Order

1. **Fix #1 (LEAK):** Remove `CanExpr::Const(_)` from the `Constness::Const` arm in `classify()`. This is a one-line removal. Add a comment explaining why `$name` constants are classified as Runtime.
2. **Fix #2-3 (WASTE):** Optionally add `debug_assert!` guards to `to_nanos()` and `to_bytes()` to catch overflow in debug builds. Not strictly necessary given the current architecture.
3. **Fix #4 (NOTE):** Optionally add clarifying comment about the unsigned encoding of signed Duration values.
4. Run `cargo t -p ori_canon` to verify no behavior changes.
5. Run `./test-all.sh` to verify no regressions.
6. Run `./clippy-all.sh` to verify no new warnings.

### Assessment

**The new code introduced by this commit is well-structured.** It follows the established const_fold patterns, uses checked arithmetic consistently, correctly defers undefined behavior (div-by-zero, overflow) to runtime, and has thorough test coverage (14 Rust + 18 Ori tests). The Duration/Size normalization to base units (nanoseconds/bytes) before arithmetic is the correct approach.

**The only actionable finding is #1** — the `CanExpr::Const` classify/extract mismatch. This is a pre-existing invariant violation that should be fixed since we're in this module.
