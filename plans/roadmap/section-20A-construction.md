---
section: "20A"
title: "Compile-Time Struct Construction"
status: not-started
reviewed: true
last_verified: "2026-03-29"
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

**Status:** Not Started (verified 2026-03-29 -- all file paths, line numbers, line counts, ExprKind match sites, arena APIs, error code ranges, dependency chain, and proposal confirmed accurate)
**Goal:** `$construct<T>` and `$construct_partial<T: Default>` expand during monomorphization to direct `ExprKind::Struct` literals (or `ExprKind::Call` for newtypes) — identical codegen to hand-written `T { field1: val1, ... }`. The existing struct pipeline (type checker completeness check, canonical lowering, eval, ARC, LLVM) handles everything downstream. Adding the `ExprKind::Construct` variant requires match arms in 11 files with exhaustive `ExprKind` dispatch, but no semantic changes to downstream phases (eval, ARC, LLVM operate on `CanExpr`, which never sees `Construct`).

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

**Key insight:** `$construct` is an expansion-phase construct. After monomorphization, no `Construct` nodes remain in the IR. The eval, ARC, and LLVM phases see only plain `CanExpr::Struct` (via normal canonicalization of `ExprKind::Struct`) — zero changes needed in those downstream phases. However, adding a new `ExprKind::Construct` variant does require match arms in all 11 files with exhaustive `ExprKind` dispatch (see 20A.1 for the full list). The variant's 9 bytes of payload (`Name` + `ExprId` + `bool`) fits within the existing 24-byte `ExprKind` size budget (`static_assert_size!(ExprKind, 24)` in `ori_ir/src/ast/expr.rs:508`).

---

## 20A.1 Parser: $construct and $construct_partial Syntax (verified 2026-03-29 -- 11/11 items not-started, all file paths and line numbers confirmed accurate)

**File(s):** `compiler/ori_ir/src/ast/expr.rs`, `compiler/ori_parse/src/grammar/expr/primary/literals.rs`

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

- [ ] **Visitor/walker support** — add match arm in all 11 `ExprKind` exhaustive matches
  - `compiler/ori_ir/src/visitor/walk_expr.rs` — add to single-child group: `ExprKind::Construct { args, .. } => { visitor.visit_expr_id(*args, arena); }`
  - `compiler/ori_ir/src/ast/expr.rs` — Debug fmt match (line 368): add `ExprKind::Construct { type_param, args, is_partial } => write!(f, "Construct({type_param:?}, {args:?}, partial={is_partial})")`
  - `compiler/ori_types/src/infer/expr/mod.rs` — type inference dispatch (line 97 match): return type `T` resolved from `type_param`, delegate expansion to Section 20.3 expansion sub-phase (the `Construct` node must be expanded before `infer_struct` runs on the replacement). Concrete arm: `ExprKind::Construct { type_param, args, is_partial } => infer_construct(engine, arena, *type_param, *args, *is_partial, span)` — add `infer_construct` function that resolves the type param, infers args, and returns the resolved struct type
  - `compiler/ori_canon/src/lower/expr.rs` — canonicalization dispatch (line 32 match): `ExprKind::Construct { .. } => unreachable!("$construct must be expanded before canonicalization")` (defense-in-depth)
  - `compiler/ori_fmt/src/formatter/inline.rs` (520 lines — at BLOAT limit) — add arm: emit `$construct<` + type name + `>(` + inline args + `)` (or `$construct_partial<...>`); add to leaf-like group near `Const` handling
  - `compiler/ori_fmt/src/formatter/broken.rs` (511 lines — at BLOAT limit) — mirror inline arm but delegate args to `emit_broken(args)`
  - `compiler/ori_fmt/src/formatter/stacked.rs` — mirror broken arm but delegate args to `emit_stacked(args)`
  - `compiler/ori_fmt/src/width/mod.rs` (579 lines — BLOAT) — add: width = `"$construct<".len()` + type_name_width + `">(".len()` + args_width + `")".len()` (adjust for `_partial` variant)
  - `compiler/oric/src/ir_dump/expr.rs` (617 lines — BLOAT) — line ~124 match: add `ExprKind::Construct { type_param, args, is_partial } => { writeln!(out, "{indent}$construct{partial}<{name}>{ty}", partial = if *is_partial { "_partial" } else { "" }, name = interner.lookup(*type_param)).unwrap(); dump_expr(out, *args, ...); return; }`
  - `compiler/oric/src/ast_dump/expr.rs` — main match at line 33 (exhaustive, no wildcard): add `ExprKind::Construct { type_param, args, is_partial } => { writeln!(out, "{indent}$construct{partial}<{name}>", ...); dump_expr(out, *args, ...); return; }` — the second match at line 536 uses `_ =>` and needs no change
  - `compiler/ori_parse/src/incremental/copier.rs` (1595 lines — BLOAT) — add to copy_expr match (line ~112): `ExprKind::Construct { type_param, args, is_partial } => { let new_args = self.copy_expr(args, new_arena); ExprKind::Construct { type_param, args: new_args, is_partial } }`
  - Adding the variant to `ExprKind` (which derives `Copy, Clone, Eq, PartialEq, Hash`) is safe: `Name`, `ExprId`, and `bool` all implement these traits
  - **Size budget**: variant payload is 9 bytes (`Name(u32)` + `ExprId(u32)` + `bool`), well within the 24-byte `ExprKind` budget enforced by `static_assert_size!(ExprKind, 24)` at `ori_ir/src/ast/expr.rs:508`
  - **Non-exhaustive matches (no changes needed)**: `ori_fmt/src/rules/*.rs`, `ori_fmt/src/width/control.rs`, `ori_arc/src/decision_tree/flatten.rs`, `oric/src/ast_dump/expr.rs` inline match at line 536 (`_ =>` wildcard), `ori_canon/src/desugar/calls.rs` at line 141 (`_ =>` wildcard) (added from verification finding 3)

### Parser Implementation

- [ ] **Add `parse_construct` in parser** (`compiler/ori_parse/src/grammar/expr/primary/literals.rs`)
  - Entry point: `$construct` detected in `parse_misc_primary()` (at `literals.rs:256`) where `TokenKind::Dollar` is currently handled
  - Currently the `$` arm (line 256-263) does: advance past `$`, `expect_ident()` to get name, produce `ExprKind::Const(name)`. To support `$construct`, the ident text must be inspected after `expect_ident()` returns — if name text equals `"construct"` or `"construct_partial"`, call `parse_construct()` instead of producing `Const`. Use `self.interner.lookup(name)` to get the text.
  - Section 20 will add: `$` + `for` → `parse_comp_for()`, `$` + `if` → `parse_comp_if()` (these check the ident text after the same `expect_ident()` call)
  - New: name text `"construct"` → `parse_construct(false, ...)`, name text `"construct_partial"` → `parse_construct(true, ...)`
  - The fast-path at `primary/mod.rs:137` (`TAG_DOLLAR` branch) already routes to `parse_misc_primary()` — no additional wiring needed there

  ```rust
  // Pseudocode — uses committed!() macro and self.cursor.expect() per parser conventions
  fn parse_construct(&mut self, is_partial: bool, start_span: Span) -> ParseOutcome<ExprId> {
      // Parse <T> — type argument
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

- [ ] **Wire into primary expression dispatch**
  - In `parse_misc_primary()` (`literals.rs:256-263`): the current `TokenKind::Dollar` arm is:
    ```rust
    TokenKind::Dollar => {
        self.cursor.advance();
        let name = committed!(self.cursor.expect_ident());
        let full_span = span.merge(self.cursor.previous_span());
        ParseOutcome::consumed_ok(self.arena.alloc_expr(Expr::new(ExprKind::Const(name), full_span)))
    }
    ```
    Replace with: after getting `name`, lookup the text via `self.interner.lookup(name)`. Match on text:
    - `"construct"` → `return self.parse_construct(false, span)`
    - `"construct_partial"` → `return self.parse_construct(true, span)`
    - anything else → fall through to existing `ExprKind::Const(name)` path
  - Coexists with Section 20's `$for` and `$if` dispatch (which will check for `"for"` and `"if"` text in the same match)

### Tests

TDD: Write failing test matrix BEFORE implementation. Verify tests fail first, then implement, then verify tests pass unchanged.

- [ ] **Write failing parse unit tests** in `compiler/ori_parse/src/grammar/expr/primary/literals/tests.rs` — FIRST, before any implementation:

  **Type x pattern matrix:**

  | Input | Expected |
  |-------|----------|
  | `$construct<User>($for field in fields_of(User) yield (field, 42))` | `ExprKind::Construct { is_partial: false, type_param: "User" }` |
  | `$construct_partial<Config>($for field in fields_of(Config) yield (field, value))` | `ExprKind::Construct { is_partial: true, type_param: "Config" }` |
  | `$construct<User>([($name_field, "Alice"), ($age_field, 30)])` | `is_partial: false`, list literal arg |
  | `$construct<T>(expr)` | generic type param `T` |
  | `$construct<Pair>(single_pair)` | single simple expression arg |

  **Edge cases:**
  - `$construct<T>(())` — unit expression as arg (boundary)
  - `$construct<T>($for field in fields_of(T) yield (field, $construct<Inner>(inner_pairs)))` — nested `$construct` inside arg

  **Error cases:**
  - `$construct` without `<T>` — parse error (missing type param)
  - `$construct<>()` — empty type param error
  - `$construct<T>` without `()` — missing args error
  - `$construct_partial` without `<T>()` — same error pattern as `$construct`

  **Roundtrip:** verify `ori fmt` on `$construct<T>(expr)` and `$construct_partial<T>(expr)` reproduces source unchanged

- [ ] **Verify failing tests fail** — run `timeout 150 cargo t -p ori_parse` and confirm all new tests fail before implementing
- [ ] **Spec tests** in `tests/spec/reflection/construct/parse/`:
  - `parse_construct.ori` — `$construct<User>(pairs)` parses without error via `ori check`
  - `parse_construct_partial.ori` — `$construct_partial<Config>(pairs)` parses without error
  - `parse_construct_error.ori` — `#compile_fail` tests for malformed syntax
- [ ] **Semantic pin:** `$construct<T>(expr)` parses to `ExprKind::Construct`, not `ExprKind::Call` — Rust unit test that asserts on the enum discriminant; only passes with the new variant (would fail if `$construct` were parsed as a regular function call)
- [ ] **Verify all tests pass in debug and release** — `timeout 150 cargo t -p ori_parse` and `timeout 150 cargo b --release`

---

## 20A.2 Monomorphization: Expansion to Struct Literal (verified 2026-03-29 -- 14/14 items not-started, arena API references confirmed accurate)

**File(s):** `compiler/ori_types/src/infer/expr/calls/monomorphization.rs` (or new sibling `expansion.rs`), `compiler/ori_types/src/infer/expr/structs/mod.rs`

**Goal:** When the monomorphizer encounters `ExprKind::Construct` with concrete `T`, expand it to a direct `ExprKind::Struct` literal (for structs) or `ExprKind::Call` to the newtype constructor (for newtypes) after validating completeness.

**Depends on:** 20A.1 (parser), Section 20.3 ($for expansion — the `args` expression is already expanded by this point).

### Expansion Logic

**Important context:** The current `monomorphization.rs` only **records** `MonoInstance` records (type substitution maps for generic functions). It does NOT walk or transform the expression AST. Section 20.3 (part of the parent Section 20) will add a new **AST expansion sub-phase** to the monomorphization subsystem — this is the infrastructure that `$construct` expansion relies on. The expansion sub-phase will walk expression trees during type checking (after concrete types are resolved), rewriting compile-time constructs into plain AST nodes before canonicalization.

The expansion happens in the same expansion sub-phase that handles `$for` and `$if` (Section 20.3). The expansion pass processes expressions in pre-order: `$for` expansions complete before parent `$construct` nodes are expanded. This ensures that when `expand_construct()` receives the `args` expression, all nested `$for` nodes have already been fully expanded into concrete value lists.

**Critical dependency on Section 20.3 infrastructure:** The `expand_construct()` pseudocode below calls `self.arena.alloc_expr()` and `self.arena.alloc_field_inits()`. In the current architecture, the `ExprArena` is owned by Salsa and typically immutable during type checking. Section 20.3 must provide a mechanism for arena allocation during expansion — likely either (a) a separate expansion arena that is merged post-expansion, or (b) mutable access to the main arena during the expansion sub-phase. This is a load-bearing architectural decision that 20A.2 consumes but does not define. If Section 20.3 is not yet implemented when work on 20A.2 begins, the expansion logic should be written as a standalone function with an `&mut ExprArena` parameter, ready to be wired into whatever arena-access mechanism Section 20.3 provides.

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
      if struct_def.is_newtype() {
          // Newtypes: emit ExprKind::Call to the newtype constructor
          // fields_of(Newtype) returns [{ name: "inner", index: 0 }]
          // so field_inits has exactly one entry for "inner"
          assert!(field_inits.len() == 1, "newtype must have exactly one field");
          let inner_value = field_inits[0].value.expect("newtype field must have value");
          let ctor_ref = self.arena.alloc_expr(Expr::new(
              ExprKind::Ident(struct_def.name), span));
          // NOTE: ExprRange is built via alloc_expr_list_inline (NOT alloc_expr_range, which does not exist)
          // See: ori_ir/src/arena/range_builders.rs:82
          let args = self.arena.alloc_expr_list_inline(&[inner_value]);
          Ok(self.arena.alloc_expr(Expr::new(ExprKind::Call {
              func: ctor_ref,
              args,
          }, span)))
      } else {
          // Structs: emit ExprKind::Struct — normal struct literal
          // NOTE: FieldInit is Clone (not Copy) — pass Vec directly
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

- [ ] **Add `infer_construct` function** in `compiler/ori_types/src/infer/expr/constructors.rs` (or a new `construct.rs` module)
  - This is the function called from the `ExprKind::Construct` arm in `infer_expr_inner()` (20A.1)
  - Pre-expansion behavior: resolve `type_param` to a concrete type via the type registry, infer the `args` expression, return the resolved struct type as the expression's type
  - Post-expansion (after Section 20.3 infrastructure exists): delegate to `expand_construct()` (20A.2), then re-infer the replacement `ExprKind::Struct`/`ExprKind::Call` node
  - The exact split between 20A.1's type inference arm and 20A.2's expansion depends on Section 20.3's expansion architecture — if expansion runs as a separate sub-phase before type checking, `infer_construct` may just call the expansion and re-dispatch; if expansion is interleaved with type checking, it may do both

- [ ] **Type-check expanded struct literal through existing pipeline**
  - After expansion produces `ExprKind::Struct`, the normal `infer_struct()` runs
  - This provides redundant completeness checking (belt-and-suspenders)
  - Field type unification happens in `infer_struct()` at `ori_types/src/infer/expr/structs/mod.rs:110`
  - No changes needed to `infer_struct()` — it receives a well-formed struct literal

### Tests

TDD: Write failing test matrix BEFORE implementation. Verify tests fail first, then implement, then verify tests pass unchanged.

- [ ] **Write failing expansion unit tests** in `compiler/ori_types/src/infer/expr/calls/monomorphization/tests.rs` (or new `expansion/tests.rs` if expansion is extracted to a separate module) — FIRST, before any implementation:

  **Type x pattern matrix:**

  | Type | $construct (full) | $construct_partial (with guard) | $construct_partial (all defaults) |
  |------|-------------------|--------------------------------|----------------------------------|
  | Multi-field struct (`User { name, age, email }`) | fields_of + yield | yield with guard (skip email) | `[]` empty pairs |
  | Single-field struct (`Wrapper { value: int }`) | single pair | n/a (single field) | `[]` |
  | Newtype (`UserId = int`) | produces `ExprKind::Call` | n/a (newtypes have no Default) | n/a |
  | Generic struct (`Pair<A, B>`) | monomorphized to `Pair<int, str>` | yield with guard | `[]` |
  | Struct with `Option` field | Some/None values | skip Optional field (filled with `None` default) | all None defaults |
  | Struct with nested struct field | nested `$construct` | partial with nested default | `[]` |

  **Edge cases:**
  - Empty struct (zero fields): `$construct<Empty>([])` produces `Empty {}` — boundary condition
  - Struct with all `Option` fields: `$construct_partial` with empty pairs fills all with `None`
  - Single-field struct vs newtype: expansion must distinguish `Wrapper { value: x }` from `UserId(x)`

  **Error dimension (each error code gets its own test):**
  - E0470: missing field — `$construct<User>` with only `name` pair, missing `age` and `email`
  - E0471: duplicate field — two pairs both named `"name"`
  - E0472: extra field — pair with `"nonexistent"` field name
  - E0473: no Default for partial — `$construct_partial<NoDefault>` where `NoDefault` lacks `Default` impl
  - Private fields from outside module — `$construct<HasPrivate>` where `HasPrivate` has private field

- [ ] **Verify failing tests fail** — run `timeout 150 cargo t -p ori_types` and confirm all new tests fail before implementing

- [ ] **Spec tests** in `tests/spec/reflection/construct/` — `.ori` files:
  - `construct_basic.ori` — multi-field struct with `assert_eq` on each field value
  - `construct_newtype.ori` — newtype expansion produces constructor call; verify `UserId(42).inner == 42`
  - `construct_generic.ori` — generic T monomorphized to `Pair<int, str>` and `Pair<bool, float>`
  - `construct_partial.ori` — partial with defaults; verify default-filled fields have correct values
  - `construct_partial_all_defaults.ori` — `$construct_partial<T>([])` fills every field with `Default.default()`
  - `construct_errors.ori` — `#compile_fail` tests for E0470, E0471, E0472, E0473 (one test per error code)
  - `construct_single_field.ori` — single-field struct (non-newtype) boundary case

- [ ] **AOT tests** in `compiler/ori_llvm/tests/aot/construct.rs` — compile and run construct scenarios through LLVM pipeline; verify output matches interpreter

- [ ] **Semantic pin:** `$construct<User>($for field in fields_of(User) yield (field, default_value(field)))` produces `User { name: "", age: 0, email: None }` — only passes with construct expansion producing correct struct literal; would fail if expansion were removed or broken
- [ ] **Semantic pin (partial):** `$construct_partial<Config>([])` produces `Config { timeout: 30, retries: 3 }` (all defaults) — only passes with Default synthesis; would fail if Default filling were removed
- [ ] **Semantic pin (newtype):** `$construct<UserId>([(inner_meta, 42)])` produces `UserId(42)` — only passes if newtype path emits `ExprKind::Call` (not `ExprKind::Struct`)
- [ ] **Verify all tests pass in debug and release** — `timeout 150 cargo t -p ori_types` and `timeout 150 cargo b --release`

---

## 20A.3 Integration, Error Messages, and Verification (verified 2026-03-29 -- 21/21 items not-started, error code range E0470-E0473 confirmed unused and safe)

**File(s):** `compiler/ori_diagnostic/src/error_code/mod.rs`, `tests/spec/reflection/construct/`, `compiler/ori_eval/src/`, `compiler/ori_llvm/src/`

**Goal:** Error messages are clear and actionable. End-to-end verification confirms construct produces identical output to hand-written struct literals in both eval and LLVM paths.

**Depends on:** 20A.1 (parser), 20A.2 (monomorphization expansion).

### Error Code Registration

- [ ] **Register error codes E0470-E0473** (`compiler/ori_diagnostic/src/error_code/mod.rs`)

  **Note on error code range:** E0470-E0473 are in the E0xxx range which is nominally the "Lexer errors" range per the `define_error_codes!` comment in `error_code/mod.rs` (line 29). These are actually monomorphization/expansion errors. This follows Section 20's convention (E0460-E0464 for reflection errors). Both sections use the E0xxx range because they represent compile-time intrinsic errors that occur before type checking proper. The E0xxx range currently only uses E0001-E0015, E0911, E0932 — so E0460-E0473 are safely in unused territory with no collision risk (verified 2026-03-29). If the error code naming convention is revised to give compile-time intrinsics their own range, both Section 20 and 20A codes should move together. **ACTION (from verification finding 1):** When implementing, update the `define_error_codes!` comment at line 29 to reflect that E04xx now includes compile-time intrinsic errors, not just lexer errors.

  | Code | Message | Context |
  |------|---------|---------|
  | E0470 | `$construct<T> is missing field 'name'` | Completeness check failed — field in struct but not in pairs |
  | E0471 | `$construct<T> has duplicate field 'name'` | Same $FieldMeta appears twice in pair list |
  | E0472 | `field 'name' does not exist on type T` | $FieldMeta doesn't match any field in T |
  | E0473 | `$construct_partial requires T: Default` | Partial construction used but T lacks Default impl |

- [ ] **Error message factories** — follow existing pattern in `ori_types/src/type_error/check_error/mod.rs`
  - `missing_field_in_construct(span, type_name, missing_fields)` — list missing fields with types
  - `duplicate_field_in_construct(span, field_name)` — point to first and second occurrence
  - `field_not_in_type(span, field_name, type_name)` — list actual fields of T
  - `construct_partial_no_default(span, type_name)` — suggest adding `: Default` or using `$construct`

- [ ] **Create error documentation** in `compiler/ori_diagnostic/src/errors/`
  - `E0470.md`, `E0471.md`, `E0472.md`, `E0473.md` with examples and fixes

### Canonicalization Handling

- [ ] **Verify `unreachable!()` arm for `Construct` in canonicalization dispatch** (added in 20A.1 as part of the 11-file match arm task)
  - `compiler/ori_canon/src/lower/expr.rs` has an exhaustive match on `ExprKind` (line 32)
  - The arm `ExprKind::Construct { .. } => unreachable!("$construct must be expanded before canonicalization")` was added in 20A.1
  - Verify this defense-in-depth guard is in place — if expansion fails to run, canonicalization must panic rather than silently miscompile

### Evaluator Handling

- [ ] **Verify no eval changes needed**
  - After expansion and canonicalization, `ExprKind::Construct` has been rewritten to `ExprKind::Struct` (structs) or `ExprKind::Call` (newtypes), which become `CanExpr::Struct` or `CanExpr::Call` respectively
  - The evaluator dispatches on `CanExpr` (not `ExprKind`) and already handles both `CanExpr::Struct` via `eval_can_struct()` and `CanExpr::Call` via existing call dispatch
  - No new `CanExpr` variant is needed — the evaluator never sees `Construct`

### LLVM Codegen Handling

- [ ] **Verify no LLVM changes needed**
  - After expansion and canonicalization, LLVM codegen sees `CanExpr::Struct` (routed through ARC IR as `ArcInstr::Construct { CtorKind::Struct(name), args }`, then to `build_struct()` at `ori_llvm/src/codegen/ir_builder/aggregates.rs:179`) or `CanExpr::Call` (for newtypes, routed through normal call codegen)
  - No new `CanExpr` variant is added, so no LLVM changes needed

### ARC Pipeline

- [ ] **Verify no ARC changes needed**
  - `CanExpr::Struct` is already lowered to `ArcInstr::Construct { CtorKind::Struct(name), args }` via `ori_arc/src/lower/collections/mod.rs:55-69`; `CanExpr::Call` flows through normal call lowering
  - No new `CanExpr` variant is added, so no ARC changes needed

### End-to-End Verification

TDD: Write flagship test files BEFORE verifying downstream phases. These tests exercise the full pipeline (parse -> type check -> expand -> canonicalize -> eval/LLVM) and serve as the integration test matrix.

All flagship tests go in `tests/spec/reflection/construct/`.

**Integration test matrix (type x construction pattern):**

| Type | $construct (full) | $construct_partial | Newtype path |
|------|-------------------|-------------------|--------------|
| `User { name: str, age: int }` | from_json.ori | partial_defaults.ori | n/a |
| `Config: Default { timeout, retries, verbose }` | n/a | partial_defaults.ori | n/a |
| `UserId = int` (newtype) | newtype.ori | n/a | newtype.ori |
| Generic `T` monomorphized to multiple types | from_json.ori (via `pub def impl`) | n/a | n/a |
| Empty struct `Unit = {}` | construct_empty.ori | n/a | n/a |

- [ ] **Flagship test: generic FromJson** (`tests/spec/reflection/construct/from_json.ori`) [SEMANTIC PIN — only passes if `$construct<Self>` correctly expands per-type via monomorphization]
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
  Verify this expands to `User { name: ..., age: ... }` for User. Use `assert_eq` on field values.

- [ ] **Flagship test: partial construction with defaults** (`tests/spec/reflection/construct/partial_defaults.ori`) [SEMANTIC PIN — only passes if Default synthesis fills missing fields]
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

- [ ] **Flagship test: newtype construction** (`tests/spec/reflection/construct/newtype.ori`) [SEMANTIC PIN — only passes if newtype path emits `ExprKind::Call`, not `ExprKind::Struct`]
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

- [ ] **Flagship test: empty struct** (`tests/spec/reflection/construct/construct_empty.ori`) — boundary case: `$construct<Unit>([])` for `type Unit = {}` produces `Unit {}`

- [ ] **Error message verification** — run each `#compile_fail` test in `tests/spec/reflection/construct/construct_errors.ori` and verify error messages contain:
  - E0470: the missing field name(s) and the type name
  - E0471: the duplicate field name and span of first occurrence
  - E0472: the invalid field name and list of valid fields
  - E0473: the type name and suggestion to add `: Default`

- [ ] **Zero-overhead verification** — LLVM IR for `$construct<User>(...)` must be identical to `User { name: x, age: y }`
  - Use `diagnostics/ir-diff.sh` to compare hand-written vs $construct versions
  - Also verify `$construct<UserId>(...)` produces identical IR to `UserId(value)`
  - No extra instructions, no intermediate allocations

- [ ] **Eval-vs-LLVM equivalence** — run reflection construct tests through both paths
  - Use `diagnostics/dual-exec-verify.sh tests/spec/reflection/construct/`

- [ ] **AOT end-to-end** — compile and run flagship tests via `ori build` + execute native binary; verify output matches interpreter

- [ ] **Verify all tests pass in debug and release** — `timeout 150 ./test-all.sh` (full suite) and `timeout 150 cargo b --release`

### Spec and Documentation

- [ ] **Update spec Clause 27** — add $construct/$construct_partial to the compile-time reflection clause
  - Run `/sync-spec` for formal language
- [ ] **Update `grammar.ebnf`** — add `$construct` and `$construct_partial` productions
  - Run `/sync-grammar`
- [ ] **Verify `.claude/rules/ori-syntax.md`** — add $construct to Compile-Time Reflection section

---

## 20A.R Third Party Review Findings

- None. (verified 2026-03-29 -- correct, no implementation exists to review)

---

### Verification Findings (2026-03-29)

Seven minor findings from independent verification. No blockers; all informational or minor quality items to address during implementation:

1. **Error code range comment** (NOTE) -- E0470-E0473 are in the E0xxx range documented as "Lexer errors" in `error_code/mod.rs` line 29. When implementing, update the `define_error_codes!` comment to reflect expanded range usage for compile-time intrinsic errors (follows Section 20's E0460-E0464 convention).

2. **Missing plan annotation cleanup section** (DRIFT) -- Per CLAUDE.md: "Every plan MUST include a final cleanup section to strip all its code annotations." The 20A.4 Completion Checklist does not include a cleanup step to remove `20A.x` code annotations. Added below.

3. **Missing `ori_canon/src/desugar/calls.rs` in non-exhaustive list** (NOTE) -- `compiler/ori_canon/src/desugar/calls.rs` has ExprKind references with a wildcard match (`_ =>` at line 141). Not mentioned in the "Non-exhaustive matches (no changes needed)" list. No change needed (wildcard handles it), but noted for completeness.

4. **Missing interaction testing spec** (WEAK TESTS) -- Test matrices focus on type x construction-pattern dimensions but do not explicitly plan interaction tests with closures, error handling (`?`), pattern matching, or trait default impl bodies. The FromJson flagship covers some interactions, but explicit interaction test items should be added during implementation.

5. **Newtype `$construct_partial` behavior under-specified** (GAP) -- The test matrix marks newtype + `$construct_partial` as "n/a" without specifying the behavior of `$construct_partial<UserId>([])`. Should clarify: either error (no Default for newtypes) or fill with `Default.default()` for the inner type. The expansion logic suggests Default-fill would work if the inner type has Default, but the plan's test matrix omits coverage.

6. **`alloc_expr_range` non-existence note accurate** (NOTE) -- The plan correctly notes that `alloc_expr_range` does not exist and `alloc_expr_list_inline` must be used instead. Verified accurate.

7. **Section 20.3 arena access uncertainty properly flagged** (NOTE) -- The plan correctly identifies the load-bearing dependency on Section 20.3's arena allocation mechanism and offers two approaches. This is proper dependency documentation, not a gap.

---

### Codebase Hygiene Findings (discovered during plan review)

These files are touched by this section and have existing hygiene issues. Fix along the way when adding `Construct` match arms:

| File | Lines | Issue | Action |
|------|-------|-------|--------|
| `ori_fmt/src/formatter/inline.rs` | 520 | At BLOAT limit (500 excl. tests) | Adding arm pushes over — evaluate if any stale/duplicated arm logic can be extracted |
| `ori_fmt/src/formatter/broken.rs` | 511 | At BLOAT limit | Same as inline.rs |
| `ori_fmt/src/width/mod.rs` | 579 | BLOAT (79 lines over) | Extract width helpers for complex variants to `width/helpers.rs` when adding Construct |
| `oric/src/ir_dump/expr.rs` | 617 | BLOAT (117 lines over) | Extract leaf-node dump to `ir_dump/expr_leaves.rs` |
| `oric/src/ast_dump/expr.rs` | 587 | BLOAT (87 lines over) | Extract leaf-node dump to `ast_dump/expr_leaves.rs` |
| `ori_parse/src/incremental/copier.rs` | 1595 | Severe BLOAT (3x limit) | Not in scope for 20A, but note for future extraction |
| `ori_ir/src/ast/expr.rs` | 510 | Barely over limit | Adding Construct variant is +3 enum lines and +1 Debug line — minor |

These are not blocking issues for 20A, but implementers should extract where practical while touching these files to avoid worsening the BLOAT.

---

## 20A.4 Completion Checklist (verified 2026-03-29 -- all 21 items confirmed not-started, correct for section status)

**Functional correctness:**
- [ ] `$construct<User>($for field in fields_of(User) yield (field, value))` produces correct `User` struct
- [ ] `$construct_partial<Config>([])` produces `Config` with all default values
- [ ] Generic `T` works — `$construct<T>` in generic function monomorphized to multiple types
- [ ] Newtype works — `$construct<UserId>(...)` expands to `UserId(value)` via `ExprKind::Call`, not `ExprKind::Struct`
- [ ] Empty struct works — `$construct<Unit>([])` produces `Unit {}`

**Error diagnostics:**
- [ ] `$construct<T>` with missing field emits E0470 with field name and type
- [ ] `$construct<T>` with duplicate field emits E0471 with both locations
- [ ] `$construct<T>` with extra field emits E0472 with valid field list
- [ ] `$construct_partial<T>` without Default emits E0473 with suggestion
- [ ] E0470-E0473 documentation files created in `compiler/ori_diagnostic/src/errors/`

**Codegen and equivalence:**
- [ ] LLVM IR identical to hand-written struct literal (`ir-diff.sh` zero differences)
- [ ] Eval and LLVM paths produce identical results (`dual-exec-verify.sh`)
- [ ] AOT end-to-end: flagship tests compile and run as native binaries with correct output

**Structural integrity:**
- [ ] No `ExprKind::Construct` nodes survive past monomorphization (`unreachable!()` in `ori_canon/src/lower/expr.rs` does not fire)
- [ ] All 11 exhaustive `ExprKind` match sites have arms for `Construct`
- [ ] `static_assert_size!(ExprKind, 24)` still passes (variant fits within size budget)

**Testing completeness:**
- [ ] Parse unit tests cover type x pattern matrix (20A.1)
- [ ] Expansion unit tests cover type x pattern x error matrix (20A.2)
- [ ] AOT tests in `compiler/ori_llvm/tests/aot/construct.rs` (20A.2)
- [ ] Spec tests in `tests/spec/reflection/construct/` cover all flagship scenarios (20A.3)
- [ ] Semantic pin tests exist for: full construct, partial construct, newtype construct
- [ ] All tests pass in both debug and release builds

**Suite and documentation:**
- [ ] `./test-all.sh` green
- [ ] `./clippy-all.sh` green
- [ ] Spec Clause 27 updated with $construct/$construct_partial
- [ ] `grammar.ebnf` updated with $construct productions
- [ ] `.claude/rules/ori-syntax.md` updated with $construct in Compile-Time Reflection section
- [ ] `/tpr-review` passed — independent Codex review found no critical or major issues (or all findings triaged)
- [ ] `/impl-hygiene-review last commit` passed — implementation hygiene review clean (phase boundaries, SSOT, algorithmic DRY, naming). MUST run AFTER `/tpr-review` is clean.

**Cleanup:**
- [ ] Remove all `20A.x` code annotations from source files (per CLAUDE.md: plan annotations are temporary scaffolding)

**Exit Criteria:** `$construct<User>($for field in fields_of(User) yield (field, parse(field.name)))` compiles to identical LLVM IR as `User { name: parse("name"), age: parse("age") }`. `$construct<UserId>(...)` expands to `UserId(value)` via constructor call (not struct literal). All 4 error codes (E0470-E0473) produce clear diagnostics. Generic, newtype, partial, and empty-struct construction all work. Eval and LLVM paths match. All tests pass in debug and release. `./test-all.sh` and `./clippy-all.sh` green.
