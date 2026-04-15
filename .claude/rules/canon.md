---
paths:
  - "**"
---

# Canon — Ori Compiler Pipeline SSOT

**Purpose.** canon.md is the factual, spec-driven map of the Ori compiler pipeline. It records how the pipeline is supposed to work — phase boundaries, canonical desugars, pattern-compilation algorithm, per-phase output invariants, and the authoritative homes for cross-cutting knowledge. It is descriptive, not prescriptive; fact-based, not aspirational.

**Scope.** canon.md stitches `parse.md`, `typeck.md`, `types.md`, `aims-rules.md`, `codegen-rules.md`, `arc.md`, `compiler.md`, `patterns.md`, `ir.md`, `llvm.md`, `eval.md`, `runtime.md`, and `impl-hygiene.md` into one navigation layer. It cites — it does not duplicate.

**Spec.** `docs/ori_lang/v2026/spec/` is authoritative for surface language semantics. `grammar.ebnf` fixes syntax; `operator-rules.md` fixes operator semantics. canon.md cites clauses; it does not re-specify them.

---

## §1 Pipeline Overview

The Ori compiler is a strictly-ordered, layered pipeline. Each phase reads the prior phase's output and produces a new IR. Phases 1–4 produce distinct output IRs; the ARC/AIMS pipeline (phases 5–7) mutates `ArcFunction` in place while preserving strict phase ordering.

| # | Phase | Crate | Input | Output | Authoritative home |
|---|-------|-------|-------|--------|--------------------|
| 1 | Lex | `ori_lexer` | Source bytes | `TokenList` (parallel `tokens` / `tags` / `flags` arrays) | `parse.md` §LB-2, §LB-4 |
| 2 | Parse | `ori_parse` | `TokenList` | AST in `ExprArena` (opaque `ExprId`) | `parse.md` §AR-1, §LB-6 |
| 3 | Type check | `ori_types` | AST | Typed IR (every `ExprId` has a resolved `Idx`) | `typeck.md` §PC-2 |
| 4 | Canonicalize | `ori_canon` | Typed IR | `CanExpr` with sugar eliminated + `DecisionTreePool` populated | `canonicalization.md`, `impl-hygiene.md` §Cross-Phase Invariant Contracts (Canon → All) |
| 5 | ARC lowering | `ori_arc` | `CanExpr` | `ArcFunction` (ARC IR with unresolved RC / reuse / drop decisions) | `arc.md`, `aims-rules.md` §7 |
| 6 | AIMS lattice analysis | `ori_arc` | `ArcFunction` | Per-SSA lattice state; converged `AimsStateMap`; per-function `MemoryContract` | `aims-rules.md` §§1–7 |
| 7 | ARC realization | `ori_arc` | `AimsStateMap` + `ArcFunction` | `ArcFunction` with RC / COW / reuse / drop instructions materialized; `FipContract` certified (Step 5a) | `aims-rules.md` §8, §VF-6 |
| 8 | LLVM codegen | `ori_llvm` | Realized `ArcFunction` | LLVM IR (verified per VR-1) | `codegen-rules.md`, `llvm.md` |
| 9 | Optimize & emit | `ori_llvm` | LLVM IR | Object / executable | `aot.md` |
| — | Evaluator (parallel) | `ori_eval` | `CanExpr` + `DecisionTreePool` | Runtime values (for const-eval and `ori run`) | `eval.md` |

Upstream crate dependency order per `compiler.md` §Architecture: `oric` → `ori_llvm` → `ori_arc/ori_repr` → `ori_canon` → `ori_types/eval/patterns` → `ori_parse` → `ori_lexer` → `ori_ir/diagnostic`. Support crates: `ori_compiler` (pure facade), `ori_registry`, `ori_stack`, `ori_fmt`, `ori_test_harness`, `ori_rt`. IO lives only in `oric`; core crates are pure (`compiler.md` §Phase-Specific Purity).

**Cross-crate note on pattern compilation.** Maranget pattern compilation is invoked from `ori_canon::patterns`, which currently delegates the core primitives to `ori_arc::decision_tree`. `ori_canon/Cargo.toml` declares `ori_arc` as a direct dependency; the `ori_arc::decision_tree` module notes that the primitives are temporarily housed there and consumed by `ori_canon`. This is the one non-upstream edge in the current pipeline — tracked as a migration target by its module documentation.

**Decision-tree consumers.** `DecisionTreePool` entries are consumed by `ori_eval` (`compiler/ori_eval/src/interpreter/can_eval/control_flow.rs`) and by `ori_arc` during ARC lowering of `CanExpr::Match` (`compiler/ori_arc/src/lower/control_flow/mod.rs`). `ori_llvm` consumes the emitted ARC IR, not `DecisionTreePool` directly (`llvm.md`).

---

## §2 The Seven Canonical Surface Desugars

Desugars in this section are surface-language rewrites performed at parse time or in the type checker. They eliminate sugared forms before the canonical-IR stage (§4.3 lists a distinct set of canonical-IR sugar variants eliminated by `ori_canon::desugar`).

| # | Desugar | Source form | Target form | Phase | Rule |
|---|---------|-------------|-------------|-------|------|
| 1 | Compound assignment | `x op= y` | `x = x op y` | Parser (parsed above Pratt); transformation described in typeck | `parse.md` §PR-5 (parse site); `typeck.md` §EX-17 (transformation) |
| 2 | Binary operator trait dispatch | `a + b`, `a == b`, `a < b`, `a & b` | `Add::add(a, b)`, `Eq::equals(a, b)`, `Comparable::compare(a, b).is_less()`, `BitAnd::bit_and(a, b)` | Type checker | `typeck.md` §EX-2, `spec/operator-rules.md` |
| 3 | Pipe operator | `x \|> f(a: v)` | `{ let $_tmp = x; f(a: v, _pipe_slot: _tmp) }` (first unspecified, default-less parameter) | Type checker | `typeck.md` §EX-13 |
| 4 | Index assignment | `list[i] = v` | `list = list.updated(key: i, value: v)` | Type checker | `typeck.md` §EX-17 |
| 5 | Field assignment | `state.f = v` | `state = { ...state, f: v }` | Type checker | `typeck.md` §EX-17 |
| 6 | Argument punning | `f(x:)` | `f(x: x)` (requires `x` in scope with matching name) | Type checker | `typeck.md` §EX-3 |
| 7 | Variant / pattern punning | `Some(value:)` pattern | `Some(value: value)` pattern | Type checker | `typeck.md` §CF-2 |

Notes:

- Compound assignment (§EX-17) is the only surface desugar with a parse-time component (`parse.md` §PR-5 parses it at top level, above the Pratt loop); the checker types the desugared form.
- `x |> .method()` desugars to `_tmp.method()` with the same temporary binding; `x |> (y -> e)` applies the lambda to `x` (`typeck.md` §EX-13).
- Operator dispatch goes through the registry — it does not hardcode primitive types (`typeck.md` §EX-2, `types.md` §RG-4).

---

## §3 Pattern Compilation — Maranget

Ori compiles `match` expressions and multi-clause function definitions through the Maranget algorithm — Luc Maranget, "Compiling Pattern Matching to Good Decision Trees" (2008). Invocation lives in `compiler/ori_canon/src/patterns/`; the core primitives currently live in `compiler/ori_arc/src/decision_tree/` (per §1 migration note). The compiled tree is stored in `DecisionTreePool` and referenced by `DecisionTreeId` on `CanExpr::Match` nodes. `ori_eval` and `ori_arc` share the exact same tree instances; `ori_llvm` sees the ARC IR emitted from that lowering and does not consume `DecisionTreePool` directly (`docs/compiler/design/07-canonicalization/pattern-compilation.md`).

**Input.** A `PatternMatrix` with one row per arm. A single-scrutinee `match` has one column with an empty path; a multi-parameter function definition has N columns with `TupleIndex(i)` scrutinee paths. Guards are attached to rows but do not participate in column selection.

**Output.** A `DecisionTree` whose `Leaf` nodes identify arm indices and whose internal nodes are constructor tests driven by column heuristics. `Fail` nodes mark uncovered value classes.

**Dual outputs — exhaustiveness and usefulness.** Both analyses fall out of the same traversal (`docs/compiler/design/07-canonicalization/pattern-compilation.md` §Integrated Exhaustiveness via Tree Walking, §Exhaustiveness and Usefulness):

- **Non-exhaustiveness** — a reachable `Fail` node in the compiled tree. Canonicalization records `PatternProblem::NonExhaustive`; the driver maps this to error `E3002` with the uncovered pattern shape (`compiler/oric/src/problem/semantic/mod.rs`).
- **Useless arm (redundancy)** — an arm index that never appears in any `Leaf`. Canonicalization records `PatternProblem::RedundantArm`; the driver maps this to warning `E3003`.

The compiled tree and the source patterns accept the same set of values, so the tree faithfully encodes the pattern matrix's coverage — one algorithm, two outputs.

**Guards.** Guards do NOT contribute to exhaustiveness coverage (`typeck.md` §CF-4). A guarded arm `P if cond -> e` is treated as unreachable under its own guard, so a match whose arms are ALL guarded requires an explicit `_` catch-all; otherwise `E3002`.

**Unified path.** Multi-clause function definitions lower to the same Maranget pipeline as explicit `match`. The shared entry point is `compile_multi_clause_patterns` in `compiler/ori_canon/src/patterns/mod.rs`, so there is exactly one code path for pattern compilation in the compiler.

**ARC integration.** `ori_arc` lowers `CanExpr::Match` using the `DecisionTreeId` stored on the node (`arc.md`). RC, reuse, and drop instructions are scheduled against the compiled tree, not against the source `match` arms.

---

## §4 Per-Phase Output Invariants

Each phase's output is a contract. Invariant enforcement varies by phase: some checks are `debug_assert!` (debug-only), some are behind opt-in flags (`ORI_VERIFY_ARC=1`), and some are always-on. Canonicalization validation is debug-only (`#[cfg(debug_assertions)]` in `ori_canon::lower`); LLVM IR verification runs at `VR-1` checkpoints when `verify_arc` is enabled; AIMS verification runs when `verify_arc` is set in `AimsPipelineConfig`. The goal (`impl-hygiene.md` §Cross-Phase Invariant Contracts) is that release builds produce clear internal errors on violation — currently achieved for type-checking contracts (always-on) but opt-in for ARC/LLVM verification.

### §4.0 Lex output — `TokenList`

- Tokens are stored as `Token { kind: TokenKind, span: Span }` in a `TokenList` with parallel `tokens`, `tags`, and `flags` arrays (`parse.md` §LB-2).
- Identifiers are interned as `Name` at lex time; literal payloads are cooked lazily (`parse.md` §LB-4).
- Lexer warnings (e.g. `DetachedDocComment`) are emitted by the lexer and processed by `oric` before parsing (`parse.md` §LB-6 note).
- No semantic state — no names resolved, no types, no scopes (`parse.md` §LB-5; `compiler.md` §Phase-Specific Purity).

### §4.1 Parse output — AST in `ExprArena`

- Every AST node carries a `Span` (consumed by `typeck.md` §PC-1).
- All AST nodes are arena-allocated via `ExprArena::alloc_expr()`; consumers hold opaque `ExprId` values (`parse.md` §AR-1).
- `ExprId` is a private `u32` newtype; raw construction outside the arena is forbidden (`parse.md` §AR-3).
- All tokens are consumed; no residual parser state (`typeck.md` §PC-1).
- `ExprKind::Error` nodes appear only where recovery occurred, and only for lexer-produced `TokenKind::Error` tokens; general parse errors are accumulated, not materialized as AST nodes (`parse.md` §ER-4).
- `TypeId::INFER` is permitted; other `TypeId`s are either pre-interned primitives or pending-resolution nominal refs (`typeck.md` §PC-1).
- Parse errors are accumulated in a deferred error list, not thrown (`parse.md` §DI-7).
- Compound assignment is desugared at parse time (`parse.md` §PR-5); no `op=` expression reaches typeck.

### §4.2 Type checker output — Typed IR

On successful check, the typed IR satisfies exactly the contract in `typeck.md` §PC-2 (mirrored by `types.md` §PC-2):

- No `Tag::Var` in any type position.
- No `Tag::Infer`.
- No `Tag::Projection` (all associated-type references normalized).
- No `Tag::SelfType` (all substituted to the implementing type).
- All `Tag::Named` entries resolved to `TypeRegistry` entries.
- All method calls resolved statically via builtin-first `resolve_builtin_method()` or `TraitRegistry::lookup_method()` — no runtime lookup remains (`typeck.md` §EX-4, `types.md` §RG-3; builtin lookup is NOT routed through `MethodRegistry`).
- All capability requirements satisfied at their use sites (`typeck.md` §CP-2).

Producer-side enforcement for the `no Tag::Var` clause is `compiler/ori_types/src/check/validators/mod.rs::validate_body_types` (re-exported at the `ori_types` crate root). It walks each body's `expr_types` plus the body's `FunctionSig` (`param_types` + `return_type`) after `InferEngine` body-checking completes and emits `E2005` (`typeck.md` §DI-1 `AmbiguousType`) once per `ExprIndex` for any surviving unbound `Tag::Var`. Gate order is `resolve_fully → HAS_ERROR → HAS_VAR` (`typeck.md` §ER-4 cascade suppression precedes `types.md` §TF-5 fast-path). Tag-dispatch child traversal delegates to the canonical `Pool::visit_children` helper (`types.md` §TF-3 propagation + the `Tag::Scheme` flag-propagation rule) — no parallel tag-dispatch ladder is maintained in the validator (`impl-hygiene.md` §Algorithmic DRY).

On failed check, failure sites carry `Tag::Error`; downstream phases skip error-typed nodes (`typeck.md` §PC-3). Codegen is gated at the driver level — any typeck error suppresses emission (`typeck.md` §PC-4).

### §4.3 Canonicalization output — `CanExpr`

After `ori_canon` has run (`impl-hygiene.md` §Cross-Phase Invariant Contracts, Canon → All rows):

- No sugar variants remain in `CanExpr` — `ori_canon::desugar` eliminates the seven canonical-IR sugar variants `CallNamed`, `MethodCallNamed`, `TemplateFull`, `TemplateLiteral`, `ListWithSpread`, `MapWithSpread`, `StructWithSpread` (`compiler/ori_canon/src/lib.rs`, `compiler/ori_canon/src/desugar/mod.rs`). This is distinct from the seven surface desugars in §2, which are eliminated earlier (parser / type checker).
- All `TypeId`s are fully resolved (no `TypeId::INFER` in canonical IR).
- `CanExpr::Match` nodes carry a `DecisionTreeId` pointing at a compiled tree in `DecisionTreePool`; source patterns are not re-walked downstream (§3).
- Constant folding (`ori_canon::const_fold`) and structural validation (`ori_canon::validate`) have run; every `CanNode` carries its resolved type.
- `ori_eval` and `ori_arc` both consume the post-canonicalization form.

### §4.4 AIMS lattice output — `AimsStateMap` (Steps 1–4)

After Step 4 (`analyze_function`) in the AIMS pipeline (`aims-rules.md` §7):

- Every SSA variable has a lattice tuple across all seven dimensions — `Access × Consumption × Cardinality × Uniqueness × Locality × Shape × Effect` (`aims-rules.md` §§1.1–1.7). CLAUDE.md's §AIMS refers to these same dimensions using the `AccessClass` / `ShapeClass` / `EffectClass` shorthand; the authoritative names used in analysis are the bare forms in `aims-rules.md`.
- Every tuple is canonicalized per the cross-dimensional feasibility rules (`CN-1`, `CN-2`, `CN-3`, `CN-5`, `CN-6`, `CN-8`); fixed point is reached in one pass in practice, defended by a bounded (max 3-round) loop (`aims-rules.md` §2).
- Every function has a converged `MemoryContract` — per-parameter `ParamContract` + `ReturnContract` + `EffectSummary` + `ContextBehavior` + provisional `FipContract` — produced by interprocedural analysis (`aims-rules.md` §5). The `FipContract` field on `MemoryContract` is assigned during interprocedural extraction (`compiler/ori_arc/src/aims/interprocedural/extract.rs`); reuse emission reads this provisional value. Step 5a (`verify_fip_contract`) later verifies or downgrades it against the realized IR (`aims-rules.md` §5 `IC-6`, §7).
- Analysis is backward (demand-based); interprocedural contracts are computed SCC-topologically, callees before callers (`aims-rules.md` §7, `PL-1a`).
- No pass relies on a stale summary (`aims-rules.md` §7, `PL-5`; CLAUDE.md §AIMS invariant 3).

### §4.5 ARC realization output — Realized ARC IR (Step 5 and Step 5a)

After Step 5 (`realize_annotations` — phase 2 COW + drops), Step 5a (`verify_fip_contract`), and post-pipeline optimization passes (`aims-rules.md` §7–§8):

- RC is balanced per block: every owned non-scalar heap value either (a) is handed off via a listed ownership-transferring instruction, or (b) has a matching `RcDec` at its last use or scope exit or CFG edge (`aims-rules.md` §8 `RL-2`, `RL-4`, `RL-5`).
- Drops are placed at last-use or scope-exit for owned values; unused owned non-scalar definitions receive an immediate cleanup dec (`aims-rules.md` §8 `RL-2`).
- COW diamonds are contracted to a single compound instruction where the CFG pattern permits (`aims-rules.md` §8 `RL-9`).
- Reuse decisions respect `Uniqueness ≠ Shared` (`aims-rules.md` §4 `DP-6`, `CN-3`).
- `FipContract::Certified` functions have zero unmatched alloc/dealloc (`aims-rules.md` §5 `IC-6`, §9 `VF-6`).
- TRMC-rewritten functions pass structural verification (`aims-rules.md` §7 `PL-10`).
- Structural verification (`VF-1`), AIMS contract consistency (`VF-2`), oracle cross-check (`VF-3`), and FIP certification (`VF-4`) all pass (`aims-rules.md` §9 Verification Layers).

### §4.6 LLVM IR output

After `codegen/arc_emitter/` translation (`codegen-rules.md`):

- Every type index is fully resolved via `pool.resolve_fully(idx)` before LLVM type construction; no `Tag::Var` reaches codegen (`codegen-rules.md` §TR-2).
- Every emitted LLVM function passes `fn_val.verify(true)` at the VR-1 checkpoints (post-emission, post-trampoline, post-derive); `ORI_VERIFY_ARC=1` runs verification at all checkpoints (`codegen-rules.md` §VR-1). Trampoline verification specifically is covered by §TM-8.
- Trampolines use canonical types; narrowing happens only at storage boundaries (`codegen-rules.md` §NR-1, §TM-2).
- RC operations emitted match the AIMS lattice decisions (`codegen-rules.md` §5, `aims-rules.md` §8); drop placement satisfies `aims-rules.md` §8 `RL-2`.
- ABI passing and classification honor target rules per `codegen-rules.md` §§AB-1 through AB-7 (indirect-passing threshold, `ParamPassing` / `ReturnPassing` classification, `sret` on ARM64, FastISel aggregate restriction, calling convention assignment, ownership-aware ABI).

---

## §5 Phase Purity and No-Bleed Rules

Phase boundaries are one-way. Each phase consumes only its defined input and produces only its defined output; no cross-phase shortcuts (`compiler.md` §Phase-Specific Purity).

- **Lexer** — scans with minimal local state (nesting depth, mode stack); produces `(tag, len)` pairs. Holds no semantic state — no names, no types, no scopes (`parse.md` §LB-5).
- **Parser** — builds AST from tokens. Owns syntax, declaration-shape validation, attribute placement / applicability checks, and parse-time warnings. Does not perform name resolution, type checking, or deeper semantic analysis. Contextual keyword resolution is syntactic disambiguation, not semantic analysis (`parse.md` §LB-6).
- **Type checker** — consumes AST, produces typed IR. Does not re-parse; does not codegen. Salsa queries are pure: same inputs → same outputs (`typeck.md` §SL-1).
- **Canonicalizer** — consumes typed IR, produces `CanExpr`. Eliminates canonical-IR sugar (§4.3), compiles patterns, folds constants, validates structure. Does not re-type-check; does not codegen (`impl-hygiene.md` §Cross-Phase Invariant Contracts).
- **Evaluator** — interprets `CanExpr`. Does not re-type-check; does not codegen (`eval.md`).
- **ARC pass** — lowers `CanExpr` to `ArcFunction`, then analyzes ownership and realizes RC / COW / reuse / drop decisions (`arc.md`, `aims-rules.md`). Does not codegen; does not interpret.
- **LLVM codegen** — emits LLVM IR from realized `ArcFunction`. Does not interpret; does not re-type-check (`codegen-rules.md`).
- **Diagnostics** — formats and renders errors. Holds no phase logic; performs no semantic analysis.
- **Optimization passes** — read IR, produce transformed IR. Analysis is pass-local.

Cross-phase contracts are listed in `impl-hygiene.md` §Cross-Phase Invariant Contracts; every violation is a phase-purity bug, not a local code issue. The current `ori_canon → ori_arc::decision_tree` dependency noted in §1 is the one crate-level upward edge in the active pipeline and is tracked as a migration target by its module documentation.

---

## §6 Single Sources of Truth (SSOTs)

One canonical home per concern. Consumers are listed where they aid navigation but are not alternate homes.

| Concern | Canonical home | Consumers / related |
|---------|----------------|---------------------|
| Surface syntax (grammar) | `docs/ori_lang/v2026/spec/grammar.ebnf` | `ori-syntax.md` (quick reference) |
| Operator semantics | `docs/ori_lang/v2026/spec/operator-rules.md` | `typeck.md` §EX-2 (trait dispatch) |
| Surface language clauses | `docs/ori_lang/v2026/spec/` (Clauses 1–27, Annexes A–E) | — |
| Parser rules (lex + parse) | `.claude/rules/parse.md` | — |
| Type checker rules | `.claude/rules/typeck.md` | — |
| Type pool, tags, interning, registries | `.claude/rules/types.md` | — |
| Method lookup / static dispatch partition | `.claude/rules/types.md` §RG-3 | `typeck.md` §EX-4 (caller rule) |
| Builtin type behavior + operator / `MethodDef` data | `.claude/rules/registry.md` | `ori_registry` crate |
| Canonicalization phase rules | `.claude/rules/canonicalization.md` | `impl-hygiene.md` §Cross-Phase Invariant Contracts (Canon → All), `docs/compiler/design/07-canonicalization/pattern-compilation.md` (design rationale) |
| AIMS lattice + pipeline + verification | `.claude/rules/aims-rules.md` | — |
| ARC IR shape | `.claude/rules/arc.md` | — |
| Function-expression patterns (Recurse / Parallel / …) | `.claude/rules/patterns.md` | — |
| LLVM codegen rules | `.claude/rules/codegen-rules.md` | `llvm.md` (LLVM-binding specifics) |
| Representation layout policy (`ReprPlan`, narrowing scope, niche encoding, `#repr(...)` semantics) | `.claude/rules/repr.md` | `codegen-rules.md` §1 `TR-*` (physical LLVM mapping), §3 `NR-*` (narrowing emission), §7 `RT-2` (RC header schema) |
| Evaluator | `.claude/rules/eval.md` | — |
| Runtime (`ori_rt`) | `.claude/rules/runtime.md` | — |
| AOT flow | `.claude/rules/aot.md` | — |
| Implementation hygiene + cross-phase invariants | `.claude/rules/impl-hygiene.md` | — |
| Derived traits (enum + `method_name` contract) | `.claude/rules/ir.md` §DerivedTrait | — |
| Testing policy + matrix rule | `.claude/rules/tests.md` | `CLAUDE.md` §TDD (process wrapper) |
| Formatter rules (`ori_fmt`) | `.claude/rules/fmt.md` | — |
| Pre-interned primitive indices | `.claude/rules/types.md` §TY-5 | — |

---

## §7 Non-Negotiable Invariants

### §7.1 AIMS — Five Load-Bearing Invariants (CLAUDE.md §AIMS)

1. **Contract ↔ realization agreement.** `FipContract::Certified` ↔ zero unmatched alloc / dealloc in realized IR (`aims-rules.md` §9, `VF-6`).
2. **Active rewrites are sound.** Identical observable behavior; behavioral verification required for every active rewrite (`aims-rules.md` §9, `VF-7` tiers a/b/c).
3. **No pass relies on stale summaries.** Pipeline ordering is load-bearing (`aims-rules.md` §7, `PL-5`).
4. **Every active subsystem is end-to-end verified.** Implementation + invariant enforcement + tests — all three (`aims-rules.md` §9, `VF-5`).
5. **The unified model stays unified.** New capabilities extend a lattice dimension, extend a contract field, or feed the lattice-driven analysis as a typed pre-pass input. No shadow RC emission paths; no parallel escape enum; no independent uniqueness tracker.

### §7.2 Phase Purity (compiler.md §Phase-Specific Purity)

- Phases do not bleed. Parser ≠ type-check; lexer ≠ parse; type-check ≠ codegen; canonicalize ≠ type-check.
- Core crates are pure — IO lives only in `oric`.
- Error recovery is monotone — recovery in an earlier phase does not create work for later phases (`parse.md` §ER-4).

### §7.3 Correctness Above All (CLAUDE.md §The One Rule)

- Every decision optimizes for correctness. Effort, time, cost, scope, risk, responsibility, ownership, and relatedness are irrelevant.
- Proper fixes only — no workarounds, hacks, shortcuts, or temporary fixes.
- If the correct fix crosses crate boundaries, the cross-crate fix is the work.

### §7.4 Cross-Phase Invariant Contracts (impl-hygiene.md)

- Every invariant crossing a phase boundary is validatable by a `debug_assert!` at the consumer's entry point or a dedicated validation pass (`impl-hygiene.md` §Cross-Phase Invariant Contracts).
- Release builds should produce a clear internal compiler error on contract violation. Currently: type-checker contracts are always-on; canonicalization validation is debug-only; ARC/LLVM verification is opt-in (`ORI_VERIFY_ARC=1`).
- Implicit invariants become invisible regressions — every load-bearing property is either a `debug_assert!` or a test (CLAUDE.md §Stabilization Discipline).

---

## §8 Cross-References

- **From canon.md to a phase**: §1 Pipeline Overview → authoritative file for that phase; §4 Per-Phase Output Invariants → owning rule anchor (parser / type-checker / type-system output contracts live under `PC-*` in `typeck.md` and `types.md`; parser-side rules live under `parse.md` §§LB-*, AR-*, DD-*, DI-*; AIMS uses §§1–9 with `CN-*` / `DP-*` / `IC-*` / `PL-*` / `RL-*` / `VF-*` rule anchors; codegen uses `TR-*` / `TM-*` / `NR-*` / `AB-*` / `VR-*`; representation layer uses `RP-*` / `RN-*` / `NI-*` / `RH-*` / `RV-*` in `repr.md`; canonicalization is anchored by `impl-hygiene.md` §Cross-Phase Invariant Contracts rows).
- **From a phase to canon.md**: when a rule touches more than one phase, cite canon.md §2 (surface desugars), §4 (output invariants), or §7 (non-negotiable invariants) for the pipeline-wide view.
- **Spec cross-reference**: every surface-language rule cites `docs/ori_lang/v2026/spec/` clauses. canon.md does not restate clauses; it points to them.
- **Design docs**: `docs/compiler/design/` carries longer-form design rationale (pattern compilation, AIMS lowering, runtime SSO, etc.). Rules files carry enforceable invariants; design docs carry the "why".

Navigation rule: if a fact is stated twice in this directory, one location is the SSOT and the other is a pointer. canon.md is the pointer layer, not an SSOT for any individual phase.
