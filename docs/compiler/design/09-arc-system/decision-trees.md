---
title: "Decision Trees"
description: "Ori Compiler Design — Pattern Compilation in ARC IR"
order: 909
section: "ARC System"
---

# Decision Trees

## Pattern Matching at the Machine Level

Pattern matching is a central feature of ML-family languages, but the way programmers write patterns — nested constructors, wildcards, guards, or-patterns — is very different from how machines execute them. A `match` expression with five arms testing nested enum variants cannot be executed as written; it must be compiled into a sequence of tests and branches that a CPU can execute efficiently.

The classical approach, developed by Augustsson (1985) and refined by [Maranget (2008)](https://doi.org/10.1017/S0956796808006771) in "Compiling Pattern Matching to Good Decision Trees," transforms pattern matrices into **decision trees** — branching structures where each internal node tests a single discriminant (enum tag, literal value, type tag) and each leaf represents a successful match. The goal is to minimize the number of tests executed on any path from root to leaf.

Decision trees are compiled during [canonicalization](../07-canonicalization/pattern-compilation.md) and stored in a shared pool (`DecisionTreePool`). The ARC system's role is to **emit** these pre-compiled trees as basic blocks in ARC IR, converting the tree's branching structure into `Switch`, `Branch`, and `Jump` terminators that the LLVM backend can translate directly to machine code.

## Decision Tree Structure

The core types are defined in `ori_ir` and shared across crates:

### DecisionTree Nodes

Each node in the tree is one of three kinds:

**Test node.** Tests a variable against a set of values and branches to subtrees. Contains the variable to test, the kind of test (`TestKind`), a map from test values to child trees, and a default/fallback subtree for unmatched cases.

**Leaf node.** A successful match. Contains the arm index (which match arm body to execute) and the variable bindings extracted from the matched pattern.

**Guard node.** A test node where the branch condition is a user-written guard expression (`if` clause). Contains the guard expression, a success subtree, and a failure subtree that falls through to the next candidate.

### Test Kinds

| Kind | What It Tests | Example |
|------|-------------|---------|
| Tag test | Variant discriminant of a sum type | `Some` vs `None` |
| Literal test | Equality with a literal value | `0`, `"hello"`, `true` |
| Guard test | Boolean guard expression | `x if x > 0` |
| Range test | Membership in a numeric range | `1..10` |

### Pattern Matrix

The compilation input is a **pattern matrix** — rows correspond to match arms, columns correspond to the components being tested. The Maranget algorithm selects a column, generates tests for it, and specializes the matrix for each test outcome, recursing until all matrices are reduced to single-row leaves.

## Pipeline Position

Decision trees occupy a specific position spanning two phases:

```mermaid
flowchart LR
    Canon["Canonicalization
    Compile pattern matrix
    → DecisionTree"] --> Pool["DecisionTreePool
    Shared storage"]
    Pool --> Emit["ARC Lowering
    Emit tree as
    basic blocks"]
    Emit --> LLVM["LLVM Codegen
    Standard block
    translation"]

    classDef canon fill:#3b1f6e,stroke:#a78bfa,color:#e9d5ff
    classDef native fill:#5c3a1e,stroke:#f59e0b,color:#fef3c7

    class Canon,Pool canon
    class Emit,LLVM native
```

The split is deliberate: pattern compilation is a frontend concern (it depends on type information, exhaustiveness checking, and pattern semantics), while block emission is a backend concern (it produces the control flow graph that ARC analysis operates on). The `DecisionTreePool` is the handoff point between the two.

## Emission to ARC IR

Each decision tree node maps to an ARC IR construct:

**Test node → Switch terminator.** The test variable's discriminant is loaded, and a `Switch` terminator maps each `TestValue` to the block ID of the corresponding subtree's entry. The default case maps to the fallback subtree. When the type's constructors are fully covered and no guard is present, the default is marked `Unreachable`.

**Leaf node → Jump terminator.** A block containing instructions to bind the extracted variables (via `Project` to extract fields from the matched value, then `Let` to bind them), ending with a `Jump` to the match arm's body block.

**Guard node → Branch terminator.** The guard expression is evaluated to produce a boolean, and a `Branch` terminator targets the success subtree (guard true) or the failure subtree (guard false, continuing to the next candidate).

Variable bindings are propagated through block parameters, maintaining SSA form across the decision tree's block structure.

## Specialization

Matrix specialization is the core operation of the Maranget algorithm. At each decision point:

1. **Select a column** — using a heuristic that favors columns with many distinct constructors (maximizing information gain per test) and few wildcards (minimizing row duplication).

2. **For each constructor `c` in the column:**
   - **Remove** rows whose pattern cannot match `c`
   - **Simplify** matching rows by replacing the constructor pattern with its sub-patterns (expanding the matrix horizontally)
   - **Keep** wildcard rows in all specializations (they match any constructor)

3. **Recurse** on each specialized sub-matrix until all are reduced to single-row leaves (successful match) or empty matrices (unreachable — should not occur after exhaustiveness checking).

### Single-Constructor Optimization

When a type has exactly one constructor (structs, tuples, single-variant enums), the test can be skipped entirely. There is only one possible outcome, so no branch is needed — the algorithm immediately decomposes the pattern into sub-patterns and continues. This avoids generating a `Switch` with a single arm, which would be a redundant indirect jump.

### Select Inlining

When a `Switch` has exactly two arms and both target blocks contain only a single assignment, the switch can be replaced with a `Select` instruction — a conditional value selection that maps to LLVM's `select` instruction (a branchless conditional move). This eliminates the branch overhead for trivial two-way matches like `Option` unwrapping.

## Connection to Exhaustiveness

Decision tree compilation is downstream of exhaustiveness checking, which runs during type checking. By the time trees are compiled, every `match` is guaranteed exhaustive. This means:

- The default case of a fully-covered test is `Unreachable` — the backend can omit it or use it for optimization hints
- No "missing case" errors can arise during emission
- Guard expressions are the sole source of runtime fallthrough — guarded arms must always have a failure path to the next candidate

## Prior Art

**[Maranget (2008)](https://doi.org/10.1017/S0956796808006771)**, "Compiling Pattern Matching to Good Decision Trees," is the foundational reference. The paper describes the matrix specialization algorithm with heuristic column selection that Ori implements. The algorithm produces decision trees that are near-optimal in the number of tests per path.

**[Augustsson (1985)](https://link.springer.com/chapter/10.1007/3-540-15975-4_48)** and **[Wadler (1987)](https://dl.acm.org/doi/10.5555/41554.41556)** developed the earlier pattern compilation algorithms that Maranget's work builds on. Augustsson's algorithm compiles to nested case expressions; Wadler described the relationship between pattern matching and function clauses.

**[GHC](https://gitlab.haskell.org/ghc/ghc)** uses a variant of the Maranget algorithm for Haskell's pattern matching, producing Core case expressions that are later lowered to STG and then Cmm. GHC's implementation handles Haskell-specific features (view patterns, pattern synonyms) that Ori does not need.

**[Rust](https://github.com/rust-lang/rust)** compiles patterns to MIR using a similar matrix-based algorithm. Rust's implementation (`rustc_mir_build/src/thir/pattern/`) handles Rust-specific features (slice patterns, constant patterns, binding modes) and produces MIR `SwitchInt` terminators analogous to ARC IR's `Switch`.

**[Elm](https://github.com/elm/compiler)** uses a decision tree approach for its pattern matching, with the additional constraint that Elm's type system makes all matches total (no runtime failures). This is similar to Ori's guarantee via exhaustiveness checking.

## Design Tradeoffs

**Shared pool vs inline trees.** Decision trees are stored in a `DecisionTreePool` during canonicalization and retrieved by reference during ARC lowering. The alternative — embedding the tree structure directly in the canonical IR — would avoid the pool indirection but would complicate canonical IR with tree-specific types. The pool keeps the canonical IR focused on expressions.

**Two-phase (compile then emit) vs single-phase.** Compiling decision trees during canonicalization and emitting during ARC lowering separates concerns: canonicalization handles pattern semantics, ARC lowering handles control flow. The alternative — compiling and emitting in one phase — would be simpler but would either pull pattern compilation into the ARC system (wrong abstraction level) or push block emission into canonicalization (wrong phase).

**Heuristic column selection.** The choice of which column to split on affects tree size and runtime test count. Maranget's heuristic (prefer columns with many constructors, few wildcards) is simple and effective but not optimal in all cases. More sophisticated heuristics (e.g., considering the depth of sub-trees) could produce smaller trees but at higher compile-time cost.
