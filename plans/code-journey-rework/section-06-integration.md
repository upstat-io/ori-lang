---
section: "06"
title: "Bug Integration & Markdown Generation"
status: not-started
reviewed: false
goal: "Integrate /add-bug auto-filing, implement JSON-to-markdown rendering, build the batch runner, and regenerate overview.md"
success_criteria:
  - "Findings with severity >= medium are auto-filed via /add-bug with source code-journey"
  - "Markdown results files are generated deterministically from JSON (not hand-written)"
  - "Batch runner (run-all-journeys.sh or equivalent) runs all 30+ journeys in fast mode < 5 min"
  - "overview.md is regenerated from JSON results (not from YAML frontmatter)"
  - "Regression detection: diff two JSON runs to show new/resolved findings"
  - "Satisfies mission criteria for /add-bug integration and batch execution"
inspired_by:
  - "Current overview.md structure (plans/code-journeys/overview.md) — Journey Index + Recurring Issues is gold"
  - "rescore-all.sh batch pattern (replaced but architecturally similar)"
depends_on: ["03", "05"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "06.1"
    title: "/add-bug Integration"
    status: not-started
  - id: "06.2"
    title: "Markdown Generation from JSON"
    status: not-started
  - id: "06.3"
    title: "Batch Runner & Overview Regeneration"
    status: not-started
  - id: "06.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "06.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 06: Bug Integration & Markdown Generation

**Status:** Not Started
**Goal:** Close the loop between code journey findings and the project's bug-tracking + documentation infrastructure. When journeys find bugs, they get filed. When journeys run, they produce human-readable results. When all journeys run, the overview is regenerated.

**Success Criteria:**
- [ ] `/add-bug` auto-fires for medium+ severity tool findings
- [ ] `-results.md` files generated deterministically from `-results.json`
- [ ] Batch runner executes all journeys and produces aggregate results
- [ ] `overview.md` regenerated from JSON with Journey Index + Recurring Issues
- [ ] Satisfies mission criteria for integration and markdown

**Context:** The old system had manual 28-35KB markdown files as the primary artifact. The new system makes JSON canonical and generates markdown from it. The `/add-bug` integration is new — it connects code journey bug-finding to the project's existing bug tracking infrastructure, closing the discovery-to-tracking loop.

**Depends on:** Section 03 (orchestrator producing JSON), Section 05 (expanded corpus to run).

---

## 06.1 /add-bug Integration

**File(s):** `.claude/skills/code-journey/SKILL.md` (background agent prompt), `.claude/skills/add-bug/`

When a code journey produces findings with severity >= medium from tool-based sources (not AI notes), the background agent should invoke `/add-bug` to file them in the bug tracker.

- [ ] Define the auto-filing criteria:
  - Severity >= medium (critical, high, medium — not low, not note)
  - Source is `tool:*` (not `ai:ir_analysis` at note level — AI note-level findings need human triage first)
  - Finding not already filed (check `filed_as` field — fingerprint-based deduplication)

- [ ] Define the mapping from journey finding to `/add-bug` fields:
  - **Title**: Finding title from JSON
  - **Severity**: Finding severity (critical → critical, high → high, medium → medium)
  - **Subsystem**: Map from journey features → bug-tracker sections:
    - `arc`, `cow`, `strings`, `lists` → section-04 (ori_llvm/ori_arc)
    - `closures`, `iterators`, `generics` → section-03 (ori_eval/ori_patterns)
    - `pattern_matching`, `option_type` → section-02 (ori_types)
  - **Repro**: Journey file path (`plans/code-journeys/NN-slug.ori`)
  - **Source**: `code-journey`
  - **Evidence**: Finding evidence from JSON

- [ ] Update the JSON result: set `filed_as` field to the bug ID after filing
- [ ] Deduplication: if a finding fingerprint already exists in the bug tracker, skip filing

- [ ] **Subsection close-out (06.1)** — MANDATORY before starting 06.2:
  - [ ] All tasks above are `[x]` and auto-filing tested
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**

---

## 06.2 Markdown Generation from JSON

**File(s):** Background agent prompt template (or standalone script)

Generate `-results.md` deterministically from `-results.json`. The markdown serves human reading and git history — it is NOT the canonical artifact (JSON is).

- [ ] Define the markdown template sections:
  1. **YAML Frontmatter**: journey number, slug, theme, difficulty, features, status, date
  2. **Source**: The `.ori` source code (read from file, embedded)
  3. **Execution Results**: Table with eval/AOT exit codes, expected, parity
  4. **Diagnostics**: Gate results — leak check, RC stats, codegen audit, dual exec (pass/fail/warn/skip)
  5. **Pipeline Summary**: Token count, node count, IR line count, binary size (from pipeline block)
  6. **Findings**: Table with ID, severity, category, location, title, evidence
  7. **Verdict**: Clean / Findings / Blocked with counts

- [ ] Template must produce deterministic output (same JSON → same markdown, byte-for-byte)
  - Sort findings by severity (critical first), then by ID
  - Use consistent date formatting (ISO 8601)
  - Use consistent number formatting

- [ ] Generate markdown for one journey and verify it matches expected structure
- [ ] No score section, no ideal IR comparison, no 7-dimension breakdown — those are gone

- [ ] **Subsection close-out (06.2)** — MANDATORY before starting 06.3:
  - [ ] All tasks above are `[x]` and markdown generation tested
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**

---

## 06.3 Batch Runner & Overview Regeneration

**File(s):** `.claude/skills/code-journey/SKILL.md` (run-all mode), `plans/code-journeys/overview.md`

- [ ] Implement the run-all mode in SKILL.md:
  - Scan all `plans/code-journeys/*.ori` files
  - Run each through the fast-mode pipeline (tools only)
  - Collect all JSON results
  - Generate overview from aggregated results

- [ ] Redesign `overview.md` generation from JSON:
  - **Journey Index**: `# | Name | Features | Expected | Eval | AOT | Verdict | Findings`
  - **Recurring Issues**: Aggregate findings by fingerprint across journeys — if same finding appears in multiple journeys, list them together
  - **Resolved Issues**: Keep the historical record (manually curated, not auto-generated)
  - **Coverage Matrix**: Map features × diagnostic tools — show which journeys test which features
  - **Remove**: Score Trend table, comparison with previous runs (score-based), positive patterns (nice but not actionable)

- [ ] Implement regression detection:
  - Compare JSON results from two runs (current vs baseline)
  - Report: new findings (regression), resolved findings (improvement), unchanged findings
  - Baseline stored as committed JSON files; new run produces fresh JSON

- [ ] Test batch mode: run all 30+ journeys, verify overview.md generated correctly

- [ ] **Subsection close-out (06.3)** — MANDATORY before marking section complete:
  - [ ] All tasks above are `[x]` and batch mode + overview verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**

---

## 06.R Third Party Review Findings

- None.

---

## 06.N Completion Checklist

- [ ] `/add-bug` auto-filing works for medium+ tool findings
- [ ] Deduplication prevents duplicate bug filings
- [ ] Markdown generation produces clean, deterministic `-results.md` files
- [ ] Batch runner completes all 30+ journeys in fast mode < 5 minutes
- [ ] `overview.md` regenerated with Journey Index + Recurring Issues
- [ ] Regression detection diff works between two JSON runs
- [ ] `timeout 150 ./test-all.sh` green
- [ ] `/tpr-review` — dual-source review of integration work
- [ ] `/impl-hygiene-review` — verify SSOT (JSON → markdown → overview is one-directional), no DRIFT
- [ ] `/improve-tooling` section-close sweep
- [ ] **Plan close-out**: Run `/create-plan` to create the website plan — redo the "What is a Code Journey" explainer page, update `journey-parser.ts` to consume the new JSON schema, replace score visualizations with findings/diagnostics display. Website repo: `/home/eric/projects/ori-lang-website/`
