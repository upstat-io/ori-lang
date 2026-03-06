---
section: "06"
title: "Verification"
status: not-started
goal: "Verify all changes are consistent, accurate, and complete"
depends_on: ["01", "02", "03", "04", "05"]
sections:
  - id: "06.1"
    title: "Messaging Consistency Audit"
    status: not-started
  - id: "06.2"
    title: "Technical Accuracy Check"
    status: not-started
  - id: "06.3"
    title: "Build & Render Verification"
    status: not-started
  - id: "06.4"
    title: "Completion Checklist"
    status: not-started
---

# Section 06: Verification

**Status:** Not Started
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

- [ ] Each surface reviewed for messaging consistency
- [ ] No surface contradicts another
- [ ] No surface references "mandatory testing" as a hard requirement without opt-in context
- [ ] Tagline is identical across README and website

---

## 06.2 Technical Accuracy Check

- [ ] All code examples in README are valid Ori syntax
- [ ] The 8-layer pipeline claims match the actual ARC implementation
- [ ] The comparison tables (memory model, feature comparison) are factually accurate
- [ ] Capability example code (`with Http = mock in { ... }`) is correct syntax
- [ ] Testing example code uses correct `tests @target` syntax
- [ ] The `oripk.toml` config format for `test-enforcement` is valid TOML (note: config system does not exist yet; verify format only)
- [ ] Links to website, playground, spec, GitHub are all valid
- [ ] Spec error codes in clause 19 match actual compiler error codes (no E0500/E0501 — use new dedicated codes)
- [ ] E3001 collision resolved — MissingTest and TestTargetNotFound have their own codes
- [ ] Module docs link to correct spec clause (19-testing.md not 13-testing.md)

---

## 06.3 Build & Render Verification

- [ ] `cargo build` succeeds (compiler changes from Section 02)
- [ ] `./test-all.sh` green (no regressions)
- [ ] Website builds: `cd website && npm run build` succeeds
- [ ] README renders correctly on GitHub (check markdown rendering)
- [ ] Website renders correctly at all breakpoints (mobile, tablet, desktop)
- [ ] No layout issues from changed text lengths (especially hero title)

---

## 06.4 Completion Checklist

- [ ] Messaging consistency audit complete — all surfaces aligned
- [ ] Technical accuracy verified — all claims factual
- [ ] Builds pass — compiler, tests, website
- [ ] Visual review complete — no layout issues
- [ ] `grep -rn "mandatory test" README.md website/ docs/ CLAUDE.md .claude/ blog/ plans/ compiler/oric/src/commands/ compiler/oric/src/problem/` returns only historical/annotated references
- [ ] E3001 error code collision resolved (Section 02)
- [ ] OG image updated and renders correctly
- [ ] PR ready for review

**Exit Criteria:** All messaging surfaces (README, website, spec, docs, rules, guide, blog, design docs, compiler comments, OG image, layout defaults) tell the same story per the positioning decisions in Section 01. Testing is framed as smart and opt-in across all surfaces. All builds pass. All links work. All code examples valid. Error codes are properly separated (no E3001 collision).
