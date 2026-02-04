---
section: "09"
title: Migration
status: not-started
goal: Update all dependent crates to use new type system
sections:
  - id: "09.1"
    title: Delete Old Code
    status: not-started
  - id: "09.2"
    title: Update ori_eval
    status: not-started
  - id: "09.3"
    title: Update ori_patterns
    status: not-started
  - id: "09.4"
    title: Update ori_llvm
    status: not-started
  - id: "09.5"
    title: Update oric
    status: not-started
  - id: "09.6"
    title: Test Migration
    status: not-started
---

# Section 09: Migration

**Status:** Not Started
**Goal:** Complete migration with no remnants of old system
**Source:** Inventory from analysis phase

---

## CRITICAL REMINDER

**This is a complete replacement. No remnants. No backwards compatibility.**

- Delete ALL old files, not just some
- Update ALL imports, not just some
- Fix ALL compilation errors
- Pass ALL existing tests

---

## 09.1 Delete Old Code

**Goal:** Remove all old type system code

### Files to DELETE in ori_types/src/

```bash
# Execute in compiler/ori_types/src/
rm lib.rs           # 47 lines
rm core.rs          # 420 lines
rm data.rs          # 210 lines
rm context.rs       # 754 lines
rm type_interner.rs # 528 lines
rm env.rs           # 329 lines
rm traverse.rs      # 706 lines
rm error.rs         # 111 lines
```

### Files to DELETE in ori_typeck/src/

```bash
# Execute in compiler/ori_typeck/src/
rm lib.rs
rm shared.rs
rm suggest.rs
rm -rf checker/     # 22 files
rm -rf infer/       # 48 files
rm -rf registry/    # 11 files
rm -rf derives/
rm -rf operators/
```

### Tasks

- [ ] Backup current tests (copy test files to temp location)
- [ ] Delete all ori_types/src/ files
- [ ] Delete all ori_typeck/src/ files
- [ ] Verify directories are empty
- [ ] Commit deletion separately for git history

---

## 09.2 Update ori_eval

**Goal:** Update interpreter to use new type system

### Current Imports

```rust
// In ori_eval/src/interpreter/
use ori_types::{SharedTypeInterner, Type, TypeData};
```

### New Imports

```rust
use ori_types::{Pool, Idx, Tag};
```

### Changes Required

| Old Usage | New Usage |
|-----------|-----------|
| `Type::Int` | `Idx::INT` |
| `Type::List(Box::new(elem))` | `pool.list(elem)` |
| `match type { Type::Int => ... }` | `match pool.tag(idx) { Tag::Int => ... }` |
| `type.clone()` | Just use `idx` (Copy) |
| `TypeInterner::intern(type)` | `pool.intern(tag, data)` |

### Files to Update

```
compiler/ori_eval/src/
├── interpreter/
│   ├── builder.rs      # TypeInterner -> Pool
│   ├── mod.rs          # Type usage
│   └── value.rs        # Type in Value
├── lib.rs
└── ...
```

### Tasks

- [ ] Update Cargo.toml dependencies
- [ ] Update all imports
- [ ] Replace `Type` with `Idx`
- [ ] Replace `TypeInterner` with `Pool`
- [ ] Update pattern matching on types
- [ ] Fix all compilation errors
- [ ] Run ori_eval tests

---

## 09.3 Update ori_patterns

**Goal:** Update pattern system to use new type system

### Current Imports

```rust
use ori_types::{SharedTypeInterner, Type};
```

### New Imports

```rust
use ori_types::{Pool, Idx, Tag};
```

### Files to Update

```
compiler/ori_patterns/src/
├── registry.rs         # Pattern type registration
├── builtins/
│   ├── mod.rs
│   ├── string.rs       # String pattern types
│   ├── numeric.rs      # Numeric pattern types
│   └── ...
└── lib.rs
```

### Tasks

- [ ] Update Cargo.toml dependencies
- [ ] Update all imports
- [ ] Replace `Type` with `Idx` in pattern definitions
- [ ] Update pattern type checking integration
- [ ] Fix all compilation errors
- [ ] Run ori_patterns tests

---

## 09.4 Update ori_llvm

**Goal:** Update LLVM backend to use new type system

### Current Imports

```rust
use ori_types::{SharedTypeInterner, Type, TypeData};
```

### New Imports

```rust
use ori_types::{Pool, Idx, Tag};
```

### Changes Required

| Old Usage | New Usage |
|-----------|-----------|
| `match type_data { TypeData::Int => ... }` | `match pool.tag(idx) { Tag::Int => ... }` |
| `type.as_function()` | `if pool.tag(idx) == Tag::Function { ... }` |
| `interner.lookup(type_id)` | `pool.tag(idx), pool.data(idx)` |

### Files to Update

```
compiler/ori_llvm/src/
├── context.rs          # Type context
├── evaluator.rs        # Type-based code generation
├── module.rs           # Module types
├── codegen/
│   ├── types.rs        # LLVM type mapping
│   └── ...
└── lib.rs
```

### Type Mapping Updates

```rust
// Old
fn llvm_type_for(&self, ty: &Type) -> LLVMTypeRef {
    match ty {
        Type::Int => self.context.i64_type(),
        Type::Float => self.context.f64_type(),
        Type::Bool => self.context.i1_type(),
        Type::List(elem) => self.list_type(elem),
        // ...
    }
}

// New
fn llvm_type_for(&self, pool: &Pool, idx: Idx) -> LLVMTypeRef {
    match pool.tag(idx) {
        Tag::Int => self.context.i64_type(),
        Tag::Float => self.context.f64_type(),
        Tag::Bool => self.context.i1_type(),
        Tag::List => {
            let elem = Idx(pool.data(idx));
            self.list_type(pool, elem)
        }
        // ...
    }
}
```

### Tasks

- [ ] Update Cargo.toml dependencies
- [ ] Update all imports
- [ ] Rewrite type mapping functions
- [ ] Update code generation for each type
- [ ] Fix all compilation errors
- [ ] Run ori_llvm tests
- [ ] Run llvm-test.sh

---

## 09.5 Update oric

**Goal:** Update CLI and Salsa queries

### Current Imports

```rust
use ori_types::{SharedTypeInterner, Type, TypeEnv, InferenceContext};
use ori_typeck::{type_check, TypeChecker, TypedModule};
```

### New Imports

```rust
use ori_types::{Pool, Idx, Tag, TypeFlags};
use ori_typeck::{type_check_module, TypedModule, InferEngine};
```

### Files to Update

```
compiler/oric/src/
├── context.rs          # Salsa context, Pool integration
├── types.rs            # Type-related utilities
├── typeck.rs           # Type checking integration
├── query/
│   ├── mod.rs          # Salsa query definitions
│   └── ...
├── eval/
│   └── ...             # Evaluator integration
├── test/
│   └── runner.rs       # Test type checking
└── lib.rs

compiler/oric/tests/
└── phases/             # Phase-specific tests
```

### Salsa Query Updates

```rust
// Old
#[salsa::tracked]
fn type_check_module(db: &dyn Db, module: ModuleId) -> TypeCheckResult {
    let interner = db.type_interner();
    let checker = TypeChecker::new(&interner);
    // ...
}

// New
#[salsa::tracked]
fn type_check_module(db: &dyn Db, module: ModuleId) -> TypeCheckResult {
    let pool = db.type_pool();
    let engine = InferEngine::new(&mut pool.write(), &arena);
    // ...
}
```

### Tasks

- [ ] Update Cargo.toml dependencies
- [ ] Update all imports
- [ ] Rewrite Salsa queries for new types
- [ ] Update context.rs Pool integration
- [ ] Update test runner
- [ ] Fix all compilation errors
- [ ] Run all oric tests
- [ ] Run ./test-all.sh

---

## 09.6 Test Migration

**Goal:** Ensure all tests pass with new system

### Test Categories

1. **Unit Tests**
   - [ ] ori_types unit tests
   - [ ] ori_typeck unit tests
   - [ ] ori_eval unit tests
   - [ ] ori_patterns unit tests
   - [ ] ori_llvm unit tests

2. **Integration Tests**
   - [ ] oric integration tests
   - [ ] Phase tests in oric/tests/phases/

3. **Spec Tests**
   - [ ] tests/spec/ conformance tests

4. **LLVM Tests**
   - [ ] ./llvm-test.sh

5. **Full Test Suite**
   - [ ] ./test-all.sh

### Migration Verification

```bash
# Step 1: Run cargo check
cargo check --workspace

# Step 2: Run clippy
./clippy-all.sh

# Step 3: Run unit tests
cargo test --workspace

# Step 4: Run spec tests
cargo st

# Step 5: Run LLVM tests
./llvm-test.sh

# Step 6: Run full suite
./test-all.sh
```

### Tasks

- [ ] Fix any failing unit tests
- [ ] Fix any failing integration tests
- [ ] Fix any failing spec tests
- [ ] Fix any failing LLVM tests
- [ ] Verify ./test-all.sh passes
- [ ] Run ./clippy-all.sh with no warnings

---

## 09.7 Post-Migration Cleanup

**Goal:** Clean up any remaining issues

### Cleanup Tasks

- [ ] Remove any TODO comments related to migration
- [ ] Remove any dead code
- [ ] Run `cargo fmt` on all changed files
- [ ] Update CLAUDE.md if type system docs changed
- [ ] Update any affected documentation
- [ ] Verify no references to old types remain

### Verification

```bash
# Search for any remaining old type references
grep -r "Type::" compiler/
grep -r "TypeData" compiler/
grep -r "TypeInterner" compiler/
grep -r "InferenceContext" compiler/  # Should only be in new code
```

### Tasks

- [ ] Search for stray old type references
- [ ] Remove all found references
- [ ] Final ./test-all.sh run
- [ ] Create summary commit

---

## 09.8 Completion Checklist

- [ ] All old ori_types code deleted
- [ ] All old ori_typeck code deleted
- [ ] ori_eval updated and tests passing
- [ ] ori_patterns updated and tests passing
- [ ] ori_llvm updated and tests passing
- [ ] oric updated and tests passing
- [ ] All spec tests passing
- [ ] ./test-all.sh passes
- [ ] ./clippy-all.sh passes with no warnings
- [ ] No remnants of old type system anywhere
- [ ] Git history clean with meaningful commits

**Exit Criteria:** The codebase compiles, all tests pass, and `grep` finds zero references to old type system types (Type::, TypeData, TypeInterner, etc.) outside of comments or documentation.
