# Phase 2 — Map the Full Landscape

Read by a Sonnet sub-agent dispatched from `/impl-hygiene-review`. Not a registered skill. Uses `scripts/intel-query.sh` (blast radius, module inventory, cross-repo similarity) to build a landscape map of the review target before Phase 3 deep analysis.

Writes `/tmp/impl-hygiene-{run}/phase-2.json` with: call graph edges for in-scope symbols, file-symbol inventory per crate, cross-repo equivalents for architectural patterns.

---

### Phase 2: Map the Full Landscape

Before diving into findings, build a high-level map of the review scope. This is the "go wide" phase — understand the shape before probing the details.

#### 2a. Identify Review Targets

Determine the distinct crates or phase boundaries to review based on the target scope:

1. List the crates (directories) in scope
2. Identify which phase boundaries exist between them (e.g., lexer→parser, parser→types)
3. Map the dependency graph between in-scope crates
4. Group crates into **review units** — each review unit is either:
   - A single crate (for internal review)
   - A pair of crates sharing a boundary (for boundary review)
   - Closely related crates that should be reviewed together

#### 2b. Map Cross-Crate Data Flow (Full Project & Multi-Crate Mode)

For full project mode or when 3+ crates are in scope, spawn an agent to trace the key data flows end-to-end through the pipeline:

1. **Type flow**: How do types get defined (registry) → checked (ori_types) → evaluated (ori_eval) → compiled (ori_llvm)?
2. **Method dispatch flow**: Where is the canonical dispatch table? How does a method call route from parse → typecheck → eval/codegen?
3. **Error flow**: How do errors propagate across phase boundaries? Where do they get accumulated, deduplicated, formatted?
4. **Memory/RC flow**: How do ownership decisions flow from AIMS analysis → ARC pass → codegen emission → runtime?

This agent produces a **flow map** — a brief summary of how each major data category crosses the phase boundaries. This map is passed to all subsequent review agents as context.

#### Intelligence-assisted map (before agent dispatch)

Follow the canonical intel-summary injection protocol:

@.claude/skills/dual-tpr/compose-intel-summary.md

Per SSOT Step F — /impl-hygiene-review flow map: use `file-symbols "<crate/path>" --repo ori` per in-scope crate, `callers`/`callees` per major dispatch or boundary symbol, `similar "<symbol>" --repo rust,swift,lean4 --limit 5` for cross-backend / prior-art checks. Use this map as input to Pass 1 and Pass 2 so the review starts from actual call-graph structure.

