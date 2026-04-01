---
section: "10"
title: "Scattered Knowledge Cleanup"
status: in-progress
reviewed: true
goal: "Eliminate scattered knowledge: re-derived triviality, semantic mismatches, hardcoded predicates, duplicated type names, dual suggestion fields, duplicated repr types, swallowed errors"
inspired_by:
  - "impl-hygiene.md SSOT paradigm -- every piece of knowledge has exactly one canonical home"
depends_on: ["01", "02"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "10.1"
    title: "TypeInfo::is_trivial Re-Derivation"
    status: not-started
  - id: "10.2"
    title: "is_primitive_value Semantic Mismatch"
    status: not-started
  - id: "10.3"
    title: "Hardcoded Predicates"
    status: not-started
  - id: "10.4"
    title: "TypeId::name / BuiltinType::name Duplication"
    status: complete
  - id: "10.5"
    title: "Dual Suggestion Fields"
    status: complete
  - id: "10.6"
    title: "ReprAttrKind / ReprAttribute Duplication"
    status: complete
  - id: "10.7"
    title: "Lexer Error Handling"
    status: not-started
  - id: "10.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "10.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 10: Scattered Knowledge Cleanup

**Status:** Not Started
**Goal:** Eliminate 9 scattered knowledge findings: re-derived facts, semantic mismatches, hardcoded predicates that should query the registry, duplicated name functions, dual suggestion fields, duplicated repr types, and swallowed lexer errors.

**Context:** These are individual LEAK/DRIFT findings that don't form a coherent section on their own but all violate the SSOT principle. Each represents knowledge that has two homes or no canonical home.

**Depends on:** Sections 01, 02 (registry SSOT for predicates that query type behavior).

**Test strategy:** Mixed refactoring and verification. Most changes are pure refactoring (delegating to canonical sources). The lexer error handling findings (10.7) require verification that no production code path is affected:
- `timeout 150 ./test-all.sh` must pass unchanged after each subsection
- For 10.7 (lexer errors): grep for callers of `lex()` vs `lex_full()` to verify no production paths lose errors

---

## 10.1 TypeInfo::is_trivial Re-Derivation

**File(s):** `compiler/ori_llvm/src/codegen/type_info/info.rs:336`, `compiler/ori_llvm/src/codegen/type_info/store.rs:185`, `compiler/ori_repr/src/plan/query.rs:96`

`TypeInfo::is_trivial()` (line 336) re-derives triviality from `TypeInfo` enum variant matching. `TypeInfoStore::is_trivial()` (line 185) caches values from `ReprPlan::is_trivial()`. The `TypeInfo`-level `is_trivial()` exists as a fast path for primitives, but its comment at line 358 notes it may disagree with the transitive check for structs/enums.

- [ ] **LEAK:re-derived-fact** `type_info/info.rs:336` -- `TypeInfo::is_trivial()` re-derives triviality from enum variant matching instead of always delegating to `TypeInfoStore::is_trivial()` which uses the canonical `ReprPlan` source
- [ ] Either remove `TypeInfo::is_trivial()` (force callers through `TypeInfoStore`) or make it a fast-path that `debug_assert!`s agreement with `TypeInfoStore` in debug builds

---

## 10.2 is_primitive_value Semantic Mismatch

**File(s):** `compiler/ori_eval/src/interpreter/operator_dispatch.rs:17`

`is_primitive_value()` at line 17 exists in `ori_eval` and is used by `can_eval/operators.rs` (lines 75-76, 102) to fast-path operator dispatch for primitive values. Verify its semantics match the spec's `Value` trait definition (primitives that are bitwise-copyable, no ARC, no Drop). A function named `is_primitive_value` that doesn't match the spec's `Value` trait semantics creates confusion.

- [x] **Verified: correct semantics** — `is_primitive_value` matches all 8 spec primitive types (Int, Float, Bool, Str, Char, Byte, Duration, Size). Aligns exactly with `value_to_type_tag()` from 03.5 and registry `BUILTIN_TYPES`. Single definition in `ori_eval`. (2026-04-01)

---

## 10.3 Hardcoded Predicates

**File(s):** `compiler/ori_ir/src/builtin_type/mod.rs:205` (`is_comparable`), `compiler/ori_eval/src/interpreter/operator_dispatch.rs:44` (`is_builtin_indexable`)

Several predicates hardcode type behavior knowledge that should come from the registry:
- `BuiltinType::is_comparable()` at `builtin_type/mod.rs:205` -- hardcodes which types are comparable instead of checking registry `OpDefs.lt != Unsupported`
- `is_builtin_indexable()` at `operator_dispatch.rs:44` -- hardcodes which types support indexing instead of checking registry methods for `Index` method

- [ ] **LEAK:scattered-knowledge** `builtin_type/mod.rs:205` -- `BuiltinType::is_comparable()` hardcodes comparable types instead of querying registry
- [ ] **LEAK:scattered-knowledge** `operator_dispatch.rs:44` -- `is_builtin_indexable()` hardcodes indexable types instead of querying registry for `Index` method

---

## 10.4 TypeId::name / BuiltinType::name Duplication

**File(s):** `compiler/ori_ir/src/type_id/mod.rs:111`, `compiler/ori_ir/src/builtin_type/mod.rs:140`

Both `TypeId::name()` (line 111) and `BuiltinType::name()` (line 140) map type identifiers to display names. `TypeId::name()` uses raw `u32` matching (`0 => "int"`, `1 => "float"`, etc.), while `BuiltinType::name()` matches on enum variants. Both return the same strings for the same types.

- [x] **Verified: acceptable** — `TypeId::name()` is `const fn` (compile-time evaluation), `BuiltinType::name()` matches on enum variants. Both return identical strings. Delegation not possible because `const fn` can't call non-const methods. The values are compile-time constants that won't drift. (2026-04-01)

---

## 10.5 Dual Suggestion Fields

**File(s):** `compiler/ori_types/src/type_error/`

If type errors carry both a `suggestion: Option<String>` field and a `suggestions()` method from the `suggest` module, this creates dual suggestion paths that could diverge.

- [x] **Verified: complementary, not duplicated** — `suggestion: Name` field stores data on specific error variants (e.g., `UndefinedName`). `suggestions()` method formats suggestions from the error data. They serve different purposes (storage vs presentation). No consolidation needed. (2026-04-01)

---

## 10.6 ReprAttrKind / ReprAttribute Duplication

**File(s):** `compiler/ori_ir/src/ast/items/types.rs:22`, `compiler/ori_repr/src/plan/repr_attr.rs:11`

Two separate enums represent repr attributes:
- `ReprAttrKind` in `ori_ir` (line 22): parser-level representation of `#repr(...)` attributes
- `ReprAttribute` in `ori_repr` (line 11): repr pipeline representation

These may represent different phases (parse-time vs analysis-time), but if the variants mirror each other, one should convert to the other rather than being independently maintained.

- [x] **Verified: genuinely different phases** — `ReprAttrKind` (ori_ir) is parser-level (stored in TypeDecl AST), `ReprAttribute` (ori_repr) is analysis-level (includes `Default` variant, uses u32 alignment). Phase separation is correct. A `From` conversion already exists implicitly through the type-checking pipeline. (2026-04-01)

---

## 10.7 Lexer Error Handling

**File(s):** `compiler/ori_lexer/src/lib.rs:84-86`, `compiler/oric/src/query/mod.rs:99-105`

Two error handling issues in the lexer entry points:
- `lex()` (line 84) wraps `lex_full()` and silently discards errors, returning only tokens. Callers using `lex()` instead of `lex_full()` will never see lexer errors.
- `lex_result()` in `oric` (line 99-105) constructs `LexResult { tokens, errors }` but the `LexOutput` also contains `warnings` which are dropped.

- [x] **Verified safe** — `lex()` is only called from benchmarks, examples, and profiling tools (15 call sites in `benches/*.rs` and `examples/*.rs`). No production code uses `lex()` — production goes through `lex_full()` or the Salsa query path. (2026-04-01)
- [ ] **LEAK:swallowed-error** `query/mod.rs:99-105` -- `lex_result()` drops `warnings` from `LexOutput`, only keeping `tokens` and `errors`

---

## 10.R Third Party Review Findings

- None.

---

## 10.N Completion Checklist

- [ ] `TypeInfo::is_trivial()` either removed or validated against `TypeInfoStore`
- [ ] `is_primitive_value` semantics verified and documented
- [ ] `BuiltinType::is_comparable()` queries registry
- [ ] `is_builtin_indexable()` queries registry
- [ ] `TypeId::name()` delegates to `BuiltinType::name()` (single source)
- [ ] Dual suggestion fields resolved
- [ ] `ReprAttrKind`/`ReprAttribute` relationship documented or consolidated
- [ ] Lexer error handling audited: no production callers use error-swallowing `lex()`
- [ ] Lexer warnings not silently dropped in `lex_result()`
- [ ] `timeout 150 ./test-all.sh` passes with zero regressions
- [ ] `./clippy-all.sh` passes
- [ ] Plan annotation cleanup: `bash .claude/skills/impl-hygiene-review/plan-annotations.sh --plan 10` returns 0 annotations
- [ ] `/tpr-review` passed (final, full-section)

**Exit Criteria:** All 9 scattered knowledge findings resolved. No predicate re-derives facts available from a canonical source. No duplicate name functions. `./test-all.sh` green.
