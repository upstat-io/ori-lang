# Proposal: Multi-File AOT Compilation

**Status:** Approved
**Author:** Eric (with Claude)
**Created:** 2026-02-01
**Approved:** 2026-02-01
**Affects:** `compiler/ori_llvm/`, `compiler/oric/src/commands/compile_common.rs`

## Summary

Enable AOT compilation of Ori programs that use `use` statements to import functions from other files. Currently, `ori build` silently produces broken binaries when the source file imports from other modules.

## Motivation

### The Problem

When compiling a multi-file Ori program:

```ori
// helper.ori
pub @my_assert (b: bool) -> void = if b then () else panic(msg: "fail")

// main.ori
use "./helper" { my_assert }
@main () -> void = my_assert(b: true)
```

Running `ori build main.ori -o main && ./main` produces exit code 48 instead of 0.

### Root Cause

The current `compile_to_llvm` function in `compile_common.rs`:

```rust
// Only compiles functions from the MAIN file
for func in &module.functions {
    compiler.compile_function(func, arena, expr_types);
}
```

This has two fatal flaws:

1. **Imported functions are never compiled** — `my_assert` from `helper.ori` is never added to the LLVM module
2. **Calls to missing functions silently fail** — When codegen can't find `my_assert`, it returns `None` and the call is omitted, producing `ret void`

### Impact

This blocks:
- **Real-world Ori projects** — Any project with imports fails silently
- **AOT Test Backend proposal** — Can't run spec tests that use helper modules
- **stdlib usage** — `use std.math { sqrt }` would fail in AOT

### Current Workaround

None. Single-file programs work, but any imports produce broken binaries.

## Design

### Approach: Separate Object Files Per Module

Each Ori module compiles to its own object file. All object files are then linked together into the final executable. This matches the approved AOT proposal and enables parallel compilation.

```
┌─────────────────────────────────────────────────────────────────────┐
│                   Multi-File AOT Compilation                         │
│                                                                      │
│  main.ori ──use "./helper"──► helper.ori ──use "./utils"──► utils.ori│
│                                                                      │
│  Compilation Order (topological):                                    │
│    1. utils.ori   → build/obj/utils.o   (no imports)                 │
│    2. helper.ori  → build/obj/helper.o  (imports utils)              │
│    3. main.ori    → build/obj/main.o    (imports helper)             │
│                                                                      │
│  Link: ld -o main build/obj/*.o libori_rt.a                          │
└─────────────────────────────────────────────────────────────────────┘
```

### Key Design Decisions

#### 1. Separate Object Files Per Module

Each `.ori` file produces one `.o` file. This:
- Enables parallel compilation of independent modules
- Integrates with existing incremental compilation infrastructure (21B.6)
- Matches the approved AOT proposal design
- Allows better caching (only recompile changed modules)

#### 2. Reuse Existing Import Resolution

Use `resolve_import` from `oric/src/eval/module/import.rs` which already handles:
- Relative paths (`./helper`, `../utils`)
- Module paths (`std.math`)
- Directory modules (`./http` → `http/mod.ori`)
- `ORI_STDLIB` environment variable
- Cycle detection via `LoadingContext`

#### 3. Module-Qualified Name Mangling

Functions from different modules may have the same name. Use module-qualified names consistent with the approved AOT proposal:

```llvm
; From main.ori
define void @_ori_main() { ... }

; From helper.ori
define void @_ori_helper_my_assert(i1 %b) { ... }

; From std/math.ori
define i64 @_ori_std_math_sqrt(i64 %n) { ... }

; From http/mod.ori
define i64 @_ori_http_mod_connect(...) { ... }
```

The mangling scheme follows `_ori_<module-path>_<function-name>`:
- `_ori_` prefix for all Ori symbols
- Module path with `/` replaced by `_`
- Function name (without `@` prefix)

This integrates with the existing `ori demangle` command.

#### 4. Directory Module Support

When importing a directory module:

```ori
use "./http" { Client, get }
```

Resolution:
1. Check if `http/mod.ori` exists
2. If yes, resolve `http` to `http/mod.ori`
3. Compile `mod.ori` and its re-exported dependencies

```
src/
├── main.ori       → build/obj/main.o
└── http/
    ├── mod.ori    → build/obj/http_mod.o
    ├── client.ori → build/obj/http_client.o (if re-exported)
    └── server.ori → (not compiled if not used)
```

#### 5. Type Signature Propagation

Imported functions need correct type signatures. Use the existing type checker results from each module via Salsa queries.

### What Gets Compiled

**Explicitly imported functions:**
```ori
use "./helper" { my_assert }  // Compiles @my_assert
```

**All public functions for module aliases:**
```ori
use "./helper" as h           // Compiles all pub functions
h.my_assert(b: true)
```

**Private functions with :: prefix:**
```ori
use "./helper" { ::internal } // Compiles @internal
```

**Transitive dependencies:**
If `@my_assert` calls `@internal_helper`, `@internal_helper` is also compiled even if not explicitly exported.

### Implementation

#### Leverage Existing Infrastructure

The implementation uses existing 21B infrastructure:

```rust
// compiler/ori_llvm/src/aot/multi_file.rs

use crate::aot::incremental::{DependencyGraph, Cache};
use crate::aot::object::emit_object;
use crate::aot::linker::link;

pub fn compile_multi_file(
    entry_path: &Path,
    options: &CompileOptions,
    build: &BuildConfig,
) -> Result<(), CompileError> {
    // 1. Build dependency graph (existing: incremental/deps.rs)
    let graph = DependencyGraph::from_entry(entry_path)?;

    // 2. Detect cycles (via LoadingContext, reuses no-circular-imports logic)
    graph.check_cycles()?;

    // 3. Topological sort for compilation order
    let order = graph.topological_order()?;

    // 4. Check cache for each module (existing: incremental/cache.rs)
    let to_compile: Vec<_> = order.iter()
        .filter(|m| !Cache::is_valid(m, options))
        .collect();

    // 5. Compile modules in parallel (existing: incremental/parallel.rs)
    let objects: Vec<PathBuf> = to_compile.par_iter()
        .map(|module| {
            let obj = compile_module_to_object(module, options)?;
            Cache::store(module, &obj, options)?;
            Ok(obj)
        })
        .collect::<Result<Vec<_>, _>>()?;

    // 6. Collect all objects (cached + newly compiled)
    let all_objects = collect_all_objects(&order, &build.cache_dir)?;

    // 7. Link all objects (existing: aot/linker.rs)
    link(&all_objects, &build.output_path, &options.target, build.link_mode)?;

    Ok(())
}
```

#### Updated `compile_to_llvm`

```rust
pub fn compile_to_llvm<'ctx>(
    context: &'ctx Context,
    db: &CompilerDb,
    source_path: &Path,
) -> Result<Module<'ctx>, CompileError> {
    let file = SourceFile::new(db, source_path)?;

    // Parse and type-check (Salsa cached)
    let parse_result = parsed(db, file);
    let type_result = typed(db, file);

    // Check for errors
    if parse_result.has_errors() || type_result.has_errors() {
        return Err(CompileError::CheckFailed { path: source_path.to_owned() });
    }

    // Generate module name from path
    let module_name = source_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("module");

    let compiler = ModuleCompiler::new(context, db.interner(), module_name);
    compiler.declare_runtime();

    // Register struct types from this module
    for type_decl in &parse_result.module.types {
        if let TypeDeclKind::Struct(fields) = &type_decl.kind {
            let field_names: Vec<_> = fields.iter().map(|f| f.name).collect();
            compiler.register_struct(type_decl.name, field_names);
        }
    }

    // Compile functions from this module (with mangled names)
    let arena = &parse_result.arena;
    let expr_types = &type_result.expr_types;
    for func in &parse_result.module.functions {
        compiler.compile_function_mangled(func, module_name, arena, expr_types);
    }

    // Declare external symbols for imports (resolved at link time)
    for import in &parse_result.module.imports {
        compiler.declare_imports(import, db)?;
    }

    Ok(compiler.into_module())
}
```

### Visibility Rules

Only compile functions that are:
1. Explicitly imported: `use "./mod" { func_a, func_b }`
2. Or all public functions for module aliases: `use "./mod" as m`
3. Or private with `::` prefix: `use "./mod" { ::private_fn }`
4. Or transitively called by any of the above

### Error Handling

Errors integrate with the existing diagnostic system:

```rust
// In ori_diagnostic (or appropriate location)

/// E5004: Import target not found
pub struct ImportNotFound {
    pub path: String,
    pub searched: Vec<PathBuf>,
    pub span: Span,
}

/// E5005: Imported item not found in module
pub struct ItemNotFound {
    pub item: String,
    pub module: PathBuf,
    pub available: Vec<String>, // For "did you mean?" suggestions
    pub span: Span,
}

/// E5006: Imported item is private
pub struct PrivateItem {
    pub item: String,
    pub module: PathBuf,
    pub span: Span,
}
```

Error messages follow the diagnostic style guide:

```
error[E5004]: import target not found
  --> src/main.ori:1:5
   |
 1 | use "./nonexistent" { helper }
   |     ^^^^^^^^^^^^^^^ module not found
   |
   = note: searched: src/nonexistent.ori, src/nonexistent/mod.ori
   = help: check that the file exists and the path is correct
```

```
error[E5006]: `secret` is private
  --> src/main.ori:1:21
   |
 1 | use "./internal" { secret }
   |                    ^^^^^^ cannot import private item
   |
   = help: use `{ ::secret }` for explicit private access (testing only)
   = help: or make `secret` public with `pub @secret`
```

## Alternatives Considered

### 1. Single LLVM Module for All Files

Compile all files into one LLVM module instead of separate objects.

**Rejected:**
- Doesn't enable parallel compilation
- Cache invalidation affects entire module
- Inconsistent with approved AOT proposal design
- Memory pressure with large projects

### 2. Generate External Declarations Only

For imported functions, just generate `declare` without `define`, relying on linker.

**Considered and Adopted:**
This is actually the correct approach for separate compilation—each module's object file declares (but doesn't define) symbols from imports. The linker resolves them.

### 3. Inline Imported Functions at Call Sites

Copy the function body into the calling module.

**Rejected:**
- Duplicates code in each object file
- Breaks if imported function calls other functions from its module
- Memory inefficient
- LTO can achieve inlining at link time anyway

## Implementation Plan

### Phase 1: Dependency Graph Infrastructure
- [ ] Add `DependencyGraph::from_entry()` using existing import resolution
- [ ] Implement topological sorting for compilation order
- [ ] Wire cycle detection to error reporting (reuse E5003)
- [ ] Handle directory modules (`mod.ori`)

### Phase 2: Per-Module Compilation
- [ ] Add `compile_module_to_object()` in `multi_file.rs`
- [ ] Implement module-qualified name mangling (`_ori_<module>_<function>`)
- [ ] Generate `declare` for imported symbols
- [ ] Update `ori demangle` to handle module paths

### Phase 3: Linking Integration
- [ ] Collect all object files for linking
- [ ] Pass correct library search paths for stdlib
- [ ] Handle `ORI_STDLIB` for std.* imports

### Phase 4: Cache Integration
- [ ] Wire up incremental cache (21B.6) to skip unchanged modules
- [ ] Store module hashes including import signatures
- [ ] Invalidate dependents when a module changes

### Phase 5: Testing
- [ ] Unit tests for `DependencyGraph`
- [ ] Unit tests for module-qualified mangling
- [ ] Integration tests with multi-file programs
- [ ] Integration tests with directory modules
- [ ] Integration tests with stdlib imports
- [ ] Run existing spec tests that use imports through AOT

## Testing

### Test Cases

1. **Basic import**: `use "./helper" { func }` compiles and runs
2. **Transitive imports**: A imports B imports C
3. **Circular import detection**: A imports B imports A → clear error (E5003)
4. **Missing import**: `use "./nonexistent"` → clear error (E5004)
5. **Missing item**: `use "./mod" { nonexistent }` → clear error (E5005)
6. **Private function**: `use "./mod" { private }` → error (E5006) without `::`
7. **Private with ::** : `use "./mod" { ::private }` → compiles
8. **Module alias**: `use "./mod" as m` then `m.func()`
9. **Directory module**: `use "./http"` resolves to `http/mod.ori`
10. **Re-exports**: `pub use "./internal" { helper }` in `mod.ori`
11. **Stdlib import**: `use std.math { abs }` with `ORI_STDLIB` set
12. **Parallel compilation**: Multiple independent modules compile in parallel
13. **Incremental rebuild**: Change one module, only that module recompiles

### Success Criteria

```bash
# This should work after implementation:
echo 'pub @helper () -> int = 42' > /tmp/helper.ori
echo 'use "./helper" { helper }
@main () -> void = assert(condition: helper() == 42)' > /tmp/main.ori

ori build /tmp/main.ori -o /tmp/main
/tmp/main  # Should exit 0

# With verbose output showing separate compilation:
ori build /tmp/main.ori -o /tmp/main -v
# Compiling /tmp/helper.ori -> /tmp/build/obj/helper.o
# Compiling /tmp/main.ori -> /tmp/build/obj/main.o
# Linking /tmp/build/obj/*.o -> /tmp/main
```

## Dependencies

- **Blocks:** AOT Test Backend proposal (can't run multi-file tests without this)
- **Depends on:** None (uses existing import resolution infrastructure)
- **Related:**
  - `no-circular-imports-proposal.md` (cycle detection)
  - `module-system-details-proposal.md` (import semantics)
  - `aot-compilation-proposal.md` (object emission, linking)

## Summary

Multi-file AOT compilation is a critical missing feature that causes silent failures for any Ori program using imports. The fix:

1. Uses existing import resolution to build a dependency graph
2. Compiles each module to its own object file (enables parallelism, caching)
3. Uses module-qualified name mangling (`_ori_<module>_<function>`)
4. Declares imported symbols (linker resolves them)
5. Integrates with existing incremental compilation infrastructure

This matches the interpreter's semantics while enabling efficient compilation of large projects.

## Design Decisions

1. **Separate objects per module**: Enables parallel compilation and incremental rebuilds
2. **Module-qualified mangling**: `_ori_<module>_<function>` format, consistent with AOT proposal
3. **Linker-resolved imports**: Each object declares imports; linker resolves
4. **Full module support**: Handles both file imports and directory modules (`mod.ori`)
5. **Error integration**: Uses existing diagnostic system with E5004-E5006 error codes
