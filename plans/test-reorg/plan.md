# Test Reorganization Plan

## Overview

Reorganize Ori compiler tests into a dual-organization structure:
- **Feature tests** (`tests/spec/`) - User-facing behavior validation
- **Phase tests** (`tests/phases/`) - Compiler component validation

## Current State (Verified)

| Metric | Value |
|--------|-------|
| Modules > 200 lines | **33** |
| Total violation lines | **~14,200** |

### Violations by Crate

| Crate | Violations | Worst Offender |
|-------|------------|----------------|
| ori_llvm | 11 | debug.rs (1,100), linker/mod.rs (1,072) |
| ori_patterns | 4 | lib.rs (709), scalar_int.rs (787) |
| oric | 4 | commands/build.rs (400) |
| ori_parse | 3 | lib.rs (494) |
| ori_typeck | 2 | registry/mod.rs (481) |
| ori_types | 2 | lib.rs (453), type_interner.rs (360) |
| ori_eval | 2 | scope_guard.rs (246) |
| ori_rt | 1 | lib.rs (686) |
| ori_lexer | 1 | lib.rs (462) |
| ori_ir | 1 | visitor.rs (390) |
| ori_fmt | 1 | width/mod.rs (405) |
| ori_diagnostic | 1 | queue.rs (327) |

### Complete Violation List (Sorted by Size)

| Lines | Crate | File |
|-------|-------|------|
| 1,100 | ori_llvm | aot/debug.rs |
| 1,072 | ori_llvm | aot/linker/mod.rs |
| 787 | ori_patterns | value/scalar_int.rs |
| 709 | ori_patterns | lib.rs |
| 686 | ori_rt | lib.rs |
| 637 | ori_llvm | aot/passes.rs |
| 636 | ori_llvm | aot/wasm.rs |
| 515 | ori_patterns | errors.rs |
| 494 | ori_parse | lib.rs |
| 481 | ori_typeck | registry/mod.rs |
| 467 | ori_llvm | aot/object.rs |
| 462 | ori_lexer | lib.rs |
| 453 | ori_types | lib.rs |
| 405 | ori_fmt | width/mod.rs |
| 400 | oric | commands/build.rs |
| 396 | ori_llvm | aot/target.rs |
| 390 | ori_ir | visitor.rs |
| 360 | ori_types | type_interner.rs |
| 359 | ori_parse | grammar/ty.rs |
| 327 | ori_diagnostic | queue.rs |
| 283 | ori_llvm | aot/mangle.rs |
| 260 | ori_llvm | aot/multi_file.rs |
| 246 | ori_eval | interpreter/scope_guard.rs |
| 244 | ori_typeck | infer/mod.rs |
| 239 | ori_llvm | aot/linker/wasm.rs |
| 236 | ori_eval | module_registration.rs |
| 221 | ori_llvm | aot/incremental/hash.rs |
| 211 | oric | query/mod.rs |
| 207 | ori_patterns | recurse.rs |
| 206 | oric | suggest.rs |
| 206 | oric | edit/tracker.rs |
| 205 | ori_llvm | aot/incremental/deps.rs |
| 204 | ori_parse | grammar/attr.rs |

---

## Target Structure

```
ori_lang/
├── tests/
│   ├── spec/                    # KEEP AS-IS
│   │   ├── types/
│   │   ├── expressions/
│   │   ├── functions/
│   │   └── ...
│   │
│   ├── phases/                  # NEW
│   │   ├── parse/
│   │   │   ├── mod.rs
│   │   │   ├── expressions.rs
│   │   │   ├── items.rs
│   │   │   ├── patterns.rs
│   │   │   └── error_recovery.rs
│   │   │
│   │   ├── typeck/
│   │   │   ├── mod.rs
│   │   │   ├── inference.rs
│   │   │   ├── unification.rs
│   │   │   ├── traits.rs
│   │   │   └── generics.rs
│   │   │
│   │   ├── eval/
│   │   │   ├── mod.rs
│   │   │   ├── expressions.rs
│   │   │   ├── patterns.rs
│   │   │   ├── methods.rs
│   │   │   └── values.rs
│   │   │
│   │   └── codegen/
│   │       ├── mod.rs
│   │       ├── llvm_ir.rs
│   │       ├── debug_info.rs
│   │       ├── linking.rs
│   │       ├── optimization.rs
│   │       └── targets.rs
│   │
│   └── compile-fail/            # KEEP AS-IS
│
└── compiler/
    └── ori_*/src/
        └── *.rs                 # Only inline tests <200 lines
```

---

## Phase Definitions

### Phase 1: Parse (`tests/phases/parse/`)
Tests the `ori_lexer` and `ori_parse` crates.

**What belongs here:**
- Tokenization edge cases
- Parser grammar coverage
- Error recovery tests
- AST structure validation
- Span correctness

**Test pattern:**
```rust
#[test]
fn parse_generic_function() {
    let source = "@foo<T>(x: T) -> T = x";
    let result = parse(source);
    assert!(result.is_ok());
    // Validate AST structure
}
```

### Phase 2: Typeck (`tests/phases/typeck/`)
Tests the `ori_typeck` and `ori_types` crates.

**What belongs here:**
- Type inference scenarios
- Unification edge cases
- Trait resolution
- Generic instantiation
- Error message quality

**Test pattern:**
```rust
#[test]
fn infer_generic_identity() {
    let source = "@id<T>(x: T) -> T = x\n@main () -> int = id(42)";
    let typed = typecheck(source);
    assert_eq!(typed.return_type("main"), Type::Int);
}
```

### Phase 3: Eval (`tests/phases/eval/`)
Tests the `ori_eval` and `ori_patterns` crates.

**What belongs here:**
- Expression evaluation
- Pattern matching
- Method dispatch
- Value representation
- Runtime behavior (interpreter)

**Test pattern:**
```rust
#[test]
fn eval_pattern_matching() {
    let source = "match(Some(42), Some(x) -> x, None -> 0)";
    let result = eval(source);
    assert_eq!(result, Value::Int(42));
}
```

### Phase 4: Codegen (`tests/phases/codegen/`)
Tests the `ori_llvm` crate.

**What belongs here:**
- LLVM IR generation
- Debug info (DWARF)
- Linking behavior
- Optimization passes
- Target-specific code
- ABI compliance

**Test pattern:**
```rust
#[test]
fn codegen_debug_info_function() {
    let source = "@add(a: int, b: int) -> int = a + b";
    let ir = compile_to_ir(source);
    assert!(ir.contains("!DISubprogram"));
}
```

---

## Migration Plan

### Stage 1: Infrastructure

1. Create directory structure:
   ```bash
   mkdir -p tests/phases/{parse,typeck,eval,codegen}
   ```

2. Create shared test utilities:
   ```
   tests/phases/common/
   ├── mod.rs           # Re-exports
   ├── parse.rs         # parse() helper
   ├── typecheck.rs     # typecheck() helper
   ├── eval.rs          # eval() helper
   └── codegen.rs       # compile_to_ir() helper
   ```

3. Update `Cargo.toml` for test dependencies

### Stage 2: Extract Extreme Violations (>1000 lines)

Extract the two 1000+ line modules:

#### 2a. `ori_llvm/src/aot/debug.rs` (1,099 lines, 66 tests)

Split into:
- `tests/phases/codegen/debug_basic_types.rs`
- `tests/phases/codegen/debug_composite_types.rs`
- `tests/phases/codegen/debug_config.rs`
- `tests/phases/codegen/debug_levels.rs`

Keep inline: Basic unit tests for `DebugInfoBuilder` methods

#### 2b. `ori_llvm/src/aot/linker/mod.rs` (1,071 lines, 73 tests)

Split into:
- `tests/phases/codegen/linker_gcc.rs`
- `tests/phases/codegen/linker_msvc.rs`
- `tests/phases/codegen/linker_wasm.rs`
- `tests/phases/codegen/linker_discovery.rs`

Keep inline: Unit tests for `LinkerDriver` trait methods

### Stage 3: Extract High Violations (500-1000 lines)

Extract 500-800 line modules (7 files):

| Source | Target | Lines |
|--------|--------|-------|
| `ori_patterns/src/value/scalar_int.rs` | `tests/phases/eval/scalar_int.rs` | 787 |
| `ori_patterns/src/lib.rs` | `tests/phases/eval/patterns.rs` | 709 |
| `ori_rt/src/lib.rs` | `tests/phases/codegen/runtime_lib.rs` | 686 |
| `ori_llvm/src/aot/passes.rs` | `tests/phases/codegen/optimization.rs` | 637 |
| `ori_llvm/src/aot/wasm.rs` | `tests/phases/codegen/wasm.rs` | 636 |
| `ori_patterns/src/errors.rs` | `tests/phases/eval/errors.rs` | 515 |
| `ori_parse/src/lib.rs` | `tests/phases/parse/parser.rs` | 494 |

### Stage 4: Extract Medium Violations (200-500 lines)

Extract 200-500 line modules (24 files):

| Source | Target | Lines |
|--------|--------|-------|
| `ori_typeck/src/registry/mod.rs` | `tests/phases/typeck/registry.rs` | 481 |
| `ori_llvm/src/aot/object.rs` | `tests/phases/codegen/object_emit.rs` | 467 |
| `ori_lexer/src/lib.rs` | `tests/phases/parse/lexer.rs` | 462 |
| `ori_types/src/lib.rs` | `tests/phases/typeck/types.rs` | 453 |
| `ori_fmt/src/width/mod.rs` | `tests/phases/common/fmt_width.rs` | 405 |
| `oric/src/commands/build.rs` | `tests/phases/codegen/build_command.rs` | 400 |
| `ori_llvm/src/aot/target.rs` | `tests/phases/codegen/targets.rs` | 396 |
| `ori_ir/src/visitor.rs` | `tests/phases/parse/visitor.rs` | 390 |
| `ori_types/src/type_interner.rs` | `tests/phases/typeck/type_interner.rs` | 360 |
| `ori_parse/src/grammar/ty.rs` | `tests/phases/parse/type_grammar.rs` | 359 |
| `ori_diagnostic/src/queue.rs` | `tests/phases/common/diagnostics.rs` | 327 |
| `ori_llvm/src/aot/mangle.rs` | `tests/phases/codegen/mangling.rs` | 283 |
| `ori_llvm/src/aot/multi_file.rs` | `tests/phases/codegen/multi_file.rs` | 260 |
| `ori_eval/src/interpreter/scope_guard.rs` | `tests/phases/eval/scope_guard.rs` | 246 |
| `ori_typeck/src/infer/mod.rs` | `tests/phases/typeck/inference.rs` | 244 |
| `ori_llvm/src/aot/linker/wasm.rs` | `tests/phases/codegen/linker_wasm_config.rs` | 239 |
| `ori_eval/src/module_registration.rs` | `tests/phases/eval/module_registration.rs` | 236 |
| `ori_llvm/src/aot/incremental/hash.rs` | `tests/phases/codegen/incremental_hash.rs` | 221 |
| `oric/src/query/mod.rs` | `tests/phases/common/query.rs` | 211 |
| `ori_patterns/src/recurse.rs` | `tests/phases/eval/recurse.rs` | 207 |
| `oric/src/suggest.rs` | `tests/phases/typeck/suggest.rs` | 206 |
| `oric/src/edit/tracker.rs` | `tests/phases/common/edit_tracker.rs` | 206 |
| `ori_llvm/src/aot/incremental/deps.rs` | `tests/phases/codegen/incremental_deps.rs` | 205 |
| `ori_parse/src/grammar/attr.rs` | `tests/phases/parse/attributes.rs` | 204 |

### Stage 5: Cleanup

1. Remove empty `mod tests` blocks from source files
2. Update CI to run phase tests
3. Update documentation:
   - **CLAUDE.md** - Update testing section with new locations
   - **.claude/rules/compiler.md** - Update Testing section
   - **.claude/rules/tests.md** - Update Test Directories section
   - **.claude/rules/aot.md** - Update any test references
   - **.claude/rules/*.md** - Check all rules files for test path references

---

## Test Helper Design

### `tests/phases/common/mod.rs`

```rust
//! Shared test utilities for phase tests.

mod parse;
mod typecheck;
mod eval;
mod codegen;

pub use parse::*;
pub use typecheck::*;
pub use eval::*;
pub use codegen::*;

/// Standard test source with prelude.
pub fn with_prelude(source: &str) -> String {
    format!("{}\n{}", PRELUDE, source)
}
```

### `tests/phases/common/parse.rs`

```rust
use ori_parse::{parse_module, ParseOutput};
use ori_ir::StringInterner;

/// Parse source code, returning the parse result.
pub fn parse(source: &str) -> Result<ParseOutput, Vec<ParseError>> {
    let interner = StringInterner::new();
    parse_module(source, &interner)
}

/// Parse and assert success.
pub fn parse_ok(source: &str) -> ParseOutput {
    parse(source).expect("parse failed")
}

/// Parse and assert failure with specific error.
pub fn parse_err(source: &str, expected: &str) {
    let err = parse(source).expect_err("expected parse error");
    assert!(err.iter().any(|e| e.message.contains(expected)));
}
```

### `tests/phases/common/typecheck.rs`

```rust
use ori_typeck::{TypeChecker, TypedModule};

/// Type check source code.
pub fn typecheck(source: &str) -> Result<TypedModule, Vec<TypeCheckError>> {
    let parsed = parse_ok(source);
    TypeChecker::check(&parsed)
}

/// Type check and assert success.
pub fn typecheck_ok(source: &str) -> TypedModule {
    typecheck(source).expect("typecheck failed")
}

/// Assert a function has a specific return type.
pub fn assert_return_type(source: &str, func: &str, expected: &str) {
    let typed = typecheck_ok(source);
    let actual = typed.function_type(func).return_type;
    assert_eq!(actual.to_string(), expected);
}
```

---

## File Movement Checklist

### ori_llvm (11 violations)

- [ ] `src/aot/debug.rs` (1,100) → `tests/phases/codegen/debug_*.rs`
- [ ] `src/aot/linker/mod.rs` (1,072) → `tests/phases/codegen/linker_*.rs`
- [ ] `src/aot/passes.rs` (637) → `tests/phases/codegen/optimization.rs`
- [ ] `src/aot/wasm.rs` (636) → `tests/phases/codegen/wasm.rs`
- [ ] `src/aot/object.rs` (467) → `tests/phases/codegen/object_emit.rs`
- [ ] `src/aot/target.rs` (396) → `tests/phases/codegen/targets.rs`
- [ ] `src/aot/mangle.rs` (283) → `tests/phases/codegen/mangling.rs`
- [ ] `src/aot/multi_file.rs` (260) → `tests/phases/codegen/multi_file.rs`
- [ ] `src/aot/linker/wasm.rs` (239) → `tests/phases/codegen/linker_wasm_config.rs`
- [ ] `src/aot/incremental/hash.rs` (221) → `tests/phases/codegen/incremental_hash.rs`
- [ ] `src/aot/incremental/deps.rs` (205) → `tests/phases/codegen/incremental_deps.rs`

### ori_patterns (4 violations)

- [ ] `src/value/scalar_int.rs` (787) → `tests/phases/eval/scalar_int.rs`
- [ ] `src/lib.rs` (709) → `tests/phases/eval/patterns.rs`
- [ ] `src/errors.rs` (515) → `tests/phases/eval/errors.rs`
- [ ] `src/recurse.rs` (207) → `tests/phases/eval/recurse.rs`

### oric (4 violations)

- [ ] `src/commands/build.rs` (400) → `tests/phases/codegen/build_command.rs`
- [ ] `src/query/mod.rs` (211) → `tests/phases/common/query.rs`
- [ ] `src/suggest.rs` (206) → `tests/phases/typeck/suggest.rs`
- [ ] `src/edit/tracker.rs` (206) → `tests/phases/common/edit_tracker.rs`

### ori_parse (3 violations)

- [ ] `src/lib.rs` (494) → `tests/phases/parse/parser.rs`
- [ ] `src/grammar/ty.rs` (359) → `tests/phases/parse/type_grammar.rs`
- [ ] `src/grammar/attr.rs` (204) → `tests/phases/parse/attributes.rs`

### ori_typeck (2 violations)

- [ ] `src/registry/mod.rs` (481) → `tests/phases/typeck/registry.rs`
- [ ] `src/infer/mod.rs` (244) → `tests/phases/typeck/inference.rs`

### ori_types (2 violations)

- [ ] `src/lib.rs` (453) → `tests/phases/typeck/types.rs`
- [ ] `src/type_interner.rs` (360) → `tests/phases/typeck/type_interner.rs`

### ori_eval (2 violations)

- [ ] `src/interpreter/scope_guard.rs` (246) → `tests/phases/eval/scope_guard.rs`
- [ ] `src/module_registration.rs` (236) → `tests/phases/eval/module_registration.rs`

### ori_rt (1 violation)

- [ ] `src/lib.rs` (686) → `tests/phases/codegen/runtime_lib.rs`

### ori_lexer (1 violation)

- [ ] `src/lib.rs` (462) → `tests/phases/parse/lexer.rs`

### ori_ir (1 violation)

- [ ] `src/visitor.rs` (390) → `tests/phases/parse/visitor.rs`

### ori_fmt (1 violation)

- [ ] `src/width/mod.rs` (405) → `tests/phases/common/fmt_width.rs`

### ori_diagnostic (1 violation)

- [ ] `src/queue.rs` (327) → `tests/phases/common/diagnostics.rs`

---

## CI Integration

### Update `.github/workflows/test.yml`

```yaml
- name: Run phase tests
  run: |
    cargo test --test phases

- name: Run spec tests (interpreter)
  run: |
    cargo st tests/spec/

- name: Run spec tests (LLVM)
  run: |
    ./target/release/ori test --backend=llvm tests/spec/
```

### Test Naming Convention

```
tests/phases/{phase}/{category}.rs

# Examples:
tests/phases/parse/expressions.rs
tests/phases/typeck/inference.rs
tests/phases/eval/pattern_matching.rs
tests/phases/codegen/debug_info.rs
```

---

## Success Criteria

1. **All inline test modules < 200 lines**
2. **Phase tests clearly organized by compiler stage**
3. **Spec tests unchanged** (feature-organized, dual-backend)
4. **CI runs both phase and spec tests**
5. **Test helpers reduce boilerplate**
6. **Clear documentation on where to add new tests**

---

## Rollback Plan

If issues arise:
1. Phase tests are additive - old inline tests still work
2. Move tests back by copying from `tests/phases/` to inline
3. Delete `tests/phases/` directory

---

## Stage Summary

| Stage | Files | Deliverable |
|-------|-------|-------------|
| Infrastructure | 0 | Directory structure, helpers |
| Extreme violations (>1000) | 2 | debug.rs, linker/mod.rs |
| High violations (500-1000) | 7 | scalar_int.rs, lib.rs, rt, passes, wasm, errors, parse |
| Medium violations (200-500) | 24 | All remaining violations |
| Cleanup | — | Documentation, CI |
| **Total** | **33** | Full compliance |

---

## Documentation Updates Required

### CLAUDE.md

Update the Commands section:
```markdown
**Tests**: `cargo t` (Rust), `cargo st` (Ori spec), `cargo test --test phases` (phase tests)
```

Update the Testing section to mention:
- Phase tests in `tests/phases/`
- When to add tests to spec vs phases

### .claude/rules/compiler.md

Update Testing section:
```markdown
## Testing

- **Inline** (`#[cfg(test)]`): <200 lines, unit tests only
- **Phase tests** (`tests/phases/`): >200 lines, organized by compiler stage
- **Spec tests** (`tests/spec/`): Language conformance, dual-backend
- **TDD for bugs**: failing test first
```

### .claude/rules/tests.md

Update Test Directories section:
```markdown
## Test Directories

- `tests/spec/` — Language feature conformance (dual-backend)
- `tests/phases/parse/` — Parser and lexer tests
- `tests/phases/typeck/` — Type checker tests
- `tests/phases/eval/` — Interpreter tests
- `tests/phases/codegen/` — LLVM backend tests
- `tests/phases/common/` — Shared test utilities
- `tests/compile-fail/` — Expected compilation failures
```

### .claude/rules/aot.md

Add section:
```markdown
## Testing

AOT-specific tests live in `tests/phases/codegen/`:
- `debug_*.rs` - DWARF/CodeView tests
- `linker_*.rs` - Linker driver tests
- `optimization.rs` - Pass manager tests
- `targets.rs` - Cross-compilation tests
```

## Questions to Resolve

1. Should `tests/phases/` be a workspace member or use path dependencies?
2. How to handle tests that span multiple phases?
3. Should phase tests also run through both backends?
4. Naming: `codegen/` vs `llvm/` for the codegen phase?
