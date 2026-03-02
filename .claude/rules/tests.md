---
paths:
  - "**test**"
---

# Specification Tests

**Tests are source of truth.** Test fails = code is wrong, not the test.

## TDD for Bugs
1. STOP — don't jump to fixing
2. Consult spec for intended behavior
3. Write MULTIPLE tests: exact case, edges, variations, guards
4. Verify tests FAIL (proves understanding)
5. Fix the code
6. Tests pass WITHOUT modification

## Anti-patterns (NEVER)
- Remove test "because it doesn't work" — investigate WHY
- Change expected to match actual — fix the compiler
- Assume `#compile_fail`/`#fail` incorrect — compiler may be too permissive
- Delete "redundant" tests — may cover different phases
- Mark `#skip` without investigating — find root cause

## Investigation Order
1. Lexer fully implements this?
2. Parser fully implements this?
3. Type checker handles this?
4. Evaluator implements this?
5. Test runner interprets attributes correctly?
6. ONLY THEN consider test is wrong

## Quality
- Test behavior, not implementation
- Edge cases: empty, boundary, error
- No flaky: no timing, shared state, order deps
- `#[ignore]` needs tracking issue
- Rust tests in sibling `tests.rs`: `#[cfg(test)] mod tests;` in source, body in `tests.rs`
  - `foo.rs` -> `foo/tests.rs`; `mod.rs` in `bar/` -> `bar/tests.rs`; `lib.rs`/`main.rs` -> `tests.rs` in same dir
  - **Allowed in source**: `#[cfg(test)]` helper fns, test-only imports, const assertions, `pub(crate) mod test_helpers;`
  - **Never in source**: `#[cfg(test)] mod tests { #[test] fn ... }` — always extract
- Ori tests in `_test/` subdirs: `foo.ori` -> `_test/foo.test.ori`
- Clear naming: `test_parses_nested_generics`
- AAA structure

## Directories
- `tests/spec/` — conformance (`.ori` + inline `@test`)
- `tests/compile-fail/` — expected failures (`#compile_fail`/`#fail`)
- `tests/run-pass/` — expected success (source + `_test/*.test.ori`)
- `tests/fmt/` — formatting
- `compiler/oric/tests/phases/` — phase integration
- `compiler/ori_llvm/tests/aot/` — AOT integration

## Running
- `cargo st` — all spec tests
- `cargo st tests/spec/types/` — specific category
- `./test-all.sh` — full suite
- `./llvm-test.sh` — LLVM unit tests
- `cargo b --release && ./target/release/ori test --backend=llvm tests/`

## Attributes
- `#skip("reason")` — skip with explanation
- `#compile_fail("substring")` — expect compile failure
- `#fail("substring")` — expect runtime failure

## Debugging / Tracing
- `ORI_LOG=debug cargo st tests/spec/types/` — all phases; `ori_types=debug` type checker only; `ori_eval=debug` evaluator only; `ORI_LOG_TREE=1` hierarchical
- Phase dumps: `ORI_DUMP_AFTER_PARSE=1`, `ORI_DUMP_AFTER_TYPECK=1`, `ORI_DUMP_AFTER_ARC=1`, `ORI_DUMP_AFTER_LLVM=1`
- AOT failures: `diagnostics/diagnose-aot.sh`, `dual-exec-debug.sh`, `codegen-audit.sh`, `ORI_TRACE_RC=1 ORI_CHECK_LEAKS=1 ./binary`
- Wrong result? `ORI_LOG=ori_eval=trace ORI_LOG_TREE=1`; type error? `ori_types=debug`; Salsa caching? `oric=debug`

## Coverage
`cargo tarpaulin -p CRATE --lib --out Stdout` — target 60-80%
