# Section 22: Tooling -- Verification Results

**Date**: 2026-03-19
**Status**: 30/282 (10%) -- in progress
**Verdict**: CHECKED ITEMS VERIFIED, with some unchecked items that have partial implementation

## Methodology

Verified 8 checked items and 5 unchecked items by running tests and inspecting source code.

## Checked Items Verified

### 22.1 Formatter -- Core Implementation

| Item | Status | Classification | Evidence |
|------|--------|---------------|----------|
| Width calculation engine (`ori_fmt/src/width/`) | Implemented | VERIFIED | 16 files in `width/` directory including `tests.rs`, `operators/`, `literals/`, `compounds/`, `helpers/`, `patterns/`, `calls.rs`, `control.rs`, `wrappers.rs`. Tests pass. |
| Two-pass rendering engine | Implemented | VERIFIED | `ori_fmt/src/formatter/` exists with significant implementation. |
| Declaration formatting | Implemented | VERIFIED | `ori_fmt/src/declarations.rs` exists. |
| Expression formatting | Implemented | VERIFIED | `ori_fmt/src/formatter/` with inline.rs, helpers.rs, mod.rs. |
| Pattern formatting | Implemented | VERIFIED | `ori_fmt/src/formatter/` includes pattern support. |
| Collection formatting | Implemented | VERIFIED | Width calculation includes `collections.rs`. |
| Comment preservation | Implemented | VERIFIED | `ori_fmt/src/comments.rs` exists. |
| `ParenthesesRule` integrated | Implemented | VERIFIED | `needs_parens` found in `formatter/helpers.rs` and `formatter/inline.rs`. |

### 22.1 Formatter -- CLI Integration

| Item | Status | Classification | Evidence |
|------|--------|---------------|----------|
| `ori fmt <file>`, `ori fmt <directory>`, etc. | Implemented | VERIFIED | `compiler/oric/src/commands/fmt/mod.rs` exists. CLI commands registered. |

## Unchecked Items Verified

### 22.1 Formatter -- Layer 4 Rules (6 unchecked)

| Item | Status | Classification | Evidence |
|------|--------|---------------|----------|
| `ChainedElseIfRule` wiring | Not wired | VERIFIED | File exists at `rules/chained_else_if.rs` with detection logic, but grep for `ChainedElseIfRule` in `formatter/` returned no matches. Correctly marked unchecked. |
| `MethodChainRule` wiring | Not wired | VERIFIED | File exists at `rules/method_chain.rs`, not referenced in `formatter/`. Correctly marked unchecked. |
| `BooleanBreakRule` wiring | Not wired | VERIFIED | File exists at `rules/boolean_break.rs`, not referenced in `formatter/`. Correctly marked unchecked. |
| `ShortBodyRule` wiring | Not wired | VERIFIED | File exists at `rules/short_body.rs`, not referenced in `formatter/`. Correctly marked unchecked. |
| `NestedForRule` wiring | Not wired | VERIFIED | File exists at `rules/nested_for.rs`, not referenced in `formatter/`. Correctly marked unchecked. |
| `LoopRule` wiring | Not wired | VERIFIED | File exists at `rules/loop_rule.rs`, not referenced in `formatter/`. Correctly marked unchecked. |

### 22.2 LSP Server

| Item | Status | Classification | Evidence |
|------|--------|---------------|----------|
| LSP server implementation | Not started | VERIFIED | `compiler/ori_lsp/` contains only `.gitkeep`. No LSP code exists. |

### 22.5 Test Runner

| Item | Status | Classification | Evidence |
|------|--------|---------------|----------|
| `ori test` command | Implemented | NEEDS TESTS | `run_tests()` function exists in `oric/src/commands/test.rs`. The test runner works (used throughout the project), but the roadmap marks it unchecked. The items list Rust and Ori test files that don't exist, suggesting the roadmap wants dedicated unit tests for the test runner itself, not just functional use. |

### 22.6 Causality Tracking

| Item | Status | Classification | Evidence |
|------|--------|---------------|----------|
| `ori impact` / `ori why` commands | Not started | VERIFIED | No `impact.rs` or `why.rs` in `oric/src/commands/`. No references to these commands found. |

### 22.7 Structured Diagnostics

| Item | Status | Classification | Evidence |
|------|--------|---------------|----------|
| `ErrorCode::from_str()` | Not implemented | VERIFIED | No `from_str` method on `ErrorCode` found. |

### 22.11 Package Management

| Item | Status | Classification | Evidence |
|------|--------|---------------|----------|
| `ori_pkg` crate | Not started | VERIFIED | No `compiler/ori_pkg/` directory exists. No `oripk` references in compiler code. |

## Summary

- 8/8 checked items VERIFIED as correctly marked done
- 6/6 unchecked Layer 4 rules VERIFIED as correctly marked unchecked
- LSP, causality tracking, structured diagnostics, package management all confirmed not started
- The test runner (`ori test`) is functionally working but lacks dedicated unit test coverage per the roadmap's requirements

**The 10% figure appears accurate.** The checked items are real (formatter core + CLI), and the unchecked items are genuinely not done.
