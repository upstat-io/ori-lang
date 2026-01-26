# Phase 4: Module System

**Goal**: Multi-file compilation

> **SPEC**: `spec/12-modules.md`
> **DESIGN**: `design/09-modules/index.md`
> **PROPOSAL**: `proposals/approved/no-circular-imports-proposal.md` — Circular import rejection

---

## CURRENT STATUS

**Infrastructure Complete:**
- [x] Parser: `use` statements (relative and module paths)
- [x] IR: `UseDef`, `UseItem`, `ImportPath` (Relative/Module variants)
- [x] Import resolution: `eval/module/import.rs` with path resolution
- [x] Loading context: cycle detection, test module detection
- [x] Visibility: `pub` keyword, `::` prefix for private access
- [x] Aliases: `as` keyword for renaming imports

**What Works Now:**
- Relative imports: `use './path' { item }`, `use '../path' { item }`
- Module imports: `use std.testing { assert_eq }`
- Private imports: `use './mod' { ::private_item }`
- Aliases: `use './math' { add as plus }`
- Test modules in `_test/` can access private items from parent module
- Module resolution walks up directory tree to find `library/`

**Tests:**
- `tests/spec/modules/use_imports.si` - module being imported
- `tests/spec/modules/_test/use_imports.test.si` - relative imports
- `tests/spec/modules/_test/use_aliases.test.si` - alias imports
- `tests/spec/modules/_test/use_private.test.si` - private imports with ::
- `tests/spec/modules/_test/test_module_access.test.si` - test module private access

---

## 4.1 Module Definition

- [x] **Implement**: Module structure — spec/12-modules.md § Module Structure
  - [x] **Rust Tests**: `sigilc/src/eval/module/import.rs` — module loading tests
  - [x] **Sigil Tests**: `tests/spec/modules/use_imports.si`

- [x] **Implement**: Module corresponds to file — spec/12-modules.md § Module Structure
  - [x] **Rust Tests**: `sigilc/src/eval/module/import.rs` — file mapping tests
  - [x] **Sigil Tests**: `tests/spec/modules/use_imports.si`

- [x] **Implement**: Module name from file path — spec/12-modules.md § Module Structure
  - [x] **Rust Tests**: `sigilc/src/eval/module/import.rs` — path resolution tests
  - [x] **Sigil Tests**: N/A (tested via Rust unit tests)

---

## 4.2 Import Parsing

**Relative imports:**

- [x] **Implement**: `use './path' { item1, item2 }` — spec/12-modules.md § Relative Imports
  - [x] **Rust Tests**: `sigilc/src/eval/module/import.rs` — relative path parsing
  - [x] **Sigil Tests**: `tests/spec/modules/_test/use_imports.test.si`

- [x] **Implement**: Parent `use '../utils' { helper }` — spec/12-modules.md § Relative Imports
  - [x] **Rust Tests**: `sigilc/src/eval/module/import.rs` — parent path resolution
  - [x] **Sigil Tests**: `tests/spec/modules/_test/use_imports.test.si`

- [x] **Implement**: Subdirectory `use './http/client' { get }` — spec/12-modules.md § Relative Imports
  - [x] **Rust Tests**: `sigilc/src/eval/module/import.rs` — subdirectory path resolution
  - [x] **Sigil Tests**: N/A (tested via Rust unit tests)

**Module imports:**

- [x] **Implement**: `use std.module { item }` — spec/12-modules.md § Module Imports
  - [x] **Rust Tests**: `sigilc/src/eval/module/import.rs` — stdlib path resolution
  - [x] **Sigil Tests**: All test files use `use std.testing { assert_eq }`

- [x] **Implement**: Nested `use std.net.http { get }` — spec/12-modules.md § Module Imports
  - [x] **Rust Tests**: `sigilc/src/eval/module/import.rs` — nested module resolution
  - [x] **Sigil Tests**: N/A (tested via Rust unit tests)

**Private imports:**

- [x] **Implement**: `use './path' { ::private_item }` — spec/12-modules.md § Private Imports
  - [x] **Rust Tests**: `sigilc/src/eval/module/import.rs` — private import handling
  - [x] **Sigil Tests**: `tests/spec/modules/_test/use_private.test.si`

- [x] **Implement**: `::` prefix — spec/12-modules.md § Private Imports
  - [x] **Rust Tests**: `sigil_parse/src/grammar/` — `::` prefix parsing
  - [x] **Sigil Tests**: `tests/spec/modules/_test/use_private.test.si`

**Aliases:**

- [x] **Implement**: `use './math' { add as plus }` — spec/12-modules.md § Aliases
  - [x] **Rust Tests**: `sigil_parse/src/grammar/` — alias parsing
  - [x] **Sigil Tests**: `tests/spec/modules/_test/use_aliases.test.si`

- [ ] **Implement**: Module alias `use std.net.http as http` — spec/12-modules.md § Aliases
  - [ ] **Rust Tests**: `sigil_parse/src/grammar/` — module alias parsing
  - [ ] **Sigil Tests**: `tests/spec/modules/module_alias.si`

---

## 4.3 Visibility

- [x] **Implement**: `pub` on functions — spec/12-modules.md § Visibility
  - [x] **Rust Tests**: `sigil_parse/src/grammar/` — `pub` keyword parsing
  - [x] **Sigil Tests**: `tests/spec/modules/use_imports.si`

- [x] **Implement**: `pub` on types — spec/12-modules.md § Visibility
  - [x] **Rust Tests**: `sigil_parse/src/grammar/` — type visibility parsing
  - [x] **Sigil Tests**: `library/std/prelude.si` — `pub type Option`, `pub type Result`

- [x] **Implement**: `pub` on config variables — spec/12-modules.md § Visibility
  - [x] **Rust Tests**: `sigil_parse/src/grammar/` — config visibility parsing
  - [x] **Sigil Tests**: `tests/spec/modules/use_imports.si`

- [ ] **Implement**: Re-exports `pub use` — spec/12-modules.md § Re-exports
  - [ ] **Rust Tests**: `sigil_parse/src/grammar/` — re-export parsing
  - [ ] **Sigil Tests**: `tests/spec/modules/reexports.si`

- [x] **Implement**: Private by default — spec/12-modules.md § Visibility
  - [x] **Rust Tests**: `sigilc/src/eval/module/` — visibility enforcement
  - [x] **Sigil Tests**: `tests/spec/modules/_test/use_private.test.si`

---

## 4.4 Module Resolution

- [x] **Implement**: File path resolution — spec/12-modules.md § Module Resolution
  - [x] **Rust Tests**: `sigilc/src/eval/module/import.rs` — path resolution tests
  - [x] **Sigil Tests**: N/A (tested via Rust unit tests)

- [x] **Implement**: Module dependency graph — spec/12-modules.md § Dependency Graph
  - [x] **Rust Tests**: `sigilc/src/eval/module/import.rs` — `LoadingContext` tests
  - [x] **Sigil Tests**: N/A (tested via Rust unit tests)

- [x] **Implement**: Cycle detection — spec/12-modules.md § Cycle Detection, proposals/approved/no-circular-imports-proposal.md
  - [x] **Rust Tests**: `sigilc/src/eval/module/import.rs` — `test_loading_context_cycle_*`
  - [x] **Sigil Tests**: N/A (tested via Rust unit tests)

- [ ] **Implement**: Enhanced cycle error messages — proposals/approved/no-circular-imports-proposal.md § Error Message
  - [ ] Show full cycle path in error (a.si -> b.si -> a.si)
  - [ ] Include actionable help: "extract shared types", "use dependency inversion", "restructure boundaries"
  - [ ] **Rust Tests**: `sigilc/src/eval/module/import.rs` — cycle error formatting tests
  - [ ] **Sigil Tests**: `tests/spec/modules/cycle_error_message.si`

- [ ] **Implement**: Report all cycles (not just first) — proposals/approved/no-circular-imports-proposal.md § Detection Algorithm
  - [ ] Continue detection after finding first cycle
  - [ ] Report each cycle with full path
  - [ ] **Rust Tests**: `sigilc/src/eval/module/import.rs` — multi-cycle detection tests
  - [ ] **Sigil Tests**: `tests/spec/modules/multiple_cycles.si`

- [x] **Implement**: Name resolution — spec/12-modules.md § Name Resolution
  - [x] **Rust Tests**: `sigilc/src/eval/module/` — name resolution tests
  - [x] **Sigil Tests**: All import tests verify name resolution

- [ ] **Implement**: Qualified access — spec/12-modules.md § Qualified Access
  - [ ] **Rust Tests**: `sigilc/src/eval/` — qualified access evaluation
  - [ ] **Sigil Tests**: `tests/spec/modules/qualified.si`

---

## 4.5 Test Modules

- [x] **Implement**: `_test/` convention — spec/12-modules.md § Test Modules
  - [x] **Rust Tests**: `sigilc/src/eval/module/import.rs` — test module detection
  - [x] **Sigil Tests**: `tests/spec/modules/_test/test_module_access.test.si`

- [x] **Implement**: Test files access private items — spec/12-modules.md § Test Modules
  - [x] **Rust Tests**: `sigilc/src/eval/module/` — private access rules
  - [x] **Sigil Tests**: `tests/spec/modules/_test/test_module_access.test.si`

---

## 4.6 Prelude

- [x] **Implement**: Types: `Option`, `Result`, `Error`, `Ordering` — spec/12-modules.md § Prelude
  - [x] **Rust Tests**: `sigilc/src/eval/` — built-in type tests
  - [x] **Sigil Tests**: Option/Result used throughout `tests/spec/`

- [x] **Implement**: Built-in functions: `print`, `panic`, `int`, `float`, `str`, `byte` — spec/12-modules.md § Prelude
  - [x] **Rust Tests**: `sigilc/src/eval/evaluator/` — `register_prelude()` tests
  - [x] **Sigil Tests**: Built-ins used throughout test suite

- [x] **Implement**: Built-in methods: `.len()`, `.is_empty()`, `.is_some()`, etc. — Lean Core
  - [x] **Rust Tests**: `sigil_eval/src/methods.rs` — method dispatch tests
  - [x] **Sigil Tests**: `tests/spec/traits/core/` — method tests

- [x] **Implement**: Auto-import prelude from `library/std/prelude.si` — spec/12-modules.md § Prelude
  - [x] `Evaluator::load_prelude()` auto-loads prelude before any module
  - [x] All public functions from prelude available without import
  - [x] **Rust Tests**: `sigilc/src/eval/evaluator/` — prelude loading tests
  - [x] **Sigil Tests**: `test_autoload.si` verifies assert_eq, is_some work without import

- [x] **Implement**: Prelude functions auto-available
  - [x] `assert`, `assert_eq`, `assert_ne`, `assert_some`, `assert_none`, `assert_ok`, `assert_err`
  - [x] `is_some`, `is_none`, `is_ok`, `is_err`
  - [x] `len`, `is_empty`
  - [x] `compare`, `min`, `max`
  - Note: Trait definitions in prelude (Eq, Comparable, etc.) parse but need Phase 3 for full integration

---

## 4.7 Import Graph Tooling

> **PROPOSAL**: `proposals/approved/no-circular-imports-proposal.md § Tooling Support`

- [ ] **Implement**: `sigil check --cycles` — Check for cycles without full compilation
  - [ ] Fast path: parse imports only, build graph, detect cycles
  - [ ] **Rust Tests**: `sigilc/src/cli/` — cycle checking tests
  - [ ] **Sigil Tests**: `tests/cli/check_cycles.si`

- [ ] **Implement**: `sigil graph --imports` — Visualize import graph
  - [ ] Output DOT format for graphviz
  - [ ] Usage: `sigil graph --imports > imports.dot && dot -Tpng imports.dot -o imports.png`
  - [ ] **Rust Tests**: `sigilc/src/cli/` — graph output tests
  - [ ] **Sigil Tests**: `tests/cli/graph_imports.si`

---

## 4.8 Remaining Work

**Not yet implemented:**
- Module alias syntax: `use std.net.http as http`
- Re-exports: `pub use './client' { get, post }`
- Qualified access: `module.function()` without importing

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
- [ ] Module alias syntax (`use std.net.http as http`)
- [ ] Re-exports (`pub use`)
- [ ] Qualified access (`module.function()`)
- [ ] Type definitions parsing (see Phase 5)
- [x] Run cargo tests: `cargo test` passes
- [ ] Run full spec tests: 166/197 passing (31 failures are pattern/syntax issues, not module issues)

**Exit Criteria**: Multi-file projects compile ✅ (core support complete)
**Status**: Phase 4 core functionality complete. Remaining items blocked by other phases.
