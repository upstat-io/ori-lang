---
paths:
  - "**test**"
---

# Specification Tests

**Tests are source of truth.** Test fails = code is wrong, not the test.
**Tests are MANDATORY.** There are zero scenarios where skipping tests is acceptable. Every code change — bug fix, feature, refactor, optimization — requires tests. No exceptions.

## TDD for Bugs
1. STOP — don't jump to fixing
2. Consult spec for intended behavior
3. Write MATRIX tests — not just "multiple":
   - **Exact failing case**: the specific input that triggered the bug
   - **Edge cases**: empty, single-element, boundary conditions
   - **Cross-type matrix**: if the fix is type-dependent, test ALL relevant types through the same code path (str, [int], Option<str>, closures, structs, maps, sets)
   - **Cross-pattern matrix**: if the fix is pattern-dependent, test ALL relevant control-flow patterns (full iteration, break, yield, guard, nested, two-call)
   - **Cross-feature matrix**: test interactions with other language features that flow through the same code path (see §Interaction Testing below)
   - **Semantic pin**: at least one test that ONLY passes with the new semantics — the permanent regression guard
   - **Negative pin**: at least one `#compile_fail` or assertion that REJECTS the old/broken behavior — proves the compiler actively prevents the regression, not just happens to avoid it
4. Verify tests FAIL (proves understanding)
5. Fix the code
6. Tests pass WITHOUT modification
7. Verify matrix completeness — missing cells are future regressions

## Matrix Testing Rule

**Every fix that touches a code path shared by multiple types or patterns requires matrix coverage.** A fix to iterator cleanup that works for `str` but isn't tested with `[int]`, `Option<str>`, closures, and maps is incomplete. Dimensions:

- **Type dimension**: all types that flow through the fixed code path
- **Pattern dimension**: all control-flow patterns that exercise the fixed code path
- **Feature dimension**: all language features that interact with the fixed code path (see §Interaction Testing)
- **Backend dimension**: debug + release builds, interpreter + AOT parity
- **Phase dimension**: if the fix touches shared infrastructure, verify parse→typeck→eval→codegen all still agree

A fix is complete when the matrix is covered. Missing cells are potential regressions waiting to happen.

**Matrix squeeze principle**: Each matrix test narrows the gap between "works" and "crashes," triangulating the bug from multiple angles. When the matrix is dense, the correct fix surface becomes surgically obvious — all surrounding cases are pinned, so the fix must thread precisely between them. This has two compounding effects:

1. **Fix precision**: A dense matrix forces fixes to be narrow and correct. A fix that's too broad breaks existing passing cells; a fix that's too narrow leaves failing cells. The matrix defines the exact boundary.
2. **Regression triangulation**: When a fix introduces a regression, the existing matrix catches it immediately and identifies exactly which dimension (type, pattern, feature, phase) the regression occupies. This turns "something broke" into "the fix doesn't handle zip iterators with unresolved type variables" — the matrix squeezes out the ambiguity.
3. **Organic convergence**: Over time, the matrix accumulates coverage that makes each subsequent fix easier and more precise. Bugs that previously required extensive investigation are immediately localized by the matrix to a specific type × pattern × feature cell.

The matrix squeeze effect is strongest when tests are added BEFORE the fix (TDD), not after — the pre-fix failing cells define the exact scope of the bug, and the pre-fix passing cells define the exact boundary the fix must respect.

**Self-verifying matrix completeness** (from Zig): When writing matrix tests that iterate over types or patterns, include a count assertion that proves every cell was visited. A matrix loop that silently skips cells is worse than no matrix at all:
```rust
let mut count = 0;
for ty in ALL_TYPES { for pattern in ALL_PATTERNS { test(ty, pattern); count += 1; } }
assert_eq!(count, ALL_TYPES.len() * ALL_PATTERNS.len()); // proves no cells skipped
```

**Fix completeness checklist** — a fix is NOT done until:
- [ ] Matrix tests cover every relevant type × pattern × feature combination
- [ ] At least one semantic pin test would fail if the fix is reverted
- [ ] At least one negative pin rejects the broken behavior
- [ ] Positive + negative pairing: every "should work" test has a corresponding "should fail" counterpart
- [ ] Debug AND release builds pass
- [ ] `ORI_CHECK_LEAKS=1` reports zero leaks on all test programs
- [ ] Interpreter and AOT produce identical results for all new tests
- [ ] Plan/roadmap updated if the fix crosses section boundaries

## Interaction Testing — Feature × Feature (MANDATORY)

**Every feature must be tested in combination with other features it can interact with.** Compilers break at feature boundaries, not within features. A type that works alone but fails inside a closure, behind a `?`, inside a `for...yield`, or through a trait method is a real bug that users will hit.

**When implementing or fixing feature A, test A × B for every relevant B:**

| If A touches... | Also test with... |
|---|---|
| Type inference | Generics, closures, trait bounds, `?` operator, pattern matching |
| Control flow (loops, match) | Break/continue with values, labels, nested loops, yield, `?` |
| Collections (list, map, set) | Iteration, COW, slicing, nested collections, generic element types |
| Closures/lambdas | Capture patterns (value types, collections, structs), recursive calls, trait bounds |
| Error handling (`?`, Result) | Closures, loops, match arms, nested `?`, `try` blocks |
| Traits/methods | Generic impls, default impls, trait objects, associated types, derived traits |
| Pattern matching | Nested patterns, guards, bindings (`@`), exhaustiveness, sum types with generics |
| ARC/memory | Loops (the back-edge), closures (captures), COW (uniqueness checks), nested structures |
| Structs/sum types | Generics, derive, pattern matching, method dispatch, nested types |
| String operations | Template literals, interpolation with complex expressions, Unicode edge cases |

**Minimum interaction coverage**: for any new feature or fix, test at least 3 cross-feature interactions. For features touching ARC/memory, test at least 5 (loops, closures, COW, nested structures, error paths).

## Cross-Phase Verification (MANDATORY)

**A test that only exercises one compiler phase is incomplete.** Production compilers (TypeScript, Swift, Zig) verify that all phases agree on every test. For Ori:

1. **Dual-execution parity**: Every spec test runs through both the interpreter and LLVM backend. A test that passes in the interpreter but not LLVM (or vice versa) is a bug — not a "backend limitation." The `test-all.sh` suite enforces this via parallel `cargo st` (interpreter) and `ori test --backend=llvm` (LLVM) runs. Any new test that is skipped for one backend MUST have a plan item tracking when it will be supported.

2. **Phase-boundary tests**: When a fix touches infrastructure shared between phases (e.g., type representations, pattern compilation, method resolution), write tests that verify the handoff:
   - **Parse → Typeck**: the parsed AST structure matches what the type checker expects
   - **Typeck → Eval**: inferred types match runtime behavior (e.g., generic instantiation produces correct values)
   - **Typeck → Codegen**: type checker decisions (RC strategy, layout, calling convention) produce correct LLVM IR
   - **ARC → Codegen**: ARC pipeline decisions (uniqueness, reuse, COW) are faithfully lowered to runtime calls

3. **Fault tolerance testing** (from Gleam): When testing error paths, verify the compiler continues analyzing subsequent definitions. An error in function `f` must not prevent type-checking function `g`. Write `#compile_fail` tests with MULTIPLE errors and verify ALL are reported, not just the first.

## ARC/Memory Testing Protocol (MANDATORY for memory-touching changes)

Adapted from Swift (SIL ARC tests), Koka (PARC @dup/@drop golden files), and Lean 4 (IR-level inc/dec tests).

**Every change to ARC/RC/COW code requires ALL of:**

1. **Behavioral correctness**: program produces the right output
2. **RC balance**: `ORI_CHECK_LEAKS=1` reports zero leaks AND `ORI_TRACE_RC=1` shows balanced inc/dec
3. **IR-level verification** (when touching `ori_arc` or `ori_llvm`): use `ORI_DUMP_AFTER_ARC=1` or `ORI_DUMP_AFTER_LLVM=1` to verify exact RC operation placement. Document expected operations in test comments.
4. **Positive + negative RC pairing** (from Swift): every test where an optimization SHOULD fire (e.g., elide a retain) must have a companion test where it SHOULD NOT fire (e.g., aliasing prevents elision). Name them together: `test_rc_elision_simple` + `test_rc_elision_blocked_by_alias`.
5. **Loop-isolated tests** (from Swift): ARC around loops is the #1 source of RC bugs. Test every loop topology separately: simple `for`, `while`, nested loop, loop with `break`, loop with `yield`, loop with `?`.
6. **COW barrier tests** (from Swift): RC motion must NOT cross `is_unique` / COW uniqueness checks. Test that `cow_branch` instructions remain as barriers.
7. **Drop elision tests** (from Koka): when a value is consumed and a new value returned, verify the old value is dropped (not leaked). When a value is NOT consumed, verify it is NOT dropped.
8. **Nested structure tests** (from Lean 4): test RC correctness for `[[int]]`, `{str: [int]}`, `Option<[str]>`, closures capturing collections — these exercise the elem_dec_fn / propagation paths.

## Negative Testing Protocol (MANDATORY)

**Every test suite must include negative tests.** A test suite with only positive tests ("this works") provides no protection against the compiler becoming too permissive.

1. **Compile-fail tests pin exact errors**: `#compile_fail("E1234")` must match the specific error code, not just "some error." If the error message changes, the test fails — this is the desired behavior. (From Rust's `//~ ERROR` annotations and Go's `// ERROR "pattern"`.)

2. **Runtime-fail tests pin exact panics**: `#fail("index out of bounds")` must match the specific panic message. A test that accepts any panic is too weak.

3. **"Must NOT compile" for every "must compile"**: when adding a new feature with type constraints (e.g., `T: Hashable` required for map keys), write both:
   - Positive: valid usage compiles and runs → `#compile_fail` would be wrong
   - Negative: invalid usage produces the correct error → `#compile_fail("E2031")` catches exactly this

4. **"Must NOT optimize" for every "must optimize"**: when testing an optimization (RC elision, COW in-place mutation, tail call), write a companion test where the optimization must NOT fire (aliasing, shared reference, non-tail position). (From Swift's `arcsequenceopts_knownsafebugs.sil`.)

5. **Forbid-output pins** (from Rust): when a fix changes behavior (e.g., a warning is no longer emitted, or an error message changes), add a test that asserts the OLD behavior does NOT appear. This is a stronger guarantee than just checking the new behavior is present.

6. **Idempotency testing** (from Swift): when testing a compiler pass/optimization, verify that running it twice produces the same result as running it once. A non-idempotent pass is a bug.

## Regression Discipline

**Every bug fix creates a permanent regression test.** The test carries a comment linking to the issue/plan item that motivated it:

```ori
// Regression: roadmap section-04.3, iterator drop was skipping nested closures
@test_nested_closure_iter_drop tests @target () -> void = { ... }
```

For Rust tests:
```rust
/// Regression: section-04.3 — iterator drop skipped nested closures
/// See: plans/roadmap/section-04.md §4.3.2
#[test]
fn test_nested_closure_iter_drop() { ... }
```

**Regression test naming**: use descriptive names that follow the `<subject>_<scenario>_<expected>` shape. `test_cow_slice_double_free_nested_list` ✓, `test_issue_42` ✗. The name must describe the regression scenario so future readers understand what's being guarded **without looking up any external artifact**. NEVER encode ephemeral identifiers in the function name — no plan names, no section/subsection numbers (`section_04_3`, `4_3_2`, `§4.3`), no plan annotations (`TPR_04_005`, `CROSS_04_014`), no bug IDs (`BUG_04_045`), no dates, no initials. Tests are permanent; those artifacts are ephemeral. Provenance (regression origin, plan reference, bug link) belongs in the `///` or `//` doc comment ABOVE the test — that location can rot harmlessly, the function name cannot. Full rules and banned-weak-descriptor list: `.claude/rules/impl-hygiene.md` §Test Function Naming. Violations are fixed (renamed) in the same `/impl-hygiene-review` pass that finds them — never deferred. (Derived from Rust's tidy enforcement of descriptive naming + the Osherove/AAA/GWT three-part test-naming convention adopted by Rust stdlib, JUnit, xUnit.net, Roslyn, Swift stdlib, and Go.)

**Crash/ICE regression tests**: if the compiler ever panics/crashes on valid or invalid input, that input becomes a permanent test — even before the fix is identified. Add it immediately as `#skip("ICE: <description>")` if it can't be fixed yet, but it MUST be recorded. (From Rust's `tests/crashes/` suite.)

## Error/Diagnostic Testing

**Every error code must be tested.** If `ori_diagnostic` defines error code E1234, there must be:
1. A `#compile_fail("E1234")` test that triggers it
2. The test input is a minimal reproducer (not a copy of real user code)
3. The error message is verified (the substring in `#compile_fail` must be specific enough to distinguish from similar errors)

**Error reporting completeness**: when a single input triggers multiple errors, verify ALL errors are reported. A compiler that reports only the first error and silently drops the rest has a fault-tolerance bug. Write multi-error `#compile_fail` tests. (From Gleam's fault tolerance testing.)

**No false positives on valid code**: for every warning or error category, write at least one test with VALID code that must NOT trigger the warning/error. A warning that fires on correct code is worse than a missing warning. (From Gleam's `assert_no_warnings!`.)

## Test Hygiene

1. **No orphan tests**: every test file must contain at least one assertion (`assert_eq`, `assert`, `#compile_fail`, `#fail`). A `.ori` test file that runs code but never asserts anything proves nothing and provides false confidence. The assertion IS the test.

2. **No trivial assertions**: `assert_eq(actual: true, expected: true)` or `assert_eq(actual: 1, expected: 1)` are not tests — they're tautologies. The assertion must test a value that was computed by the code under test.

3. **`#skip` budget**: a file with more than 3 `#skip` annotations is a red flag. Either the feature is not implemented (and the tests should be in a plan, not committed with `#skip`) or there are bugs to fix. Each `#skip` must have a plan item tracking its resolution.

4. **Stale test detection**: if a `#compile_fail` test starts PASSING (compiler no longer rejects the input), that is a regression — the compiler became too permissive. The test runner should detect this. Similarly, if a `#fail` test starts succeeding, either the runtime behavior changed or the test is no longer exercising the failure path.

5. **Test file naming**: Ori spec tests use kebab-case matching the feature: `tests/spec/traits/iterator/map-filter-collect.ori`. Rust tests use snake_case: `test_map_filter_collect`. Both must be descriptive of what is being tested.

## Anti-patterns (NEVER)
- Remove test "because it doesn't work" — investigate WHY
- Change expected to match actual — fix the compiler
- Assume `#compile_fail`/`#fail` incorrect — compiler may be too permissive
- Delete "redundant" tests — may cover different phases
- Mark `#skip` without investigating — find root cause
- Test only one type when the code path handles multiple types — matrix coverage required
- Test only the happy path when break/yield/guard/nested are possible — pattern coverage required
- Write only positive tests — negative tests (must-reject, must-not-optimize) are equally required
- Write a test without an assertion — running code is not testing it
- Test a feature in isolation without interaction tests — features break at boundaries
- Commit a test that is only verified in one backend — dual-execution parity is required
- Fix ARC/memory code without IR-level verification — behavioral tests alone miss RC imbalances that happen to not leak in the specific test case
- Accept "it works on my machine" — debug AND release, interpreter AND LLVM, all must pass

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
- `tests/aims/` — AIMS scenario tests (ARC, COW, FIP, TRMC)
- `tests/valgrind/` — memory safety (Valgrind, not in `test-all.sh`)
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

## Prior Art Reference

These rules are derived from production compiler testing practices:
- **Rust**: revision-based matrix testing, tidy hygiene enforcement, 4-layer error code verification, `forbid-output` negative pins, descriptive test naming, `tests/crashes/` for ICE regression
- **Go**: code-generated exhaustive type × operation matrices, `// asmcheck` multi-architecture assembly verification, `test/fixedbugs/` regression corpus
- **Zig**: comptime/runtime dual execution, self-verifying matrix counters (`comptime assert(perms == N)`), multi-backend × multi-target matrix, one-file-per-error-scenario
- **TypeScript**: baseline captures every expression's type (not just tested ones), auto-derived configuration matrix from compiler flags, cross-phase simultaneous verification
- **Gleam**: fault tolerance testing (error recovery doesn't block analysis), `assert_no_warnings!` negative tests, issue-linked regression tests
- **Roc**: per-expression type annotation tests, cross-phase pinning in one test (`+emit:mono`), parse snapshot tripartite (pass/fail/malformed), no-orphan enforcement
- **Elm**: `Expected` context type (provenance in every error), Damerau-Levenshtein suggestions, architecture-encoded error quality
- **Swift**: IR-level RC tests (retain/release at SIL), positive + negative pairing, `is_unique` barrier tests, loop-isolated ARC tests, verifier-as-test (`-enable-sil-verify-all`), idempotency testing
- **Koka**: exact IR shape golden files (@dup/@drop), drop elision tests, reuse-through-match tests, effect × ARC interaction testing
- **Lean 4**: IR-level inc/dec/is_shared tests, bug-series systematic coverage (closure_bug1-8), issue-numbered regression naming
