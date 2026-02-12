---
plan: "dpr_pattern-matching_02112026"
title: "Design Pattern Review: Pattern Matching"
status: draft
---

# Design Pattern Review: Pattern Matching

## Ori Today

Ori's pattern matching is distributed across four compiler phases. Parsing (`ori_parse`) produces `MatchPattern` AST nodes with 10 variants (Wildcard, Binding, Literal, Variant, Struct, Tuple, List, Range, Or, At). Canonicalization (`ori_canon::patterns::compile_patterns`) flattens these into `FlatPattern` values (owned, arena-free), builds a `PatternMatrix` (one row per arm, one column per scrutinee), and compiles it to a `DecisionTree` via the Maranget (2008) algorithm implemented in `ori_arc::decision_tree::compile::compile()`. The type checker (`ori_types`) resolves ambiguous `Binding` patterns to unit variants through the `PatternKey`/`PatternResolution` bridge stored in `TypedModule::pattern_resolutions`. The evaluator (`ori_eval::exec::decision_tree::eval_decision_tree`) walks the pre-compiled tree against runtime `Value`s, using a callback closure for guard evaluation.

The existing infrastructure has real strengths. The four-phase separation keeps concerns clean: the decision tree is compiled once during canonicalization and consumed immutably by both the interpreter and the LLVM backend. The `FlatPattern` representation is self-contained (no arena references), which makes the Maranget algorithm testable in isolation -- the `ori_arc::decision_tree::compile` module has thorough unit tests for bool, int, enum, tuple, struct, list, or-pattern, guard, and multi-column matching. The `ScrutineePath`/`PathInstruction` system for navigating sub-values is well-designed, with ref-based resolution (`Resolved::Ref`/`Resolved::Owned`) avoiding unnecessary clones during evaluation. The `DecisionTreePool` with `SharedDecisionTree` (`Arc<DecisionTree>`) enables O(1) sharing between backends.

The critical gap is **exhaustiveness checking**. Today, compilation succeeds regardless of pattern coverage. If no arm matches at runtime, `eval_decision_tree` returns `EvalError("non-exhaustive match: no arm matched")`. The type checker has a `TypeErrorKind::NonExhaustiveMatch { missing: Vec<String> }` variant and the diagnostic infrastructure (`SemanticProblem::NonExhaustiveMatch`) is wired up, but no analysis actually populates these. There is no dead arm detection (unreachable patterns produce no warning). Pattern resolution for unit variants has a fragile fallback in `ori_canon::patterns::try_resolve_unit_variant` that scans all module type definitions when the scrutinee type is unresolved. Guard expressions receive no static analysis for side effects or termination. The decision tree compilation algorithm has no column reordering optimization beyond the "most distinct constructors" heuristic in `pick_column`.

## Prior Art

### Rust -- Constructor Matrix + Usefulness

Rust's pattern exhaustiveness checker (`rustc_pattern_analysis`) separates the algorithm from the type system via a `PatCx` trait, making the core ~5.2K lines backend-agnostic. Patterns are deconstructed into `Constructor + fields` pairs, and exhaustiveness is checked via a "usefulness" algorithm: a pattern is useful if there exists a value matched by it but not by any earlier pattern. The `Missing` constructor represents "all constructors not explicitly listed" without enumerating them, and constructor splitting handles infinite types (integer ranges, strings) by grouping overlapping ranges. Witness generation via `WitnessMatrix` produces human-readable counterexamples by reverse-applying constructors. The key tradeoff is complexity: the trait abstraction and constructor splitting logic are substantial, but they handle every edge case (nested or-patterns, opaque types, `#[non_exhaustive]`).

### Gleam -- Decision Trees via Maranget Algorithm

Gleam compiles patterns directly to decision trees with an explicit `RuntimeCheck` enum (15+ variants: `Int`, `Variant`, `StringPrefix`, `NonEmptyList`, etc.). Missing pattern extraction walks the compiled decision tree looking for `Decision::Fail` nodes, reconstructing the patterns that would reach them. This is architecturally simpler than Rust's matrix approach: the decision tree IS the exhaustiveness artifact, and missing patterns are read directly from failure paths. The `Decision::Guard` node explicitly encodes guard fallthrough. Gleam's approach works well for a language with algebraic types and no open type hierarchies -- close to Ori's type system.

### Elm -- Simplified Patterns + Matrix Walk

Elm reduces all patterns to exactly three variants: `Anything | Literal | Ctor`. The exhaustiveness checker walks a pattern matrix using Maranget's algorithm in ~653 lines of Haskell, producing `Error::Incomplete` (with missing patterns) or `Error::Redundant` (with the redundant arm's source location). Built-in unions (unit, pair, triple, list) are handled as hardcoded `Ctor` constructors. Elm's extreme simplicity enables clear error messages and fast compilation. The limitation is that custom pattern types and open sets are not supported, but for a language with only closed algebraic types, this works perfectly.

## Proposed Best-of-Breed Design

### Core Idea

Ori should adopt a **two-pass architecture**: (1) compile patterns to decision trees via the existing Maranget algorithm (unchanged), then (2) analyze the compiled tree for exhaustiveness and redundancy. This follows Gleam's insight that the decision tree itself encodes all coverage information, while borrowing Rust's trait-based abstraction to make the analysis type-system-aware without coupling it to a specific type representation.

The exhaustiveness checker operates as a post-compilation pass in `ori_canon`, after `compile_patterns` produces a `DecisionTree` and before it is stored in `DecisionTreePool`. The checker walks the tree, collects all `Fail` leaves, and reconstructs the missing patterns that would reach them (following Gleam's `MissingPatternsGenerator`). Simultaneously, it detects arms that are never reachable in any tree path (following Elm's `Redundant` error). The results are returned alongside the tree and accumulated as diagnostics via the existing `TypeErrorKind::NonExhaustiveMatch` and a new `TypeErrorKind::RedundantPattern` variant.

### Key Design Choices

1. **Decision-tree-based analysis, not separate matrix walk** (inspired by Gleam). The Maranget algorithm already computes the optimal branching structure. Walking the compiled tree to find `Fail` nodes is simpler and more maintainable than re-running a parallel usefulness algorithm on the pattern matrix. This avoids duplicating the specialization logic that already exists in `ori_arc::decision_tree::compile`.

2. **Trait-based type information** (inspired by Rust's `PatCx`). The exhaustiveness checker needs to know "what constructors exist for this type?" to distinguish between "all constructors covered" (no default needed) and "infinite type" (default required). A `ConstructorSet` trait abstracts over Ori's `Pool`/`Tag` system, enabling the checker to query variant counts for enums, known values for booleans, and "infinite" for int/str/float without depending on `ori_types` internals.

3. **Three-tier constructor classification** (inspired by Elm's simplicity, refined by Rust's `Missing`). All Ori types fall into: (a) **Finite closed** -- `bool` (2 values), enum with N variants, unit; (b) **Finite open** -- list lengths with known patterns; (c) **Infinite** -- `int`, `str`, `float`, ranges. For finite closed types, missing constructors are enumerated. For infinite types, a default/wildcard arm is required, and the `Missing` pseudo-constructor (from Rust) represents "everything else" in diagnostics.

4. **Witness pattern reconstruction** (inspired by Gleam's `MissingPatternsGenerator` + Rust's `unspecialize`). When a `Fail` node is reached during tree analysis, the path from root to that node (the sequence of `Switch` tests and edges traversed) encodes exactly which pattern is missing. Reconstructing the `FlatPattern` from this path produces a concrete counterexample for the diagnostic message (e.g., "missing pattern: `None`" or "missing pattern: `Err(_)`").

5. **Reachability tracking via arm bitmap** (inspired by Elm's `Redundant` detection). During tree analysis, collect the set of `arm_index` values that appear in any `Leaf` or `Guard` node. Arms not in this set are unreachable. This is O(n) in tree size and produces precise diagnostics (the exact source span of the dead arm).

6. **Guard-aware exhaustiveness** (inspired by Gleam's `Decision::Guard`). Guards make exhaustiveness undecidable in general. Ori follows Rust's approach: treat guarded arms as **not covering** their pattern for exhaustiveness purposes. A `Guard` node's `on_fail` subtree must eventually reach a non-guard `Leaf` or the pattern is considered non-exhaustive. This is conservative but sound.

7. **Salsa-compatible result type** (Ori-specific). The analysis results (`missing_patterns`, `redundant_arms`) are stored as plain `Vec<String>` and `Vec<Span>` on a new `PatternAnalysis` struct that is `Clone + Eq + Hash + Debug`. This ensures the canonicalization query remains deterministic and cacheable.

8. **Incremental-friendly placement** (Ori-specific). The analysis runs inside `ori_canon::patterns::compile_patterns`, which is called per-function during canonicalization. Since canonicalization is a Salsa query, changed functions re-run analysis while unchanged functions use cached results. No separate "exhaustiveness pass" is needed.

### What Makes Ori's Approach Unique

**ARC memory and expression-based semantics create optimization opportunities none of the reference compilers exploit.** Because Ori has ARC (not GC), pattern matching interacts with ownership: destructuring a variant in a match arm can potentially reuse the variant's memory if the refcount is 1. The existing `ori_arc` crate already implements reset/reuse optimizations for ARC; extending this to match arms means the exhaustiveness checker can inform the ARC optimizer about which arms consume which sub-values, enabling in-place mutation of matched data (similar to Swift's `SILOptimizer/ARC` but integrated with the decision tree rather than added as a post-hoc optimization).

**Ori's mandatory test requirement creates a unique feedback loop with exhaustiveness.** Every function must have tests. If a match expression is non-exhaustive, the missing pattern is also a missing test case. The exhaustiveness checker can suggest not just "add pattern `None`" but "add a test case that exercises the `None` branch." This connects pattern coverage to test coverage in a way that Rust, Gleam, and Elm do not.

**Capability-based effects interact with guard expressions.** When a guard expression uses a capability (e.g., `if uses Http`), the exhaustiveness checker can distinguish between pure guards (statically analyzable) and effectful guards (must be treated conservatively). Pure boolean guards on enum discriminants could be promoted to additional Switch edges in the decision tree, eliminating the guard callback entirely.

### Concrete Types & Interfaces

```rust
// ---- ori_canon::exhaustiveness ----

/// Result of exhaustiveness and redundancy analysis on a compiled decision tree.
///
/// Stored alongside the `DecisionTree` in `CanonResult` for diagnostics.
/// All fields are Salsa-compatible: Clone + Eq + Hash + Debug.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PatternAnalysis {
    /// Is the match exhaustive (no `Fail` nodes reachable)?
    pub is_exhaustive: bool,
    /// Missing patterns as human-readable strings for diagnostics.
    /// Empty if exhaustive.
    pub missing_patterns: Vec<String>,
    /// Arm indices that are never reachable in any tree path.
    /// Empty if all arms are reachable.
    pub redundant_arms: Vec<usize>,
}

/// Trait abstracting type information needed for exhaustiveness checking.
///
/// Inspired by Rust's `PatCx` trait. Decouples the checker from `ori_types`
/// internals. Implementations query the `Pool` for variant counts, field
/// types, and constructor classification.
pub trait ConstructorInfo {
    /// How many constructors does this type have?
    /// Returns `ConstructorCount::Finite(n)` for enums/bool,
    /// `ConstructorCount::Infinite` for int/str/float.
    fn constructor_count(&self, test_kind: TestKind) -> ConstructorCount;

    /// Get all constructors for a finite type (enum variants, bool values).
    /// Returns `None` for infinite types.
    fn all_constructors(&self, test_kind: TestKind, path: &ScrutineePath)
        -> Option<Vec<TestValue>>;

    /// Format a test value as a human-readable pattern string for diagnostics.
    fn format_pattern(&self, test_value: &TestValue) -> String;

    /// Format a "missing wildcard" pattern for an infinite type.
    fn format_wildcard(&self, test_kind: TestKind) -> String;
}

/// Classification of constructor sets by finiteness.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConstructorCount {
    /// Exactly `n` constructors exist (enum variants, bool values).
    /// If all `n` are covered, no default is needed.
    Finite(usize),
    /// Infinitely many values (int, str, float, ranges).
    /// A default/wildcard arm is always required.
    Infinite,
}

/// Analyze a compiled decision tree for exhaustiveness and redundancy.
///
/// Called by `compile_patterns` after tree construction, before storage
/// in `DecisionTreePool`.
///
/// # Arguments
///
/// - `tree`: The compiled decision tree.
/// - `arm_count`: Total number of arms in the original match expression.
/// - `info`: Type information for constructor enumeration.
///
/// # Returns
///
/// `PatternAnalysis` with missing patterns and redundant arm indices.
pub fn analyze_exhaustiveness<I: ConstructorInfo>(
    tree: &DecisionTree,
    arm_count: usize,
    info: &I,
) -> PatternAnalysis {
    let mut reachable_arms = vec![false; arm_count];
    let mut missing = Vec::new();
    let path_context = Vec::new(); // accumulates (TestKind, TestValue) pairs

    walk_tree(tree, &path_context, &mut reachable_arms, &mut missing, info);

    let redundant_arms: Vec<usize> = reachable_arms
        .iter()
        .enumerate()
        .filter(|(_, &reached)| !reached)
        .map(|(i, _)| i)
        .collect();

    PatternAnalysis {
        is_exhaustive: missing.is_empty(),
        missing_patterns: missing,
        redundant_arms,
    }
}

/// Walk a decision tree, collecting reachable arms and missing patterns.
///
/// `path_context` accumulates the sequence of tests taken to reach the
/// current node. When a `Fail` node is reached, `path_context` describes
/// exactly which pattern is missing.
fn walk_tree<I: ConstructorInfo>(
    tree: &DecisionTree,
    path_context: &[(TestKind, Option<TestValue>)],
    reachable_arms: &mut [bool],
    missing: &mut Vec<String>,
    info: &I,
) {
    match tree {
        DecisionTree::Leaf { arm_index, .. } => {
            reachable_arms[*arm_index] = true;
        }

        DecisionTree::Guard {
            arm_index, on_fail, ..
        } => {
            // The guard arm is reachable (guard might pass).
            reachable_arms[*arm_index] = true;
            // The on_fail subtree is also reachable (guard might fail).
            walk_tree(on_fail, path_context, reachable_arms, missing, info);
        }

        DecisionTree::Switch {
            test_kind,
            edges,
            default,
            ..
        } => {
            // Walk each edge.
            for (tv, subtree) in edges {
                let mut ctx = path_context.to_vec();
                ctx.push((*test_kind, Some(tv.clone())));
                walk_tree(subtree, &ctx, reachable_arms, missing, info);
            }

            // Walk the default branch.
            if let Some(default_tree) = default {
                let mut ctx = path_context.to_vec();
                ctx.push((*test_kind, None)); // None = "everything else"
                walk_tree(default_tree, &ctx, reachable_arms, missing, info);
            } else {
                // No default branch. Check if the edges cover all constructors.
                let covered_count = edges.len();
                match info.constructor_count(*test_kind) {
                    ConstructorCount::Finite(total) if covered_count >= total => {
                        // All constructors covered -- no missing pattern.
                    }
                    _ => {
                        // Missing constructors exist. Reconstruct the missing pattern.
                        let pattern = reconstruct_missing_pattern(
                            path_context, *test_kind, edges, info,
                        );
                        missing.push(pattern);
                    }
                }
            }
        }

        DecisionTree::Fail => {
            // This node is reachable -- reconstruct what pattern is missing.
            let pattern = reconstruct_missing_from_context(path_context, info);
            missing.push(pattern);
        }
    }
}

/// Reconstruct a missing pattern description from the path context.
///
/// Each entry in `path_context` is a (TestKind, Option<TestValue>) pair
/// representing a test that was taken. `None` means "the default branch"
/// (i.e., "not any of the explicit edges").
fn reconstruct_missing_from_context<I: ConstructorInfo>(
    path_context: &[(TestKind, Option<TestValue>)],
    info: &I,
) -> String {
    if path_context.is_empty() {
        return "_".to_string();
    }

    // Build pattern string from innermost test outward.
    let mut parts = Vec::new();
    for (test_kind, test_value) in path_context {
        match test_value {
            Some(tv) => parts.push(info.format_pattern(tv)),
            None => parts.push(info.format_wildcard(*test_kind)),
        }
    }
    parts.join(", ")
}

/// Reconstruct the missing pattern for a Switch node with no default
/// that doesn't cover all constructors.
fn reconstruct_missing_pattern<I: ConstructorInfo>(
    path_context: &[(TestKind, Option<TestValue>)],
    test_kind: TestKind,
    edges: &[(TestValue, DecisionTree)],
    info: &I,
) -> String {
    // Get all constructors for this type.
    if let Some(all_ctors) = info.all_constructors(test_kind, &[]) {
        let covered: Vec<&TestValue> = edges.iter().map(|(tv, _)| tv).collect();
        let missing_ctors: Vec<&TestValue> = all_ctors
            .iter()
            .filter(|c| !covered.iter().any(|covered_tv| *covered_tv == c))
            .collect();

        let missing_strs: Vec<String> =
            missing_ctors.iter().map(|tv| info.format_pattern(tv)).collect();

        if path_context.is_empty() {
            missing_strs.join(" | ")
        } else {
            let prefix = reconstruct_missing_from_context(path_context, info);
            format!("{prefix} with {}", missing_strs.join(" | "))
        }
    } else {
        // Infinite type -- should have had a default.
        let prefix = reconstruct_missing_from_context(path_context, info);
        format!("{prefix}, {}", info.format_wildcard(test_kind))
    }
}


// ---- Pool-based ConstructorInfo implementation ----

/// Implementation of `ConstructorInfo` backed by `ori_types::Pool`.
///
/// Provides type information for exhaustiveness checking by querying the
/// type pool for enum variant counts, boolean completeness, etc.
pub struct PoolConstructorInfo<'a> {
    pool: &'a ori_types::Pool,
    interner: &'a ori_ir::StringInterner,
    /// Maps ScrutineePath → resolved type Idx for sub-scrutinees.
    /// Populated during pattern compilation.
    scrutinee_types: rustc_hash::FxHashMap<ScrutineePath, ori_types::Idx>,
}

impl<'a> ConstructorInfo for PoolConstructorInfo<'a> {
    fn constructor_count(&self, test_kind: TestKind) -> ConstructorCount {
        match test_kind {
            TestKind::BoolEq => ConstructorCount::Finite(2),
            TestKind::EnumTag => {
                // Look up the scrutinee type at the root path.
                // Enum variant count comes from the pool.
                // If we can't determine the type, treat as infinite (conservative).
                ConstructorCount::Infinite // Refined per-switch via scrutinee_types
            }
            TestKind::IntEq
            | TestKind::StrEq
            | TestKind::FloatEq
            | TestKind::IntRange
            | TestKind::ListLen => ConstructorCount::Infinite,
        }
    }

    fn all_constructors(
        &self,
        test_kind: TestKind,
        _path: &ScrutineePath,
    ) -> Option<Vec<TestValue>> {
        match test_kind {
            TestKind::BoolEq => {
                Some(vec![TestValue::Bool(true), TestValue::Bool(false)])
            }
            _ => None, // Enum constructors require per-switch type lookup
        }
    }

    fn format_pattern(&self, test_value: &TestValue) -> String {
        match test_value {
            TestValue::Tag { variant_name, .. } => {
                self.interner.lookup(*variant_name).to_string()
            }
            TestValue::Int(v) => v.to_string(),
            TestValue::Str(name) => {
                format!("\"{}\"", self.interner.lookup(*name))
            }
            TestValue::Bool(v) => v.to_string(),
            TestValue::Float(bits) => format!("{}", f64::from_bits(*bits)),
            TestValue::IntRange { lo, hi, inclusive } => {
                if *inclusive {
                    format!("{lo}..={hi}")
                } else {
                    format!("{lo}..{hi}")
                }
            }
            TestValue::ListLen { len, is_exact } => {
                if *is_exact {
                    format!("[_; {len}]")
                } else {
                    format!("[_; {len}..]")
                }
            }
        }
    }

    fn format_wildcard(&self, test_kind: TestKind) -> String {
        match test_kind {
            TestKind::EnumTag => "_".to_string(),
            TestKind::IntEq => "_".to_string(),
            TestKind::StrEq => "_".to_string(),
            TestKind::BoolEq => "_".to_string(),
            TestKind::FloatEq => "_".to_string(),
            TestKind::IntRange => "_".to_string(),
            TestKind::ListLen => "[..]".to_string(),
        }
    }
}
```

## Implementation Roadmap

### Phase 1: Foundation
- [ ] Create `ori_canon/src/exhaustiveness.rs` with `PatternAnalysis`, `ConstructorInfo` trait, and `ConstructorCount` enum
- [ ] Implement `analyze_exhaustiveness()` tree walker that collects reachable arms and detects `Fail` nodes
- [ ] Implement `PoolConstructorInfo` for `bool` exhaustiveness (simplest finite type: exactly 2 constructors)
- [ ] Add `PatternAnalysis` to `CanonResult` (alongside `DecisionTreePool`) for downstream consumption
- [ ] Wire `analyze_exhaustiveness()` into `compile_patterns()` -- call after `ori_arc::decision_tree::compile::compile()`, before `DecisionTreePool::push()`

### Phase 2: Core
- [ ] Extend `PoolConstructorInfo` to handle enum types: query `Pool::enum_variant_count()` for `TestKind::EnumTag`, return `ConstructorCount::Finite(n)` when scrutinee type resolves to `Tag::Enum`
- [ ] Extend `PoolConstructorInfo::all_constructors()` to enumerate enum variants (name + index) for missing pattern reconstruction
- [ ] Handle builtin Option/Result: `Tag::Option` has 2 constructors (None, Some), `Tag::Result` has 2 (Ok, Err)
- [ ] Implement missing pattern reconstruction (`reconstruct_missing_pattern`) producing human-readable strings like "None", "Err(_)", "Running"
- [ ] Implement redundant arm detection: compare reachable arm bitmap against total arm count, emit `TypeErrorKind::RedundantPattern` (new variant) with source span
- [ ] Add `TypeErrorKind::RedundantPattern { arm_span: Span }` to `ori_types::type_error::check_error`
- [ ] Wire diagnostics: emit `NonExhaustiveMatch` and `RedundantPattern` errors through Salsa accumulation in `ori_canon`
- [ ] Add conformance tests in `tests/spec/` for exhaustive match on bool, Option, Result, and user-defined enums
- [ ] Add conformance tests for non-exhaustive match error messages
- [ ] Add conformance tests for redundant arm warnings

### Phase 3: Polish
- [ ] Handle guard-aware exhaustiveness: guarded arms do not count as covering their pattern; verify `on_fail` subtree reaches non-guard Leaf
- [ ] Handle infinite types: int/str/float switches without a default emit `NonExhaustiveMatch` with `missing: ["_"]`
- [ ] Handle list pattern exhaustiveness: `[_]`, `[_, _]`, `[..]` coverage analysis
- [ ] Track `scrutinee_types` per Switch node by recording scrutinee type during `compile_patterns` (thread type info through the Maranget algorithm alongside `ScrutineePath`)
- [ ] Improve missing pattern formatting: nested patterns like "Some(None)" instead of flat "Some, None"
- [ ] Performance: avoid allocating `path_context` vectors per recursion by using a stack-based accumulator with `SmallVec<[(TestKind, Option<TestValue>); 8]>`
- [ ] Investigate ARC reuse opportunities: annotate `Leaf` nodes with consumed sub-values so `ori_arc` can emit `reset`/`reuse` instructions for destructured variants
- [ ] Connect exhaustiveness gaps to test suggestions: when a function has a non-exhaustive match, suggest a test case that exercises the missing pattern

## References

- `compiler/ori_ir/src/canon/tree.rs` -- `DecisionTree`, `FlatPattern`, `PatternRow`, `TestKind`, `TestValue`, `PathInstruction`, `ScrutineePath`
- `compiler/ori_ir/src/canon/mod.rs` -- `CanExpr::Match`, `DecisionTreePool`, `DecisionTreeId`, `CanonResult`
- `compiler/ori_ir/src/pattern_resolution.rs` -- `PatternKey`, `PatternResolution`
- `compiler/ori_ir/src/ast/patterns/binding.rs` -- `MatchPattern` AST variants
- `compiler/ori_canon/src/patterns.rs` -- `compile_patterns()`, `flatten_arm_pattern()`, `try_resolve_unit_variant()`
- `compiler/ori_arc/src/decision_tree/compile.rs` -- Maranget algorithm: `compile()`, `specialize_matrix()`, `pick_column()`, `ConstructorKey`
- `compiler/ori_arc/src/decision_tree/flatten.rs` -- `flatten_pattern()`, type resolution helpers
- `compiler/ori_eval/src/exec/decision_tree.rs` -- `eval_decision_tree()`, `MatchResult`, path resolution
- `compiler/ori_types/src/type_error/check_error.rs` -- `TypeErrorKind::NonExhaustiveMatch`
- `compiler/ori_types/src/type_error/problem.rs` -- `TypeErrorKind::NonExhaustiveMatch` (type checker level)
- `compiler/ori_types/src/type_error/suggest.rs` -- Suggestion generation for non-exhaustive match
- `compiler/ori_patterns/src/errors.rs` -- `EvalErrorKind::NonExhaustiveMatch`, `non_exhaustive_match()` factory
- `~/projects/reference_repos/lang_repos/rust/compiler/rustc_pattern_analysis/src/` -- `PatCx` trait, `Constructor`, `usefulness.rs`
- `~/projects/reference_repos/lang_repos/gleam/compiler-core/src/exhaustiveness.rs` -- `Decision`, `RuntimeCheck`, `MissingPatternsGenerator`
- `~/projects/reference_repos/lang_repos/elm/compiler/src/Nitpick/PatternMatches.hs` -- `Pattern`, `Error`, matrix walk
