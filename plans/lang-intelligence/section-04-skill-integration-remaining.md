---
section: "04"
title: "Skill Integration: Remaining Skills (Tier 2+3)"
status: not-started
reviewed: false
goal: "Complete the Claude ecosystem integration by adding intelligence queries to design-pattern-review, create-draft-proposal, continue-roadmap, and review-bugs."
success_criteria:
  - "/design-pattern-review Agent B uses intelligence graph for prior art discovery"
  - "/create-draft-proposal populates Prior Art section from intelligence graph"
  - "/continue-roadmap queries intelligence after focus resolution for section-relevant context"
  - "/review-bugs cross-references bugs with similar issues in reference compilers"
  - "All skills use scripts/intel-query.sh — no open-coded Neo4j logic"
depends_on: ["01", "02"]
third_party_review:
  status: none
  updated: null
---

# 04 Skill Integration: Remaining Skills (Tier 2+3)

## 04.0 Goal

Complete the integration by wiring intelligence into the 4 remaining skills that benefit from cross-language context. Each skill has a natural insertion point identified by the research agents.

## 04.1 Design Pattern Review

**File**: `.claude/skills/design-pattern-review/SKILL.md`
**Insertion**: Agent B template (lines 131-139) — before reading reference repos.

- [ ] Add intelligence pre-query before Agent B's reference repo research
- [ ] Query: `scripts/intel-query.sh compare "<design topic>"` to identify which repos have the most relevant prior art
- [ ] Use results to prioritize which reference repos Agent B should read first (instead of scanning all 10)
- [ ] Intelligence narrows the search, Agent B still reads actual source code

### Subsection 04.1 close-out
**`/improve-tooling` retrospective**: Did intelligence successfully narrow Agent B's search? Any design domains that need custom presets?

---

## 04.2 Draft Proposal

**File**: `.claude/skills/create-draft-proposal/SKILL.md`
**Insertion**: Before Step 5 (File Creation), populating the `## Prior Art` section.

- [ ] Add intelligence query step before writing the proposal file
- [ ] Query: `scripts/intel-query.sh search "<proposed feature>" --limit 20` + `scripts/intel-query.sh compare "<feature concept>"`
- [ ] Populate `## Prior Art` section with structured results: which languages implemented this, what issues arose, what approaches were rejected
- [ ] Intelligence provides the discovery; the proposal author still verifies against actual implementations

### Subsection 04.2 close-out
**`/improve-tooling` retrospective**: Did intelligence help populate Prior Art sections? Any feature categories that need dedicated presets?

---

## 04.3 Continue Roadmap

**File**: `.claude/skills/continue-roadmap/SKILL.md`
**Insertion**: AFTER reroute resolution and scanner focus selection (per Codex finding #5 — querying before focus is resolved produces noise).

- [ ] Add intelligence query step after the scanner's Focus Selection block determines the active section
- [ ] Extract section topic and subsystem from the selected section's frontmatter
- [ ] Query: `scripts/intel-query.sh search "<section topic>"` + relevant preset
- [ ] Present results as "Cross-language context for this section" — what other compilers encountered when building this feature
- [ ] Skip if section topic doesn't map to a meaningful intelligence query (e.g., pure infrastructure sections)

### Subsection 04.3 close-out
**`/improve-tooling` retrospective**: Was the section-topic extraction accurate enough for useful queries? Any sections where intelligence was noise rather than signal?

---

## 04.4 Review Bugs

**File**: `.claude/commands/review-bugs.md`
**Insertion**: Step 2 (OBE Check, lines 45-72) and Step 5 (Recommendations, lines 138-145).

- [ ] At OBE check: query for whether similar bugs were resolved in reference compilers (signal that the approach is known)
- [ ] At recommendations: query for bug clustering patterns — "this class of bug appeared in 3+ compilers after feature X"
- [ ] Use results to prioritize bug fix order (bugs with known fixes in other compilers → higher confidence in fix approach)

### Subsection 04.4 close-out
**`/improve-tooling` retrospective**: Did cross-referencing bugs help with prioritization? Any bug categories where intelligence was particularly valuable?

---

## 04.R Third Party Review Findings

- None.

## Completion Checklist

- [ ] All 4 skills modified with intelligence integration
- [ ] All use `scripts/intel-query.sh` exclusively
- [ ] All degrade gracefully when intelligence unavailable
- [ ] No test regressions: `timeout 150 ./test-all.sh`
- [ ] `/tpr-review` clean
- [ ] `/impl-hygiene-review` clean
- [ ] `/improve-tooling` section-close sweep
