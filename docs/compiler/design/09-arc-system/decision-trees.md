---
title: "Decision Trees"
description: "Ori Compiler Design — Pattern Compilation in ARC IR"
order: 909
section: "ARC System"
---

# Decision Trees

## Overview

Pattern match compilation transforms `match` expressions into efficient decision trees
that test discriminants and branch to the correct arm body. The algorithm is based on
Maranget (2008), "Compiling Pattern Matching to Good Decision Trees," which produces
trees that minimize the number of tests needed to reach a decision.

Decision trees are compiled during canonicalization and stored in a shared pool. The ARC
lowering phase then emits them as basic blocks with Switch, Branch, and Jump terminators.

## Key Types

The core types are defined in `ori_ir::canon::tree` and shared across crates:

### DecisionTree

The compiled tree structure. Each node is one of:

- **Test node**: Tests a variable against a set of values and branches to subtrees.
  Contains the variable to test, the kind of test (`TestKind`), and a map from test
  values to child trees, plus a default/fallback subtree.
- **Leaf node**: A successful match. Contains the arm index and the variable bindings
  extracted from the matched pattern.
- **Guard node**: A test node where the branch condition is a user-written guard
  expression (`if` clause on a match arm). Contains the guard expression, a success
  subtree, and a failure subtree that falls through to the next candidate.

### PatternMatrix

A matrix of pattern rows being compiled. Each row corresponds to a match arm (or a
remaining candidate after specialization). Columns correspond to the components being
matched against. The compilation algorithm selects a column, generates tests for it,
and specializes the matrix for each test outcome.

### FlatPattern

A flattened representation of a pattern after normalization. Nested patterns (e.g.,
`Some(Point { x, y })`) are decomposed into a sequence of flat tests and bindings.
This simplifies the compilation algorithm, which operates on flat sequences rather
than recursive pattern trees.

### TestKind

Describes what kind of test to perform:

- **Tag test**: Check the variant tag of a sum type (e.g., `Some` vs `None`).
- **Literal test**: Compare against a literal value (integer, string, boolean).
- **Guard test**: Evaluate a boolean guard expression.
- **Range test**: Check membership in a range pattern.

### TestValue

The concrete value being tested against — a variant tag index, a literal constant,
or a range boundary. TestValues are the keys in a test node's branch map.

## Pipeline Integration

Decision trees occupy a specific position in the compilation pipeline:

1. **Canonicalization** (`ori_canon`): The `compile/` module builds decision trees from
   pattern matrices. Each `match` expression produces a `DecisionTree` that is stored
   in the `DecisionTreePool`.

2. **ARC lowering**: When lowering canonical IR to ARC IR, the emitter reads decision
   trees from the pool and generates basic blocks.

3. **LLVM codegen**: The ARC IR blocks produced by decision tree emission are translated
   to LLVM IR like any other basic blocks — no special handling is needed at this stage.

### Relevant Modules

- `compile/` — Builds the decision tree from the pattern matrix using the Maranget
  algorithm. Selects the best column to split on using a heuristic (typically the
  column with the most distinct constructors, to maximize information gain per test).
- `emit.rs` — Emits the decision tree as ARC IR. Walks the tree and creates basic
  blocks, terminators, and variable bindings.
- `emit_switches.rs` — Switch-based emission for multi-arm matches. Generates Switch
  terminators that map discriminant values to target blocks, producing efficient jump
  tables when the backend supports them.
- `flatten.rs` — Converts nested patterns to the flat representation used by the
  compilation algorithm.
- `specialize.rs` — Matrix specialization for narrowing candidates at each decision
  point.

## Emission to ARC IR

Each node type in the decision tree maps to an ARC IR construct:

- **Test node** becomes a basic block ending in a `Switch` terminator. The Switch maps
  each `TestValue` to the block ID of the corresponding subtree's entry block. The
  default case maps to the fallback subtree.

- **Leaf node** becomes a basic block ending in a `Jump` terminator to the arm body
  block. Before the jump, the block contains instructions to bind the extracted
  variables (copies or moves from the matched value's fields).

- **Guard node** becomes a basic block ending in a `Branch` terminator. The guard
  expression is evaluated to produce a boolean, and the branch targets the success
  subtree (guard true) or the failure subtree (guard false, which continues matching
  against remaining candidates).

Variable bindings extracted during pattern matching are propagated through block
parameters, ensuring SSA form is maintained across the decision tree's block structure.

## Specialization Algorithm

Matrix specialization is the core operation of the Maranget algorithm. At each decision
point, the algorithm selects a column and generates tests for the distinct constructors
(or literals) that appear in that column. For each constructor `c`, the matrix is
specialized:

1. **Remove** rows whose pattern in the selected column cannot match `c`.
2. **Simplify** rows whose pattern does match `c` — replace the constructor pattern
   with its sub-patterns (fields/payloads), expanding the matrix horizontally.
3. **Wildcard rows** (patterns that match anything) are kept in all specializations,
   since they match regardless of the constructor.

The result is a smaller matrix for each branch of the test, and the algorithm recurses
until all matrices are reduced to leaf nodes (single matching arm) or empty (unreachable,
which should not occur if exhaustiveness checking passed).

### Single-Constructor Optimization

When a type has exactly one constructor (e.g., a struct, a tuple, or a single-variant
sum type), the specialization can skip the test entirely. There is only one possible
outcome, so no branch is needed — the algorithm immediately decomposes the pattern
into its sub-patterns and continues. This avoids generating a Switch with a single
arm, which would be a redundant indirect jump.

### Heuristic Column Selection

The algorithm must choose which column to split on at each step. The heuristic favors
columns where:

- The number of distinct constructors is high (more information per test).
- Wildcard patterns are few (fewer rows propagated to all branches).
- The column appears leftmost among equally good candidates (stability).

Good column selection directly affects the size of the generated decision tree and the
number of tests executed at runtime.

## Connection to Pattern Exhaustiveness

Decision tree compilation is downstream of exhaustiveness checking, which runs during
type checking. By the time decision trees are compiled, every match expression is
guaranteed to be exhaustive. This means the compilation algorithm can assume that the
default/fallback case of a fully-covered test is unreachable, enabling the backend to
omit the default branch or mark it as `unreachable` for optimization.

Guard expressions are the exception: guards can cause a match arm to fail at runtime,
so the fallback after a guarded arm must always be present, routing to the next
candidate row.
