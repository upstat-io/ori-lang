---
section: "02"
title: "Crate Scaffolding & Purity Enforcement"
status: complete
goal: "Create ori_registry crate with zero dependencies, correct module structure, workspace integration, and compile/test-time purity enforcement"
depends_on:
  - "01"
sections:
  - id: "02.1"
    title: "Create crate directory & Cargo.toml"
    status: complete
  - id: "02.2"
    title: "Module structure"
    status: complete
  - id: "02.3"
    title: "Purity enforcement tests"
    status: complete
  - id: "02.4"
    title: "Workspace integration"
    status: complete
  - id: "02.5"
    title: "Crate documentation"
    status: complete
---

# Section 02: Crate Scaffolding & Purity Enforcement

**Context:** Section 01 defines the data model types (`TypeTag`, `MemoryStrategy`, `Ownership`, `OpStrategy`, `MethodDef`, `ParamDef`, `OpDefs`, `TypeDef`). This section creates the crate that houses those types and the type definitions that use them. The crate must be at the absolute bottom of the dependency graph — below even `ori_ir` — because it has zero dependencies. This is the same architectural position as a `#[no_std]` data crate: pure types, pure constants, no behavior.

**Design rationale:** The purity contract is the entire value proposition. If `ori_registry` ever gains a runtime dependency, it can no longer serve as the foundation crate. Every compiler phase would transitively depend on whatever `ori_registry` depends on, creating coupling that defeats the purpose. The enforcement tests in this section are not optional — they are the mechanism that prevents decay over time.

---

## 02.1 Create Crate Directory & Cargo.toml

**Path:** `compiler/ori_registry/`

- [x] Create directory `compiler/ori_registry/src/`
- [x] Create `compiler/ori_registry/src/defs/`
- [x] Create `compiler/ori_registry/Cargo.toml` with the exact contents below

### Cargo.toml (exact contents)

```toml
[package]
name = "ori_registry"
description = "Pure-data type behavioral registry for the Ori compiler — zero dependencies, const-constructible"
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true

# PURITY CONTRACT: This crate MUST have zero [dependencies].
# Only [dev-dependencies] are allowed (for tests).
# Enforcement: see src/tests.rs — purity_cargo_toml_has_no_dependencies
[dependencies]

[dev-dependencies]
pretty_assertions.workspace = true

[lints]
workspace = true
```

**Key decisions:**
- Empty `[dependencies]` section is written explicitly (not omitted) to make the purity contract visible in the file itself.
- The comment above `[dependencies]` names the enforcement test that will catch violations.
- Uses `workspace = true` for `version`, `edition`, `license`, `repository`, and `lints` — identical pattern to `ori_ir`, `ori_diagnostic`, and all other workspace crates.
- `pretty_assertions` in dev-dependencies matches every other compiler crate.

### Verification

- [x] `[dependencies]` section is present but empty
- [x] `[dev-dependencies]` contains only `pretty_assertions`
- [x] No `[features]` section
- [x] No `[build-dependencies]` section
- [x] `edition.workspace = true` resolves to `"2021"`

---

## 02.2 Module Structure

**Layout:**

```
compiler/ori_registry/
├── Cargo.toml
└── src/
    ├── lib.rs              # Crate root: mod declarations + pub use re-exports ONLY
    ├── tags.rs             # TypeTag, MemoryStrategy, Ownership, OpStrategy, TypeProjection,
    │                       #   TypeParamArity, MethodKind, DeiPropagation enums + ReturnTag
    ├── method.rs           # MethodDef, ParamDef structs
    ├── operator.rs         # OpDefs struct
    ├── type_def.rs         # TypeDef struct — the central aggregate
    ├── query.rs            # BUILTIN_TYPES, find_type(), find_method(), convenience iterators
    ├── tests.rs            # Purity enforcement tests (#[cfg(test)])
    └── defs/
        ├── mod.rs          # pub use of all type constants (INT, FLOAT, STR, etc.)
        ├── int.rs          # const INT: TypeDef = ...
        ├── float.rs        # const FLOAT: TypeDef = ...
        ├── str.rs          # const STR: TypeDef = ...
        ├── bool.rs         # const BOOL: TypeDef = ...
        ├── byte.rs         # const BYTE: TypeDef = ...
        └── char.rs         # const CHAR: TypeDef = ...
```

### File responsibilities

Each file has a single, clear responsibility:

| File | Contains | Approx Lines |
|------|----------|-------------|
| `lib.rs` | Module declarations + `pub use` re-exports ONLY (no function bodies) | ~25 |
| `tags.rs` | `TypeTag`, `MemoryStrategy`, `Ownership`, `OpStrategy`, `ReturnTag`, `TypeProjection`, `TypeParamArity`, `MethodKind`, `DeiPropagation` enums | ~200 |
| `method.rs` | `MethodDef`, `ParamDef` structs with derives | ~60 |
| `operator.rs` | `OpDefs` struct with derives + `UNSUPPORTED` const | ~60 |
| `type_def.rs` | `TypeDef` struct — aggregates methods, operators, memory strategy, tag | ~30 |
| `query.rs` | `BUILTIN_TYPES` array, `find_type()`, `find_method()`, convenience iterators | ~80 |
| `tests.rs` | All purity enforcement tests | ~80 |
| `defs/mod.rs` | `pub use` re-exports for all type constants | ~30 |
<!-- With const fn helper (~1 line/method): manageable. Without helper (~12 lines/method): float.rs exceeds 500-line limit. -->
| `defs/int.rs` | `pub const INT: TypeDef` with 35 methods (using `const fn` helper) and operators | ~120 |
| `defs/float.rs` | `pub const FLOAT: TypeDef` with 42 methods (using `const fn` helper) and operators | ~140 |
| `defs/str.rs` | `pub const STR: TypeDef` with all string methods (~45+ methods) and operators | ~180 |
| `defs/bool.rs` | `pub const BOOL: TypeDef` with 8 methods and operators | ~70 |
| `defs/byte.rs` | `pub const BYTE: TypeDef` with 23 methods and operators | ~100 |
| `defs/char.rs` | `pub const CHAR: TypeDef` with 16 methods and operators | ~80 |

### Skeleton file contents

**`src/lib.rs`:**
```rust
//! Ori Registry — Pure-Data Type Behavioral Specifications
//!
//! This crate is the single source of truth for every builtin type's
//! behavioral specification: methods, operators, memory strategy, and
//! ownership semantics.
//!
//! # Purity Contract
//!
//! This crate has **zero runtime dependencies**. It contains only:
//! - `const` data definitions
//! - `const fn` constructors and lookups
//! - Enums and structs that derive standard traits
//!
//! No IO, no allocation, no side effects, no `unsafe`.
//! All data is `const`-constructible and baked into `.rodata`.

mod tags;
mod method;
mod operator;
mod type_def;
mod query;
pub mod defs;

pub use self::tags::{
    DeiPropagation, MemoryStrategy, MethodKind, OpStrategy, Ownership,
    ReturnTag, TypeParamArity, TypeProjection, TypeTag,
};
pub use self::method::{MethodDef, ParamDef};
pub use self::operator::OpDefs;
pub use self::type_def::TypeDef;
// BUILTIN_TYPES is assembled in defs/mod.rs (close to type definitions)
pub use self::defs::BUILTIN_TYPES;
// Query functions live in query.rs (no function bodies in lib.rs)
pub use self::query::{
    borrowing_methods, find_method, find_type, has_method, method_names_for,
    methods_for,
};

#[cfg(test)]
mod tests;
```

**`src/tags.rs`:**
```rust
//! Core enums for type behavioral specifications.
//!
//! These enums are the vocabulary shared by all compiler phases.
//! Every enum is `Copy` — they are tags, not containers.

// TypeTag, MemoryStrategy, Ownership, OpStrategy,
// ReturnTag, TypeProjection, TypeParamArity, MethodKind, DeiPropagation
// (exact definitions from Section 01)
```

**`src/method.rs`:**
```rust
//! Method and parameter definitions.
//!
//! `MethodDef` is the central unit of method specification — it declares
//! the method's name, receiver ownership, parameters, and return type
//! without encoding any behavior.
```

**`src/operator.rs`:**
```rust
//! Operator strategy definitions.
//!
//! `OpDefs` declares how each operator category (arithmetic, comparison,
//! equality, bitwise, logical) is implemented for a given type.
```

**`src/type_def.rs`:**
```rust
//! The `TypeDef` aggregate — one struct per builtin type.
//!
//! A `TypeDef` is the complete behavioral specification for a single
//! builtin type. It aggregates the type's tag, memory strategy,
//! methods, and operator strategies.
```

**`src/defs/mod.rs`:**
```rust
//! Builtin type definitions — one `pub static` per type.
//!
//! Each submodule exports a single `pub static TypeDef`.
//! As new builtin types are added (Sections 05-07), add a new
//! file here and update BUILTIN_TYPES.

// Primitives (Section 03)
mod int;
mod float;
mod bool;
mod byte;
mod char;

// String (Section 04)
mod str;

// Compound types (Section 05) — added when Section 05 is implemented
// mod duration;
// mod size;
// mod ordering;
// mod error;
// mod channel;

// Collections & wrappers (Section 06) — added when Section 06 is implemented
// mod list;   // May be a directory (list/mod.rs + list/methods.rs) if >500 lines
// mod map;
// mod set;
// mod range;
// mod tuple;
// mod option;
// mod result;

// Iterators (Section 07) — added when Section 07 is implemented
// mod iterator;

pub use self::bool::BOOL;
pub use self::byte::BYTE;
pub use self::char::CHAR;
pub use self::float::FLOAT;
pub use self::int::INT;
pub use self::str::STR;
```

**`src/defs/int.rs` (representative pattern — all defs/ files follow this):**
```rust
//! `int` type definition.

use crate::{
    MemoryStrategy, OpDefs, OpStrategy, TypeDef, TypeParamArity, TypeTag,
};

/// Method definitions populated in Section 03.
static INT_METHODS: [crate::MethodDef; 0] = [];

pub static INT: TypeDef = TypeDef {
    tag: TypeTag::Int,
    name: "int",
    memory: MemoryStrategy::Copy,
    type_params: TypeParamArity::Fixed(0),
    methods: &INT_METHODS,
    operators: OpDefs {
        add: OpStrategy::IntInstr,
        sub: OpStrategy::IntInstr,
        mul: OpStrategy::IntInstr,
        div: OpStrategy::IntInstr,
        rem: OpStrategy::IntInstr,
        floor_div: OpStrategy::IntInstr,
        eq: OpStrategy::IntInstr,
        neq: OpStrategy::IntInstr,
        lt: OpStrategy::IntInstr,
        gt: OpStrategy::IntInstr,
        lt_eq: OpStrategy::IntInstr,
        gt_eq: OpStrategy::IntInstr,
        neg: OpStrategy::IntInstr,
        not: OpStrategy::Unsupported,
        bit_and: OpStrategy::IntInstr,
        bit_or: OpStrategy::IntInstr,
        bit_xor: OpStrategy::IntInstr,
        bit_not: OpStrategy::IntInstr,
        shl: OpStrategy::IntInstr,
        shr: OpStrategy::IntInstr,
    },
};
```

### Extensibility

When future types are added (Duration, Size, Ordering, etc.), the process is:
1. Create `src/defs/duration/mod.rs` with `pub const DURATION: TypeDef = ...` and `#[cfg(test)] mod tests;` at the bottom
2. Create `src/defs/duration/tests.rs` with unit tests (sibling test file per hygiene rules)
3. Add `mod duration;` and `pub use self::duration::DURATION;` to `src/defs/mod.rs`
4. Add `&defs::DURATION` to `BUILTIN_TYPES` in `lib.rs`

No other files change. No other crates need modification for the declaration itself.

### Checklist

- [x] Create `src/lib.rs` with module declarations and `pub use` re-exports ONLY (no function bodies)
- [x] Create `src/tags.rs` with all enums from Section 01 (TypeTag, MemoryStrategy, Ownership, OpStrategy, ReturnTag, TypeProjection, TypeParamArity, MethodKind, DeiPropagation)
- [x] Create `src/method.rs` with `MethodDef` and `ParamDef` from Section 01
- [x] Create `src/operator.rs` with `OpDefs` from Section 01
- [x] Create `src/type_def.rs` with `TypeDef` from Section 01
- [x] Create `src/query.rs` with `BUILTIN_TYPES`, `find_type()`, `find_method()`, convenience iterators (Section 08)
- [x] Create `src/tests.rs` (purity enforcement — see 02.3)
- [x] Create `src/defs/mod.rs` with re-exports
- [x] Create `src/defs/int.rs` with empty-methods placeholder `INT`
- [x] Create `src/defs/float.rs` with empty-methods placeholder `FLOAT`
- [x] Create `src/defs/str.rs` with empty-methods placeholder `STR`
- [x] Create `src/defs/bool.rs` with empty-methods placeholder `BOOL`
- [x] Create `src/defs/byte.rs` with empty-methods placeholder `BYTE`
- [x] Create `src/defs/char.rs` with empty-methods placeholder `CHAR`
- [x] Verify `cargo check -p ori_registry` passes
- [x] Verify no file exceeds 100 lines (target: each file is small and focused)

---

## 02.3 Purity Enforcement Tests

**File:** `compiler/ori_registry/src/tests.rs`

These tests enforce the purity contract structurally. They run on every `cargo test` and will catch violations immediately — not at code review, not at runtime, not in CI, but at the moment someone runs tests after adding a dependency or non-const function.

### Test 1: Cargo.toml has no dependencies

```rust
#[test]
fn purity_cargo_toml_has_no_dependencies() {
    let cargo_toml = include_str!("../Cargo.toml");

    // Find the [dependencies] section
    let deps_start = cargo_toml
        .find("[dependencies]")
        .expect("Cargo.toml must have a [dependencies] section (even if empty)");

    // Find the next section header after [dependencies]
    let after_deps = &cargo_toml[deps_start + "[dependencies]".len()..];
    let next_section = after_deps.find("\n[").map_or(after_deps.len(), |i| i);
    let deps_body = after_deps[..next_section].trim();

    // The body between [dependencies] and the next section must be empty
    // (comments are allowed — they're not dependencies)
    let non_comment_lines: Vec<&str> = deps_body
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            !trimmed.is_empty() && !trimmed.starts_with('#')
        })
        .collect();

    assert!(
        non_comment_lines.is_empty(),
        "ori_registry MUST have zero [dependencies]. Found:\n{}",
        non_comment_lines.join("\n")
    );
}
```

**Why:** This is the primary purity guard. If someone adds `rustc-hash = ...` or any other dependency, this test fails with a clear error message naming the offending line.

### Test 2: All public types are Copy or Clone

```rust
#[test]
fn purity_core_enums_are_copy() {
    // These are compile-time checks — if they compile, they pass.
    // The test function exists to document the contract.
    fn assert_copy<T: Copy>() {}
    fn assert_clone<T: Clone>() {}

    assert_copy::<TypeTag>();
    assert_copy::<MemoryStrategy>();
    assert_copy::<Ownership>();
    assert_copy::<OpStrategy>();
    assert_copy::<MethodDef>();
    assert_copy::<ParamDef>();
    assert_copy::<OpDefs>();

    // TypeDef is intentionally Clone-only (520 bytes — too large for implicit Copy)
    assert_clone::<TypeDef>();
}
```

**Why:** If a type loses its `Copy` or `Clone` derive (e.g., someone adds a non-Copy field), consuming crates break. This test catches it locally with a clear name.

### Test 3: All TypeDef entries are const-constructible

```rust
#[test]
fn purity_type_defs_are_const() {
    // If these compile, const-constructibility is proven.
    // The test verifies they exist and have the expected tag.
    use crate::defs::*;

    const _: TypeTag = INT.tag;
    const _: TypeTag = FLOAT.tag;
    const _: TypeTag = STR.tag;
    const _: TypeTag = BOOL.tag;
    const _: TypeTag = BYTE.tag;
    const _: TypeTag = CHAR.tag;

    assert_eq!(INT.tag, TypeTag::Int);
    assert_eq!(FLOAT.tag, TypeTag::Float);
    assert_eq!(STR.tag, TypeTag::Str);
    assert_eq!(BOOL.tag, TypeTag::Bool);
    assert_eq!(BYTE.tag, TypeTag::Byte);
    assert_eq!(CHAR.tag, TypeTag::Char);
}
```

**Why:** The `const _:` lines are the real enforcement — they fail at compile time if the struct or its fields aren't const-constructible. The `assert_eq!` lines verify correctness.

### Test 4: No unsafe in crate

```rust
#[test]
fn purity_no_unsafe_code() {
    // This is enforced by workspace lints: unsafe_code = "deny"
    // But we verify explicitly by scanning source files.
    let src_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/src");
    let mut found_unsafe = Vec::new();

    fn scan_dir(dir: &std::path::Path, results: &mut Vec<String>) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    scan_dir(&path, results);
                } else if path.extension().is_some_and(|e| e == "rs") {
                    if let Ok(contents) = std::fs::read_to_string(&path) {
                        for (i, line) in contents.lines().enumerate() {
                            let trimmed = line.trim();
                            if trimmed.contains("unsafe ")
                                && !trimmed.starts_with("//")
                                && !trimmed.starts_with("///")
                                && !trimmed.starts_with("#[deny(unsafe")
                                && !trimmed.starts_with("#[forbid(unsafe")
                                && !trimmed.contains("unsafe_code")
                            {
                                results.push(format!(
                                    "{}:{}: {}",
                                    path.display(),
                                    i + 1,
                                    trimmed
                                ));
                            }
                        }
                    }
                }
            }
        }
    }

    scan_dir(std::path::Path::new(src_dir), &mut found_unsafe);
    assert!(
        found_unsafe.is_empty(),
        "ori_registry MUST NOT contain unsafe code. Found:\n{}",
        found_unsafe.join("\n")
    );
}
```

**Why:** The workspace lint `unsafe_code = "deny"` already prevents this at compile time, but the test provides a second layer that also catches `#[allow(unsafe_code)]` escape hatches.

### Test 5: Public API has no &mut parameters

```rust
#[test]
fn purity_no_mutable_api() {
    // Heuristic enforcement: scan public function signatures for &mut.
    // A pure-data crate should never need mutable references in its public API.
    let src_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/src");
    let mut violations = Vec::new();

    fn scan_dir(dir: &std::path::Path, results: &mut Vec<String>) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    scan_dir(&path, results);
                } else if path.extension().is_some_and(|e| e == "rs") {
                    // Skip test files
                    if path.file_name().is_some_and(|n| n == "tests.rs") {
                        continue;
                    }
                    if let Ok(contents) = std::fs::read_to_string(&path) {
                        for (i, line) in contents.lines().enumerate() {
                            let trimmed = line.trim();
                            if trimmed.starts_with("pub ")
                                && trimmed.contains("fn ")
                                && trimmed.contains("&mut")
                            {
                                results.push(format!(
                                    "{}:{}: {}",
                                    path.display(),
                                    i + 1,
                                    trimmed
                                ));
                            }
                        }
                    }
                }
            }
        }
    }

    scan_dir(std::path::Path::new(src_dir), &mut violations);
    assert!(
        violations.is_empty(),
        "ori_registry public API MUST NOT have &mut parameters (pure data). Found:\n{}",
        violations.join("\n")
    );
}
```

**Why:** A pure-data crate has no state to mutate. If a public function takes `&mut`, someone is adding behavior to the crate. This catches it before it becomes entrenched.

### Test 6: No allocating types in public API

```rust
#[test]
fn purity_no_heap_allocation_types() {
    // Scan for String, Vec, Box, Arc, HashMap, etc. in non-test source files.
    // TypeDef uses &'static [MethodDef] (slice references to const data),
    // not Vec<MethodDef> (heap allocation).
    let src_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/src");
    let heap_types = ["String", "Vec<", "Box<", "Arc<", "Rc<", "HashMap", "BTreeMap"];
    let mut violations = Vec::new();

    fn scan_dir(
        dir: &std::path::Path,
        heap_types: &[&str],
        results: &mut Vec<String>,
    ) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    scan_dir(&path, heap_types, results);
                } else if path.extension().is_some_and(|e| e == "rs") {
                    if path.file_name().is_some_and(|n| n == "tests.rs") {
                        continue;
                    }
                    if let Ok(contents) = std::fs::read_to_string(&path) {
                        for (i, line) in contents.lines().enumerate() {
                            let trimmed = line.trim();
                            // Skip comments and doc comments
                            if trimmed.starts_with("//") {
                                continue;
                            }
                            for heap_type in heap_types {
                                if trimmed.contains(heap_type) {
                                    results.push(format!(
                                        "{}:{}: {} (contains `{}`)",
                                        path.display(),
                                        i + 1,
                                        trimmed,
                                        heap_type
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    scan_dir(std::path::Path::new(src_dir), &heap_types, &mut violations);
    assert!(
        violations.is_empty(),
        "ori_registry MUST NOT use heap-allocating types (use &'static slices). Found:\n{}",
        violations.join("\n")
    );
}
```

**Why:** The entire point of const-constructible data is that it lives in `.rodata`, not on the heap. `Vec<MethodDef>` would require runtime allocation; `&'static [MethodDef]` is a pointer to const data. This test catches the most common way someone might violate const-constructibility.

### Checklist

- [x] Create `src/tests.rs` with all 6 enforcement tests
- [x] `purity_cargo_toml_has_no_dependencies` — parses Cargo.toml, asserts empty `[dependencies]`
- [x] `purity_core_enums_are_copy` — compile-time Copy/Clone assertion
- [x] `purity_type_defs_are_const` — compile-time const-constructibility proof
- [x] `purity_no_unsafe_code` — source scan for `unsafe` keyword
- [x] `purity_no_mutable_api` — source scan for `&mut` in public function signatures
- [x] `purity_no_heap_allocation_types` — source scan for `String`, `Vec<`, `Box<`, `Arc<`, etc.
- [x] Verify `cargo test -p ori_registry` passes with all 6 tests green

---

## 02.4 Workspace Integration

### Step 1: Add to workspace members

**File:** `Cargo.toml` (workspace root)

Add `"compiler/ori_registry"` to the `[workspace.members]` list, immediately after `"compiler/ori_ir"`:

```toml
[workspace]
resolver = "2"
members = [
    "compiler/ori_ir",
    "compiler/ori_registry",        # <-- NEW: pure-data type behavioral registry
    "compiler/ori_diagnostic",
    "compiler/ori_lexer_core",
    "compiler/ori_lexer",
    "compiler/ori_types",
    "compiler/ori_parse",
    "compiler/ori_patterns",
    "compiler/ori_eval",
    "compiler/ori_fmt",
    "compiler/ori_arc",
    "compiler/ori_canon",
    "compiler/ori_compiler",
    "compiler/ori_stack",
    "compiler/oric",
]
```

**Rationale for position:** `ori_registry` is at the same dependency layer as `ori_ir` (Layer 0 — foundation, zero upward dependencies). Placing it immediately after `ori_ir` reflects this architectural relationship. Both are consumed by everything above; neither consumes anything.

### Step 2: Add to workspace dependencies

**File:** `Cargo.toml` (workspace root)

Add to the `[workspace.dependencies]` section, immediately after `ori_ir`:

```toml
# Internal crates (workspace members only; excluded crates use path deps directly)
ori_ir = { path = "compiler/ori_ir" }
ori_registry = { path = "compiler/ori_registry" }
ori_diagnostic = { path = "compiler/ori_diagnostic" }
# ... rest unchanged
```

### Step 3: Add ori_registry as dependency to consuming crates

Each consuming crate adds `ori_registry.workspace = true` to its `[dependencies]`:

**`compiler/ori_types/Cargo.toml`:**
```toml
[dependencies]
ori_ir.workspace = true
ori_registry.workspace = true       # <-- NEW
ori_diagnostic.workspace = true
# ... rest unchanged
```

**`compiler/ori_eval/Cargo.toml`:**
```toml
[dependencies]
ori_ir.workspace = true
ori_registry.workspace = true       # <-- NEW
ori_patterns.workspace = true
# ... rest unchanged
```

**`compiler/ori_arc/Cargo.toml`:**
```toml
[dependencies]
ori_ir.workspace = true
ori_registry.workspace = true       # <-- NEW
ori_types.workspace = true
# ... rest unchanged
```

**`compiler/ori_ir/Cargo.toml`:**
No change. `ori_ir` does not depend on `ori_registry` — they are peers at Layer 0.

**`compiler/oric/Cargo.toml`:**
```toml
[dependencies]
# ... existing deps
ori_ir.workspace = true
ori_registry.workspace = true       # <-- NEW (for future direct registry access)
# ... rest unchanged
```

**`compiler/ori_llvm/Cargo.toml`** (excluded from workspace, uses path deps):
```toml
[dependencies]
# Ori crates (relative paths since excluded from workspace)
ori_arc = { path = "../ori_arc", features = ["cache"] }
ori_ir = { path = "../ori_ir", features = ["cache"] }
ori_registry = { path = "../ori_registry" }    # <-- NEW (no features needed)
ori_rt = { path = "../ori_rt" }
ori_types = { path = "../ori_types", features = ["cache"] }
```

**Note on `ori_llvm`:** Since `ori_llvm` is excluded from the workspace, it uses a relative path dependency directly. `ori_registry` has no features (and should never have features), so no feature specification is needed.

### Step 4: Verify builds

- [x] `cargo check -p ori_registry` passes
- [x] `cargo test -p ori_registry` passes (6 purity tests)
- [x] `cargo check --workspace` passes (all workspace members)
- [x] `cargo b` passes (LLVM build with ori_registry)
- [x] `./test-all.sh` passes (no regressions)

### Step 5: Verify dependency graph

- [x] `ori_registry` depends on nothing (zero edges inward)
- [x] `ori_types`, `ori_eval`, `ori_arc`, `ori_llvm`, `oric` all depend on `ori_registry`
- [x] No cycles introduced — verify with `cargo tree -p ori_registry` (should show zero deps)
- [x] Verify with `cargo tree -p ori_types -i ori_registry` (should show ori_registry as leaf)

### Checklist

- [x] Add `"compiler/ori_registry"` to workspace members (after `ori_ir`)
- [x] Add `ori_registry = { path = "compiler/ori_registry" }` to workspace dependencies
- [x] Add `ori_registry.workspace = true` to `ori_types/Cargo.toml`
- [x] Add `ori_registry.workspace = true` to `ori_eval/Cargo.toml`
- [x] Add `ori_registry.workspace = true` to `ori_arc/Cargo.toml`
- [x] Add `ori_registry = { path = "../ori_registry" }` to `ori_llvm/Cargo.toml`
- [x] Add `ori_registry.workspace = true` to `oric/Cargo.toml`
- [x] `cargo check -p ori_registry` passes
- [x] `cargo test -p ori_registry` passes
- [x] `cargo check --workspace` passes
- [x] `cargo b` passes
- [x] `./test-all.sh` passes
- [x] `cargo tree -p ori_registry` shows zero dependencies

---

## 02.5 Crate Documentation

Every file gets a module-level `//!` doc comment explaining its purpose and its role in the purity contract. No external documentation files (no README.md) — this is an internal crate.

### Documentation requirements

- [x] `lib.rs`: Full crate-level doc comment explaining mission, purity contract, and usage pattern
- [x] `tags.rs`: Doc comment explaining the enum vocabulary shared by all phases
- [x] `method.rs`: Doc comment explaining `MethodDef` as the unit of method specification
- [x] `operator.rs`: Doc comment explaining `OpDefs` as operator strategy declarations
- [x] `type_def.rs`: Doc comment explaining `TypeDef` as the central aggregate
- [x] `defs/mod.rs`: Doc comment explaining the one-file-per-type organization
- [x] Each `defs/*.rs` file: Doc comment naming the type it defines
- [x] All `pub` items have `///` doc comments
- [x] No `#[allow(missing_docs)]` — every public item documented

### Documentation style

Doc comments follow the project convention:
- First line: one-sentence summary (imperative voice for functions, noun phrase for types)
- Blank line, then details if needed
- `# Examples` section for query API functions (Section 08 will add these)
- Cross-references to related types via `[`TypeTag`]` intra-doc links

---

## Exit Criteria

All of the following must be true before this section is marked complete:

1. **Crate exists:** `compiler/ori_registry/` directory with all files from 02.2
2. **Zero dependencies:** `cargo tree -p ori_registry` shows no dependencies
3. **Compiles clean:** `cargo check -p ori_registry` with zero warnings
4. **Purity tests pass:** `cargo test -p ori_registry` runs 6 tests, all green
5. **Workspace integrated:** `cargo check --workspace` passes with ori_registry in members
6. **Consuming crates wired:** `ori_types`, `ori_eval`, `ori_arc`, `ori_llvm`, `oric` all have ori_registry in `[dependencies]`
7. **LLVM build works:** `cargo b` passes with ori_registry available
8. **No regressions:** `./test-all.sh` passes
9. **Documented:** Every file and every `pub` item has doc comments
10. **Types from Section 01 compile:** All enums and structs from Section 01 are defined in the appropriate module files and are const-constructible
