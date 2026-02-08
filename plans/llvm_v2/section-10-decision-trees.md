---
section: "10"
title: Pattern Match Decision Trees
status: complete
goal: Compile match expressions to efficient decision trees during AST-to-ARC-IR lowering, producing Switch terminators that map trivially to LLVM switch instructions
sections:
  - id: "10.1"
    title: Decision Tree Data Structures
    status: complete
  - id: "10.2"
    title: Decision Tree Construction Algorithm
    status: complete
  - id: "10.3"
    title: Guard & Or-Pattern Handling
    status: complete
  - id: "10.4"
    title: ARC IR Emission from Decision Trees
    status: complete
---

# Section 10: Pattern Match Decision Trees

**Status:** Complete (2 optimization items deferred)
**Goal:** Compile match expressions to efficient decision trees that produce optimal branching. Decision tree compilation happens during **AST-to-ARC IR lowering** in `ori_arc` (per Q1 decision). The result is ARC IR basic blocks with `Switch` terminators. LLVM emission is trivial: a `Switch` terminator maps directly to an LLVM `switch` instruction.

**Crate:** `ori_arc` (no LLVM dependency). The decision tree algorithm is part of the AST-to-ARC-IR lowering pass (Section 06.0). The `ori_llvm` crate does NOT contain pattern compilation logic -- it just emits `Switch` terminators as LLVM `switch` instructions and `Branch` terminators as LLVM `br` instructions.

**Reference compilers:**
- **Roc** `crates/compiler/mono/src/ir/decision_tree.rs` -- `DecisionTree` enum with `Match`/`Decision` variants for the tree, `Decider` enum with `Leaf`/`Chain`/`Guarded`/`FanOut` variants for the execution plan. Maranget-style algorithm.
- **Elm** `compiler/src/Nitpick/PatternMatches.hs` -- Classic Maranget implementation for exhaustiveness and decision trees
- **Swift** -- Pattern compilation to SIL `switch_enum` instruction
- **Maranget (2008)** "Compiling Pattern Matching to Good Decision Trees" -- The foundational algorithm that Roc and Elm implement

**Current state:** `ori_llvm/src/matching.rs` compiles match arms sequentially (arm 1 check -> arm 2 check -> ...). This is O(n) per match. More critically, the if-else chain of `icmp` + `cond_br` instructions misses LLVM's `switch` terminator, which enables jump table (O(1)) and binary search compilation by the LLVM backend. Decision trees produce `switch` terminators for tag dispatch, achieving O(log n) or O(1) performance.

**Exhaustiveness note:** Exhaustiveness checking is performed by `ori_types` before codegen ever runs. The decision tree compiler can assume the match is exhaustive. The `Fail` node in the decision tree maps to LLVM `unreachable` -- if reached, it indicates a compiler bug, not a user error.

---

## 10.1 Decision Tree Data Structures

```rust
/// A compiled decision tree for pattern matching.
///
/// Constructed during AST → ARC IR lowering in `ori_arc`.
/// Emitted as ARC IR basic blocks with Switch/Branch terminators.
pub enum DecisionTree {
    /// Test a scrutinee, branch based on the result.
    Switch {
        /// How to reach the value being tested (path from root scrutinee).
        path: ScrutineePath,
        /// The kind of test being performed.
        test_kind: TestKind,
        /// Branches: each edge maps a test value to a subtree.
        edges: Vec<(TestValue, DecisionTree)>,
        /// Default subtree for values not covered by any edge.
        /// This handles wildcards and catch-all patterns.
        default: Option<Box<DecisionTree>>,
    },
    /// Reached a match arm. Bind variables and execute the body.
    Leaf {
        arm_index: usize,
        bindings: Vec<(Name, ScrutineePath)>,
    },
    /// Guarded leaf. Test a guard condition; if it fails, fall through
    /// to the next compatible arm (not just the next arm in source order).
    Guard {
        arm_index: usize,
        bindings: Vec<(Name, ScrutineePath)>,
        /// Decision tree to execute if the guard fails.
        /// This is the continuation of pattern matching with
        /// the remaining compatible arms.
        on_fail: Box<DecisionTree>,
    },
    /// Unreachable. Exhaustiveness guarantees this won't execute.
    /// Maps to LLVM `unreachable` instruction.
    Fail,
}
```

### Scrutinee Path Tracking

When testing nested patterns, the scrutinee for inner tests is derived by projecting fields from the outer scrutinee. A `ScrutineePath` tracks how to reach any sub-scrutinee from the root.

```rust
/// A path from the root scrutinee to a sub-value.
///
/// Example: matching `Cons(Pair(x, _), _)`:
///   - Root scrutinee: the list value
///   - Path to `x`: [TagPayload(0), TupleIndex(0)]
///     (get Cons payload at field 0, then get first element of Pair)
///
/// **Implementation note:** ScrutineePath is cloned frequently during matrix
/// specialization (once per edge per recursion level). For typical patterns
/// (depth <= 4), use `SmallVec<[PathInstruction; 4]>` to avoid heap allocation
/// on clones. Deeply nested patterns (depth > 4) spill to the heap, which is
/// acceptable since such patterns are rare. This optimization matters because
/// the Maranget algorithm's time complexity is proportional to the number of
/// path clones.
pub type ScrutineePath = SmallVec<[PathInstruction; 4]>;

/// One step in a scrutinee path.
pub enum PathInstruction {
    /// Extract the payload of an enum variant at the given field index.
    /// Used after a tag test confirms the variant.
    TagPayload(u32),
    /// Extract element at index from a tuple.
    TupleIndex(u32),
    /// Extract a named field from a struct.
    StructField(Name),
    /// Extract element at index from a list (for list pattern matching).
    ListElement(u32),
}

// Note: As-patterns (`x @ P`) do NOT require an additional PathInstruction variant.
// `x` is bound to the scrutinee at the current path without extending the path.
// The inner pattern `P` is processed normally through the existing instructions.
// During `extract_bindings`, as-patterns simply record a binding at the current
// ScrutineePath; the sub-pattern continues matching at the same path.
```

### Test Kinds and Values

```rust
/// What kind of test to perform on a scrutinee.
///
/// The test kind is separate from the test value. A Switch node
/// has one TestKind and multiple TestValue edges.
pub enum TestKind {
    /// Compare the tag of an enum/union value.
    /// Edges are TestValue::Tag variants.
    EnumTag,
    /// Compare an integer value.
    /// Edges are TestValue::Int variants.
    IntEq,
    /// Compare a string value.
    /// Edges are TestValue::Str variants.
    StrEq,
    /// Compare a boolean value.
    /// Edges are TestValue::Bool variants.
    BoolEq,
    /// Compare a float value (exact bit equality).
    /// Edges are TestValue::Float variants.
    FloatEq,
    /// Check if a value falls within an integer range.
    /// Edges are TestValue::IntRange variants.
    IntRange,
    /// Check the length of a list (for list patterns).
    /// Edges are TestValue::ListLen variants.
    ListLen,
}

/// A specific test value for one edge of a Switch node.
pub enum TestValue {
    /// Tag match for an enum variant.
    Tag {
        /// Discriminant index used for the switch instruction.
        variant_index: u32,
        /// Variant name for diagnostics and readability.
        variant_name: Name,
    },
    /// Integer literal match.
    Int(i64),
    /// String literal match.
    Str(Name),
    /// Boolean literal match.
    Bool(bool),
    /// Float literal match (exact bit equality).
    Float(u64), // f64 bits for exact comparison
    /// Integer range match (inclusive on both ends).
    IntRange { lo: i64, hi: i64 },
    /// List length match (exact length or minimum length).
    ListLen { len: u32, is_exact: bool },
}
```

**Forward-looking variants:** `IntRange` and `FloatEq` in `TestKind`/`TestValue` are forward-looking and may not be in the 0.1-alpha spec. The data structures should support them from the start (to avoid breaking changes later), but the initial implementation can omit their handling — the construction algorithm simply won't produce these variants until the language adds range patterns and float matching.

**Tag type derivation:** The tag discriminant type is NOT hardcoded to `i8`. During ARC IR emission, the tag type is derived from `TypeInfo` for the enum being matched. Small enums (up to 256 variants) use `i8`, larger enums use `i16` or `i32`. The LLVM `switch` instruction's case values must match the tag type. This is determined at emission time from the `TypeInfo` of the scrutinee, not baked into the decision tree.

- [x] Define `DecisionTree` enum with `Switch`, `Leaf`, `Guard`, `Fail` variants
- [x] Define `ScrutineePath` and `PathInstruction` for nested pattern access
- [x] Define `TestKind` enum (EnumTag, IntEq, StrEq, BoolEq, FloatEq, IntRange, ListLen)
- [x] Define `TestValue` enum with proper payloads (Tag has variant_index + variant_name)
- [x] Derive tag type from TypeInfo, not hardcoded i8

## 10.2 Decision Tree Construction Algorithm

The core algorithm follows Maranget (2008), as implemented in Roc and Elm. It operates on a **pattern matrix** where rows are match arms and columns are sub-patterns at each scrutinee position.

### Input

```rust
/// A row in the pattern matrix (one match arm).
struct PatternRow {
    /// Remaining patterns to test (one per column).
    patterns: Vec<Pattern>,
    /// The arm index in the original match expression.
    arm_index: usize,
    /// Guard condition, if any.
    guard: Option<ExprId>,
}

/// The pattern matrix: rows of arms, columns of sub-patterns.
type PatternMatrix = Vec<PatternRow>;
```

### Algorithm: `compile(matrix, paths)`

Recursive algorithm that produces a `DecisionTree`:

```
fn compile(matrix: PatternMatrix, paths: Vec<ScrutineePath>) -> DecisionTree:
    // 1. EMPTY MATRIX: no arms left → Fail (unreachable by exhaustiveness)
    if matrix.is_empty():
        return DecisionTree::Fail

    // 2. FIRST ROW ALL WILDCARDS: match found → Leaf or Guard
    if matrix[0].patterns.iter().all(|p| p.is_wildcard_or_variable()):
        let bindings = extract_bindings(matrix[0], paths)
        if matrix[0].guard.is_some():
            // Guard present: if guard fails, continue matching with
            // remaining compatible rows (not just the next row!)
            let on_fail = compile(matrix[1..], paths)
            return DecisionTree::Guard {
                arm_index: matrix[0].arm_index,
                bindings,
                on_fail: Box::new(on_fail),
            }
        else:
            return DecisionTree::Leaf {
                arm_index: matrix[0].arm_index,
                bindings,
            }

    // 3. PICK COLUMN: choose the best column to split on
    let col = pick_column(&matrix)
    let path = paths[col]

    // 4. GATHER EDGES: collect all distinct test values at the chosen column
    let test_values = collect_test_values(&matrix, col)
    let test_kind = infer_test_kind(&test_values)

    // 5. BUILD EDGES: for each test value, filter compatible rows and recurse
    let edges = test_values.iter().map(|tv| {
        let specialized = specialize_matrix(&matrix, col, tv, paths)
        let subtree = compile(specialized.matrix, specialized.paths)
        (tv.clone(), subtree)
    }).collect()

    // 6. DEFAULT: rows with wildcards at the chosen column form the default
    let default_rows = default_matrix(&matrix, col, paths)
    let default = if default_rows.is_empty() {
        None
    } else {
        Some(Box::new(compile(default_rows.matrix, default_rows.paths)))
    }

    DecisionTree::Switch { path, test_kind, edges, default }
```

### Column Selection Heuristic

Choose the column that provides the most information -- this minimizes the tree size:

```
fn pick_column(matrix: &PatternMatrix) -> usize:
    // Heuristic: pick the column with the most distinct constructors
    // (most branching power). Break ties by choosing the leftmost column.
    // This follows Maranget's "column with the most information" strategy.
    matrix[0].patterns
        .iter()
        .enumerate()
        .filter(|(_, p)| !p.is_wildcard_or_variable())
        .max_by_key(|(col, _)| count_distinct_constructors(matrix, *col))
        .map(|(col, _)| col)
        .unwrap_or(0)
```

### Matrix Specialization

When splitting on a test value `tv` at column `col`:

```
fn specialize_matrix(matrix, col, tv, paths) -> SpecializedMatrix:
    // For each row in the matrix:
    //   - If pattern at `col` matches `tv`: decompose it, add sub-patterns
    //   - If pattern at `col` is wildcard: keep (compatible with any value)
    //   - If pattern at `col` is a different constructor: exclude
    //
    // Sub-patterns from the matched constructor become new columns.
    // The scrutinee paths are extended with the appropriate PathInstructions.
```

### Default Matrix

```
fn default_matrix(matrix, col, paths) -> DefaultMatrix:
    // Keep only rows where the pattern at `col` is a wildcard/variable.
    // Remove column `col` from all rows (it's been tested).
    // Remove the corresponding path from `paths`.
```

- [x] Implement `compile()` recursive algorithm
- [x] Implement `pick_column()` heuristic (most distinct constructors)
- [x] Implement `specialize_matrix()` for constructor decomposition
- [x] Implement `default_matrix()` for wildcard/catch-all rows
- [x] Implement `extract_bindings()` to collect variable bindings from patterns
- [x] Implement `collect_test_values()` to gather distinct constructors at a column
- [x] Handle nested patterns via path extension during specialization
- [x] Handle literal patterns (int, string, bool, float) alongside constructor patterns
- [x] Handle range patterns as `IntRange` test values
- [x] Handle list patterns with length checks + element extraction
- [ ] Optimize: merge identical subtrees (DAG instead of tree)

## 10.3 Guard & Or-Pattern Handling

### Guards

Guard expressions introduce conditional matching: even when the pattern matches structurally, the arm is only taken if the guard evaluates to `true`. On guard failure, matching must continue with the **next compatible arm**, not just the next arm in source order.

```ori
match value {
    Some(x) if x > 0 -> positive(x)
    Some(x) if x < 0 -> negative(x)
    Some(x) -> zero(x)
    None -> default()
}
```

The decision tree for this match:

```
Switch(tag of value):
  Some → Guard(arm 0, guard: x > 0,
           on_fail: Guard(arm 1, guard: x < 0,
             on_fail: Leaf(arm 2)))
  None → Leaf(arm 3)
```

When the first guard (`x > 0`) fails, the decision tree falls through to the second guard (`x < 0`), and then to the unguarded arm. The `on_fail` chain preserves all compatible arms as fallback paths.

**Implementation note:** During `compile()`, when a guarded row matches, all subsequent compatible rows (rows that would also match the same structural pattern) are included in the `on_fail` subtree. This ensures no compatible arm is skipped when a guard fails.

### Or-Patterns

Or-patterns combine multiple patterns sharing the same body:

```ori
match shape {
    Circle(r) | Sphere(r) -> use_radius(r)
    Square(s) | Cube(s) -> use_side(s)
    _ -> other()
}
```

Or-patterns `A | B -> body` are expanded into two branches that share the same leaf. The body is emitted once; both patterns jump to it:

```
Switch(tag of shape):
  Circle → Leaf(arm 0, bindings: [r = payload])
  Sphere → Leaf(arm 0, bindings: [r = payload])  // Same arm_index!
  Square → Leaf(arm 1, bindings: [s = payload])
  Cube   → Leaf(arm 1, bindings: [s = payload])  // Same arm_index!
  _      → Leaf(arm 2)
```

At ARC IR emission time, leaves with the same `arm_index` share a single body block. Each pattern path jumps to the shared body block, passing bindings as block parameters. The body is emitted exactly once.

**Variable binding constraint:** All alternatives in an or-pattern must bind the same set of variables with the same types. This is checked by `ori_types` before codegen.

- [x] Implement guard-aware compilation: `on_fail` chain includes all compatible arms
- [x] Implement or-pattern expansion: shared leaf label for `A | B -> body`
- [x] Emit shared body block for or-pattern arms (jump from each alternative)
- [x] Test: guards with overlapping patterns fall through correctly
- [x] Test: or-patterns bind variables from different constructor shapes

## 10.4 ARC IR Emission from Decision Trees

The decision tree is emitted as ARC IR basic blocks during AST-to-ARC-IR lowering. Each `Switch` node becomes a block with a `Switch` or `Branch` terminator. Each `Leaf` becomes a block with the arm body.

### Emission Algorithm

```rust
/// Emit a decision tree as ARC IR basic blocks.
///
/// Called during AST → ARC IR lowering (Section 06.0).
/// The root scrutinee is already available as an ArcVarId.
fn emit_decision_tree(
    builder: &mut ArcIrBuilder,
    tree: &DecisionTree,
    root_scrutinee: ArcVarId,
    merge_block: ArcBlockId,  // Where all arms jump after executing their body
) {
    match tree {
        DecisionTree::Switch { path, test_kind, edges, default } => {
            // 1. Navigate to the sub-scrutinee via path instructions.
            let scrutinee = resolve_path(builder, root_scrutinee, path);

            // 2. For enum tag tests: extract the tag value.
            //    Tag type is derived from TypeInfo of the scrutinee — NOT hardcoded i8.
            //    Small enums (<=256 variants) use i8, larger use i16 or i32.

            match test_kind {
                TestKind::EnumTag => {
                    // Extract tag from scrutinee
                    let tag = builder.emit_project_tag(scrutinee);

                    // Create blocks for each edge + default
                    let default_block = builder.new_block("match_default");
                    let case_blocks: Vec<_> = edges.iter().map(|(tv, _)| {
                        let TestValue::Tag { variant_index, .. } = tv;
                        let block = builder.new_block(&format!("case_{}", variant_index));
                        (*variant_index as u64, block)
                    }).collect();

                    // Emit Switch terminator (maps to LLVM switch instruction)
                    builder.emit_switch(tag, &case_blocks, default_block);

                    // 3. For each case: extract payload fields, bind variables, recurse
                    for ((tv, subtree), &(_, case_block)) in edges.iter().zip(&case_blocks) {
                        builder.position_at(case_block);
                        let TestValue::Tag { variant_index, .. } = tv;

                        // Extract payload via struct_gep (field access after tag)
                        // Each payload field becomes an ArcVarId bound in the subtree
                        emit_payload_extraction(builder, scrutinee, *variant_index);

                        // Recurse into subtree
                        emit_decision_tree(builder, subtree, root_scrutinee, merge_block);
                    }

                    // 4. Default block
                    builder.position_at(default_block);
                    if let Some(default_tree) = default {
                        emit_decision_tree(builder, default_tree, root_scrutinee, merge_block);
                    } else {
                        // Exhaustiveness guarantees unreachable
                        builder.emit_unreachable();
                    }
                }
                TestKind::IntEq | TestKind::BoolEq => {
                    // Switch on integer/bool value (LLVM switch instruction)
                    // Similar to tag dispatch above
                }
                TestKind::StrEq | TestKind::FloatEq => {
                    // Sequential comparison (no LLVM switch for strings/floats)
                    // Emit if-else chain using Branch terminators
                }
                TestKind::IntRange => {
                    // Range check: lo <= value && value <= hi
                    // Emit Branch terminators for bounds checks
                }
                TestKind::ListLen => {
                    // Length check: compare list length
                    // Emit Branch or Switch depending on pattern count
                }
            }
        }
        DecisionTree::Leaf { arm_index, bindings } => {
            // Bind pattern variables by resolving paths from root scrutinee
            for (name, path) in bindings {
                let value = resolve_path(builder, root_scrutinee, path);
                builder.bind_variable(*name, value);
            }
            // Emit arm body (compile the arm's expression)
            let result = builder.compile_expr(arm_body[*arm_index]);
            // Jump to merge block with result
            builder.emit_jump(merge_block, &[result]);
        }
        DecisionTree::Guard { arm_index, bindings, on_fail } => {
            // Bind pattern variables
            for (name, path) in bindings {
                let value = resolve_path(builder, root_scrutinee, path);
                builder.bind_variable(*name, value);
            }
            // Compile guard expression
            let guard_result = builder.compile_expr(guard_expr[*arm_index]);
            // Branch: if guard passes, execute arm body; if fails, continue matching
            let body_block = builder.new_block("guard_pass");
            let fail_block = builder.new_block("guard_fail");
            builder.emit_branch(guard_result, body_block, fail_block);

            // Guard pass: execute arm body, jump to merge
            builder.position_at(body_block);
            let result = builder.compile_expr(arm_body[*arm_index]);
            builder.emit_jump(merge_block, &[result]);

            // Guard fail: continue matching with remaining compatible arms
            builder.position_at(fail_block);
            emit_decision_tree(builder, on_fail, root_scrutinee, merge_block);
        }
        DecisionTree::Fail => {
            // Exhaustiveness guarantees this is unreachable.
            // Maps to LLVM `unreachable` instruction.
            builder.emit_unreachable();
        }
    }
}

/// Navigate from root scrutinee to a sub-value via path instructions.
fn resolve_path(
    builder: &mut ArcIrBuilder,
    root: ArcVarId,
    path: &[PathInstruction],
) -> ArcVarId {
    let mut current = root;
    for step in path {
        current = match step {
            PathInstruction::TagPayload(field) => {
                builder.emit_project(current, *field)
            }
            PathInstruction::TupleIndex(idx) => {
                builder.emit_project(current, *idx)
            }
            PathInstruction::StructField(name) => {
                builder.emit_project_named(current, *name)
            }
            PathInstruction::ListElement(idx) => {
                builder.emit_list_index(current, *idx)
            }
        };
    }
    current
}
```

### Payload and Field Extraction

After branching on a tag, the payload fields must be extracted before pattern variables can be bound. This uses `Project` instructions in the ARC IR (which map to `struct_gep` in LLVM):

```
// ARC IR for matching Cons(head, tail):
block_case_cons:
    %head = Project { value: %scrutinee, field: 0 }  // First payload field
    %tail = Project { value: %scrutinee, field: 1 }  // Second payload field
    ... (use %head, %tail in arm body)
```

At LLVM emission time, each `Project` becomes a `struct_gep` instruction indexing into the variant's payload struct. The field index accounts for the tag field offset.

### LLVM Emission (in `ori_llvm`)

The LLVM emission layer is trivial because all pattern compilation happens in `ori_arc`:

```
ArcTerminator::Switch { scrutinee, cases, default }
    → LLVM `switch` instruction on the scrutinee value
    → Case values are i8/i16/i32 constants (tag type from TypeInfo)
    → Default block maps to the `default` ARC block

ArcTerminator::Branch { cond, then_block, else_block }
    → LLVM `br i1 %cond, label %then, label %else`

ArcInstr::Project { dst, value, field }
    → LLVM `struct_gep` or `extractvalue` instruction

ArcTerminator::Unreachable
    → LLVM `unreachable` instruction
```

No pattern-matching logic exists in `ori_llvm`. It is a mechanical translation of ARC IR terminators to LLVM instructions.

- [x] Implement `emit_decision_tree()` in ARC IR lowering
- [x] Implement `resolve_path()` for nested scrutinee navigation
- [x] Implement payload extraction via `Project` instructions after tag tests
- [x] Implement shared body blocks for or-patterns (same arm_index, one body)
- [x] Emit tag type from TypeInfo (not hardcoded i8)
- [x] Map `Fail` nodes to `Unreachable` terminators
- [x] Verify trivial LLVM emission: Switch -> LLVM switch, Branch -> LLVM br
- [ ] Benchmark: compare against current sequential approach

---

**Exit Criteria:** Match expressions compile to decision trees during AST-to-ARC-IR lowering. Enum matches produce `Switch` terminators (mapping to LLVM `switch`). Guards fall through to the next compatible arm, not the next sequential arm. Or-patterns share a single body block. Nested patterns use path-based scrutinee tracking. The LLVM emission layer contains no pattern compilation logic -- it mechanically translates ARC IR terminators.
