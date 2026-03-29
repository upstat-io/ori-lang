# Section 13: Conditional Compilation — Verification Results

**Verified**: 2026-03-28
**Re-verified**: 2026-03-29 (confirmed current codebase matches findings, section frontmatter and checkboxes updated)
**Status in roadmap**: in-progress (updated 2026-03-29)
**Actual status**: PARTIAL — substantial parser/IR infrastructure for `#target`, `#cfg`, file-level `#!target`/`#!cfg` attributes. Spec file exists. No evaluation/pruning, no codegen integration.

## Summary

The conditional compilation section is marked `not-started` but has significant hidden implementation:
- **`#target(...)` parsing**: COMPLETE — all fields (os, arch, family, not_os, not_arch, not_family, any_os, any_arch)
- **`#cfg(...)` parsing**: COMPLETE — debug, release, not_debug, feature, not_feature, any_feature
- **File-level `#!target`/`#!cfg`**: COMPLETE parsing
- **Item-level attributes**: COMPLETE — stored on functions, types, constants, imports, impls
- **Feature name validation**: COMPLETE — `is_valid_feature_name()` enforces identifier rules
- **Attribute validation**: COMPLETE — rejects unsupported attrs on wrong item kinds
- **Spec file**: EXISTS at `docs/ori_lang/v2026/spec/25-conditional-compilation.md`
- **Conditional evaluation/pruning**: NOT implemented
- **Compile-time constants**: NOT implemented
- **Build configuration**: NOT implemented

---

## 13.1 Target Attribute

### Spec
- [done] `spec/25-conditional-compilation.md` EXISTS — covers target attribute syntax, OS, arch, family values
  - File: `docs/ori_lang/v2026/spec/25-conditional-compilation.md`

### IR
- [done] `TargetAttr` struct — `compiler/ori_ir/src/ast/items/function.rs:315`
  - Fields: `os`, `arch`, `family`, `any_os`, `any_arch`, `not_os`, `not_arch`, `not_family`
- [done] Stored on `FunctionDef::target_attr` — `compiler/ori_ir/src/ast/items/function.rs`
- [done] Stored on `TypeDecl::target_attr` — `compiler/ori_ir/src/ast/items/types.rs:67`
- [done] Stored on imports — `compiler/ori_ir/src/ast/items/imports.rs`
- [done] Stored on impl blocks — `compiler/ori_ir/src/ast/items/traits.rs`
- [done] `FileAttr::Target` variant — `compiler/ori_ir/src/ast/items/function.rs`

### Parser
- [done] `parse_target_attr_body()` — `compiler/ori_parse/src/grammar/attr/conditional.rs:32`
  - Handles: os, arch, family, not_os, not_arch, not_family, any_os, any_arch
  - Unknown parameter names produce errors
- [done] Item-level `#target(...)` on functions, types, constants, imports, impls
- [done] File-level `#!target(...)` parsing
- [done] `any_os`/`any_arch` list parsing with `[...]` syntax

### Formatter
- [done] Conditional attrs formatted via `ori_fmt/src/declarations/mod.rs` (stores attrs on items)

### Compiler — Evaluation
- [todo] No target evaluation against build target
- [todo] No AST pruning for false branches
- [todo] No tracking for error messages

### Tests
- [done] Extensive parser tests in `compiler/oric/tests/phases/parse/file_attr.rs` (27 tests):
  - File-level target os, arch, family, not_os, not_arch, not_family, any_os, any_arch
  - Combined params, single/trailing-comma lists, edge cases
- [done] Parser tests in `compiler/oric/tests/phases/parse/attr_validation.rs` (28+ tests):
  - Attribute placement validation (reject on traits, extends, extern blocks)
  - Accept on functions, types, constants, imports, impls
  - Orphaned attrs at EOF diagnosed
- [todo] No spec tests (`tests/spec/conditional/target_basic.ori` does not exist)
- [todo] No LLVM tests, no AOT tests

---

## 13.2 OR Conditions

### Parser
- [done] `any_os` with list syntax `["linux", "macos"]` — `conditional.rs:102-103`
- [done] `any_arch` with list syntax — `conditional.rs:103`
- [done] List parsing `parse_attr_string_list()` — handles comma-separated strings, trailing commas

### Tests
- [done] File-level `any_os`, `any_arch` tests — `file_attr.rs`
- [todo] No evaluator tests (OR condition matching)
- [todo] No spec tests

---

## 13.3 Negation

### IR
- [done] `not_os`, `not_arch`, `not_family` fields on `TargetAttr`
- [done] `not_debug`, `not_feature` fields on `CfgAttr`

### Parser
- [done] All negation forms parsed — `conditional.rs:99-101` for target, `conditional.rs:173-175` for cfg
- [done] `not_debug` as bare identifier — `conditional.rs:193`

### Tests
- [done] Parser tests for `not_os`, `not_arch`, `not_family` — `file_attr.rs`
- [done] Parser test for `not_debug` — `file_attr.rs`
- [done] Parser test for `not_feature` — `file_attr.rs` / `attr_validation.rs`
- [todo] No evaluator tests (boolean NOT logic)
- [todo] No spec tests

---

## 13.4 Cfg Attribute

### IR
- [done] `CfgAttr` struct — `compiler/ori_ir/src/ast/items/function.rs:339`
  - Fields: `debug`, `release`, `not_debug`, `feature`, `any_feature`, `not_feature`
- [done] Stored on functions, types, constants, imports, impls (via `cfg_attr` field)
- [done] `FileAttr::Cfg` variant for file-level

### Parser
- [done] `parse_cfg_attr_body()` — `compiler/ori_parse/src/grammar/attr/conditional.rs:139`
  - Handles: debug, release, not_debug (bare identifiers)
  - Handles: feature, not_feature (keyed), any_feature (list)
- [done] Feature name validation — `is_valid_feature_name()` at `conditional.rs:17`
  - Validates identifier rules (letter/underscore start, alphanumeric+underscore body)
  - Produces `E0932` error for invalid names

### Tests
- [done] Parser tests for debug, release, not_debug, feature, any_feature — `file_attr.rs`
- [done] Feature name validation tests (valid: ssl, _private, Feature123; invalid: hyphenated, digit-start, special chars, empty, dot) — `file_attr.rs:277-377`
- [todo] No compiler evaluation tests
- [todo] No spec tests

---

## 13.5 Feature Flags

- [done] Feature name validation at parse time (E0932 error for invalid names)
- [todo] No `ori.toml` features section parsing
- [todo] No feature dependency resolution
- [todo] No `--feature`, `--no-default-features`, `--all-features` CLI flags
- [todo] No tests beyond parser validation

---

## 13.6 File-Level Conditions

### IR
- [done] `FileAttr` enum with `Target` and `Cfg` variants — `compiler/ori_ir/src/ast/items/function.rs`
- [done] `Module::file_attr: Option<FileAttr>` field

### Parser
- [done] `#!target(...)` and `#!cfg(...)` parsing — detected by `#!` prefix
- [done] Invalid file-level attrs rejected (derive, skip, repr) — `attr_validation.rs`
- [done] Position requirement enforced (before imports/declarations)

### Tests
- [done] 27 parser tests for file-level attrs — `file_attr.rs`
- [todo] No evaluation (file skipping when condition is false)
- [todo] No spec tests

---

## 13.7 Compile-Time Constants

- [todo] `$target_os`, `$target_arch`, `$target_family`, `$debug`, `$release` NOT registered
  - Not in `ori_types/src/infer/expr/identifiers.rs`
  - Not in `ori_eval`
- [todo] No compile-time evaluation of comparisons
- [todo] No dead code elimination for false branches
- [todo] No tests

---

## 13.8 Build Configuration

- [todo] No `ori.toml` configuration parsing for features/targets
- [todo] No `--target`, `--feature`, `--debug`/`--release` CLI integration for conditional compilation
  - NOTE: `ori build --release` exists for build optimization but NOT for `#cfg(release)` evaluation
- [todo] No tests

---

## 13.9 Diagnostics

- [done] Error code `E0932` for invalid feature names — `conditional.rs:262`
- [done] Error messages for unknown target/cfg parameters
- [done] Orphaned attrs at EOF diagnosed — `attr_validation.rs:196-267`
- [done] Diagnostic messages list valid declaration kinds per Spec section 25.4
- [todo] No platform-mismatch diagnostics (showing active configuration)
- [todo] No "unknown OS/arch" warnings
- [todo] No suggestions for invalid values
- [todo] No compile-fail tests in `tests/compile-fail/conditional/`

---

## 13.10 compile_error Built-in

- [todo] `compile_error` not recognized as a built-in function
  - Not in type checker identifier inference
  - Not in evaluator function registry
  - Not in standard library
- [todo] No compile-time error emission
- [todo] No tests

---

## Correction Needed

The roadmap status should be changed from `not-started` to `partial`. Key completed items:
1. Spec file `spec/25-conditional-compilation.md` exists
2. `TargetAttr` and `CfgAttr` IR structures with all fields (os, arch, family, any_os, any_arch, negation, feature, any_feature)
3. `FileAttr` enum for file-level `#!target`/`#!cfg`
4. Full parser for `#target(...)`, `#cfg(...)`, `#!target(...)`, `#!cfg(...)` with all variants
5. Feature name validation (`is_valid_feature_name`, E0932)
6. Attribute placement validation (reject on wrong item kinds)
7. Orphaned attribute diagnostics
8. Item-level attrs stored on functions, types, constants, imports, impls
9. Extensive parser test suite (55+ tests across `file_attr.rs` and `attr_validation.rs`)

Estimated: ~40% of items have hidden implementation (parser/IR/diagnostics layer fully done).

The major gaps are:
- No conditional evaluation logic (no target/cfg matching against build configuration)
- No AST pruning of false branches
- No compile-time constants (`$target_os`, `$debug`, etc.)
- No `compile_error` built-in
- No build system integration for features
- No end-to-end tests
