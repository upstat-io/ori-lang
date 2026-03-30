---
section: 20
title: Compile-Time Reflection
status: not-started
last_verified: "2026-03-29"
reviewed: true
tier: 8
goal: Enable zero-cost compile-time structural reflection via fields_of, variants_of, $for expansion, $if branching, and splice access
spec:
  - spec/27-reflection.md
inspired_by:
  - "Zig @typeInfo + inline for + @field (src/Sema.zig, src/InternPool.zig)"
  - "C++26 P2996 static reflection (template for, [: :] splice)"
depends_on: ["18", "2", "3"]
third_party_review:
  status: none
  updated: null
verification_notes:
  - "ERROR CODE DRIFT: E0460-E0464/W0461 are in the E0xxx (lexer) range but these are type checker/monomorphization errors — should be E2xxx per diagnostic conventions. When implementing, use E2042+ or document E04xx as a deliberate compile-time expansion sub-range."
  - "SPEC STALE: spec/27-reflection.md contains the OLD runtime Reflect model (Reflect trait, TypeInfo, Unknown, #derive(Reflect)). Has SUPERSEDED header but body not rewritten. grammar.ebnf has no $for/$if/splice productions."
  - "DEPENDENCY BLOCKED: Section 18.0 (const eval bridge) is not-started. Registration (20.1) and parsing (20.2) can proceed independently, but expansion (20.3) and resolution (20.4) are blocked on it."
  - "ori-syntax.md documents reflection features that have zero implementation — correct as design target documentation, not a hygiene violation."
sections:
  - id: "20.1"
    title: Compile-Time Metadata Types and Intrinsics
    status: not-started
    verified: "2026-03-29"
  - id: "20.2"
    title: "Parser: $for, $if, and Splice Syntax"
    status: not-started
    verified: "2026-03-29"
  - id: "20.3"
    title: "$for Expansion During Monomorphization"
    status: not-started
    verified: "2026-03-29"
  - id: "20.4"
    title: "$if Dead Branch Elimination and Splice Resolution"
    status: not-started
    verified: "2026-03-29"
  - id: "20.5"
    title: Type Classification Intrinsics
    status: not-started
    verified: "2026-03-29"
  - id: "20.6"
    title: Integration and Verification
    status: not-started
    verified: "2026-03-29"
  - id: "20.R"
    title: Third Party Review Findings
    status: done
    verified: "2026-03-29"
  - id: "20.7"
    title: Completion Checklist
    status: not-started
    verified: "2026-03-29"
---

# Section 20: Compile-Time Reflection

**Status:** Not Started
**Goal:** Compile-time structural reflection works end-to-end: `fields_of(T)` returns field metadata, `$for` expands heterogeneously during monomorphization, `$if` eliminates dead branches, and `value.[field]` resolves to direct field access — all with zero runtime overhead and identical codegen to hand-written code.

**Context:** The approved `compile-time-reflection-proposal.md` (2026-03-26) supersedes the runtime reflection model (`Reflect` trait, `TypeInfo`, `Unknown`). Compile-time reflection is zero-cost, requires no opt-in (`#derive`), and works on any type. The flagship target is a pure Ori JSON parser using `$for` + `fields_of(T)` + SIMD intrinsics.

**Proposal:** `proposals/approved/compile-time-reflection-proposal.md`
**Supersedes:** `proposals/superseded/reflection-api-proposal.md` (runtime reflection), Section 20 (old runtime reflection)

**Reference implementations:**
- **Zig** `src/Sema.zig`, `src/InternPool.zig`: `@typeInfo` + `inline for` + `@field` — zero-cost, type-safe, comptime field iteration
- **C++26** P2996: `^^T` reflect, `template for` expansion, `[:expr:]` splice — zero overhead, fully typed

**Depends on:** Section 18 (Const Generics — specifically 18.0 const eval bridge), Section 2 (Type Inference), Section 3 (Traits).

---

## Architecture

```
Parse ──→ Type Check ──→ Monomorphize ──→ ARC Lower ──→ LLVM Codegen
                              ↑
                    $for/$if expansion here
                    fields_of(T) evaluated here
                    splice resolved to field access
                    each expanded copy type-checked
```

**Key insight:** Expansion happens during monomorphization — after type inference resolves `T` to a concrete type, but early enough for expanded code to be type-checked. This is architecturally identical to Zig's `inline for` and C++26's `template for`.

**Data flow:**
1. Parser produces `ExprKind::CompFor`, `ExprKind::CompIf`, `ExprKind::Splice` AST nodes
2. Type checker validates const iterables/conditions; defers expansion to monomorphization
3. Monomorphizer encounters `$for`/`$if` with concrete `T`, evaluates `fields_of(T)`, expands/branches
4. Each expanded copy is independently type-checked with concrete field types
5. ARC lowerer and LLVM codegen see only plain `ExprKind::Field` access — no reflection constructs remain

---

## 20.1 Compile-Time Metadata Types and Intrinsics

> Verified not-started (2026-03-29): No `$FieldMeta`, `$VariantMeta`, `Tag::FieldMeta`, `Tag::VariantMeta` in type pool. No `fields_of`/`variants_of`/`name_of` intrinsic registration. No test files or `tests/spec/reflection/` directory.

**File(s):** `compiler/ori_ir/src/ast/expr.rs`, `compiler/ori_types/src/output/mod.rs`, `compiler/ori_types/src/infer/expr/identifiers.rs`, `compiler/ori_types/src/check/registration/mod.rs`

**Goal:** Register `fields_of(T)`, `variants_of(T)`, `name_of(T)` as compile-time intrinsics and define `$FieldMeta`, `$VariantMeta` as compiler-internal types.

**Depends on:** Section 18.0 (const eval bridge — provides `ori_types` → `ori_eval` invocation path). If 18.0 is not complete, intrinsics can still be registered but cannot be evaluated until the bridge exists.

### Metadata Types

- [ ] **Add `$FieldMeta` representation to the type pool** (`compiler/ori_types/src/pool/`)
  - Two fields: `name: str`, `index: int`
  - Compile-time-only flag — cannot appear in runtime types
  - Follow `Tag` pattern in pool: add `Tag::FieldMeta`, `Tag::VariantMeta`

- [ ] **Add `$VariantMeta` representation**
  - Three fields: `name: str`, `index: int`, `fields: [$FieldMeta]`
  - Same compile-time-only flag

### Intrinsic Registration

- [ ] **Register `fields_of` intrinsic** in `ori_types/src/check/registration/mod.rs`
  - Type signature: `<T>(T) -> [$FieldMeta]` (or type-parameter-only form)
  - Returns empty `[]` for non-struct types (primitives, sum types, collections, tuples)
  - Returns `[$FieldMeta { name: "inner", index: 0 }]` for newtypes
  - Only public fields (excludes `::`-prefixed)
  - Warning W0461 when called on known non-struct literal type

- [ ] **Register `variants_of` intrinsic**
  - Type signature: `<T>(T) -> [$VariantMeta]`
  - Returns empty `[]` for non-sum types

- [ ] **Register `name_of` intrinsic**
  - Type signature: `<T>(T) -> str`
  - Returns unqualified type name without type arguments

### Metadata Extraction

- [ ] **Implement field metadata extraction from type pool**
  - Given a concrete struct `Idx`, extract ordered field list from pool
  - Build `$FieldMeta` values: name from `Name` interner, index from declaration order
  - Handle newtypes: single field named "inner"
  - Handle private fields: skip `::`-prefixed names

### Tests

- [ ] **Matrix tests (types):** struct (multiple fields), struct (single field), struct (no public fields), newtype, sum type, primitive, collection, tuple, generic struct (monomorphized), nested struct
- [ ] **Semantic pin:** `fields_of(NewType)` returns `[{name: "inner", index: 0}]` — only passes with newtype support
- [ ] **TDD:** Write failing tests first → implement → verify pass in debug and release

---

## 20.2 Parser: $for, $if, and Splice Syntax

> Verified not-started (2026-03-29): No `ExprKind::CompFor`, `CompIf`, or `Splice` variants in AST. No `parse_comp_for`/`parse_comp_if` functions. Parser `$` dispatch only produces `ExprKind::Const(name)`. No parse tests for reflection syntax.

**File(s):** `compiler/ori_parse/src/grammar/expr/primary/control_flow.rs`, `compiler/ori_parse/src/grammar/expr/postfix.rs`, `compiler/ori_ir/src/ast/expr.rs`, `compiler/ori_ir/src/token/kind.rs`

**Goal:** Parse `$for`, `$if`, and `value.[field]` splice syntax, producing new AST nodes.

### Parser: $for

- [ ] **Add `ExprKind::CompFor` to IR** (`compiler/ori_ir/src/ast/expr.rs`)
  ```rust
  ExprKind::CompFor {
      pattern: BindingPatternId,
      iter: ExprId,
      guard: ExprId,       // ExprId::INVALID = no guard
      body: ExprId,
      is_yield: bool,
  }
  ```
  Note: No label — `$for` is expansion, not a loop. `break`/`continue` are invalid.

- [ ] **Add `parse_comp_for` in parser** (`control_flow.rs`)
  - Detect `$for` via `TokenKind::Dollar` followed by `for` keyword (adjacency check)
  - Reuse `for` grammar: `$for pattern in const_iterable [if const_guard] do|yield body`
  - Do NOT set `IN_LOOP` context (break/continue are invalid)
  - Follow pattern of `parse_for_loop()` at line 211

### Parser: $if

- [ ] **Add `ExprKind::CompIf` to IR**
  ```rust
  ExprKind::CompIf {
      cond: ExprId,
      then_branch: ExprId,
      else_branch: ExprId,   // ExprId::INVALID = no else
  }
  ```

- [ ] **Add `parse_comp_if` in parser** (`control_flow.rs`)
  - Detect `$if` via `TokenKind::Dollar` followed by `if` keyword
  - Reuse `if` grammar: `$if const_cond then expr [else expr]`
  - Follow pattern of `parse_if_expr()` at line 153

### Parser: Splice Access

- [ ] **Add `ExprKind::Splice` to IR**
  ```rust
  ExprKind::Splice {
      receiver: ExprId,
      field_expr: ExprId,   // compile-time $FieldMeta expression
  }
  ```

- [ ] **Add splice parsing in postfix dispatch** (`postfix.rs`)
  - Detect `.[` after dot token — `value.[field]`
  - In `parse_postfix_dot()` (line 178): if next token after `.` is `[`, parse splice
  - Parse `field_expr` as expression, expect `]`
  - Follow existing index parsing pattern from `apply_postfix_ops()`

### Primary Expression Dispatch

- [ ] **Wire `$for` and `$if` into primary expression dispatch**
  - In `parse_primary()`: when encountering `TokenKind::Dollar`, peek next token
  - `$` + `for` → `parse_comp_for()`
  - `$` + `if` → `parse_comp_if()`
  - `$` + identifier → existing `ExprKind::Const(name)` path

### Tests

- [ ] **Parse tests:** `$for x in items yield x`, `$for x in items do body`, `$for x in items if guard yield body`, `$if cond then a else b`, `$if cond then a` (no else), `value.[field]`, `value.[field] = x` (assignment), nested `$for`/`$if`
- [ ] **Error tests:** `$for` with `break` (error), `$for` with `continue` (error), `$if` without `then` (error), `.[` without matching `]` (error)
- [ ] **Semantic pin:** `$for` parses to `CompFor` not `For` — only passes with new AST variant
- [ ] **TDD:** Write failing parse tests first

---

## 20.3 $for Expansion During Monomorphization

> Verified not-started (2026-03-29): No expansion logic in monomorphization. No `CompFor` handling. BLOCKED on Section 18.0 (const eval bridge, also not-started).

**File(s):** `compiler/ori_types/src/infer/expr/calls/monomorphization.rs`, `compiler/ori_types/src/infer/mod.rs`, `compiler/ori_types/src/pool/substitute/mod.rs`

**Goal:** When monomorphizer encounters `$for` with concrete `T`, expand the loop body N times (once per field/variant), producing independently type-checked expressions.

**Depends on:** 20.1 (metadata intrinsics), 20.2 (parser), Section 18.0 (const eval bridge for evaluating const iterables).

### Expansion Logic

- [ ] **Add expansion sub-phase to monomorphization**
  - Location: after `maybe_record_mono_instance()` resolves all type variables
  - When encountering `ExprKind::CompFor` in a monomorphized function body:
    1. Evaluate const iterable (e.g., `fields_of(User)` → concrete `[$FieldMeta]`)
    2. For each element, duplicate the body with the iteration variable bound to that element
    3. If `is_yield`: collect results into a list expression
    4. If `do`: sequence as void expressions
    5. Type-check each expanded copy independently (field types differ per iteration)

- [ ] **Homogeneous expansion** (const lists)
  - Iterable is a compile-time list of uniform type (e.g., `[1, 2, 3]`)
  - Each body has the same type — result is `[T]`
  - Straightforward unrolling

- [ ] **Heterogeneous expansion** (fields_of/variants_of)
  - Each iteration may produce differently-typed expressions
  - `value.[field]` has type `str` for one field, `int` for another
  - Result type of `$for...yield` is `[T]` where `T` is the **unified type** of all expanded bodies
  - Compile error E0464 if types are incompatible

- [ ] **Guard evaluation**
  - `$for field in fields_of(T) if const_guard yield body`
  - Guard must be compile-time evaluable (e.g., `is_option(value.[field])`)
  - Skip iterations where guard is false

- [ ] **Empty iteration**
  - `$for...yield` over empty list → `[]`
  - `$for...do` over empty list → `void`

### Integration with MonoInstance

- [ ] **Expansion produces new AST nodes** in the expression arena
  - Expanded `$for` becomes a list literal (yield) or block sequence (do)
  - Each expanded body references concrete field types via `body_type_map`
  - The `MonoInstance::body_type_map` must include types from expanded copies

### Tests

- [ ] **Matrix tests (expansion patterns):** homogeneous `$for` (const list), heterogeneous `$for` (fields_of), nested `$for` (variants_of → fields), `$for` with guard, empty iteration, single-field struct, multi-field struct, generic struct monomorphized
- [ ] **Matrix tests (yield vs do):** `$for...yield` (list result), `$for...do` (void), `$for...yield` with type unification
- [ ] **Semantic pin:** `$for field in fields_of(User) yield field.name` produces `["name", "age"]` — only passes with heterogeneous expansion
- [ ] **TDD:** Write failing expansion tests first

---

## 20.4 $if Dead Branch Elimination and Splice Resolution

> Verified not-started (2026-03-29): No compile-time branch elimination, no splice resolution logic. None of E0462/E0463/E0464 error codes registered. BLOCKED on Section 18.0. ERROR CODE DRIFT: E0462-E0464 are type checker errors but assigned to E0xxx (lexer) range -- should be E2xxx per diagnostic conventions.

**File(s):** `compiler/ori_types/src/infer/expr/`, `compiler/ori_types/src/infer/expr/calls/monomorphization.rs`

**Goal:** `$if` conditions resolve at monomorphization time; dead branches are not type-checked. Splice `value.[field]` resolves to direct `ExprKind::Field` access.

**Depends on:** 20.1 (metadata types), 20.2 (parser), 20.3 (expansion context).

### $if Resolution

- [ ] **Evaluate $if condition at monomorphization time**
  - Condition must be compile-time evaluable boolean
  - If true: emit only the then-branch (type-check only then-branch)
  - If false: emit only the else-branch (type-check only else-branch)
  - Dead branch is parsed but NOT type-checked (syntax errors still reported)
  - Compile error E0463 if condition is not compile-time determinable

- [ ] **Support nested $if and $if inside $for**
  - Common pattern: `$if is_struct(T) then { $for field in fields_of(T)... } else { value.to_str() }`
  - Dead branch elimination prevents type errors from irrelevant branch

### Splice Resolution

- [ ] **Resolve `ExprKind::Splice` to `ExprKind::Field` during expansion**
  - Inside `$for field in fields_of(T)`, `value.[field]` resolves to `value.name`, `value.age`, etc.
  - The `field_expr` must evaluate to a `$FieldMeta` value at compile time
  - Extract `field.name` → look up field in struct type → produce `ExprKind::Field { receiver, field: name }`
  - Result type is the concrete field type (resolved from pool)

- [ ] **Splice on left side of assignment**
  - `value.[field] = x` → `value.name = x` (follows existing index/field assignment rules)

- [ ] **Nested splice**
  - `value.[outer_field].[inner_field]` — outer field is itself a struct

### Error Messages

- [ ] **E0462: Splice on non-struct value** — `items.[field]` where `items` is not a struct
- [ ] **E0463: $if with non-const condition** — runtime value in $if condition
- [ ] **E0464: Heterogeneous yield type mismatch** — `$for` yield produces incompatible types

### Tests

- [ ] **Matrix tests ($if):** true branch only, false branch only, dead branch with type error (should not trigger), nested $if, $if inside $for, $if with is_struct/is_enum/is_option
- [ ] **Matrix tests (splice):** single field access, multiple fields in $for, nested struct splice, splice assignment, splice with different field types
- [ ] **Semantic pin:** `$if is_struct(int) then compile_error("oops") else 42` produces `42` — dead branch not type-checked
- [ ] **TDD:** Write failing tests first

---

## 20.5 Type Classification Intrinsics

> Verified not-started (2026-03-29): No Ori-level `is_struct`/`is_enum`/`is_primitive`/`is_collection`/`is_option`/`is_result`/`is_tuple` intrinsic registration. Existing Rust-internal `Tag::is_primitive()` etc. are unrelated (operator dispatch, not Ori compile-time predicates).

**File(s):** `compiler/ori_types/src/check/registration/mod.rs`, `compiler/ori_types/src/infer/expr/identifiers.rs`

**Goal:** `is_struct(T)`, `is_enum(T)`, `is_primitive(T)`, `is_collection(T)`, `is_option(T)`, `is_result(T)`, `is_tuple(T)` — compile-time predicates for type classification, usable in `$if` conditions.

**Depends on:** 20.1 (intrinsic registration pattern), 20.4 ($if for consumption).

### Intrinsic Registration

- [ ] **Register 7 type classification intrinsics**
  - Each takes a type parameter OR an expression (checks static type of expression, does not evaluate)
  - Type parameter form: `is_struct(T) -> bool`
  - Expression form: `is_option(value.[field]) -> bool` — inspects compile-time type
  - Returns compile-time boolean (usable in `$if` conditions)

### Implementation

- [ ] **Type parameter form** — trivial: check `Tag` of resolved type in pool
  - `is_struct`: `Tag::Struct`
  - `is_enum`: `Tag::Sum` / `Tag::Variant`
  - `is_primitive`: check against `Tag::Int`, `Tag::Float`, `Tag::Bool`, `Tag::Str`, `Tag::Char`, `Tag::Byte`, `Tag::Void`
  - `is_collection`: `Tag::List`, `Tag::Map`, `Tag::Set`
  - `is_option`: `Tag::Option`
  - `is_result`: `Tag::Result`
  - `is_tuple`: `Tag::Tuple`

- [ ] **Expression form** — resolve expression's static type, then check tag
  - Inside `$for`, `value.[field]` has a concrete static type at each iteration
  - The intrinsic inspects this type without evaluating the expression

### Tests

- [ ] **Matrix tests (type classification):** struct, sum type, newtype, primitive (int, float, bool, str, char, byte, void), collection ([T], {K:V}, Set<T>), Option, Result, tuple, function type, generic (monomorphized)
- [ ] **Matrix tests (expression form):** `is_option(value.[field])` inside $for, `is_struct(value.[field])` for nested struct, `is_primitive(value.[field])` for mixed-type struct
- [ ] **Semantic pin:** `is_struct(int)` returns `false`, `is_struct(User)` returns `true`
- [ ] **TDD:** Write failing tests first

---

## 20.6 Integration and Verification

> Verified not-started (2026-03-29): No evaluator or LLVM handling for reflection constructs (AST variants do not exist). Spec Clause 27 still contains OLD runtime Reflect model -- body not rewritten despite SUPERSEDED header. ori-syntax.md update is done (design target documentation). `tests/spec/reflection/` directory does not exist. None of E0460/W0461 error codes registered. ERROR CODE DRIFT: E0460/W0461 should be E2xxx range.

**File(s):** `tests/spec/reflection/`, `compiler/ori_eval/src/`, `compiler/ori_llvm/src/`

**Goal:** End-to-end verification that compile-time reflection produces correct, zero-cost code across both eval and LLVM paths.

**Depends on:** All prior subsections (20.1-20.5).

### Evaluator Support

- [ ] **Implement $for expansion in interpreter** (`compiler/ori_eval/src/interpreter/`)
  - Evaluator must handle `ExprKind::CompFor`, `ExprKind::CompIf`, `ExprKind::Splice`
  - Either expand during const-eval or interpret directly (expand inline)
  - Must produce identical results to LLVM path

### LLVM Codegen

- [ ] **Verify $for expansion produces no LLVM overhead**
  - After monomorphization, no `CompFor`/`CompIf`/`Splice` nodes reach codegen
  - All expansion happens before codegen — LLVM sees only plain field access
  - `ExprKind::Splice` is already resolved to `ExprKind::Field` before codegen

### Spec and Documentation

- [ ] **Rewrite spec Clause 27** for compile-time reflection model
  - Replace runtime `Reflect`/`TypeInfo`/`Unknown` with `fields_of`/`variants_of`/`$for`/`$if`/splice
  - Run `/sync-spec` and `/sync-grammar`
- [ ] **Update `.claude/rules/ori-syntax.md`** — already done (2026-03-26 propagation audit)

### End-to-End Tests

- [ ] **Flagship test: structural debug** — `$for field in fields_of(T) yield "{field.name}: {value.[field]}"` for User struct
- [ ] **Flagship test: structural equality** — `$for field in fields_of(T) yield a.[field] == b.[field]` with `.all()`
- [ ] **Recursive reflection test** — `deep_debug<T>` that recurses into nested structs
- [ ] **Default impl test** — `pub def impl ToJson` using `$for` + `fields_of(Self)`
- [ ] **Eval-vs-LLVM equivalence** — run `/code-journey` to verify both paths match

### Error Message Quality

- [ ] **E0460: $for over non-const iterable** — clear message with `$` vs non-`$` hint
- [ ] **W0461: fields_of on known non-struct** — warning, not error
- [ ] **E0462: splice on non-struct value** — show actual type
- [ ] **E0463: $if with non-const condition** — suggest `if` instead
- [ ] **E0464: heterogeneous yield mismatch** — show per-iteration types

---

## 20.R Third Party Review Findings

> Verified done (2026-03-29): "None" is correct -- no implementation exists to review.

- None.

---

## 20.7 Completion Checklist

> Verified not-started (2026-03-29): All 15 items unimplemented. No reflection functionality exists in any compiler phase.

- [ ] `fields_of(T)` returns correct metadata for structs, newtypes, and empty for non-struct types
- [ ] `variants_of(T)` returns correct metadata for sum types
- [ ] `name_of(T)` returns unqualified type name
- [ ] `$for...yield` produces correct list from heterogeneous expansion
- [ ] `$for...do` produces correct void sequence
- [ ] `$if` eliminates dead branch — dead branch with type error does NOT trigger error
- [ ] `value.[field]` resolves to direct field access — zero overhead in LLVM IR
- [ ] Type classification intrinsics work in both type-param and expression form
- [ ] All error messages (E0460-E0464, W0461) are clear and actionable
- [ ] Eval and LLVM paths produce identical results for all reflection tests
- [ ] `./test-all.sh` green
- [ ] `./clippy-all.sh` green
- [ ] Spec Clause 27 rewritten for compile-time model
- [ ] `grammar.ebnf` updated with `$for`, `$if`, splice productions
- [ ] `/tpr-review` passed — independent Codex review found no critical or major issues (or all findings triaged)

**Exit Criteria:** `$for field in fields_of(User) yield field.name` evaluated at compile time produces `["name", "age", "email"]` with zero runtime overhead. LLVM IR for reflection-using code is identical to hand-written field-by-field code. All 5 error codes produce clear, actionable diagnostics. Eval and LLVM paths match for all tests. `./test-all.sh` and `./clippy-all.sh` green.

---

## Verification Notes (2026-03-29)

### ERROR CODE DRIFT

Error codes E0460-E0464 and W0461 are assigned in the E0xxx range (lexer errors per diagnostic conventions), but all reflection errors are type checker/monomorphization errors and belong in the **E2xxx** range. Current highest E2xxx code is E2041. When implementing, either:
1. Reassign to E2042-E2046 (consistent with type checker convention), or
2. Document E04xx as a deliberate "compile-time expansion" sub-range in `.claude/rules/diagnostic.md`.

### SPEC CLAUSE 27 STALE

`docs/ori_lang/v2026/spec/27-reflection.md` contains the OLD runtime Reflect model (`Reflect` trait, `TypeInfo`, `Unknown`, `FieldInfo`, `VariantInfo`, `field_by_index`/`field_by_name`, `#derive(Reflect)`). Has a `SUPERSEDED (2026-03-26)` header but the body has not been rewritten. The `grammar.ebnf` has no `$for`, `$if`, or splice productions. This is tracked in 20.6 but is a significant documentation gap.

### DEPENDENCY: SECTION 18.0 NOT STARTED

The const eval bridge (Section 18.0) is entirely unchecked. This blocks 20.3 (expansion) and 20.4 (resolution) but does NOT block 20.1 (registration) or 20.2 (parsing).

### PLAN QUALITY GAPS (from verification)

The plan is well-structured but has these gaps to address when implementing:
- **Missing negative test specifications**: Positive test matrices lack explicit negative pins (e.g., `$for` with runtime iterable must fail).
- **Missing interaction testing**: No cross-feature interaction tests specified (reflection + closures, reflection + generics, reflection + traits).
- **Missing ARC/memory testing**: No `ORI_CHECK_LEAKS=1` verification mentioned for expanded code.
- **Missing cleanup section**: No final annotation cleanup section per CLAUDE.md requirements.
- **Missing dual-execution verification mechanism**: Completion checklist mentions eval/LLVM parity but does not specify the tool (`/code-journey` or `dual-exec-verify.sh`).
- **Heterogeneous yield type unification**: Plan does not specify what "unified type" means for `$for...yield` when field types differ (Ori has no implicit conversions).
