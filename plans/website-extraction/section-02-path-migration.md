---
section: "02"
title: "Path Migration"
status: not-started
goal: "All relative paths updated from ../ to ../ori_lang/ so content loads from sibling repo"
depends_on: []
sections:
  - id: "02.1"
    title: "Content Collections"
    status: not-started
  - id: "02.2"
    title: "Remark Plugins"
    status: not-started
  - id: "02.3"
    title: "Build Scripts"
    status: not-started
  - id: "02.4"
    title: "Playground WASM Cargo.toml"
    status: not-started
  - id: "02.5"
    title: "Playground Error Messages & Hardcoded Paths"
    status: not-started
  - id: "02.6"
    title: "Completion Checklist"
    status: not-started
---

# Section 02: Path Migration

**Status:** Not Started
**Goal:** All relative paths in the website code point to `../ori_lang/` instead of `../`, so content loads correctly when the website repo is a sibling of the compiler repo.

**Context:** When the website lived inside `ori_lang/website/`, relative paths like `../docs/guide` resolved to `ori_lang/docs/guide`. Now that the website is at `ori-lang-website/`, those paths must become `../ori_lang/docs/guide`. This is the core of the migration — every path that reaches into the parent repo must be updated.

**Reference implementations:**
- **warpkit-website** `src/content.config.ts`: Uses `../warpkit/guide` and `../warpkit/docs` to load content from sibling repo

---

## 02.1 Content Collections

**File(s):** `src/content.config.ts`

Update all `glob()` base paths and plan loader paths. Current → New:

- [ ] `../docs/guide` → `../ori_lang/docs/guide`
- [ ] `../docs/ori_lang/v2026/spec` → `../ori_lang/docs/ori_lang/v2026/spec`
- [ ] `../docs/compiler/design` → `../ori_lang/docs/compiler/design`
- [ ] `../docs/tooling/formatter/design` → `../ori_lang/docs/tooling/formatter/design`
- [ ] `../docs/tooling/lsp/design` → `../ori_lang/docs/tooling/lsp/design`
- [ ] `../plans/roadmap` → `../ori_lang/plans/roadmap`
- [ ] `../plans/code-journeys` → `../ori_lang/plans/code-journeys`
- [ ] `../plans/${r.dir}` → `../ori_lang/plans/${r.dir}` (in `planSectionLoader` calls)
- [ ] `../blog` → `../ori_lang/blog`
- [ ] `../docs/ori_lang/proposals` → `../ori_lang/docs/ori_lang/proposals`

Also update `src/lib/plan-data.ts`:

- [ ] Default `plansBase` parameter in both `loadReroutes()` and `loadParallelPlans()`: `'../plans'` → `'../ori_lang/plans'`

Also update `src/loaders/proposal-loader.ts`:

- [ ] `filePath: \`docs/ori_lang/proposals/${dir}/${file}\`` → `filePath: \`ori_lang/docs/ori_lang/proposals/${dir}/${file}\`` (store metadata path, line 119)

`src/loaders/plan-section-loader.ts` uses `filePath.replace(resolve(process.cwd(), '..') + '/', '')` which dynamically strips the parent directory:
- **Before**: cwd = `ori_lang/website/`, parent = `ori_lang/`, strips `ori_lang/` leaving `plans/roadmap/section-01.md`
- **After**: cwd = `ori-lang-website/`, parent = `projects/`, strips `projects/` leaving `ori_lang/plans/roadmap/section-01.md`
- The stored `filePath` will now be `ori_lang/plans/...` instead of `plans/...`. Verify that nothing downstream depends on the exact shape of this path (it is used for display/linking only, so the change should be benign, but verify).

- [ ] Verify `plan-section-loader.ts` filePath behavior — the stored relative path will change from `plans/...` to `ori_lang/plans/...` after migration. Check if any page templates or components use this path for linking.

---

## 02.2 Remark Plugins

**File(s):** `src/remark/remark-include.mjs`, `src/remark/remark-md-links.mjs`

### remark-include.mjs — NO CHANGES NEEDED

Resolves `{{#include path}}` relative to the markdown file being processed (`file.history[0]`). Since the markdown files remain in `ori_lang/` and the remark plugin resolves relative to each file's own directory, this works correctly without changes.

### remark-md-links.mjs — CLEANUP NEEDED + verify

Uses `COLLECTION_MAPPINGS` with `sourceBase` paths like `docs/ori_lang/v2026/spec`, `docs/guide`, etc. These are matched via `filePath.includes(m.sourceBase)`. Since the filePaths will contain `ori_lang/docs/ori_lang/v2026/spec`, the `includes()` check still matches. However:

- [ ] Verify that `remark-md-links.mjs` `COLLECTION_MAPPINGS` `sourceBase` values still match after migration — the plugin uses `filePath.includes(m.sourceBase)` which should work since `ori_lang/docs/guide` still contains `docs/guide`, but edge cases (e.g., if Astro resolves to an absolute path that changes structure) should be tested
- [ ] Run `bun run build` and check that cross-references in spec documents resolve correctly (e.g., clause-to-clause links within the spec)
- [ ] **Clean up debug logging**: Remove the 4 `console.log` statements at lines 22-25 of `remark-md-links.mjs`. These are debug spew (`file keys:`, `file.data keys:`, `file.history:`, `file.path:`) that produce noise on every markdown file processed during build. They should have been removed after initial development.

---

## 02.3 Build Scripts

**File(s):** `package.json`, `scripts/generate-changelog.sh`, `scripts/add-compiler-design-frontmatter.sh`

- [ ] Update `prebuild` in `package.json`:
  - `cp ../install.sh public/install.sh` → `cp ../ori_lang/install.sh public/install.sh`
  - `copy-wasm.sh`, `rebuild-wasm.sh`, and `build-tutorial.mjs` use `$SCRIPT_DIR`/`$WEBSITE_DIR` or `process.cwd()` relative paths — no changes needed
  - `generate-changelog.sh` and `add-compiler-design-frontmatter.sh` have hardcoded `../` paths — see below
- [ ] Update `scripts/generate-changelog.sh`:
  - Currently uses `REPO_ROOT="$(dirname "$WEBSITE_DIR")"` then `cd "$REPO_ROOT"` and runs `git log`
  - After migration, `REPO_ROOT` would be the parent of `ori-lang-website/` (e.g., `projects/`) which is not a git repo
  - Must point to `ori_lang/` repo specifically: `REPO_ROOT="$WEBSITE_DIR/../ori_lang"` (or accept it as a parameter)
  - The git log must run inside the ori_lang repo to get compiler commit history
- [ ] Update `scripts/add-compiler-design-frontmatter.sh`:
  - Currently has `DESIGN_DIR="../docs/compiler/design"` (hardcoded, not using `$SCRIPT_DIR`)
  - Must become `DESIGN_DIR="../ori_lang/docs/compiler/design"` or use `$SCRIPT_DIR/../..` pattern
  - Note: this script is NOT in the prebuild hook or CI -- it's a manual one-time script. But it should still be updated to work from the new repo location.
  - **Hygiene**: While updating the path, add `SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"` (like the other scripts) and switch from CWD-relative to `$SCRIPT_DIR`-relative: `DESIGN_DIR="$SCRIPT_DIR/../../ori_lang/docs/compiler/design"`. Currently it only works when run from the website root directory.

---

## 02.4 Playground WASM Cargo.toml

**File(s):** `playground-wasm/Cargo.toml`

Update compiler crate path dependencies. The playground-wasm will be at `ori-lang-website/playground-wasm/`, so paths to compiler crates go through `../../ori_lang/compiler/` (two levels up: playground-wasm → ori-lang-website → projects → ori_lang):

- [ ] `ori_compiler`: `path = "../../compiler/ori_compiler"` → `path = "../../ori_lang/compiler/ori_compiler"`
  - Note: current path from `ori_lang/website/playground-wasm/` goes `../../` to `ori_lang/`, then `compiler/`. New path from `ori-lang-website/playground-wasm/` goes `../../` to `projects/`, then `ori_lang/compiler/`.
- [ ] `ori_eval`: `path = "../../compiler/ori_eval"` → `path = "../../ori_lang/compiler/ori_eval"`
- [ ] `ori_diagnostic`: `path = "../../compiler/ori_diagnostic"` → `path = "../../ori_lang/compiler/ori_diagnostic"`

**CI note:** The CI symlink (`ln -s "$GITHUB_WORKSPACE/ori_lang" "$GITHUB_WORKSPACE/../ori_lang"`) makes `../../ori_lang/` resolve correctly from playground-wasm in both local dev and CI environments.

---

## 02.5 Playground Error Messages & Hardcoded Paths

**File(s):** `src/components/playground/wasm-runner.ts`, `src/components/playground/Playground.svelte`, `scripts/copy-wasm.sh`

- [ ] Update the WASM-not-loaded error message in `wasm-runner.ts` `runOri()`:
  - Currently: `'cd website/playground-wasm && wasm-pack build --target web --out-dir pkg'`
  - Should become: `'cd playground-wasm && wasm-pack build --target web --out-dir pkg'` (no longer inside `website/`)
- [ ] Update the WASM load error message in `Playground.svelte` (line 47):
  - Currently: `'cd website/playground-wasm && wasm-pack build --target web --out-dir pkg'`
  - Should become: `'cd playground-wasm && wasm-pack build --target web --out-dir pkg'`
- [ ] Update the error message in `scripts/copy-wasm.sh` (line 14):
  - Currently: `"Build WASM first: cd website/playground-wasm && wasm-pack build --target web --out-dir pkg"`
  - Should become: `"Build WASM first: cd playground-wasm && wasm-pack build --target web --out-dir pkg"`

---

## 02.6 Completion Checklist

- [ ] `bun run dev` starts Astro dev server and loads all content collections without errors
- [ ] All content pages render (guide, spec, compiler-design, formatter, lsp, roadmap, plans, proposals, blog, journeys)
- [ ] Playground WASM builds: `cd playground-wasm && wasm-pack build --target web --out-dir pkg`
- [ ] Tutorial manifest generates: `node scripts/build-tutorial.mjs`
- [ ] Changelog generates: `bash scripts/generate-changelog.sh`
- [ ] `grep -r 'cd website/' src/ scripts/` returns no results (all `website/` prefix paths updated)
- [ ] No debug `console.log` statements remain in remark plugins

**Exit Criteria:** Running `bun run dev` in `ori-lang-website/` with `ori_lang/` as a sibling directory renders all pages correctly, and `bun run build` produces a complete `dist/` with all content.
