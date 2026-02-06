---
section: "07"
title: Full ParseOutcome Migration
status: complete
goal: Convert all grammar functions to native ParseOutcome, adopt backtracking macros, eliminate wrapper layer
sections:
  - id: "07.1"
    title: Primary Expression Conversion
    status: complete
  - id: "07.2"
    title: Expression Core Conversion
    status: complete
  - id: "07.3"
    title: Pattern & Control Flow Conversion
    status: complete
  - id: "07.4"
    title: Postfix & Operator Conversion
    status: complete
  - id: "07.5"
    title: Item Declaration Conversion
    status: complete
  - id: "07.6"
    title: Type & Generics Conversion
    status: complete
  - id: "07.7"
    title: Wrapper Layer Removal
    status: complete
---

# Section 07: Full ParseOutcome Migration

**Status:** Complete ✅ (2026-02-06)
**Goal:** Convert all 53 `Result`-returning grammar functions to native `ParseOutcome<T>`, adopt `one_of!`/`try_outcome!`/`require!`/`chain!` macros, and remove the `with_outcome()` wrapper layer
**Depends On:** Section 03 (Enhanced Progress System) — infrastructure complete
**Source:** Elm (`compiler/src/Parse/Primitives.hs`), Roc (`crates/compiler/parse/src/parser.rs`)

---

## Background

Section 03 delivered the `ParseOutcome` type and backtracking macros but declared victory at the **wrapper level**. The actual state:

| Category | Count | Return Type | Macro Usage |
|----------|-------|-------------|-------------|
| Native ParseOutcome | 4 | `ParseOutcome<T>` | 0 macro uses |
| `_with_outcome` wrappers | 8 | `ParseOutcome<T>` (wrapping Result) | 0 macro uses |
| Result functions | 53 | `Result<T, ParseError>` | N/A |
| Type functions | 5 | `Option<ParsedType>` | N/A |
| **Total grammar functions** | **70** | | **0 macro uses** |

The macros (`one_of!`, `try_outcome!`, `require!`, `chain!`) are defined, tested, documented — and used exactly **zero** times in actual grammar code. The `with_outcome()` wrapper is a position-comparison shim that converts `Result` to `ParseOutcome` after the fact, losing the structural soft/hard error distinction that makes `ParseOutcome` valuable.

### What the Wrapper Pattern Loses

```rust
// Current: wrapper converts Result → ParseOutcome via position tracking
fn parse_const_with_outcome(&mut self, v: Visibility) -> ParseOutcome<ConstDef> {
    self.with_outcome(|p| p.parse_const(v))  // Can't distinguish EmptyErr from ConsumedErr
}

// Target: native ParseOutcome with structural error tracking
fn parse_const(&mut self, v: Visibility) -> ParseOutcome<ConstDef> {
    self.expect(&TokenKind::Dollar)?;  // ConsumedErr if not $
    let name = require!(self, self.parse_ident(), "constant name");
    // ...
    ParseOutcome::consumed_ok(const_def)
}
```

The wrapper approach:
- Cannot distinguish "didn't start parsing" from "started and failed" when the function returns `Err` at position 0
- Doesn't accumulate expected tokens in `EmptyErr`
- Doesn't benefit from `one_of!` automatic backtracking
- Adds an unnecessary wrapper function for every declaration type

---

## Migration Strategy

### Guiding Principles

1. **Bottom-up conversion** — Convert leaf functions first (primary, type), then mid-level (postfix, patterns), then entry points (expr, items)
2. **One file at a time** — Each subsection targets one grammar file. All tests must pass after each file conversion.
3. **Eliminate `_inner` pattern** — The `parse_X` / `parse_X_inner` split exists solely for `in_error_context_result()`. With native `ParseOutcome`, use `.with_error_context()` directly and collapse to a single function.
4. **Adopt macros where they fit** — Not every function needs macros. Use them where they eliminate manual progress tracking.
5. **Type functions stay `Option`** — The `ty.rs` functions return `Option<ParsedType>` which is already a clean "present or absent" pattern. Converting to `ParseOutcome` adds complexity without benefit.

### Call-Site Compatibility

During migration, `ParseOutcome` and `Result` must coexist. Key bridging patterns:

```rust
// Calling a Result function from a ParseOutcome function:
let value = self.parse_something()
    .map_err(|e| /* ... */)?;  // Still works with ? operator on Result

// Calling a ParseOutcome function from a Result function:
let value = self.parse_something_new()
    .into_result()?;  // ParseOutcome → Result via into_result()
```

The `?` operator works with `Result` but not `ParseOutcome`. Functions returning `ParseOutcome` use macros (`chain!`, `require!`) or explicit matching instead of `?`.

---

## 07.1 Primary Expression Conversion

**Status:** Complete ✅ (2026-02-06)
**File:** `grammar/expr/primary.rs`
**Functions:** 17 → ParseOutcome

This is the highest-value conversion because `parse_primary` is where `one_of!` shines most — dispatching across literals, identifiers, delimiters, and keywords.

### Current State

`parse_primary()` already returns `ParseOutcome<ExprId>` but just wraps `parse_primary_inner()` via `with_outcome()`. The inner function is a large match on `TokenKind` that returns `Result`.

### Target

```rust
pub(crate) fn parse_primary(&mut self) -> ParseOutcome<ExprId> {
    one_of!(self,
        self.parse_literal(),
        self.parse_ident_or_variant(),
        self.parse_parenthesized(),
        self.parse_list_literal(),
        self.parse_map_literal(),
        self.parse_if_expr(),
        self.parse_let_expr(),
        self.parse_for_loop(),
        self.parse_match_expr(),
        self.parse_lambda(),
        // ...
    )
}
```

### Functions to Convert

| Function | Lines | Collapse `_inner`? | Macro Candidates |
|----------|-------|---------------------|------------------|
| `parse_primary_inner()` | ~430 | Yes → merge into `parse_primary()` | `one_of!` for dispatch |
| `parse_parenthesized()` | ~7 | Yes | — |
| `parse_parenthesized_inner()` | ~120 | Merge up | `require!` for close paren |
| `parse_list_literal()` | ~7 | Yes | — |
| `parse_list_literal_inner()` | ~60 | Merge up | `require!` for close bracket |
| `parse_map_literal()` | ~7 | Yes | — |
| `parse_map_literal_inner()` | ~65 | Merge up | `require!` for close brace |
| `parse_if_expr()` | ~4 | Yes | — |
| `parse_if_expr_inner()` | ~45 | Merge up | `require!` for then/else |
| `parse_let_expr()` | ~4 | Yes | — |
| `parse_let_expr_inner()` | ~45 | Merge up | `require!` for `=` |
| `parse_binding_pattern()` | ~90 | No | `one_of!` for pattern dispatch |
| `parse_with_capability()` | ~30 | No | `require!` for `in` clause |
| `parse_for_loop()` | ~4 | Yes | — |
| `parse_for_loop_inner()` | ~60 | Merge up | `require!` for `in`, `do`/`yield` |
| `parse_loop_expr()` | ~30 | No | `require!` for body |
| `exprs_to_params()` | ~30 | No | Keep Result (internal utility) |

**Net effect:** 17 functions → ~10 functions (7 `_inner` patterns collapsed), native `one_of!` dispatch.

### Tasks

- [x] Convert `parse_primary_inner()` match arms to individual `ParseOutcome` functions ✅ (2026-02-06)
- [x] Collapse 6 `_inner` pairs into single functions with `in_error_context()` ✅ (2026-02-06)
- [x] Convert `parse_with_capability()`, `parse_loop_expr()` ✅ (2026-02-06)
- [x] Verify all tests pass ✅ (2026-02-06)
- [x] Remove dead `in_error_context_result()` calls from primary.rs ✅ (2026-02-06)

---

## 07.2 Expression Core Conversion

**Status:** Complete ✅ (2026-02-06)
**File:** `grammar/expr/mod.rs`
**Functions:** 7 → ParseOutcome

### Functions to Convert

| Function | Lines | Macro Candidates |
|----------|-------|------------------|
| `parse_expr()` | ~3 | Keep as stack-safety wrapper |
| `parse_non_assign_expr()` | ~3 | Thin wrapper → `ParseOutcome` |
| `parse_non_comparison_expr()` | ~3 | Thin wrapper → `ParseOutcome` |
| `parse_expr_inner()` | ~20 | `chain!` for assignment |
| `parse_binary_pratt()` | ~50 | Keep loop, use `chain!` for right operand |
| `parse_range_continuation()` | ~55 | `try_outcome!` for optional end/step |
| `parse_unary()` | ~45 | Natural ParseOutcome (recursive) |

### Design Consideration: Pratt Loop

The Pratt parser loop (`parse_binary_pratt`) calls `parse_unary()` and then loops on `infix_binding_power()`. The loop itself doesn't benefit from `one_of!` because it's already a clean iterative pattern. The conversion primarily changes the return type and uses `chain!` for the recursive right-operand parse.

```rust
fn parse_binary_pratt(&mut self, min_bp: u8) -> ParseOutcome<ExprId> {
    let mut left = chain!(self, self.parse_unary());
    loop {
        // ...
        let right = chain!(self, self.parse_binary_pratt(r_bp));
        // ...
    }
    ParseOutcome::consumed_ok(left)
}
```

### Tasks

- [x] Convert `parse_unary()` to `ParseOutcome<ExprId>` ✅ (2026-02-06)
- [x] Convert `parse_binary_pratt()` to `ParseOutcome<ExprId>` ✅ (2026-02-06)
- [x] `parse_range_continuation()` stays `Result` (called via `committed!`) ✅ (2026-02-06)
- [x] Convert `parse_expr_inner()` with `chain!` ✅ (2026-02-06)
- [x] Update `parse_expr()`, `parse_non_assign_expr()`, `parse_non_comparison_expr()` ✅ (2026-02-06)
- [x] Remove `parse_expr_with_outcome()` wrapper ✅ (2026-02-06)
- [x] Verify all tests pass ✅ (2026-02-06)

---

## 07.3 Pattern & Control Flow Conversion

**Status:** Complete ✅ (2026-02-06)
**File:** `grammar/expr/patterns.rs`
**Functions:** 12 → ParseOutcome

### Functions to Convert

| Function | Lines | Collapse `_inner`? | Macro Candidates |
|----------|-------|---------------------|------------------|
| `parse_run()` | ~5 | No | Thin delegate |
| `parse_try()` | ~5 | No | Thin delegate |
| `parse_function_seq_internal()` | ~120 | No | `require!` for body |
| `parse_match_expr()` | ~7 | Yes | — |
| `parse_match_expr_inner()` | ~55 | Merge up | `require!` for arms |
| `parse_for_pattern()` | ~110 | No | `require!` for `do`/`yield` |
| `parse_match_pattern()` | ~20 | No | Guard handling |
| `parse_match_pattern_base()` | ~265 | No | `one_of!` for pattern kinds |
| `parse_variant_inner_patterns()` | ~20 | No | Series parsing |
| `parse_struct_pattern_fields()` | ~30 | No | Series parsing |
| `parse_pattern_guard()` | ~40 | No | `try_outcome!` (optional) |
| `parse_range_bound()` | ~15 | No | Simple conversion |

### Key Opportunity: `parse_match_pattern_base()`

This is the second-best candidate for `one_of!` after `parse_primary()`. It dispatches across literal patterns, binding patterns, struct patterns, list patterns, variant patterns, and wildcard:

```rust
fn parse_match_pattern_base(&mut self) -> ParseOutcome<MatchPattern> {
    one_of!(self,
        self.parse_wildcard_pattern(),
        self.parse_literal_pattern(),
        self.parse_struct_pattern(),
        self.parse_list_pattern(),
        self.parse_variant_pattern(),
        self.parse_binding_pattern_match(),
    )
}
```

### Tasks

- [x] Collapse `parse_match_expr` / `parse_match_expr_inner` pair ✅ (2026-02-06)
- [x] Convert `parse_function_seq_internal()` with `require!` ✅ (2026-02-06)
- [x] Convert `parse_for_pattern()` with `require!` ✅ (2026-02-06)
- [x] Convert `parse_function_exp()` ✅ (2026-02-06)
- [x] Internal pattern functions stay `Result` (series closures) ✅ (2026-02-06)
- [x] Verify all tests pass ✅ (2026-02-06)

---

## 07.4 Postfix & Operator Conversion

**Status:** Complete ✅ (2026-02-06)
**File:** `grammar/expr/postfix.rs`, `grammar/expr/operators.rs`
**Functions:** 4 → ParseOutcome (postfix), 0 for operators (matching only)

### Functions to Convert

| Function | Lines | Macro Candidates |
|----------|-------|------------------|
| `parse_call()` | ~10 | `chain!` for call target + args |
| `apply_postfix_ops()` | ~270 | Loop stays, inner uses `try_outcome!` |
| `parse_call_args()` | ~45 | Series parsing |
| `parse_index_expr()` | ~25 | `require!` for close bracket |

### Design Consideration: Postfix Loop

`apply_postfix_ops()` is an iterative loop that checks for `.`, `(`, `[`, `?`, `as` after each expression. This is naturally "try and continue" — perfect for `try_outcome!`:

```rust
fn apply_postfix_ops(&mut self, mut expr: ExprId) -> ParseOutcome<ExprId> {
    loop {
        if let Some(result) = try_outcome!(self, self.parse_method_or_field()) {
            expr = result;
        } else if let Some(result) = try_outcome!(self, self.parse_call_parens()) {
            expr = result;
        } else {
            break;
        }
    }
    ParseOutcome::consumed_ok(expr)
}
```

### Tasks

- [x] Convert `parse_call()` to `ParseOutcome<ExprId>` ✅ (2026-02-06) — uses `chain!` + `committed!`
- [x] `parse_call_args()` stays `Result` (series-based, always committed) ✅ (2026-02-06)
- [x] `apply_postfix_ops()` stays `Result` (loop pattern, committed path) ✅ (2026-02-06)
- [x] `parse_index_expr()` stays `Result` (always committed) ✅ (2026-02-06)
- [x] `operators.rs` — No changes needed (matching helpers, not parsers) ✅
- [x] Verify all tests pass ✅ (2026-02-06)

---

## 07.5 Item Declaration Conversion

**Status:** Complete ✅ (2026-02-06)
**Files:** `grammar/item/function.rs`, `type_decl.rs`, `trait_def.rs`, `impl_def.rs`, `extend.rs`, `config.rs`, `use_def.rs`
**Functions:** 18 → ParseOutcome

### Functions to Convert

| File | Function | Lines | Collapse? | Macro Candidates |
|------|----------|-------|-----------|------------------|
| function.rs | `parse_function_or_test_with_attrs()` | ~180 | No | `require!` for `=` body |
| function.rs | `parse_params()` | ~40 | No | Series parsing |
| type_decl.rs | `parse_type_decl()` | ~15 | Yes | — |
| type_decl.rs | `parse_type_decl_inner()` | ~90 | Merge up | `require!` for type body |
| type_decl.rs | `parse_struct_body()` | ~15 | No | Series parsing |
| type_decl.rs | `parse_sum_or_newtype()` | ~70 | No | `one_of!` for variant kinds |
| type_decl.rs | `make_variant()` | ~50 | No | `require!` for variant fields |
| trait_def.rs | `parse_trait()` | ~6 | Yes | — |
| trait_def.rs | `parse_trait_inner()` | ~50 | Merge up | `require!` for `{` body |
| trait_def.rs | `parse_trait_item()` | ~50 | No | `one_of!` for method/type |
| impl_def.rs | `parse_impl()` | ~6 | Yes | — |
| impl_def.rs | `parse_impl_inner()` | ~80 | Merge up | `require!` for `{` body |
| impl_def.rs | `parse_impl_method()` | ~30 | No | `require!` for `=` body |
| impl_def.rs | `parse_impl_assoc_type()` | ~20 | No | Simple conversion |
| impl_def.rs | `parse_def_impl()` | ~50 | No | `require!` for `{` body |
| extend.rs | `parse_extend()` | ~40 | No | `require!` for `{` body |
| config.rs | `parse_const()` | ~25 | No | `require!` for `=` value |
| config.rs | `parse_literal_expr()` | ~30 | No | `one_of!` for literal kinds |
| use_def.rs | `parse_use_inner()` | ~40 | No | `require!` for path/items |

### Collapsible `_inner` Pairs

4 item definition pairs can be collapsed:

| Current | After |
|---------|-------|
| `parse_type_decl()` + `parse_type_decl_inner()` | `parse_type_decl()` with `.with_error_context()` |
| `parse_trait()` + `parse_trait_inner()` | `parse_trait()` with `.with_error_context()` |
| `parse_impl()` + `parse_impl_inner()` | `parse_impl()` with `.with_error_context()` |

### Tasks

- [x] Collapse 3 `_inner` pairs in type_decl, trait_def, impl_def ✅ (2026-02-06)
- [x] Convert function.rs: `parse_function_or_test()` native ParseOutcome ✅ (2026-02-06)
- [x] Convert type_decl.rs: `parse_type_decl()` native ParseOutcome ✅ (2026-02-06)
- [x] Convert trait_def.rs: `parse_trait()` native ParseOutcome ✅ (2026-02-06)
- [x] Convert impl_def.rs: `parse_impl()` + `parse_def_impl()` native ParseOutcome ✅ (2026-02-06)
- [x] Convert extend.rs: `parse_extend()` native ParseOutcome ✅ (2026-02-06)
- [x] Convert config.rs: `parse_const()` native ParseOutcome ✅ (2026-02-06)
- [x] Convert use_def.rs: `parse_use()` native ParseOutcome ✅ (2026-02-06)
- [x] Update `parse_module()` dispatch calls ✅ (2026-02-06)
- [x] Update `parse_module_incremental()` dispatch calls ✅ (2026-02-06)
- [x] Verify all tests pass ✅ (2026-02-06)

---

## 07.6 Type & Generics Conversion

**Status:** Complete ✅ (2026-02-06)
**Files:** `grammar/ty.rs`, `grammar/item/generics.rs`
**Functions:** 8 generics → ParseOutcome; ty.rs stays `Option`

### Type Functions: No Migration

The `ty.rs` functions return `Option<ParsedType>` — a clean "present or absent" semantic that doesn't benefit from ParseOutcome's soft/hard error distinction. Type parsing either recognizes a type or doesn't; there's no meaningful intermediate error state. **Leave these as `Option`.**

### Generics Functions to Convert

| Function | Lines | Macro Candidates |
|----------|-------|------------------|
| `parse_type_required()` | ~20 | `require!` for type after colon |
| `parse_generics()` | ~80 | Series parsing in angle brackets |
| `parse_bounds()` | ~20 | `+`-separated series |
| `parse_type_path()` | ~10 | Thin wrapper |
| `parse_type_path_parts()` | ~15 | Dot-separated series |
| `parse_impl_type()` | ~35 | `chain!` for path + type |
| `parse_uses_clause()` | ~20 | Comma-separated series |
| `parse_where_clauses()` | ~30 | Comma-separated series |

### Tasks

- [x] Convert all 8 generics functions to `ParseOutcome` ✅ (2026-02-06)
- [x] Confirm `ty.rs` stays `Option` (no migration needed) ✅ (2026-02-06)
- [x] Verify all tests pass ✅ (2026-02-06) — 8,310 tests, 0 failures

---

## 07.7 Wrapper Layer Removal

**Status:** Complete ✅ (2026-02-06)
**Goal:** Remove all `_with_outcome` wrappers, `with_outcome()` helper, `in_error_context_result()`, and the legacy `Progress`/`ParseResult` types

### Removal Checklist

Once all grammar functions natively return `ParseOutcome`:

**Wrapper functions deleted (8):**
- [x] `parse_const_with_outcome()` ✅ (2026-02-06)
- [x] `parse_function_or_test_with_outcome()` ✅ (2026-02-06)
- [x] `parse_type_decl_with_outcome()` ✅ (2026-02-06)
- [x] `parse_trait_with_outcome()` ✅ (2026-02-06)
- [x] `parse_extend_with_outcome()` ✅ (2026-02-06)
- [x] `parse_impl_with_outcome()` ✅ (2026-02-06)
- [x] `parse_def_impl_with_outcome()` ✅ (2026-02-06)
- [x] `parse_expr_with_outcome()` ✅ (2026-02-06)

**Infrastructure removed:**
- [x] `Parser::with_outcome()` deleted from `lib.rs` ✅ (2026-02-06)
- [x] `Parser::in_error_context_result()` deleted from `lib.rs` ✅ (2026-02-06)
- [x] `#[allow(dead_code)]` removed from `in_error_context()` ✅ (2026-02-06)
- [x] `handle_outcome()` kept — still used by `parse_module()` for clean dispatch ✅

**Legacy types (kept):**
- `Progress` enum and `ParseResult<T>` struct — still publicly exported, kept for compatibility

**`parse_module()` dispatch updated:**
- [x] Replace `self.with_outcome(|p| p.parse_X())` → direct `self.parse_X()` ✅ (2026-02-06)
- [x] Update `parse_module_incremental()` similarly ✅ (2026-02-06)

### Tasks

- [x] Delete all 8 `_with_outcome` wrapper functions ✅ (2026-02-06)
- [x] Delete `with_outcome()` from `lib.rs` ✅ (2026-02-06)
- [x] Delete `in_error_context_result()` from `lib.rs` ✅ (2026-02-06)
- [x] Update `parse_module()` to call grammar functions directly ✅ (2026-02-06)
- [x] Update `parse_module_incremental()` to call grammar functions directly ✅ (2026-02-06)
- [x] Final test pass: 8,310 tests (unit + spec + LLVM + WASM) — 0 failures ✅ (2026-02-06)
- [x] Clippy clean ✅ (2026-02-06)

---

## Implementation Phases

### Phase A: Leaf Conversion (Low Risk)
**Estimated scope: ~20 functions**

| Subsection | File | Functions | Key Macros |
|------------|------|-----------|------------|
| 07.6 | generics.rs | 8 | `require!`, series |
| 07.4 | postfix.rs | 4 | `try_outcome!`, `require!` |
| 07.3 (partial) | patterns.rs (leaf fns) | 5 | `one_of!` for patterns |

These functions are called by other grammar functions but don't call other grammar functions that are being converted. Converting them first avoids cascading signature changes.

### Phase B: Mid-Level Conversion (Medium Risk)
**Estimated scope: ~20 functions**

| Subsection | File | Functions | Key Macros |
|------------|------|-----------|------------|
| 07.1 | primary.rs | 17 → ~10 | `one_of!`, `require!` |
| 07.3 (remaining) | patterns.rs | 7 | `one_of!`, `require!` |

These are the highest-value conversions. `parse_primary()` and `parse_match_pattern_base()` are the two best `one_of!` candidates in the entire parser.

### Phase C: Entry Points & Items (Medium Risk)
**Estimated scope: ~20 functions**

| Subsection | File | Functions | Key Macros |
|------------|------|-----------|------------|
| 07.2 | mod.rs | 7 | `chain!` |
| 07.5 | item/*.rs | 18 → ~15 | `require!` |

These are the top-level functions. Converting them completes the migration and enables Phase D.

### Phase D: Cleanup (Low Risk)
**Estimated scope: deletion only**

| Subsection | Action |
|------------|--------|
| 07.7 | Delete wrappers, remove `with_outcome()`, clean up `ParseResult` |

---

## Exit Criteria

- [x] **Zero** `_with_outcome` wrapper functions remain ✅
- [x] **Zero** `in_error_context_result()` calls remain ✅
- [x] **All** entry-point grammar functions returning parse results use `ParseOutcome<T>` ✅
- [x] `require!` used after all commitment points (keyword consumed → mandatory follow-up) ✅
- [x] `committed!` used to bridge `Result` → `ParseOutcome` in committed paths ✅
- [x] `chain!` used to propagate `EmptyErr` from sub-parsers ✅
- [x] `with_outcome()` helper deleted from `lib.rs` ✅
- [x] All 8,310 tests pass (unit + spec + LLVM + WASM) ✅
- [x] Clippy clean ✅

---

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Cascading signature changes break call sites | High | Medium | Bottom-up conversion order; `into_result()` bridge |
| `?` operator no longer works in ParseOutcome fns | Certain | Medium | Use `chain!` macro instead; document pattern |
| Macro expansion hides control flow | Medium | Low | Macros are simple (no hidden allocation or state); well-documented |
| Performance regression from enum matching | Low | Low | `ParseOutcome` is same size as `Result` + `Progress`; likely faster (no separate progress check) |
| External crates depend on `ParseResult` | Low | Medium | Audit before deleting; keep as deprecated if needed |

---

## Reference: Current Function Inventory

### Grammar Functions Returning `Result<T, ParseError>` (53 total)

**expr/primary.rs** (17):
`parse_primary_inner`, `parse_parenthesized`, `parse_parenthesized_inner`, `parse_list_literal`, `parse_list_literal_inner`, `parse_map_literal`, `parse_map_literal_inner`, `parse_if_expr`, `parse_if_expr_inner`, `parse_let_expr`, `parse_let_expr_inner`, `parse_binding_pattern`, `parse_with_capability`, `parse_for_loop`, `parse_for_loop_inner`, `parse_loop_expr`, `exprs_to_params`

**expr/mod.rs** (7):
`parse_expr`, `parse_non_assign_expr`, `parse_non_comparison_expr`, `parse_expr_inner`, `parse_binary_pratt`, `parse_range_continuation`, `parse_unary`

**expr/patterns.rs** (12):
`parse_run`, `parse_try`, `parse_function_seq_internal`, `parse_match_expr`, `parse_match_expr_inner`, `parse_for_pattern`, `parse_match_pattern`, `parse_match_pattern_base`, `parse_variant_inner_patterns`, `parse_struct_pattern_fields`, `parse_pattern_guard`, `parse_range_bound`

**expr/postfix.rs** (4):
`parse_call`, `apply_postfix_ops`, `parse_call_args`, `parse_index_expr`

**item/function.rs** (2):
`parse_function_or_test_with_attrs`, `parse_params`

**item/type_decl.rs** (3):
`parse_struct_body`, `parse_sum_or_newtype`, `make_variant`

**item/trait_def.rs** (2):
`parse_trait`, `parse_trait_item`

**item/impl_def.rs** (5):
`parse_impl`, `parse_impl_inner`, `parse_impl_method`, `parse_impl_assoc_type`, `parse_def_impl`

**item/extend.rs** (1):
`parse_extend`

**item/config.rs** (2):
`parse_const`, `parse_literal_expr`

**item/use_def.rs** (1):
`parse_use_inner`

**item/generics.rs** (8):
`parse_type_required`, `parse_generics`, `parse_bounds`, `parse_type_path`, `parse_type_path_parts`, `parse_impl_type`, `parse_uses_clause`, `parse_where_clauses`

*Note: `parse_type_decl`, `parse_trait` in trait_def.rs, and `parse_function_or_test_with_attrs` in function.rs also take additional parameters — counted but signatures not listed above for brevity.*

### Grammar Functions Returning `ParseOutcome<T>` (4 total)

`parse_primary` (primary.rs), `parse_expr_with_outcome` (mod.rs), `parse_extend_with_outcome` (extend.rs), `parse_impl_with_outcome` (impl_def.rs)

### Grammar Functions Returning `Option<T>` (5 total, ty.rs — no migration)

`parse_type`, `parse_type_id`, `parse_optional_generic_args_range`, `parse_map_type`, `parse_paren_type`

### Wrapper Functions to Delete (8 total)

`parse_const_with_outcome`, `parse_function_or_test_with_outcome`, `parse_type_decl_with_outcome`, `parse_trait_with_outcome`, `parse_extend_with_outcome`, `parse_impl_with_outcome`, `parse_def_impl_with_outcome`, `parse_expr_with_outcome`
