# Section 13: Conditional Compilation -- Verification Results

**Verified**: 2026-03-19
**Section status**: 0/202 (0%) -- not-started
**Verdict**: CONFIRMED NOT STARTED (with partial infrastructure)

---

## Summary

Section 13 is correctly marked as not-started. However, significant parser-level infrastructure already exists for `#target(...)`, `#cfg(...)`, and `#!target()`/`#!cfg()` file-level attributes. This infrastructure parses and stores the attributes but no downstream phase (type checker, evaluator, LLVM codegen) consumes them, making conditional compilation non-functional end-to-end.

### What exists (parse-only infrastructure):

1. **Lexer**: `HashBang` (`#!`) token recognized, `Hash` (`#`) already existed
2. **Parser**: Full parsing of `#target(os:, arch:, family:, not_os:)`, `#cfg(debug, release, not_debug, feature:, not_feature:)`, and `#!target()`/`#!cfg()` file-level attributes
3. **IR**: `TargetAttr`, `CfgAttr`, `FileAttr` structs defined in `ori_ir` with fields for `any_os`, `any_feature` (though parser doesn't fill them)
4. **Formatter**: `ori_fmt` correctly formats file-level attributes
5. **Visitor**: `visit_file_attr` hook exists in `ori_ir` visitor
6. **Spec**: `spec/25-conditional-compilation.md` exists and is comprehensive (25.1 through 25.10, error codes E0930-E0933)

### What does NOT exist:

1. **Type checker**: No consumption of `TargetAttr`/`CfgAttr` -- items are not pruned based on conditions
2. **Evaluator**: No awareness of conditional compilation attributes
3. **LLVM codegen**: No conditional compilation support
4. **Compile-time constants**: `$target_os`, `$target_arch`, `$target_family`, `$debug`, `$release` are not registered in `ori_types` identifier inference
5. **`compile_error` builtin**: Not recognized in type checker or evaluator
6. **CLI flags**: No `--feature`, `--cfg`, `--no-default-features`, `--all-features` flags (only `--target` exists for LLVM triple, not for conditional compilation)
7. **Build system**: No `ori.toml` parsing for features
8. **Ori spec tests**: `tests/spec/conditional/` directory does not exist
9. **OR conditions parsing**: `any_os`, `any_arch`, `any_feature` fields exist in IR but parser never fills them (array syntax not parsed)
10. **Diagnostics**: No condition-aware error messages (E0930-E0933 not implemented)

---

## Spot-Check Results (10 items sampled)

### 13.1 -- Target Attribute

**Item**: `[ ] Lexer/Parser: Parse target attributes -- #target(...) syntax` (line 128)
**Status**: PARTIAL -- Parser DOES parse `#target(os:, arch:, family:, not_os:)` into `TargetAttr`. Tests pass (24 parser attr tests, 16 file_attr phase tests). But the parsed attribute is never consumed by any downstream phase, so it has no effect on compilation.
**Classification**: CONFIRMED INCOMPLETE -- parsing done, semantic processing not started

**Item**: `[ ] Compiler: Target evaluation -- Evaluate against build target` (line 133)
**Status**: NOT IMPLEMENTED -- No code in `ori_types` or `ori_eval` references `TargetAttr`. No build target evaluation logic exists.
**Classification**: CONFIRMED NOT STARTED

**Item**: `[ ] Ori Tests: tests/spec/conditional/target_basic.ori` (line 138)
**Status**: NOT IMPLEMENTED -- Directory `tests/spec/conditional/` does not exist.
**Classification**: CONFIRMED NOT STARTED

### 13.2 -- OR Conditions

**Item**: `[ ] Parser: Parse any_* variants -- Array literal values` (line 173)
**Status**: NOT IMPLEMENTED -- `TargetAttr.any_os: Vec<Name>` and `CfgAttr.any_feature: Vec<Name>` fields exist in IR but the parser never populates them. The parser's `parse_target_attr_body` only handles `os`, `arch`, `family`, `not_os` -- no `any_os` / `any_arch` branch.
**Classification**: CONFIRMED NOT STARTED

### 13.3 -- Negation

**Item**: `[ ] Parser: Parse not_* variants` (line 219)
**Status**: PARTIAL -- `not_os` is parsed in `#target(not_os: "windows")`. But `not_arch` and `not_family` are NOT parsed (no match arms). `#cfg(not_debug)` is parsed. `#cfg(not_feature: "x")` is parsed.
**Classification**: CONFIRMED INCOMPLETE -- partial negation support in parser, no semantic processing

### 13.4 -- Cfg Attribute

**Item**: `[ ] Parser: Parse cfg attributes -- Boolean flags (debug, release)` (line 266)
**Status**: PARTIAL -- Parser handles `debug`, `release`, `not_debug`, `feature:`, `not_feature:`. But `any_feature:` is not parsed. And no downstream phase uses the parsed `CfgAttr`.
**Classification**: CONFIRMED INCOMPLETE -- parsing mostly done, semantic processing not started

**Item**: `[ ] Compiler: Cfg evaluation -- Accept --debug / --release flags` (line 271)
**Status**: NOT IMPLEMENTED -- Build options have `release: bool` but it's for optimization level, not for setting `#cfg(release)`. No `--cfg` flag exists.
**Classification**: CONFIRMED NOT STARTED

### 13.6 -- File-Level Conditions

**Item**: `[ ] Lexer: Recognize #! token` (line 383)
**Status**: DONE -- `HashBang` token exists in lexer (`ori_lexer_core`), parser handles it in `parse_file_attribute()`, and 16 phase tests verify parsing. Tests pass.
**Classification**: CONFIRMED INCOMPLETE -- lexer/parser done, semantic processing not started

### 13.7 -- Compile-Time Constants

**Item**: `[ ] Type checker: Compile-time evaluation -- Evaluate comparisons at compile time` (line 459)
**Status**: NOT IMPLEMENTED -- No `$target_os`, `$target_arch`, `$target_family`, `$debug`, `$release` registered in `ori_types/infer/expr/identifiers.rs`. These identifiers would produce "unknown identifier" errors.
**Classification**: CONFIRMED NOT STARTED

### 13.10 -- compile_error Built-in

**Item**: `[ ] Compiler: compile_error evaluation` (line 652)
**Status**: NOT IMPLEMENTED -- `compile_error` string not found in `ori_types` or `ori_eval`. The Rust `compile_error!()` macro is used in `oric/main.rs` and `oric/lib.rs` for Rust-level feature gates, but no Ori-level `compile_error` builtin exists.
**Classification**: CONFIRMED NOT STARTED

---

## Test Execution

```
Parser attribute tests:       24 passed (ori_parse::grammar::attr)
File-level attribute tests:   16 passed (oric::phases::parse::file_attr)
```

All existing parser infrastructure tests pass. No semantic-level tests exist because no semantic processing is implemented.

---

## Assessment

The 0% status is accurate for end-to-end functionality. Roughly 15-20% of the total work has been done at the parser/IR level:

- `#target(os:, arch:, family:, not_os:)` parsing: done
- `#cfg(debug, release, not_debug, feature:, not_feature:)` parsing: done
- `#!target()`/`#!cfg()` file-level parsing: done
- IR data structures: done
- Formatter support: done
- OR conditions (`any_*`) parsing: NOT done
- Negation (`not_arch`, `not_family`): NOT done
- Semantic evaluation (type checker pruning): NOT done
- Compile-time constants: NOT done
- `compile_error` builtin: NOT done
- CLI flags (`--feature`, `--cfg`): NOT done
- Build system (`ori.toml` features): NOT done
- Diagnostics (E0930-E0933): NOT done
- LLVM codegen support: NOT done
- All Ori spec tests: NOT done

The section is genuinely not-started from a user-facing perspective. The parser infrastructure is a foundation but provides zero observable behavior.
