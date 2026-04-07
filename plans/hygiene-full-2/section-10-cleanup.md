---
section: "10"
title: "Cleanup"
status: not-started
reviewed: false
goal: "Verify all hygiene fixes, run full test suite, and delete this plan directory"
depends_on: ["01", "02", "03", "04", "05", "06", "07", "08", "09"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "10.1"
    title: "Final Verification"
    status: not-started
  - id: "10.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "10.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 10: Cleanup

**Status:** Not Started
**Goal:** Verify no behavior changes across the entire hygiene sweep, then delete this plan.

**Depends on:** All previous sections.

---

## 10.1 Final Verification

- [ ] `timeout 150 ./test-all.sh` -- all tests pass
- [ ] `timeout 150 cargo b --release && timeout 150 ./test-all.sh` -- release build + tests pass (FastISel differences can cause issues)
- [ ] `./clippy-all.sh` -- zero warnings
- [ ] `./fmt-all.sh` -- no formatting changes
- [ ] `bash .claude/skills/impl-hygiene-review/plan-annotations.sh` -- zero stale annotations from THIS plan (hygiene-full-2)
- [ ] No production files >500 lines: `find compiler/ -name "*.rs" -not -path "*/test*" -not -path "*/bench*" -not -path "*/target/*" | while read f; do lines=$(wc -l < "$f"); if [ "$lines" -gt 500 ]; then echo "$lines $f"; fi; done | sort -rn` (excluding validated exemptions with `// FILE SIZE EXEMPTION:` comments)
- [ ] No production functions >100 lines (excluding validated exemptions with `// SIZE EXEMPTION:` comments)
- [ ] `grep -rn "// ===\|// ---\|// ───\|// ──" compiler/*/src/ --include="*.rs" | grep -v test | wc -l` returns 0
- [ ] All unsafe blocks in ori_rt have SAFETY comments (use the Python verification script from Section 09.N -- output must be empty)
- [ ] Verify no new stale TODOs introduced: `grep -rn "// TODO" compiler/*/src/ --include="*.rs" | grep -v test | wc -l` is same or lower than before this plan started
- [ ] Delete this plan directory: `rm -rf plans/hygiene-full-2/`

- [ ] **Subsection close-out (10.1)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-10.1 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 10.1: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 10.R Third Party Review Findings

- None.

---

## 10.N Completion Checklist

- [ ] All verification checks pass
- [ ] `/improve-tooling` retrospective completed — MANDATORY at section close, even for cleanup/meta sections. Reflect on the entire hygiene-full-2 plan's debugging journey: which `diagnostics/` scripts and hygiene helpers you ran repeatedly, where the `plan-annotations.sh` output was hard to interpret, what manual file-size / function-size auditing you did that should be automated, what hygiene-rule violations the existing tooling missed. Cleanup sections are uniquely valuable for retrospective because they exercise the *full* tooling surface across the whole plan. Implement every accepted improvement NOW (zero deferral) and commit each via SEPARATE `/commit-push`. See `.claude/skills/improve-tooling/SKILL.md` "Retrospective Mode".
- [ ] Plan directory deleted
