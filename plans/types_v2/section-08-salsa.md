---
section: "08"
title: Salsa Integration
status: in-progress
goal: Incremental compilation support via Salsa queries
sections:
  - id: "08.1"
    title: Derive Requirements
    status: complete
  - id: "08.2"
    title: Query Design
    status: complete
  - id: "08.3"
    title: TypedModule Output
    status: complete
  - id: "08.4"
    title: Pool Sharing
    status: not-started
  - id: "08.5"
    title: Determinism Guarantees
    status: complete
  - id: "08.6"
    title: Error Rendering Bridge
    status: not-started
  - id: "08.7"
    title: Import Registration API
    status: complete
---

# Section 08: Salsa Integration

**Status:** In Progress (~80%)
**Goal:** Wire V2 type checker into oric's Salsa query pipeline
**Source:** Current oric implementation (`oric/src/query/mod.rs`, `oric/src/typeck.rs`)

---

## Background

### Current oric Architecture (V1)

The Salsa query chain in `oric/src/query/mod.rs`:

```
tokens(db, file) → parsed(db, file) → typed(db, file) → evaluated(db, file)
```

Key integration points:

| Component | Location | Role |
|-----------|----------|------|
| `typed()` query | `oric/src/query/mod.rs:90-94` | Calls `typeck::type_check_with_imports()` |
| Import resolution | `oric/src/typeck.rs:184-261` | Extracts `ImportedFunction` from parsed modules |
| Type checker wrapper | `oric/src/typeck.rs:495-565` | Builds `TypeChecker`, registers imports, calls `check_module()` |
| `evaluated()` query | `oric/src/query/mod.rs:130-205` | Creates `SharedTypeInterner`, shares with checker + evaluator |
| `CompilerDb` | `oric/src/db.rs:59-86` | Salsa storage + string interner + file cache |

### Key Design Constraints

1. **V2 runs alongside V1** — New `typed_v2()` query coexists with `typed()`. No replacement yet (Section 09).
2. **Per-module Pool** — Pool is owned by `ModuleChecker`, NOT a Salsa output (Pool is mutable, not `Eq`/`Hash`).
3. **Cross-arena imports** — Imported functions' `ExprId`s reference foreign arenas. V2 must resolve types using its own Pool.
4. **Error rendering in oric** — V2's `TypeCheckError` uses `Idx` (needs Pool to render). Conversion lives in oric.
5. **Import resolution reuse** — Reuse `resolve_imports_for_type_checking()` for file/path resolution; re-resolve types via V2.

---

## 08.1 Derive Requirements

**Goal:** Ensure all types derive required traits
**Status:** ✅ Complete (2026-02-04)

### Types That Must Derive

```rust
// Core types — Copy + Clone + Eq + PartialEq + Hash + Debug
Idx, Tag, TypeFlags, Rank

// Error types — Clone + Eq + PartialEq + Hash + Debug
TypeCheckError, TypeErrorKind, ErrorContext, ArityMismatchKind
UnifyError, UnifyContext, ArityKind
ContextKind, Expected, ExpectedOrigin, SequenceKind
Suggestion, Replacement, TypeProblem

// Output types — Clone + Eq + PartialEq + Hash + Debug
TypedModuleV2, FunctionSigV2, TypeCheckResultV2
```

### Tasks

- [x] Audit all public types for required derives ✅ (2026-02-04)
- [x] Add derives where missing ✅ (2026-02-04)
- [x] Verify no Salsa-incompatible fields ✅ (2026-02-04)
- [ ] Add compile-time verification (static assert trait bounds)

---

## 08.2 Query Design

**Goal:** Add `typed_v2()` Salsa query that uses the V2 type checker

### Current V1 Query

```rust
// oric/src/query/mod.rs:90-94
#[salsa::tracked]
pub fn typed(db: &dyn Db, file: SourceFile) -> TypedModule {
    let parse_result = parsed(db, file);
    let file_path = file.path(db);
    typeck::type_check_with_imports(db, &parse_result, file_path)
}
```

### V2 Query Design

```rust
// oric/src/query/mod.rs (new)

/// Type check using the V2 type system (runs alongside V1 during migration).
#[salsa::tracked]
pub fn typed_v2(db: &dyn Db, file: SourceFile) -> TypeCheckResultV2 {
    let parse_result = parsed(db, file);
    let file_path = file.path(db);
    typeck_v2::type_check_v2_with_imports(db, &parse_result, file_path)
}
```

### V2 Bridge Module

**File:** `oric/src/typeck_v2.rs` (new)

```rust
/// Type check a module with imports using the V2 type system.
pub fn type_check_v2_with_imports(
    db: &dyn Db,
    parse_result: &ParseOutput,
    current_file: &Path,
) -> TypeCheckResultV2 {
    let interner = db.interner();

    // Use closure-based API — oric orchestrates import registration
    // without ori_types knowing about oric-specific types
    ori_types::check_module_with_imports(
        &parse_result.module,
        &parse_result.arena,
        interner,
        |checker| {
            // 1. Register prelude functions
            register_prelude_v2(db, current_file, checker);

            // 2. Register imported functions and module aliases
            register_imports_v2(db, parse_result, current_file, checker);
        },
    ).0  // Discard Pool (not needed for typed_v2 query)
}

fn register_imports_v2(
    db: &dyn Db,
    parse_result: &ParseOutput,
    current_file: &Path,
    checker: &mut ModuleChecker<'_>,
) {
    // Reuse V1 path/file resolution, but re-resolve types via V2
    for import in &parse_result.module.imports {
        let imported_file = resolve_import_path(db, &import.path, current_file);
        if let Some(imported_file) = imported_file {
            let imported_parsed = parsed(db, imported_file);

            // Register public functions from imported module
            for func in &imported_parsed.module.functions {
                if func.visibility.is_public() {
                    checker.register_imported_function(func, &imported_parsed.arena);
                }
            }

            // Handle module aliases (use std.http as http)
            if let Some(alias) = import.alias {
                checker.register_module_alias(alias, &imported_parsed.module, &imported_parsed.arena);
            }
        }
    }
}

fn register_prelude_v2(
    db: &dyn Db,
    current_file: &Path,
    checker: &mut ModuleChecker<'_>,
) {
    // Load prelude ParseOutput via Salsa query
    let prelude_file = load_prelude_file(db, current_file);
    if let Some(prelude_file) = prelude_file {
        let prelude_parsed = parsed(db, prelude_file);
        for func in &prelude_parsed.module.functions {
            if func.visibility.is_public() {
                checker.register_imported_function(func, &prelude_parsed.arena);
            }
        }
    }
}
```

### Tasks

- [x] Create `oric/src/typeck_v2.rs` with bridge functions ✅ (2026-02-05)
- [x] Add `typed_v2()` query to `oric/src/query/mod.rs` ✅ (2026-02-05)
- [x] Register `typeck_v2` module in `oric/src/lib.rs` ✅ (2026-02-05)
- [x] Implement `register_imports_v2()` using V1 path resolution + V2 type resolution ✅ (2026-02-05)
- [x] Implement `register_prelude_v2()` ✅ (2026-02-05)
- [x] Add 8 query tests to `oric/src/query/tests.rs` ✅ (2026-02-05)
- [x] Add caching/incremental/determinism tests ✅ (2026-02-05)

### Implementation Notes (2026-02-05)

**Architecture:** Created `oric/src/typeck_v2.rs` as a thin bridge module. The closure-based API
(`check_module_with_imports`) lets oric orchestrate import resolution while `ori_types` handles
type resolution internally. This is much simpler than V1's bridge which manually converts
`ParsedType → Type → ImportedFunction`.

**Prelude loading:** Reuses `prelude_candidates()` and `is_prelude_file()` from `typeck.rs`
(made `pub(crate)`). Registers each public prelude function via V2's
`register_imported_function(func, &prelude_arena)`.

**Import resolution:** Reuses `resolve_import()` from `crate::eval::module::import` for
path/file resolution. Handles module aliases, individual item imports with aliases, and
private access (`::` prefix) — mirroring V1's behavior but with simpler registration.

**Error handling:** Added `TypeCheckError::import_error()` constructor and `TypeErrorKind::ImportError`
variant to `ori_types` for import resolution failures.

**Determinism (08.5):** Fixed `ModuleChecker::finish()` and `finish_with_pool()` to sort
`functions` by `Name` before returning. This ensures stable output regardless of `FxHashMap`
iteration order. Verified with `test_typed_v2_determinism`.

**Tests (8 total):**
- `test_typed_v2_basic` — simple `@main () -> int = 42` succeeds
- `test_typed_v2_with_error` — type mismatch produces errors
- `test_typed_v2_caching` — second call uses cache (empty logs)
- `test_typed_v2_incremental` — mutation triggers recomputation
- `test_typed_v2_function_signatures` — correct parameter/return types
- `test_typed_v2_empty_module` — empty source produces no errors
- `test_typed_v2_multiple_functions` — multiple function signatures
- `test_typed_v2_determinism` — same source → identical result, sorted by name

**Verification:** 521 ori_types tests pass, 0 clippy warnings, full test suite passes.

---

## 08.3 TypedModule Output

**Goal:** Define the typed module output structure
**Status:** ✅ Complete (2026-02-04)

### Implementation

Created `ori_types/src/output/mod.rs` with:
- `TypedModuleV2`: Uses `Vec<Idx>` for expression types, `Vec<FunctionSigV2>` for functions
- `FunctionSigV2`: Complete function signature with type params, capabilities, flags
- `TypeCheckResultV2`: Wrapper with `ErrorGuaranteed` for type-level error tracking
- All types derive `Clone, Eq, PartialEq, Hash, Debug` for Salsa compatibility
- 7 unit tests covering basic operations

### Tasks

- [x] Create `ori_types/src/output/mod.rs` ✅ (2026-02-04)
- [x] Define `TypedModuleV2` with all fields ✅ (2026-02-04)
- [x] Define `FunctionSigV2` with full signature info ✅ (2026-02-04)
- [ ] Define `TypeDef` for type exports (deferred — uses TypeRegistry)
- [x] Define `TypeCheckResultV2` wrapper ✅ (2026-02-04)
- [x] Ensure all types are Salsa-compatible ✅ (2026-02-04)

---

## 08.4 Pool Sharing

**Goal:** Decide and implement pool sharing strategy

### Decision: Per-Module Pool

Each `typed_v2()` call creates a fresh `ModuleChecker` with its own `Pool`. The `TypeCheckResultV2` is the Salsa query result. The Pool is NOT part of the query result — it's discarded after type checking.

**Rationale:**

| Strategy | Pros | Cons | Verdict |
|----------|------|------|---------|
| Global Pool | Maximum dedup | Not Eq/Hash, contention, harder invalidation | ❌ |
| **Per-Module Pool** | **Simple, incremental-friendly, deterministic** | **Cross-module types need re-resolution** | ✅ **Chosen** |
| Hybrid | Best of both | Complex implementation | ❌ Premature |

### How This Maps to Current System

The current system uses `SharedTypeInterner` (`Arc<TypeInterner>`) for sharing between type checker and evaluator. The V2 equivalent:

| Scenario | V1 | V2 |
|----------|----|----|
| `typed()` query | `TypedModule` (TypeIds opaque without interner) | `TypeCheckResultV2` (Idxs opaque without Pool) |
| `evaluated()` query | Creates `SharedTypeInterner`, shares with checker + evaluator | Creates Pool, passes to checker; shares with evaluator (**Section 09**) |
| Import resolution | Extracts `ImportedFunction` with `Type` values | Re-resolves from AST using importing module's Pool |

### Cross-Module Type References

Imported functions' `Idx` values reference the imported module's Pool and cannot be used directly. Instead, the importing `ModuleChecker` re-resolves types from the imported function's AST using its own Pool. This is handled by `register_imported_function(func, foreign_arena)` (see 08.7).

### Evaluator Integration (Deferred to Section 09)

The `evaluated()` query will need the Pool to pass type information to the evaluator. Approach:

```rust
// Section 09: evaluated_v2() will call V2 type checker directly (not via typed_v2 query)
// to get both TypeCheckResultV2 and Pool:
let (result, pool) = ori_types::check_module_with_imports(...);
// Pool is wrapped in Arc for read-only sharing with evaluator
let pool = Arc::new(pool);
```

### Tasks

- [x] Decide on pool sharing strategy: **per-module** ✅
- [x] Verify per-module pool works with import re-resolution ✅ (2026-02-04, via 08.7 import tests)
- [ ] Document Pool lifecycle for Section 09 evaluator integration
- [x] Add tests for cross-module type checking ✅ (2026-02-04, 7 import integration tests)

---

## 08.5 Determinism Guarantees

**Goal:** Ensure type checking is fully deterministic
**Status:** ✅ Complete (2026-02-05)

### Analysis

The V2 system is deterministic by construction:

| Property | V2 Status | Notes |
|----------|-----------|-------|
| No random | ✅ | No randomness anywhere |
| No time | ✅ | No timestamps |
| No IO | ✅ | Pure computation |
| Stable var IDs | ✅ | Sequential counter, fresh Pool per module |
| Stable ordering | ✅ | Functions sorted by `Name` in `finish()` |

### FxHashMap → Vec Ordering: FIXED

`ModuleChecker.signatures` is a `FxHashMap<Name, FunctionSigV2>`. The `finish()` and
`finish_with_pool()` methods now sort `functions` by `Name` (which derives `Ord`) before
returning. This guarantees deterministic output regardless of hash map iteration order.

### Tasks

- [x] Audit for non-deterministic operations ✅ (2026-02-05)
- [x] Sort functions by name in `finish()` / `finish_with_pool()` ✅ (2026-02-05)
- [x] Add determinism test: `test_typed_v2_determinism` ✅ (2026-02-05)
- [ ] Add whitespace-change test: different whitespace → same result (via Salsa early cutoff on tokens)

---

## 08.6 Error Rendering Bridge

**Goal:** Make V2 type errors displayable in the CLI

### Problem

V2's `TypeCheckError` (in `ori_types/src/type_error/check_error.rs`) uses `Idx` for type references. `Idx` requires a Pool to resolve to a human-readable type name. The Pool is per-module and not part of the Salsa query result.

This is different from V1's `TypeCheckError` (in `ori_typeck/src/checker/types.rs`) which stores type names as `String`.

### Design: Rendering in oric

Error rendering lives in `oric`, not `ori_types`. This keeps ori_types independent of `ori_diagnostic`.

```rust
// In oric/src/typeck_v2.rs or a dedicated reporting module

fn render_v2_errors(
    errors: &[ori_types::TypeCheckError],
    pool: &Pool,
    interner: &StringInterner,
) -> Vec<Diagnostic> {
    errors.iter().map(|error| {
        // Use pool.format_type(idx) from pool/format.rs to resolve Idx → type name
        // Map TypeErrorKind variants to Diagnostic with spans and suggestions
        error_to_diagnostic(error, pool, interner)
    }).collect()
}
```

### Pool Access for Error Rendering

For `typed_v2()`, the Pool is discarded. To render errors, we need the Pool alongside the errors. Options:

1. **For `ori check`**: Call `check_module_with_imports()` directly (returns `(TypeCheckResultV2, Pool)`), don't go through `typed_v2()` query
2. **For `ori run`**: `evaluated_v2()` already has the Pool (Section 09)
3. **For testing**: Integration tests in ori_types already have Pool access

**Note:** Full CLI integration deferred to Section 09. During migration, V1 continues to handle error display.

### Tasks

- [ ] Implement `error_to_diagnostic()` bridge function in oric
- [ ] Use `Pool::format_type()` for Idx → type name resolution
- [ ] Wire to CLI's `ori check` command (alongside V1)
- [ ] Test error messages for common type errors

---

## 08.7 Import Registration API

**Goal:** Enable V2 type checker to accept imported functions from other modules

### Problem

The V2 `ModuleChecker` currently only processes single-module code. Real Ori code uses imports:
- `use std.math (sqrt, pow)` — individual function imports
- `use std.http as http` — module alias imports
- Prelude functions — automatically loaded

### Cross-Arena Challenge

Imported functions' `ExprId`s reference a **different** `ExprArena` than the importing module. The V1 system solves this by eagerly extracting `Type` values (self-contained). V2 must resolve types from the imported function's AST using the importing module's own Pool.

### Design: `register_imported_function(func, foreign_arena)`

Add a method to `ModuleChecker` that takes a `Function` AST node and its arena, then resolves types using the checker's own Pool.

#### Step 1: `infer_function_signature_from()` in `check/signatures.rs`

A variant of `infer_function_signature()` that takes an explicit arena parameter:

```rust
/// Infer a function signature using a foreign arena.
///
/// Used for imported functions whose ExprIds reference a different arena.
pub(super) fn infer_function_signature_from(
    &mut self,
    func: &Function,
    arena: &ExprArena,  // foreign arena, NOT self.arena
) -> FunctionSigV2 {
    // Same logic as infer_function_signature but using the provided arena
    // for get_generic_params(), get_params(), etc.
}
```

#### Step 2: Import registration on `ModuleChecker`

```rust
// In check/mod.rs
impl<'a> ModuleChecker<'a> {
    /// Register an imported function from another module's arena.
    pub fn register_imported_function(
        &mut self,
        func: &Function,
        foreign_arena: &ExprArena,
    ) {
        let sig = self.infer_function_signature_from(func, foreign_arena);
        let fn_type = sig.to_function_type(&mut self.pool);
        self.env.bind(sig.name, fn_type);
        self.signatures.insert(sig.name, sig);
    }

    /// Register a module alias (e.g., `use std.http as http`).
    pub fn register_module_alias(
        &mut self,
        alias: Name,
        module: &Module,
        foreign_arena: &ExprArena,
    ) {
        let mut funcs = Vec::new();
        for func in &module.functions {
            if func.visibility.is_public() {
                let sig = self.infer_function_signature_from(func, foreign_arena);
                funcs.push(sig);
            }
        }
        self.module_aliases.insert(alias, funcs);
        // Create module namespace type in Pool and bind in env
    }
}
```

#### Step 3: New field on `ModuleChecker`

```rust
/// Module alias imports for qualified access (e.g., `http.get(...)`).
module_aliases: FxHashMap<Name, Vec<FunctionSigV2>>,
```

#### Step 4: Closure-based API in `check/api.rs`

```rust
/// Type check a module with custom import registration.
///
/// The closure is called before signature collection (Pass 1),
/// allowing imports to be registered in the checker's environment.
pub fn check_module_with_imports<F>(
    module: &Module,
    arena: &ExprArena,
    interner: &StringInterner,
    register_imports: F,
) -> (TypeCheckResultV2, Pool)
where
    F: FnOnce(&mut ModuleChecker<'_>),
{
    let mut checker = ModuleChecker::new(arena, interner);
    register_imports(&mut checker);
    checker.check_module_impl(module);
    checker.finish_with_pool()
}
```

#### Step 5: Timing in `check_module_impl()`

Imported signatures must be in the environment BEFORE `collect_signatures()` freezes the base env. Confirm that `register_imported_function()` adds to `self.env` (not `self.base_env`), and `collect_signatures()` inherits them.

### Tasks

- [x] Add `infer_function_signature_from()` to `check/signatures.rs` ✅ (2026-02-04)
- [x] Add `register_imported_function()` to `ModuleChecker` ✅ (2026-02-04)
- [x] Add `register_module_alias()` to `ModuleChecker` ✅ (2026-02-04)
- [x] Add `import_env` + `module_aliases` fields to `ModuleChecker` ✅ (2026-02-04)
- [x] Add `check_module_with_imports()` to `check/api.rs` ✅ (2026-02-04)
- [x] Export new API from `ori_types/src/lib.rs` ✅ (2026-02-04)
- [x] Verify import timing with `collect_signatures()` / `freeze_base_env()` ✅ (2026-02-04)
- [x] Add 7 import integration tests in `check/integration_tests.rs` ✅ (2026-02-04)

### Implementation Notes (2026-02-04)

**Architecture:** Refactored `resolve_type_with_vars()` to accept an explicit `arena: &ExprArena`
parameter, enabling the same resolution logic for both local and foreign arenas. Created
`infer_function_signature_from()` and `infer_function_signature_with_arena()` as the shared
implementation that both local and import paths use.

**Import environment:** Added `import_env: TypeEnvV2` field to `ModuleChecker`. The
`collect_signatures()` pass now creates a child of `import_env` instead of a fresh environment,
so imports are visible as the grandparent scope: `import_env → base_env → child_env (per-function)`.
Local functions correctly shadow imported ones by construction.

**Closure-based API:** `check_module_with_imports(module, arena, interner, |checker| { ... })` lets
`oric` orchestrate import resolution without `ori_types` knowing about Salsa types. The closure
receives `&mut ModuleChecker` and calls `register_imported_function()` / `register_module_alias()`.

**Tests (7 total):**
- `import_simple_function` — positional call to imported function
- `import_without_registration_fails` — unregistered function → UnknownIdent
- `import_function_with_different_types` — `len(str) -> int` with correct types
- `import_return_type_mismatch_detected` — imported function return type vs consumer signature
- `import_does_not_shadow_local` — local function shadows imported one
- `import_multiple_functions` — chained calls to two imported functions
- `import_module_alias_stores_signatures` — verifies only public functions in alias

**Verification:** 521 ori_types tests pass, 7419 total tests pass, zero clippy warnings.

---

## 08.8 Implementation Sequence

### Phase A: Import Support in ori_types (08.7) ✅ Complete (2026-02-04)

1. ✅ `infer_function_signature_from()` in `check/signatures.rs`
2. ✅ `register_imported_function()` and `register_module_alias()` on `ModuleChecker`
3. ✅ `import_env` + `module_aliases` fields on `ModuleChecker`
4. ✅ `check_module_with_imports()` in `check/api.rs`
5. ✅ 7 import integration tests

### Phase B: Salsa Query Bridge in oric (08.2) ✅ Complete (2026-02-05)

1. ✅ Created `oric/src/typeck_v2.rs` with bridge functions
2. ✅ Added `typed_v2()` query to `oric/src/query/mod.rs`
3. ✅ Implemented `register_imports_v2()` and `register_prelude_v2()`
4. ✅ Added 8 query tests (basic, error, caching, incremental, signatures, empty, multiple, determinism)

### Phase C: Verification (08.4, 08.5) ✅ Complete (2026-02-05)

1. ✅ Pool sharing verified with per-module strategy (cross-module via import tests)
2. ✅ Determinism tests (`test_typed_v2_determinism` — same input → same output)
3. ✅ FxHashMap ordering fixed — `finish()` sorts functions by `Name`

### Phase D: Error Rendering (08.6)

1. `error_to_diagnostic()` bridge in oric
2. Basic error display tests

---

## 08.9 Critical Files

| File | Change | Status |
|------|--------|--------|
| `compiler/ori_types/src/check/mod.rs` | Import registration, `module_aliases`, deterministic `finish()` | ✅ |
| `compiler/ori_types/src/check/api.rs` | `check_module_with_imports()` | ✅ |
| `compiler/ori_types/src/check/signatures.rs` | `infer_function_signature_from()` with explicit arena | ✅ |
| `compiler/ori_types/src/type_error/check_error.rs` | `TypeCheckError::import_error()`, `TypeErrorKind::ImportError` | ✅ |
| `compiler/ori_types/src/lib.rs` | Export new `check_module_with_imports` | ✅ |
| `compiler/ori_types/src/check/integration_tests.rs` | 7 import tests | ✅ |
| `compiler/oric/src/typeck_v2.rs` | **New**: V2 bridge, import resolution, prelude loading | ✅ |
| `compiler/oric/src/typeck.rs` | Made `prelude_candidates`, `is_prelude_file` `pub(crate)` | ✅ |
| `compiler/oric/src/query/mod.rs` | `typed_v2()` query | ✅ |
| `compiler/oric/src/lib.rs` | Register `typeck_v2` module | ✅ |
| `compiler/oric/src/query/tests.rs` | 8 V2 query tests | ✅ |

---

## 08.10 Completion Checklist

- [x] All public types derive required traits (08.1) ✅
- [x] `TypedModuleV2` output structure complete (08.3) ✅
- [x] Import registration API in ori_types (08.7) ✅ (2026-02-04)
- [x] `typed_v2()` Salsa query in oric (08.2) ✅ (2026-02-05)
- [x] Pool sharing strategy verified with imports (08.4) ✅ (2026-02-04)
- [x] Determinism verified with tests (08.5) ✅ (2026-02-05)
- [ ] Error rendering bridge in oric (08.6)
- [x] Import integration tests passing (7 tests) ✅ (2026-02-04)
- [x] V2 query tests passing (8 tests) ✅ (2026-02-05)
- [x] `./test-all.sh` passes ✅ (2026-02-05)

**Exit Criteria:** V2 type checker is callable from oric via `typed_v2()` Salsa query, handles imports (including prelude), produces correct `TypeCheckResultV2` output, and is verified deterministic. V1 continues operating unchanged.

**Status:** Exit criteria MET except error rendering (08.6, deferred to Section 09).

---

## Verification

```bash
cargo c -p ori_types           # ori_types compiles with new API
cargo c -p oric                # oric compiles with typed_v2 query
cargo t -p ori_types           # V2 tests pass (including import tests)
cargo t -p oric                # Salsa integration tests pass
./clippy-all.sh                # No warnings
./test-all.sh                  # Full regression suite
```
