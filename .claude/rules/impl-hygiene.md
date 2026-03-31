---
paths:
  - "*.rs"
---

# Hygiene Rules

These rules cover **code quality**: structure, naming, types, errors, performance. Process rules (TDD, git workflow, CI) live in CLAUDE.md. No duplication between them.

## Finding Categories

- **LEAK** — Logic, data, or control living outside its canonical home. The most dangerous category — side logic is how clean architectures decay. Subcategories:
  - **Phase bleeding**: a phase doing work that belongs to another phase
  - **Backward reference**: later phase calling back into earlier phase
  - **Swallowed error**: error silently dropped instead of propagated
  - **Duplicated dispatch**: routing/matching logic duplicated outside the canonical dispatch point
  - **Scattered knowledge**: type/method/operator behavior encoded ad hoc instead of read from the registry or canonical source
  - **Validation bypass**: validation rules implemented at consumption sites instead of at the canonical validation point
  - **Inline policy**: business logic (defaults, thresholds, formatting rules) hardcoded at call sites instead of centralized
  - **Algorithmic duplication**: two or more sites performing the same multi-step operation (even on different types) where the control-flow skeleton is identical — the algorithm has no canonical home
  Default: **Critical**. Every LEAK creates a second source of truth that WILL drift. Fix immediately — never defer.
- **DRIFT** — Registration data present in one location but missing from a parallel sync point. Default: **Major**.
- **GAP** — Feature supported in one phase but blocked/missing in another. Default: **Major**.
- **WASTE** — Unnecessary allocation, clone, or transformation at boundary. Default: **Minor**.
- **EXPOSURE** — Internal state leaking through boundary types. Default: **Minor**.
- **BLOAT** — File exceeds limits, mixes responsibilities, lacks submodule structure. Default: **Minor**.
- **NOTE** — Observation, acceptable tradeoff, documented exception. Default: **Informational**.

5+ findings clustered in one module = design problem; escalate to architectural review, not individual fixes.
**LEAK escalation**: 3+ LEAKs in one module = systemic side logic; the module lacks a canonical dispatch/query point. Don't patch individual LEAKs — introduce the missing canonical home first.

## Paradigms

Two paradigms govern all hygiene rules. Every rule in this document is a specific application of one or both.

### Single Source of Truth (SSOT)

Every piece of knowledge in the compiler has exactly **one canonical home**. All other locations that need that knowledge **query** or **derive from** the canonical source — they never maintain independent copies. This is the foundation of global coherence.

**Ori's architectural centers:**

| Knowledge Domain | Canonical Home | Consumers Query Via |
|---|---|---|
| Builtin type behavior (methods, operators, memory) | `ori_registry` | `find_type()`, `find_method()`, `OpDefs` |
| Type structure (interned types, relationships) | Type pool (`ori_types`) | Salsa queries, `TypeId` lookups |
| Memory analysis facts (ownership, borrowing) | AIMS (`ori_arc`) | Borrow/ownership annotations on IR |
| Representation decisions (layout, ABI) | repr-opt (`ori_llvm`) | Codegen queries |
| Language semantics | Spec (`docs/ori_lang/v2026/spec/`) | Developer reference |
| Syntax | Grammar (`spec/grammar.ebnf`) | Parser implementation |
| Diagnostic identity | Error codes (`ori_diagnostic`) | Code-based matching |
| Incremental computation | Salsa DB | Memoized query results |

**Three failure modes:**

1. **No home** — knowledge scattered with no canonical source. Fix: create the canonical home, migrate consumers to query it.
2. **Multiple homes** — two+ locations both claiming authority, no clear winner. Fix: designate one as canonical, derive the rest via queries or generation.
3. **Shadow home** — canonical source exists but consumers bypass it with local copies. This is a **LEAK** — fix by removing the local copy and wiring the consumer to the canonical source.

**Enforcement mechanisms (pick the strongest one that applies):**

1. **Type-level** (strongest): knowledge can only be constructed in one place. Consumers receive opaque handles (`TypeId`, `Name`, `ExprId`).
2. **Compile-time**: exhaustive match on source-of-truth enum forces consumers to handle all cases.
3. **Test-time**: exhaustiveness tests iterate the canonical list and verify all consumers are in sync (registry enforcement pattern).
4. **Query pattern**: consumers call a function on the canonical owner rather than maintaining their own lookup table.

**The test**: if you can answer "where is X defined?" with exactly one file path, SSOT holds. If you hesitate or name two places, it doesn't.

### No Side Logic

The complement of SSOT. Defined in detail in the "Side Logic" section below. Side logic is any logic living outside its canonical home — the mechanism by which SSOT degrades. Every LEAK finding is an SSOT violation in action.

## Phase Boundaries

- **One-way data flow**: later phases never call back into earlier phases
- **No circular imports**: `ori_lexer` never imports `ori_parse`
- **Minimal boundary types**: pass only what next phase needs
- **Clean ownership transfer**: move at boundaries, borrow within phases; no unnecessary `.clone()`
- **No phase bleeding**: lexer doesn't parse, parser doesn't type-check, type checker doesn't codegen
- **Phase purity**: output depends only on input; no global mutable state, no side channels
- **Delayed action queues**: when forward references needed in imperative passes (ARC, optimization), enqueue work — don't loop or backtrack. Salsa's demand-driven model handles forward refs via memoized queries.

### Phase-Specific Purity

- **Lexer**: scanning with minimal local state (nesting depth, mode stack); produces `(tag, len)`. No semantic state (names, types, scopes).
- **Parser**: syntax only; builds AST from tokens; no name resolution or semantic validation
- **Type Checker**: consumes AST, produces typed IR; no re-parsing, no codegen. Salsa queries must be pure.
- **Evaluator**: interprets typed IR; no re-type-checking, no codegen
- **ARC Pass**: analyzes ownership on IR; no codegen, no interpretation
- **LLVM Codegen**: emits LLVM IR from typed IR; no interpretation, no re-type-checking
- **Diagnostics**: formats and renders errors; no phase logic, no semantic analysis
- **Optimization Passes**: reads IR, produces transformed IR; analysis is pass-local

### Error Recovery Per Phase

- **Parser**: inserts error nodes, synchronizes to next statement
- **Type Checker**: uses error type (TyError) that unifies with anything; continues checking
- **Evaluator**: accumulates errors, skips dependent evaluations
- **Codegen**: aborts if any type errors remain — requires error-free input

### Error Recovery Monotonicity

Recovery in one phase must not *create work* for a later phase. Error types and error nodes should propagate silently without generating cascading diagnostics.

- **TyError is a poison type**: once introduced, it unifies with everything and produces no further type errors. Any code path that generates a new error involving TyError is a monotonicity violation.
- **Error nodes are terminal**: an error node in the AST should be skipped by subsequent phases (eval, codegen), not re-diagnosed. If codegen encounters an error node that wasn't filtered, that's a phase contract violation.
- **Recovery must be conservative**: a recovered parse tree may contain structurally invalid subtrees. Later phases must handle these gracefully (skip, not crash). If a recovered tree causes a later phase to panic, the recovery is incorrect.

### Lowering Completeness

Every language construct must be lowered in **both** backends (evaluator and LLVM codegen). A construct that works in eval but crashes in codegen (or vice versa) is a **GAP** that's invisible until a user hits it.

- **Dual-execution parity**: for every new IR node, expression kind, or language feature — verify that both eval and LLVM produce identical observable results. `dual-exec-verify.sh` automates this.
- **New variant checklist**: when adding a new `ExprKind`, `CanExpr`, or `StmtKind` variant, update ALL of: canonicalization lowering, evaluator dispatch, ARC lowering, LLVM codegen emission. A variant handled in only some phases is a **GAP**.
- **Strategy dispatch ≠ exhaustive match**: if a backend uses strategy-driven dispatch (e.g., `DeriveStrategy`) rather than direct pattern matching on IR variants, ensure the strategy table covers all variants. Strategy dispatch can silently ignore new variants that direct matching would catch.
- **Catch-all arms hide gaps**: `_ => unreachable!()` or `_ => todo!()` in IR dispatch matches are deferred GAPs. Each should either handle the variant or be tracked as a known gap with a plan item.

### Span Provenance Through Lowering

Spans must survive every IR transformation. Each lowering step (AST → CanExpr → ARC IR → LLVM IR) must propagate spans to their destination nodes.

- **No span-free IR nodes**: every node in every IR must carry a span back to source. A node with `Span::DUMMY` outside of compiler-generated code (e.g., protocol functions, builtin desugaring) is a provenance violation.
- **Lowering preserves spans**: when transforming `ExprKind::If` into `CanExpr::If`, the span of the source `if` expression propagates to the canonical form. The lowering step doesn't invent new spans.
- **Error attribution**: if a runtime error or codegen error points to a nonsensical source location, the span was likely dropped during a lowering step. Trace backward through the IR chain to find where.
- **ARC-inserted operations**: RC increments, decrements, and drops inserted by the ARC pass don't have "natural" source spans. They should carry the span of the expression that caused the insertion (e.g., the function call that requires an RC increment on an argument).

## Side Logic — Root of Architectural Decay

Side logic is any logic that lives outside its canonical home. It is the primary mechanism by which clean architectures degrade into historical drift. Each instance creates a second source of truth that can diverge from the canonical one. In a compiler with strong architectural centers (registry for builtin behavior, pool for type structure, AIMS for memory facts, repr-opt for representation decisions), leaked logic directly undermines global coherence.

**The cascade**: one side-logic shortcut invites another. Within months, the canonical source becomes "one of several places" that defines behavior, and eventually no single location is authoritative. This is irreversible without major refactoring.

### Detection Heuristics

1. **The "where would I look?" test**: If someone asks "where is X's behavior defined?" and the answer isn't a single location, there's a LEAK.
2. **The "what if it changes?" test**: If changing behavior X requires edits in N locations and N > 1 (excluding tests and docs), there's a LEAK. The registry enforcement pattern (one source + validation tests) is the correct alternative.
3. **The "copy-paste smell" test**: If a match arm, if-chain, or lookup table mirrors structure from another file, one of them is side logic. If the duplication is *algorithmic* (same control-flow skeleton, different types/operations), see also §Algorithmic DRY.
4. **The "special case" test**: If a function has `if type == SomeSpecificType { ... }` outside the canonical dispatch point for that type, it's side logic.

### Common Side Logic Patterns (All Are LEAK)

- **Ad hoc type knowledge**: Checking `is_string()` or `is_list()` to apply special behavior outside the registry/dispatch system. The registry defines behavior — consumers query it.
- **Duplicated dispatch tables**: A match on `TypeTag` or `MethodKind` that parallels an existing canonical match elsewhere. Add a method to the canonical dispatcher instead.
- **Inline defaults**: Hardcoding a default value, threshold, or policy at a call site instead of defining it in the type/config that owns it.
- **Re-derived facts**: Computing something that a prior phase already computed and stored. Query the stored result.
- **Format logic outside formatters**: Building display strings for types/values outside `Display`/`Debug`/diagnostic formatters.
- **Validation at consumption**: Checking invariants at every use site instead of enforcing them at construction (parse-don't-validate pattern).

### Remediation

The fix for side logic is always the same: **move the logic to its canonical home and have the consumption site query/call it**. Never "fix" a LEAK by adding a comment explaining why the duplication exists. If the canonical home doesn't exist yet, create it — that's the real fix.

## Data Flow

- **Zero-copy**: spans reference source by position; tokens carry `(tag, len)`, not string copies. Owned copies acceptable for diagnostic messages and error context that outlive the phase.
- **Phase-scoped allocation**: Salsa interning for cross-phase data; temporaries freed when phase completes; no leaking scratch data to next phase
- **Interned values via opaque indices**: cross boundaries with `Name`, `ExprId`, `TypeId` — never raw `u32`
- **No heap allocation in hot paths**: no `format!()`, `String::from()`, `Box::new()` per token. `Vec::new()` is fine (zero-alloc); avoid `Vec::with_capacity()`, `String::from()` per iteration.
- **Source text owned by Salsa**: all phase access via borrow from Salsa queries. No phase copies the full source text into its own data structures.
- **Hash lookups over linear scans**: for collections > ~8 items; small fixed-size lists may use linear scan
- **Pre-allocate**: `Vec::with_capacity()` with reasonable estimates

### Hot Paths

Hot: lexer scan loop, parser expression/statement parsing, type inference unification, IR traversal/transformation, codegen emission. Cold: error/diagnostic formatting, CLI, startup, test setup. When unsure, profile.

### String Hygiene

- Identifiers always interned via `Name` — never compare identifier strings directly
- Source text always `&str` borrowed from Salsa
- Error messages may own `String`
- No `format!()` in hot paths; use `write!()` to a buffer for complex string building

## Error Handling

- **Accumulate, don't bail**: each phase collects all errors in one pass; cap at configurable limit (default ~100) and emit "too many errors, stopping"
- **Phase-scoped error types**: lexer errors != parse errors != type errors. Each type is an enum with one variant per distinct error condition. Every variant carries: span, error code, context for the diagnostic formatter.
- **Upstream errors propagated**: use `?` for propagation, `impl From<UpstreamError>` for cross-phase conversion, `.map_err()` for context at boundaries. Never `Ok(default)` to swallow errors.
- **Errors carry spans**: every error includes source position; spanless errors are bugs
- **Recovery is explicit**: enum variants, not implicit booleans
- **Structured construction**: `Diagnostic::error(code).with_message(...).with_label(...)` — never `format!()` strings
- **Expected context**: every "expected X, got Y" MUST include WHY — annotation, return type, parameter
- **Deduplication**: deduplicate by (error code, primary span). Suppress follow-on: if error at span S produces TyError, suppress subsequent type errors involving TyError at child spans.
- **Edit-distance suggestions**: Damerau-Levenshtein for "did you mean?" — threshold: `distance <= min(name.len() - 1, max(2, name.len() / 3))`
- **Error codes are stable API**: once assigned, never reuse or change meaning. Ranges: E0xxx = parse, E1xxx = type check, E2xxx = semantic. Tests assert on error codes, not exact message text.
- **Anti-patterns**: `match err { Err(_) => Ok(default) }`, `if let Ok(x) = fallible` (silently drops error), `.unwrap_or_default()` on Result in production code

### Diagnostic Message Quality

- Plain language, not compiler internals; lead with what's wrong
- Suggest how to fix it; show the fix, not just the problem
- No "you" blame language; show code context when possible

## Type Discipline

- **Separate raw vs cooked types**: each phase boundary has distinct input/output types where the transformation is non-trivial
- **Newtypes for all IDs**: `ExprId`, `TypeId`, `TokenIndex` — not raw `u32`. Inner field private, construct via `new()`/`From`, `.0` access only inside defining module.
- **Metadata in sidecars**: metadata (spans, comments, debug info, source locations) travels in sidecars or indexed maps, not inline in core data structures. Core types stay lean. Exception: spans are small enough (u32 pair) to inline.
- **No phase state in output types**: AST nodes carry structure + spans, not parser cursor or inference state. Inference variables (unification, skolem) must be resolved before emitting typed IR.
- **Pre-compute derivable metadata**: at construction/interning time (e.g., TypeFlags, is_generic, size). O(1) queries, never re-walk composite structures. Salsa memoization handles invalidation for interned values; avoid manual caches.
- **Option vs Result**: `Option` for absent/not found (lookup miss). `Result` for failure with diagnostic info. Never `Result<T, ()>` — use `Option`. Never `Option` when None should carry an error.
- **Type aliases**: for long generic types (e.g., `Result<T, E>` with fixed E). Never for simple types. Alias names add semantic meaning. Don't shadow std types without purpose.

### Dispatch Choice

- Static dispatch (generics) by default
- `dyn Trait` only for user-extensible plugin points or heterogeneous collections
- Cost: `&dyn` < `Box<dyn>` < `Arc<dyn>`. Never `Arc<dyn>` in hot paths.

## Pass Composition

- **Each pass is (IR, Config) → IR**: configuration (opt level, target) is an explicit parameter. No env vars, no static mut, no thread-locals.
- **Explicit pass ordering**: dependencies documented in `//!` module doc of the pass manager and comments at each invocation site. Assert ordering with tests.
- **No shared mutable state between passes**: inter-pass communication via IR only. Exception: diagnostic accumulation is append-only — passes may append but not read/mutate each other's diagnostics.
- **Boundary validation**: `debug_assert!` invariants before crossing to next phase. Examples: all type variables resolved before codegen, no unreachable blocks in IR, RC ops balanced after ARC pass.

### Cross-Phase Invariant Contracts

Each phase produces output that downstream phases *assume* satisfies certain invariants. These assumptions must be **explicit and validated**, not implicit and hoped-for. An implicit invariant is a silent corruption vector.

**Known contracts (each must have a `debug_assert!` or validation pass):**

| Producer | Consumer | Contract |
|----------|----------|----------|
| Type Checker → Codegen | All type variables resolved | No `Idx` with `Tag::Var` in typed IR |
| Type Checker → Eval | All types concrete | No unresolved inference variables |
| ARC Pass → Codegen | RC ops balanced per function | Every `rc_inc` has a matching `rc_dec` on all paths |
| ARC Pass → Codegen | Drop placement correct | Drops at end-of-scope, not use-site |
| ARC Pass → Codegen | COW patterns valid | Uniqueness checks before mutation |
| Canon → All | No sugar variants | CanExpr contains no desugared-away variants |
| Canon → All | All TypeIds resolved | No `TypeId::INFER` in canonical IR |
| Parser → TypeChecker | Error nodes marked | Error recovery nodes carry error marker |

**Rules:**
- Every contract must be validatable by a `debug_assert!` at the consumer's entry point or a dedicated validation pass
- If a contract is violated in release builds (where `debug_assert!` is stripped), the consumer must produce a clear internal error, not silently emit wrong code
- When adding a new invariant that a downstream phase relies on, add the `debug_assert!` in the *consumer*, not just the producer — the consumer is where the assumption becomes dangerous
- Cross-phase contracts are the **most fragile** invariants in the compiler. A change to the ARC pass that subtly breaks RC balance will not be caught until codegen emits wrong code, which may not be caught until runtime. Defense in depth: validate at producer exit AND consumer entry.

### Debug/Release Parity

Debug and release builds must produce **identical observable output** for the same input program. Verification-only `debug_assert!` and validation passes are exempt (they add checks, not change behavior).

- **No semantic divergence**: `#[cfg(debug_assertions)]` blocks must not change codegen logic, control flow, or observable output. They may only add verification, logging, or assertions.
- **FastISel awareness**: LLVM's FastISel (used in debug/JIT) can produce different instruction selection than the full optimization pipeline. Known divergences must be documented and tested (e.g., the "never load >16B struct in JIT" rule).
- **Test both modes**: `cargo test` (debug) and `cargo test --release` must both pass. A test that passes in debug but fails in release (or vice versa) indicates a parity violation.
- **Optimization-sensitive codegen**: if codegen emits different IR depending on optimization level (e.g., inlining thresholds, loop unrolling hints), document why and ensure the semantic output is identical.

## Invariant Explicitness

- **Implicit invariants are invisible regressions.** If correctness depends on a property (RC balanced after loop, scope restored after block, phantom var inserted before iteration, elem_dec_fn non-NULL for heap types), it MUST be either:
  - A `debug_assert!` at the point where the invariant is relied upon, OR
  - A test that would fail if the invariant is violated
- **Semantic changes require semantic pins.** When a fix changes observable behavior (RC emission pattern, element cleanup order, scope lifetime, dec function selection), add a regression test that ONLY passes with the new semantics. This test is the permanent guard against revert.
- **Cross-section fixes require cross-section plan updates.** If implementing Section X requires changing code owned by Section Y, you MUST update Section Y's plan to reflect the change. A partial fix absorbed silently across section boundaries creates invisible dependencies that compound into cascading failures.

## Narrow the Front

- **Complete one fix fully before starting another.** RC + control-flow + lowering interactions multiply failure surfaces. Concurrent changes across these domains compound risk.
- **"Fully" means**: fix + matrix tests + semantic pin + plan update. A fix without matrix tests is incomplete. A fix with tests but without plan update (when cross-section) is incomplete.
- **Prefer depth over breadth.** Fix one element type across all patterns before fixing a second element type. Fix one loop variant completely before the other. This reduces the number of concurrent moving parts and makes failures narrow and explainable.

## Registration Sync Points

Application of the SSOT paradigm to enum variants, lookup tables, and parallel data structures.

- **Canonical source drives all consumers**: one location is the source of truth — others derive from it or are validated against it. Never maintain independent parallel lists.
- **No manual mirroring**: centralize via `from_str()`, `all()`, iterator — not parallel lists. If you must have parallel structure, generate or validate it from the canonical source.
- **Compile-time or test-time enforcement**: add test iterating source-of-truth list. Prefer compile-time (exhaustive match) over test-time where possible.
- **Flag drift as finding**: new variant in one location but missing from parallel = **DRIFT**
- **Flag duplication as finding**: parallel lookup table that could query the canonical source instead = **LEAK:scattered-knowledge**
- **New type checklist**: new pub types need: Debug derive, Display if user-facing, From conversions for cross-phase types, documentation, tests. New types trigger sync requirements — not just new enum variants.

## Gap Detection

- **Cross-phase capability mismatch = GAP**: one phase supports a feature, another blocks it
- **Never silently work around a gap**: flag immediately
- **Audit across phases**: when adding capability, verify full pipeline: lexer → parser → typeck → eval → codegen

## Compiler-Specific Hygiene

Compiler codebases have failure modes that generic software engineering rules don't catch. These rules address patterns unique to multi-phase compilation pipelines.

### IR Variant Exhaustiveness

When a new expression kind, statement kind, or type tag is added to any IR, **every consuming phase must handle it**. Rust's exhaustive match catches this within a single crate, but cross-crate consumers using strategy dispatch, visitor patterns, or catch-all arms can silently ignore new variants.

- **Exhaustive match preferred**: direct pattern matching on IR enums (no `_` catch-all) is the strongest enforcement. The compiler itself becomes the exhaustiveness checker.
- **Strategy dispatch must be validated**: if a phase uses strategy-driven dispatch (e.g., `DeriveStrategy`) rather than direct matching, add a test that iterates `ALL` variants and asserts each has a corresponding strategy entry. Strategy tables are manually maintained — they don't get compiler-enforced exhaustiveness for free.
- **Cross-crate exhaustiveness test**: for every IR enum that is consumed by 2+ crates, add a test in each consumer that iterates the `ALL` constant (or a generated list) and asserts coverage. This catches the case where a new variant is added to `ori_ir` but not handled in `ori_eval` or `ori_llvm`.
- **`_ => unreachable!()` is a deferred GAP**: every such arm is a variant the consumer has not thought about. Track as a gap. `_ => todo!()` is worse — it compiles but panics at runtime.

### Layout Computation

Type layout (size, alignment, field offsets, discriminant encoding) must be computed **once** and cached, not recomputed by each consumer.

- **Single computation point**: layout is computed after type checking (when all struct fields and enum variants are finalized) and before codegen.
- **Cache via Salsa or interning**: layout results keyed by `TypeId`. Same type = same layout, always. No non-determinism.
- **Consumers query, never compute**: codegen queries layout facts from the cache, never re-derives them from field types. If two codegen functions both need the size of a struct, they query the same cached result.
- **Repr pragmas are inputs**: `#repr("c")`, `#repr("packed")`, `#repr("aligned", N)` feed into layout computation as configuration, not as ad-hoc overrides scattered through codegen.

### Interning Discipline

All identifiers, types, and expressions that are compared for equality or used as keys must be interned. String comparison for semantic identity is a bug.

- **Identifiers**: always `Name` (interned at lex time). Never compare identifier `String`s directly. Name equality is pointer/index equality (O(1)).
- **Types**: always `Idx` (interned in Pool). Never compare type structures directly. Type equality is index equality.
- **Expressions**: always `ExprId` / `CanId` (arena-allocated). Never compare AST subtrees by structure.
- **Violation detection**: grep for `== "identifier_name"` in non-test code. Any string comparison that should be a `Name` comparison is a LEAK:scattered-knowledge — the interning layer is being bypassed.
- **Pre-interned constants**: frequently-used names (keywords, builtins, common method names) should be pre-interned at startup for O(1) lookup. If the same string is interned per-call-site, that's a WASTE.

### Aspirational Patterns (from Reference Compilers)

Patterns used by established compilers that Ori should grow toward. Not current violations — these are architectural north stars for future hygiene reviews to measure against.

#### Type Folding (Rust: `TypeFolder<TyCtxt>`)

Rust separates *traversal* (visiting types read-only) from *transformation* (folding types into new types). Ori has `Visitor<'ast>` for traversal but no equivalent `TypeFolder` for recursive type transformation.

**Current state**: Type substitution (`pool/substitute/`, `unify/substitute.rs`) uses ad-hoc recursive matching. Each consumer of type transformation rolls its own walk.

**North star**: A `TypeFolder` trait where consumers implement only the interesting cases and `super_fold_with()` handles recursion. This would eliminate the algorithmic duplication in substitution (§Algorithmic DRY) and provide a single canonical recursion skeleton for all type-to-type transformations.

**Adoption path**: Extract the shared recursion skeleton from the 2-3 existing substitution implementations into a trait. Consumers implement `fold_var()`, `fold_named()`, etc. Default methods recurse via `super_fold_with()`.

#### Packed Symbol Representation (Roc: `Symbol = (ModuleId, IdentId)`)

Roc packs module identity and identifier identity into a single `u64`, enabling O(1) equality and perfect hashing without indirection. Ori's `Name` is interned (O(1) equality) but doesn't encode module provenance — cross-module name resolution requires secondary lookups.

**North star**: A `Symbol` type that encodes both the defining module and the identifier in a single word. This eliminates the need for separate "which module does this name come from?" lookups during type checking and codegen.

**Adoption path**: Design a `Symbol = (ModuleId, Name)` pair with niche optimization for `Option<Symbol>`. Migrate cross-module name resolution to use `Symbol` instead of `Name` + context.

#### Deduplication by (Code, Span) with Follow-On Suppression (Rust)

Rust deduplicates diagnostics by `(error_code, primary_span)` and suppresses follow-on errors that involve `TyError` at child spans. This prevents the "100 errors from one typo" problem.

**Current state**: Ori has `DiagnosticQueue` with dedup + follow-on filtering, but the suppression logic may not be as aggressive as Rust's child-span-based suppression.

**North star**: Every error involving a type that transitively contains `TyError` is suppressed if a prior error at an ancestor span already reported the root cause. Users see the *root* error, not the cascade.

**Adoption path**: Audit `DiagnosticQueue` dedup logic. Add child-span suppression: if error at span `S` produces `TyError`, suppress errors at spans contained within `S` that mention `TyError`.

#### Explicit Phase Job Queue (Zig: `Compilation.zig`)

Zig defines compilation as a queue of jobs, each tagged with a stage that determines execution order. This makes phase ordering explicit and auditable — you can inspect the queue to see what runs when.

**Current state**: Ori uses Salsa's demand-driven model, which handles ordering implicitly via query dependencies. This is correct but makes phase ordering invisible — you can't easily answer "what order do these phases run in?" without tracing query calls.

**North star**: A documented phase graph (even if Salsa handles execution) that explicitly lists: phase name → input type → output type → invariants guaranteed. This is documentation, not code — but it makes the implicit explicit and gives hygiene reviews a reference to audit against.

**Adoption path**: Add a `//!` module doc in the Salsa query module that lists all tracked functions in execution order with their contracts. This is a documentation task, not an architecture change.

#### Layout Caching via Query (Rust: `TyCtxt::layout_of`)

Rust computes type layout via a memoized query on `TyCtxt`. The layout is computed once (lazily, on first query), cached, and never recomputed. All layout consumers go through `layout_of()`.

**Current state**: Ori's layout computation lives in `ori_llvm` (codegen-time). No cross-phase caching.

**North star**: A Salsa-tracked `layout_of(TypeId) -> Layout` query that any phase can call. Codegen queries it for emission. ARC queries it for alignment-aware optimization. Future optimization passes query it for size-based decisions.

**Adoption path**: Extract layout computation from `ori_llvm` into a shared query (possibly in `ori_types` or a new `ori_layout` crate). Wire it through Salsa for memoization.

## Cascading Fix Detection

- **Whack-a-mole = architectural issue**: fix at one callsite moves failure to next → STOP
- **Three-strike rule**: same fix at 3+ callsites = missing abstraction; fix at boundary
- **More heuristics**: >3-4 params → config struct. Same enum matched in 3+ files → centralize dispatch. Same error string in 3+ places → error factory function.
- **Present options**: (1) architectural issue, (2) why per-site patches won't scale, (3) 2-3 options

## Algorithmic DRY — No Duplicated Algorithms

SSOT ensures every piece of **knowledge** has one canonical home. This section ensures every **algorithm** — a multi-step operation with a recognizable control-flow skeleton — also has one home. Duplicated algorithms are `LEAK:algorithmic-duplication` findings.

An algorithm is duplicated when two or more sites share the same control-flow skeleton (loop structure, branch conditions, error handling shape) and differ only in:
- **Types** — same traversal over different type parameters
- **Operations** — same loop harness with different per-element callbacks
- **Field names** — same structural access pattern on different structs
- **Phase context** — same validation/dispatch logic in eval and codegen

Knowledge duplication drifts when facts change. Algorithmic duplication drifts when the *protocol* changes — a new step is added to one copy but not the others. Both are equally dangerous.

### Detection Heuristics

1. **The "diff the bodies" test**: If two function bodies differ only in type names, field names, or closure bodies but share the same control-flow skeleton (loops, branches, error paths), the skeleton is an extractable algorithm.
2. **The "count the steps" test**: If 3+ call sites perform the same sequence of 2+ operations (even with different arguments), extract a higher-order function.
3. **The "inline lambda" test**: If you could copy-paste a block and only change the closures/callbacks, the surrounding scaffold is the algorithm to extract.
4. **The "cross-backend mirror" test**: If eval and codegen (or any two phases) maintain parallel dispatch tables, match arms, or routing logic with the same structure, the shared structure needs a single source. This is the most dangerous form — cross-crate duplication drifts silently because no single-crate test catches the desync.
5. **The "match arm count" test**: If the same enum/tag is matched in N files with similar arm structure, N-1 of those matches are candidates for consolidation into a canonical dispatcher.

### Thresholds

- **2 instances, >5 lines of shared skeleton**: extract immediately. Two non-trivial copies is already one too many.
- **3+ instances, any size**: always extract. No exceptions. This is the "missing abstraction" threshold.
- **Cross-crate duplication**: even 2 instances = extract to a shared crate or shared metadata source. Cross-crate copy-paste is the most dangerous because drift is invisible — different test suites, different maintainers, different change cadences.
- **Cross-backend (eval ↔ codegen)**: parallel dispatch tables of any size = extract method metadata to a shared registry. Both backends must query the single source for method names, arg counts, valid receiver types, and routing keys.

### Remediation Hierarchy

When algorithmic duplication is found, select the **first** approach that fits:

1. **Generic function** (`<T>` / trait bounds) — steps identical, only types differ. Example: `substitute_single_child()` generalized over all single-child container tags.
2. **Higher-order function** (closure parameters) — skeleton identical, per-element operations differ. Example: iterator consumer loop with a `folder: FnMut(Acc, Item) -> Acc` parameter replacing 11 separate consumer functions.
3. **Trait + blanket impl** — pattern crosses type families with shared interface. Example: `ResolutionContext` trait unifying 4 type-resolution functions that differ only in parameter/self-type handling.
4. **Data-driven dispatch** (registry table) — routing structure identical, entries differ. Example: method dispatch tables replaced by a `HashMap<(TypeTag, Name), Handler>` populated from a single registry.
5. **Macro** — last resort, when duplication is syntactic (identical token structure) rather than semantic. Example: `define_require_arg!` for 11 type-specific argument extractors. Prefer any of the above when the shared structure is semantic.

### What This Is NOT

- **Not "never repeat a line"** — three similar `map.insert()` calls aren't duplication. The bar is *structural*: same multi-step algorithm with the same control-flow shape.
- **Not speculative generalization** — extract only when you have 2+ concrete instances. Never for a hypothetical future need. "Might need this later" is not a reason to abstract.
- **Not "every helper is good"** — a helper called once that just relocates code is noise, not DRY. The test: does the extraction eliminate a *second copy*, or does it just move a single copy?
- **Not a license to over-abstract** — `fold_iterator(init, folder)` is good. `AbstractIteratorConsumerStrategyFactory` is not. The extraction should be simpler than the duplication it replaces. If the abstraction is harder to understand than the copies, the copies are better.

### Interaction with SSOT

Algorithmic DRY is the complement of SSOT:
- **SSOT** asks: "where is this *fact* defined?" — answer must be one place
- **Algorithmic DRY** asks: "where is this *operation* defined?" — answer must be one place

When both apply (e.g., a dispatch table that encodes both facts and routing), fix the SSOT violation first (centralize the data), then the algorithmic violation (consolidate the routing logic that queries it). The data-driven dispatch pattern often fixes both at once.

## File Organization

- **500-line limit**: source files (excluding tests); exceeding = **BLOAT** finding
- **Proactive split**: split at ~450 lines if you know more code is coming. Don't wait until over the limit.
- **Single responsibility per file**: one logical operation or one type family. Anti-pattern: `utils.rs`, `helpers.rs`, `misc.rs`. Every file name describes its domain.
- **Submodule extraction**: logical group exceeding ~200 lines → sibling submodule; parent `mod.rs` = dispatch hub
- **Directory structure**: mirrors the logical phase/pass structure
- **Split when touching**: touching a file over 500 lines without splitting = finding
- **Tests in sibling `tests.rs`**: `#[cfg(test)] mod tests;` declaration only — body in sibling file
- **Section markers**: plain `// Section name` on its own line, preceded by blank line. No decorative characters. If sections exceed ~200 lines, extract to submodule instead.
- **Banner removal**: if you touch a file with decorative banners (`// ===`, `// ---`), remove them.

### File Layout (top to bottom)

1. `//!` module docs
2. `mod` declarations
3. Imports (3 groups, blank-line separated: external → crate → relative, alphabetical within)
4. Type aliases
5. Type definitions (structs, enums)
6. Inherent `impl` blocks (immediately after their type)
7. Trait `impl` blocks (immediately after inherent impls)
8. Free functions
9. `#[cfg(test)] mod tests;` at bottom

### Module Roles

- `lib.rs` is an **index**: `//!` doc, `mod` declarations, `pub use` re-exports — no function bodies. Strict, no exceptions.
- `mod.rs` **dispatches**: routes to submodules, holds shared private items
- Leaf files **implement**: actual logic lives here

### Crate Organization

- Each crate has a single documented purpose
- Module nesting max 4 levels (e.g., `ori_types::check::registration::traits`). Deeper = missing abstraction.
- If a crate has >50 source files, consider splitting
- Shared utilities live in dedicated crates (`ori_diagnostic`, `ori_patterns`, `ori_ir`). No `utils` modules in phase crates. If 3+ crates need the same utility, extract to a shared crate.

### Import Hygiene

- 3 groups separated by blank lines: external → crate → relative, alphabetical within
- No glob imports (`use foo::*`) except in test modules and preludes
- No unused imports
- Re-export only types that are part of your crate's public API contract. Consumers import from the crate that owns the type.

## Impl Block Method Ordering

1. **Constructors**: `new`, `with_*`, `from_*`, factory methods
2. **Accessors**: getters, `as_*` (cheap ref conversions)
3. **Predicates**: `is_*`, `has_*`, `can_*`, `contains`
4. **Public operations**: the main thing this type does
5. **Conversion/consumption**: `into_*`, `to_*`
6. **Private helpers**: in call-order grouping, not alphabetical

Within each group: pub before pub(crate) before private (loose).

## Struct/Enum Ordering

**Struct fields:**
1. Primary data (core state)
2. Secondary/derived data
3. Configuration/options
4. Flags/booleans last

Inline comments on struct fields when purpose isn't obvious.

**Enum variants:** ordered by frequency/importance (common first) or logically grouped (keywords together, operators together). Match arms follow the enum's declaration order.

## Naming

**Functions** — verb-based prefixes:
- Predicates: `is_*`, `has_*`, `can_*`
- Conversions: `into_*` (consuming), `to_*` (borrowing), `as_*` (cheap ref), `from_*` (construct)
- Processing: `cook_*` (lexer), `parse_*` (parser), `check_*` (typeck), `eval_*` (evaluator), `emit_*` (codegen)
- Consumption: `eat_*` (advance past), `skip_*` (advance+discard)
- Resolution: `resolve_*`, `lookup_*`, `fresh_*`
- Factory: `new`, `with_*`

**Variables** — scope-scaled:
- 1 char in <= 3 lines: `c`, `i`, `n`, `b`
- 2-4 chars in <= 15 lines: `ch`, `tok`, `pos`, `len`, `src`, `buf`, `err`, `kw`
- Descriptive in larger scopes: `token_span`, `base_offset`, `content_str`

**Constants**: `SCREAMING_SNAKE_CASE`, descriptive names.
**Type aliases**: `PascalCase`, suffix with purpose.
**Modules**: `snake_case`, noun-based.
**Crates**: `ori_` prefix.
**Generic parameters**: `T`/`E`/`K`/`V` for standard patterns; descriptive names when 2+ type params or domain-specific meaning. Never bare `T` with 3+ type params.

## Visibility

- Private by default; minimize pub surface
- `pub(crate)` for cross-module internal use
- `pub(super)` for parent-module access; prefer narrowest visibility that works
- No dead pub items; no dead code
- Items pub only for testing: `#[cfg(test)] pub` or `pub(crate)` with `// test-only` comment
- `#[non_exhaustive]` for public library APIs only. Internal compiler enums should be exhaustively matched — the compiler error on new variants catches missing match arms.

## Comments

- `//!` module doc on every file; `///` on all `pub` items
- All pub types and functions get `///` docs; use `` [`TypeName`] `` for cross-references; no docs that just restate the function name
- Comment WHY, not WHAT; `debug_assert!` for preconditions
- **Anti-patterns**: `// increment counter` (restates code), `// TODO` without context, `// This is a hack` without explaining the proper fix
- No decorative banners (`// ===`, `// ---`, `// ***`)
- No comments restating code, no commented-out code ever (use version control), no bare `// TODO`
- TODOs: format `// TODO(phase): description` — e.g., `// TODO(typeck): handle generic associated types`. Every TODO references a plan or roadmap item. No orphan TODOs.
- Section labels in large enums/matches: plain `// Section name`
- **Spec citations required**: code implementing grammar rules, operator semantics, type rules, or language semantics must cite the spec clause. Format: `// Spec: Clause N.M — description`
- **Plan annotations are temporary scaffolding**: Annotations referencing plans (`TPR-04-005`, `CROSS-04-014`, `§04.3 Phase A`, `BUG-04-07`, `Section 04.2`, `section-04-name`) are allowed during active plan execution — they aid navigation. They MUST be removed when the plan completes. Every plan MUST include a final cleanup section to strip all its code annotations. Stale annotations from completed plans are hygiene violations (**DRIFT** category). Run `.claude/skills/impl-hygiene-review/plan-annotations.sh` to scan. Only **spec references** (`Spec: Clause N.M`) are permanent.

## Derives

- **Debug on all pub types** (required for tracing and error context)
- **Clone** only when semantic copying makes sense
- **PartialEq/Eq** only for types used in comparisons/hash maps
- **Hash** only when Eq is derived
- **Derive** when impl is standard (field-by-field equality, hash, debug)
- **Manual** only when behavior differs — articulate WHY

## Lint Discipline

- `#[expect(clippy::lint, reason = "...")]` — never bare `#[allow(clippy::...)]`
- `#![deny(unsafe_code)]` in pure crates: `ori_ir`, `ori_diagnostic`, `ori_types`, `ori_eval`, `ori_patterns`, `ori_parse`, `ori_lexer`
- `unsafe` allowed only in: `ori_llvm`, `ori_rt`, `oric` (FFI, LLVM bindings, runtime)
- **Project-wide denied lints**: `unwrap_used` (prod code), `panic`, `todo`, `dbg_macro`, `print_stdout`/`print_stderr`. Override with `#[expect(reason)]` when genuinely needed.
- **No commented-out code ever**: use version control. Not in any branch.

## Performance Annotations

- `#[cold]` on error factory functions
- `#[inline]`: 1-5 lines freely. 6-20 lines only if profiling shows benefit or cross-crate hot path. >20 lines never.
- `#[repr(C)]` only for FFI types; default layout for everything else
- **Size assertions** on types in per-token/per-expression arrays or passed by value in hot loops. Add when size exceeds 2 machine words (16 bytes on 64-bit): `const _: () = assert!(size_of::<T>() == N);`
- `#[must_use]` on all pub functions returning `Result`/`Option`, builder methods returning `Self`, and pure functions where ignoring the return is always a bug. On types like `Diagnostic`, `Error`.

## Style

- Functions < 100 lines (target < 50). Exempt: dispatch tables, exhaustive enum matches. Match arms > 3 lines should call a helper.
- **Nesting depth**: max 4 levels. Prefer early returns (guard clauses) to reduce nesting.
- **Pattern consistency**: similar operations across files must use the same structural pattern. Before writing a new file, read 2-3 siblings for established conventions.
- **Iterator style**: iterator chains for transformations (map/filter/collect), manual loops for complex control flow (break/continue/multiple mutations). If a chain needs 4+ combinators or a closure > 3 lines, switch to a loop.
- No dead/commented-out code

## Clone Discipline

- Clone acceptable on cold/error paths and test setup
- Prefer `&str`/`Cow` over `String` at boundaries
- `Arc` only for shared ownership across threads/tasks
- No `.clone()` in hot paths without a comment justifying it

## Unsafe & FFI

- Every `unsafe` block requires a `// SAFETY:` comment explaining the invariant
- Minimize unsafe scope — extract safe logic outside the unsafe block
- **FFI exports**: `ori_` prefix, `#[no_mangle]` + `extern "C"`, null checks on pointer params
- **C types**: use `std::ffi` (`c_char`, `c_int`, `c_void`, `CStr`, `CString`). Never `i8`/`i32` for C types. Never assume char is signed.
- **ABI contract**: codegen and runtime must agree on struct layouts. Changes to `ori_rt` function signatures must update `ori_llvm` call sites in the same commit.
- **Platform-specific code**: isolate behind `#[cfg(target_arch)]` blocks. Abstract platform differences.

## Salsa & Caching

- Queries must be pure: no side effects, no non-determinism
- No `Arc<Mutex<T>>` in query inputs or values
- Prefer fine-grained queries over coarse
- Salsa interned values automatically invalidated. For non-Salsa caches, document the invalidation strategy. Prefer Salsa queries over manual caches.

## Panic & Assertion

- **Never panic on user input**: all user-facing errors go through the diagnostic system
- **`.unwrap()`**: only with comment proving infallibility, or in tests. Production code: `.expect("reason")` or propagate with `?`.
- **`assert!()`**: for invariants whose violation would cause unsound codegen or safety issues. Always include a message.
- **`debug_assert!()`**: for expensive invariant checks (O(n) or worse)
- **`unreachable!()`**: for impossible code paths. Include context message. Never `panic!()` for impossible states.
- **Recursion depth**: all recursive traversals (type folding, expression visiting) must have a depth limit. Default: 256. Report clear error, don't stack overflow.

## Tracing & Logging

- Use `tracing` macros only (`trace!`, `debug!`, `info!`, `warn!`, `error!`). Never `println!`/`eprintln!` in library crates.
- `tracing::error!()` is for internal compiler failures only. User-facing errors go through the Diagnostic system.
- Error construction emits `trace!()` at debug level. Error recovery emits `warn!()`.
- `#[tracing::instrument]` on public APIs
- No sensitive data in logs, no log spam in hot loops

## Test Hygiene

- Test behavior, not implementation. Each test tests one thing.
- No test ordering dependencies; no shared mutable state between tests
- Descriptive names: `test_X_when_Y_then_Z`
- No `#[ignore]` without a tracking issue/reason
- **Test data**: fixtures in `tests/fixtures/` or `tests/spec/`. No hardcoded absolute paths — use relative paths or `CARGO_MANIFEST_DIR`. Test data committed to repo.

## Conditional Compilation

- `#[cfg(test)]` for test modules and test helpers only, not for production logic branching
- `#[cfg(debug_assertions)]` for debug-only checks
- Production code must not change behavior based on `#[cfg(test)]`

## Lifetime Annotations

- Prefer elision when possible
- Descriptive names for long-lived borrows: `'src`, `'ast`, `'ctx`
- Single-letter (`'a`) only for local/obvious cases
- Avoid >2 lifetime parameters per function

## API Stability

- Pub items in `lib.rs` are the stable API surface
- Breaking changes to pub crate APIs must update all downstream consumers in the same commit
- When replacing a code path, remove the old code in the same commit. No deprecation for internal compiler code.

## Dependencies

- Prefer `std` over external crates
- New external deps require justification
- Features are additive only (never remove functionality). Each feature documented in `Cargo.toml`.

## Concurrency

- Compiler internals are single-threaded (Salsa handles parallelism)
- No global mutable state (`static mut`, `lazy_static` with mutation). All state flows through function parameters or Salsa queries.
- Thread-safety required only at `ori_rt` FFI boundary

## CI & Build

- All code passes `./clippy-all.sh`, `./test-all.sh`, `./fmt-all.sh` before merge. No exceptions.
- No warnings in CI
- Build must succeed with default features and `--all-features`

## Commit Hygiene

- One logical change per commit; conventional commit format (`feat`/`fix`/`refactor`)
- Cross-crate changes that must be in sync go in a single commit
- Large refactors broken into phases: (1) add new API alongside old, (2) migrate consumers, (3) remove old API. Never break the build between phases.
- No WIP/temp commits on dev/master

## Technical Debt

- Fix when you find it. If it can't be fixed in the current change, add an entry to the active plan or create a roadmap item. No untracked debt.
- Experimental/prototype code lives in feature branches, never in dev/master.
