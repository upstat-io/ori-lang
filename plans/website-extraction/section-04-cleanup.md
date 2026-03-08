---
section: "04"
title: "Cleanup & Governance"
status: not-started
goal: "website/ removed from ori_lang, governance rules updated for new repo"
depends_on: ["03"]
sections:
  - id: "04.1"
    title: "Remove Website from ori_lang"
    status: not-started
  - id: "04.2"
    title: "Update Governance"
    status: not-started
  - id: "04.3"
    title: "Completion Checklist"
    status: not-started
---

# Section 04: Cleanup & Governance

**Status:** Not Started
**Goal:** The `website/` directory is removed from ori_lang, the old deploy workflow is deleted, and governance rules are updated to cover the new repo.

**Context:** Only perform this cleanup AFTER Section 03 is verified working. The old website code should remain in ori_lang until the new pipeline is confirmed to deploy correctly.

**Depends on:** Section 03 (CI/CD must be working before removing the old code).

---

## 04.1 Remove Website from ori_lang

**File(s):** `ori_lang/website/`, `ori_lang/.github/workflows/deploy-website.yml`

- [ ] Delete `ori_lang/website/` directory entirely (including `playground-wasm/`)
- [ ] Delete `ori_lang/.github/workflows/deploy-website.yml`
- [ ] Check for any remaining references to `website/` in ori_lang:
  - `.github/workflows/auto-release.yml` references `website/playground-wasm/Cargo.toml`, `website/src/layouts/BaseLayout.astro`, and `website/package.json` (version sync via `sync-version.sh`) — these `git add` paths must be removed
  - `scripts/sync-version.sh` updates versions in `website/playground-wasm/Cargo.toml`, `website/src/layouts/BaseLayout.astro`, and `website/package.json` — these entries must be removed (version sync for the website will need a separate mechanism in ori-lang-website)
  - `.gitignore` entries mentioning `website/`
  - Root `Cargo.toml` or `.cargo/config.toml` (playground-wasm is a standalone workspace, should not be referenced — verified clean)
  - Any scripts in `scripts/` that reference `website/`
- [ ] Update `scripts/sync-version.sh`:
  - Remove `update_cargo_version "$ROOT_DIR/website/playground-wasm/Cargo.toml"` call
  - Remove `update_astro_version "$ROOT_DIR/website/src/layouts/BaseLayout.astro"` call
  - Remove `update_npm_version "$ROOT_DIR/website/package.json"` call
  - Verify `--check` mode still passes after removal
- [ ] Update `auto-release.yml` "Sync versions and commit" step:
  - Remove `website/playground-wasm/Cargo.toml`, `website/src/layouts/BaseLayout.astro`, `website/package.json` from the `git add` command (line ~107-109)
- [ ] Commit the removal

---

## 04.2 Update Governance

**File(s):** `/home/eric/projects/CLAUDE.md`

Follow the warpkit/orijs pattern for approval rules.

- [ ] Add `ori-lang-website` folder to the CLAUDE.md approval rules alongside `orijs` and `warpkit`:
  ```markdown
  **MANDATORY**: No changes may be made to OriJs (orijs folder), WarpKit (warpkit folder),
  or ori-lang-website (ori-lang-website folder) code without explicit user approval for
  EACH individual change. This rule ONLY applies to the orijs, warpkit, and ori-lang-website folders.
  ```
- [ ] Update any memory files that reference `website/` paths (e.g., MEMORY.md)

---

## 04.3 Completion Checklist

- [ ] `ori_lang/website/` does not exist
- [ ] `ori_lang/.github/workflows/deploy-website.yml` does not exist
- [ ] `grep -r "website/" ori_lang/.github/` returns no results (deploy-website.yml deleted, auto-release.yml website paths removed)
- [ ] CLAUDE.md updated with ori-lang-website governance
- [ ] `./test-all.sh` still passes in ori_lang (no broken references)

**Exit Criteria:** The ori_lang repo has no website code, and the website deploys independently from ori-lang-website.
