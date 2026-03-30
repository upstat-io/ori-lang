---
section: 4
title: Module System
status: in-progress
last_verified: "2026-03-29"
reviewed: true
tier: 1
goal: Multi-file compilation
spec:
  - spec/18-modules.md
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
    status: in-progress
  - id: "4.12"
    title: Extension Methods
    status: in-progress
---

# Section 4: Module System

**Goal**: Multi-file compilation

> **SPEC**: `spec/18-modules.md`
> **DESIGN**: `design/09-modules/index.md`
> **PROPOSAL**: `proposals/approved/no-circular-imports-proposal.md` — Circular import rejection
> **PROPOSAL**: `proposals/approved/module-system-details-proposal.md` — Entry points, re-export chains, visibility

**Status**: In-progress — Core evaluator complete (4.1-4.6), LLVM multi-file infrastructure present (dependency graph, topological sort, symbol mangling), tooling pending (4.7), module details pending (4.8), constants parser done / evaluator pending (4.11), extension parser + basic evaluator done / type checker pending (4.12). Verified 2026-03-29.

---

## 4.1 Module Definition

- [x] **Implement**: Module structure — spec/18-modules.md § Module Structure [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `oric/src/imports/tests.rs` — 14 tests: path resolution, test module detection, parent module import rules, cycle detection; `ori_eval/src/module_registration/tests.rs` — 9 tests: function/variant/newtype/impl/extend/def_impl registration
  - [x] **Ori Tests**: `tests/spec/modules/use_imports.ori` (13 tests, pub/private functions, types, closures, higher-order functions, config vars)
  - [ ] **LLVM Support**: LLVM codegen for module loading — multi_file.rs infrastructure exists (451 lines, dep graph, topo sort, module-qualified mangling), no integration tests
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/module_tests.rs` — module loading codegen (file does not exist)
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: Module corresponds to file — spec/18-modules.md § Module Structure [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `oric/src/imports/mod.rs::resolve_relative_import_tracked()` — probes `<path>.ori` then `<path>/mod.ori`
  - [x] **Ori Tests**: `tests/spec/modules/_test/directory_module.test.ori` — directory module resolution

- [x] **Implement**: Module name from file path — spec/18-modules.md § Module Structure [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `oric/src/imports/tests.rs` — `generate_relative_candidates_file_module`, `_parent_path`, `_nested_directory`, `_with_extension` (4 tests); `ori_llvm/src/aot/multi_file/tests.rs` — `derive_module_name` (3 tests)
  - [x] **Ori Tests**: N/A (tested via Rust unit tests)

---

## 4.2 Import Parsing

**Relative imports:**

- [x] **Implement**: `use './path' { item1, item2 }` — spec/18-modules.md § Relative Imports [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `oric/src/imports/tests.rs` — path resolution tests
  - [x] **Ori Tests**: `tests/spec/modules/_test/use_imports.test.ori` (4 tests: add, make_multiplier, calculate, double)

- [x] **Implement**: Parent `use '../utils' { helper }` — spec/18-modules.md § Relative Imports [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `oric/src/imports/tests.rs::generate_relative_candidates_parent_path`
  - [x] **Ori Tests**: `tests/spec/modules/_test/use_imports.test.ori` (uses `"../use_imports"`)

- [x] **Implement**: Subdirectory `use './http/client' { get }` — spec/18-modules.md § Relative Imports [done] (verified 2026-03-29) WEAK TESTS
  - [x] **Rust Tests**: `oric/src/imports/tests.rs::generate_relative_candidates_nested_directory`
  - [x] **Ori Tests**: N/A (tested via Rust unit tests only)
  - [ ] **Gap**: Need Ori spec test for subdirectory relative imports (currently Rust unit test only, no end-to-end `.ori` test)

**Module imports:**

- [x] **Implement**: `use std.module { item }` — spec/18-modules.md § Module Imports [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `oric/src/imports/tests.rs::resolve_module_path_not_found` — stdlib path resolution
  - [x] **Ori Tests**: All 4181 test files use `use std.testing { assert_eq }`

- [ ] **Implement**: Nested `use std.net.http { get }` — spec/18-modules.md § Module Imports
  - [ ] **Rust Tests**: Parser supports multi-segment paths (verified via `test_import_without_def_basic`); no runtime test
  - [ ] **Ori Tests**: N/A — no nested stdlib modules exist yet to test

**Private imports:**

- [x] **Implement**: `use './path' { ::private_item }` — spec/18-modules.md § Private Imports [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `oric/tests/phases/parse/imports.rs::test_import_private_basic`, `test_import_private_with_alias`
  - [x] **Ori Tests**: `tests/spec/modules/_test/use_private.test.ori` (2 tests: private fn access, private + public combo)

- [x] **Implement**: `::` prefix — spec/18-modules.md § Private Imports [done] (verified 2026-03-29)
  - [x] **Rust Tests**: Parser tests verify `items[0].is_private` flag set for `::` prefix
  - [x] **Ori Tests**: `tests/spec/modules/_test/use_private.test.ori`

**Aliases:**

- [x] **Implement**: `use './math' { add as plus }` — spec/18-modules.md § Aliases [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_parse/src/grammar/` — alias parsing
  - [x] **Ori Tests**: `tests/spec/modules/_test/use_aliases.test.ori` (3 tests: add as plus, make_multiplier as create_multiplier, double as twice)

- [x] **Implement**: Module alias `use std.net.http as http` — spec/18-modules.md § Aliases [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_parse/src/grammar/` — module alias parsing
  - [x] **Ori Tests**: `tests/spec/modules/_test/module_alias.test.ori` (11 tests: qualified access `math.add()`, `math.subtract()`, `math.multiply()`, `math.square()`, `math.double()`, `math.make_adder()`, `math.use_internal()`)
  - Note: Parsing and runtime complete; qualified access works via evaluator. Type checker ModuleNamespace support pending.

---

## 4.3 Visibility

- [x] **Implement**: `pub` on functions — spec/18-modules.md § Visibility [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_parse/src/grammar/` — `pub` keyword parsing
  - [x] **Ori Tests**: `tests/spec/modules/use_imports.ori` (9 pub functions); `_test/use_imports.test.ori` imports 4 of them

- [x] **Implement**: `pub` on types — spec/18-modules.md § Visibility [done] (verified 2026-03-29) WEAK TESTS
  - [x] **Rust Tests**: `ori_parse/src/grammar/` — type visibility parsing
  - [x] **Ori Tests**: `library/std/prelude.ori` — `pub type Option`, `pub type Result`; `use_imports.ori` has `pub type Point`
  - [ ] **Gap**: No cross-module type import test — `pub type Point` is declared but never imported in a cross-module test

- [ ] **Implement**: `pub` on config variables — spec/18-modules.md § Visibility [partial] INCOMPLETE
  - [x] **Parser**: Config visibility parsing works — 6 parser tests in `oric/tests/phases/parse/imports.rs`
  - [x] **Ori Declaration**: `tests/spec/modules/use_imports.ori` (`pub $default_timeout`, private `$internal_limit`)
  - [ ] **Evaluator**: Constant import resolution NOT implemented — `_test/use_constants.test.ori` is `#skip("constant import resolution not yet implemented")`
  - Note: Parser done, evaluator not done. Previously marked [x] incorrectly — reopened (verified 2026-03-29)

- [x] **Implement**: Re-exports `pub use` — spec/18-modules.md § Re-exports [done] (verified 2026-03-29) WEAK TESTS
  - [x] **Rust Tests**: `ori_parse/src/grammar/` — re-export parsing
  - [x] **Ori Tests**: `tests/spec/modules/reexporter.ori` (`pub use "./math_lib" { add, multiply }`, self-test only)
  - [ ] **Gap**: No cross-module re-export consumption test — `reexporter.ori` tests itself but no file imports from it
  - Note: Basic re-export works; multi-level chain resolution pending (4.8)

- [x] **Implement**: Private by default — spec/18-modules.md § Visibility [done] (verified 2026-03-29) NEEDS PIN
  - [x] **Rust Tests**: `oric/src/imports/tests.rs` — visibility enforcement
  - [x] **Ori Tests**: `tests/spec/modules/_test/use_private.test.ori` (private access with `::` prefix)
  - [ ] **Gap**: No `#compile_fail` test for importing a private item without `::` from a non-test module

---

## 4.4 Module Resolution

- [x] **Implement**: File path resolution — spec/18-modules.md § Module Resolution [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `oric/src/imports/tests.rs` — 4 candidate generation tests (`generate_relative_candidates_*`)
  - [x] **Ori Tests**: `tests/spec/modules/_test/directory_module.test.ori` (2 tests), `_test/precedence.test.ori` (file precedence over dir)

- [x] **Implement**: Module dependency graph — spec/18-modules.md § Dependency Graph [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `oric/src/imports/tests.rs` — `LoadingContext` cycle detection; `ori_llvm/src/aot/incremental/deps/tests.rs` — 12 tests
  - [x] **Ori Tests**: N/A (tested via Rust unit tests)
  - Note: Both eval (via Salsa) and LLVM (explicit `DependencyGraph`) have dependency tracking with tests

- [x] **Implement**: Cycle detection — spec/18-modules.md § Cycle Detection, proposals/approved/no-circular-imports-proposal.md [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `oric/src/imports/tests.rs` — `loading_context_cycle_detection`, `loading_context_cycle_error`; `ori_llvm/src/aot/multi_file/tests.rs::test_graph_build_context_cycle_detection`
  - [x] **Ori Tests**: N/A (tested via Rust unit tests)
  - Note: Both eval and LLVM backends have cycle detection

- [ ] **Implement**: Enhanced cycle error messages — proposals/approved/no-circular-imports-proposal.md § Error Message
  - [ ] Show full cycle path in error (a.ori -> b.ori -> a.ori)
  - [ ] Include actionable help: "extract shared types", "use dependency inversion", "restructure boundaries"
  - [ ] **Rust Tests**: `oric/src/imports/tests.rs` — cycle error formatting tests
  - [ ] **Ori Tests**: `tests/spec/modules/cycle_error_message.ori`

- [ ] **Implement**: Report all cycles (not just first) — proposals/approved/no-circular-imports-proposal.md § Detection Algorithm
  - [ ] Continue detection after finding first cycle
  - [ ] Report each cycle with full path
  - [ ] **Rust Tests**: `oric/src/imports/tests.rs` — multi-cycle detection tests
  - [ ] **Ori Tests**: `tests/spec/modules/multiple_cycles.ori`

- [x] **Implement**: Name resolution — spec/18-modules.md § Name Resolution [done] (verified 2026-03-29)
  - [x] **Rust Tests**: All import test files exercise name resolution
  - [x] **Ori Tests**: All import tests verify name resolution (use_imports, use_private, use_aliases, module_alias)

- [x] **Implement**: Qualified access — spec/18-modules.md § Qualified Access [done] evaluator (verified 2026-03-29)
  - [x] **Rust Tests**: Qualified access evaluation tested via module_alias tests
  - [x] **Ori Tests**: `tests/spec/modules/_test/module_alias.test.ori` (11 tests: `math.add()`, `math.multiply()`, etc.)
  - [ ] **LLVM Support**: LLVM codegen for qualified access dispatch — multi_file.rs has module-qualified mangling (`_ori_<module>$<function>`)
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/module_tests.rs` — qualified access codegen (file does not exist)
  - [ ] **AOT Tests**: No AOT coverage yet
  - Note: Runtime evaluation complete; type checker needs ModuleNamespace support

---

## 4.5 Test Modules

- [x] **Implement**: `_test/` convention — spec/18-modules.md § Test Modules [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `oric/src/imports/tests.rs` — 4 `is_test_module` tests: valid, not_in_test_dir, wrong_extension, nested
  - [x] **Ori Tests**: `tests/spec/modules/_test/test_module_access.test.ori` (2 tests)

- [x] **Implement**: Test files access private items — spec/18-modules.md § Test Modules [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `oric/src/imports/tests.rs` — 3 `is_parent_module_import` tests
  - [x] **Ori Tests**: `tests/spec/modules/_test/test_module_access.test.ori` (accesses private `internal_helper` without `::` prefix)

---

## 4.6 Prelude

- [x] **Implement**: Types: `Option`, `Result`, `Error`, `Ordering` — spec/18-modules.md § Prelude [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_eval/src/interpreter/` — built-in type tests
  - [x] **Ori Tests**: Option/Result used throughout 4181 spec tests, Ordering verified in `tests/spec/types/ordering/`
  - [ ] **LLVM Support**: LLVM codegen for prelude type representations — Option/Result have inline IR in lower_calls.rs
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/module_tests.rs` — prelude type codegen (file does not exist)
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: Built-in functions: `print`, `panic`, `int`, `float`, `str`, `byte` — spec/18-modules.md § Prelude [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `register_prelude()` registers 8 function_vals; print/panic registered separately
  - [x] **Ori Tests**: Built-ins used throughout test suite
  - [x] **LLVM Support**: LLVM codegen for built-in functions — `print` via `_ori_print`, `panic` via `_ori_panic`, conversions via inline IR
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/module_tests.rs` — built-in function codegen (file does not exist)
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: Built-in methods: `.len()`, `.is_empty()`, `.is_some()`, etc. — Lean Core [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_eval/src/methods.rs` — method dispatch tests
  - [x] **Ori Tests**: `tests/spec/traits/core/` — len (14 tests), comparable (58 tests); `tests/spec/types/` — option, result tests
  - [x] **LLVM Support**: LLVM codegen for built-in methods — inline IR in `lower_calls.rs` (len, is_empty, is_some, is_none, unwrap, unwrap_or, is_ok, is_err, compare)
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/module_tests.rs` — built-in method codegen (file does not exist)
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: Auto-import prelude from `library/std/prelude.ori` — spec/18-modules.md § Prelude [done] (verified 2026-03-29)
  - [x] `resolve_imports()` loads prelude via `prelude_candidates()` walk-up search
  - [x] All public functions from prelude available without import
  - [x] **Rust Tests**: `ori_eval/src/interpreter/` — prelude loading tests
  - [x] **Ori Tests**: All 4181 test files use `use std.testing { assert_eq }` which depends on prelude
  - [ ] **LLVM Support**: LLVM codegen for prelude auto-loading
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/module_tests.rs` — prelude loading codegen (file does not exist)
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: Prelude functions auto-available [done] (verified 2026-03-29)
  - [x] `assert`, `assert_eq`, `assert_ne`, `assert_some`, `assert_none`, `assert_ok`, `assert_err`
  - [x] `is_some`, `is_none`, `is_ok`, `is_err`
  - [x] `len`, `is_empty`
  - [x] `compare`, `min`, `max`
  - [ ] **LLVM Support**: LLVM codegen for prelude functions — partial (print, panic, len, compare have IR; assert_* not yet)
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/module_tests.rs` — prelude function codegen (file does not exist)
  - [ ] **AOT Tests**: No AOT coverage yet
  - Note: Trait definitions in prelude (Eq, Comparable, etc.) parse but need Section 3 for full integration

---

## 4.7 Import Graph Tooling

> **PROPOSAL**: `proposals/approved/no-circular-imports-proposal.md § Tooling Support`

- [ ] **Implement**: `ori check --cycles` — Check for cycles without full compilation
  - [ ] Fast path: parse imports only, build graph, detect cycles
  - [ ] **Rust Tests**: `oric/src/commands/` — cycle checking tests
  - [ ] **Ori Tests**: `tests/cli/check_cycles.ori`

- [ ] **Implement**: `ori graph --imports` — Visualize import graph
  - [ ] Output DOT format for graphviz
  - [ ] Usage: `ori graph --imports > imports.dot && dot -Tpng imports.dot -o imports.png`
  - [ ] **Rust Tests**: `oric/src/commands/` — graph output tests
  - [ ] **Ori Tests**: `tests/cli/graph_imports.ori`

---

## 4.8 Module System Details

> **PROPOSAL**: `proposals/approved/module-system-details-proposal.md`

### Entry Point Files

- [ ] **Implement**: `lib.ori` as library entry point — spec/18-modules.md § Entry Point Files
  - [ ] **Rust Tests**: `ori_eval/src/interpreter/module/` — library entry detection
  - [ ] **Ori Tests**: `tests/spec/modules/library_entry.ori`

- [ ] **Implement**: Distinguish `lib.ori` vs `mod.ori` — spec/18-modules.md § Entry Point Files
  - [ ] Package root requires `lib.ori`, not `mod.ori`
  - [ ] **Rust Tests**: `ori_eval/src/interpreter/module/` — entry point validation
  - [ ] **Ori Tests**: `tests/spec/modules/entry_point_validation.ori`

### Binary-Library Separation

- [ ] **Implement**: Binary accesses library via public API only — spec/18-modules.md § Library + Binary
  - [ ] `use "my_pkg" { item }` accesses `lib.ori` exports
  - [ ] `use "my_pkg" { ::private }` is an error (no private access)
  - [ ] **Rust Tests**: `ori_eval/src/interpreter/module/` — binary-library access tests
  - [ ] **Ori Tests**: `tests/spec/modules/binary_library_access.ori`

### Re-export Chains

- [ ] **Implement**: Multi-level re-export resolution — spec/18-modules.md § Re-export Chains
  - [ ] Track visibility through chain (all levels must be `pub`)
  - [ ] Aliases propagate through chains
  - [ ] **Rust Tests**: `ori_eval/src/interpreter/module/` — re-export chain tests
  - [ ] **Ori Tests**: `tests/spec/modules/reexport_chain.ori`

- [ ] **Implement**: Diamond re-exports — spec/18-modules.md § Re-export Chains
  - [ ] Same item via multiple paths is not an error
  - [ ] **Rust Tests**: `ori_eval/src/interpreter/module/` — diamond import tests
  - [ ] **Ori Tests**: `tests/spec/modules/diamond_reexport.ori`

### Error Messages

- [ ] **Implement**: E1101 (missing module) — proposals/approved/module-system-details-proposal.md § Error Messages
  - [ ] Show paths checked: `file.ori`, `file/mod.ori`
  - [ ] **Rust Tests**: `ori_diagnostic/src/` — error formatting tests
  - [ ] **Ori Tests**: `tests/spec/modules/error_missing_module.ori`

- [ ] **Implement**: E1102 (missing export) — proposals/approved/module-system-details-proposal.md § Error Messages
  - [ ] Show available exports in error message
  - [ ] "Did you mean?" suggestion
  - [ ] **Rust Tests**: `ori_diagnostic/src/` — error formatting tests
  - [ ] **Ori Tests**: `tests/spec/modules/error_missing_export.ori`

- [ ] **Implement**: E1103 (private item) — proposals/approved/module-system-details-proposal.md § Error Messages
  - [ ] Help text: "use `::item` for explicit private access"
  - [ ] **Rust Tests**: `ori_diagnostic/src/` — error formatting tests
  - [ ] **Ori Tests**: `tests/spec/modules/error_private_item.ori`

---

## 4.9 Remaining Work (Pre-existing)

**Parsing/Runtime complete, type checker pending:**
- [x] Module alias syntax: `use "../math_lib" as math` — parsing [done], runtime [done] (verified 2026-03-29, 11 tests in module_alias.test.ori)
- [x] Re-exports: `pub use './client' { get, post }` — basic parsing [done], basic resolution [done] (verified 2026-03-29, reexporter.ori self-test only) WEAK TESTS
  - [ ] **Gap**: No cross-module re-export consumption test
- [x] Qualified access: `module.function()` — runtime [done] (verified 2026-03-29, 11 tests in module_alias.test.ori)
- [ ] Type checker ModuleNamespace support — pending
- [ ] Multi-level re-export chain resolution — pending (4.8)
- [ ] Nested stdlib modules (`std.net.http`) — no modules to test yet

---

## 4.11 Module-Level Constants

**Source**: `grammar.ebnf § constant_decl`, `spec/12-constants.md`

Module-level constants declared with `let $NAME = value`.

```ori
let $PI = 3.14159
let $MAX_SIZE: int = 1000
pub let $VERSION = "1.0.0"
```

**Status**: Parser complete (verified 2026-03-29), evaluator incomplete.

### Parser

- [x] **Implement**: Parse `let $NAME = value` — `constant_decl` production [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `compiler/ori_parse/src/grammar/item/config/mod.rs` — `parse_const()` implementation; `oric/tests/phases/parse/imports.rs` — `test_import_constant_basic`, `_multiple`, `_mixed_with_regular`, `_mixed_with_private` (6 tests)
  - [x] **Ori Tests**: `tests/spec/modules/use_imports.ori` — declares `pub $default_timeout = 30` and `$internal_limit = 100`

- [x] **Implement**: Parse typed constants `let $NAME: Type = value` [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `oric/tests/phases/parse/imports.rs` — constant parsing tests cover typed forms

- [x] **Implement**: Parse public constants `pub let $NAME = value` [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `oric/tests/phases/parse/imports.rs` — visibility parsing for constants

### Evaluator

- [ ] **Implement**: Evaluate module-level constants at load time
  - [ ] **Rust Tests**: `ori_eval/src/interpreter/mod.rs` — constant evaluation
  - [ ] **Ori Tests**: `tests/spec/declarations/constants_eval.ori`
  - [ ] **LLVM Support**: LLVM codegen for module constants
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/constant_tests.rs`
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Register constants in module namespace
  - [ ] **Rust Tests**: `ori_eval/src/interpreter/module_loading.rs` — constant registration
  - [ ] **Ori Tests**: `tests/spec/modules/import_constants.ori`

### Type Checker

- [ ] **Implement**: Type check constant initializers
  - [ ] **Rust Tests**: `ori_types/src/check/` — constant type checking
  - [ ] **Ori Tests**: `tests/spec/types/constant_types.ori`

- [ ] **Implement**: Enforce constant expression restrictions (no function calls with side effects)
  - [ ] **Rust Tests**: `ori_types/src/check/` — constant expression validation
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

**Status**: Parser and basic evaluator complete (verified 2026-03-29). Type checker integration, conflict detection, orphan rules, error codes (E0850-E0852), LLVM codegen pending.

### Extension Definition

- [x] **Implement**: `extend Type { @method (self) -> T = ... }` — proposals/approved/extension-methods-proposal.md § Extension Definition [partial] (verified 2026-03-29)
  - [x] Parse `extend` blocks — `compiler/ori_parse/src/grammar/item/extend.rs`
  - [x] Register extension methods in type environment — `ori_eval/src/module_registration/tests.rs` has `test_collect_extend_methods` and `test_collect_extend_methods_with_config` (2 tests)
  - [x] **Rust Tests**: `oric/tests/phases/parse/extensions.rs` — 12 parser tests: extend with where clause, multiple bounds, multiple methods, extension import variants
  - [x] **Ori Tests**: `tests/spec/extensions/list_methods.ori` — end-to-end test with `extend str { @shout, @whisper }` and `extend [T] { @count, @empty }`
  - [ ] **Type Checker**: Type checker integration for extension method resolution — not implemented
  - [ ] **Conflict Detection**: Conflict detection for ambiguous extension methods — not implemented

- [ ] **Implement**: Constrained extensions with angle brackets — proposals/approved/extension-methods-proposal.md § Constrained Extensions
  - [x] Parse `extend<T: Clone> [T] { ... }` syntax — parser supports this (verified 2026-03-29)
  - [ ] Type checker constraint enforcement
  - [ ] **Ori Tests**: `tests/spec/extensions/constrained.ori`

- [ ] **Implement**: Constrained extensions with where clause — proposals/approved/extension-methods-proposal.md § Constrained Extensions
  - [x] Parse `extend [T] where T: Clone { ... }` syntax — parser supports this (verified 2026-03-29)
  - [ ] Type checker where clause enforcement
  - [ ] **Ori Tests**: `tests/spec/extensions/constrained_where.ori`

- [ ] **Implement**: Extension visibility — proposals/approved/extension-methods-proposal.md § Visibility
  - [ ] `pub extend` makes all methods public
  - [ ] Non-pub `extend` is module-private
  - [ ] Block-level visibility only (no per-method pub)
  - [ ] **Rust Tests**: `ori_eval/src/interpreter/module/` — visibility tests
  - [ ] **Ori Tests**: `tests/spec/extensions/visibility.ori`

- [ ] **Implement**: Extension restrictions — proposals/approved/extension-methods-proposal.md § What Can Be Extended
  - [ ] Error on field addition attempt
  - [ ] Error on trait implementation in extend block
  - [ ] Error on override of existing method
  - [ ] Error on static method (no self)
  - [ ] **Rust Tests**: `ori_diagnostic/src/` — restriction error tests
  - [ ] **Ori Tests**: `tests/spec/extensions/restrictions.ori`

### Extension Import

- [x] **Implement**: `extension "path" { Type.method }` — proposals/approved/extension-methods-proposal.md § Extension Import [partial] (verified 2026-03-29)
  - [x] Parse `extension` import syntax — `compiler/ori_parse/src/grammar/item/extension_import.rs`
  - [x] Method-level granularity — parser enforces method-level granularity
  - [x] **Rust Tests**: `oric/tests/phases/parse/extensions.rs` — extension import tests: basic, multiple items, relative path, public, private, with regular imports, multiple types, missing dot error
  - [ ] **Ori Tests**: `tests/spec/extensions/import.ori` — end-to-end extension import test (not yet created)
  - [ ] **Runtime**: Extension import resolution in evaluator — not implemented

- [ ] **Implement**: Wildcard prohibition — proposals/approved/extension-methods-proposal.md § Import Syntax
  - [ ] Error on `extension "path" { Type.* }`
  - [ ] **Rust Tests**: `ori_diagnostic/src/` — wildcard error tests
  - [ ] **Ori Tests**: `tests/spec/extensions/no_wildcard.ori`

- [ ] **Implement**: Re-export extensions — proposals/approved/extension-methods-proposal.md § Scoping
  - [ ] `pub extension "path" { Type.method }` for re-export
  - [ ] No transitive propagation without explicit re-export
  - [ ] **Rust Tests**: `ori_eval/src/interpreter/module/` — re-export tests
  - [ ] **Ori Tests**: `tests/spec/extensions/reexport.ori`

### Method Resolution

- [ ] **Implement**: Resolution order — proposals/approved/extension-methods-proposal.md § Resolution Order
  - [ ] Inherent > Trait > Extension
  - [ ] **Rust Tests**: `ori_types/src/check/` — resolution order tests
  - [ ] **Ori Tests**: `tests/spec/extensions/resolution_order.ori`

- [ ] **Implement**: Conflict detection — proposals/approved/extension-methods-proposal.md § Conflict Resolution
  - [ ] Error on ambiguous extension methods
  - [ ] Qualified syntax for disambiguation: `module.Type.method(v)`
  - [ ] **Rust Tests**: `ori_types/src/check/` — conflict detection tests
  - [ ] **Ori Tests**: `tests/spec/extensions/conflict.ori`

### Orphan Rules

- [ ] **Implement**: Same-package rule — proposals/approved/extension-methods-proposal.md § Orphan Rules
  - [ ] Extension must be in same package as type OR trait bound
  - [ ] Error for foreign type without local trait bound
  - [ ] **Rust Tests**: `ori_types/src/check/` — orphan rule tests
  - [ ] **Ori Tests**: `tests/spec/extensions/orphan.ori`

### Error Messages

- [ ] **Implement**: E0850 (ambiguous extension) — proposals/approved/extension-methods-proposal.md § Error Messages
  - [ ] Show all candidate extensions
  - [ ] Help text for qualified syntax
  - [ ] **Rust Tests**: `ori_diagnostic/src/` — error formatting tests
  - [ ] **Ori Tests**: `tests/spec/extensions/error_ambiguous.ori`

- [ ] **Implement**: E0851 (method not found) — proposals/approved/extension-methods-proposal.md § Error Messages
  - [ ] Suggest extension import if method exists in known module
  - [ ] **Rust Tests**: `ori_diagnostic/src/` — error formatting tests
  - [ ] **Ori Tests**: `tests/spec/extensions/error_not_found.ori`

- [ ] **Implement**: E0852 (orphan violation) — proposals/approved/extension-methods-proposal.md § Error Messages
  - [ ] Show package location of foreign type
  - [ ] Help: "define a newtype wrapper or use a local trait bound"
  - [ ] **Rust Tests**: `ori_diagnostic/src/` — error formatting tests
  - [ ] **Ori Tests**: `tests/spec/extensions/error_orphan.ori`

### LLVM Support

- [ ] **Implement**: Extension method codegen — Extension methods in LLVM backend
  - [ ] Same codegen as regular methods
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/extension_tests.rs`
  - [ ] **AOT Tests**: No AOT coverage yet

**Note on Type Definitions:**
- Full prelude with user-defined Option, Result, etc. requires Section 5 (Type Declarations)
- Currently using built-in types in evaluator
- See section-05-type-declarations.md § 5.1-5.4 for type definition work

---

## 4.10 Section Completion Checklist

- [x] Core module imports working (relative, module, private, aliases) [done] (verified 2026-03-29)
- [x] Visibility system working (`pub`, private by default, `::`) [done] (verified 2026-03-29) — note: `pub` on config vars parser-only, evaluator incomplete
- [x] Module resolution working (path resolution, stdlib lookup, directory modules, file precedence) [done] (verified 2026-03-29)
- [x] Cycle detection working (Rust unit tests: `loading_context_cycle_*` in `oric/src/imports/tests.rs`) [done] (verified 2026-03-29)
- [x] Test module private access working (`_test/` convention, `test_module_access.test.ori`) [done] (verified 2026-03-29)
- [x] Built-in prelude types and functions working (Option, Result, Ordering, print, panic, etc.) [done] (verified 2026-03-29)
- [x] Auto-load stdlib prelude (`use std.testing` works in all 4181 test files) [done] (verified 2026-03-29)
- [x] `Self` type parsing in traits (see Section 3) [done] (verified 2026-03-29)
- [x] Trait/impl parsing at module level (see Section 3) [done] (verified 2026-03-29)
- [x] Module alias syntax (`use "../path" as alias`) — parsing/runtime complete [done] (verified 2026-03-29)
- [x] Re-exports (`pub use`) — basic parsing/resolution complete [done] (verified 2026-03-29) WEAK TESTS — self-test only, no cross-module consumption
- [x] Qualified access (`module.function()`) — runtime complete [done] (verified 2026-03-29)
- [ ] Type checker ModuleNamespace support — pending
- [ ] LLVM multi-file AOT compilation — infrastructure exists (multi_file.rs, 15 unit tests), no integration tests
- [ ] Enhanced cycle error messages (4.4) — pending
- [ ] Type definitions parsing (see Section 5)
- [ ] Module system negative tests — zero `#compile_fail` tests exist for the module system
- [ ] Cross-module type import test — `pub type Point` declared but never imported cross-module
- [ ] Cross-module re-export consumption test — `reexporter.ori` tests itself but nobody imports from it
- [ ] `oric/src/imports/mod.rs` file split — 580 lines, exceeds 500-line limit
- [ ] Run full test suite: `./test-all.sh`
- [ ] `/tpr-review` passed — independent Codex review found no critical or major issues (or all findings triaged)

**Exit Criteria**: Multi-file projects compile (core support complete)
**Status**: Section 4 evaluator and parser complete. 27 of 35 [x] items fully verified, 4 have weak tests, 1 incomplete (config var imports reopened), 1 needs negative pin. LLVM multi-file infrastructure present (15 unit tests, zero integration tests). Extension parser and basic evaluator ahead of roadmap. Constant parser ahead of roadmap. Verified 2026-03-29.
