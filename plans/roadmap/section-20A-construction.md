---
section: "20A"
title: "Compile-Time Struct Construction"
status: not-started
reviewed: true
tier: 8
goal: "$construct<T> and $construct_partial<T> expand to direct struct literals during monomorphization — zero overhead, fully typed, complete field coverage"
inspired_by:
  - "Zig zirStructInit() completeness + default field mechanism (src/Sema.zig:19438-19750)"
  - "C++26 P2996 template for + splice struct construction"
depends_on: ["20"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "20A.1"
    title: "Parser: $construct and $construct_partial Syntax"
    status: not-started
  - id: "20A.2"
    title: "Monomorphization: Expansion to Struct Literal"
    status: not-started
  - id: "20A.3"
    title: "Integration, Error Messages, and Verification"
    status: not-started
  - id: "20A.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "20A.4"
    title: "Completion Checklist"
    status: not-started
---

# Section 20A: Compile-Time Struct Construction

**Status:** Not Started
**Goal:** `$construct<T>` and `$construct_partial<T: Default>` expand during monomorphization to direct `ExprKind::Struct` literals (or `ExprKind::Call` for newtypes) — identical codegen to hand-written `T { field1: val1, ... }`. The existing struct pipeline (type checker completeness check, canonical lowering, eval, ARC, LLVM) handles everything downstream. Adding the `ExprKind::Construct` variant requires match arms in 11 files with exhaustive `ExprKind` dispatch, but no semantic changes to downstream phases (eval, ARC, LLVM operate on `CanExpr`, which never sees `Construct`). <!-- reviewed: cohesion fix -->

**Context:** The approved `compile-time-construction-proposal.md` (2026-03-26) completes the compile-time reflection story by adding **construction** to complement the **inspection** primitives from Section 20 (`fields_of`, `$for`, splice). Without construction, generic deserialization is impossible — you can iterate fields and parse values, but cannot assemble them into a typed struct. The flagship use case is a pure Ori JSON parser (`pub def impl FromJson`) that uses `$construct<Self>` with `$for field in fields_of(Self)`.

**Proposal:** `proposals/approved/compile-time-construction-proposal.md`

**Reference implementations:**
- **Zig** `src/Sema.zig:19438-19750`: `zirStructInit()` — completeness checking at struct literal level after inline-for expansion, optional default field values via `structFieldDefaultValue()`
- **C++26** P2996: `template for` expansion produces struct initializer list — completeness enforced by aggregate initialization rules

**Depends on:** Section 20 (Compile-Time Reflection — provides `fields_of(T)`, `$for`, `$FieldMeta`, `$if`, splice, monomorphization expansion infrastructure).

---

## Architecture

```
$construct<T>(                     ExprKind::Construct
    $for field in fields_of(T)       { type_param: Name,
        yield (field, value)            args: ExprId,
)                                       is_partial: false }
    |                                       |
    v (monomorphization)                    v
    $for already expanded              Extract ($FieldMeta, value) pairs
    by Section 20 machinery            from expanded args expression
    |                                       |
    v                                       v
                                   Match FieldMeta.name to struct fields
                                   Completeness check (E0470-E0472)
                                   For partial: fill missing with Default.default()
                                       |
                                       v
                                   Struct: ExprKind::Struct { name, fields }
                                   Newtype: ExprKind::Call { func, args }
                                   (normal struct literal or ctor call — existing pipeline)
                                       |
                                       v
                                   infer_struct (ori_types)
                                   [unchanged — receives well-formed struct literal]
                                       |
                                       v
                                   ori_canon: ExprKind::Struct → CanExpr::Struct
                                   [unchanged — normal canonicalization]
                                       |
                          +------------+------------+
                          |            |            |
                          v            v            v
                     eval_can_struct  lower_struct   build_struct
                      (ori_eval)      (ori_arc)      (ori_llvm)
                     [unchanged]    [unchanged]     [unchanged]
```

**Key insight:** `$construct` is an expansion-phase construct. After monomorphization, no `Construct` nodes remain in the IR. The eval, ARC, and LLVM phases see only plain `CanExpr::Struct` (via normal canonicalization of `ExprKind::Struct`) — zero changes needed in those downstream phases. However, adding a new `ExprKind::Construct` variant does require match arms in all 11 files with exhaustive `ExprKind` dispatch (see 20A.1 for the full list). The variant's 9 bytes of payload (`Name` + `ExprId` + `bool`) fits within the existing 24-byte `ExprKind` size budget (`static_assert_size!(ExprKind, 24)` in `ori_ir/src/ast/expr.rs:508`). <!-- reviewed: cohesion fix -->

---

## 20A.1 Parser: $construct and $construct_partial Syntax

**File(s):** `compiler/ori_ir/src/ast/expr.rs`, `compiler/ori_parse/src/grammar/expr/primary/literals.rs` <!-- reviewed: accuracy fix — Dollar token dispatch is in parse_misc_primary() in literals.rs, not control_flow.rs -->

**Goal:** Parse `$construct<T>(expr)` and `$construct_partial<T>(expr)` as compile-time intrinsic calls, producing a new `ExprKind::Construct` AST node.

### IR Representation

- [ ] **Add `ExprKind::Construct` variant to `ExprKind`** (`compiler/ori_ir/src/ast/expr.rs`)
  ```rust
  /// Compile-time struct construction: $construct<T>(field_pairs)
  /// Expands to ExprKind::Struct during monomorphization.
  Construct {
      /// The type to construct (resolved to concrete type at monomorphization)
      type_param: Name,
      /// Single expression producing compile-time [($FieldMeta, value)] pairs
      /// Typically a $for...yield expression
      args: ExprId,
      /// true = $construct_partial (allows missing fields, requires T: Default)
      is_partial: bool,
  },
  ```

- [ ] **Visitor/walker support** — add match arm in all 11 `ExprKind` exhaustive matches <!-- reviewed: cohesion fix — verified exact count via wildcard analysis -->
  - `compiler/ori_ir/src/visitor/walk_expr.rs` — visit `args` recursively
  - `compiler/ori_ir/src/ast/expr.rs` — Debug fmt match (line 368)
  - `compiler/ori_types/src/infer/expr/mod.rs` — type inference dispatch: return type `T` resolved from `type_param`, delegate expansion to Section 20.3 expansion sub-phase (the `Construct` node must be expanded before `infer_struct` runs on the replacement)
  - `compiler/ori_canon/src/lower/expr.rs` — canonicalization dispatch (`unreachable!()` — must be expanded before canon)
  - `compiler/ori_fmt/src/formatter/inline.rs`, `broken.rs`, `stacked.rs` — formatting (emit `$construct<T>(...)` / `$construct_partial<T>(...)` as-is)
  - `compiler/ori_fmt/src/width/mod.rs` — width calculation (sum of `$construct<` + type name + `>(` + args width + `)`)
  - `compiler/oric/src/ir_dump/expr.rs` (line 124) — IR debug dump (exhaustive, no wildcard)
  - `compiler/oric/src/ast_dump/expr.rs` (line 33) — AST debug dump (exhaustive, no wildcard; the second match at line 519 uses `_ =>` and needs no change)
  - `compiler/ori_parse/src/incremental/copier.rs` — incremental parse copier (copy `type_param`, recurse into `args`, copy `is_partial`)
  - Adding the variant to `ExprKind` (which derives `Copy, Clone, Eq, PartialEq, Hash`) is safe: `Name`, `ExprId`, and `bool` all implement these traits
  - **Size budget**: variant payload is 9 bytes (`Name(u32)` + `ExprId(u32)` + `bool`), well within the 24-byte `ExprKind` budget enforced by `static_assert_size!(ExprKind, 24)` at `ori_ir/src/ast/expr.rs:508` <!-- reviewed: cohesion fix -->
  - **Non-exhaustive matches (no changes needed)**: `ori_fmt/src/rules/*.rs`, `ori_fmt/src/width/control.rs`, `ori_arc/src/decision_tree/flatten.rs`, `oric/src/ast_dump/expr.rs` inline match (line 519) — all use `_ =>` wildcard arms

### Parser Implementation

- [ ] **Add `parse_construct` in parser** (`compiler/ori_parse/src/grammar/expr/primary/literals.rs`) <!-- reviewed: accuracy fix — Dollar handling is in literals.rs:parse_misc_primary(), not control_flow.rs -->
  - Entry point: `$construct` detected in `parse_misc_primary()` (at `literals.rs:256`) where `TokenKind::Dollar` is currently handled
  - Currently `$` + ident only produces `ExprKind::Const(name)` — extend to check identifier text
  - Section 20 will add: `$` + `for` → `parse_comp_for()`, `$` + `if` → `parse_comp_if()`
  - New: `$` + `construct` → `parse_construct()`, `$` + `construct_partial` → `parse_construct()` with `is_partial=true`
  - Also wire into fast-path at `primary/mod.rs:137` (`TAG_DOLLAR` branch) since `$construct` starts with `$`

  ```rust
  // Pseudocode — uses committed!() macro and self.cursor.expect() per parser conventions
  fn parse_construct(&mut self, is_partial: bool, start_span: Span) -> ParseOutcome<ExprId> {
      // Parse <T> — type argument <!-- reviewed: accuracy fix — parser API uses self.cursor.expect(), not self.expect() -->
      committed!(self.cursor.expect(&TokenKind::Lt));
      let type_name = committed!(self.cursor.expect_ident());
      committed!(self.cursor.expect(&TokenKind::Gt));

      // Parse (expr) — single argument expression
      committed!(self.cursor.expect(&TokenKind::LParen));
      let args = self.parse_expr()?;
      committed!(self.cursor.expect(&TokenKind::RParen));

      let span = start_span.merge(self.cursor.previous_span());
      ParseOutcome::consumed_ok(self.arena.alloc_expr(Expr::new(ExprKind::Construct {
          type_param: type_name,
          args,
          is_partial,
      }, span)))
  }
  ```

- [ ] **Wire into primary expression dispatch** <!-- reviewed: accuracy fix — dispatch is in parse_misc_primary, not parse_primary directly -->
  - In `parse_misc_primary()` (`literals.rs`): after consuming `$`, check if the next identifier is `construct` or `construct_partial`
  - If next is identifier `construct` → `parse_construct(false, ...)`
  - If next is identifier `construct_partial` → `parse_construct(true, ...)`
  - Otherwise fall through to existing `ExprKind::Const(name)` path
  - Coexists with Section 20's `$for` and `$if` dispatch (which will likely be added in the same function or in a shared `$`-prefix dispatcher)

### Tests

- [ ] **Parse tests (TDD — write first, verify fail):**
  - `$construct<User>($for field in fields_of(User) yield (field, 42))` — parses to `Construct`
  - `$construct_partial<Config>($for field in fields_of(Config) yield (field, value))` — `is_partial=true`
  - `$construct<User>([($name_field, "Alice"), ($age_field, 30)])` — list literal arg
  - `$construct<T>(expr)` — generic type param
  - **Matrix (error cases):** `$construct` without `<T>` (error), `$construct<>()` (error), `$construct<T>` without `()` (error)
- [ ] **Semantic pin:** `$construct<T>(expr)` parses to `ExprKind::Construct`, not `ExprKind::Call` — only passes with new variant
- [ ] **Verify all tests pass in debug and release**

---

## 20A.2 Monomorphization: Expansion to Struct Literal

**File(s):** `compiler/ori_types/src/infer/expr/calls/monomorphization.rs` (or new sibling `expansion.rs`), `compiler/ori_types/src/infer/expr/structs/mod.rs` <!-- reviewed: accuracy/feasibility fix — monomorphization.rs currently only records MonoInstance records; it does not walk or transform the AST. Section 20.3 will add AST expansion infrastructure to the monomorphization subsystem. -->

**Goal:** When the monomorphizer encounters `ExprKind::Construct` with concrete `T`, expand it to a direct `ExprKind::Struct` literal (for structs) or `ExprKind::Call` to the newtype constructor (for newtypes) after validating completeness. <!-- reviewed: cohesion fix — newtypes use constructor calls, not struct literals -->

**Depends on:** 20A.1 (parser), Section 20.3 ($for expansion — the `args` expression is already expanded by this point).

### Expansion Logic

**Important context:** The current `monomorphization.rs` only **records** `MonoInstance` records (type substitution maps for generic functions). It does NOT walk or transform the expression AST. Section 20.3 (part of the parent Section 20) will add a new **AST expansion sub-phase** to the monomorphization subsystem — this is the infrastructure that `$construct` expansion relies on. The expansion sub-phase will walk expression trees during type checking (after concrete types are resolved), rewriting compile-time constructs into plain AST nodes before canonicalization.

The expansion happens in the same expansion sub-phase that handles `$for` and `$if` (Section 20.3). The expansion pass processes expressions in pre-order: `$for` expansions complete before parent `$construct` nodes are expanded. This ensures that when `expand_construct()` receives the `args` expression, all nested `$for` nodes have already been fully expanded into concrete value lists.

**Critical dependency on Section 20.3 infrastructure:** The `expand_construct()` pseudocode below calls `self.arena.alloc_expr()` and `self.arena.alloc_field_inits()`. In the current architecture, the `ExprArena` is owned by Salsa and typically immutable during type checking. Section 20.3 must provide a mechanism for arena allocation during expansion — likely either (a) a separate expansion arena that is merged post-expansion, or (b) mutable access to the main arena during the expansion sub-phase. This is a load-bearing architectural decision that 20A.2 consumes but does not define. If Section 20.3 is not yet implemented when work on 20A.2 begins, the expansion logic should be written as a standalone function with an `&mut ExprArena` parameter, ready to be wired into whatever arena-access mechanism Section 20.3 provides. <!-- reviewed: cohesion fix — hidden dependency made explicit -->

- [ ] **Add `expand_construct` to monomorphization** (`monomorphization.rs`)
  ```rust
  fn expand_construct(
      &mut self,
      type_param: Name,   // resolved to concrete struct type
      args: ExprId,        // expanded $for result: list of ($FieldMeta, value) pairs
      is_partial: bool,
      span: Span,
  ) -> Result<ExprId, TypeCheckError> {
      // 1. Resolve T to concrete struct type from MonoInstance
      let struct_type = self.resolve_type(type_param)?;
      let struct_def = self.get_struct_def(struct_type)?;

      // 2. Extract ($FieldMeta, value) pairs from expanded args
      let pairs = self.extract_field_value_pairs(args)?;

      // 3. Match each $FieldMeta.name to struct field
      let mut field_inits: Vec<FieldInit> = Vec::new();
      let mut seen_fields: FxHashSet<Name> = FxHashSet::default();

      for (meta, value_expr) in &pairs {
          let field_name = meta.name; // interned Name
          // E0471: duplicate
          if !seen_fields.insert(field_name) {
              return Err(duplicate_field_in_construct(span, field_name));
          }
          // E0472: field doesn't exist
          if !struct_def.has_field(field_name) {
              return Err(field_not_in_type(span, field_name, struct_type));
          }
          field_inits.push(FieldInit { name: field_name, value: Some(*value_expr), span });
      }

      // 4. Completeness check
      let provided: FxHashSet<Name> = seen_fields;
      let missing: Vec<Name> = struct_def.fields()
          .filter(|f| !provided.contains(&f.name))
          .map(|f| f.name)
          .collect();

      if !missing.is_empty() {
          if is_partial {
              // Fill missing with Default.default()
              for field_name in &missing {
                  let default_call = self.synthesize_default_call(field_name, struct_def)?;
                  field_inits.push(FieldInit { name: *field_name, value: Some(default_call), span });
              }
          } else {
              // E0470: missing field
              return Err(missing_field_in_construct(span, struct_type, missing));
          }
      }

      // 5. Emit the appropriate ExprKind based on type kind
      if struct_def.is_newtype() { // <!-- reviewed: cohesion fix — newtypes use Call, not Struct -->
          // Newtypes: emit ExprKind::Call to the newtype constructor
          // fields_of(Newtype) returns [{ name: "inner", index: 0 }]
          // so field_inits has exactly one entry for "inner"
          assert!(field_inits.len() == 1, "newtype must have exactly one field");
          let inner_value = field_inits[0].value.unwrap();
          let ctor_ref = self.arena.alloc_expr(Expr::new(
              ExprKind::Ident(struct_def.name), span));
          let args = self.arena.alloc_expr_range(&[inner_value]);
          Ok(self.arena.alloc_expr(Expr::new(ExprKind::Call {
              func: ctor_ref,
              args,
          }, span)))
      } else {
          // Structs: emit ExprKind::Struct — normal struct literal
          let field_range = self.arena.alloc_field_inits(field_inits); // returns FieldInitRange (arena/range_builders.rs:199)
          Ok(self.arena.alloc_expr(Expr::new(ExprKind::Struct {
              name: struct_def.name,
              fields: field_range,
          }, span)))
      }
  }
  ```

### Visibility Rules

- [ ] **Enforce visibility at expansion time**
  - `$construct<T>` follows the same visibility rules as struct literal construction
  - `fields_of(T)` already returns only public fields (Section 20.1)
  - Completeness check compares against ALL fields of `T` (including private)
  - If `T` has private fields, the completeness check will report E0470 for each — this is correct behavior: you cannot generically construct a type with private fields from outside its module
  - Types with private fields must implement construction traits manually

### Default Synthesis for $construct_partial

- [ ] **Synthesize `Default.default()` calls for missing fields**
  - For each missing field, resolve its concrete type from `struct_def.field_type(field_name)`, then emit a `Default.default()` call with return type = field type
  - Uses the same trait dispatch mechanism as generic function monomorphization
  - The `Default` bound is checked at monomorphization time when `is_partial = true`. If any field type lacks a `Default` impl, emit E0473. For generic types, require explicit `where T: Default` constraint to use `$construct_partial<T>`
  - Follow Zig's pattern (`Sema.zig:5030-5083`): fetch default value per field, error if no default exists

### Type Checking Integration

- [ ] **Type-check expanded struct literal through existing pipeline**
  - After expansion produces `ExprKind::Struct`, the normal `infer_struct()` runs
  - This provides redundant completeness checking (belt-and-suspenders)
  - Field type unification happens in `infer_struct()` at `ori_types/src/infer/expr/structs/mod.rs:110`
  - No changes needed to `infer_struct()` — it receives a well-formed struct literal

### Tests

- [ ] **Matrix tests (TDD — write failing tests first):**

  **Type dimension:** multi-field struct (`User { name, age, email }`), single-field struct, newtype (`UserId = int` — expansion detects newtype via `struct_def.is_newtype()` and produces `ExprKind::Call` to the constructor, not `ExprKind::Struct`; see proposal 5.4), generic struct (`Pair<A, B>`), struct with Option field, struct with nested struct field <!-- reviewed: cohesion fix — newtypes use Call not Struct -->

  **Pattern dimension:** `$construct<T>($for...yield)` (standard), `$construct<T>(list_literal)` (direct), `$construct_partial<T>($for...yield with guard)` (partial), `$construct_partial<T>([])` (all defaults), generic T monomorphized to different concrete types

  **Error dimension:** missing field (E0470), duplicate field (E0471), extra field (E0472), no Default for partial (E0473), private fields from outside module

- [ ] **Semantic pin:** `$construct<User>($for field in fields_of(User) yield (field, default_value(field)))` produces `User { name: "", age: 0, email: None }` — only passes with construct expansion producing correct struct literal
- [ ] **Semantic pin (partial):** `$construct_partial<Config>([])` produces `Config { timeout: 30, retries: 3 }` (all defaults) — only passes with Default synthesis
- [ ] **Verify all tests pass in debug and release**

---

## 20A.3 Integration, Error Messages, and Verification

**File(s):** `compiler/ori_diagnostic/src/error_code/mod.rs`, `tests/spec/reflection/construct/`, `compiler/ori_eval/src/`, `compiler/ori_llvm/src/`

**Goal:** Error messages are clear and actionable. End-to-end verification confirms construct produces identical output to hand-written struct literals in both eval and LLVM paths.

**Depends on:** 20A.1 (parser), 20A.2 (monomorphization expansion).

### Error Code Registration

- [ ] **Register error codes E0470-E0473** (`compiler/ori_diagnostic/src/error_code/mod.rs`)

  **Note on error code range:** E0470-E0473 are in the E0xxx range which is nominally the "Lexer errors" range per the `define_error_codes!` comment in `error_code/mod.rs`. These are actually monomorphization/expansion errors. This follows Section 20's convention (E0460-E0464 for reflection errors). Both sections use the E0xxx range because they represent compile-time intrinsic errors that occur before type checking proper. If the error code naming convention is revised to give compile-time intrinsics their own range, both Section 20 and 20A codes should move together. <!-- reviewed: cohesion fix — error code range convention noted -->

  | Code | Message | Context |
  |------|---------|---------|
  | E0470 | `$construct<T> is missing field 'name'` | Completeness check failed — field in struct but not in pairs |
  | E0471 | `$construct<T> has duplicate field 'name'` | Same $FieldMeta appears twice in pair list |
  | E0472 | `field 'name' does not exist on type T` | $FieldMeta doesn't match any field in T |
  | E0473 | `$construct_partial requires T: Default` | Partial construction used but T lacks Default impl |

- [ ] **Error message factories** — follow existing pattern in `ori_types/src/type_error/check_error/mod.rs` <!-- reviewed: accuracy fix — errors.rs does not exist; error factories are in type_error/check_error/ -->
  - `missing_field_in_construct(span, type_name, missing_fields)` — list missing fields with types
  - `duplicate_field_in_construct(span, field_name)` — point to first and second occurrence
  - `field_not_in_type(span, field_name, type_name)` — list actual fields of T
  - `construct_partial_no_default(span, type_name)` — suggest adding `: Default` or using `$construct`

- [ ] **Create error documentation** in `compiler/ori_diagnostic/src/errors/` <!-- reviewed: accuracy fix — error docs are at src/errors/, not src/error_code/docs/ -->
  - `E0470.md`, `E0471.md`, `E0472.md`, `E0473.md` with examples and fixes

### Canonicalization Handling <!-- reviewed: feasibility fix — added missing section; canon has exhaustive ExprKind match that needs an arm -->

- [ ] **Add `unreachable!()` arm for `Construct` in canonicalization dispatch**
  - `compiler/ori_canon/src/lower/expr.rs` has an exhaustive match on `ExprKind` (line 32)
  - Add: `ExprKind::Construct { .. } => unreachable!("$construct must be expanded before canonicalization")`
  - This is the defense-in-depth guard — if expansion fails to run, canonicalization will panic rather than silently miscompile

### Evaluator Handling <!-- reviewed: accuracy fix — eval operates on CanExpr, not ExprKind -->

- [ ] **Verify no eval changes needed**
  - After expansion and canonicalization, `ExprKind::Construct` has been rewritten to `ExprKind::Struct` (structs) or `ExprKind::Call` (newtypes), which become `CanExpr::Struct` or `CanExpr::Call` respectively <!-- reviewed: cohesion fix -->
  - The evaluator dispatches on `CanExpr` (not `ExprKind`) and already handles both `CanExpr::Struct` via `eval_can_struct()` and `CanExpr::Call` via existing call dispatch
  - No new `CanExpr` variant is needed — the evaluator never sees `Construct`

### LLVM Codegen Handling <!-- reviewed: accuracy fix — LLVM operates on CanExpr via ARC IR, not ExprKind -->

- [ ] **Verify no LLVM changes needed**
  - After expansion and canonicalization, LLVM codegen sees `CanExpr::Struct` (routed through ARC IR as `ArcInstr::Construct { CtorKind::Struct(name), args }`, then to `build_struct()` at `ori_llvm/src/codegen/ir_builder/aggregates.rs:179`) or `CanExpr::Call` (for newtypes, routed through normal call codegen)
  - No new `CanExpr` variant is added, so no LLVM changes needed

### ARC Pipeline <!-- reviewed: accuracy fix — ARC operates on CanExpr, not ExprKind -->

- [ ] **Verify no ARC changes needed**
  - `CanExpr::Struct` is already lowered to `ArcInstr::Construct { CtorKind::Struct(name), args }` via `ori_arc/src/lower/collections/mod.rs:55-69`; `CanExpr::Call` flows through normal call lowering
  - No new `CanExpr` variant is added, so no ARC changes needed

### End-to-End Verification

- [ ] **Flagship test: generic FromJson**
  ```ori
  type User = { name: str, age: int }

  trait FromJson {
      @from_json (json: JsonValue) -> Result<Self, Error>
  }

  pub def impl FromJson {
      @from_json (json: JsonValue) -> Result<Self, Error> = {
          let obj = json.as_object()?
          $construct<Self>(
              $for field in fields_of(Self) yield {
                  (field, FromJson.from_json(json: obj[field.name])?)
              }
          )
      }
  }
  ```
  Verify this expands to `User { name: ..., age: ... }` for User.

- [ ] **Flagship test: partial construction with defaults**
  ```ori
  type Config: Default = { timeout: int, retries: int, verbose: bool }

  @from_env () -> Config = {
      $construct_partial<Config>(
          $for field in fields_of(Config) if has_env(field.name) yield {
              (field, parse_env(field.name))
          }
      )
  }
  ```

- [ ] **Flagship test: newtype construction** <!-- reviewed: cohesion fix — newtype path verification -->
  ```ori
  type UserId = int

  @from_json_id (json: JsonValue) -> Result<UserId, Error> = {
      $construct<UserId>(
          $for field in fields_of(UserId) yield {
              (field, json.as_int()?)
          }
      )
  }
  ```
  Verify this expands to `UserId(json.as_int()?)` — a constructor call, not a struct literal.

- [ ] **Zero-overhead verification** — LLVM IR for `$construct<User>(...)` must be identical to `User { name: x, age: y }`
  - Use `diagnostics/ir-diff.sh` to compare hand-written vs $construct versions
  - Also verify `$construct<UserId>(...)` produces identical IR to `UserId(value)` <!-- reviewed: cohesion fix -->
  - No extra instructions, no intermediate allocations

- [ ] **Eval-vs-LLVM equivalence** — run reflection construct tests through both paths
  - Use `diagnostics/dual-exec-verify.sh tests/spec/reflection/construct/`

### Spec and Documentation

- [ ] **Update spec Clause 27** — add $construct/$construct_partial to the compile-time reflection clause
  - Run `/sync-spec` for formal language
- [ ] **Update `grammar.ebnf`** — add `$construct` and `$construct_partial` productions
  - Run `/sync-grammar`
- [ ] **Verify `.claude/rules/ori-syntax.md`** — add $construct to Compile-Time Reflection section

---

## 20A.R Third Party Review Findings

- None.

---

## 20A.4 Completion Checklist

- [ ] `$construct<User>($for field in fields_of(User) yield (field, value))` produces correct `User` struct
- [ ] `$construct_partial<Config>([])` produces `Config` with all default values
- [ ] `$construct<T>` with missing field emits E0470 with field name and type
- [ ] `$construct<T>` with duplicate field emits E0471 with both locations
- [ ] `$construct<T>` with extra field emits E0472 with valid field list
- [ ] `$construct_partial<T>` without Default emits E0473 with suggestion
- [ ] Generic `T` works — `$construct<T>` in generic function monomorphized to multiple types
- [ ] Newtype works — `$construct<UserId>(...)` expands to `UserId(value)` via `ExprKind::Call`, not `ExprKind::Struct`
- [ ] LLVM IR identical to hand-written struct literal (`ir-diff.sh` zero differences)
- [ ] Eval and LLVM paths produce identical results (`dual-exec-verify.sh`)
- [ ] No `ExprKind::Construct` nodes survive past monomorphization (`unreachable!()` in `ori_canon/src/lower/expr.rs` does not fire)
- [ ] All 11 exhaustive `ExprKind` match sites have arms for `Construct` <!-- reviewed: cohesion fix -->
- [ ] `static_assert_size!(ExprKind, 24)` still passes (variant fits within size budget) <!-- reviewed: cohesion fix -->
- [ ] `./test-all.sh` green
- [ ] `./clippy-all.sh` green
- [ ] Spec Clause 27 updated with $construct/$construct_partial
- [ ] `grammar.ebnf` updated with $construct productions

**Exit Criteria:** `$construct<User>($for field in fields_of(User) yield (field, parse(field.name)))` compiles to identical LLVM IR as `User { name: parse("name"), age: parse("age") }`. `$construct<UserId>(...)` expands to `UserId(value)` via constructor call (not struct literal). All 4 error codes (E0470-E0473) produce clear diagnostics. Generic, newtype, and partial construction all work. Eval and LLVM paths match. `./test-all.sh` and `./clippy-all.sh` green.
