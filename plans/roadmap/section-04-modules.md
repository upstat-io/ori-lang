---
section: 4
title: Module System
status: in-progress
tier: 1
goal: Multi-file compilation
spec:
  - spec/12-modules.md
sections:
  - id: "4.1"
    title: Module Definition
    status: in-progress
  - id: "4.2"
    title: Import Parsing
    status: in-progress
  - id: "4.3"
    title: Visibility
    status: in-progress
  - id: "4.4"
    title: Module Resolution
    status: in-progress
  - id: "4.5"
    title: Test Modules
    status: complete
  - id: "4.6"
    title: Prelude
    status: in-progress
  - id: "4.7"
    title: Import Graph Tooling
    status: not-started
  - id: "4.8"
    title: Module System Details
    status: not-started
  - id: "4.9"
    title: Remaining Work (Pre-existing)
    status: in-progress
  - id: "4.10"
    title: Section Completion Checklist
    status: in-progress
  - id: "4.11"
    title: Module-Level Constants
    status: not-started
  - id: "4.12"
    title: Extension Methods
    status: not-started
---

# Section 4: Module System

**Goal**: Multi-file compilation

> **SPEC**: `spec/12-modules.md`
> **DESIGN**: `design/09-modules/index.md`
> **PROPOSAL**: `proposals/approved/no-circular-imports-proposal.md` — Circular import rejection
> **PROPOSAL**: `proposals/approved/module-system-details-proposal.md` — Entry points, re-export chains, visibility

**Status**: In-progress — Core evaluator complete (4.1-4.6), LLVM multi-file infrastructure present (dependency graph, topological sort, symbol mangling), tooling pending (4.7), module details pending (4.8), constants pending (4.11), extension methods pending (4.12). Verified 2026-02-10.

---

## 4.1 Module Definition

- [x] **Implement**: Module structure — spec/12-modules.md § Module Structure ✅ (2026-02-10)
  - [x] **Rust Tests**: `oric/src/eval/module/import.rs` — module loading tests
  - [x] **Ori Tests**: `tests/spec/modules/use_imports.ori` (10 tests, pub/private functions, types, config vars)
  - [ ] **LLVM Support**: LLVM codegen for module loading — multi_file.rs infrastructure exists, no dedicated tests
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/module_tests.rs` — module loading codegen (file does not exist)

- [x] **Implement**: Module corresponds to file — spec/12-modules.md § Module Structure ✅ (2026-02-10)
  - [x] **Rust Tests**: `oric/src/eval/module/import.rs` — file mapping tests
  - [x] **Ori Tests**: `tests/spec/modules/use_imports.ori`

- [x] **Implement**: Module name from file path — spec/12-modules.md § Module Structure ✅ (2026-02-10)
  - [x] **Rust Tests**: `oric/src/eval/module/import.rs` — path resolution tests (`test_generate_relative_candidates_file_module`)
  - [x] **Ori Tests**: N/A (tested via Rust unit tests)

---

## 4.2 Import Parsing

**Relative imports:**

- [x] **Implement**: `use './path' { item1, item2 }` — spec/12-modules.md § Relative Imports ✅ (2026-02-10)
  - [x] **Rust Tests**: `oric/src/eval/module/import.rs` — relative path parsing
  - [x] **Ori Tests**: `tests/spec/modules/_test/use_imports.test.ori` (4 tests: add, make_multiplier, calculate, double)

- [x] **Implement**: Parent `use '../utils' { helper }` — spec/12-modules.md § Relative Imports ✅ (2026-02-10)
  - [x] **Rust Tests**: `oric/src/eval/module/import.rs` — parent path resolution (`test_generate_relative_candidates`)
  - [x] **Ori Tests**: `tests/spec/modules/_test/use_imports.test.ori` (uses `"../use_imports"`)

- [x] **Implement**: Subdirectory `use './http/client' { get }` — spec/12-modules.md § Relative Imports ✅ (2026-02-10)
  - [x] **Rust Tests**: `oric/src/eval/module/import.rs` — subdirectory path resolution (`test_generate_relative_candidates_nested_directory`)
  - [x] **Ori Tests**: N/A (tested via Rust unit tests)

**Module imports:**

- [x] **Implement**: `use std.module { item }` — spec/12-modules.md § Module Imports ✅ (2026-02-10)
  - [x] **Rust Tests**: `oric/src/eval/module/import.rs` — stdlib path resolution
  - [x] **Ori Tests**: All test files use `use std.testing { assert_eq }`

- [ ] **Implement**: Nested `use std.net.http { get }` — spec/12-modules.md § Module Imports
  - [ ] **Rust Tests**: `oric/src/eval/module/import.rs` — nested module resolution
  - [ ] **Ori Tests**: N/A — no nested stdlib modules exist yet to test

**Private imports:**

- [x] **Implement**: `use './path' { ::private_item }` — spec/12-modules.md § Private Imports ✅ (2026-02-10)
  - [x] **Rust Tests**: `oric/src/eval/module/import.rs` — private import handling
  - [x] **Ori Tests**: `tests/spec/modules/_test/use_private.test.ori` (2 tests: private fn access, private + public combo)

- [x] **Implement**: `::` prefix — spec/12-modules.md § Private Imports ✅ (2026-02-10)
  - [x] **Rust Tests**: `ori_parse/src/grammar/` — `::` prefix parsing
  - [x] **Ori Tests**: `tests/spec/modules/_test/use_private.test.ori`

**Aliases:**

- [x] **Implement**: `use './math' { add as plus }` — spec/12-modules.md § Aliases ✅ (2026-02-10)
  - [x] **Rust Tests**: `ori_parse/src/grammar/` — alias parsing
  - [x] **Ori Tests**: `tests/spec/modules/_test/use_aliases.test.ori` (3 tests: aliased functions)

- [x] **Implement**: Module alias `use std.net.http as http` — spec/12-modules.md § Aliases ✅ (2026-02-10)
  - [x] **Rust Tests**: `ori_parse/src/grammar/` — module alias parsing
  - [x] **Ori Tests**: `tests/spec/modules/_test/module_alias.test.ori` (11 tests: qualified access `math.add()`, etc.)
  - Note: Parsing and runtime complete; qualified access works via evaluator. Type checker ModuleNamespace support pending.

---

## 4.3 Visibility

- [x] **Implement**: `pub` on functions — spec/12-modules.md § Visibility ✅ (2026-02-10)
  - [x] **Rust Tests**: `ori_parse/src/grammar/` — `pub` keyword parsing
  - [x] **Ori Tests**: `tests/spec/modules/use_imports.ori` (`pub @add`, `pub @make_multiplier`, etc.)

- [x] **Implement**: `pub` on types — spec/12-modules.md § Visibility ✅ (2026-02-10)
  - [x] **Rust Tests**: `ori_parse/src/grammar/` — type visibility parsing
  - [x] **Ori Tests**: `library/std/prelude.ori` — `pub type Option`, `pub type Result`; `use_imports.ori` has `pub type Point`

- [x] **Implement**: `pub` on config variables — spec/12-modules.md § Visibility ✅ (2026-02-10)
  - [x] **Rust Tests**: `ori_parse/src/grammar/` — config visibility parsing
  - [x] **Ori Tests**: `tests/spec/modules/use_imports.ori` (`pub $default_timeout`, private `$internal_limit`)

- [x] **Implement**: Re-exports `pub use` — spec/12-modules.md § Re-exports ✅ (2026-02-10)
  - [x] **Rust Tests**: `ori_parse/src/grammar/` — re-export parsing
  - [x] **Ori Tests**: `tests/spec/modules/reexporter.ori` (`pub use "./math_lib" { add, multiply }`)
  - Note: Basic re-export works; multi-level chain resolution pending (4.8)

- [x] **Implement**: Private by default — spec/12-modules.md § Visibility ✅ (2026-02-10)
  - [x] **Rust Tests**: `oric/src/eval/module/` — visibility enforcement
  - [x] **Ori Tests**: `tests/spec/modules/_test/use_private.test.ori` (private access with `::` prefix)

---

## 4.4 Module Resolution

- [x] **Implement**: File path resolution — spec/12-modules.md § Module Resolution ✅ (2026-02-10)
  - [x] **Rust Tests**: `oric/src/eval/module/import.rs` — path resolution tests (`test_generate_relative_candidates_*`)
  - [x] **Ori Tests**: `tests/spec/modules/_test/directory_module.test.ori` (file + dir modules), `_test/precedence.test.ori` (file precedence over dir)

- [x] **Implement**: Module dependency graph — spec/12-modules.md § Dependency Graph ✅ (2026-02-10)
  - [x] **Rust Tests**: `oric/src/eval/module/import.rs` — `LoadingContext` tests
  - [x] **Ori Tests**: N/A (tested via Rust unit tests)
  - Note: LLVM also has `DependencyGraph` in `ori_llvm/src/aot/incremental/deps.rs` for AOT multi-file

- [x] **Implement**: Cycle detection — spec/12-modules.md § Cycle Detection, proposals/approved/no-circular-imports-proposal.md ✅ (2026-02-10)
  - [x] **Rust Tests**: `oric/src/eval/module/import.rs` — `test_loading_context_cycle_detection`, `test_loading_context_cycle_error`
  - [x] **Ori Tests**: N/A (tested via Rust unit tests)
  - Note: LLVM multi_file.rs also has cycle detection (`CyclicDependency` error)

- [ ] **Implement**: Enhanced cycle error messages — proposals/approved/no-circular-imports-proposal.md § Error Message
  - [ ] Show full cycle path in error (a.ori -> b.ori -> a.ori)
  - [ ] Include actionable help: "extract shared types", "use dependency inversion", "restructure boundaries"
  - [ ] **Rust Tests**: `oric/src/eval/module/import.rs` — cycle error formatting tests
  - [ ] **Ori Tests**: `tests/spec/modules/cycle_error_message.ori`

- [ ] **Implement**: Report all cycles (not just first) — proposals/approved/no-circular-imports-proposal.md § Detection Algorithm
  - [ ] Continue detection after finding first cycle
  - [ ] Report each cycle with full path
  - [ ] **Rust Tests**: `oric/src/eval/module/import.rs` — multi-cycle detection tests
  - [ ] **Ori Tests**: `tests/spec/modules/multiple_cycles.ori`

- [x] **Implement**: Name resolution — spec/12-modules.md § Name Resolution ✅ (2026-02-10)
  - [x] **Rust Tests**: `oric/src/eval/module/` — name resolution tests
  - [x] **Ori Tests**: All import tests verify name resolution (use_imports, use_private, use_aliases, module_alias)

- [x] **Implement**: Qualified access — spec/12-modules.md § Qualified Access ✅ evaluator (2026-02-10)
  - [x] **Rust Tests**: `oric/src/eval/` — qualified access evaluation
  - [x] **Ori Tests**: `tests/spec/modules/_test/module_alias.test.ori` (11 tests: `math.add()`, `math.multiply()`, etc.)
  - [ ] **LLVM Support**: LLVM codegen for qualified access dispatch — multi_file.rs has module-qualified mangling (`_ori_<module>$<function>`)
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/module_tests.rs` — qualified access codegen (file does not exist)
  - Note: Runtime evaluation complete; type checker needs ModuleNamespace support

---

## 4.5 Test Modules

- [x] **Implement**: `_test/` convention — spec/12-modules.md § Test Modules ✅ (2026-02-10)
  - [x] **Rust Tests**: `oric/src/eval/module/import.rs` — test module detection (`test_is_test_module_valid`, `_not_in_test_dir`, `_wrong_extension`, `_nested`)
  - [x] **Ori Tests**: `tests/spec/modules/_test/test_module_access.test.ori` (2 tests)

- [x] **Implement**: Test files access private items — spec/12-modules.md § Test Modules ✅ (2026-02-10)
  - [x] **Rust Tests**: `oric/src/eval/module/import.rs` — private access rules (`test_is_parent_module_import_*`)
  - [x] **Ori Tests**: `tests/spec/modules/_test/test_module_access.test.ori` (accesses private items without `::` prefix)

---

## 4.6 Prelude

- [x] **Implement**: Types: `Option`, `Result`, `Error`, `Ordering` — spec/12-modules.md § Prelude ✅ (2026-02-10)
  - [x] **Rust Tests**: `oric/src/eval/` — built-in type tests
  - [x] **Ori Tests**: Option/Result used throughout `tests/spec/`, Ordering verified in `tests/spec/types/ordering/`
  - [ ] **LLVM Support**: LLVM codegen for prelude type representations — Option/Result have inline IR in lower_calls.rs
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/module_tests.rs` — prelude type codegen (file does not exist)

- [x] **Implement**: Built-in functions: `print`, `panic`, `int`, `float`, `str`, `byte` — spec/12-modules.md § Prelude ✅ (2026-02-10)
  - [x] **Rust Tests**: `oric/src/eval/evaluator/` — `register_prelude()` tests
  - [x] **Ori Tests**: Built-ins used throughout test suite
  - [x] **LLVM Support**: LLVM codegen for built-in functions — `print` via `_ori_print`, `panic` via `_ori_panic`, conversions via inline IR
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/module_tests.rs` — built-in function codegen (file does not exist)

- [x] **Implement**: Built-in methods: `.len()`, `.is_empty()`, `.is_some()`, etc. — Lean Core ✅ (2026-02-10)
  - [x] **Rust Tests**: `ori_eval/src/methods.rs` — method dispatch tests
  - [x] **Ori Tests**: `tests/spec/traits/core/` — len (14 tests), comparable (58 tests); `tests/spec/types/` — option, result tests
  - [x] **LLVM Support**: LLVM codegen for built-in methods — inline IR in `lower_calls.rs` (len, is_empty, is_some, is_none, unwrap, unwrap_or, is_ok, is_err, compare)
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/module_tests.rs` — built-in method codegen (file does not exist)

- [x] **Implement**: Auto-import prelude from `library/std/prelude.ori` — spec/12-modules.md § Prelude ✅ (2026-02-10)
  - [x] `Evaluator::load_prelude()` auto-loads prelude before any module
  - [x] All public functions from prelude available without import
  - [x] **Rust Tests**: `oric/src/eval/evaluator/` — prelude loading tests
  - [x] **Ori Tests**: All test files use `use std.testing { assert_eq }` which depends on prelude
  - [ ] **LLVM Support**: LLVM codegen for prelude auto-loading
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/module_tests.rs` — prelude loading codegen (file does not exist)

- [x] **Implement**: Prelude functions auto-available ✅ (2026-02-10)
  - [x] `assert`, `assert_eq`, `assert_ne`, `assert_some`, `assert_none`, `assert_ok`, `assert_err`
  - [x] `is_some`, `is_none`, `is_ok`, `is_err`
  - [x] `len`, `is_empty`
  - [x] `compare`, `min`, `max`
  - [ ] **LLVM Support**: LLVM codegen for prelude functions — partial (print, panic, len, compare have IR; assert_* not yet)
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/module_tests.rs` — prelude function codegen (file does not exist)
  - Note: Trait definitions in prelude (Eq, Comparable, etc.) parse but need Section 3 for full integration

---

## 4.7 Import Graph Tooling

> **PROPOSAL**: `proposals/approved/no-circular-imports-proposal.md § Tooling Support`

- [ ] **Implement**: `ori check --cycles` — Check for cycles without full compilation
  - [ ] Fast path: parse imports only, build graph, detect cycles
  - [ ] **Rust Tests**: `oric/src/cli/` — cycle checking tests
  - [ ] **Ori Tests**: `tests/cli/check_cycles.ori`

- [ ] **Implement**: `ori graph --imports` — Visualize import graph
  - [ ] Output DOT format for graphviz
  - [ ] Usage: `ori graph --imports > imports.dot && dot -Tpng imports.dot -o imports.png`
  - [ ] **Rust Tests**: `oric/src/cli/` — graph output tests
  - [ ] **Ori Tests**: `tests/cli/graph_imports.ori`

---

## 4.8 Module System Details

> **PROPOSAL**: `proposals/approved/module-system-details-proposal.md`

### Entry Point Files

- [ ] **Implement**: `lib.ori` as library entry point — spec/12-modules.md § Entry Point Files
  - [ ] **Rust Tests**: `oric/src/eval/module/` — library entry detection
  - [ ] **Ori Tests**: `tests/spec/modules/library_entry.ori`

- [ ] **Implement**: Distinguish `lib.ori` vs `mod.ori` — spec/12-modules.md § Entry Point Files
  - [ ] Package root requires `lib.ori`, not `mod.ori`
  - [ ] **Rust Tests**: `oric/src/eval/module/` — entry point validation
  - [ ] **Ori Tests**: `tests/spec/modules/entry_point_validation.ori`

### Binary-Library Separation

- [ ] **Implement**: Binary accesses library via public API only — spec/12-modules.md § Library + Binary
  - [ ] `use "my_pkg" { item }` accesses `lib.ori` exports
  - [ ] `use "my_pkg" { ::private }` is an error (no private access)
  - [ ] **Rust Tests**: `oric/src/eval/module/` — binary-library access tests
  - [ ] **Ori Tests**: `tests/spec/modules/binary_library_access.ori`

### Re-export Chains

- [ ] **Implement**: Multi-level re-export resolution — spec/12-modules.md § Re-export Chains
  - [ ] Track visibility through chain (all levels must be `pub`)
  - [ ] Aliases propagate through chains
  - [ ] **Rust Tests**: `oric/src/eval/module/` — re-export chain tests
  - [ ] **Ori Tests**: `tests/spec/modules/reexport_chain.ori`

- [ ] **Implement**: Diamond re-exports — spec/12-modules.md § Re-export Chains
  - [ ] Same item via multiple paths is not an error
  - [ ] **Rust Tests**: `oric/src/eval/module/` — diamond import tests
  - [ ] **Ori Tests**: `tests/spec/modules/diamond_reexport.ori`

### Error Messages

- [ ] **Implement**: E1101 (missing module) — proposals/approved/module-system-details-proposal.md § Error Messages
  - [ ] Show paths checked: `file.ori`, `file/mod.ori`
  - [ ] **Rust Tests**: `oric/src/diagnostics/` — error formatting tests
  - [ ] **Ori Tests**: `tests/spec/modules/error_missing_module.ori`

- [ ] **Implement**: E1102 (missing export) — proposals/approved/module-system-details-proposal.md § Error Messages
  - [ ] Show available exports in error message
  - [ ] "Did you mean?" suggestion
  - [ ] **Rust Tests**: `oric/src/diagnostics/` — error formatting tests
  - [ ] **Ori Tests**: `tests/spec/modules/error_missing_export.ori`

- [ ] **Implement**: E1103 (private item) — proposals/approved/module-system-details-proposal.md § Error Messages
  - [ ] Help text: "use `::item` for explicit private access"
  - [ ] **Rust Tests**: `oric/src/diagnostics/` — error formatting tests
  - [ ] **Ori Tests**: `tests/spec/modules/error_private_item.ori`

---

## 4.9 Remaining Work (Pre-existing)

**Parsing/Runtime complete, type checker pending:**
- [x] Module alias syntax: `use "../math_lib" as math` — parsing ✅, runtime ✅ (verified via 11 tests in module_alias.test.ori)
- [x] Re-exports: `pub use './client' { get, post }` — basic parsing ✅, basic resolution ✅ (verified via reexporter.ori)
- [x] Qualified access: `module.function()` — runtime ✅ (verified via module_alias.test.ori)
- [ ] Type checker ModuleNamespace support — pending
- [ ] Multi-level re-export chain resolution — pending (4.8)
- [ ] Nested stdlib modules (`std.net.http`) — no modules to test yet

---

## 4.11 Module-Level Constants

**Source**: `grammar.ebnf § constant_decl`, `spec/04-constants.md`

Module-level constants declared with `let $NAME = value`.

```ori
let $PI = 3.14159
let $MAX_SIZE: int = 1000
pub let $VERSION = "1.0.0"
```

**Status**: Parser complete, evaluator incomplete.

### Parser

- [ ] **Implement**: Parse `let $NAME = value` — `constant_decl` production
  - [ ] **Rust Tests**: `ori_parse/src/grammar/` — constant parsing
  - [ ] **Ori Tests**: `tests/spec/declarations/constants.ori`

- [ ] **Implement**: Parse typed constants `let $NAME: Type = value`
  - [ ] **Rust Tests**: `ori_parse/src/grammar/` — typed constant parsing

- [ ] **Implement**: Parse public constants `pub let $NAME = value`
  - [ ] **Rust Tests**: `ori_parse/src/grammar/` — visibility parsing

### Evaluator

- [ ] **Implement**: Evaluate module-level constants at load time
  - [ ] **Rust Tests**: `ori_eval/src/interpreter/mod.rs` — constant evaluation
  - [ ] **Ori Tests**: `tests/spec/declarations/constants_eval.ori`
  - [ ] **LLVM Support**: LLVM codegen for module constants
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/constant_tests.rs`

- [ ] **Implement**: Register constants in module namespace
  - [ ] **Rust Tests**: `ori_eval/src/interpreter/module_loading.rs` — constant registration
  - [ ] **Ori Tests**: `tests/spec/modules/import_constants.ori`

### Type Checker

- [ ] **Implement**: Type check constant initializers
  - [ ] **Rust Tests**: `ori_typeck/src/checker/` — constant type checking
  - [ ] **Ori Tests**: `tests/spec/types/constant_types.ori`

- [ ] **Implement**: Enforce constant expression restrictions (no function calls with side effects)
  - [ ] **Rust Tests**: `ori_typeck/src/checker/` — constant expression validation
  - [ ] **Ori Compile-Fail Tests**: `tests/compile-fail/constant_non_const_expr.ori`

### Import/Export

- [ ] **Implement**: Export constants via `pub let`
  - [ ] **Ori Tests**: `tests/spec/modules/export_constants.ori`

- [ ] **Implement**: Import constants via `use "path" { $CONST }`
  - [ ] **Ori Tests**: `tests/spec/modules/import_constants.ori`

---

## 4.12 Extension Methods

> **PROPOSAL**: `proposals/approved/extension-methods-proposal.md`

Extension methods add methods to existing types without modifying their definition.

### Extension Definition

- [ ] **Implement**: `extend Type { @method (self) -> T = ... }` — proposals/approved/extension-methods-proposal.md § Extension Definition
  - [ ] Parse `extend` blocks
  - [ ] Register extension methods in type environment
  - [ ] **Rust Tests**: `ori_parse/src/grammar/` — extension parsing tests
  - [ ] **Ori Tests**: `tests/spec/extensions/definition.ori`

- [ ] **Implement**: Constrained extensions with angle brackets — proposals/approved/extension-methods-proposal.md § Constrained Extensions
  - [ ] `extend<T: Clone> [T] { ... }` syntax
  - [ ] **Rust Tests**: `ori_parse/src/grammar/` — generic extension parsing
  - [ ] **Ori Tests**: `tests/spec/extensions/constrained.ori`

- [ ] **Implement**: Constrained extensions with where clause — proposals/approved/extension-methods-proposal.md § Constrained Extensions
  - [ ] `extend [T] where T: Clone { ... }` syntax (equivalent to angle bracket)
  - [ ] **Rust Tests**: `ori_parse/src/grammar/` — where clause parsing
  - [ ] **Ori Tests**: `tests/spec/extensions/constrained_where.ori`

- [ ] **Implement**: Extension visibility — proposals/approved/extension-methods-proposal.md § Visibility
  - [ ] `pub extend` makes all methods public
  - [ ] Non-pub `extend` is module-private
  - [ ] Block-level visibility only (no per-method pub)
  - [ ] **Rust Tests**: `oric/src/eval/module/` — visibility tests
  - [ ] **Ori Tests**: `tests/spec/extensions/visibility.ori`

- [ ] **Implement**: Extension restrictions — proposals/approved/extension-methods-proposal.md § What Can Be Extended
  - [ ] Error on field addition attempt
  - [ ] Error on trait implementation in extend block
  - [ ] Error on override of existing method
  - [ ] Error on static method (no self)
  - [ ] **Rust Tests**: `oric/src/diagnostics/` — restriction error tests
  - [ ] **Ori Tests**: `tests/spec/extensions/restrictions.ori`

### Extension Import

- [ ] **Implement**: `extension "path" { Type.method }` — proposals/approved/extension-methods-proposal.md § Extension Import
  - [ ] Parse `extension` import syntax
  - [ ] Method-level granularity required
  - [ ] **Rust Tests**: `ori_parse/src/grammar/` — extension import parsing
  - [ ] **Ori Tests**: `tests/spec/extensions/import.ori`

- [ ] **Implement**: Wildcard prohibition — proposals/approved/extension-methods-proposal.md § Import Syntax
  - [ ] Error on `extension "path" { Type.* }`
  - [ ] **Rust Tests**: `oric/src/diagnostics/` — wildcard error tests
  - [ ] **Ori Tests**: `tests/spec/extensions/no_wildcard.ori`

- [ ] **Implement**: Re-export extensions — proposals/approved/extension-methods-proposal.md § Scoping
  - [ ] `pub extension "path" { Type.method }` for re-export
  - [ ] No transitive propagation without explicit re-export
  - [ ] **Rust Tests**: `oric/src/eval/module/` — re-export tests
  - [ ] **Ori Tests**: `tests/spec/extensions/reexport.ori`

### Method Resolution

- [ ] **Implement**: Resolution order — proposals/approved/extension-methods-proposal.md § Resolution Order
  - [ ] Inherent > Trait > Extension
  - [ ] **Rust Tests**: `oric/src/typeck/` — resolution order tests
  - [ ] **Ori Tests**: `tests/spec/extensions/resolution_order.ori`

- [ ] **Implement**: Conflict detection — proposals/approved/extension-methods-proposal.md § Conflict Resolution
  - [ ] Error on ambiguous extension methods
  - [ ] Qualified syntax for disambiguation: `module.Type.method(v)`
  - [ ] **Rust Tests**: `oric/src/typeck/` — conflict detection tests
  - [ ] **Ori Tests**: `tests/spec/extensions/conflict.ori`

### Orphan Rules

- [ ] **Implement**: Same-package rule — proposals/approved/extension-methods-proposal.md § Orphan Rules
  - [ ] Extension must be in same package as type OR trait bound
  - [ ] Error for foreign type without local trait bound
  - [ ] **Rust Tests**: `oric/src/typeck/` — orphan rule tests
  - [ ] **Ori Tests**: `tests/spec/extensions/orphan.ori`

### Error Messages

- [ ] **Implement**: E0850 (ambiguous extension) — proposals/approved/extension-methods-proposal.md § Error Messages
  - [ ] Show all candidate extensions
  - [ ] Help text for qualified syntax
  - [ ] **Rust Tests**: `oric/src/diagnostics/` — error formatting tests
  - [ ] **Ori Tests**: `tests/spec/extensions/error_ambiguous.ori`

- [ ] **Implement**: E0851 (method not found) — proposals/approved/extension-methods-proposal.md § Error Messages
  - [ ] Suggest extension import if method exists in known module
  - [ ] **Rust Tests**: `oric/src/diagnostics/` — error formatting tests
  - [ ] **Ori Tests**: `tests/spec/extensions/error_not_found.ori`

- [ ] **Implement**: E0852 (orphan violation) — proposals/approved/extension-methods-proposal.md § Error Messages
  - [ ] Show package location of foreign type
  - [ ] Help: "define a newtype wrapper or use a local trait bound"
  - [ ] **Rust Tests**: `oric/src/diagnostics/` — error formatting tests
  - [ ] **Ori Tests**: `tests/spec/extensions/error_orphan.ori`

### LLVM Support

- [ ] **Implement**: Extension method codegen — Extension methods in LLVM backend
  - [ ] Same codegen as regular methods
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/extension_tests.rs`

**Note on Type Definitions:**
- Full prelude with user-defined Option, Result, etc. requires Section 5 (Type Declarations)
- Currently using built-in types in evaluator
- See section-05-type-declarations.md § 5.1-5.4 for type definition work

---

## 4.10 Section Completion Checklist

- [x] Core module imports working (relative, module, private, aliases) ✅
- [x] Visibility system working (`pub`, private by default, `::`) ✅
- [x] Module resolution working (path resolution, stdlib lookup, directory modules, file precedence) ✅
- [x] Cycle detection working (Rust unit tests: `test_loading_context_cycle_*`) ✅
- [x] Test module private access working (`_test/` convention, `test_module_access.test.ori`) ✅
- [x] Built-in prelude types and functions working (Option, Result, Ordering, print, panic, etc.) ✅
- [x] Auto-load stdlib prelude (`use std.testing` works in all test files) ✅
- [x] `Self` type parsing in traits (see Section 3) ✅
- [x] Trait/impl parsing at module level (see Section 3) ✅
- [x] Module alias syntax (`use "../path" as alias`) — parsing/runtime complete ✅
- [x] Re-exports (`pub use`) — basic parsing/resolution complete ✅
- [x] Qualified access (`module.function()`) — runtime complete ✅
- [ ] Type checker ModuleNamespace support — pending
- [ ] LLVM multi-file AOT compilation — infrastructure exists (multi_file.rs), no integration tests
- [ ] Enhanced cycle error messages (4.4) — pending
- [ ] Type definitions parsing (see Section 5)
- [ ] Run full test suite: `./test-all.sh`

**Exit Criteria**: Multi-file projects compile (core support complete)
**Status**: Section 4 evaluator and parser complete. LLVM multi-file infrastructure present. Type checker support for module namespaces pending. Verified 2026-02-10.
