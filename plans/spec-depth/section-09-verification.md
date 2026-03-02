---
section: "09"
title: "Verification"
status: not-started
goal: "Verify spec internal consistency after all expansions"
depends_on: ["01", "02", "03", "04", "05", "06", "07", "08"]
sections:
  - id: "09.1"
    title: "Cross-Reference Audit"
    status: not-started
  - id: "09.2"
    title: "Terminology Consistency"
    status: not-started
  - id: "09.3"
    title: "Completeness Check"
    status: not-started
  - id: "09.4"
    title: "Completion Checklist"
    status: not-started
---

# Section 09: Verification

**Status:** Not Started
**Goal:** After expanding 8+ spec clauses, verify that the spec is internally consistent — no contradictions, no dangling cross-references, no terminology drift.

---

## 09.1 Cross-Reference Audit

- [ ] Every `See [...]` cross-reference in the spec resolves to an existing section
- [ ] No circular "see X" → "see Y" → "see X" chains without substance
- [ ] Grammar.ebnf references match actual section names
- [ ] Annex B operator rules match §14 expression semantics
- [ ] Annex C built-in signatures match §9 trait definitions

---

## 09.2 Terminology Consistency

- [ ] `shall` used consistently for requirements (not `must`)
- [ ] `NOTE` used for informative text (not `> **Note:**`)
- [ ] `EXAMPLE` used for examples (not `Valid:` / `Invalid:`)
- [ ] "compile-time error" vs "error" — used consistently
- [ ] "panic" vs "runtime error" — used consistently (should be "panic")
- [ ] Type names: consistent capitalization (`int` not `Int`, `Option` not `option`)

---

## 09.3 Completeness Check

- [ ] Every type in the predeclared identifiers list has its semantics defined somewhere
- [ ] Every operator in the precedence table has semantics in §14 or Annex B
- [ ] Every keyword has its usage defined in at least one clause
- [ ] Every built-in function in Annex C has signature + behavior
- [ ] Every panic condition is listed in the §23 panic catalogue
- [ ] Every error code referenced in the spec exists in the diagnostic system

---

## 09.4 Completion Checklist

- [ ] Cross-reference audit complete — 0 broken links
- [ ] Terminology audit complete — all normative keywords consistent
- [ ] Completeness check — no undefined terms or dangling references
- [ ] Grammar.ebnf synced with any new EBNF added in clause prose
- [ ] `ori-syntax.md` quick reference synced with spec changes

**Exit Criteria:** `grep -c "See \[" docs/ori_lang/v2026/spec/*.md` shows all cross-references; manual spot-check of 20 random references confirms they resolve correctly. No terminology violations found by searching for forbidden patterns (`must`, `> **Note:**`, `Valid:`).
