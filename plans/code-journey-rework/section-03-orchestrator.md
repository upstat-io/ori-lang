---
section: "03"
title: "Orchestrator Skill"
status: not-started
reviewed: false
goal: "Rewrite the SKILL.md orchestrator to compose real diagnostic tools and AI IR analysis, producing canonical JSON results"
success_criteria:
  - "/code-journey runs on a single .ori file and produces valid JSON matching the Section 02 schema"
  - "Fast mode (tools only) completes in < 30 seconds per journey"
  - "Deep mode (tools + AI) completes in < 5 minutes per journey"
  - "Background agent runs diagnostic tools first, then AI analysis (tool failures block AI)"
  - "All existing modes work: single file, --add, --summary, --infinity, run-all"
  - "Satisfies mission criterion: /code-journey produces JSON + markdown, not scores"
inspired_by:
  - "Current SKILL.md Steps 0-2 (reusable phase capture infrastructure)"
  - "diagnose-aot.sh (multi-tool composition pattern)"
  - "Codex consultation: background agent owns all post-capture, hybrid AI prompt"
depends_on: ["02", "04"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "03.1"
    title: "Main Agent Orchestration"
    status: not-started
  - id: "03.2"
    title: "Background Agent — Diagnostic Tools (Fast Mode)"
    status: not-started
  - id: "03.3"
    title: "Background Agent — AI IR Analysis (Deep Mode)"
    status: not-started
  - id: "03.4"
    title: "JSON Production & Markdown Generation"
    status: not-started
  - id: "03.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "03.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 03: Orchestrator Skill

**Status:** Not Started
**Goal:** Rewrite SKILL.md to compose real diagnostic tools with AI IR analysis, replacing the broken Python scoring pipeline. The orchestrator preserves the proven main agent / background agent separation but changes what the background agent does.

**Success Criteria:**
- [ ] `/code-journey 01-arithmetic.ori` produces a valid JSON result file
- [ ] Fast mode (diagnostic tools only) runs in < 30s per journey
- [ ] Deep mode (tools + AI) runs in < 5 min per journey
- [ ] Tool failures block AI analysis (not the reverse)
- [ ] All modes work: single file, --add N, --summary, --infinity, run-all
- [ ] Satisfies mission criteria for JSON + markdown output

**Context:** The main agent's Steps 0-2 (build run list, run eval/AOT, capture phase dumps) are proven infrastructure — reuse them exactly. Step 3 (spawn background agent) stays architecturally but the background agent's instructions completely change. Step 4 (status/mode logic) is reusable. Step 5 (overview generation) is rewritten to produce from JSON.

**Reference implementations:**
- **Current SKILL.md** Steps 0-2: Phase capture with env vars and temp files
- **diagnose-aot.sh** `diagnostics/diagnose-aot.sh`: Multi-tool composition pattern (7 sections, sequential execution, status tracking)

**Depends on:** Section 02 (JSON schema — must know what to produce) and Section 04 (tool JSON output — can use structured output if available).

---

## 03.1 Main Agent Orchestration

**File(s):** `.claude/skills/code-journey/SKILL.md`

The main agent's responsibilities are unchanged from the current system. Rebuild the SKILL.md skeleton (from Section 01.2) into the complete new orchestrator.

- [ ] Preserve Step 0: Build the Run List — scan `plans/code-journeys/*.ori`, parse args, determine mode
- [ ] Preserve Step 1: Run Both Paths — eval (`cargo run -- run file.ori`), AOT (`ori build file.ori` + execute binary), capture exit codes + stdout to `/tmp/journey_N/`
- [ ] Preserve Step 2: Capture Phase Dumps — all env vars and temp files:
  - `ORI_LOG=ori_lexer=debug` → `lexer.txt`
  - `ORI_LOG=ori_parse=debug` → `parser.txt`
  - `ORI_LOG=ori_types=debug` → `typeck.txt`
  - `ORI_DUMP_AFTER_ARC=1` → `arc_ir.txt`
  - `ORI_DUMP_AFTER_LLVM=1` → `llvm_ir.txt`
  - `size -A binary` → `sections.txt`
  - `objdump -d binary` → `disasm.txt`
- [ ] Add Step 2.5: Capture provenance metadata to `/tmp/journey_N/provenance.json`:
  ```json
  { "commit": "$(git rev-parse HEAD)", "dirty": $(git diff --quiet && echo false || echo true), "timestamp": "$(date -Iseconds)" }
  ```
- [ ] Rewrite Step 3: Spawn background agent with new prompt template (see 03.2/03.3)
  - Pass: journey metadata, source file path, eval/AOT results, temp dir path, analysis mode (fast/deep)
  - Default mode: `deep` when running single journey or run-all; `fast` when running `--add` batch
- [ ] Preserve Step 4: Mode-dependent continue/stop logic
- [ ] Rewrite Step 5: Overview generation reads JSON results, not YAML frontmatter (deferred to Section 06)
- [ ] Add `--fast` flag: force fast mode on any invocation
- [ ] Add `--deep` flag: force deep mode on any invocation (default for single journey)

- [ ] **Subsection close-out (03.1)** — MANDATORY before starting 03.2:
  - [ ] All tasks above are `[x]` and SKILL.md main agent flow verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**

---

## 03.2 Background Agent — Diagnostic Tools (Fast Mode)

**File(s):** `.claude/skills/code-journey/SKILL.md` (background agent prompt template)

The background agent receives the temp directory with all phase dumps and runs diagnostic tools against the journey program. This is the "fast mode" — deterministic, < 30 seconds.

- [ ] Define the background agent prompt template for fast mode. The agent:
  1. Reads journey metadata (number, slug, theme, difficulty, features, expected result)
  2. Reads eval/AOT exit codes and stdout from temp files
  3. Runs `ORI_CHECK_LEAKS=1 /tmp/journey_N/binary` → captures live count + status
  4. Runs `diagnostics/rc-stats.sh file.ori` → captures per-function balance (use `--json` if available from Section 04, otherwise parse text output)
  5. Runs `diagnostics/codegen-audit.sh file.ori` → captures findings (use `--json` if available, otherwise parse text)
  6. Runs `timeout 150 diagnostics/dual-exec-verify.sh --test-only --json file.ori` → if applicable
  7. Reads `llvm_ir.txt` metadata: line count, function count (grep for `^define`)
  8. Reads `sections.txt` for binary size data
  9. Reads `provenance.json`
  10. Assembles the JSON result per Section 02 schema (findings from tools only, ai_gate = "skip")
  11. Writes `plans/code-journeys/NN-slug-results.json`
  12. Generates `plans/code-journeys/NN-slug-results.md` from JSON (template in Section 06)
  13. Cleans up `/tmp/journey_N/`

- [ ] Tool failure handling:
  - If AOT compilation fails: execution.aot.status = "compile_fail", skip leak_check + rc_stats + codegen_audit, set diagnostic statuses to "skip"
  - If eval fails but AOT succeeds (or vice versa): record parity mismatch as a finding
  - If any diagnostic tool crashes: record as finding with category "tool_failure", continue other tools

- [ ] Test fast mode: run on J01 (arithmetic — simplest journey), verify JSON output matches schema

- [ ] **Subsection close-out (03.2)** — MANDATORY before starting 03.3:
  - [ ] All tasks above are `[x]` and fast mode produces valid JSON
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**

---

## 03.3 Background Agent — AI IR Analysis (Deep Mode)

**File(s):** `.claude/skills/code-journey/SKILL.md` (background agent prompt template, deep mode extension)

Deep mode runs after fast mode's diagnostic tools complete successfully. If tools found blocking issues (compilation failure, parity mismatch), AI analysis is skipped (tool_gate = "fail", ai_gate = "skip"). Otherwise, the agent reads the LLVM IR and performs structured analysis.

- [ ] Define the AI analysis prompt structure (hybrid: structured passes + open-ended):

  **Structured passes** (checklist — agent MUST check each):
  1. **RC lifecycle**: For each function, verify inc/dec balance matches rc-stats output. Look for paths where inc occurs but dec is conditional (branch-dependent leaks).
  2. **Closure/env ownership**: Check that closure environment allocation has matching deallocation. Look for captured fat pointers (str, [T]) whose RC is not properly managed.
  3. **Iterator/drop semantics**: Verify that `ori_iter_drop` is called on every path (including break, continue, early exit). Check elem_dec_fn invocation.
  4. **ABI/attributes**: Verify nounwind, noalias, readonly, dereferenceable are present where applicable. Flag missing attributes.
  5. **Control flow**: Check for empty blocks, redundant branches (both targets identical), trivial phi nodes, unreachable code after noreturn calls.
  6. **Aggregate materialization**: Look for field-by-field GEP+load+insertvalue chains that should be aggregate loads.
  7. **Landing pads**: Verify invoke+landingpad is only used for functions that CAN throw. Nounwind functions should use call, not invoke.
  8. **Unwind cleanup**: Check that landingpad blocks properly decrement RC for all live values. Missing cleanup = panic-path leak.

  **Open-ended anomaly pass**:
  - "What would a careful LLVM codegen reviewer flag that the structured passes missed?"
  - Every finding MUST cite exact IR lines as evidence
  - Cross-reference with diagnostic tool results (don't re-report what tools already found)

- [ ] Seed the analysis with historical bug classes from overview.md:
  - Option struct wrapping (+7 unjustified instructions per iteration)
  - [str] double-free in iterator + list cleanup
  - Closure capturing str → unresolved type variable Idx leak
  - Aggregate field-by-field materialization (9:1 reduction opportunity)
  - Landing pad over-generation on nounwind functions
  - SSO guard ptrtoint duplication

- [ ] Finding output format: each finding produces a JSON object matching the schema's findings array item (category, severity, location, title, evidence, source: "ai:ir_analysis")

- [ ] Test deep mode: run on J15 (nested fat — the journey with known LOWs), verify AI analysis finds the dead loop load that tool-only mode might miss

- [ ] **Subsection close-out (03.3)** — MANDATORY before starting 03.4:
  - [ ] All tasks above are `[x]` and deep mode produces valid JSON with AI findings
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**

---

## 03.4 JSON Production & Markdown Generation

**File(s):** `.claude/skills/code-journey/SKILL.md`

- [ ] Implement the JSON assembly logic in the background agent prompt:
  - Merge tool results + AI findings into the schema structure
  - Compute finding ID fingerprints (SHA-256 of category+location+title, truncated to 12 hex)
  - Set verdict: clean (0 findings), findings (has findings), blocked (tool_gate fail)
  - Write to `plans/code-journeys/NN-slug-results.json`

- [ ] Implement markdown generation (simple template — full implementation in Section 06):
  - Read JSON, produce markdown with sections: Source, Execution, Diagnostics, Pipeline Summary, Findings, Verdict
  - Write to `plans/code-journeys/NN-slug-results.md`

- [ ] Auto-filing via `/add-bug`: when findings have severity >= medium AND source is tool-based (not AI note-level):
  - Invoke `/add-bug` with: subsystem derived from journey features, severity from finding, repro = journey file path, source = "code-journey"
  - Record filed bug ID in finding's `filed_as` field

- [ ] **Subsection close-out (03.4)** — MANDATORY before marking section complete:
  - [ ] All tasks above are `[x]` and end-to-end flow verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**

---

## 03.R Third Party Review Findings

- None.

---

## 03.N Completion Checklist

- [ ] `/code-journey 01-arithmetic.ori` produces valid JSON + markdown
- [ ] `/code-journey 15-fat-nested-collections.ori` in deep mode finds the known LOWs
- [ ] Fast mode completes in < 30s per journey
- [ ] Deep mode completes in < 5 min per journey
- [ ] All modes work (single file, --add, --summary, --infinity, run-all)
- [ ] `--fast` and `--deep` flags override default mode selection
- [ ] `/add-bug` integration fires on medium+ findings
- [ ] `timeout 150 ./test-all.sh` green
- [ ] `/tpr-review` — dual-source review of orchestrator
- [ ] `/impl-hygiene-review` — verify SSOT (JSON is sole canonical artifact), no LEAK (background agent owns all post-capture)
- [ ] `/improve-tooling` section-close sweep
