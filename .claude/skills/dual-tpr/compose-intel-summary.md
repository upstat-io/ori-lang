# Intelligence Summary Injection — Canonical SSOT

**Single source of truth** for pre-query / summary-injection behavior across
ALL consumers: /tpr-review Step 0.75, /review-work Step 1.5, /review-plan,
/independent-review, /review-bugs, /tp-help, /verify-tpr, /sync-claude,
/fix-next-bug, /fix-bug, and the .claude/hooks/pre-review-intel.sh hook.
Every such consumer MUST @-include this file from its intel section rather
than inlining the pattern.

## Protocol

### Step A — Availability check

```
Bash (foreground):
  scripts/intel-query.sh status
```

Parse the JSON. If `status != "ok"`, skip silently. Do NOT emit an empty
section in the consumer's prompt — skipping means NO Intelligence Summary
block appears at all.

### Step B — Subsystem and symbol identification

For code/plan review modes, use the same scope the consumer is operating
on (e.g., `git diff --name-only HEAD~5..HEAD`). For custom-objective mode,
extract relevant file paths or symbol names from the objective text.

Map subsystems to presets per `.claude/rules/intelligence.md` §Subsystem
Mapping. DO NOT hardcode the mapping here.

### Step C — Run the queries

Up to 5 queries total to keep the summary bounded. Output is visible in
Claude's context — do NOT capture into a variable.

1. Subsystem preset OR directed search:
   `scripts/intel-query.sh --human <preset-or-search> --limit 5`
2. For top 3-5 changed files:
   `scripts/intel-query.sh --human file-symbols "<path-fragment>" --repo ori`
3. For each high-signal symbol:
   `scripts/intel-query.sh --human callers "<symbol>" --repo ori`
   `scripts/intel-query.sh --human callees "<symbol>" --repo ori`
   `scripts/intel-query.sh --human similar "<symbol>" --repo rust,swift,go --limit 5`

If any query returns empty, skip it silently in the summary.

### Step D — Condense into a bounded Intelligence Summary (≤500 chars)

Format:

```
**Intelligence Summary (from intelligence graph):**
- [rust#12345] Similar bug / pattern — short phrase (N reactions)
- [swift#6789] Reference implementation — short phrase
- [ori] <symbol> called by N sites across M modules — blast radius note
```

Rules:
- Maximum 5 bullets.
- Maximum 500 characters (hard cap; truncate with `…` if needed).
- Reference-repo citations use `[repo#N]` issue shorthand or
  `[repo:path]` for symbol results.
- Ori citations use `[ori]` prefix.
- Do NOT cite a result as authoritative — this is DISCOVERY for the
  consumer, not conclusions.

### Step E — Inject into the consumer's prompt

The consumer is responsible for placing the summary into its own prompt
template (e.g., after the `## Scope:` header in a reviewer prompt,
after the objective in a custom-objective prompt). This helper produces
the summary text; the consumer chooses where to place it.

### Step F — Optional domain-specific extensions

Some consumers extend Step C with workflow-specific queries. Extensions
layer on top of the base protocol; they do NOT replace any step above.

**Review-family:**
- **`/review-bugs`** — for each high-priority bug:
  - `search "<bug title keywords>" --limit 5`
  - `fixed "<bug category>" --repo rust,swift,koka,lean4 --limit 5`
  - Enrich `### Recommended Actions` with fix-approach confidence.
- **`/fix-bug`** Phase 1 investigation:
  - `callers <repro_symbol>` for blast radius
  - `similar <repro_symbol> --repo rust,swift,koka,lean4` for reference fixes
- **`/impl-hygiene-review`** flow map:
  - `file-symbols "<crate/path>" --repo ori` per in-scope crate
  - `similar "<boundary symbol>" --repo rust,swift,lean4` for prior art

**Planning/proposal:**
- **`/create-plan`**, **`/create-draft-proposal`** — prior-art reconnaissance:
  - `symbols "<topic>" --repo ori --limit 15-20`
  - `search "<topic>" --limit 5`, `compare "<feature concept>" --limit 5`
- **`/review-draft-proposal`** — conflict + purity analysis:
  - `symbols "<proposal topic>" --repo ori --limit 15`
  - `similar "<proposed feature>" --repo rust,swift,go --limit 5`

**Traversal/testing:**
- **`/code-journey`**, **`/rosetta-test`** — exercised-path mapping:
  - `symbols "<feature keyword>" --repo ori`
  - `callers "<main exercised symbol>"`, `callees`
  - `similar "<symbol>" --repo rust,swift,go --limit 5`

**Maintenance:**
- **`/sync-claude`** — drift detection: `file-symbols` on `.claude/` paths
- **`/improve-tooling`** — pre-create existence check: `symbols "<keyword>" --repo ori --kind function`
- **`/add-bug`** — lightweight blast-radius: `callers "<buggy function>" --repo ori`
- **`/tp-help`** — context enrichment: `callers`/`callees`/`similar` for the discussed symbols
- **`/continue-roadmap`** — per-section reconnaissance: `search "<section title>"` + opportunistic preset

Extensions stay bounded: each adds at most 2-3 bullets to the summary,
keeping the 500-char / 5-bullet cap. Never cite an extension result as
authoritative — verify-before-citing (Step D) applies to all queries.
New consumers follow the same pattern: @-include the SSOT for
availability check + Step B-D, then add domain-specific subcommands
in their local section.

## Graceful degradation

If `scripts/intel-query.sh status` returns unavailable, the entire
summary is OMITTED. Do NOT emit an empty "Intelligence Summary: no
results" block — that's noise. The consumer's prompt should be
syntactically valid whether or not the summary appears.

## Banned patterns

- Inlining this template in any consumer instead of `@`-including it
- Open-coding Neo4j access (bypassing `scripts/intel-query.sh`)
- Emitting a summary without the availability check
- Citing a graph result without verifying against actual code

## Consumers

Every consumer of this file references it via `@.claude/skills/dual-tpr/compose-intel-summary.md`
at its intel section. Updates to this protocol propagate automatically.

## Related

- `.claude/rules/intelligence.md` — when-to-query workflow inventory, subsystem mapping
- `.claude/skills/query-intel/SKILL.md` — full capability surface
- `scripts/intel-query.sh` — the canonical wrapper (206 lines; see §08 for planned UX improvements)
- `.claude/skills/dual-tpr/polling-protocol.md` — sibling SSOT for dual-source polling
- `.claude/skills/dual-tpr/compose-rules-brief.md` — sibling SSOT for rules-brief composition
