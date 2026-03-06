---
section: "05"
title: "Spec & Documentation Updates"
status: not-started
goal: "Update spec, CLAUDE.md, rules files, guide, blog, design docs, proposals, skills, compiler comments, and diagnostic docs to reflect configurable test enforcement and new messaging"
depends_on: ["01", "02"]
sections:
  - id: "05.1"
    title: "Spec Clause 19 (Testing)"
    status: not-started
  - id: "05.2"
    title: "CLAUDE.md Updates"
    status: not-started
  - id: "05.3"
    title: "Rules Files"
    status: not-started
  - id: "05.4"
    title: "Additional Surfaces Requiring Updates"
    status: not-started
  - id: "05.5"
    title: "Completion Checklist"
    status: not-started
---

# Section 05: Spec & Documentation Updates

**Status:** Not Started
**Goal:** Update all documentation surfaces — spec, CLAUDE.md, rules files, guide, blog, design docs, proposals, skills, compiler comments, and diagnostic docs — to reflect configurable test enforcement and the new messaging.

**Context:** 20+ files across the repository reference "mandatory testing" as a hard requirement. These need to be updated to describe the configurable policy and align with the new positioning.

**Depends on:** Section 01 (Positioning Strategy) — new messaging must be finalized for guide/blog/design docs. Section 02 (Testing Policy) — the exact config format and default behavior must be finalized for spec/compiler changes.

---

## 05.1 Spec Clause 19 (Testing)

**File:** `docs/ori_lang/v2026/spec/19-testing.md`

The spec uses normative language (`shall`) for test requirements. This needs to change from:
- "Every function **shall** have at least one attached test" (hard requirement, §19.2)

To:
- "A conforming implementation **shall** support test enforcement at three levels..." (capability)
- "When test enforcement is set to `error`, every function declaration **shall** have..." (conditional)

### Changes

- [ ] Read current spec clause 19
- [ ] Change "shall have tests" to "shall support configurable test enforcement"
- [ ] Document the three enforcement levels (off/warn/error)
- [ ] Document the default behavior
- [ ] Fix spec error code examples: replace E0500/E0501 (not in compiler's error code registry) with the new dedicated codes assigned in Section 02
- [ ] Preserve all other testing semantics (dep-graph, `tests @target`, floating tests)

---

## 05.2 CLAUDE.md Updates

**Files:** `/home/eric/projects/ori_lang/CLAUDE.md`

### Current References to Mandatory Testing

```
Line 28: "Mandatory verification: functions need tests; contracts (pre()/post())"
```

```
Line 105: "Every function (except `@main`) requires tests"
```

### Changes

- [ ] Line 28: Change "Mandatory verification: functions need tests" to "Configurable verification: functions can require tests" (or similar per Section 01 tone)
- [ ] Line 28: Update design pillar description to describe opt-in enforcement
- [ ] Line 105: Change "Every function (except `@main`) requires tests" to describe configurable enforcement with `test-enforcement` setting

---

## 05.3 Rules Files

**Files:**
- `.claude/rules/ori-syntax.md` — contains test *syntax* examples (`@t tests @fn`) but NOT the "requires tests" mandate
- `.claude/rules/ori-lang.md` — does NOT reference mandatory testing (confirmed)

### Changes

- [ ] Review ori-syntax.md test syntax section (currently just syntax, no mandate — likely no changes needed)
- [ ] Verify all rules files are internally consistent with updated CLAUDE.md

---

## 05.4 Additional Surfaces Requiring Updates

The following files also reference "mandatory testing" and must be updated for messaging consistency. These were not captured in sections 05.1–05.3.

### Guide (`docs/guide/`)

- **`docs/guide/01-getting-started.md`** (lines 237, 285):
  - `"This is Ori's **mandatory testing** at work."` — reframe as "smart testing"
  - `"### Why Mandatory Testing?"` heading — rename
- **`docs/guide/03-functions.md`** (line 459): `"Every function needs at least one test:"` — add opt-in context
- **`docs/guide/12-testing.md`** (line 3, 10):
  - YAML description: `"Mandatory testing, assertions, mocking..."` — reframe
  - `"Testing isn't optional in Ori"` — reframe as configurable

- [ ] Update `docs/guide/01-getting-started.md` — reframe mandatory testing references
- [ ] Update `docs/guide/03-functions.md` — add enforcement configurability context
- [ ] Update `docs/guide/12-testing.md` — reframe description and intro

### Blog (`blog/`)

- **`blog/building-ori-from-scratch.md`** (lines 103, 105):
  - `"### 1. Tests Are Mandatory"`
  - `"Every function (except @main) must have at least one test or the program doesn't compile. Period."`

- [ ] Update `blog/building-ori-from-scratch.md` — reframe to configurable enforcement

### Design Docs (`docs/compiler/design/`)

- **`docs/compiler/design/14-testing/index.md`** — heavily references mandatory testing throughout (lines 41, 43, 47, 49, 51, 264, 266, 284, 290, 296, 315, 335, 341)
  - `"### Why Mandatory Testing"` heading
  - `"## The Mandatory Testing Philosophy"` heading
  - Entire philosophy section assumes non-configurable enforcement
- **`docs/compiler/design/14-testing/test-discovery.md`** (lines 36, 299)
- **`docs/compiler/design/14-testing/test-runner.md`** (line 500)
- **`docs/compiler/design/01-architecture/index.md`** (line 229): `"Tests are not optional in Ori."`
- **`docs/compiler/design/15-platform-targets/conditional-compilation.md`** (line 69)

- [ ] Update all `docs/compiler/design/14-testing/` files — reframe philosophy as configurable
- [ ] Update `docs/compiler/design/01-architecture/index.md` — reframe test requirement
- [ ] Update `docs/compiler/design/15-platform-targets/conditional-compilation.md` — minor reference

### Proposals (`docs/ori_lang/proposals/`)

- **`docs/ori_lang/proposals/approved/dependency-aware-testing-proposal.md`** (lines 409, 411)
- **`docs/ori_lang/proposals/approved/test-execution-model-proposal.md`** (line 511)
- **`docs/ori_lang/proposals/approved/checks-proposal.md`** (lines 451, 543)
- **`docs/ori_lang/proposals/drafts/test-driven-pgo-proposal.md`** (lines 11, 50, 161)

NOTE: Proposals are historical records. Consider adding a note at the top rather than rewriting content. E.g.: `> NOTE: This proposal predates configurable test enforcement. References to "mandatory testing" should be read as "test-enforcement = error".`

- [ ] Decide: rewrite proposal text or add historical note
- [ ] Update or annotate affected proposals

### Archived Design Docs (`docs/ori_lang/v2026/archived-design/`)

These are under `archived-design/` and may be considered historical. Key files:
- **`00-index.md`** (lines 3, 90, 116, 131)
- **`01-philosophy/01-ai-first-design.md`** (lines 164, 192, 417)
- **`11-testing/01-mandatory-tests.md`** — entire document about mandatory testing
- **`11-testing/index.md`**, **`02-test-syntax.md`**, **`03-compile-fail-tests.md`** — cross-references
- **`14-capabilities/index.md`**, **`03-testing-effectful-code.md`** — references

NOTE: These are archived. Add a note at top of each linking to the updated spec rather than rewriting.

- [ ] Decide: annotate archived docs or leave as historical
- [ ] If annotating: add note to `11-testing/01-mandatory-tests.md` pointing to updated spec

### Module Docs (`docs/ori_lang/v2026/modules/`)

- **`std.testing/index.md`** (line 370): broken link to `../../spec/13-testing.md` (should be `19-testing.md`)

- [ ] Fix broken spec link in `std.testing/index.md`

### Skills (`.claude/skills/`)

- **`.claude/skills/design-pattern-review/SKILL.md`** (line 188): `"Mandatory tests for all functions"`

- [ ] Change "Mandatory tests for all functions" (line 188) to "Configurable test enforcement (off/warn/error)" in `.claude/skills/design-pattern-review/SKILL.md`

### Website Assets and Layout

> **Note:** `website/public/og-image.svg` and `website/src/layouts/BaseLayout.astro` are
> covered in **Section 04** (subsections 04.5 and 04.6). They are listed here for
> completeness of the "mandatory testing" surface inventory but should be implemented
> as part of Section 04, not duplicated.

### Roadmap (`plans/roadmap/`)

- **`plans/roadmap/00-overview.md`** (line 11): `"Mandatory tests bound to functions"` — reframe
- **`plans/roadmap/section-14-testing.md`** (lines 6, 53, 71, 73, 74): goal and task descriptions reference "mandatory testing" — reframe as "configurable testing enforcement"
- **`plans/roadmap/section-22-tooling.md`** (line 218): references "mandatory testing" in context of testing framework

- [ ] Update `plans/roadmap/00-overview.md` — reframe "Mandatory tests" reference
- [ ] Update `plans/roadmap/section-14-testing.md` — reframe goal and task descriptions
- [ ] Update `plans/roadmap/section-22-tooling.md` — minor reference update

### Compiler Source Comments

- **`compiler/oric/src/commands/check.rs`** (lines 11, 53): doc comments reference mandatory coverage
- **`compiler/oric/src/problem/semantic/mod.rs`** (line 336): note says `"every function requires at least one test"`

- [ ] Update doc comments in `check.rs` (lines 11, 53) to describe configurable enforcement instead of mandatory coverage
- [ ] Update `SemanticProblem::MissingTest` note text in `semantic/mod.rs` (line 336) from "every function requires at least one test" to mention `--test-enforcement` configurability

### Diagnostic Docs

- **`docs/compiler/design/13-diagnostics/problem-types.md`** (lines 204, 220, 232, 243): describes `MissingTest` as "active in production" and shows `check_test_coverage()` — needs update to reference configurable enforcement

- [ ] Update `docs/compiler/design/13-diagnostics/problem-types.md` (lines 204, 220, 232, 243) to describe `MissingTest` as severity-configurable, not always-error

---

## 05.5 Completion Checklist

- [ ] Spec clause 19 updated with configurable enforcement
- [ ] Spec error code examples fixed (E0500/E0501 replaced with actual code)
- [ ] CLAUDE.md updated (both line 28 design pillar and line 105 files & tests section)
- [ ] ori-syntax.md reviewed (currently no mandate text — may need no changes)
- [ ] Guide files updated (`docs/guide/01-getting-started.md`, `03-functions.md`, `12-testing.md`)
- [ ] Blog file updated or annotated (`blog/building-ori-from-scratch.md`)
- [ ] Design docs updated (`docs/compiler/design/14-testing/`, `01-architecture/`)
- [ ] Proposals annotated with historical note
- [ ] Archived design docs annotated or left with note
- [ ] Module docs broken link fixed (`std.testing/index.md`)
- [ ] Skills file updated (`.claude/skills/design-pattern-review/SKILL.md`)
- [ ] OG image SVG updated (covered by **Section 04.6** — verify here, don't duplicate work)
- [ ] BaseLayout.astro defaults updated (covered by **Section 04.5** — verify here, don't duplicate work)
- [ ] Compiler source comments updated
- [ ] Diagnostic docs updated (`docs/compiler/design/13-diagnostics/problem-types.md`)
- [ ] Roadmap files updated (`plans/roadmap/00-overview.md`, `section-14-testing.md`, `section-22-tooling.md`)
- [ ] No file in the repo describes testing as unconditionally mandatory
- [ ] `grep -rn "mandatory test" docs/ .claude/ CLAUDE.md README.md website/ blog/ plans/` returns 0 results (or results are in clearly-marked historical context only)

**Exit Criteria:** All normative documentation describes test enforcement as configurable. The spec defines the three levels and their semantics. No documentation file implies that tests are always required without opt-in. All website surfaces (including OG image, layout defaults, structured data) aligned.
