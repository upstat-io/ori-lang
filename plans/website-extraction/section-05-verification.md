---
section: "05"
title: "Verification"
status: not-started
goal: "All website functionality works end-to-end from the new repo"
depends_on: ["04"]
sections:
  - id: "05.1"
    title: "Local Development"
    status: not-started
  - id: "05.2"
    title: "CI Deployment"
    status: not-started
  - id: "05.3"
    title: "Content Rendering"
    status: not-started
  - id: "05.4"
    title: "Broken Link Check"
    status: not-started
  - id: "05.5"
    title: "Completion Checklist"
    status: not-started
---

# Section 05: Verification

**Status:** Not Started
**Goal:** All website functionality works correctly from the new repo — local dev, CI builds, content rendering, playground, and deployment.

**Context:** This section is the final verification pass. Every page, every feature, every workflow must be tested to ensure nothing was lost in the migration.

**Depends on:** Section 04 (all migration work complete).

---

## 05.1 Local Development

- [ ] Clone both repos as siblings:
  ```
  /home/eric/projects/ori_lang/
  /home/eric/projects/ori-lang-website/
  ```
- [ ] Run `bun install` in ori-lang-website
- [ ] Run `bun run dev` — Astro dev server starts without errors
- [ ] WASM playground loads and runs Ori code
- [ ] Hot reload works for website source changes
- [ ] Content changes in ori_lang are reflected on dev server reload

---

## 05.2 CI Deployment

- [ ] Push to `main` on ori-lang-website triggers GitHub Actions build
- [ ] Build completes successfully (WASM, Astro, deploy)
- [ ] Content change in ori_lang triggers `repository_dispatch`
- [ ] Website rebuild from dispatch completes successfully
- [ ] `workflow_dispatch` manual trigger works

---

## 05.3 Content Rendering

Verify every content collection renders correctly:

- [ ] **Guide pages** (`/guide/...`) — all chapters load
- [ ] **Spec pages** (`/docs/spec/...`) — all clauses render, cross-references work
  - Specifically verify that `[clause-name](./XX-name.md)` links in spec docs resolve to `/docs/spec/XX-name` (exercises `remark-md-links.mjs` COLLECTION_MAPPINGS)
  - Verify `{{#include path}}` directives work (exercises `remark-include.mjs` relative path resolution)
- [ ] **Compiler design** (`/docs/compiler-design/...`) — all pages render
- [ ] **Formatter docs** (`/docs/formatter/...`) — all pages render
- [ ] **LSP docs** (`/docs/lsp/...`) — all pages render
- [ ] **Roadmap** (`/roadmap/...`) — all sections render, task counts correct
- [ ] **Plan sections** (`/roadmap/plans/...`) — reroute and parallel plans render
- [ ] **Proposals** (`/proposals/...`) — all proposals render
- [ ] **Blog** — all posts render
- [ ] **Code journeys** — journey cards and data render
- [ ] **Playground** (`/playground`) — Monaco editor loads, WASM executes Ori code
  - Verify Ori syntax highlighting in code blocks (exercises `ori.tmLanguage.json` Shiki grammar)
  - Verify EBNF syntax highlighting in spec code blocks (exercises `ebnf.tmLanguage.json` Shiki grammar)
  - Verify Monaco editor Ori syntax highlighting (exercises `ori-monarch.ts` and `ori-theme.ts`)
- [ ] **Tutorial** (`/tutorial/...`) — all lessons load with starter/solution code
- [ ] **Changelog** — generated from ori_lang git history
- [ ] **Test results** — badge/status displayed (or graceful fallback)
- [ ] **Install script** (`/install.sh`) — downloads correctly
- [ ] **Version display** — `softwareVersion` in BaseLayout.astro shows correct version (verify the chosen version sync mechanism from Section 03.3 works)

---

## 05.4 Broken Link Check

- [ ] Run a broken link checker against the built site (`bun run build` then serve `dist/` locally and run a link checker)
  - Options: `linkinator`, `broken-link-checker`, or `astro check` (if it has link validation)
  - Pay special attention to:
    - Cross-references between spec clauses
    - Links from roadmap/plan pages to source files
    - Proposal links
    - Guide chapter navigation

---

## 05.5 Completion Checklist

- [ ] Local `bun run dev` works with no errors
- [ ] Local `bun run build` produces complete `dist/`
- [ ] CI build succeeds on push to main
- [ ] CI build succeeds on repository_dispatch
- [ ] `ori-lang.com` serves all pages correctly
- [ ] HTTPS works on `ori-lang.com`
- [ ] Playground executes Ori code
- [ ] All content/feature items in 05.3 verified (guide, spec, compiler-design, formatter, lsp, roadmap, plans, proposals, blog, journeys, playground, tutorial, changelog, test results, install script, version display)
- [ ] Broken link check passes (05.4)
- [ ] `./test-all.sh` passes in ori_lang (no regressions)
- [ ] `sync-version.sh --check` passes in ori_lang (no references to removed website paths)

**Exit Criteria:** Every page on ori-lang.com renders correctly from the new repo, the playground works, CI deploys automatically on both website and content changes, and the ori_lang repo has no website code remaining.
