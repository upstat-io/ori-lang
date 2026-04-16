---
paths:
  - "compiler/ori_canon/**/*.rs"
---

# Canonicalization Phase (ori_canon)

## Pipeline Position

Stage 4: between type checking (`ori_types`) and both the evaluator (`ori_eval`) and ARC lowering (`ori_arc`). Input: type-checked AST (`ExprArena` + `TypeCheckResult`). Output: `CanExpr` + `CanArena` + `DecisionTreePool` (via `CanonResult`).

Both `ori_eval` and `ori_arc` consume the post-canonicalization form — neither operates on the raw AST.

## Responsibilities

Four phases, run in order:
1. **Desugaring** (`desugar/mod.rs`) — eliminates 7 canonical-IR sugar variants
2. **Pattern compilation** (`patterns/`) — Maranget algorithm → `DecisionTreePool`
3. **Constant folding** (`const_fold/`) — compile-time evaluation of constant expressions
4. **Type attachment** — every `CanNode` carries its resolved type

## The 7 Canonical-IR Desugars

These are distinct from the 7 surface desugars (performed earlier in parser/typeck, see `canon.md` §2). These eliminate IR-level sugar variants from `CanExpr`:

| # | Sugar Variant | Desugars To |
|---|---------------|-------------|
| 1 | `CallNamed` | Positional `Call` |
| 2 | `MethodCallNamed` | Positional `MethodCall` |
| 3 | `TemplateFull` | `Str` — trivial no-interpolation case (handled inline in `lower/expr.rs`) |
| 4 | `TemplateLiteral` | `.to_str()` / `FormatWith` + `.concat()` chain — interpolation/formatting case (`desugar/mod.rs`) |
| 5 | `ListWithSpread` | Method calls (append/extend) |
| 6 | `MapWithSpread` | Method calls (insert/extend) |
| 7 | `StructWithSpread` | Method calls (field-by-field copy + overrides) |

## Graph-first, manual second

Before reading the Maranget citation and cross-crate references below, query
the intelligence graph:

- `scripts/intel-query.sh --human similar "<symbol>" --repo rust,gleam,elm,roc,koka --limit 5`
  — semantic equivalents in functional-first pattern-compilation reference compilers
- `scripts/intel-query.sh --human callers "compile_multi_clause_patterns" --repo ori`
  — all callers of the unified pattern-compilation entry point
- `scripts/intel-query.sh --human file-symbols "patterns/" --repo ori` — the
  module inventory before editing the Maranget entry point
- `scripts/intel-query.sh --human ori-patterns --limit 5` — pre-curated subsystem
  view for pattern compilation + decision tree work

The graph covers Ori plus 10 reference compilers, synced on every commit. Manual reference-repo reading
stays authoritative — but only AFTER the graph narrows the search. Never
cite a graph result without verifying against the actual source. See
`.claude/rules/intelligence.md` for the canonical when-to-query workflow and subcommand reference and
`.claude/skills/query-intel/compose-intel-summary.md` for the canonical
query protocol used by review-family skills.

## Pattern Compilation — Maranget

Luc Maranget, "Compiling Pattern Matching to Good Decision Trees" (2008). Invocation: `compiler/ori_canon/src/patterns/`. Core primitives: `compiler/ori_arc/src/decision_tree/` (temporary location — migration target per module docs).

- **Input**: `PatternMatrix` with one row per arm
- **Output**: `DecisionTree` stored in `DecisionTreePool`, referenced by `DecisionTreeId` on `CanExpr::Match`
- **Exhaustiveness**: reachable `Fail` node → `PatternProblem::NonExhaustive` → `E3002` (non-exhaustive match, via `compiler/oric/src/problem/semantic/mod.rs`)
- **Usefulness**: arm index never in any `Leaf` → `PatternProblem::RedundantArm` → warning `E3003` (redundant pattern)
- **Guards**: do NOT contribute to exhaustiveness coverage — guarded-only matches require explicit `_` catch-all
- **Multi-clause functions**: lower through the same pipeline as explicit `match` via `compile_multi_clause_patterns`

## Output Invariants

After canonicalization (per `impl-hygiene.md` §Cross-Phase Invariant Contracts):

- No sugar variants remain in `CanExpr` (all 7 eliminated)
- All `TypeId`s are fully resolved (no `TypeId::INFER`)
- `CanExpr::Match` nodes carry `DecisionTreeId` pointing at compiled trees
- Every `CanNode` carries its resolved type
- Constant folding and structural validation have completed

## DecisionTree Consumers

| Consumer | How it uses the tree |
|---|---|
| `ori_eval` (`can_eval/control_flow.rs`) | Evaluates `DecisionTreePool` entries by walking nodes |
| `ori_arc` (`lower/control_flow/mod.rs`) | Lowers `CanExpr::Match` using `DecisionTreeId` → ARC IR blocks |
| `ori_llvm` | Does NOT consume `DecisionTreePool` — it sees ARC IR only |

## Public API

- `lower(src, type_result, pool, root, interner) -> CanonResult` — main entry point (single expression)
- `lower_module(module, src, type_result, pool, interner) -> CanonResult` — module-level
- `validate(result: &CanonResult) -> ()` — debug-only structural validation via `debug_assert!` panics (not a collected `Vec` — panics on first violation). Called automatically from `lower()` under `#[cfg(debug_assertions)]` only. This is a debug-mode assertion checker, not a production API.

## Internal Modules

- `desugar/` — 7-variant sugar elimination
- `patterns/` — Maranget pattern compilation entry point
- `const_fold/` — constant folding (integers, floats, booleans, strings, Duration arithmetic/comparisons with nanosecond normalization, Size arithmetic/comparisons with byte normalization and overflow bounds)
- `lower/` — AST → CanExpr lowering
- `validate.rs` — post-canonicalization structural checks (single file, not a directory)
- `exhaustiveness/` — exhaustiveness analysis (pub(crate))

## Cross-References

- `canon.md` §2 — surface desugars (performed before canonicalization, in parser/typeck)
- `canon.md` §3 — Maranget algorithm design details
- `canon.md` §4.3 — output invariants (mirrors this file)
- `impl-hygiene.md` §Cross-Phase Invariant Contracts — Canon → All rows
- `docs/compiler/design/07-canonicalization/pattern-compilation.md` — design rationale
