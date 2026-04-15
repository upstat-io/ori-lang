---
paths:
  - "**"
---

# Intelligence Graph

## Availability

The intelligence graph at `../lang_intelligence/` is OPTIONAL. Check before querying:
```
scripts/intel-query.sh status
```
If unavailable, proceed normally — intelligence is additive, never blocking.

## Canonical Pre-Query Protocol (SSOT)

Skills and commands that run a pre-query / intel-summary-injection workflow
MUST `@`-include `.claude/skills/dual-tpr/compose-intel-summary.md` — the
single source of truth for the availability-check → `file-symbols` → `callers`
→ `callees` → `similar` → bounded ≤500-char summary protocol. Inlining the
pattern instead of `@`-including the SSOT is an **Algorithmic DRY violation**
(`impl-hygiene.md` §SSOT). Domain-specific extensions (e.g., `/review-bugs`
using `search`/`fixed`, `/fix-bug` using `similar` on the repro symbol) live
in SSOT Step F or in the consumer's own section after the `@`-include; they
do NOT replace the base protocol.

The SSOT supplies the availability check too — only the SSOT file, this rule
file, and `.claude/commands/query-intel.md` are permitted to contain the
literal `scripts/intel-query.sh status` string. All other consumers acquire
it via `@`-include.

## When to Query

Query the intelligence graph proactively in these workflows. All entries
below are live consumers that query the graph today.

- **Design decisions**: Before choosing an approach, query `similar` for how reference compilers handled it
- **Bug investigation** (/fix-bug Phase 1): `callers`/`callees` for blast radius, `similar` for reference fixes
- **Bug autopilot** (/fix-next-bug): lightweight `callers`-only blast-radius preview on the selected bug's repro symbol to help gauge scope before choosing interactive vs. autopilot mode (Step 4.5 — /fix-bug Phase 1 runs its own full investigation)
- **Bug triage** (/review-bugs, /add-bug): `callers` to assess blast radius, `file-symbols` to cluster related bugs
- **TPR reviews** (/tpr-review Step 0.75): `file-symbols` + `callers`/`callees` for module inventory + blast radius
- **TPR triage** (/verify-tpr): `callers` for blast-radius on high-severity findings or findings where the blast radius is ambiguous (Step 2.5 — not every finding, to avoid query exhaustion; informs accept/reject decisions, does not replace them)
- **Code reviews** (/review-work, /independent-review): `file-symbols` for module context, `callers` for impact
- **Hygiene reviews** (/impl-hygiene-review): `callers`/`callees` for flow mapping, `similar` for cross-backend mirrors
- **Plan reviews** (/review-plan): `symbols`/`file-symbols` to validate plan assumptions against actual code
- **Plan creation** (/create-plan): `symbols`/`similar` for intelligence reconnaissance before architecture
- **Proposals** (/create-draft-proposal): `similar` + `symbols` for prior art discovery
- **Proposal review** (/review-draft-proposal): `similar` + `symbols` for conflict check and purity analysis
- **Pattern review** (/design-pattern-review): `similar` for instant cross-repo equivalents, `callers`/`callees` for Ori dispatch mapping
- **Third-party help** (/tp-help): `callers`/`callees`/`similar` to enrich context package for reviewers
- **Roadmap** (/continue-roadmap): `file-symbols`/`callers`/`callees`/`similar` for section-relevant code surface
- **Roadmap verification** (/verify-roadmap): Phase 1 review agents only — `file-symbols` on section-scope crates and `callers`/`callees` on high-signal symbols; supplies ambient blast-radius context before verification starts (Phase 2 update agents do not query)
- **Execution tracing** (/code-journey, /rosetta-test): `callers`/`callees` to map exercised paths, `similar` for cross-repo equivalents
- **Doc sync** (/sync-claude): `file-symbols` on changed crates to detect doc-symbol drift — symbols present in the graph but missing from rules files (new additions), or symbols in rules files but absent from the graph (removed/renamed) (Step 1.5)
- **Spec sync** (/sync-spec): `callers` on every symbol referenced in the spec change as a blast-radius check before identifying spec files — prevents silent behavior drift when a spec edit ships without updating an implementation call site (Update Process item 1)
- **Grammar sync** (/sync-grammar): `file-symbols "compiler/ori_parse/"` and `file-symbols "compiler/ori_lexer/"` to inventory parser/lexer types before reading grammar.ebnf; flags productions whose implementation symbol is not covered (parse-site gap) (Update Process item 1)
- **Tooling** (/improve-tooling): `symbols` to check if similar tools already exist before creating new ones

## How to Query

Always use the canonical helper — never open-code Neo4j access:
```
# Issue/PR search (external repos)
scripts/intel-query.sh search "pattern matching exhaustiveness"
scripts/intel-query.sh compare "type inference"
scripts/intel-query.sh fixed "memory leak" --repo rust,swift
scripts/intel-query.sh hot --repo rust
scripts/intel-query.sh ori-arc                          # also: ori-inference, ori-codegen, ori-patterns, ori-diagnostics
scripts/intel-query.sh sentiment pain --repo go         # rank by pain/controversy/excitement
scripts/intel-query.sh landscape --repo rust            # per-label sentiment aggregation
scripts/intel-query.sh ori-sentiment                    # highest-pain in ARC-relevant repos

# Code symbol queries (Ori + reference repos — 191K+ symbols, 505K+ CALLS edges)
scripts/intel-query.sh symbols "IteratorValue" --repo ori              # find symbols by name
scripts/intel-query.sh symbols "iter" --repo ori --kind function       # filter by kind (function|type|sum_type|...)
scripts/intel-query.sh callers "eval_iter_next" --repo ori             # who calls this function?
scripts/intel-query.sh callees "eval_iter_last" --repo ori             # what does it call?
scripts/intel-query.sh file-symbols "iterator/consumers" --repo ori    # all symbols in matching files

# Cross-repo semantic similarity (vector embeddings — finds equivalents by meaning, not name)
scripts/intel-query.sh similar "eval_iter_collect" --repo rust         # Rust equivalents to Ori function
scripts/intel-query.sh similar "emit_rc_inc" --repo rust,swift         # cross-repo codegen matches
scripts/intel-query.sh similar "check_exhaustiveness"                   # search ALL other repos

# Raw Cypher
scripts/intel-query.sh cypher "MATCH (i:Issue)-[:FIXES]->(b) RETURN count(i)"
```

## Symbol-First Workflow

When investigating code, reviewing changes, or exploring a subsystem, use this workflow BEFORE manual grep or reference-repo browsing:

1. **Inventory the module**: `scripts/intel-query.sh file-symbols "<path-fragment>" --repo ori`
2. **Map blast radius**: `scripts/intel-query.sh callers "<symbol>" --repo ori` and `callees "<symbol>" --repo ori`
3. **Find cross-repo equivalents**: `scripts/intel-query.sh similar "<symbol>" --repo rust,swift,go --limit 5`
4. **Then read the matched source** — the graph tells you WHERE to look; you still verify by reading the actual code

This replaces manual grep-based navigation. The graph answers "what calls X?", "what does X call?", and "what's the Rust equivalent of X?" in sub-second time. Use it aggressively — it's free and fast.

## Subsystem Mapping

Map file paths or bug subsystems to intelligence presets:

| File path pattern | Preset |
|---|---|
| `compiler/ori_arc/`, `compiler/ori_rt/src/rc/` | `ori-arc` |
| `compiler/ori_types/src/infer/`, `compiler/ori_types/src/check/` | `ori-inference` |
| `compiler/ori_llvm/` | `ori-codegen` |
| `compiler/ori_patterns/`, `compiler/ori_eval/src/methods/`, `compiler/ori_canon/src/patterns/` | `ori-patterns` |
| `compiler/ori_diagnostic/`, `compiler/oric/src/diagnostic/` | `ori-diagnostics` |
| Other / mixed | `search "<key terms from diff>"` |

Skills that query intelligence (`/tpr-review`, `/fix-bug`) reference this table — do not duplicate it.

## How to Use Results

Results are for DISCOVERY, not replacement:
- A hit means "investigate this" — read the actual source code and issue
- Weight by: state_reason (completed > not_planned), reactions, author_association (MEMBER > NONE)
- Cross-language convergence is highest signal (3+ repos hit same issue class)
- Never cite a Neo4j result without verifying against the actual code/issue
