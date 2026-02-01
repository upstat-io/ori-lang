---
phase: 4
title: Module System
status: in-progress
tier: 1
goal: Multi-file compilation
spec:
  - spec/12-modules.md
sections:
  - id: "4.1"
    title: Module Definition
    status: complete
  - id: "4.2"
    title: Import Parsing
    status: complete
  - id: "4.3"
    title: Visibility
    status: complete
  - id: "4.4"
    title: Module Resolution
    status: in-progress
  - id: "4.5"
    title: Test Modules
    status: complete
  - id: "4.6"
    title: Prelude
    status: complete
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
    title: Phase Completion Checklist
    status: complete
  - id: "4.11"
    title: Extension Methods
    status: not-started
---

# Phase 4: Module System

**Goal**: Multi-file compilation

> **SPEC**: `spec/12-modules.md`
> **DESIGN**: `design/09-modules/index.md`
> **PROPOSAL**: `proposals/approved/no-circular-imports-proposal.md` — Circular import rejection
> **PROPOSAL**: `proposals/approved/module-system-details-proposal.md` — Entry points, re-export chains, visibility

**Status**: 🔶 Partial — Core complete (4.1-4.6, 4.10), tooling pending (4.7), module details pending (4.8), extension methods pending (4.11)

---

## 4.1 Module Definition

- [x] **Implement**: Module structure — spec/12-modules.md § Module Structure
  - [x] **Rust Tests**: `oric/src/eval/module/import.rs` — module loading tests
  - [x] **Ori Tests**: `tests/spec/modules/use_imports.ori`

- [x] **Implement**: Module corresponds to file — spec/12-modules.md § Module Structure
  - [x] **Rust Tests**: `oric/src/eval/module/import.rs` — file mapping tests
  - [x] **Ori Tests**: `tests/spec/modules/use_imports.ori`

- [x] **Implement**: Module name from file path — spec/12-modules.md § Module Structure
  - [x] **Rust Tests**: `oric/src/eval/module/import.rs` — path resolution tests
  - [x] **Ori Tests**: N/A (tested via Rust unit tests)

---

## 4.2 Import Parsing

**Relative imports:**

- [x] **Implement**: `use './path' { item1, item2 }` — spec/12-modules.md § Relative Imports
  - [x] **Rust Tests**: `oric/src/eval/module/import.rs` — relative path parsing
  - [x] **Ori Tests**: `tests/spec/modules/_test/use_imports.test.ori`

- [x] **Implement**: Parent `use '../utils' { helper }` — spec/12-modules.md § Relative Imports
  - [x] **Rust Tests**: `oric/src/eval/module/import.rs` — parent path resolution
  - [x] **Ori Tests**: `tests/spec/modules/_test/use_imports.test.ori`

- [x] **Implement**: Subdirectory `use './http/client' { get }` — spec/12-modules.md § Relative Imports
  - [x] **Rust Tests**: `oric/src/eval/module/import.rs` — subdirectory path resolution
  - [x] **Ori Tests**: N/A (tested via Rust unit tests)

**Module imports:**

- [x] **Implement**: `use std.module { item }` — spec/12-modules.md § Module Imports
  - [x] **Rust Tests**: `oric/src/eval/module/import.rs` — stdlib path resolution
  - [x] **Ori Tests**: All test files use `use std.testing { assert_eq }`

- [x] **Implement**: Nested `use std.net.http { get }` — spec/12-modules.md § Module Imports
  - [x] **Rust Tests**: `oric/src/eval/module/import.rs` — nested module resolution
  - [x] **Ori Tests**: N/A (tested via Rust unit tests)

**Private imports:**

- [x] **Implement**: `use './path' { ::private_item }` — spec/12-modules.md § Private Imports
  - [x] **Rust Tests**: `oric/src/eval/module/import.rs` — private import handling
  - [x] **Ori Tests**: `tests/spec/modules/_test/use_private.test.ori`

- [x] **Implement**: `::` prefix — spec/12-modules.md § Private Imports
  - [x] **Rust Tests**: `ori_parse/src/grammar/` — `::` prefix parsing
  - [x] **Ori Tests**: `tests/spec/modules/_test/use_private.test.ori`

**Aliases:**

- [x] **Implement**: `use './math' { add as plus }` — spec/12-modules.md § Aliases
  - [x] **Rust Tests**: `ori_parse/src/grammar/` — alias parsing
  - [x] **Ori Tests**: `tests/spec/modules/_test/use_aliases.test.ori`

- [x] **Implement**: Module alias `use std.net.http as http` — spec/12-modules.md § Aliases
  - [x] **Rust Tests**: `ori_parse/src/grammar/` — module alias parsing
  - [x] **Ori Tests**: `tests/spec/modules/_test/module_alias.test.ori`
  - Note: Parsing and runtime complete; qualified access needs type checker support

---

## 4.3 Visibility

- [x] **Implement**: `pub` on functions — spec/12-modules.md § Visibility
  - [x] **Rust Tests**: `ori_parse/src/grammar/` — `pub` keyword parsing
  - [x] **Ori Tests**: `tests/spec/modules/use_imports.ori`

- [x] **Implement**: `pub` on types — spec/12-modules.md § Visibility
  - [x] **Rust Tests**: `ori_parse/src/grammar/` — type visibility parsing
  - [x] **Ori Tests**: `library/std/prelude.ori` — `pub type Option`, `pub type Result`

- [x] **Implement**: `pub` on config variables — spec/12-modules.md § Visibility
  - [x] **Rust Tests**: `ori_parse/src/grammar/` — config visibility parsing
  - [x] **Ori Tests**: `tests/spec/modules/use_imports.ori`

- [x] **Implement**: Re-exports `pub use` — spec/12-modules.md § Re-exports
  - [x] **Rust Tests**: `ori_parse/src/grammar/` — re-export parsing
  - [x] **Ori Tests**: `tests/spec/modules/reexporter.ori`
  - Note: Parsing complete; full re-export resolution pending

- [x] **Implement**: Private by default — spec/12-modules.md § Visibility
  - [x] **Rust Tests**: `oric/src/eval/module/` — visibility enforcement
  - [x] **Ori Tests**: `tests/spec/modules/_test/use_private.test.ori`

---

## 4.4 Module Resolution

- [x] **Implement**: File path resolution — spec/12-modules.md § Module Resolution
  - [x] **Rust Tests**: `oric/src/eval/module/import.rs` — path resolution tests
  - [x] **Ori Tests**: N/A (tested via Rust unit tests)

- [x] **Implement**: Module dependency graph — spec/12-modules.md § Dependency Graph
  - [x] **Rust Tests**: `oric/src/eval/module/import.rs` — `LoadingContext` tests
  - [x] **Ori Tests**: N/A (tested via Rust unit tests)

- [x] **Implement**: Cycle detection — spec/12-modules.md § Cycle Detection, proposals/approved/no-circular-imports-proposal.md
  - [x] **Rust Tests**: `oric/src/eval/module/import.rs` — `test_loading_context_cycle_*`
  - [x] **Ori Tests**: N/A (tested via Rust unit tests)

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

- [x] **Implement**: Name resolution — spec/12-modules.md § Name Resolution
  - [x] **Rust Tests**: `oric/src/eval/module/` — name resolution tests
  - [x] **Ori Tests**: All import tests verify name resolution

- [x] **Implement**: Qualified access — spec/12-modules.md § Qualified Access
  - [x] **Rust Tests**: `oric/src/eval/` — qualified access evaluation
  - [ ] **Ori Tests**: `tests/spec/modules/qualified.ori`
  - Note: Runtime evaluation complete; type checker needs ModuleNamespace support

---

## 4.5 Test Modules

- [x] **Implement**: `_test/` convention — spec/12-modules.md § Test Modules
  - [x] **Rust Tests**: `oric/src/eval/module/import.rs` — test module detection
  - [x] **Ori Tests**: `tests/spec/modules/_test/test_module_access.test.ori`

- [x] **Implement**: Test files access private items — spec/12-modules.md § Test Modules
  - [x] **Rust Tests**: `oric/src/eval/module/` — private access rules
  - [x] **Ori Tests**: `tests/spec/modules/_test/test_module_access.test.ori`

---

## 4.6 Prelude

- [x] **Implement**: Types: `Option`, `Result`, `Error`, `Ordering` — spec/12-modules.md § Prelude
  - [x] **Rust Tests**: `oric/src/eval/` — built-in type tests
  - [x] **Ori Tests**: Option/Result used throughout `tests/spec/`

- [x] **Implement**: Built-in functions: `print`, `panic`, `int`, `float`, `str`, `byte` — spec/12-modules.md § Prelude
  - [x] **Rust Tests**: `oric/src/eval/evaluator/` — `register_prelude()` tests
  - [x] **Ori Tests**: Built-ins used throughout test suite

- [x] **Implement**: Built-in methods: `.len()`, `.is_empty()`, `.is_some()`, etc. — Lean Core
  - [x] **Rust Tests**: `ori_eval/src/methods.rs` — method dispatch tests
  - [x] **Ori Tests**: `tests/spec/traits/core/` — method tests

- [x] **Implement**: Auto-import prelude from `library/std/prelude.ori` — spec/12-modules.md § Prelude
  - [x] `Evaluator::load_prelude()` auto-loads prelude before any module
  - [x] All public functions from prelude available without import
  - [x] **Rust Tests**: `oric/src/eval/evaluator/` — prelude loading tests
  - [x] **Ori Tests**: `test_autoload.ori` verifies assert_eq, is_some work without import

- [x] **Implement**: Prelude functions auto-available
  - [x] `assert`, `assert_eq`, `assert_ne`, `assert_some`, `assert_none`, `assert_ok`, `assert_err`
  - [x] `is_some`, `is_none`, `is_ok`, `is_err`
  - [x] `len`, `is_empty`
  - [x] `compare`, `min`, `max`
  - Note: Trait definitions in prelude (Eq, Comparable, etc.) parse but need Phase 3 for full integration

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
- Module alias syntax: `use std.net.http as http` — parsing ✅, runtime ✅, type checker ❌
- Re-exports: `pub use './client' { get, post }` — parsing ✅, full resolution pending
- Qualified access: `module.function()` — runtime ✅, type checker needs ModuleNamespace support

---

## 4.11 Extension Methods

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
- Full prelude with user-defined Option, Result, etc. requires Phase 5 (Type Declarations)
- Currently using built-in types in evaluator
- See phase-05-type-declarations.md § 5.1-5.4 for type definition work

---

## 4.10 Phase Completion Checklist

- [x] Core module imports working (relative, module, private, aliases)
- [x] Visibility system working (`pub`, private by default, `::`)
- [x] Module resolution working (path resolution, stdlib lookup)
- [x] Cycle detection working
- [x] Test module private access working
- [x] Built-in prelude types and functions working
- [x] Auto-load stdlib prelude ✅
- [x] `Self` type parsing in traits
- [x] Trait/impl parsing at module level
- [x] Module alias syntax (`use std.net.http as http`) — parsing/runtime complete
- [x] Re-exports (`pub use`) — parsing complete
- [x] Qualified access (`module.function()`) — runtime complete, type checker pending
- [ ] Type definitions parsing (see Phase 5)
- [x] Run full test suite: `./test-all`

**Exit Criteria**: Multi-file projects compile ✅ (core support complete)
**Status**: Phase 4 parsing and runtime complete. Type checker support for module namespaces pending.
