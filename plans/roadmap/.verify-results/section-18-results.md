# Section 18: Const Generics — Verification Results

**Verified**: 2026-03-28
**Verified by**: Claude Opus 4.6 (1M context) — roadmap verification agent
**Section status**: in-progress
**Section file**: `plans/roadmap/section-18-const-generics.md`

## Files Loaded

- `/home/eric/projects/ori_lang/CLAUDE.md` (all 183 lines)
- All 20 rules files in `.claude/rules/`: aot.md, arc.md, cargo.md, compiler.md, diagnostic.md, eval.md, impl-hygiene.md, ir.md, llvm.md, ori-lang.md, ori-syntax.md, parse.md, patterns.md, registry.md, roadmap.md, runtime.md, spec.md, tests.md, typeck.md, types.md
- `docs/ori_lang/v2026/spec/08-types.md` (Clause 8 -- types, including sections 8.2.2 Fixed-Capacity List, 8.3.1 Const Generic Parameters, 8.3.2 Const Bounds)
- `docs/ori_lang/v2026/spec/09-properties-of-types.md` (checked -- no const generics content found in this file)

## Methodology

For each item, I performed:
1. **Code search**: Grep/Glob for implementations in relevant compiler crates
2. **Test search**: Located and read all test files (Rust unit tests and Ori spec tests)
3. **Test execution**: Ran tests with `timeout 150` where applicable
4. **Spec cross-reference**: Verified against spec files and proposals

## Summary

| Subsection | Status Claimed | Status Verified | Notes |
|------------|---------------|-----------------|-------|
| 18.0 Const Evaluation Termination | not-started | [done] not-started confirmed | No implementation, no tests, no error codes |
| 18.1 Const Type Parameters | in-progress | [done] in-progress confirmed, partial | Parser and body-level typeck done; call-site unification, LLVM, AOT all missing |
| 18.2 Fixed-Capacity Lists | not-started | [partial] some parser + basic test coverage exists | Parser handles `[T, max N]`, type checker treats as `[T]`, 10 passing Ori tests |
| 18.3 Fixed-Size Arrays (Future) | not-started | [done] not-started confirmed | No implementation at all |
| 18.4 Const Expressions in Types | not-started | [done] not-started confirmed | IR has `ConstExpr` variant but no evaluation |
| 18.5 Const Bounds | in-progress | [done] in-progress confirmed, partial | Parser done; type checker explicitly defers const bounds |
| 18.6 Default Const Values | not-started | [partial] parser already supports `= value` syntax | Parser handles `$N: int = 10` but typeck/eval not done |
| 18.7 Const in Trait Bounds | not-started | [done] not-started confirmed | No implementation |
| 18.8 Expanded Const Generic Eligibility | not-started | [done] not-started confirmed | No implementation |
| 18.9 Associated Consts in Traits | not-started | [done] not-started confirmed | No implementation |
| 18.10 Const Functions in Type Positions | not-started | [done] not-started confirmed | No implementation |

---

## 18.0 Const Evaluation Termination

### Item: Step limit enforcement (1M operations)
- **Roadmap**: `[ ]`
- **Verified**: [todo] NOT IMPLEMENTED
- **Evidence**: No files matching `const_eval_limits`, `step_limit`, or error codes E0500-E0504 found anywhere in compiler. No `ori_types/tests/const_eval_limits.rs` exists. No `tests/spec/const/` directory exists.

### Item: Recursion depth limit (1000 frames)
- **Roadmap**: `[ ]`
- **Verified**: [todo] NOT IMPLEMENTED
- **Evidence**: Same as above. No recursion limit infrastructure for const evaluation.

### Item: Memory limit (100 MB)
- **Roadmap**: `[ ]`
- **Verified**: [todo] NOT IMPLEMENTED

### Item: Time limit (10 seconds)
- **Roadmap**: `[ ]`
- **Verified**: [todo] NOT IMPLEMENTED

### Item: Configurable limits via ori.toml
- **Roadmap**: `[ ]`
- **Verified**: [todo] NOT IMPLEMENTED. No `ori_config` crate or config file handling found.

### Item: Per-expression limit override via #const_limit(...)
- **Roadmap**: `[ ]`
- **Verified**: [todo] NOT IMPLEMENTED

### Item: Partial evaluation for mixed const/runtime arguments
- **Roadmap**: `[ ]`
- **Verified**: [todo] NOT IMPLEMENTED

### Item: Allow local mutable bindings in const functions
- **Roadmap**: `[ ]`
- **Verified**: [todo] NOT IMPLEMENTED

### Item: Allow loop expressions in const functions
- **Roadmap**: `[ ]`
- **Verified**: [todo] NOT IMPLEMENTED

### Item: Const evaluation caching
- **Roadmap**: `[ ]`
- **Verified**: [todo] NOT IMPLEMENTED

### Item: Error diagnostics (E0500-E0504)
- **Roadmap**: `[ ]`
- **Verified**: [todo] NOT IMPLEMENTED. Grep for E0500-E0504 across entire compiler found zero matches.

**Subsection summary**: All 11 items are correctly marked `[ ]`. Status "not-started" is accurate.

---

## 18.1 Const Type Parameters

### Item: Spec -- Const parameter syntax
- **Roadmap**: `[ ]`
- **Verified**: [partial] SPEC EXISTS but item not checked off
- **Evidence**: `docs/ori_lang/v2026/spec/08-types.md` section 8.3.1 "Const Generic Parameters" has comprehensive specification including syntax, allowed types, default values, parameter ordering, instantiation, monomorphization, and const expressions in types. The spec IS written -- the roadmap should mark the spec sub-items appropriately.
- STALE STATUS: The spec items (`const N: int in type parameters`, `Allowed const types`, `Scope rules`) may be partially done given the spec exists in 08-types.md.

### Item: Parser -- Parse const parameters
- **Roadmap**: `[x]` for `$` sigil, type annotation, position mixing
- **Verified**: [done] ALL THREE SUB-ITEMS CONFIRMED
- **Evidence**:
  - `compiler/ori_parse/src/grammar/item/generics/mod.rs` lines 73-101: Parser checks for `$`, consumes it, parses `identifier : type` with optional default `= expr`.
  - `compiler/ori_ir/src/ast/items/traits.rs`: `GenericParam` struct has `is_const: bool`, `const_type: Option<ParsedType>`, `default_value: Option<ExprId>` fields.
  - Rust tests in `compiler/ori_parse/src/grammar/item/generics/tests.rs`: 5 tests pass, covering type bounds, const bounds, and mixed.
  - Rust tests in `compiler/ori_parse/src/tests/compositional.rs`: `test_const_generics_combinations` passes.
  - All parser tests pass: `cargo test -p ori_parse -- const` (27 passed).

### Item: Type checker -- Const parameter validation (body-level)
- **Roadmap**: `[x]` for track const vs type, validate const type; `[ ]` for unification with const values
- **Verified**: [done] FIRST TWO CORRECT, THIRD CORRECTLY UNCHECKED
- **Evidence**:
  - `compiler/ori_types/src/check/signatures/mod.rs` lines 139-155: Filters `is_const` params, resolves const type via `resolve_const_param_type()`, stores in `ConstParamInfo`. Function `resolve_const_param_type()` handles INT and BOOL.
  - `compiler/ori_types/src/check/bodies/mod.rs` lines 66-70: Binds const params to their type in body scope (`param_env.bind(cp.name, cp.const_type)`).
  - Rust tests: 4 tests in `ori_types/src/check/signatures/tests.rs` verify const param type resolution for int, bool, named int, and None -> error cases. All pass.
  - Unification with const values at call site: NOT IMPLEMENTED -- no call-site const value resolution found in type checker.

### Item: Test -- tests/spec/types/const_generics.ori
- **Roadmap**: `[x]` for basic, multiple, mixed
- **Verified**: [done] CORRECT
- **Evidence**: File `tests/spec/types/const_generics.ori` exists with:
  - `@const_param<$N: int>` -- basic const param [done]
  - `@multi_const<$A: int, $B: int>` -- multiple [done]
  - `@mixed_params<T, $N: int>` -- mixed [done]
  - Also tests: `@bool_param<$F: bool>`, `@add_one<$N: int>`, `@const_param_default<$N: int = 10>`
  - File `ori check` passes for the non-skipped portions (Array type tests are correctly `#skip`ped).
  - Test run confirms 4181 passed, 0 failed, 42 skipped when running test suite including this file.
  - WEAK TESTS: No assertions in the const_generics.ori file -- functions are declared but never called or tested via `@test`. The functions type-check but are never exercised at runtime.

### LLVM/AOT items (all under 18.1)
- **Roadmap**: All `[ ]`
- **Verified**: [todo] CORRECTLY UNCHECKED. No `ori_llvm/tests/const_generic_tests.rs` file exists. No AOT tests for const generics.

---

## 18.2 Fixed-Capacity Lists

### Item: Spec -- Fixed-capacity list type
- **Roadmap**: `[ ]`
- **Verified**: [partial] SPEC EXISTS
- **Evidence**: `docs/ori_lang/v2026/spec/08-types.md` section 8.2.2 "Fixed-Capacity List" has full specification: syntax `[T, max N]`, subtype relationship, methods table, conversion methods, trait implementations. The spec is written and complete.
- STALE STATUS: The spec sub-items (`Type syntax`, `Relationship to dynamic [T]`, `Capacity limit semantics`) are all covered in the existing spec.

### Item: Grammar -- Parse fixed-capacity list type
- **Roadmap**: `[ ]`
- **Verified**: [partial] GRAMMAR AND PARSER EXIST
- **Evidence**:
  - `docs/ori_lang/v2026/spec/grammar.ebnf` line 358: `fixed_list_type = "[" type "," "max" const_expr "]"` -- grammar rule exists.
  - `compiler/ori_parse/src/grammar/ty/mod.rs`: Parser handles `[T, max N]` syntax.
  - `compiler/ori_ir/src/parsed_type/mod.rs`: `ParsedType::FixedList { elem, capacity }` variant exists.
  - Rust tests: `test_parse_fixed_list_integer_literal` and `test_parse_fixed_list_const_param` both pass.
  - `max` is listed as context-sensitive keyword in grammar.ebnf line 69.
- STALE STATUS: Both sub-items (`list_type grammar rule`, `max as soft keyword`) are implemented.

### Item: Types -- Fixed-capacity list type representation
- **Roadmap**: `[ ]`
- **Verified**: [partial] PARTIALLY IMPLEMENTED
- **Evidence**:
  - No `Type::FixedList` in the type pool. Both `compiler/ori_types/src/check/registration/type_resolution.rs` line 131 and `compiler/ori_types/src/infer/expr/type_resolution.rs` line 51 contain `ParsedType::FixedList { elem, capacity: _ } => // Treat as regular list for now`. The capacity is discarded.
  - Subtype relationship `[T, max N] <: [T]` is trivially satisfied since fixed lists ARE regular lists in the current implementation.
  - Capacity must be compile-time constant: not enforced (the capacity is parsed but ignored).

### Item: Methods -- Fixed-capacity list methods
- **Roadmap**: `[ ]`
- **Verified**: [todo] NOT IMPLEMENTED. No `.capacity()`, `.is_full()`, `.remaining()`, `.try_push()`, `.push_or_drop()`, `.push_or_oldest()`, `.to_dynamic()` methods exist. The test file has all method tests commented out.

### Item: Methods -- Dynamic list conversion methods
- **Roadmap**: `[ ]`
- **Verified**: [todo] NOT IMPLEMENTED. No `.to_fixed<$N>()` or `.try_to_fixed<$N>()` methods exist. Tests commented out.

### Item: Traits -- Trait implementations for [T, max N]
- **Roadmap**: `[ ]`
- **Verified**: [todo] NOT IMPLEMENTED as distinct from `[T]`. Since fixed lists are treated as regular lists, they inherit regular list traits, but this is not explicit.

### Item: Memory -- Inline storage representation
- **Roadmap**: `[ ]`
- **Verified**: [todo] NOT IMPLEMENTED. No inline allocation -- currently uses heap-allocated list representation.

### Item: Test -- tests/spec/types/fixed_capacity_list.ori
- **Roadmap**: `[ ]`
- **Verified**: [partial] TEST FILE EXISTS under different name
- **Evidence**: `tests/spec/types/fixed_list_types.ori` exists with 10 tests, all passing. Tests cover:
  - Basic declaration and operations (empty, partial, full) -- 3 tests passing
  - Different types (str, bool) -- 2 tests passing
  - Fixed-capacity methods -- ALL COMMENTED OUT (capacity, is_full, remaining, push, try_push, push_or_drop, push_or_oldest)
  - Conversion methods -- ALL COMMENTED OUT (to_dynamic, to_fixed, try_to_fixed)
  - Subtype relationship with `[T]` -- 1 test passing
  - Index access -- 2 tests passing
  - Iteration -- 1 test passing
  - Const generic capacity -- COMMENTED OUT
  - Basic usage -- 1 test passing
- WEAK TESTS: The passing tests only verify that `[T, max N]` works as `[T]`. No capacity-specific behavior is tested because the feature is not implemented.

---

## 18.3 Fixed-Size Arrays (Future)

### All items
- **Roadmap**: All `[ ]`
- **Verified**: [todo] CORRECTLY UNCHECKED. No `Type::FixedArray`, no `[T, size N]` parser support, no tests. Status "not-started" is accurate.

---

## 18.4 Const Expressions in Types

### Item: Spec -- Const expression rules
- **Roadmap**: `[ ]`
- **Verified**: [partial] SPEC EXISTS
- **Evidence**: `docs/ori_lang/v2026/spec/08-types.md` section 8.3.1 under "Const Expressions in Types" specifies allowed arithmetic operations in type positions. The spec is written.

### Item: Const evaluator -- Evaluate const expressions
- **Roadmap**: `[ ]`
- **Verified**: [todo] NOT IMPLEMENTED. `ParsedType::ConstExpr(ExprId)` variant exists in the IR but no const expression evaluator at type-checking time.

### Item: Type checker -- Validate const expressions
- **Roadmap**: `[ ]`
- **Verified**: [todo] NOT IMPLEMENTED

### Item: Test -- tests/spec/types/const_expressions.ori
- **Roadmap**: `[ ]`
- **Verified**: [todo] FILE DOES NOT EXIST

---

## 18.5 Const Bounds

### Item: Grammar -- Update grammar.ebnf
- **Roadmap**: `[ ]`
- **Verified**: [partial] GRAMMAR EXISTS
- **Evidence**: `docs/ori_lang/v2026/spec/grammar.ebnf` lines 262-275 contain full const bound grammar:
  - `const_constraint = const_bound_expr`
  - `const_bound_expr = const_or_expr`
  - `const_or_expr = const_and_expr { "||" const_and_expr }`
  - `const_and_expr = const_not_expr { "&&" const_not_expr }`
  - `const_not_expr = "!" const_not_expr | const_cmp_expr`
  - `const_cmp_expr = const_expr comparison_op const_expr | "(" const_bound_expr ")"`
- All 5 sub-items in the roadmap are covered by this grammar. STALE STATUS.

### Item: Parser -- Parse const bounds
- **Roadmap**: `[x]` for where clauses, comparisons, arithmetic, bitwise, multiple where clauses, Rust tests
- **Verified**: [done] ALL SIX SUB-ITEMS CONFIRMED
- **Evidence**:
  - `compiler/ori_parse/src/grammar/item/generics/mod.rs` lines 336-342: Parser handles `ConstBound` variant in where clause parsing, using `parse_non_assign_expr()` for the full expression including compound operators.
  - `compiler/ori_ir/src/ast/items/traits.rs`: `WhereClause::ConstBound { expr, span }` variant exists.
  - Rust tests in `compiler/ori_parse/src/grammar/item/generics/tests.rs`: `test_where_const_bound` and `test_where_mixed_type_and_const_bounds` both pass. Tests verify `is_const_bound()` returns true for `N > 0` and mixed type+const bounds parse correctly.
  - All parser const bound tests pass: `cargo test -p ori_parse -- where` (9 passed).
  - WEAK TESTS: Only 2 Rust tests cover const bounds specifically. No tests for compound `&&`/`||`, arithmetic in bounds, bitwise in bounds, or `!` negation despite roadmap claiming all these are done. The parser uses `parse_non_assign_expr()` which handles all expression syntax, so the parser likely supports these, but there are no tests pinning it.

### Item: Type checker -- Validate const bounds at compile time
- **Roadmap**: `[ ]`
- **Verified**: [todo] NOT IMPLEMENTED
- **Evidence**: `compiler/ori_types/src/check/signatures/mod.rs` line 219: "Collect where-clauses (only type bounds; const bounds are deferred)". The type checker explicitly skips const bounds.

### Item: Const evaluator -- Overflow handling
- **Roadmap**: `[ ]`
- **Verified**: [todo] NOT IMPLEMENTED. No E1033 error code exists.

### Item: Error messages -- Const bound error codes
- **Roadmap**: `[ ]`
- **Verified**: [todo] NOT IMPLEMENTED. Grep for E1030, E1031, E1032, E1033 returns zero results across entire compiler.

### Item: Test -- tests/spec/types/const_bounds.ori
- **Roadmap**: `[ ]`
- **Verified**: [todo] FILE DOES NOT EXIST. `tests/spec/types/const_generics.ori` has two `#skip`ped tests for const bounds (`@const_bound`, `@mixed_bounds`).

---

## 18.6 Default Const Values

### Item: Spec -- Default const values
- **Roadmap**: `[ ]`
- **Verified**: [partial] SPEC EXISTS
- **Evidence**: `docs/ori_lang/v2026/spec/08-types.md` section 8.3.1 "Default Values" specifies default const values.

### Item: Parser -- Parse default const
- **Roadmap**: `[ ]`
- **Verified**: [partial] PARSER ALREADY SUPPORTS THIS
- **Evidence**: `compiler/ori_parse/src/grammar/item/generics/mod.rs` lines 87-95: After parsing `$N: int`, the parser checks for `= expr` and stores it as `default_value`. The `GenericParam` struct has `default_value: Option<ExprId>` field. Test file `const_generics.ori` has `@const_param_default<$N: int = 10>` which type-checks successfully.
- STALE STATUS: This is already partially implemented (parsing works) but not checked off.

### Item: Type checker -- Apply defaults
- **Roadmap**: `[ ]`
- **Verified**: [todo] NOT IMPLEMENTED. The `ConstParamInfo` struct stores `default_value: Option<ExprId>` but no code applies defaults at call sites.

### Item: Test -- tests/spec/types/const_defaults.ori
- **Roadmap**: `[ ]`
- **Verified**: [todo] FILE DOES NOT EXIST

---

## 18.7 Const in Trait Bounds

### All items
- **Roadmap**: All `[ ]`
- **Verified**: [todo] CORRECTLY UNCHECKED. No associated const support in trait system. No `$SIZE: int` in trait defs. No tests.

---

## 18.8 Expanded Const Generic Eligibility (Capability Unification)

### All items
- **Roadmap**: All `[ ]`
- **Verified**: [todo] CORRECTLY UNCHECKED. Type checker currently hardcodes `int` and `bool` as the only valid const types via `resolve_const_param_type()`. No `Eq + Hashable` registry lookup.

---

## 18.9 Associated Consts in Traits (Capability Unification)

### All items
- **Roadmap**: All `[ ]`
- **Verified**: [todo] CORRECTLY UNCHECKED. No `AssocConst` in `TraitItem` or `ImplItem`. No parser support for `$name: Type` in traits.

---

## 18.10 Const Functions in Type Positions (Capability Unification)

### All items
- **Roadmap**: All `[ ]`
- **Verified**: [todo] CORRECTLY UNCHECKED. No const function analysis or type-position const evaluation.

---

## Section Completion Checklist

All items correctly marked `[ ]`:
- [ ] All items above have all checkboxes marked -- **correct, mostly unchecked**
- [ ] Spec updated -- **partially done** (spec sections exist in 08-types.md but roadmap doesn't reflect this)
- [ ] CLAUDE.md updated with const generic syntax -- `.claude/rules/ori-syntax.md` already has const generics documented
- [ ] `[T, max N]` fixed-capacity lists work -- **partially** (parses and treated as `[T]`, capacity ignored)
- [ ] `$N: int` const parameters in types work -- **body-level only**
- [ ] Const expressions in type positions work -- **not implemented**
- [ ] Const bounds work -- **parser only, type checker defers**
- [ ] All tests pass -- N/A (section not complete)

---

## Findings

### STALE STATUS (4 items)

1. **18.2 Grammar/Parser for fixed-capacity lists**: Roadmap marks all `[ ]` but parser and grammar already implement `[T, max N]` syntax with `max` as soft keyword. The grammar.ebnf rule exists. Two parser Rust tests pass. Should be `[x]`.

2. **18.2 Types representation (partial)**: Roadmap marks `[ ]` but `ParsedType::FixedList { elem, capacity }` exists in IR, parser creates it, type checker handles it (treating as regular list). Sub-item "Subtype relationship: `[T, max N] <: [T]`" is trivially satisfied. Should be `[partial]`.

3. **18.5 Grammar for const bounds**: Roadmap marks `[ ]` for all 5 grammar sub-items but the full grammar exists in `grammar.ebnf` lines 262-275. Should be `[x]`.

4. **18.6 Parser for default const**: Roadmap marks `[ ]` but parser already handles `$N: int = expr` syntax with `default_value` field. Should be `[x]` or `[partial]`.

### WEAK TESTS (3 items)

1. **18.1 const_generics.ori**: Functions are declared but never called via `@test`. No assertions verify behavior at runtime. These tests only verify type-checking passes, not runtime correctness.

2. **18.2 fixed_list_types.ori**: 10 tests pass but only verify `[T, max N]` behaves as `[T]`. All capacity-specific method tests are commented out. No semantic pin for fixed-capacity behavior.

3. **18.5 Parser const bound tests**: Only 2 Rust tests exist for const bounds parsing despite the roadmap claiming compound `&&`/`||`, arithmetic, bitwise, and negation are all done. The parser likely handles these (via `parse_non_assign_expr()`) but there are no test pins.

### MISSING TESTS (3 items)

1. **tests/spec/types/const_bounds.ori**: Referenced in roadmap but does not exist.
2. **tests/spec/types/const_defaults.ori**: Referenced in roadmap but does not exist.
3. **tests/spec/types/const_expressions.ori**: Referenced in roadmap but does not exist.

### PROPOSAL REFERENCES (verified)

All 4 proposals referenced in the roadmap exist:
- `proposals/approved/const-evaluation-termination-proposal.md` [exists]
- `proposals/approved/const-generics-proposal.md` [exists]
- `proposals/approved/fixed-capacity-list-proposal.md` [exists]
- `proposals/approved/const-generic-bounds-proposal.md` [exists]
- `proposals/approved/capability-unification-generics-proposal.md` [exists]

### SPEC REFERENCES (verified)

- `spec/08-types.md` sections 8.2.2, 8.3.1, 8.3.2 exist with const generics content
- `spec/09-properties-of-types.md` has no const generics content (roadmap references "Const in Traits" section that doesn't exist there yet)

### NO LLVM/AOT COVERAGE

Zero LLVM codegen or AOT tests exist for any const generics functionality. No `ori_llvm/tests/const_generic_tests.rs` or `ori_llvm/tests/fixed_capacity_tests.rs` files exist. This is consistent with roadmap marking all LLVM items as `[ ]`.

---

## Item Count

| Category | Count |
|----------|-------|
| Total items (approximate, counting all `- [ ]` and `- [x]` lines) | ~200+ (many are LLVM/AOT sub-items) |
| Items marked `[x]` | 9 (all in 18.1 and 18.5 parser sections) |
| Items correctly marked `[x]` | 9 (all verified) |
| Items that should be `[x]` but are `[ ]` | ~10 (grammar/parser items in 18.2, 18.5, 18.6) |
| Items correctly marked `[ ]` | ~180+ |
| Items incorrectly marked `[x]` | 0 |
