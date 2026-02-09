---
section: "03"
title: Pattern Compilation
status: not-started
goal: Compile match expression patterns to decision trees during canonicalization using Maranget's algorithm, shared by both backends
sections:
  - id: "03.1"
    title: Pattern Compiler Integration
    status: not-started
  - id: "03.2"
    title: MatchPattern → Matrix Conversion
    status: not-started
  - id: "03.3"
    title: Interpreter Decision Tree Evaluator
    status: not-started
  - id: "03.4"
    title: Completion Checklist
    status: not-started
---

# Section 03: Pattern Compilation

**Status:** Not Started
**Goal:** Compile match expression patterns to decision trees during the lowering pass. The Maranget algorithm (already designed in LLVM V2 Section 10) is called during lowering and the resulting `DecisionTree` is stored in `CanonResult.decision_trees` via `DecisionTreeId`. Both `ori_eval` and `ori_arc` consume the same pre-compiled trees.

**File:** `compiler/ori_canon/src/patterns.rs`

**Prior art:**
- **Roc** `crates/compiler/mono/src/ir/decision_tree.rs` — Maranget-style producing `DecisionTree` with `Match`/`Decision` variants
- **Elm** `compiler/src/Optimize/DecisionTree.hs` — Scott-Ramsey heuristics for column selection
- **Ori LLVM V2** `plans/llvm_v2/section-10-decision-trees.md` — `DecisionTree`, `ScrutineePath`, `TestKind`, `TestValue` types + Maranget algorithm (types now in `ori_ir`, algorithm reused)
- **Maranget (2008)** "Compiling Pattern Matching to Good Decision Trees"

---

## 03.1 Pattern Compiler Integration

Wire the pattern compilation algorithm into the lowering pass.

- [ ] When lowering `ExprKind::Match { scrutinee, arms }`:
  - [ ] Lower the scrutinee expression → `CanId`
  - [ ] Convert each match arm's patterns to `PatternMatrix` input format
  - [ ] Call `compile_patterns(matrix, paths)` → `DecisionTree`
  - [ ] Store in `DecisionTreePool` → get `DecisionTreeId`
  - [ ] Lower each arm body → `CanId` (stored in `CanRange` for arms)
  - [ ] Construct `CanExpr::Match { scrutinee, decision_tree, arms }`

- [ ] Determine where the Maranget algorithm lives:
  - [ ] **Option A**: Keep in `ori_arc/src/decision_tree/compile.rs`, call from `ori_canon`
  - [ ] **Option B**: Move to `ori_canon/src/patterns.rs` (since `ori_canon` is the canonical location for pattern compilation)
  - [ ] **Preferred**: Option B — the algorithm is logically part of canonicalization, not ARC analysis. `ori_arc` keeps only the ARC IR emission logic (`emit.rs`)

---

## 03.2 MatchPattern → Matrix Conversion

Convert Ori's `MatchPattern` variants (from `ori_ir`) to the `PatternRow`/`PatternMatrix` input format expected by the Maranget algorithm.

```rust
/// A simplified pattern for the decision tree compiler.
/// Derived from Ori's MatchPattern + type checker's PatternResolution.
enum CompilerPattern {
    Wildcard,
    Variable(Name),
    Literal(LiteralValue),
    Constructor { tag: u32, name: Name, sub_patterns: Vec<CompilerPattern> },
    Tuple(Vec<CompilerPattern>),
    List { elements: Vec<CompilerPattern>, rest: Option<Name> },
    Range { lo: i64, hi: i64 },
    Or(Vec<CompilerPattern>),
    As { pattern: Box<CompilerPattern>, binding: Name },
}
```

- [ ] Implement `convert_pattern(match_pattern, type_result) -> CompilerPattern`
  - [ ] `MatchPattern::Wildcard` → `CompilerPattern::Wildcard`
  - [ ] `MatchPattern::Binding(name)` → `CompilerPattern::Variable(name)`
  - [ ] `MatchPattern::Literal(expr_id)` → evaluate literal → `CompilerPattern::Literal(value)`
  - [ ] `MatchPattern::Variant { name, payload }` → use `PatternResolution` to get tag index → `CompilerPattern::Constructor { tag, name, sub_patterns }`
  - [ ] `MatchPattern::Struct { fields }` → `CompilerPattern::Constructor` with field sub-patterns
  - [ ] `MatchPattern::Tuple(patterns)` → `CompilerPattern::Tuple(sub_patterns)`
  - [ ] `MatchPattern::List(patterns)` → `CompilerPattern::List { elements, rest }`
  - [ ] `MatchPattern::Range { start, end }` → `CompilerPattern::Range { lo, hi }`
  - [ ] `MatchPattern::Or(patterns)` → `CompilerPattern::Or(alternatives)`
  - [ ] `MatchPattern::As { pattern, binding }` → `CompilerPattern::As { pattern, binding }`
- [ ] Build `PatternMatrix` from converted patterns
  - [ ] Each match arm becomes a `PatternRow`
  - [ ] Guard expressions recorded on rows (for `DecisionTree::Guard` nodes)

---

## 03.3 Interpreter Decision Tree Evaluator

The evaluator needs a new function to walk decision trees at runtime. This replaces sequential arm testing for match expressions.

- [ ] Implement `eval_decision_tree(interp, tree, scrutinee_value) -> EvalResult` in `ori_eval`
  - [ ] `DecisionTree::Switch { path, test_kind, edges, default }`:
    - [ ] Navigate to sub-value via `resolve_path(scrutinee, path)`
    - [ ] Test the sub-value against each edge's `TestValue`
    - [ ] Branch to matching edge's subtree, or default
  - [ ] `DecisionTree::Leaf { arm_index, bindings }`:
    - [ ] For each `(name, path)`: bind `name` to `resolve_path(scrutinee, path)` in scope
    - [ ] Evaluate the arm body at `arm_index`
  - [ ] `DecisionTree::Guard { arm_index, bindings, on_fail }`:
    - [ ] Bind variables
    - [ ] Evaluate guard expression
    - [ ] If true: evaluate arm body
    - [ ] If false: recurse into `on_fail` tree
  - [ ] `DecisionTree::Fail` → `unreachable!()` (exhaustiveness guarantees)

- [ ] Implement `resolve_path(value: &Value, path: &ScrutineePath) -> Value`
  - [ ] `PathInstruction::TagPayload(i)` → extract variant payload field i
  - [ ] `PathInstruction::TupleIndex(i)` → extract tuple element i
  - [ ] `PathInstruction::StructField(name)` → extract struct field
  - [ ] `PathInstruction::ListElement(i)` → extract list element i

- [ ] Implement test matching for each `TestKind`:
  - [ ] `EnumTag` → compare variant discriminant
  - [ ] `IntEq` → compare integer value
  - [ ] `StrEq` → compare string value
  - [ ] `BoolEq` → compare boolean value
  - [ ] `ListLen` → compare list length (exact or minimum)
  - [ ] `IntRange` → check if integer is within range bounds

---

## 03.4 Completion Checklist

- [ ] All 10 `MatchPattern` variants handled in conversion
- [ ] Decision tree compilation called during lowering for every `Match` expression
- [ ] Decision trees stored in `DecisionTreePool`, referenced by `DecisionTreeId`
- [ ] `eval_decision_tree()` implemented and tested in `ori_eval`
- [ ] `ori_arc` emits ARC IR from decision trees stored in `CanonResult` (not compiled separately)
- [ ] All existing match tests pass with decision tree evaluation
- [ ] Guards fall through to next compatible arm (not next sequential arm)
- [ ] Or-patterns share arm body with correct bindings

**Exit Criteria:** Match expressions are compiled to decision trees during canonicalization. Both `ori_eval` and `ori_arc` consume the same pre-compiled trees from `CanonResult`. Pattern matching behavior is identical across both backends. The Maranget algorithm is implemented once.
