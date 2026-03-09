---
title: "Appendix E: Coding Guidelines"
description: "Ori Compiler Design — Appendix E: Coding Guidelines"
order: 9005
section: "Appendices"
---

# Appendix E: Coding Guidelines

## E.1 Conceptual Foundations

Compiler codebases impose stricter demands on coding standards than typical application software. The reason is combinatorial: a compiler with *k* phases creates O(*k*^2) potential interaction surfaces. A change to how the lexer represents string literals may ripple through the parser, type checker, evaluator, code generator, and runtime. When the Ori lexer gained decimal duration literals, the parser needed new precedence rules, the type checker needed new coercion paths, the evaluator needed new arithmetic dispatch, and the LLVM backend needed new constant folding. A single feature touched five crates. Consistency across these boundaries is not aesthetic preference but engineering necessity.

Standards serve three functions simultaneously. First, they act as *living documentation*: a new contributor reading unfamiliar code can predict structure, naming, and error handling idioms without studying each file individually. When every file follows the same organizational template, the question shifts from "how is this file structured?" to "what does this file do?" The cognitive overhead of reading a new module drops from understanding both content and form to understanding content alone.

Second, they enable *safe refactoring*: when every file follows the same pattern, automated tools and human reviewers can detect when a change breaks convention, which often signals a logic error rather than a style choice. A function that suddenly appears in the wrong section of a file, or an import that breaks the three-group convention, warrants investigation.

Third, they enforce *boundary discipline*: compilers succeed or fail at phase boundaries, and consistent conventions make those boundaries visible and verifiable. If every evaluator function starts with `eval_` and every parser function starts with `parse_`, a call from `eval_binary_op` to `parse_expression` is immediately suspicious.

There is a productive tension between convention and pragmatism. These guidelines represent defaults, not absolutes. A grammar module that naturally exceeds normal length limits is not improved by artificial splitting. A hot-path function that benefits from an unusual structure should not be contorted to match a template. The goal is that deviations from convention are *deliberate and documented*, never accidental or lazy. When the convention and the code disagree, one of them is wrong; determining which requires judgment, not rules.

The guidelines in this appendix are organized roughly in order of impact: file-level organization first (because it affects every file), then code-level patterns (because they affect every function), then cross-cutting concerns like documentation and synchronization (because they affect every change).

One final note on scope: these guidelines apply to the compiler implementation, written in Rust. They do not govern the Ori language itself, which has its own style conventions defined in the specification and enforced by the formatter. The distinction matters because compiler code and language code have different constraints, different audiences, and different lifetimes. Compiler code is read by compiler developers; language code is read by language users. The standards that serve one audience may not serve the other.

## E.2 File Organization

Source files have a hard limit of 500 lines, excluding tests. This constraint is not arbitrary: files beyond this threshold consistently correlate with tangled responsibilities, difficult navigation, and merge conflicts. The limit exists to force decomposition *before* a file becomes unmanageable, not after.

Every source file follows a canonical ordering:

1. **Module documentation** (`//!` doc comments)
2. **Module declarations** (`mod` and `pub mod`)
3. **Imports**, organized in three groups separated by blank lines:
   - External crates (alphabetical)
   - Internal crates (`crate::`, grouped by module)
   - Relative imports (`super::`, local)
4. **Type aliases**
5. **Type definitions** (structs, enums)
6. **Inherent `impl` blocks** (immediately after the type they implement)
7. **Trait implementations**
8. **Free functions**
9. **`#[cfg(test)] mod tests;`** declaration at the bottom (declaration only; body in a separate file)

This ordering means that a reader scanning top-to-bottom encounters the module's purpose, its dependencies, its types, its behavior, and finally its verification, in that order. The consistency matters more than any particular ordering choice: once internalized, it eliminates the "where is this defined?" question entirely.

When a file approaches the 500-line limit, split it into submodules before it exceeds. The parent `mod.rs` becomes a dispatch hub: re-exports, delegation, and brief orchestration. Logical cohesion determines the split boundaries, not arbitrary line counts. A file containing closure compilation, operator dispatch, and method resolution has three natural submodules regardless of its current line count. The `scripts/extract_tests.py` utility automates the mechanical work of moving test blocks into sibling files, handling module declarations and import adjustments.

The inherent `impl` block placement deserves emphasis: it appears immediately after the type definition, not at the end of the file or in a separate module. When a reader encounters a struct, the next thing they see is how to construct and use it. This locality principle reduces the navigation required to understand a type's API.

A common question is when to convert a single file into a directory module. The threshold is not purely about line count. A file should become a directory when it contains two or more logically independent subsystems that would benefit from independent test suites, or when the file's match arms naturally cluster into groups that could be tested and evolved separately. The conversion is mechanical: `foo.rs` becomes `foo/mod.rs`, and extracted subsystems become `foo/bar.rs` with `mod bar;` in `mod.rs`.

The 500-line limit is enforced during code review, not by tooling. The reason is that some files genuinely need temporary exemptions during mid-refactor states. However, touching a file that already exceeds 500 lines without reducing its size is itself a finding: the act of working in an oversized file is the right time to split it, because the developer already has the context needed to choose good split boundaries.

## E.3 Test File Placement

Tests live in sibling files, never inline. The source file contains only a `#[cfg(test)] mod tests;` declaration at its bottom; the test body resides in a separate file. This separation keeps production code readable and allows test suites to grow without inflating source files toward the 500-line limit. It also means that test code is in predictable locations: given any source file, its tests can be found without searching.

The placement follows a deterministic pattern:

- `foo.rs` places tests in `foo/tests.rs`
- `bar/mod.rs` places tests in `bar/tests.rs`
- `lib.rs` or `main.rs` places tests in `tests.rs` in the same directory

Test files follow their own internal structure. A module doc comment explains what is being tested. Imports come next. Then categorized sections using nested modules group related cases:

```rust
//! Tests for type inference on binary expressions.

use super::*;

mod arithmetic {
    use super::*;

    #[test]
    fn infer_int_addition() {
        // Arrange
        let expr = parse("1 + 2");
        // Act
        let ty = infer(&expr);
        // Assert
        assert_eq!(ty, Type::Int);
    }
}

mod comparison {
    use super::*;
    // ...
}
```

Each test follows the AAA pattern (Arrange, Act, Assert) with clear separation between setup, execution, and verification. This structure makes individual test failures immediately interpretable: the arrange block shows the input, the act block shows what was called, and the assert block shows what was expected. When a test fails in CI, the three sections tell the story without requiring the reader to untangle interleaved setup and assertions.

Test naming is descriptive and specific. `test_parses_nested_generics_with_bounds` communicates intent; `test1` does not. Error cases use suffixes like `_error`, `_invalid`, or `_fails`. Related tests group into nested modules so that `cargo test -- arithmetic` runs just that cluster without running the entire suite.

Every test suite must cover four dimensions: happy path (normal operation), edge cases (boundaries, empty inputs, zero values), error conditions (invalid input, overflow, type mismatches), and round-trip operations where applicable. A test suite that only covers the happy path provides false confidence: it verifies that the code works when everything goes right, which is the situation that least needs verification.

For bug fixes, tests are written *before* the fix and verified to fail. This discipline serves two purposes: it confirms that the test actually exercises the bug (a test that passes before the fix is testing the wrong thing), and it creates a regression guard that prevents the bug from recurring. The sequence is: consult the specification, write multiple failing tests (the exact case, edge cases, and related variations), verify they fail, fix the code, verify the tests pass without modification. If the tests need modification after the fix, either the tests or the fix is wrong. Writing multiple tests rather than a single test forces deeper understanding of the bug's scope and catches fixes that are too narrow.

Reference test suites from established languages can inform test case selection. Go's `strconv` tests for type conversions, Rust's `std` tests for collection operations, and the IEEE 754 specification for floating-point behavior all provide battle-tested edge cases that are worth adapting.

## E.4 Import Organization

Imports are organized into three groups, each separated by a blank line, each sorted alphabetically within the group:

```rust
// External crates
use salsa::Database;
use std::collections::HashMap;

// Internal crates
use ori_ir::{ExprId, Name};
use ori_patterns::Value;

// Relative imports
use crate::eval::Environment;
use super::helpers;
```

This grouping makes dependency direction visible at a glance. External crates appear first because they are the most stable and least likely to change. Internal crates (workspace siblings) appear next, representing the most important architectural dependencies: these are the cross-crate boundaries where phase discipline matters most. Relative imports appear last because they represent the most local, most volatile dependencies. When reviewing a file's imports, the three groups answer "what does this module depend on?" at three different scales: ecosystem, project, and module.

The alphabetical ordering within groups is not about aesthetics; it is about merge conflicts. When two branches both add an import to the same group, alphabetical ordering minimizes the chance that they touch the same line. It also makes it trivial to detect duplicate imports during review.

Within a single `use` statement, imported items are also alphabetized: `use ori_ir::{ExprId, Name, Span, TypeId};`. When a braced import list exceeds three or four items, consider whether the file is depending on too much of the imported crate, which may indicate a boundary violation.

## E.5 Impl Block Method Ordering

Methods within an `impl` block follow a consistent ordering that mirrors the lifecycle of a value:

1. **Constructors**: `new`, `with_*`, `from_*` -- how to create a value
2. **Accessors**: getters, `as_*` conversions -- how to inspect a value
3. **Predicates**: `is_*`, `has_*`, `can_*` -- how to query a value's state
4. **Public operations**: the module's primary behavior -- how to use a value
5. **Conversion and consumption**: `into_*`, `to_*` -- how to transform or consume a value
6. **Private helpers**: grouped by which public method calls them

This ordering means that the most common operations (creating a value, inspecting it, using it) appear near the top of the impl block. A reader looking for the constructor does not scroll past hundreds of lines of internal helpers.

Private helpers cluster at the bottom, grouped not alphabetically but by call order: the helpers used by the first public method appear before the helpers used by the second. Reading a public method and then scrolling down reveals its implementation helpers in the sequence they are invoked. A reader tracing control flow never has to scroll upward.

For trait implementations, methods appear in the order the trait defines them. This makes it easy to verify that all required methods are present by comparing the impl block against the trait definition.

When a type has both an inherent impl and multiple trait impls, the inherent impl appears first (immediately after the type definition), followed by trait impls in a consistent order: standard library traits (`Debug`, `Display`, `Clone`, `PartialEq`, `Hash`) before project-specific traits, sorted alphabetically within each group. This ordering is predictable without memorization.

The distinction between inherent methods and trait methods has API implications. Inherent methods are the type's own interface; trait methods are how the type participates in a protocol. A method like `to_string()` that serves both purposes should be a trait implementation, not an inherent method, because trait-based dispatch enables generic programming. Inherent methods that shadow trait methods create confusion and should be avoided.

## E.6 Naming Conventions

Names encode structural information through consistent conventions.

**Types** use PascalCase: `TypeChecker`, `ParseResult`, `InferEngine`. Type parameters use single uppercase letters (`T`, `E`, `K`, `V`) for standard generic roles, or short PascalCase (`Ctx`, `Db`) when the role needs clarification beyond what a single letter conveys.

**Functions** use snake_case with verb prefixes that signal which compiler phase they belong to: `parse_*` for parsing, `eval_*` for evaluation, `check_*` for type checking, `cook_*` and `eat_*` for lexer consumption, `skip_*` for advancing past tokens. These prefixes are not decorative; they make phase boundaries visible in call stacks, grep results, and profiler output. A function named `eval_binary_op` is immediately locatable within the evaluator; a function named `binary_op` could be anywhere.

**Constants** use SCREAMING_SNAKE_CASE: `MAX_RECURSION_DEPTH`, `DEFAULT_ARENA_CAPACITY`. Module-level constants that are not `pub` still use this convention for visual distinction from local bindings.

**Modules** use snake_case and reflect their content: `type_registry`, `error_codes`, `method_dispatch`. Module names should be greppable and unambiguous across the workspace.

**Variables** scale with scope. In a three-line closure, `n` is clear. In a fifteen-line loop body, `idx` or `name` suffices. In a function spanning dozens of lines, `current_expression` or `resolved_type` prevents ambiguity. The principle: a variable's name length should be proportional to the distance between its definition and its last use. A variable used once on the next line needs less context than one used twenty lines later in a different conditional branch.

**Abbreviations** are acceptable when they are universal within the domain. `expr` for expression, `ty` for type, `decl` for declaration, `stmt` for statement, `ident` for identifier, and `op` for operator are all clear to any compiler developer. Abbreviations that are project-specific or ambiguous (does `ctx` mean parse context or type-checking context?) should be spelled out or prefixed.

**Error-related naming** follows its own conventions. Error types are suffixed with `Error`: `ParseError`, `TypeError`, `EvalError`. Error factory functions are named for the condition they describe: `undefined_variable`, `type_mismatch`, `division_by_zero`. Error codes use the project's code scheme (e.g., `E2029`) and are defined in a centralized registry. Diagnostic severity levels follow standard conventions: `error` for compilation failures, `warning` for suspicious but valid code, `note` for additional context attached to a primary diagnostic.

**Boolean methods** always read as questions: `is_empty()`, `has_errors()`, `can_unify()`. The naming should make the `true` case obvious without consulting the documentation. `is_valid()` is clear; `check()` returning a boolean is ambiguous (does `true` mean "check passed" or "check found problems"?).

## E.7 Function Size and Complexity

Functions target fewer than 50 lines and must not exceed 100. This is a structural constraint, not a formatting preference: functions beyond 100 lines almost always contain multiple responsibilities that should be separate. A 150-line function is not a function; it is a module that has not yet been extracted.

Match expressions receive particular attention because they are the primary dispatch mechanism in a compiler. No single match should contain more than 20 arms in one file. When arms share structure, they should be grouped into helper functions: three or more arms performing the same operation with different inputs is a signal to extract a helper that takes the varying part as a parameter. Large match expressions in compilers tend to grow monotonically as the language gains features; without active management, they become the longest and least reviewable parts of the codebase.

The derive-versus-manual decision follows a simple rule: derive when the standard behavior is correct, implement manually only when behavior genuinely differs from the derived version. Unnecessary manual implementations of `Debug`, `Clone`, or `PartialEq` are maintenance burdens that drift from the types they describe as fields are added or reordered. A manual `Debug` impl that omits a field is a debugging trap that may persist for months before someone notices the missing information.

When decomposing a large function, prefer extracting named helpers over deeply nested closures. A closure captures its environment implicitly; a function receives its inputs explicitly. The explicit version is easier to test in isolation and easier to understand when reading the parent function.

Cyclomatic complexity is a useful secondary metric. A function with many nested conditionals or loops is harder to test exhaustively than one with a flat structure. When a function requires more than half a dozen test cases to cover all branches, it is a candidate for decomposition regardless of its line count.

Guard clauses reduce nesting and make the happy path prominent. Instead of:

```rust
fn process(value: &Value) -> Result<Output, Error> {
    if let Value::Int(n) = value {
        if n > 0 {
            // ... actual logic ...
        } else {
            Err(negative_value())
        }
    } else {
        Err(expected_int())
    }
}
```

Prefer early returns that handle error cases first:

```rust
fn process(value: &Value) -> Result<Output, Error> {
    let Value::Int(n) = value else {
        return Err(expected_int());
    };
    if n <= 0 {
        return Err(negative_value());
    }
    // ... actual logic at top indentation level ...
}
```

The second version makes the normal flow obvious: after the guards, every remaining line is the happy path. This pattern is especially valuable in compiler code where functions often handle a dozen error conditions before reaching the core logic.

Nesting depth is itself a code smell. If a function body requires more than three or four levels of indentation, it should be restructured: extract the inner block into a helper function, use guard clauses to eliminate nesting, or restructure the control flow to use early returns. Deep nesting hides the relationship between conditions and makes it difficult to verify that all paths are handled correctly.

## E.8 Error Handling

Error handling in a compiler is not an edge case; it is the primary user interface. Most compilation attempts fail, especially during development. The compiler's job is to explain what went wrong and how to fix it. The quality of error messages determines whether a language is pleasant or hostile to use.

**`Result<T, E>`** is for recoverable situations: malformed user input, missing files, type mismatches. The compiler accumulates all errors in a single pass rather than bailing on the first failure, because a user who fixes one error only to discover the next is a user whose time has been wasted.

**`panic!`** is for invariant violations: conditions that indicate a bug in the compiler itself, not in user code. A panic should never be reachable through any sequence of valid or invalid user inputs. If a user can trigger a panic, that is a compiler bug regardless of how unusual the input.

**`unreachable!()`** marks code paths that are logically impossible given prior control flow. If reached, it signals a compiler bug and should include a message explaining what invariant was violated.

Each compiler phase defines its own error types. Lexer errors are not parse errors; parse errors are not type errors. Phase-scoped error types make it impossible to accidentally conflate failures from different stages. They also enable phase-appropriate recovery strategies: the parser can attempt synchronization after a syntax error, while the type checker accumulates diagnostics and continues checking subsequent declarations.

Error factory functions are marked `#[cold]` to keep error construction off the hot path. The `#[cold]` attribute tells the optimizer that this branch is unlikely, allowing it to optimize the surrounding code for the common (non-error) case:

```rust
#[cold]
fn undefined_variable(name: &str, span: Span) -> TypeError {
    TypeError::new(format!("undefined variable: {name}"), span)
}
```

Error messages follow consistent conventions: start lowercase, be specific ("expected `)` after argument list" not "syntax error"), include context ("undefined variable: `count`"), and suggest fixes when possible ("did you mean `counter`?"). Every error carries a span. Spanless errors are compiler bugs: they produce messages that point nowhere, leaving the user to guess which part of their code is at fault.

Error accumulation requires careful design. The diagnostic queue must deduplicate errors (the same span should not produce the same message twice) and filter follow-on errors (a missing semicolon should not cascade into dozens of subsequent parse errors). Earlier errors take priority: when a lexer error and a parser error overlap the same span, the lexer error is shown because it is the root cause. Recovery after errors is explicit, using enum states like `Recovery::Allowed` rather than implicit booleans, so that the recovery policy is visible in the type signature.

The `ErrorGuaranteed` pattern provides type-level proof that at least one error has been reported. Functions that can only be called after an error return `ErrorGuaranteed` as a zero-sized token; downstream code that receives this token knows it can safely produce degraded output without reporting additional errors. This prevents the common problem of error cascades, where a single root cause produces dozens of follow-on diagnostics that obscure the original issue.

## E.9 Performance Patterns

Compiler performance is dominated by allocation pressure and memory access patterns, not algorithmic cleverness. The following patterns address the bottlenecks that matter in practice.

**Minimize allocations in hot paths.** Reuse buffers across iterations. Pre-allocate arenas with reasonable estimates (`source_len / 20` for expressions, `source_len / 2` for tokens). Avoid `String::from()`, `Vec::new()`, or `Box::new()` per token in the lexer-to-parser boundary. A compiler that allocates per-token will spend more time in the allocator than in compilation logic.

**Avoid unnecessary clones.** Borrow when the callee does not need ownership. When a function signature takes `Value` by value, every caller must clone; taking `&Value` removes that cost. Clone only when ownership transfer or mutation genuinely requires it. In an arena-based architecture, most data is borrowed from the arena; cloning should be the exception that requires justification.

**Prefer iterators over indexing.** `for item in items.iter()` lets the compiler elide bounds checks and apply vectorization. `for i in 0..items.len() { &items[i] }` introduces a bounds check per iteration that the optimizer often cannot remove because it cannot prove the slice is not modified between iterations.

**Eliminate quadratic patterns.** Linear scans through collections that grow with input size are the most common performance regression in compilers. Replace with hash-based lookups. `O(n^2)` in the type checker means compile time grows quadratically with program size. Users with large programs notice immediately.

**Mark hot functions for inlining.** Small functions called across crate boundaries benefit from `#[inline]`. This is particularly important for parser index operations and lexer token classification, where the function call overhead can exceed the function body cost. Benchmark before and after: `#[inline]` on cold functions increases code size without benefit and can worsen instruction-cache performance.

**Guard recursion.** Recursive descent through deeply nested ASTs can overflow the stack. Use `ensure_sufficient_stack` to check remaining stack space before recursive calls. Mark panicking accessors with `#[track_caller]` so that stack traces point to the call site rather than the accessor implementation, which is essential for diagnosing which part of the AST triggered the overflow.

**Use arenas for phase-local data.** Each compiler phase should allocate its temporary data structures in a phase-scoped arena that is freed when the phase completes. This prevents data from one phase leaking into the next, and it means that the cost of individual deallocations is replaced by a single bulk free at phase boundaries. Tokens, AST nodes, and intermediate type representations are all candidates for arena allocation.

**Profile before optimizing.** The `ORI_LOG` tracing infrastructure and the diagnostic scripts provide low-cost ways to identify bottlenecks. Optimization effort spent on code that accounts for 1% of compilation time is wasted regardless of how elegant the optimization is.

**Intern frequently compared strings.** Identifier comparison is one of the most common operations in a compiler. Comparing interned `Name` values is an integer comparison (one instruction); comparing `String` values requires traversing both strings (O(n) instructions). Every identifier that is looked up, matched, or stored in a hash map should be interned. The interning cost is paid once at creation; the comparison savings are paid at every use. As a rule of thumb, any string that will be compared more than once should be interned; any string that will only be displayed (error messages, formatted output) should not.

**Avoid `Arc` in hot paths.** Atomic reference counting involves cache-line contention on the reference count field. In single-threaded compiler phases (which is most of them), `Rc` or arena borrowing is cheaper. Reserve `Arc` for data that genuinely crosses thread boundaries, such as shared state in a parallel type-checking pass or cached results in an incremental compilation system.

## E.10 Type Safety

The type system is the first line of defense against phase-confusion bugs. Compiler code manipulates dozens of distinct identifier namespaces (expressions, types, names, tokens, spans), and mixing them up produces bugs that are difficult to diagnose because the values are all integers that look plausible.

**Newtypes for all identifiers.** `ExprId`, `Name`, `TypeId`, and `TokenIndex` are distinct types wrapping the same underlying integer. This makes it impossible to pass an expression ID where a type ID is expected, a class of bug that raw `u32` silently permits. The small syntactic cost of `.0` or `.into()` at boundaries is repaid by every bug that is caught at compile time rather than at runtime.

**Builder pattern for complex construction.** When a type requires more than three or four configuration values, a builder with `#[must_use]` on each setter method prevents partially-configured instances from escaping:

```rust
#[must_use]
pub fn max_recursion(mut self, depth: usize) -> Self {
    self.max_recursion = depth;
    self
}
```

The `#[must_use]` attribute ensures that the caller does not accidentally discard the builder. Without it, `builder.max_recursion(100);` silently drops the configured builder and uses the original, a bug that produces no compiler warning and no runtime error, only incorrect behavior.

**Arena-based identity.** In an arena-allocated system, equality and identity are distinct concepts. Two `ExprId` values that point to structurally identical expressions are not necessarily the same expression: they may appear at different source locations with different types. Conversely, the same `ExprId` always refers to the same expression. Comparison operations should use IDs (cheap integer comparison) rather than structural equality (expensive recursive comparison) whenever identity semantics are sufficient.

**Exhaustive matching.** Wildcard patterns (`_`) in match expressions hide new variants. When an enum gains a variant, every match on that enum should be forced to consider the new case. Wildcards are acceptable only as a deliberate catch-all after all known variants are handled, and should carry a comment explaining why exhaustive enumeration is not appropriate for this specific match.

**Checked conversions.** Use `try_from` instead of `as` for numeric conversions. `as` silently truncates or wraps; `try_from` makes the failure explicit. When a conversion is known to be safe, document why: `usize::try_from(n).unwrap_or(usize::MAX)` with a comment explaining the fallback. Bare `unwrap` on a conversion is a panic waiting for a sufficiently large input.

**Salsa compatibility.** Types that participate in Salsa's incremental computation framework must derive `Clone`, `Eq`, `PartialEq`, `Hash`, and `Debug`. They must not contain `Arc<Mutex<T>>`, function pointers, `dyn Trait` objects, or any other types that break determinism. Salsa requires that the same inputs always produce the same outputs; non-deterministic types (random values, timestamps, system state) are prohibited in query signatures. Functions tracked by Salsa should use manual `tracing::debug!()` events rather than `#[tracing::instrument]`, because the instrument attribute interacts poorly with Salsa's macro expansion.

## E.11 Visibility and API Design

Default to private. Expose `pub(crate)` for cross-module access within a crate. Reserve `pub` for the crate's external API. Every public item is a commitment that constrains future evolution; every private item is free to change without coordination. The distinction between `pub` and `pub(crate)` matters: `pub` crosses the crate boundary and becomes part of the inter-crate contract, while `pub(crate)` allows internal reorganization without breaking downstream consumers.

**Config structs replace long parameter lists.** When a function takes more than three or four parameters, bundle them into a named struct. This eliminates argument-order bugs (did `width` come before `height`?) and makes call sites self-documenting through field names. It also makes adding optional parameters a backward-compatible change: add a field with a default.

**Enums replace boolean flags.** `mode: CompileMode::Debug` is self-explanatory at every call site; `debug: true` requires reading the function signature to understand what `true` means. Enums also extend gracefully when a third option appears; a boolean does not, and converting a boolean to an enum later requires changing every call site.

**Return iterators, not `Vec`.** A function returning `impl Iterator<Item = T>` lets callers choose whether to collect, filter, or take a prefix. A function returning `Vec<T>` forces allocation even when the caller only needs the first element or plans to filter immediately. The iterator version is also lazy: it does no work until iterated.

**RAII guards for context management.** Push-pop patterns (entering a scope, enabling a mode, acquiring a resource) should use guard types whose `Drop` implementation restores the previous state. This eliminates the class of bugs where an early return, a `?` propagation, or a panic during unwinding skips the cleanup step.

**Dispatch strategy.** Use enums for fixed sets of variants (expression types, token kinds, operator categories) because they provide exhaustive matching and static dispatch. Use `dyn Trait` only for user-extensible extension points where the set of implementations is not known at compile time. The cost hierarchy is `&dyn Trait` (cheapest, borrowed) < `Box<dyn Trait>` (owned, heap-allocated) < `Arc<dyn Trait>` (shared, atomic reference counting). Choose the cheapest level that satisfies the ownership requirements.

**IO isolation.** Only the top-level orchestration crate (`oric`) performs IO: reading files, writing output, interacting with the file system. All core crates (parser, type checker, evaluator) are pure: they take input data and produce output data, with no side effects. This isolation makes core crates testable without file system fixtures and ensures that the compiler can be embedded in other tools (LSP servers, build systems) without IO conflicts.

**Continuous improvement.** When working in any file, fix all issues encountered: dead code, unclear names, duplicated logic, missing documentation. There is no boundary between "your code" and "other code." If it is broken, fix it. If it is messy, clean it. If it has drifted from the conventions described in this appendix, bring it back into alignment. The cost of fixing a problem when you already have the context is far lower than the cost of returning to fix it later, when the context must be rebuilt from scratch.

## E.12 Clippy Compliance

The codebase must pass `cargo clippy --workspace -- -D warnings` with pedantic lints enabled. Clippy warnings are not suggestions; they are requirements.

When a lint fires on code that is genuinely correct, suppress it with `#[expect(clippy::lint_name, reason = "...")]` rather than `#[allow(...)]`. The `#[expect]` attribute is verified by the compiler: if the suppressed lint stops firing because the underlying code changed, the compiler emits a warning about the now-unnecessary suppression. This prevents stale suppressions from accumulating silently across the codebase.

Bare `#[allow(clippy::...)]` without a reason string is prohibited. Cargo.toml-level lint disabling is prohibited. Every suppression must explain *why* the lint does not apply to this specific case, because the next person reading that code needs to know whether the suppression is still valid or whether it is masking a real problem.

Common pedantic lints and their proper resolutions:

| Lint | Resolution |
|------|------------|
| `cast_precision_loss` | Use checked conversions or lossless conversion paths |
| `cast_sign_loss` | Validate non-negative before cast, or use `unsigned_abs()` |
| `float_cmp` | Use `partial_cmp()` for IEEE 754 semantics |
| `too_many_arguments` | Extract a parameter struct |
| `match_same_arms` | Merge with or-patterns |
| `arithmetic_side_effects` | Use `saturating_*`, `checked_*`, or `wrapping_*` operations |

Note that certain operations do not trigger the lints they appear to trigger. `saturating_sub` and `.min()` do not trigger `arithmetic_side_effects` because they cannot overflow. `unsigned_abs()` does not trigger `cast_sign_loss` because the result is always non-negative. Understanding which operations are safe by construction avoids unnecessary suppressions.

The `./clippy-all.sh` script runs clippy across the entire workspace and should pass before any commit. When a new clippy version introduces a new lint that fires on existing code, the response is to fix the code or add a justified suppression, not to pin the clippy version or disable the lint globally.

The full pre-commit verification sequence is: `./fmt-all.sh` (formatting), `./clippy-all.sh` (linting), `./test-all.sh` (tests). All three must pass. Running them in this order catches the cheapest problems first: formatting issues are fixed in seconds, lint violations in minutes, and test failures may require deeper investigation.

## E.13 Documentation Standards

Every source file begins with a `//!` module doc comment explaining the module's purpose and its role within the compiler. Every public item carries a `///` doc comment. These are not optional courtesies; they are part of the code's specification.

Documentation explains *why*, not *what*. The function signature already says what the function does; the doc comment explains the design decision, the invariant being maintained, or the spec section being implemented:

```rust
/// Resolve method calls on iterator adaptors.
///
/// Adaptors like `map` and `filter` wrap an inner iterator.
/// Resolution delegates to the inner iterator's type for `Item`
/// inference. Per spec section 14.8, adaptor chains preserve
/// laziness: no element is computed until `next()` is called.
```

Reference spec sections explicitly: `// Per spec section 10.3: try unwraps Result/Option`. This creates a traceable link between implementation and specification, so that when the spec changes, every affected implementation can be located by searching for the section number. The link runs both directions: a spec author can search the codebase to find which code implements a given section, and a code author can consult the spec to verify their implementation.

No decorative banners (lines of `=` or `-`), no commented-out code, no `TODO` without an associated tracking mechanism. Dead comments are worse than no comments: they erode trust in all comments. If code is removed, delete it; version control preserves history. If work is deferred, track it in the planning system, not in a source comment that nobody searches.

For error documentation, use the `# Errors` section in doc comments to enumerate the conditions under which a function returns `Err`. For panic documentation, use the `# Panics` section to document invariant violations. These sections are not boilerplate; they are the contract between the function and its callers. A caller cannot handle errors it does not know about.

Inline comments within function bodies should be reserved for non-obvious logic: why a particular order of operations matters, why a seemingly simpler approach does not work, or what specification rule governs a particular branch. Comments that restate the code ("increment counter") add visual noise without information.

Tracing instrumentation is a form of documentation that is also executable. Public API entry points should carry `#[tracing::instrument]` with appropriate `skip` attributes for large or non-`Debug` arguments. This means that enabling `ORI_LOG=debug` produces a useful narrative of the compiler's execution without any additional effort. The tracing output serves as both a debugging tool and a specification of the compiler's call structure: if the trace does not match the expected sequence of phases, something is wrong.

## E.14 Single Source of Truth

When the same fact must appear in multiple locations, one location is the source of truth and all others are derived or validated against it. This principle prevents the most insidious category of compiler bugs: silent divergence between components that must agree.

The `DerivedTrait` enum in `ori_ir` is the canonical example. It defines the set of traits that can be derived. Four consuming crates (`ori_types`, `ori_eval`, `ori_llvm`, and `library/std`) must each handle every variant. Adding a variant to the enum without updating all four consumers is a synchronization failure that may compile successfully but produce incorrect behavior at runtime, because each consumer has its own match expression that silently falls through to a default case.

The defense against drift is *mechanical enforcement*. Shared methods like `from_name()` and `all_variants()` on the source-of-truth type allow consuming crates to iterate the full set. Tests in each consumer verify coverage against the source:

```rust
#[test]
fn all_derived_traits_handled() {
    for (name, _) in DerivedTrait::all_variants() {
        assert!(
            handles_trait(name),
            "unhandled derived trait: {name}"
        );
    }
}
```

When centralization is not possible because consuming crates add phase-specific behavior, tests iterate the source-of-truth list and assert that every variant is handled. The test fails the moment a new variant is added to the source without updating the consumer. This turns a runtime surprise into a CI failure.

Manual mirroring, where a developer is expected to remember to update parallel lists in separate crates, is not a synchronization strategy. It is a bug waiting for a sufficiently busy afternoon.

The principle extends beyond enums. Centralized registries that are validated by tests (such as `ori_registry::BUILTIN_TYPES`, which declares every built-in type and its methods in a single location consumed by the type checker, evaluator, and LLVM backend) use test-time enforcement to prevent drift between consumers. Error code registries, operator precedence tables, and keyword lists all follow the same pattern: one authoritative location, mechanical validation everywhere else.

When adding a new cross-cutting feature (a new operator, a new derived trait, a new built-in function), the first step is to identify all the synchronization points. The second step is to update them all atomically, in a single commit that touches every affected location. The third step is to verify that the synchronization tests pass. If no synchronization test exists for the category of change being made, writing one is part of the feature work, not a follow-up task. A feature that adds a new variant without adding a synchronization test is incomplete.

## E.15 Prior Art

These guidelines draw from established standards in systems programming and compiler engineering:

- [**Rust API Guidelines**](https://rust-lang.github.io/api-guidelines/) define the conventions for Rust library APIs, including naming, documentation, and type safety patterns that this document adapts for compiler internals. The naming conventions (E.6) and visibility rules (E.11) follow Rust API Guidelines closely.
- [**Google C++ Style Guide**](https://google.github.io/styleguide/cppguide.html) demonstrates how a large-scale codebase maintains consistency across thousands of contributors, particularly its treatment of naming conventions and header organization. The import grouping convention (E.4) draws from Google's include ordering.
- [**LLVM Coding Standards**](https://llvm.org/docs/CodingStandards.html) address compiler-specific concerns including pass structure, error handling, and the performance constraints unique to compiler infrastructure. The performance patterns (E.9) and error factory conventions (E.8) reflect LLVM's approach.
- [**Linux Kernel Coding Style**](https://www.kernel.org/doc/html/latest/process/coding-style.html) takes an opinionated stance on function length, indentation, and naming that has proven effective across decades and millions of lines of systems code. The function size limits (E.7) are adapted from the kernel's eight-indent-level heuristic translated to Rust's conventions.

No single external standard maps perfectly onto a Rust-based compiler project. The value of studying prior art is not to copy rules but to understand the *reasoning* behind them, then adapt that reasoning to the specific constraints of the project at hand.

Several language-specific compiler projects also merit study for their approach to code organization and contributor guidance. The [Rust compiler development guide](https://rustc-dev-guide.rust-lang.org/) documents how `rustc` manages its own internal complexity across hundreds of crates and provides detailed guidance on diagnostic quality, query system design, and test infrastructure. The [GHC commentary](https://gitlab.haskell.org/ghc/ghc/-/wikis/commentary) provides insight into how a mature functional compiler handles pass structure and intermediate representations. These resources are particularly useful when the general-purpose style guides do not address compiler-specific concerns like arena allocation strategies, incremental computation, or error recovery.

The [Zig style guide](https://ziglang.org/documentation/master/#Style-Guide) is notable for its brevity and its emphasis on measurable properties (line length, function size) over subjective qualities. The Gleam compiler's codebase demonstrates how a relatively small team can maintain high consistency through strong conventions and comprehensive test coverage. Studying multiple approaches reveals that the specific conventions matter less than the consistency with which they are applied.

What all of these sources share is a recognition that coding standards are not bureaucracy imposed on engineering but engineering applied to the problem of sustainable collaboration at scale. The standards exist so that the codebase can grow without the complexity growing faster. A compiler that cannot be maintained cannot be improved, and a compiler that cannot be improved will eventually be replaced.
