---
section: "04"
title: Pattern Compilation
status: not-started
goal: Compile match arms to decision trees for efficient dispatch, integrating exhaustiveness checking
sections:
  - id: "04.1"
    title: Decision Tree IR
    status: not-started
  - id: "04.2"
    title: Pattern Compiler
    status: not-started
  - id: "04.3"
    title: Decision Tree Evaluator
    status: not-started
  - id: "04.4"
    title: Exhaustiveness Integration
    status: not-started
---

# Section 04: Pattern Compilation

**Status:** 📋 Planned
**Goal:** Compile match expressions to decision trees for O(depth) dispatch instead of O(arms) sequential testing — inspired by Gleam and Elm's pattern compilation.
**Dependencies:** Section 03 (Environment — `with_match_bindings` already exists in `ori_eval/src/interpreter/scope_guard.rs` as `Interpreter::with_match_bindings`; this section uses it as-is. No new Section 03 functionality is required.), Section 08 (Canonical EvalIR — decision trees stored in EvalIR, bodies become EvalIrId post-migration. See transitional approach below for independent development.)

**New directory:** `ori_eval/src/pattern/` — create this directory for all pattern compilation code. It depends on `ori_types::{PatternKey, PatternResolution}` for variant vs. variable disambiguation.

**Phase:** Decision tree compilation uses a **compile-on-first-use** transitional approach. Before Section 08 (EvalIR) is ready, the pattern compiler runs lazily on each match expression the first time it is evaluated, and the resulting decision tree is cached (memoized) for subsequent evaluations. After Section 08, compilation moves to the **EvalIR lowering** phase — between type checking and evaluation — and cached trees are stored in the EvalIR. The evaluator (Section 04.3) then executes the pre-compiled tree at runtime.

---

## Prior Art Analysis

### Current Ori: Sequential Arm Testing
The current evaluator tests each match arm sequentially via `try_match()`. Each arm's pattern is matched against the scrutinee; if it fails, the next arm is tried. Guard expressions are evaluated when a pattern matches. This is O(n) in the number of arms, with redundant tests (e.g., checking the same tag multiple times for different arms).

### Gleam: Decision Tree + Exhaustiveness
Gleam compiles patterns to a `Decision` tree (`Switch`, `Run`, `Guard`, `Fail`) that minimizes redundant tests. Variable bindings are accumulated as `BoundValue`. Unreachable patterns are detected via **variant inference** — if a value is known to be `Some(x)`, the `None` branch is unreachable.

### Elm: DecisionTree + Inline/Jump Optimization
Elm's `Optimize/DecisionTree.hs` (based on Scott & Ramsey) compiles patterns to `Decision` nodes with `Path` (how to reach a sub-value) and `Test` (what to check). The `Optimize/Case.hs` pass then decides: **Inline** if a branch appears once, **Jump** if it appears multiple times. This optimization is crucial for code generation but also benefits interpretation.

### Roc: Exhaustiveness via roc_exhaustive
Roc's `exhaustive.rs` sketches patterns, reifies against types, and checks usefulness. Results are `ExhaustiveSummary` with missing patterns and redundant arms. This happens in canonicalization, before evaluation.

---

## 04.1 Decision Tree IR

Define the decision tree representation:

```rust
/// A compiled decision tree for a match expression.
pub enum DecisionTree {
    /// Terminal: execute the arm at this index
    Leaf {
        arm_index: usize,
        bindings: Vec<(Name, BoundValue)>,
    },

    /// Test a value and branch based on result
    Switch {
        /// Path to the value being tested
        path: AccessPath,
        /// Branches for each constructor/literal
        branches: Vec<(TestKind, DecisionTree)>,
        /// Default branch (wildcard / variable)
        default: Option<Box<DecisionTree>>,
    },

    /// Evaluate a guard expression
    Guard {
        arm_index: usize,
        bindings: Vec<(Name, BoundValue)>,
        /// If guard passes → body
        on_true: Box<DecisionTree>,
        /// If guard fails → try next
        on_false: Box<DecisionTree>,
    },

    /// No match possible (non-exhaustive error)
    Fail,
}

/// How to access a sub-value from the scrutinee
pub enum AccessPath {
    /// The scrutinee itself
    Root,
    /// Field of a struct/variant at index
    Field { parent: Box<AccessPath>, index: u32 },
    /// Element of a tuple at index
    TupleElem { parent: Box<AccessPath>, index: u32 },
    /// Inner value of Some/Ok/Err
    Unwrap { parent: Box<AccessPath> },
    /// Head of a list
    ListHead { parent: Box<AccessPath> },
    /// Tail of a list
    ListTail { parent: Box<AccessPath> },
}

/// What to test at a switch point
pub enum TestKind {
    /// Constructor match: variant name + arity
    Constructor { type_name: Name, variant_name: Name, arity: u16 },
    /// Literal match
    Literal(LiteralTest),
    /// Some(_) vs None
    IsSome,
    IsNone,
    /// Ok(_) vs Err(_)
    IsOk,
    IsErr,
    /// List patterns
    IsEmpty,
    IsCons,
    /// Range pattern: value in start..end or start..=end
    InRange { start: Option<LiteralTest>, end: Option<LiteralTest>, inclusive: bool },
}

pub enum LiteralTest {
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(String),
    Char(char),
}

/// Or-pattern: alternative patterns merged during compilation.
/// `A | B` becomes branches for both A and B pointing to the same leaf.
/// The compiler expands or-patterns into separate rows in the pattern matrix.

/// A value extracted during pattern matching
pub enum BoundValue {
    /// The value at the given path
    Path(AccessPath),
    /// A literal value (from an as-pattern with literal)
    Literal(Value),
}
```

- [ ] Define `DecisionTree` enum in `ori_eval/src/pattern/decision.rs`
- [ ] Define `AccessPath` enum for sub-value access
- [ ] Define `TestKind` enum for switch conditions
- [ ] Define `BoundValue` enum for binding extraction
- [ ] Implement `Display` for all types (debugging)
- [ ] Implement `Debug` for all types (tracing)

---

## 04.2 Pattern Compiler

The pattern compiler transforms a list of match arms into a decision tree:

```rust
pub struct PatternCompiler<'a> {
    /// Type checker's pattern resolutions (variant vs variable disambiguation)
    resolutions: &'a [(PatternKey, PatternResolution)],
    /// String interner for name resolution
    interner: &'a StringInterner,
}

impl<'a> PatternCompiler<'a> {
    /// Compile a match expression's arms into a decision tree.
    pub fn compile(
        &self,
        arms: &[MatchArm],
        arena: &ExprArena,
    ) -> DecisionTree {
        let rows: Vec<PatternRow> = arms.iter().enumerate()
            .map(|(i, arm)| PatternRow {
                patterns: vec![(AccessPath::Root, &arm.pattern)],
                guard: arm.guard,
                arm_index: i,
            })
            .collect();

        self.compile_rows(rows)
    }

    /// Core algorithm: compile a matrix of pattern rows.
    fn compile_rows(&self, rows: Vec<PatternRow>) -> DecisionTree {
        // Base cases
        if rows.is_empty() {
            return DecisionTree::Fail;
        }

        let first = &rows[0];
        if first.patterns.iter().all(|(_, p)| self.is_wildcard(p)) {
            // All patterns are wildcards — this arm matches
            let bindings = self.extract_bindings(first);
            return if let Some(guard) = first.guard {
                let leaf_bindings = bindings.clone();
                DecisionTree::Guard {
                    arm_index: first.arm_index,
                    bindings,
                    on_true: Box::new(DecisionTree::Leaf {
                        arm_index: first.arm_index,
                        bindings: leaf_bindings,
                    }),
                    on_false: Box::new(self.compile_rows(rows[1..].to_vec())),
                }
            } else {
                DecisionTree::Leaf {
                    arm_index: first.arm_index,
                    bindings,
                }
            };
        }

        // Choose the column with the most variety (heuristic)
        let column = self.choose_column(&rows);
        let (path, _) = &rows[0].patterns[column];

        // Collect all constructors/literals in this column
        let tests = self.collect_tests(&rows, column);

        // Specialize for each constructor/literal
        let branches: Vec<(TestKind, DecisionTree)> = tests.into_iter()
            .map(|test| {
                let specialized = self.specialize_rows(&rows, column, &test);
                (test, self.compile_rows(specialized))
            })
            .collect();

        // Default: rows with wildcards in this column
        let default_rows = self.default_rows(&rows, column);
        let default = if default_rows.is_empty() {
            None
        } else {
            Some(Box::new(self.compile_rows(default_rows)))
        };

        DecisionTree::Switch {
            path: path.clone(),
            branches,
            default,
        }
    }
}
```

**Algorithm** (based on Maranget's "Compiling Pattern Matching to Good Decision Trees"):
1. If all remaining patterns are wildcards, emit a Leaf
2. Choose the column with the most variety (minimize tests)
3. For each constructor/literal in that column, specialize the matrix
4. Build a Switch node with branches and optional default
5. Recurse on each specialized sub-matrix

- [ ] Implement `PatternCompiler` in `ori_eval/src/pattern/compiler.rs`
  - [ ] `compile(arms, arena) -> DecisionTree` — main entry point
  - [ ] `compile_rows(rows) -> DecisionTree` — recursive core
  - [ ] `choose_column(rows) -> usize` — heuristic for column selection
  - [ ] `collect_tests(rows, column) -> Vec<TestKind>` — enumerate tests
  - [ ] `specialize_rows(rows, column, test) -> Vec<PatternRow>` — matrix specialization
  - [ ] `default_rows(rows, column) -> Vec<PatternRow>` — wildcard rows
  - [ ] `extract_bindings(row) -> Vec<(Name, BoundValue)>` — binding collection
  - [ ] `is_wildcard(pattern) -> bool` — check if pattern matches anything
- [ ] Handle ALL parser MatchPattern variants (no gaps):
  - [ ] `Wildcard` → wildcard row (matches anything)
  - [ ] `Binding(name)` → variable binding (or unit variant via pattern_resolutions)
  - [ ] `Literal(expr)` → literal test (Int, Float, Bool, Str, Char)
  - [ ] `Variant { name, inner }` → constructor test with sub-patterns
  - [ ] `Struct { fields }` → struct destructuring (field access paths). Handle shorthand: `None` inner = bind field name directly (field name becomes binding name), `Some(inner_pattern)` = recurse into sub-pattern for that field.
  - [ ] `Tuple(elems)` → tuple element access paths
  - [ ] `List { elements, rest }` → IsCons/IsEmpty chain with head/tail access
  - [ ] `Range { start, end, inclusive }` → InRange test
  - [ ] `Or(alternatives)` → expand to duplicate rows in pattern matrix (each alternative gets same leaf)
  - [ ] `At { name, pattern }` → as-pattern: bind name AND continue matching inner pattern
- [ ] Use pattern resolutions from type checker
  - [ ] Distinguish unit variants from variable bindings
  - [ ] Binary search in sorted resolutions array

---

## 04.3 Decision Tree Evaluator

Execute a compiled decision tree against a scrutinee value:

```rust
impl<'a> Interpreter<'a> {
    /// Evaluate a match expression using a compiled decision tree.
    pub fn eval_decision_tree(
        &mut self,
        scrutinee: &Value,
        tree: &DecisionTree,
        arms: &[MatchArm],
    ) -> EvalResult {
        // Note: MatchArm.body is ExprId pre-Section 08 migration. After Section 08,
        // match arms are compiled into the decision tree during EvalIR lowering, and
        // arm bodies become EvalIrId references within the tree. The evaluator then
        // operates on EvalIrId exclusively.
        match tree {
            DecisionTree::Leaf { arm_index, bindings } => {
                let resolved = self.resolve_bindings(scrutinee, bindings)?;
                self.with_match_bindings(resolved, |scoped| {
                    scoped.eval(arms[*arm_index].body)
                })
            }

            DecisionTree::Switch { path, branches, default } => {
                let target = self.access_path(scrutinee, path)?;
                for (test, subtree) in branches {
                    if self.test_matches(&target, test)? {
                        return self.eval_decision_tree(scrutinee, subtree, arms);
                    }
                }
                match default {
                    Some(subtree) => self.eval_decision_tree(scrutinee, subtree, arms),
                    None => Err(non_exhaustive_match()),
                }
            }

            DecisionTree::Guard { arm_index, bindings, on_true, on_false } => {
                let resolved = self.resolve_bindings(scrutinee, bindings)?;
                let guard_passed = self.with_match_bindings(resolved, |scoped| {
                    // SAFETY: Guard node only created for arms with guards
                    let guard_value = scoped.eval(arms[*arm_index].guard.unwrap())?;
                    Ok(guard_value.is_truthy())
                })?;
                if guard_passed {
                    self.eval_decision_tree(scrutinee, on_true, arms)
                } else {
                    self.eval_decision_tree(scrutinee, on_false, arms)
                }
            }

            DecisionTree::Fail => Err(non_exhaustive_match()),
        }
    }

    /// Navigate an AccessPath to extract a sub-value
    fn access_path(&self, root: &Value, path: &AccessPath) -> EvalResult<Value> {
        match path {
            AccessPath::Root => Ok(root.clone()),
            AccessPath::Field { parent, index } => {
                let parent_val = self.access_path(root, parent)?;
                parent_val.field_at(*index as usize)
            }
            AccessPath::Unwrap { parent } => {
                let parent_val = self.access_path(root, parent)?;
                parent_val.unwrap_inner()
            }
            // ... other paths
        }
    }
}
```

- [ ] Integrate `eval_decision_tree()` with existing `eval_match` methods
  - [ ] **Phase 1 (compile-on-first-use):** `eval_match` compiles the decision tree on first use, caches it (keyed by ExprId), then delegates to `eval_decision_tree`. Existing `eval_match` callers remain unchanged — the decision tree is an internal optimization, not a new API.
  - [ ] **Phase 2 (post-EvalIR, Section 08):** `eval_decision_tree` replaces `eval_match` entirely. Decision trees are pre-compiled during EvalIR lowering and stored in the IR. `eval_match` is removed.
- [ ] Implement `eval_decision_tree()` in Interpreter
  - [ ] Handle `Leaf` — bind variables, evaluate body
  - [ ] Handle `Switch` — navigate path, test branches, fallback to default
  - [ ] Handle `Guard` — bind variables, evaluate guard, branch
  - [ ] Handle `Fail` — non-exhaustive match error
- [ ] Implement `access_path()` — navigate to sub-values
  - [ ] `Root` — the scrutinee itself
  - [ ] `Field { parent, index }` — struct/variant field
  - [ ] `TupleElem { parent, index }` — tuple element
  - [ ] `Unwrap { parent }` — inner value of Option/Result
  - [ ] `ListHead`/`ListTail` — list destructuring
- [ ] Implement `test_matches()` — test a value against a TestKind
  - [ ] Constructor tests (check variant name)
  - [ ] Literal tests (equality check)
  - [ ] IsSome/IsNone, IsOk/IsErr tests
  - [ ] IsCons/IsEmpty list tests
  - [ ] InRange tests (boundary comparison with inclusive flag)
- [ ] Implement `resolve_bound()` — materialize BoundValue to Value

---

## 04.4 Exhaustiveness Integration

Connect the pattern compiler to exhaustiveness checking (currently in roadmap Section 9):

```rust
/// Check exhaustiveness and compile patterns in one pass.
pub fn compile_and_check(
    compiler: &PatternCompiler,
    arms: &[MatchArm],
    scrutinee_type: &Type,
    arena: &ExprArena,
) -> (DecisionTree, Vec<PatternWarning>) {
    let tree = compiler.compile(arms, arena);
    let warnings = check_exhaustiveness(&tree, scrutinee_type);
    (tree, warnings)
}

pub enum PatternWarning {
    /// A pattern arm is unreachable (covered by earlier arms)
    Unreachable { arm_index: usize, span: Span },
    /// Missing pattern cases
    NonExhaustive { missing: Vec<String>, span: Span },
}
```

- [ ] Design exhaustiveness check on decision trees
  - [ ] `check_exhaustiveness(tree, type) -> Vec<PatternWarning>` — find missing/redundant patterns
  - [ ] Walk tree: any `Fail` node reachable = non-exhaustive
  - [ ] Any `Leaf` node unreachable from any path = redundant arm
- [ ] Report warnings during type checking phase
  - [ ] Non-exhaustive matches: emit error with example missing pattern
  - [ ] Redundant arms: emit warning with arm location
- [ ] Cache compiled decision trees (compile-on-first-use)
  - [ ] **Pre-Section 08:** Store compiled trees in a `FxHashMap<ExprId, DecisionTree>` on the Interpreter. On first `eval_match`, compile and cache; on subsequent calls, look up cached tree. This makes Section 04 fully independent of Section 08.
  - [ ] **Post-Section 08:** Move cache to EvalIR. Decision trees are compiled during lowering and stored alongside match expressions in the IR arena. The Interpreter cache is removed.
  - [ ] Avoid recompilation on each evaluation

---

## 04.5 Performance Validation

- [ ] Benchmark pattern matching before/after compilation
  - [ ] Case: 10+ arm match on enum variants — measure speedup
  - [ ] Case: nested pattern matching — measure depth vs. sequential
  - [ ] Case: guard-heavy matches — ensure no regression
- [ ] Profile decision tree memory usage
  - [ ] Ensure tree size is proportional to pattern complexity, not exponential

---

## 04.6 Completion Checklist

- [ ] `DecisionTree` IR defined with all node types
- [ ] `PatternCompiler` compiles match arms to decision trees
- [ ] `eval_decision_tree()` evaluates compiled trees correctly
- [ ] All pattern types handled (wildcard, binding, literal, constructor, struct, tuple, list, range, or-pattern, as-pattern)
- [ ] Guard expressions integrated into decision tree
- [ ] Exhaustiveness checking on decision trees
- [ ] All existing match tests pass unchanged
- [ ] Pattern matching benchmark shows improvement

**Exit Criteria:** Match expressions are compiled to decision trees that minimize redundant testing, with integrated exhaustiveness checking.
