---
section: "07"
title: "Scratchpad Triage & Migration"
status: not-started
reviewed: false
goal: "Audit all 30 files in `ori_lang/scratchpad/` (live count as of plan execution), migrate 12 canonical design documents to their proper `docs/` homes (7 to `docs/ori_lang/v2026/design/` with Astro `docsSchema`, 3 to `docs/compiler/design/` with `docsSchema`, 1 to `docs/tooling/` with `docsSchema`, 1 to `docs/ori_lang/proposals/drafts/` with **body-format proposal schema** parsed by `ori-lang-website/src/loaders/proposal-loader.ts` — H1 title + `**Author:** ...` / `**Created:** ...` body keys + `## Summary` paragraph, NOT YAML frontmatter), delete 18 superseded or throwaway files, then remove the `scratchpad/` directory entirely so 704K of gitignored drift goes away."
success_criteria:
  - "7 files migrated to `docs/ori_lang/v2026/design/` with Astro `docsSchema` frontmatter (language philosophy, syntax principles, let-mut rationale, block-expression design, etc.)"
  - "3 files migrated to `docs/compiler/design/` with Astro `docsSchema` frontmatter (aot-test-gaps, lexer-architecture, error-handling-ux)"
  - "1 file migrated to `docs/tooling/` with Astro `docsSchema` frontmatter (ori-fmt-code-review plan)"
  - "1 file migrated to `docs/ori_lang/proposals/drafts/` with **body-format proposal schema** (H1 title + `**Author:**` / `**Created:**` body keys + `## Summary` paragraph, matching `ori-lang-website/src/loaders/proposal-loader.ts:22-59`) — package-system iteration-6 only; iterations 1-5 deleted as superseded"
  - "18 files deleted from scratchpad/ (includes `07-modern-lang-repos.md` per the conditional DELETE decision in §07.1)"
  - "`scratchpad/` directory removed (either via rmdir or via git clean — it's gitignored so no commit is needed for the deletion itself; the migrations are what commit)"
  - "`ori-lang-website` Astro build succeeds with all new `docs/*` collection entries — `content.config.ts` docsSchema validation passes for every migrated file (title + order at minimum)"
  - "`./test-all.sh` green post-migration (no test exercises scratchpad paths)"
  - "Each of the 14 migrations has a commit: `docs(migration): move scratchpad/{old-path} to {new-path}` OR a batched commit per destination directory (`docs(design): migrate 7 design notes from scratchpad/ to docs/ori_lang/v2026/design/`)"
inspired_by:
  - "rustc `rust/dev-guide` migration — scattered design notes collected into a canonical design doc home with consistent frontmatter"
  - "Roc `design/` directory — explicit separation of design history from spec and plans"
depends_on: ["06"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "07.1"
    title: "Pre-Migration Re-verification"
    status: not-started
  - id: "07.2"
    title: "Migrate to docs/ori_lang/v2026/design/ (7 files)"
    status: not-started
  - id: "07.3"
    title: "Migrate to docs/compiler/design/ (3 files)"
    status: not-started
  - id: "07.4"
    title: "Migrate to docs/tooling/ (1 file)"
    status: not-started
  - id: "07.5"
    title: "Migrate to docs/ori_lang/proposals/drafts/ (1 file)"
    status: not-started
  - id: "07.6"
    title: "Delete Superseded & Throwaway Files (17 files)"
    status: not-started
  - id: "07.7"
    title: "Remove scratchpad/ Directory & Verify Website Build"
    status: not-started
  - id: "07.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "07.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 07: Scratchpad Triage & Migration

**Status:** Not Started
**Goal:** Pass 1 Agent 2 did the per-file triage of `ori_lang/scratchpad/` and produced destination recommendations. Fresh `find scratchpad -type f` at §01.1 baseline time confirmed **30 files** (not 31 — the pre-execution count in the original census had a one-off error). This section EXECUTES the recommendations against the 30-file live state: **12 files migrate** to their appropriate `docs/` homes (11 with Astro `docsSchema` frontmatter; 1 to `docs/ori_lang/proposals/drafts/` with body-format proposal schema), **18 files are deleted** (duplicate existing docs, superseded by later iterations, throwaway experiments, or — for `07-modern-lang-repos.md` — conditional delete per Agent 2's recommendation which §07.1 confirms), and `scratchpad/` as a directory is removed. Because several `docs/` subdirectories are **cross-repo globs consumed by `ori-lang-website/src/content.config.ts`**, every migration must use the correct schema for its destination — `docsSchema` (title + order) for `guide`, `spec`, `compiler-design`, `formatter`, `lsp` collections, and **body-format schema parsed by `proposal-loader.ts`** for the `proposals` collection.

**Success Criteria:**

- [ ] 14 files exist in their new `docs/` homes with proper Astro frontmatter
- [ ] 17+ files deleted from `scratchpad/`
- [ ] `scratchpad/` directory absent (or empty + rmdir'd)
- [ ] `ori-lang-website` Astro build succeeds with no schema errors on any docs collection
- [ ] `./test-all.sh` green post-migration
- [ ] Each migrated file traces back to its original `scratchpad/` path in the commit message (provenance preservation)
- [ ] Satisfies mission criterion: "**Scratchpad is empty or absent**"

**Context:** `scratchpad/` is 704K of gitignored design notes accumulated over time. Most files are either:
- **Superseded** by later formal work (e.g., `aot-llvm-findings.md` is already captured in `plans/aot_codegen_pipeline/`)
- **Duplicated** by `.claude/rules/*.md` files (e.g., general parser/type-system/interpreter/codegen reference notes)
- **Throwaway experiments** (e.g., `dice_game.ori`, `snake.ori` in `scratchpad/experiments/`)
- **Not-yet-formalized** but valuable design history (e.g., `14-let-mut-evaluation.md`, `15-block-expression-syntax.md`, `01-language-design-principles.md`)

Pass 1 Agent 2 produced the per-file recommendations. §07 executes them.

**Critical schema constraint:** The Astro content-collection loader in `ori-lang-website/src/content.config.ts` requires `docsSchema` compliance for several docs subtrees:
```ts
const docsSchema = z.object({
  title: z.string(),
  description: z.string().optional(),
  order: z.number(),
  section: z.string().optional(),
  sidebar_title: z.string().optional(),
  sidebar_order: z.number().optional(),
  sidebar_path: z.string().optional(),
});
```
Collections using `docsSchema` (per Pass 2 read of `content.config.ts`):
- `guide` (base: `../ori_lang/docs/guide`)
- `spec` (base: `../ori_lang/docs/ori_lang/v2026/spec`)
- `compilerDesign` (base: `../ori_lang/docs/compiler/design`) — **§07.3 destination**
- `formatter` (base: `../ori_lang/docs/tooling/formatter/design`)
- `lsp` (base: `../ori_lang/docs/tooling/lsp/design`)
- (proposals has a different schema per `proposalLoader`)

The `docs/ori_lang/v2026/design/` destination (§07.2) is NOT currently in `content.config.ts` — verify this during §07.1 pre-migration check. If it IS a cross-repo glob, schema compliance is required. If it's not (purely internal design history), frontmatter is optional but still good practice for consistency.

**Reference implementations:**
- **rustc** (`rust/dev-guide`): design notes live in a canonical `dev-guide/` with mdBook frontmatter. Historical notes that predate the canonical home were migrated with frontmatter added during the dev-guide split.
- **Roc** (`roc/design/`): explicit separation of `design/` (long-term architectural thinking) from `plans/` (current work) from `langref/` (language reference). Roc's design files all have consistent frontmatter.

**Depends on:** §06 (the `docs/tooling/lsp/design/` rewrite must have already happened — otherwise the scratchpad-migrated files in that area might collide with the rewrite).

---

## 07.1 Pre-Migration Re-verification

**File(s):** No edits — verification only

Before any migration runs, re-verify the Pass 1 triage classifications against the current state of `scratchpad/`. Files may have been added or removed between Pass 1 and plan execution; the triage recommendations need to be refreshed.

- [ ] List the current state of scratchpad/:
  ```bash
  cd /home/eric/projects/ori_lang
  find scratchpad -type f | sort > plans/project-reorganization/baseline/scratchpad-inventory-before.txt
  wc -l plans/project-reorganization/baseline/scratchpad-inventory-before.txt
  ```
  - [ ] **30 files expected** (fresh count at §01.1 baseline, superseding the original 31-file claim). If the count differs from 30: investigate drift — either a new scratchpad file was added since baseline (add it to the classification) or one was removed (cross off the affected entry).

- [ ] Compare to Agent 2's recommendations. Files expected per the overview's scratchpad table:
  - `scratchpad/aot-llvm-findings.md` → DELETE (duplicates plans/aot_codegen_pipeline/)
  - `scratchpad/aot-test-gaps.md` → `docs/compiler/design/aot-test-gaps.md`
  - `scratchpad/formatter-architecture.md` → DELETE (superseded)
  - `scratchpad/ori_fmt-code-review.md` → `docs/tooling/ori-fmt-code-review.md`
  - `scratchpad/spec-to-architecture-mapping.md` → DELETE (spec-captured)
  - `scratchpad/design-ideas/01-language-design-principles.md` → `docs/ori_lang/v2026/design/language-design-principles.md`
  - `scratchpad/design-ideas/02-lexer-patterns.md` → `docs/compiler/design/lexer-architecture.md`
  - `scratchpad/design-ideas/03-parser-patterns.md` → DELETE (general ref, covered by .claude/rules/parse.md)
  - `scratchpad/design-ideas/04-type-system-design.md` → DELETE (general ref, covered by spec + rules)
  - `scratchpad/design-ideas/05-interpreter-patterns.md` → DELETE (general ref, covered by .claude/rules/eval.md)
  - `scratchpad/design-ideas/06-codegen-patterns.md` → DELETE (general ref, covered by .claude/rules/aot.md)
  - `scratchpad/design-ideas/07-modern-lang-repos.md` → DELETE (conditional — delete unless actively referenced)
  - `scratchpad/design-ideas/08-error-handling-ux.md` → `docs/compiler/design/error-handling-and-diagnostics-design.md`
  - `scratchpad/design-ideas/09-testing-tooling.md` → DELETE (duplicates .claude/rules/tests.md)
  - `scratchpad/design-ideas/10-syntax-design-principles.md` → `docs/ori_lang/v2026/design/syntax-design-principles.md`
  - `scratchpad/design-ideas/11-sigil-syntax-evaluation.md` → `docs/ori_lang/v2026/design/syntax-design-review.md`
  - `scratchpad/design-ideas/12-language-beauty.md` → `docs/ori_lang/v2026/design/language-beauty-philosophy.md`
  - `scratchpad/design-ideas/13-syntax-improvements.md` → `docs/ori_lang/v2026/design/syntax-improvements-decisions.md`
  - `scratchpad/design-ideas/14-let-mut-evaluation.md` → `docs/ori_lang/v2026/design/let-mut-design-rationale.md`
  - `scratchpad/design-ideas/15-block-expression-syntax.md` → `docs/ori_lang/v2026/design/block-expression-syntax-design.md`
  - `scratchpad/experiments/dice_game.ori` → DELETE
  - `scratchpad/experiments/expr_eval.ori` → DELETE
  - `scratchpad/experiments/game_of_life.ori` → DELETE
  - `scratchpad/experiments/snake.ori` → DELETE
  - `scratchpad/package-system/iteration-1.md` → DELETE (superseded)
  - `scratchpad/package-system/iteration-2.md` → DELETE (superseded)
  - `scratchpad/package-system/iteration-3.md` → DELETE (superseded)
  - `scratchpad/package-system/iteration-4.md` → DELETE (superseded)
  - `scratchpad/package-system/iteration-5.md` → DELETE (superseded)
  - `scratchpad/package-system/iteration-6.md` → `docs/ori_lang/proposals/drafts/package-system-proposal.md`
  - [ ] Compare this list against `scratchpad-inventory-before.txt`. Every actual file must appear in the classification above. Any missing → STOP and reconcile. Any extra → new file, classify and add to the appropriate subsection.

- [ ] Verify whether `docs/ori_lang/v2026/design/` is a cross-repo glob target in `content.config.ts`:
  ```bash
  grep -n "v2026/design" /home/eric/projects/ori-lang-website/src/content.config.ts
  ```
  - [ ] If matches appear: schema compliance (title + order) is REQUIRED for §07.2 migrations
  - [ ] If no matches: the directory is purely local; frontmatter is optional but still good practice. For consistency, add it anyway.

- [ ] Verify `docs/compiler/design/`, `docs/tooling/`, and `docs/ori_lang/proposals/drafts/` target directories exist (they should, from previous plan work):
  ```bash
  ls -d docs/ori_lang/v2026/design/ docs/compiler/design/ docs/tooling/ docs/ori_lang/proposals/drafts/ 2>&1
  ```
  - [ ] Each path either exists or needs to be created by this section's migrations

- [ ] **Subsection close-out (07.1)** — MANDATORY before starting 07.2:
  - [ ] Scratchpad inventory captured (file list baseline)
  - [ ] All 31 files classified (reconciled with Agent 2's recommendations)
  - [ ] `content.config.ts` cross-repo glob coverage verified for each destination
  - [ ] Target directories identified (existing or to-be-created)
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — reflect
        on the file-triage-reconciliation process. Was there friction in
        mapping Agent 2's per-file recommendations to the current filesystem
        state? Is there a `scripts/dev/triage-diff.sh <baseline-list>
        <current-list>` helper that shows added/removed/renamed files since
        a baseline? Broadly useful for any audit-and-migrate operation.
        If yes, add and commit via `build(scripts): add triage-diff.sh —
        surfaced by project-reorganization/section-07.1 retrospective`.

---

## 07.2 Migrate to docs/ori_lang/v2026/design/ (7 files)

**File(s):** 7 new files in `docs/ori_lang/v2026/design/`, 7 deletions in `scratchpad/design-ideas/`

The 7 files in this subsection are Ori-specific language design philosophy and syntax decisions. They're already well-structured; the migration just adds Astro frontmatter and relocates them.

**Frontmatter pattern** (use this shape for all 7 files):
```markdown
---
title: "Language Design Principles"
order: 1
description: "Core philosophy driving Ori syntax and semantics."
---

(existing content unchanged)
```
The `order` value determines sidebar ordering in the website. Assign sequential values 1-7 based on the logical reading order (philosophy → principles → individual decisions → history).

- [ ] Ensure target directory exists:
  ```bash
  mkdir -p docs/ori_lang/v2026/design
  ```

- [ ] **Migration 1 of 7: language-design-principles.md**
  - [ ] Read source: `scratchpad/design-ideas/01-language-design-principles.md`
  - [ ] Prepend Astro frontmatter with `title: "Language Design Principles"`, `order: 1`, `description: "Core philosophy driving Ori's syntax and semantic choices."`
  - [ ] Write to `docs/ori_lang/v2026/design/language-design-principles.md` using `Write` tool
  - [ ] Delete source: `git rm scratchpad/design-ideas/01-language-design-principles.md` (wait — scratchpad is gitignored, so `git rm` fails. Use `rm` directly.)
  - [ ] Verify: `rm scratchpad/design-ideas/01-language-design-principles.md` succeeds

- [ ] **Migration 2 of 7: syntax-design-principles.md**
  - [ ] Read source: `scratchpad/design-ideas/10-syntax-design-principles.md`
  - [ ] Prepend frontmatter: title "Syntax Design Principles", order 2, description "Applied principles for Ori's syntax (parser-friendliness, consistency, explicitness)."
  - [ ] Write to `docs/ori_lang/v2026/design/syntax-design-principles.md`
  - [ ] `rm` source

- [ ] **Migration 3 of 7: syntax-design-review.md**
  - [ ] Read source: `scratchpad/design-ideas/11-sigil-syntax-evaluation.md`
  - [ ] Frontmatter: title "Syntax Design Review", order 3, description "Self-assessment of Ori's syntax against its own design principles."
  - [ ] Write to `docs/ori_lang/v2026/design/syntax-design-review.md`
  - [ ] `rm` source

- [ ] **Migration 4 of 7: language-beauty-philosophy.md**
  - [ ] Read source: `scratchpad/design-ideas/12-language-beauty.md`
  - [ ] Frontmatter: title "Language Beauty Philosophy", order 4, description "Aesthetic principles: simplicity, clarity, consistency — the values behind Ori's visual design."
  - [ ] Write to `docs/ori_lang/v2026/design/language-beauty-philosophy.md`
  - [ ] `rm` source

- [ ] **Migration 5 of 7: syntax-improvements-decisions.md**
  - [ ] Read source: `scratchpad/design-ideas/13-syntax-improvements.md`
  - [ ] Frontmatter: title "Syntax Improvement Decisions", order 5, description "Specific syntax decisions with rationale and alternatives considered."
  - [ ] Write to `docs/ori_lang/v2026/design/syntax-improvements-decisions.md`
  - [ ] `rm` source

- [ ] **Migration 6 of 7: let-mut-design-rationale.md**
  - [ ] Read source: `scratchpad/design-ideas/14-let-mut-evaluation.md`
  - [ ] Frontmatter: title "let / let mut Design Rationale", order 6, description "Why Ori adopted let / let mut as its binding syntax — Rust compatibility, explicitness, AI codegen."
  - [ ] Write to `docs/ori_lang/v2026/design/let-mut-design-rationale.md`
  - [ ] `rm` source

- [ ] **Migration 7 of 7: block-expression-syntax-design.md**
  - [ ] Read source: `scratchpad/design-ideas/15-block-expression-syntax.md`
  - [ ] Frontmatter: title "Block Expression Syntax Design", order 7, description "Replacing run() with { } block syntax: problem, decision, implementation impact."
  - [ ] Write to `docs/ori_lang/v2026/design/block-expression-syntax-design.md`
  - [ ] `rm` source

- [ ] Verify all 7 files exist in target with frontmatter:
  ```bash
  ls docs/ori_lang/v2026/design/
  # Expected: 7 files
  for f in docs/ori_lang/v2026/design/*.md; do
    echo "=== $f ==="
    head -5 "$f"
  done
  ```

- [ ] Commit this subsection's migrations as a batch:
  ```bash
  git add docs/ori_lang/v2026/design/
  git commit -m "docs(design): migrate 7 design notes from scratchpad to docs/ori_lang/v2026/design/

Sources (all from scratchpad/design-ideas/, now gone):
- 01-language-design-principles.md → language-design-principles.md
- 10-syntax-design-principles.md → syntax-design-principles.md
- 11-sigil-syntax-evaluation.md → syntax-design-review.md
- 12-language-beauty.md → language-beauty-philosophy.md
- 13-syntax-improvements.md → syntax-improvements-decisions.md
- 14-let-mut-evaluation.md → let-mut-design-rationale.md
- 15-block-expression-syntax.md → block-expression-syntax-design.md

Astro docsSchema frontmatter (title + order + description) added to each.
Ordering 1-7 matches reading flow (philosophy → principles → decisions).

Refs: plans/project-reorganization/section-07-scratchpad-triage.md"
  ```

- [ ] **Subsection close-out (07.2)** — MANDATORY before starting 07.3:
  - [ ] All 7 files in target with frontmatter
  - [ ] All 7 sources removed from scratchpad
  - [ ] Batch commit landed
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — reflect
        on the 7-file frontmatter-prepending migration. This is mechanically
        repetitive — read file, prepend 5 lines, write new file, delete old.
        Is there a `scripts/dev/migrate-with-frontmatter.sh <src> <dst>
        <title> <order> [description]` helper that automates the read-prepend-
        write-rm sequence? Strongly yes — it would be used 12+ more times
        in 07.3-07.5. Build it NOW. Commit via `build(scripts): add
        migrate-with-frontmatter.sh — surfaced by project-reorganization/
        section-07.2 retrospective`. Use it for the remaining migrations.

---

## 07.3 Migrate to docs/compiler/design/ (3 files)

**File(s):** 3 new files in `docs/compiler/design/`, 3 deletions in `scratchpad/`

These are compiler architecture notes with durable value. The destination is a cross-repo glob (per Pass 2 read of content.config.ts), so schema compliance is mandatory.

- [ ] Ensure target directory exists (likely does):
  ```bash
  mkdir -p docs/compiler/design
  ls docs/compiler/design/ | head
  ```

- [ ] **Migration 1 of 3: aot-test-gaps-design.md**
  - [ ] Source: `scratchpad/aot-test-gaps.md`
  - [ ] Use the `migrate-with-frontmatter.sh` helper (from 07.2 retrospective) OR manually:
    - Read source
    - Prepend frontmatter: title "AOT Test Coverage Gaps", order 10, description "Audit of missing AOT test scenarios across CLI integration, error handling, end-to-end pipeline, and performance testing."
    - Write to `docs/compiler/design/aot-test-gaps-design.md`
    - `rm` source
  - [ ] Note: `order: 10` to leave room for future additions at the start of `compiler/design/`; adjust if existing files already define an ordering

- [ ] **Migration 2 of 3: lexer-architecture.md**
  - [ ] Source: `scratchpad/design-ideas/02-lexer-patterns.md`
  - [ ] Frontmatter: title "Lexer Architecture Reference", order 11, description "Token classification, two-stage lexing, cursor/source abstraction, span tracking — applied to Ori's lexer."
  - [ ] Write to `docs/compiler/design/lexer-architecture.md`
  - [ ] `rm` source

- [ ] **Migration 3 of 3: error-handling-and-diagnostics-design.md**
  - [ ] Source: `scratchpad/design-ideas/08-error-handling-ux.md`
  - [ ] Frontmatter: title "Error Handling and Diagnostics Design", order 12, description "Compiler error message design: anatomy, principles, span-based reporting, error recovery strategies."
  - [ ] Write to `docs/compiler/design/error-handling-and-diagnostics-design.md`
  - [ ] `rm` source

- [ ] Verify and commit:
  ```bash
  ls docs/compiler/design/ | grep -E '(aot-test-gaps|lexer-architecture|error-handling)'
  git add docs/compiler/design/
  git commit -m "docs(design): migrate 3 compiler design notes from scratchpad to docs/compiler/design/

Sources:
- scratchpad/aot-test-gaps.md → aot-test-gaps-design.md
- scratchpad/design-ideas/02-lexer-patterns.md → lexer-architecture.md
- scratchpad/design-ideas/08-error-handling-ux.md → error-handling-and-diagnostics-design.md

Astro docsSchema frontmatter added to each (this destination is a cross-repo
glob per ori-lang-website/src/content.config.ts:compilerDesign).

Refs: plans/project-reorganization/section-07-scratchpad-triage.md"
  ```

- [ ] **Subsection close-out (07.3)** — MANDATORY before starting 07.4:
  - [ ] 3 target files with frontmatter
  - [ ] 3 sources removed
  - [ ] Commit landed
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — did
        the `migrate-with-frontmatter.sh` helper (from 07.2) work well for
        this subsection? If yes: document success. If it needed
        adjustments (e.g., a `--order-base N` flag to auto-increment):
        improve the helper now and commit. If wasn't built: document
        "Retrospective 07.3: manual migration was adequate for 3 files;
        helper not built."

---

## 07.4 Migrate to docs/tooling/ (1 file)

**File(s):** `docs/tooling/ori-fmt-code-review-plan.md` (new), `scratchpad/ori_fmt-code-review.md` (deleted)

- [ ] Ensure target directory exists:
  ```bash
  mkdir -p docs/tooling
  ```

- [ ] **Migration: ori-fmt-code-review-plan.md**
  - [ ] Source: `scratchpad/ori_fmt-code-review.md`
  - [ ] Frontmatter: title "ori_fmt Code Review Plan", order 20, description "Review findings and remediation plan for the formatter crate — 32 findings across critical/high/medium/low severity."
  - [ ] Write to `docs/tooling/ori-fmt-code-review-plan.md`
  - [ ] `rm` source

- [ ] Verify and commit:
  ```bash
  ls docs/tooling/
  git add docs/tooling/
  git commit -m "docs(tooling): migrate ori_fmt code review plan from scratchpad

Source: scratchpad/ori_fmt-code-review.md → docs/tooling/ori-fmt-code-review-plan.md

Astro docsSchema frontmatter added (this destination is not currently a
cross-repo glob in content.config.ts, but schema compliance aids future
integration if the tooling/ subtree gets added).

Refs: plans/project-reorganization/section-07-scratchpad-triage.md"
  ```

- [ ] **Subsection close-out (07.4)** — MANDATORY before starting 07.5:
  - [ ] Target file exists with frontmatter
  - [ ] Source removed
  - [ ] Commit landed
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — the
        migration was trivial. Document: "Retrospective 07.4: single-file
        migration; helper (if built) or manual migration both adequate."

---

## 07.5 Migrate to docs/ori_lang/proposals/drafts/ (1 file)

**File(s):** `docs/ori_lang/proposals/drafts/package-system-proposal.md` (new — only the FINAL iteration-6), `scratchpad/package-system/iteration-6.md` (deleted)

Iterations 1-5 of the package-system proposal are superseded by iteration-6 (they show the design evolution, not decision-bearing content). §07.6 will delete iterations 1-5. This subsection migrates ONLY iteration-6 as the canonical draft proposal.

- [ ] Ensure target directory exists:
  ```bash
  mkdir -p docs/ori_lang/proposals/drafts
  ```

- [ ] **Migration: package-system-proposal.md**
  - [ ] Source: `scratchpad/package-system/iteration-6.md`
  - [ ] **CRITICAL — proposals use a BODY-FORMAT schema, not YAML frontmatter.** The website's `proposal-loader.ts:22-59` extracts metadata by parsing the markdown body, not the frontmatter. Fields are extracted as follows:
    - **title**: from the first H1 heading (`# Proposal: X` or `# X`)
    - **status**: from the directory path (`drafts/` → `draft`, `approved/` → `approved`, `rejected/` → `rejected`)
    - **author**: from a `**Author:** <name>` line within the first 20 lines of the body
    - **created**: from a `**Created:** <YYYY-MM-DD>` line within the first 20 lines
    - **approved**: from a `**Approved:** <YYYY-MM-DD>` line (only for `approved/` status)
    - **rejected**: from a `**Rejected:** <YYYY-MM-DD>` line (only for `rejected/` status)
    - **summary**: from the first paragraph following a `## Summary` heading (up to 200 chars)
  - [ ] Verify the loader behavior against the actual source:
    ```bash
    sed -n '22,59p' ../ori-lang-website/src/loaders/proposal-loader.ts
    ```
    Confirm: title regex `/^#\s+(?:Proposal:\s*)?(.+)/`, body-key regex `\*\*{Key}:\*\*\s*(.+)`, summary scans from `## Summary` heading.
  - [ ] Pick an existing draft proposal as a template to verify the shape:
    ```bash
    ls /home/eric/projects/ori_lang/docs/ori_lang/proposals/drafts/
    head -30 docs/ori_lang/proposals/drafts/$(ls docs/ori_lang/proposals/drafts/ | head -1) 2>/dev/null
    ```
  - [ ] Normalize `iteration-6.md` into body-format shape. The target file structure is:
    ```markdown
    # Proposal: Package Management System

    **Author:** Ori Core Team
    **Created:** 2026-03-01

    ## Summary

    Manifest-based package system with version semantics, stdlib bundling, and distribution strategy for the Ori compiler and ecosystem.

    ## Motivation

    <body content from iteration-6.md — migrate existing sections as-is below this point>

    ## Design

    <...>
    ```
    (Do NOT add a `---`-delimited YAML frontmatter block. The proposal loader ignores it.)
  - [ ] Read `scratchpad/package-system/iteration-6.md` in full, identify any existing H1 (if present, keep it; if absent, prepend one), add `**Author:**` and `**Created:**` body keys, and ensure a `## Summary` section exists with a concise first paragraph.
  - [ ] Write the normalized file to `docs/ori_lang/proposals/drafts/package-system-proposal.md` via the `Write` tool.
  - [ ] Verify the loader can parse it:
    ```bash
    # Sanity check: the file should have an H1, **Author:** / **Created:** keys, and a ## Summary section
    head -20 docs/ori_lang/proposals/drafts/package-system-proposal.md
    grep -q '^# ' docs/ori_lang/proposals/drafts/package-system-proposal.md && echo "H1 present"
    grep -q '\*\*Author:\*\*' docs/ori_lang/proposals/drafts/package-system-proposal.md && echo "Author present"
    grep -q '\*\*Created:\*\*' docs/ori_lang/proposals/drafts/package-system-proposal.md && echo "Created present"
    grep -q '^## Summary' docs/ori_lang/proposals/drafts/package-system-proposal.md && echo "Summary section present"
    ```
  - [ ] `rm` source (`scratchpad/package-system/iteration-6.md`)

- [ ] Verify the proposals cross-repo glob still works:
  ```bash
  grep -n 'proposals' /home/eric/projects/ori-lang-website/src/content.config.ts
  # The proposalLoader is used — confirm this destination is a loader target
  ```

- [ ] Commit:
  ```bash
  git add docs/ori_lang/proposals/drafts/
  git commit -m "docs(proposals): migrate package-system draft proposal from scratchpad

Source: scratchpad/package-system/iteration-6.md (final iteration)
Target: docs/ori_lang/proposals/drafts/package-system-proposal.md

Iterations 1-5 are deleted separately in section-07.6 as superseded
evolution history. Only iteration-6 is canonical.

Proposal frontmatter (title + status + author + created + summary)
matches the proposalLoader schema used by
ori-lang-website/src/content.config.ts.

Refs: plans/project-reorganization/section-07-scratchpad-triage.md"
  ```

- [ ] **Subsection close-out (07.5)** — MANDATORY before starting 07.6:
  - [ ] Target file exists with proposal frontmatter
  - [ ] iteration-6 source removed (iterations 1-5 remain for §07.6)
  - [ ] Commit landed
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — the
        proposal schema differs from docsSchema. Did the `migrate-with-
        frontmatter.sh` helper (from 07.2) handle this? If it only supports
        docsSchema-shaped frontmatter, extend it with a `--schema proposal|
        docs` flag. Commit extension. Otherwise document: "Retrospective
        07.5: manual frontmatter was fine for one-off different schema."

---

## 07.6 Delete Superseded & Throwaway Files (18 files)

**File(s):** 18 files deleted from `scratchpad/` (reconciled from the 30-file live inventory: 30 total − 12 migrations = 18 deletions, including `07-modern-lang-repos.md` which Agent 2 flagged as conditional DELETE)

Delete everything that's not migrating. All files are in gitignored `scratchpad/`, so `rm` is used directly (not `git rm`).

- [ ] Delete the 5 root scratchpad files that are not migrating:
  ```bash
  rm scratchpad/aot-llvm-findings.md
  rm scratchpad/formatter-architecture.md
  rm scratchpad/spec-to-architecture-mapping.md
  ```
  Wait — Pass 1 Agent 2 classified `aot-llvm-findings.md` as DELETE (duplicates plans/aot_codegen_pipeline/), `formatter-architecture.md` as DELETE (superseded), `spec-to-architecture-mapping.md` as DELETE (spec-captured). Three files.
  - (`aot-test-gaps.md` was migrated in 07.3; `ori_fmt-code-review.md` was migrated in 07.4; so only these 3 root files delete.)

- [ ] Delete the 6 general-reference design-ideas files:
  ```bash
  rm scratchpad/design-ideas/03-parser-patterns.md
  rm scratchpad/design-ideas/04-type-system-design.md
  rm scratchpad/design-ideas/05-interpreter-patterns.md
  rm scratchpad/design-ideas/06-codegen-patterns.md
  rm scratchpad/design-ideas/07-modern-lang-repos.md  # conditional — delete unless still actively referenced
  rm scratchpad/design-ideas/09-testing-tooling.md
  ```
  - [ ] Agent 2 noted `07-modern-lang-repos.md` as "DELETE (conditional)" — it's prior-art compiler research. If you still actively consult it, migrate to `docs/compiler/design/prior-art-compiler-architectures.md` instead of deleting. For this plan: **default to delete** since Pass 2 research is already captured in `plans/project-reorganization/00-overview.md`'s "Reference implementations studied" section.

- [ ] Delete the 4 experimental .ori programs:
  ```bash
  rm scratchpad/experiments/dice_game.ori
  rm scratchpad/experiments/expr_eval.ori
  rm scratchpad/experiments/game_of_life.ori
  rm scratchpad/experiments/snake.ori
  ```

- [ ] Delete the 5 superseded package-system iterations (iteration-6 was migrated in 07.5):
  ```bash
  rm scratchpad/package-system/iteration-1.md
  rm scratchpad/package-system/iteration-2.md
  rm scratchpad/package-system/iteration-3.md
  rm scratchpad/package-system/iteration-4.md
  rm scratchpad/package-system/iteration-5.md
  ```

- [ ] Deletion math: 3 (root: aot-llvm-findings, formatter-architecture, spec-to-architecture-mapping) + 6 (design-ideas: 03, 04, 05, 06, 07-modern-lang-repos, 09) + 4 (experiments/*.ori) + 5 (package-system/iterations 1-5) = **18 deletions**. Combined with §07.2-§07.5's **12 migrations**, the total is **30 files** — matching the §01 baseline count exactly.

- [ ] Verify scratchpad is now empty:
  ```bash
  find scratchpad -type f 2>&1
  # Expected: empty — 12 migrated + 18 deleted = 30 files processed, matching baseline
  ```
  - [ ] The file-count math: 12 + 18 = 30, which equals the §01.1 baseline count
  - [ ] If scratchpad/ still has files: classify each and handle (migrate or delete) as additional work in this subsection. Do NOT proceed to §07.7 with stragglers.

- [ ] No commit needed — all deletions are in gitignored `scratchpad/`, which doesn't touch git state. The commits for §07 are from 07.2, 07.3, 07.4, 07.5 which covered the migrations (additions + removals from git). §07.6 is filesystem-only cleanup.

- [ ] **Subsection close-out (07.6)** — MANDATORY before starting 07.7:
  - [ ] All non-migrated scratchpad files deleted (17 or 18 depending on modern-lang-repos decision)
  - [ ] `scratchpad/` contains no more .md/.ori files (may still have empty subdirs)
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — bulk
        file deletion. Was there friction in the 18 `rm` commands? A
        `scripts/dev/bulk-delete.sh <file-list-file>` helper that takes a
        list and deletes each with confirmation would prevent accidents.
        Overkill for this one-off. Document: "Retrospective 07.6: bulk
        rm adequate for one-time scratchpad cleanup; no reusable tooling
        warranted."

---

## 07.7 Remove scratchpad/ Directory & Verify Website Build

**File(s):** `scratchpad/` (removed from filesystem)

- [ ] Verify scratchpad is now empty of files (subdirectories may remain):
  ```bash
  find scratchpad -type f
  # Expected: no output
  ```

- [ ] Remove all empty subdirectories and then the top-level scratchpad:
  ```bash
  find scratchpad -type d -empty -delete
  # Or: rmdir -p scratchpad/experiments scratchpad/design-ideas scratchpad/package-system scratchpad
  ```

- [ ] Verify:
  ```bash
  ls scratchpad 2>&1 || echo "absent (expected)"
  ```

- [ ] The `scratchpad` entry in `.gitignore:84` can stay (harmless — if someone re-creates the directory in the future, it'll be auto-ignored).

- [ ] Run the website build to verify every migrated file satisfies the cross-repo glob schemas:
  ```bash
  cd /home/eric/projects/ori-lang-website
  timeout 150 bun run build 2>&1 | tail -40
  ```
  - [ ] Build succeeds (exit 0)
  - [ ] No schema errors on any docs collection
  - [ ] No "missing required field" errors on the new files

- [ ] Return to ori_lang and verify `./test-all.sh` is still green:
  ```bash
  cd /home/eric/projects/ori_lang
  timeout 150 ./test-all.sh 2>&1 | tail -10
  ```

- [ ] **Subsection close-out (07.7)** — MANDATORY before 07.R:
  - [ ] `scratchpad/` directory absent
  - [ ] Website build succeeds
  - [ ] `./test-all.sh` green
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — the
        cross-repo build verification is the same action as §04.5. If a
        `scripts/dev/verify-website-build.sh` helper was built in §04.5's
        retrospective (probably not — that subsection's retrospective
        noted "one-time"), this is the second usage, which argues for
        building it. Decide: is `bun run build` + tail standard enough to
        not need a helper? Probably yes. Document: "Retrospective 07.7:
        direct bun run build + tail is adequate."

---

## 07.R Third Party Review Findings

<!-- Dual-source /tpr-review findings (iteration 1, 2026-04-11). Both reviewers read the plan AND the live source tree. All findings verified against live source before fix. -->

- [x] `[TPR-XX-003-codex][medium]` `plans/project-reorganization/section-07-scratchpad-triage.md:416` — Remove the LEAK in proposal metadata migration.
  Evidence: 07.5 told the agent to prepend YAML frontmatter to `docs/ori_lang/proposals/drafts/package-system-proposal.md` and treat that as the proposalLoader schema. Live verification confirmed `ori-lang-website/src/loaders/proposal-loader.ts:22-59` does NOT read YAML frontmatter — it extracts title from the first H1, author/created/approved/rejected from `**Key:**` body lines within the first 20 lines, and summary from the `## Summary` section body. Only `status` comes from the directory path (`drafts/`→draft, etc.).
  Impact: The migrated proposal would not have conformed to the actual metadata contract. Title, author, created, summary would have been disconnected from the loader's real source of truth.
  Required plan update: Change 07.5 to normalize the proposal into body format (H1 + `**Author:**` / `**Created:**` body keys + `## Summary` paragraph).
  Basis: fresh_verification. Confidence: high.
  Resolved: Fixed on 2026-04-11. Rewrote §07.5 with the correct body-format schema, added verification commands that check for H1, `**Author:**`, `**Created:**`, and `## Summary` in the output file. Updated §07 goal, success criteria, and 00-overview.md mission criterion to reflect the correct format.

- [x] `[TPR-XX-005-codex][medium]` `plans/project-reorganization/section-07-scratchpad-triage.md:516` — Resolve the DRIFT in scratchpad inventory math.
  Evidence: The section carried unresolved counts — goal and success criteria said 31 files / 14 migrations / 17 deletions; line 516 acknowledged 18 deletions; line 119 expected 31 files. Fresh `find scratchpad -type f | wc -l` returns 30, not 31. The FIXME comment in §07.6 acknowledged the contradiction but left the overview totals inconsistent.
  Impact: §07 was not execution-ready because its basic inventory could not be verified as written. An implementing agent would not know whether a count mismatch was expected drift, a missed file, or a plan bug.
  Required plan update: Reconcile the live scratchpad inventory, fix section totals and overview totals to a single consistent set of numbers, make the conditional `07-modern-lang-repos.md` treatment explicit in the headline counts.
  Basis: fresh_verification. Confidence: high.
  Resolved: Fixed on 2026-04-11. Reconciled all count references across §07 frontmatter (goal, success_criteria), §07 context paragraph, §07.1 Pre-Migration expected count, §07.6 title + deletion math + verification, §07.N completion checklist, and 00-overview.md (mission success criterion, current state/target state diagrams, dependency graph prose, implementation sequence, scratchpad metrics table, code-deleted estimate) to the reconciled numbers: **30 total files, 12 migrations, 18 deletions**. The conditional `07-modern-lang-repos.md` is now explicit as a DELETE in the headline counts.

---

## 07.N Completion Checklist

- [ ] 7 files migrated to `docs/ori_lang/v2026/design/` with Astro frontmatter (07.2)
- [ ] 3 files migrated to `docs/compiler/design/` with Astro frontmatter (07.3)
- [ ] 1 file migrated to `docs/tooling/` (07.4)
- [ ] 1 file migrated to `docs/ori_lang/proposals/drafts/` with proposal frontmatter (07.5)
- [ ] 18 files deleted from scratchpad (07.6) — reconciled math: 12 migrations + 18 deletions = 30 files matching baseline
- [ ] `scratchpad/` directory absent (07.7)
- [ ] Website build succeeds with all new migrated files (07.7)
- [ ] `./test-all.sh` green
- [ ] 4 batch commits landed (one per destination in 07.2-07.5)
- [ ] Plan annotation cleanup: N/A (no `.rs` files modified)
- [ ] **Plan sync** — update plan metadata:
  - [ ] This section's frontmatter `status` → `complete`
  - [ ] `00-overview.md` Quick Reference table for §07 → `Complete`
  - [ ] `00-overview.md` mission success criterion "Scratchpad is empty or absent" → checked
  - [ ] `00-overview.md` Known Bugs rows for scratchpad duplication → `Fixed`
  - [ ] `00-overview.md` Metrics table: update scratchpad counts to canonical reconciled values (30 total, 12 migrated, 18 deleted, 0 remaining)
  - [ ] `index.md` updated
- [ ] `/tpr-review` passed — check: (a) no migrated file missing frontmatter, (b) no migrated file has wrong schema (docsSchema vs proposal), (c) every deletion is justified
- [ ] `/impl-hygiene-review` passed (AFTER TPR clean) — SSOT compliance (no duplicated content between migrated docs/ files and existing spec/rules), no BLOAT (did we preserve content that was already duplicated elsewhere?)
- [ ] `/improve-tooling` **section-close sweep** — per-subsection retrospectives (07.1-07.7) should already be committed. Cross-subsection pattern check: the biggest insight is the `migrate-with-frontmatter.sh` helper (from 07.2), which enabled 07.3, 07.4, 07.5 to be faster. Verify the helper's generality — does it handle both `docsSchema` and `proposals` frontmatter shapes? If not, extend it now. Commit any extensions via `build(scripts): ... — surfaced by section-07 close sweep`. If helper was not built: re-evaluate decision — it was used 12 times, that's high reusability.

**Exit Criteria:** `scratchpad/` directory absent. 14 migrated files exist at their destinations with frontmatter matching the expected schema. 17-18 files deleted. `ori-lang-website` Astro build succeeds with all new files validated. `./test-all.sh` green. Mission success criterion "Scratchpad is empty or absent" satisfied.
