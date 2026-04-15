# Intelligence Summary Injection — Canonical SSOT

**Single source of truth** for pre-query / summary-injection behavior. Every
current consumer `@`-includes this file from its intel section rather than
inlining the pattern.

**Current consumers (24, all `@`-including this file):**

- Review-family skills/commands (6): `/tpr-review` (Step 0.75),
  `/review-work` skill (Step 1.5), `/review-plan` (Step 3.5),
  `/review-work` command (Intelligence map), `/independent-review`
  (Phase B), `/review-bugs` (Step 5.5).
- Wider skill consumers (15): `/tp-help`, `/add-bug`, `/improve-tooling`,
  `/design-pattern-review`, `/create-draft-proposal`, `/fix-bug`,
  `/impl-hygiene-review`, `/review-draft-proposal`, `/create-plan`,
  `/rosetta-test`, `/code-journey`, `/continue-roadmap`, `/verify-tpr`,
  `/sync-claude`, `/fix-next-bug`.
- Command consumers (3): `/sync-spec`, `/sync-grammar`, `/verify-roadmap`.

**Planned future consumers (not yet migrated):**

- `.claude/hooks/pre-review-intel.sh` — this hook does not yet exist.
  It will be created by `plans/query-intel-adoption` §07 "Hook-heavy
  ambient automation" and will call this SSOT's protocol inline.

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

### Step F — Consumer extension registry

Step F is the canonical registry of per-consumer extensions that go beyond
the base SSOT protocol (Steps A-E). Each entry MUST match the consumer's
actual `scripts/intel-query.sh` invocations verbatim — drift between Step F
and the consumer file is a `DRIFT:intel-extension-registry` finding.
Extensions layer on top of the base protocol; they do NOT replace any step
above.

**Review-family (no extensions — base protocol only):**

The 6 review-family consumers (`/tpr-review` Step 0.75, `/review-work` skill
Step 1.5, `/review-plan` Step 3.5, `/review-work` command Intelligence map,
`/independent-review` Phase B, `/review-bugs` Step 5.5 availability-check
portion) use Steps A-E of the base protocol without workflow-specific
extensions. They inject the Intelligence Summary into reviewer prompts or
use it to prioritize adjacent-file reads. `/review-bugs` ADDITIONALLY uses
the bug-workflow extension below.

**TPR/verification consumers:**

- **`/verify-tpr`** (Step 2.5) — per-finding blast-radius:
  - `callers "<finding symbol>" --repo ori` (high-severity or ambiguous-blast-radius findings)

**Doc-sync consumers:**

- **`/sync-claude`** (Step 1.5) — crate symbol inventory:
  - `file-symbols "<crate-path-fragment>" --repo ori` (per changed crate)

**Bug-workflow consumers:**

- **`/review-bugs`** (Step 5.5) — bug cross-reference enrichment:
  - `search "<bug title keywords>" --limit 5`
  - `fixed "<bug category>" --repo rust,swift,koka,lean4 --limit 5`
  - `callers "<repro symbol>" --repo ori` (blast radius)
  - `file-symbols "<subsystem path>" --repo ori` (bug clustering)
  - `similar "<buggy function>" --repo rust,swift,koka,lean4 --limit 5` (reference fixes)

- **`/fix-bug`** (5a Intelligence Graph Query) — bug investigation:
  - `search "<bug description keywords>" --limit 5`
  - `fixed "<bug category>" --repo rust,swift,koka,lean4 --limit 5`
  - `similar "<buggy function or concept>" --repo rust,swift,koka,lean4 --limit 5`

- **`/add-bug`** (Step 4) — lightweight blast-radius:
  - `callers "<buggy function>" --repo ori`
  - `file-symbols "<subsystem path>" --repo ori`

- **`/fix-next-bug`** (Step 4.5) — lightweight blast-radius preview:
  - `callers "<repro symbol>" --repo ori`

**Planning/proposal consumers:**

- **`/create-plan`** (Step 2.5) — plan reconnaissance:
  - `symbols "<topic keyword>" --repo ori --limit 20`
  - `file-symbols "<likely path>" --repo ori`
  - For high-signal symbols: `callers`, `callees`, `similar "<symbol>" --repo rust,swift,go,koka --limit 5` (note: `koka` is included in addition to Step C's base `rust,swift,go` because plan reconnaissance benefits from Koka's effect-system prior art)

- **`/create-draft-proposal`** (Step 4.5) — prior-art reconnaissance:
  - `search "<proposal topic>" --limit 5`
  - `compare "<feature concept>" --limit 5`
  - `symbols "<Ori keyword>" --repo ori --limit 15`
  - `similar "<feature concept or Ori symbol>" --repo rust,swift,go,koka --limit 5`

- **`/review-draft-proposal`** (CONDITIONAL Prior Art) — conflict + purity analysis:
  - `symbols "<proposal topic>" --repo ori --limit 15`
  - `similar "<proposed feature concept>" --repo rust,swift,go --limit 5`

**Pattern/code-traversal consumers:**

- **`/design-pattern-review`** (STEP 1.5) — prior-art + Ori implementation mapping:
  - `compare "{DOMAIN}" --limit 5`
  - `search "{DOMAIN}" --limit 5`
  - Preset if `{DOMAIN}` maps per `.claude/rules/intelligence.md` §Subsystem Mapping
  - `symbols "{DOMAIN keyword}" --repo ori --kind function --limit 15`
  - `similar "{DOMAIN entry symbol}" --repo rust,swift,zig,gleam --limit 5`

- **`/code-journey`** (Intelligence map) — journey planning:
  - `symbols "<feature keyword>" --repo ori --limit 15`
  - `callers "<main exercised symbol>" --repo ori`, `callees`
  - `similar "<symbol>" --repo rust,swift,go --limit 5`

- **`/rosetta-test`** (I. Cross-Language Intelligence) — stress-test context:
  - `symbols "<feature keyword>" --repo ori --limit 15`
  - `file-symbols "<suspect module path>" --repo ori`
  - `callers "<failing symbol>" --repo ori`, `callees`
  - `similar "<failing symbol>" --repo rust,swift,go --limit 5`
  - `search "<failure mode>" --limit 5`

**Analysis/maintenance consumers:**

- **`/impl-hygiene-review`** (Intelligence-assisted map) — flow map:
  - `file-symbols "<crate/path>" --repo ori` per in-scope crate
  - `callers "<symbol>" --repo ori`, `callees` per major dispatch/boundary symbol
  - `similar "<symbol>" --repo rust,swift,lean4 --limit 5`

- **`/tp-help`** — context enrichment for third-party help:
  - `callers`/`callees`/`similar` on the discussed symbols

- **`/improve-tooling`** — pre-create existence check:
  - `symbols "<keyword>" --repo ori --kind function --limit 10`

- **`/continue-roadmap`** (Step 2.1) — per-section reconnaissance:
  - `search "<title keywords>" --limit 5`
  - Preset if the section title maps to a subsystem
  - `file-symbols`, `callers`/`callees`, `similar` per Step C on section-body symbols

**Spec/grammar consumers:**

- **`/sync-spec`** (Update Process item 1) — blast-radius before spec edits:
  - `callers "<affected symbol>" --repo ori`

- **`/sync-grammar`** (Update Process item 1) — parser/lexer type inventory:
  - `file-symbols "compiler/ori_parse/" --repo ori`
  - `file-symbols "compiler/ori_lexer/" --repo ori`

**Roadmap consumers:**

- **`/verify-roadmap`** (Phase 1, Step 2 agent prompt) — review-agent context:
  - `file-symbols "<section scope crate>" --repo ori`
  - `callers`/`callees` on high-signal symbols

**Registry contract:** when a consumer adds, removes, or changes queries in
its extension, the maintainer MUST update this Step F entry in the same
commit. This is an SSOT obligation — Step F is the single source of truth
for consumer extensions. Extensions stay bounded: each adds at most 2-3
bullets to the summary, keeping the 500-char / 5-bullet cap. Never cite an
extension result as authoritative — verify-before-citing (Step D) applies
to all queries. New consumers follow the same pattern: `@`-include the SSOT
for availability check + Steps B-E, then add their domain-specific
subcommands in their local section AND register them here.

## Graceful degradation

If `scripts/intel-query.sh status` returns unavailable, the entire
summary is OMITTED. Do NOT emit an empty "Intelligence Summary: no
results" block — that's noise. The consumer's prompt should be
syntactically valid whether or not the summary appears.

## Banned Patterns

- Inlining this template in any consumer instead of `@`-including it
- Open-coding Neo4j access (bypassing `scripts/intel-query.sh`)
- Emitting a summary without the availability check
- Citing a graph result without verifying against actual code

## Consumers

Every consumer of this file references it via `@.claude/skills/dual-tpr/compose-intel-summary.md`
at its intel section. Updates to this protocol propagate automatically.

**Full consumer list (24 total), by origin section:**

Migrated in `plans/query-intel-adoption` §03 (18 original consumers):

- Skills (14): `/tpr-review`, `/review-work` (skill), `/review-plan`, `/independent-review`, `/review-bugs`, `/tp-help`, `/add-bug`, `/improve-tooling`, `/design-pattern-review`, `/create-draft-proposal`, `/fix-bug`, `/impl-hygiene-review`, `/review-draft-proposal`, `/create-plan`, `/rosetta-test`, `/code-journey`, `/continue-roadmap`
- Commands (1): `/review-work` (command)

Migrated in `plans/query-intel-adoption` §05.1 (3 skills):

- `/verify-tpr`, `/sync-claude`, `/fix-next-bug`

Migrated in `plans/query-intel-adoption` §05.2 (3 commands):

- `/sync-spec`, `/sync-grammar`, `/verify-roadmap`

**How to add yourself as a consumer:**

1. Add `@.claude/skills/dual-tpr/compose-intel-summary.md` at your skill/command's
   intel section (not at the top — place it where the query is invoked).
2. If your consumer runs queries beyond Steps A-E, add a Step F entry in the
   registry above describing the exact `scripts/intel-query.sh` subcommands you
   call. Keep extensions bounded (2-3 extra bullets max).
3. Update the "Current consumers" count at the top of this file.
4. Update `.claude/rules/intelligence.md` "When to Query" with a matching bullet
   (co-committed with the consumer edit — per the §05.3 sequencing note).
5. Add yourself to the consumer list in this section under a new "Migrated in
   …" grouping.

## Related

- `.claude/rules/intelligence.md` — when-to-query workflow inventory, subsystem mapping
- `.claude/skills/query-intel/SKILL.md` — full capability surface
- `scripts/intel-query.sh` — the canonical wrapper (206 lines; see §08 for planned UX improvements)
- `.claude/skills/dual-tpr/polling-protocol.md` — sibling SSOT for dual-source polling
- `.claude/skills/dual-tpr/compose-rules-brief.md` — sibling SSOT for rules-brief composition
