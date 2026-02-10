---
section: 4
title: Module System
status: not-started
tier: 1
goal: Multi-file compilation
spec:
  - spec/12-modules.md
sections:
  - id: "4.1"
    title: Module Definition
    status: not-started
  - id: "4.2"
    title: Import Parsing
    status: not-started
  - id: "4.3"
    title: Visibility
    status: not-started
  - id: "4.4"
    title: Module Resolution
    status: not-started
  - id: "4.5"
    title: Test Modules
    status: not-started
  - id: "4.6"
    title: Prelude
    status: not-started
  - id: "4.7"
    title: Import Graph Tooling
    status: not-started
  - id: "4.8"
    title: Module System Details
    status: not-started
  - id: "4.9"
    title: Remaining Work (Pre-existing)
    status: not-started
  - id: "4.10"
    title: Section Completion Checklist
    status: not-started
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

**Status**: Partial — Core complete (4.1-4.6, 4.10), tooling pending (4.7), module details pending (4.8), extension methods pending (4.11)

---

## 4.1 Module Definition

- [ ] **Implement**: Module structure — spec/12-modules.md § Module Structure
  - [ ] **Rust Tests**: `oric/src/eval/module/import.rs` — module loading tests
  - [ ] **Ori Tests**: `tests/spec/modules/use_imports.ori`
  - [ ] **LLVM Support**: LLVM codegen for module loading
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/module_tests.rs` — module loading codegen

- [ ] **Implement**: Module corresponds to file — spec/12-modules.md § Module Structure
  - [ ] **Rust Tests**: `oric/src/eval/module/import.rs` — file mapping tests
  - [ ] **Ori Tests**: `tests/spec/modules/use_imports.ori`

- [ ] **Implement**: Module name from file path — spec/12-modules.md § Module Structure
  - [ ] **Rust Tests**: `oric/src/eval/module/import.rs` — path resolution tests
  - [ ] **Ori Tests**: N/A (tested via Rust unit tests)

---

## 4.2 Import Parsing

**Relative imports:**

- [ ] **Implement**: `use './path' { item1, item2 }` — spec/12-modules.md § Relative Imports
  - [ ] **Rust Tests**: `oric/src/eval/module/import.rs` — relative path parsing
  - [ ] **Ori Tests**: `tests/spec/modules/_test/use_imports.test.ori`

- [ ] **Implement**: Parent `use '../utils' { helper }` — spec/12-modules.md § Relative Imports
  - [ ] **Rust Tests**: `oric/src/eval/module/import.rs` — parent path resolution
  - [ ] **Ori Tests**: `tests/spec/modules/_test/use_imports.test.ori`

- [ ] **Implement**: Subdirectory `use './http/client' { get }` — spec/12-modules.md § Relative Imports
  - [ ] **Rust Tests**: `oric/src/eval/module/import.rs` — subdirectory path resolution
  - [ ] **Ori Tests**: N/A (tested via Rust unit tests)

**Module imports:**

- [ ] **Implement**: `use std.module { item }` — spec/12-modules.md § Module Imports
  - [ ] **Rust Tests**: `oric/src/eval/module/import.rs` — stdlib path resolution
  - [ ] **Ori Tests**: All test files use `use std.testing { assert_eq }`

- [ ] **Implement**: Nested `use std.net.http { get }` — spec/12-modules.md § Module Imports
  - [ ] **Rust Tests**: `oric/src/eval/module/import.rs` — nested module resolution
  - [ ] **Ori Tests**: N/A (tested via Rust unit tests)

**Private imports:**

- [ ] **Implement**: `use './path' { ::private_item }` — spec/12-modules.md § Private Imports
  - [ ] **Rust Tests**: `oric/src/eval/module/import.rs` — private import handling
  - [ ] **Ori Tests**: `tests/spec/modules/_test/use_private.test.ori`

- [ ] **Implement**: `::` prefix — spec/12-modules.md § Private Imports
  - [ ] **Rust Tests**: `ori_parse/src/grammar/` — `::` prefix parsing
  - [ ] **Ori Tests**: `tests/spec/modules/_test/use_private.test.ori`

**Aliases:**

- [ ] **Implement**: `use './math' { add as plus }` — spec/12-modules.md § Aliases
  - [ ] **Rust Tests**: `ori_parse/src/grammar/` — alias parsing
  - [ ] **Ori Tests**: `tests/spec/modules/_test/use_aliases.test.ori`

- [ ] **Implement**: Module alias `use std.net.http as http` — spec/12-modules.md § Aliases
  - [ ] **Rust Tests**: `ori_parse/src/grammar/` — module alias parsing
  - [ ] **Ori Tests**: `tests/spec/modules/_test/module_alias.test.ori`
  - Note: Parsing and runtime complete; qualified access needs type checker support

---

## 4.3 Visibility

- [ ] **Implement**: `pub` on functions — spec/12-modules.md § Visibility
  - [ ] **Rust Tests**: `ori_parse/src/grammar/` — `pub` keyword parsing
  - [ ] **Ori Tests**: `tests/spec/modules/use_imports.ori`

- [ ] **Implement**: `pub` on types — spec/12-modules.md § Visibility
  - [ ] **Rust Tests**: `ori_parse/src/grammar/` — type visibility parsing
  - [ ] **Ori Tests**: `library/std/prelude.ori` — `pub type Option`, `pub type Result`

- [ ] **Implement**: `pub` on config variables — spec/12-modules.md § Visibility
  - [ ] **Rust Tests**: `ori_parse/src/grammar/` — config visibility parsing
  - [ ] **Ori Tests**: `tests/spec/modules/use_imports.ori`

- [ ] **Implement**: Re-exports `pub use` — spec/12-modules.md § Re-exports
  - [ ] **Rust Tests**: `ori_parse/src/grammar/` — re-export parsing
  - [ ] **Ori Tests**: `tests/spec/modules/reexporter.ori`
  - Note: Parsing complete; full re-export resolution pending

- [ ] **Implement**: Private by default — spec/12-modules.md § Visibility
  - [ ] **Rust Tests**: `oric/src/eval/module/` — visibility enforcement
  - [ ] **Ori Tests**: `tests/spec/modules/_test/use_private.test.ori`

---

## 4.4 Module Resolution

- [ ] **Implement**: File path resolution — spec/12-modules.md § Module Resolution
  - [ ] **Rust Tests**: `oric/src/eval/module/import.rs` — path resolution tests
  - [ ] **Ori Tests**: N/A (tested via Rust unit tests)

- [ ] **Implement**: Module dependency graph — spec/12-modules.md § Dependency Graph
  - [ ] **Rust Tests**: `oric/src/eval/module/import.rs` — `LoadingContext` tests
  - [ ] **Ori Tests**: N/A (tested via Rust unit tests)

- [ ] **Implement**: Cycle detection — spec/12-modules.md § Cycle Detection, proposals/approved/no-circular-imports-proposal.md
  - [ ] **Rust Tests**: `oric/src/eval/module/import.rs` — `test_loading_context_cycle_*`
  - [ ] **Ori Tests**: N/A (tested via Rust unit tests)

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

- [ ] **Implement**: Name resolution — spec/12-modules.md § Name Resolution
  - [ ] **Rust Tests**: `oric/src/eval/module/` — name resolution tests
  - [ ] **Ori Tests**: All import tests verify name resolution

- [ ] **Implement**: Qualified access — spec/12-modules.md § Qualified Access
  - [ ] **Rust Tests**: `oric/src/eval/` — qualified access evaluation
  - [ ] **Ori Tests**: `tests/spec/modules/qualified.ori`
  - [ ] **LLVM Support**: LLVM codegen for qualified access dispatch
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/module_tests.rs` — qualified access codegen
  - Note: Runtime evaluation complete; type checker needs ModuleNamespace support

---

## 4.5 Test Modules

- [ ] **Implement**: `_test/` convention — spec/12-modules.md § Test Modules
  - [ ] **Rust Tests**: `oric/src/eval/module/import.rs` — test module detection
  - [ ] **Ori Tests**: `tests/spec/modules/_test/test_module_access.test.ori`

- [ ] **Implement**: Test files access private items — spec/12-modules.md § Test Modules
  - [ ] **Rust Tests**: `oric/src/eval/module/` — private access rules
  - [ ] **Ori Tests**: `tests/spec/modules/_test/test_module_access.test.ori`

---

## 4.6 Prelude

- [ ] **Implement**: Types: `Option`, `Result`, `Error`, `Ordering` — spec/12-modules.md § Prelude
  - [ ] **Rust Tests**: `oric/src/eval/` — built-in type tests
  - [ ] **Ori Tests**: Option/Result used throughout `tests/spec/`
  - [ ] **LLVM Support**: LLVM codegen for prelude type representations
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/module_tests.rs` — prelude type codegen

- [ ] **Implement**: Built-in functions: `print`, `panic`, `int`, `float`, `str`, `byte` — spec/12-modules.md § Prelude
  - [ ] **Rust Tests**: `oric/src/eval/evaluator/` — `register_prelude()` tests
  - [ ] **Ori Tests**: Built-ins used throughout test suite
  - [ ] **LLVM Support**: LLVM codegen for built-in functions
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/module_tests.rs` — built-in function codegen

- [ ] **Implement**: Built-in methods: `.len()`, `.is_empty()`, `.is_some()`, etc. — Lean Core
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — method dispatch tests
  - [ ] **Ori Tests**: `tests/spec/traits/core/` — method tests
  - [ ] **LLVM Support**: LLVM codegen for built-in methods
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/module_tests.rs` — built-in method codegen

- [ ] **Implement**: Auto-import prelude from `library/std/prelude.ori` — spec/12-modules.md § Prelude
  - [ ] `Evaluator::load_prelude()` auto-loads prelude before any module
  - [ ] All public functions from prelude available without import
  - [ ] **Rust Tests**: `oric/src/eval/evaluator/` — prelude loading tests
  - [ ] **Ori Tests**: `test_autoload.ori` verifies assert_eq, is_some work without import
  - [ ] **LLVM Support**: LLVM codegen for prelude auto-loading
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/module_tests.rs` — prelude loading codegen

- [ ] **Implement**: Prelude functions auto-available
  - [ ] `assert`, `assert_eq`, `assert_ne`, `assert_some`, `assert_none`, `assert_ok`, `assert_err`
  - [ ] `is_some`, `is_none`, `is_ok`, `is_err`
  - [ ] `len`, `is_empty`
  - [ ] `compare`, `min`, `max`
  - [ ] **LLVM Support**: LLVM codegen for prelude functions
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/module_tests.rs` — prelude function codegen
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
- Module alias syntax: `use std.net.http as http` — parsing, runtime, type checker
- Re-exports: `pub use './client' { get, post }` — parsing, full resolution pending
- Qualified access: `module.function()` — runtime, type checker needs ModuleNamespace support

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

- [ ] Core module imports working (relative, module, private, aliases)
- [ ] Visibility system working (`pub`, private by default, `::`)
- [ ] Module resolution working (path resolution, stdlib lookup)
- [ ] Cycle detection working
- [ ] Test module private access working
- [ ] Built-in prelude types and functions working
- [ ] Auto-load stdlib prelude
- [ ] `Self` type parsing in traits
- [ ] Trait/impl parsing at module level
- [ ] Module alias syntax (`use std.net.http as http`) — parsing/runtime complete
- [ ] Re-exports (`pub use`) — parsing complete
- [ ] Qualified access (`module.function()`) — runtime complete, type checker pending
- [ ] Type definitions parsing (see Section 5)
- [ ] Run full test suite: `./test-all.sh`

**Exit Criteria**: Multi-file projects compile (core support complete)
**Status**: Section 4 parsing and runtime complete. Type checker support for module namespaces pending.
