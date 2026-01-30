# Phase 4: Module System

**Goal**: Multi-file compilation

> **SPEC**: `spec/12-modules.md`
> **DESIGN**: `design/09-modules/index.md`
> **PROPOSAL**: `proposals/approved/no-circular-imports-proposal.md` — Circular import rejection

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

## 4.8 Remaining Work

**Parsing/Runtime complete, type checker pending:**
- Module alias syntax: `use std.net.http as http` — parsing ✅, runtime ✅, type checker ❌
- Re-exports: `pub use './client' { get, post }` — parsing ✅, full resolution pending
- Qualified access: `module.function()` — runtime ✅, type checker needs ModuleNamespace support

**Nice to have (lower priority):**
- Extension imports: `extension std.iter.extensions { Iterator.count }`

**Note on Type Definitions:**
- Full prelude with user-defined Option, Result, etc. requires Phase 5 (Type Declarations)
- Currently using built-in types in evaluator
- See phase-05-type-declarations.md § 5.1-5.4 for type definition work

---

## 4.9 Phase Completion Checklist

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
