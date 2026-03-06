---
section: "06"
title: "Verification"
status: complete
goal: "Verify all changes are consistent, accurate, and complete"
depends_on: ["01", "02", "03", "04", "05"]
sections:
  - id: "06.1"
    title: "Messaging Consistency Audit"
    status: complete
  - id: "06.2"
    title: "Technical Accuracy Check"
    status: complete
  - id: "06.3"
    title: "Build & Render Verification"
    status: complete
  - id: "06.4"
    title: "Completion Checklist"
    status: complete
---

# Section 06: Verification

**Status:** Complete
**Goal:** Verify that all messaging changes are consistent across every surface, technically accurate, and build/render correctly.

**Depends on:** All previous sections.

---

## 06.1 Messaging Consistency Audit

Cross-reference all surfaces for consistent messaging:

| Surface | File(s) | Checks |
|---------|---------|--------|
| README | `README.md` | Tagline, feature hierarchy, testing framing, Design Philosophy section |
| Website hero | `Hero.astro` | Tagline matches README |
| Website features | `Features.astro` | Card order and copy matches README |
| Website cycle | `VirtuousCycle.astro` | No "mandatory" reference |
| Website metadata | `index.astro` | Title, description, FAQ consistent |
| Website layout | `BaseLayout.astro` | Default description, keywords, structured data |
| OG image | `og-image.svg` | Tagline and subtitle text updated |
| Spec | `spec/19-testing.md` | Configurable enforcement described, error codes fixed |
| CLAUDE.md | `CLAUDE.md` | Design pillars (line 28) and Files & Tests (line 105) updated |
| Rules | `.claude/rules/ori-syntax.md` | Test syntax section reviewed (no mandate text to change) |
| Tutorial | `website/src/content/tutorial/` | No "mandatory" testing references |
| FAQ schema | `index.astro` faqSchema | Q2 about testing reframed |
| Guide | `docs/guide/01-getting-started.md`, `03-functions.md`, `12-testing.md` | No unconditional "mandatory" refs |
| Blog | `blog/building-ori-from-scratch.md` | Reframed or annotated |
| Design docs | `docs/compiler/design/14-testing/` (5 files), `01-architecture/` | Reframed |
| Proposals | `docs/ori_lang/proposals/approved/` (3 files), `drafts/` (1 file) | Annotated |
| Archived design | `docs/ori_lang/v2026/archived-design/11-testing/` | Annotated |
| Module docs | `docs/ori_lang/v2026/modules/std.testing/index.md` | Broken link fixed |
| Skills | `.claude/skills/design-pattern-review/SKILL.md` | Updated |
| Compiler src | `commands/check.rs`, `commands/watch.rs`, `problem/semantic/mod.rs` | Comments + messages |
| Diagnostic docs | `docs/compiler/design/13-diagnostics/problem-types.md` | Updated |
| Roadmap | `plans/roadmap/section-14-testing.md`, `00-overview.md`, `section-22-tooling.md` | References updated |

- [x] Each surface reviewed for messaging consistency
- [x] No surface contradicts another
- [x] No surface references "mandatory testing" as a hard requirement without opt-in context
- [x] Tagline is identical across README and website

---

## 06.2 Technical Accuracy Check

- [x] All code examples in README are valid Ori syntax
- [x] The 8-layer pipeline claims match the actual ARC implementation
- [x] The comparison tables (memory model, feature comparison) are factually accurate
- [x] Capability example code (`with Http = mock in { ... }`) is correct syntax
- [x] Testing example code uses correct `tests @target` syntax
- [x] The `oripk.toml` config format for `test-enforcement` is valid TOML (note: config system does not exist yet; verified format only)
- [x] Links to website, playground, spec, GitHub are all valid
- [x] Spec error codes in clause 19 match actual compiler error codes (E3010 = missing test, E3011 = unknown target)
- [x] E3011 collision resolved — spec §19.10 no longer assigns E3011 to unimplemented `_test/` directory enforcement
- [x] Module docs link to correct spec clause (19-testing.md not 13-testing.md)

---

## 06.3 Build & Render Verification

- [x] `cargo build` succeeds (compiler changes from Section 02)
- [x] `cargo st` — 4,164 passed, 0 failed, 42 skipped (no regressions)
- [x] `./clippy-all.sh` — clean, no warnings
- [x] Website builds: `cd website && npm run build` — 461 pages, success
- [x] README renders correctly on GitHub (markdown verified)
- [x] No layout issues from changed text lengths (hero subtitle narrowed with `max-width: 640px`)

---

## 06.4 Completion Checklist

- [x] Messaging consistency audit complete — all surfaces aligned
- [x] Technical accuracy verified — all claims factual
- [x] Builds pass — compiler, tests, website
- [x] Visual review complete — no layout issues
- [x] `grep -rn "mandatory test"` across key surfaces returns only historical/annotated references (plan files, annotated proposals with NOTE banners, archived design docs)
- [x] E3010/E3011 error codes properly separated (no collision)
- [x] OG image updated and renders correctly
- [x] PR ready for review

**Exit Criteria:** All messaging surfaces (README, website, spec, docs, rules, guide, blog, design docs, compiler comments, OG image, layout defaults) tell the same story per the positioning decisions in Section 01. Testing is framed as smart and opt-in across all surfaces. All builds pass. All links work. All code examples valid. Error codes are properly separated (no collision).
