# Phase 6 — Generate Plan (Separate Agent)

Read by an **Opus** sub-agent dispatched from `/impl-hygiene-review` ONLY when the coordinator decides a plan is needed (e.g., findings count exceeds inline-fix scope). Not a registered skill. This is judgment-heavy plan authorship — which findings group into which plan sections, what the dependency order is, what the mission success criteria should be.

Consumes `/tmp/impl-hygiene-{run}/phase-5.json`. Writes `/tmp/impl-hygiene-{run}/phase-6.json` with: plan directory path created, sections outlined, findings assigned to sections.

---

### Phase 6: Generate Plan (Separate Agent)

Spawn a **separate Agent** to generate the fix plan. This agent should use `/create-plan` (via the **Skill** tool). Pass it:

1. **All compiled findings** from Phase 5
2. **The plan name**: `hygiene-{target-short-name}` (e.g., `hygiene-ori-types`, `hygiene-lexer-parser`, `hygiene-last-commit`, `hygiene-full`)
3. **The mission statement** — The plan's `## Mission` MUST express the architectural end state, not enumerate findings. The mission is about what the code should **become**: a cohesive architecture with clean design, clear phase boundaries, correct solutions, and every piece of knowledge in its canonical home. Then describe the **specific design problems** in each area — what's architecturally wrong, not category counts. The standard is `.claude/rules/impl-hygiene.md`.

   **Pattern:**
   > Achieve {architectural end state description}. This sweep addresses {area A} — where {design problem in A}; {area B} — where {design problem in B}; and {area C} — where {design problem in C}. The standard is `.claude/rules/impl-hygiene.md`.

   **Rules:**
   - Frame as the **architectural destination**, not a task manifest — "cohesive architecture with clean design and correct solutions", not "eliminate N LEAKs"
   - Describe each area's **design problem** — "scattered cow_mode checks with no canonical dispatch", not "5 LEAK findings in ori_rt"
   - Finding counts, category breakdowns, and priority ordering belong in `## Metrics`, NOT in the mission
   - The mission must read as a design vision that someone could evaluate the code against when the work is done

The agent should create a plan that:

1. Lists every LEAK, DRIFT, GAP, WASTE, EXPOSURE, and BLOAT finding with `file:line` references
2. Groups by boundary (e.g., "lexer→parser", "parser→types") or by violation type for full-project mode
3. Estimates scope: "N boundaries, ~M findings"
4. Orders: **LEAKs first and separately** (side logic is the root of all evil — every LEAK is a ticking architectural bomb), then drift (sync), then gaps (feature coverage), then bloat (file organization), then waste (perf), then exposure (type safety). LEAKs must NEVER be deferred — they cascade.
5. **Algorithmic duplication findings get their own section** — these often require coordinated multi-file refactoring (extracting a shared helper, adding a generic function, creating a data-driven dispatch table). Group by the algorithm being duplicated, not by where the copies live.

The **final section** of the plan must be a cleanup step:

```markdown

---

### Plan Section Format

Each section groups findings by boundary or violation cluster:

```
## {Boundary: Phase A → Phase B}

**Interface types:** {list types crossing this boundary}
**Entry points:** {list key functions}

### Active Plan Context

{List each plan file read and its relevance. If a plan has a reroute/suspension, note it here.}
- `plans/trait_arch/` — Active reroute: all roadmap work suspended until trait architecture refactor completes
- (none) — if no plan files were found

### Findings

1. **[LEAK:duplicated-dispatch]** `file:line` — {description} — **canonical home**: `{canonical_file:line}`
2. **[LEAK:algorithmic-duplication]** `file_a:line` ↔ `file_b:line` — {description of shared skeleton} — **extraction**: {generic fn / HOF / trait / data-driven / macro}
3. **[LEAK:scattered-knowledge]** `file:line` — {description} — **canonical home**: `{canonical_file:line}`
4. **[DRIFT]** `file:line` — {description}
   → covered by plans/{plan}/ ({section name})
5. **[DRIFT] [PLANNED]** `file:line` — {description}
   → fix described in plans/{plan}/{section}.md
6. **[GAP]** `file:line` — {description}
7. **[WASTE]** `file:line` — {description}
8. **[EXPOSURE]** `file:line` — {description}
```

