---
section: "04"
title: "Blog Migration (Cross-Repo, Atomic)"
status: not-started
reviewed: false
goal: "Migrate the 3 tracked blog posts from `ori_lang/blog/` to `ori-lang-website/src/content/blog/` atomically across the repository boundary — preserving their existing Astro-compliant frontmatter, changing the website's cross-repo glob loader base from `'../ori_lang/blog'` to `'./content/blog'`, and deleting `ori_lang/blog/` from tracking — with per-file `AskUserQuestion` approval for every edit to `ori-lang-website/` per `projects/CLAUDE.md`."
success_criteria:
  - "`ori-lang-website/src/content/blog/` contains 3 files: building-ori-from-scratch.md, cross-compilation-nightmare.md, three-weeks-of-compiler-plumbing.md"
  - "Each migrated file has the same frontmatter as the original (title, date, description)"
  - "`ori-lang-website/src/content.config.ts:85-92` blog loader base is `'./content/blog'` (not `'../ori_lang/blog'`)"
  - "`git ls-files blog/` in ori_lang returns empty"
  - "`bun run dev` in ori-lang-website (or equivalent astro build) renders the 3 blog posts without 404s or schema errors"
  - "`./test-all.sh` in ori_lang still green (the blog migration does not touch anything test-all exercises)"
  - "Each edit to ori-lang-website went through AskUserQuestion approval per projects/CLAUDE.md — 4 approvals total (1 loader + 3 content files)"
  - "Satisfies mission criterion: '**Blog is served from website**'"
inspired_by:
  - "Astro content collection loader swaps — canonical pattern when splitting a monorepo with cross-repo globs"
depends_on: ["02"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "04.1"
    title: "Pre-Migration Verification"
    status: not-started
  - id: "04.2"
    title: "Website Loader Update (AskUserQuestion #1)"
    status: not-started
  - id: "04.3"
    title: "Blog Content Creation (AskUserQuestion ×3)"
    status: not-started
  - id: "04.4"
    title: "ori_lang/blog/ Removal"
    status: not-started
  - id: "04.5"
    title: "Website Build Verification"
    status: not-started
  - id: "04.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "04.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 04: Blog Migration (Cross-Repo, Atomic)

**Status:** Not Started
**Goal:** Move `ori_lang/blog/`'s 3 files to `ori-lang-website/src/content/blog/`, update the website's Astro content-collection loader base to point at the new local path, and delete the source files from `ori_lang/`. Because this touches two repositories, the plan is executed as a strict sequence (TPR-XX-003-codex iteration 3 fix — prior goal prose said "loader first then content" which contradicted the actual §04.2 → §04.3 → §04.4 subsection order): **first create the content files** in `ori-lang-website/src/content/blog/` (§04.2), **then switch the loader base** in `content.config.ts` from `'../ori_lang/blog'` to `'./content/blog'` (§04.3), **then delete the source files** from `ori_lang/blog/` (§04.4). This ordering guarantees the website build is consistent at every intermediate state: at step 1 the website still reads from the old cross-repo path and sees all content; at step 2 the loader switch points at the new local path which now has all content; at step 3 the old path is deleted but no longer referenced. If the order were reversed (loader first), there would be a moment where the new local path is empty but already referenced, breaking the build.

**Success Criteria:**

- [ ] `ori-lang-website/src/content/blog/` contains exactly 3 files with the expected names
- [ ] Each file's frontmatter (`title`, `date`, `description`) matches the original byte-for-byte
- [ ] Each file's body content matches the original byte-for-byte (verified by `sha256sum` comparison)
- [ ] `ori-lang-website/src/content.config.ts:85-92` loader base reads `'./content/blog'` (local to `src/`)
- [ ] `git ls-files blog/` in ori_lang returns empty
- [ ] Astro dev server (or static build) renders all 3 blog posts successfully
- [ ] 4 AskUserQuestion approvals documented in this section's checklist (1 for loader, 3 for content files)
- [ ] `./test-all.sh` green in ori_lang post-migration
- [ ] Satisfies mission criterion: "**Blog is served from website**"

**Context:** The blog content is currently read by `ori-lang-website` via a **cross-repo filesystem glob** at `content.config.ts:85-92`:
```ts
const blog = defineCollection({
  loader: glob({ pattern: '*.md', base: '../ori_lang/blog' }),
  schema: z.object({
    title: z.string(),
    date: z.coerce.date(),
    description: z.string(),
  }),
});
```
This cross-repo coupling is fragile — it only works when both repos are siblings in the filesystem layout, and it conflates "blog content authored by the compiler team" with "compiler source tree." The user identified this as the #1 reorg problem during the original request: "blog needs to be moved over to the ori-lang-website repo as that def doesn't belong in here."

Pass 2 research confirmed the 3 files already have Astro-compliant frontmatter (title, date, description) — no rewriting needed. The migration is:
1. **Change the glob base**: `'../ori_lang/blog'` → `'./content/blog'`. Astro's glob loader is relative to the website repo's `src/content/config.ts` directory, so `'./content/blog'` means `src/content/blog/` (because content.config.ts lives in `src/`).
2. **Create the 3 files** at `ori-lang-website/src/content/blog/` with identical content.
3. **Delete** `ori_lang/blog/`.

**Critical ordering constraint:** The website build runs `astro dev` or `astro build`, and the glob loader must resolve to a non-empty directory or the build errors with "no content collection entries found." So the sequence must be:
1. First: create target files + update loader base atomically in the website repo
2. THEN: remove source files from ori_lang

If we did the removal first, the website would be momentarily broken (loader points at empty old path) until the loader update + content create landed. Doing the content create + loader update first means the website is briefly consuming the SAME files from two places (new local glob + old cross-repo glob both point at the identical content), which is harmless.

Actually, re-examining: the new loader `'./content/blog'` is LOCAL; the old loader `'../ori_lang/blog'` is CROSS-REPO. They can't both exist simultaneously in content.config.ts (it's a single `blog` collection definition). So the correct sequencing is:
1. Create the target files in `ori-lang-website/src/content/blog/` — website build still works because old cross-repo loader is still reading from ori_lang/blog/ which still exists
2. Change the loader base in content.config.ts — website now reads from NEW path; old ori_lang/blog/ still exists but is no longer glob-referenced
3. Delete ori_lang/blog/ — nothing references it anymore

This means step 1 happens BEFORE step 2, not the other way around. Updating the plan's subsection order accordingly.

**Reference implementations:**
- **Astro docs**: `glob` loader base semantics — `'./content/blog'` is relative to the `content.config.ts` file (which lives in `src/`), resolving to `src/content/blog/`
- **Rust monorepo splits (e.g., `rust-clippy` extraction from `rust-lang/rust`)**: always update the downstream consumer's reference BEFORE removing files from the upstream location

**Depends on:** §02 (stale `website/` paths in ori_lang must be fixed first, so `./test-all.sh` is usable as a regression guard during the migration).

---

## 04.1 Pre-Migration Verification

**File(s):** No edits — verification only

Before touching either repo, verify the current state matches the plan's assumptions. If `ori-lang-website/src/content/blog/` already exists (partially migrated from a prior attempt), or if the blog files' frontmatter has changed since Pass 2 research, the plan must detect and handle it.

- [ ] Verify the 3 source files exist with expected names:
  ```bash
  cd /home/eric/projects/ori_lang
  ls blog/
  # Expected: building-ori-from-scratch.md  cross-compilation-nightmare.md  three-weeks-of-compiler-plumbing.md
  ```
  - [ ] All 3 files present

- [ ] Verify the existing frontmatter on each file:
  ```bash
  for f in blog/*.md; do
    echo "=== $f ==="
    sed -n '1,6p' "$f"
    echo ""
  done
  ```
  - [ ] Each file has `---` frontmatter block with `title:`, `date:`, `description:`
  - [ ] No other frontmatter fields that the website schema would reject

- [ ] Capture sha256 of each source file (for post-migration byte-equality verification):
  ```bash
  sha256sum blog/*.md > plans/project-reorganization/baseline/blog-source-sha256.txt
  cat plans/project-reorganization/baseline/blog-source-sha256.txt
  ```

- [ ] Verify the target directory does NOT exist yet (we'll create it):
  ```bash
  ls ../ori-lang-website/src/content/blog 2>&1
  # Expected: "No such file or directory"
  ```
  - [ ] If it exists: STOP. A prior migration attempt may have left partial state. Audit contents, reconcile with source, and proceed only when target is empty or in known state.

- [ ] Verify the current loader base in `content.config.ts`:
  ```bash
  sed -n '85,92p' ../ori-lang-website/src/content.config.ts
  ```
  - [ ] Confirm: `base: '../ori_lang/blog'`
  - [ ] If already different (e.g., someone already changed it): STOP and reconcile.

- [ ] Verify the ori-lang-website repo is clean enough to commit to:
  ```bash
  cd ../ori-lang-website
  git status --porcelain | head -20
  ```
  - [ ] Document the current state of the website repo's working tree. This is for situational awareness before we make changes — the website repo may have unrelated in-flight work that should not be bundled with the blog migration commit.
  - [ ] If dirty: discuss with user whether to stash, isolate, or proceed. Do NOT silently commit alongside unrelated changes.

- [ ] **Subsection close-out (04.1)** — MANDATORY before starting 04.2:
  - [ ] All 3 source files verified present with expected frontmatter
  - [ ] sha256 baselines captured
  - [ ] Target directory confirmed not yet existing
  - [ ] Current loader base confirmed stale (`'../ori_lang/blog'`)
  - [ ] Website repo state documented
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — reflect
        on cross-repo verification: is there a `scripts/dev/verify-sibling-repo.sh
        <repo-name> <expected-clean|dirty>` helper for pre-flight checks
        before cross-repo operations? §04 is the only cross-repo section
        in this plan, so one-time value is limited, but future plans that
        touch orijs, warpkit, or warpkit-website may benefit. If yes, add
        with conservative scope (just "is this path+git-status as expected")
        and commit via `build(scripts): add verify-sibling-repo.sh —
        surfaced by project-reorganization/section-04.1 retrospective`.
        Otherwise: "Retrospective 04.1: one-time cross-repo checks; no
        reusable tooling warranted."

---

## 04.2 Blog Content Creation (AskUserQuestion ×3)

**File(s):** `ori-lang-website/src/content/blog/building-ori-from-scratch.md`, `cross-compilation-nightmare.md`, `three-weeks-of-compiler-plumbing.md` (all 3 NEW)

**CRITICAL per `projects/CLAUDE.md`:** Every edit to `ori-lang-website/` requires individual `AskUserQuestion` approval. This subsection makes 3 file creations in the website repo, so it triggers 3 separate approval prompts.

Note: this subsection (content create) deliberately runs BEFORE the loader update (04.3 below — I renamed from the earlier plan where it was 04.2). The rationale: at step 04.2, the website's build is still reading from `'../ori_lang/blog'` (cross-repo), so the new files in `src/content/blog/` sit unused. At step 04.3, the loader switches to the new path, and the old `ori_lang/blog/` becomes orphaned (still physically present but no longer loaded). At step 04.4, the orphaned files are deleted. At every intermediate state, the website build succeeds.

- [ ] Create the target directory:
  ```bash
  cd /home/eric/projects/ori-lang-website
  mkdir -p src/content/blog
  ```

- [ ] **AskUserQuestion #1 — building-ori-from-scratch.md**:
  - [ ] Read the source file content (entire file, ~200 lines per Pass 1):
    ```bash
    cat /home/eric/projects/ori_lang/blog/building-ori-from-scratch.md
    ```
  - [ ] Use `AskUserQuestion` with:
    - **Question**: "Approve creating `ori-lang-website/src/content/blog/building-ori-from-scratch.md` with the exact byte content of `ori_lang/blog/building-ori-from-scratch.md`? (Cross-repo migration — per projects/CLAUDE.md rule, every ori-lang-website edit needs individual approval.)"
    - **Options**: "Approve", "Show me the full content first", "Skip this file (investigate why)"
  - [ ] On approval: use `Write` tool to create `/home/eric/projects/ori-lang-website/src/content/blog/building-ori-from-scratch.md` with the exact source content
  - [ ] Verify sha256 equals source:
    ```bash
    sha256sum /home/eric/projects/ori-lang-website/src/content/blog/building-ori-from-scratch.md \
      /home/eric/projects/ori_lang/blog/building-ori-from-scratch.md
    # Both hashes must match
    ```

- [ ] **AskUserQuestion #2 — cross-compilation-nightmare.md**:
  - [ ] Read source file content
  - [ ] `AskUserQuestion` approval (same shape as #1)
  - [ ] On approval: `Write` to `ori-lang-website/src/content/blog/cross-compilation-nightmare.md`
  - [ ] Verify sha256 equals source

- [ ] **AskUserQuestion #3 — three-weeks-of-compiler-plumbing.md**:
  - [ ] Read source file content
  - [ ] `AskUserQuestion` approval (same shape)
  - [ ] On approval: `Write` to `ori-lang-website/src/content/blog/three-weeks-of-compiler-plumbing.md`
  - [ ] Verify sha256 equals source

- [ ] Verify all 3 files exist in the target:
  ```bash
  ls /home/eric/projects/ori-lang-website/src/content/blog/
  # Expected: 3 .md files
  sha256sum /home/eric/projects/ori-lang-website/src/content/blog/*.md
  # Hashes match plans/project-reorganization/baseline/blog-source-sha256.txt
  ```

- [ ] **Subsection close-out (04.2)** — MANDATORY before starting 04.3:
  - [ ] 3 AskUserQuestion approvals captured (either all approved, or any skipped files are documented with explicit rationale)
  - [ ] 3 target files exist with byte-equal content
  - [ ] sha256 verification complete
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — reflect
        on the 3-prompt approval flow. Was it tedious? Is there a safe
        batched-approval pattern that would still satisfy projects/CLAUDE.md
        (e.g., "approve all 3 files with identical diff semantics")? Probably
        NOT — the rule explicitly says "one change at a time, no batching."
        The friction is intentional. Document: "Retrospective 04.2: per-file
        approval friction is intentional per projects/CLAUDE.md; no tooling
        change warranted."

---

## 04.3 Website Loader Update (AskUserQuestion #4)

**File(s):** `ori-lang-website/src/content.config.ts` (edit lines 85-92)

Change the blog collection loader's glob base from the cross-repo `'../ori_lang/blog'` to the local `'./content/blog'`. Now that the content files exist at the new path (from 04.2), the loader update does not cause a broken-build window.

- [ ] Read the current loader definition in full:
  ```bash
  sed -n '85,92p' /home/eric/projects/ori-lang-website/src/content.config.ts
  ```
  Confirm it matches:
  ```ts
  const blog = defineCollection({
    loader: glob({ pattern: '*.md', base: '../ori_lang/blog' }),
    schema: z.object({
      title: z.string(),
      date: z.coerce.date(),
      description: z.string(),
    }),
  });
  ```

- [ ] **AskUserQuestion #4 — content.config.ts loader base**:
  - [ ] Use `AskUserQuestion` with:
    - **Question**: "Approve updating `ori-lang-website/src/content.config.ts:86` blog loader base from `'../ori_lang/blog'` to `'./content/blog'`? (Cross-repo migration — per projects/CLAUDE.md rule, every ori-lang-website edit needs individual approval.)"
    - **Options**: "Approve", "Show me the full diff first", "Adjust the destination (you'll specify)"
  - [ ] On approval: use `Edit` tool on `/home/eric/projects/ori-lang-website/src/content.config.ts`:
    ```
    old_string: '  loader: glob({ pattern: '*.md', base: '../ori_lang/blog' }),'
    new_string: '  loader: glob({ pattern: '*.md', base: './content/blog' }),'
    ```

- [ ] Verify the edit:
  ```bash
  sed -n '85,92p' /home/eric/projects/ori-lang-website/src/content.config.ts
  # Expected: base: './content/blog'
  ```

- [ ] **Subsection close-out (04.3)** — MANDATORY before starting 04.4:
  - [ ] AskUserQuestion approval captured
  - [ ] `content.config.ts:86` shows the new local base path
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — reflect
        on the single-line edit flow. For cross-repo single-line edits, is
        AskUserQuestion the ideal approval mechanism, or would a lighter
        "show diff, type Y/N" approach be more ergonomic? The harness
        provides AskUserQuestion as the canonical approval mechanism; no
        alternative exists. Document: "Retrospective 04.3: no tooling gaps —
        AskUserQuestion is the canonical cross-repo approval mechanism."

---

## 04.4 ori_lang/blog/ Removal

**File(s):** `ori_lang/blog/building-ori-from-scratch.md`, `cross-compilation-nightmare.md`, `three-weeks-of-compiler-plumbing.md` (all DELETED)

Now that the website reads from the new local path, delete the source files from `ori_lang/` and remove the empty `blog/` directory.

- [ ] Verify the website still has the files and the loader is updated (sanity before deletion):
  ```bash
  ls /home/eric/projects/ori-lang-website/src/content/blog/
  grep -n "base: './content/blog'" /home/eric/projects/ori-lang-website/src/content.config.ts
  ```
  - [ ] 3 files present in website
  - [ ] Loader base is local

- [ ] Delete the source files using `git rm`:
  ```bash
  cd /home/eric/projects/ori_lang
  git rm blog/building-ori-from-scratch.md \
         blog/cross-compilation-nightmare.md \
         blog/three-weeks-of-compiler-plumbing.md
  ```

- [ ] Remove the now-empty `blog/` directory (if git doesn't auto-remove it — it usually does):
  ```bash
  rmdir blog 2>&1 || ls blog/
  # If rmdir fails because directory is not empty, investigate what's left
  ```
  - [ ] Expected: directory gone. If not: investigate.

- [ ] Verify:
  ```bash
  git ls-files blog/
  # Expected: empty

  ls blog/ 2>&1
  # Expected: "No such file or directory"
  ```

- [ ] **Subsection close-out (04.4)** — MANDATORY before starting 04.5:
  - [ ] `git ls-files blog/` empty
  - [ ] `ori_lang/blog/` directory does not exist
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — reflect
        on the `git rm` flow. Did the rmdir-after-rm pattern feel clunky?
        (git does auto-remove empty dirs in most cases, so this was likely
        unnecessary.) No tooling gap. Document: "Retrospective 04.4: no
        tooling gaps — git rm handled directory cleanup automatically."

---

## 04.5 Website Build Verification

**File(s):** No edits — verification only

Prove the website build still works with the new local loader and that all 3 blog posts render correctly.

- [ ] Run the Astro dev server briefly (or production build) to confirm the blog collection loads:
  ```bash
  cd /home/eric/projects/ori-lang-website
  # Either:
  timeout 30 bun run dev 2>&1 | head -50 || true
  # Or (if the repo uses npm/pnpm):
  # timeout 30 npm run dev 2>&1 | head -50 || true
  ```
  - [ ] Log shows Astro startup WITHOUT schema errors on the blog collection
  - [ ] Log shows 3 blog entries loaded

- [ ] Alternatively, run a static build (cleaner for verification):
  ```bash
  timeout 150 bun run build 2>&1 | tail -30
  # Check exit code and that dist/blog/*.html files were generated
  ls dist/blog/ 2>&1 || echo "no dist/blog dir"
  ```
  - [ ] Build succeeds (exit code 0)
  - [ ] 3 blog HTML files generated

- [ ] Run `./test-all.sh` in ori_lang to confirm nothing on the ori_lang side regressed:
  ```bash
  cd /home/eric/projects/ori_lang
  timeout 150 ./test-all.sh 2>&1 | tail -10
  ```
  - [ ] Exit code 0, all phases PASS

- [ ] **Commit ori_lang side** (the blog removal):
  ```bash
  cd /home/eric/projects/ori_lang
  git commit -m "feat(plan): migrate blog to ori-lang-website

Blog content moved cross-repo to its architecturally correct home:
  ori_lang/blog/*.md → ori-lang-website/src/content/blog/*.md

Sibling repo (ori-lang-website) receives:
- building-ori-from-scratch.md
- cross-compilation-nightmare.md
- three-weeks-of-compiler-plumbing.md

Loader base updated in ori-lang-website/src/content.config.ts:86
from '../ori_lang/blog' to './content/blog' — blog collection is now
local to the website repo, eliminating the cross-repo filesystem
glob coupling.

Refs: plans/project-reorganization/section-04-blog-migration.md"
  ```

- [ ] **Commit ori-lang-website side** (the loader update + content files):
  ```bash
  cd /home/eric/projects/ori-lang-website
  git add src/content/blog/ src/content.config.ts
  git commit -m "feat: receive blog migration from ori_lang

Blog content received from ori_lang/blog/ — content.config.ts blog
collection loader base updated from cross-repo glob ('../ori_lang/blog')
to local glob ('./content/blog'). Three posts added to
src/content/blog/:
- building-ori-from-scratch.md
- cross-compilation-nightmare.md
- three-weeks-of-compiler-plumbing.md

Refs: ../ori_lang/plans/project-reorganization/section-04-blog-migration.md"
  ```
  - [ ] Note: in a true dual-repo workflow, the two commits land in separate repositories. They are conceptually atomic (both must land together or neither lands), but git does not support cross-repo atomic commits. The plan's sequencing (04.2 content create → 04.3 loader update → 04.4 source delete) ensures that at any point in time, the website build is consistent even if one side's commit lands minutes before the other.

- [ ] **Subsection close-out (04.5)** — MANDATORY before 04.R:
  - [ ] Website build succeeds with new local loader
  - [ ] `./test-all.sh` green in ori_lang
  - [ ] Both commits landed (1 in ori_lang, 1 in ori-lang-website)
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — reflect
        on the cross-repo verification journey. Was there friction in
        running `bun run build` (or whatever the website uses) — specifically,
        not knowing which package manager is canonical, or the build taking
        too long for a verification check? Is there a
        `scripts/dev/verify-website-build.sh` helper for the next time
        cross-repo content moves in/out? Probably not — this is a one-time
        migration. Document: "Retrospective 04.5: one-time cross-repo
        verification; no reusable tooling warranted."

---

## 04.R Third Party Review Findings

<!-- Dual-source /tpr-review iteration 3 (2026-04-11) surfaced one finding: goal prose contradicted subsection order. Resolved inline. -->

- [x] `[TPR-XX-003-codex][medium]` (iteration 3) `plans/project-reorganization/section-04-blog-migration.md:49` — DRIFT: Reconcile §04 with the content-first blog migration order.
  Evidence: The §04 section goal prose said the sequence is "**update the website loader first**, then **create the content files in the website**". But the same section's context analysis at lines 87-92 and the actual subsection structure (§04.2 Content Creation BEFORE §04.3 Loader Update BEFORE §04.4 ori_lang Removal) followed the correct content-first order. The goal prose and the subsection implementation disagreed about the ordering.
  Impact: An executor or reviewer reading only the goal prose could follow the wrong order and create a broken-build window (update loader → new path empty → website fails to build → THEN populate new path). The subsection bodies were correct but the goal summary was misleading.
  Required plan update: Rewrite the goal prose to match the final subsection order — create content files first, then switch the loader, then delete the old blog files.
  Basis: fresh_verification. Confidence: high.
  Resolved: Fixed on 2026-04-11 iteration 3. Rewrote §04 body-level goal prose with explicit 3-step sequencing that matches §04.2 → §04.3 → §04.4: (1) first create content files in `ori-lang-website/src/content/blog/`; (2) then switch the loader base in `content.config.ts`; (3) then delete the source files from `ori_lang/blog/`. Added explicit explanation of WHY this order is correct: at step 1 the website still reads from the old cross-repo path and sees all content; at step 2 the loader switches to the new path which now has all content; at step 3 the old path is deleted but no longer referenced. Noted that the reverse order would create a broken-build window.

---

## 04.N Completion Checklist

- [ ] 3 blog files exist in `ori-lang-website/src/content/blog/` with byte-equal content to the originals (sha256 match)
- [ ] `ori-lang-website/src/content.config.ts:86` loader base is `'./content/blog'`
- [ ] `git ls-files blog/` in ori_lang empty
- [ ] `ori_lang/blog/` directory does not exist
- [ ] Website build succeeds (Astro `bun run build` exit 0)
- [ ] `./test-all.sh` green in ori_lang post-migration
- [ ] 4 `AskUserQuestion` approvals recorded (3 content files + 1 loader)
- [ ] Two commits landed: ori_lang `feat(plan): migrate blog to ori-lang-website` + ori-lang-website `feat: receive blog migration from ori_lang`
- [ ] Plan annotation cleanup: N/A (no `.rs` files modified)
- [ ] **Plan sync** — update plan metadata:
  - [ ] This section's frontmatter `status` → `complete`
  - [ ] `00-overview.md` Quick Reference table for §04 → `Complete`
  - [ ] `00-overview.md` mission success criterion "Blog is served from website" → checked
  - [ ] `index.md` updated
- [ ] `/tpr-review` passed — cross-repo section, review should check ordering discipline (04.2 content BEFORE 04.3 loader BEFORE 04.4 delete) and byte-equality verification
- [ ] `/impl-hygiene-review` passed (AFTER TPR clean)
- [ ] `/improve-tooling` **section-close sweep** — per-subsection retrospectives (04.1-04.5) should already be committed. Cross-subsection pattern check: the entire 5-subsection flow was fundamentally "cross-repo migration sequence" — is there a reusable pattern for future cross-repo work (orijs, warpkit)? Unlikely since each cross-repo move has different coupling semantics. Document: "Section-04 close sweep: per-subsection retrospectives covered everything; each cross-repo migration is unique enough that no reusable tooling is warranted."

**Exit Criteria:** `sha256sum` of `ori-lang-website/src/content/blog/*.md` matches the pre-migration hashes of `ori_lang/blog/*.md` captured in §01.1 baseline. `ori-lang-website/src/content.config.ts:86` reads `base: './content/blog'`. `git ls-files blog/` in ori_lang returns empty. Astro dev/build succeeds in the website repo with all 3 blog posts rendered. `./test-all.sh` green in ori_lang. Mission success criterion "Blog is served from website" is satisfied.
