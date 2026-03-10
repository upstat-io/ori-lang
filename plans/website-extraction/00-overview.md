---
plan: "website-extraction"
title: "Website Extraction: Move ori-lang.com to Separate Repository"
status: in-progress
references:
  - "https://github.com/upstat-io/ori-lang-website"
  - "https://github.com/upstat-io/warpkit-website (reference pattern)"
---

# Website Extraction: Move ori-lang.com to Separate Repository

## Mission

Extract the ori-lang.com website from `ori_lang/website/` into the standalone `ori-lang-website` repository, following the same pattern used by warpkit-website: relative paths for local development, CI clone + symlink for deployment. This decouples website deploys from compiler releases and keeps the compiler repo focused.

## Architecture

```
Local development:
  /home/eric/projects/
  ├── ori_lang/           (compiler, docs, plans, blog, spec, stdlib)
  │   ├── docs/
  │   ├── plans/
  │   ├── blog/
  │   ├── install.sh
  │   └── compiler/       (ori_compiler, ori_eval, ori_diagnostic, ...)
  └── ori-lang-website/   (Astro + Svelte website)
      ├── src/
      │   └── content.config.ts  →  ../ori_lang/docs/*, ../ori_lang/plans/*
      ├── playground-wasm/
      │   └── Cargo.toml         →  ../../ori_lang/compiler/*
      ├── astro.config.mjs
      └── package.json

CI deployment (GitHub Actions):
  $GITHUB_WORKSPACE/              (ori-lang-website checkout)
  ├── ori_lang/                   (cloned into workspace)
  $GITHUB_WORKSPACE/../ori_lang → $GITHUB_WORKSPACE/ori_lang  (symlink so ../ori_lang resolves)
```

## Design Principles

1. **Warpkit pattern parity**: Use the exact same approach — relative paths work locally when repos are siblings; CI creates a symlink to make the same paths resolve.

2. **Content stays in ori_lang**: All docs, spec, plans, blog, and proposals remain in the compiler repo. The website repo only has presentation code (Astro, Svelte, CSS) and the WASM playground build.

3. **One-step local dev**: `bun run dev` in ori-lang-website should work immediately if both repos are cloned as siblings. No manual symlinks or env vars for local development.

## Git History Strategy

**Decision: Fresh history (no git-filter-branch)**

The ori-lang-website repo starts with a clean initial commit containing the copied files. Rationale:
- The website code has relatively short history and few contributors.
- `git filter-branch` / `git subtree split` would carry over compiler commits that are irrelevant to the website.
- The warpkit-website repo used fresh history (precedent).
- The ori_lang repo retains full history of the `website/` directory in its git log for reference.

If history preservation is desired, use `git subtree split --prefix=website/ -b website-history` in ori_lang, then merge that branch into ori-lang-website. This is optional and can be done at any time.

## Rollback Plan

If the migration fails or causes extended downtime:
1. Re-enable GitHub Pages on ori_lang repo (Settings > Pages > Source: GitHub Actions)
2. Add custom domain `ori-lang.com` back to ori_lang repo settings
3. Manually trigger `deploy-website.yml` in ori_lang via `workflow_dispatch`
4. Site should be back within ~5 minutes

The old `website/` directory and `deploy-website.yml` should remain in ori_lang until Section 05 verification is complete.

## Section Dependency Graph

```
01 Repo Setup ──┐
                ├──► 03 CI/CD ──► 04 Cleanup
02 Path Migration┘                    │
                                      ▼
                                 05 Verification
```

- Sections 01 and 02 are independent and can be worked in parallel.
- Section 03 requires both 01 and 02 (needs the repo set up with correct paths).
- Section 04 requires 03 (don't remove from ori_lang until the new pipeline works).
- Section 05 is final verification.

## Implementation Sequence

```
Phase 1 - Setup (parallel)
  └─ 01: Copy website/ contents to ori-lang-website repo
  └─ 02: Update all relative paths from ../ to ../ori_lang/
         (content collections, remark plugins, build scripts, Cargo.toml, error messages in 3 files)
         Clean up debug logging in remark-md-links.mjs

Phase 2 - CI/CD
  └─ 03.1: Create deploy.yml in ori-lang-website (clone + symlink + build)
  └─ 03.2: Create notify-website.yml in ori_lang (dispatch on content changes)
  └─ 03.3: Decide and implement version sync strategy
  └─ 03.4: DNS switchover (CNAME from ori_lang → ori-lang-website)
  Gate: CI deploys successfully to GitHub Pages; ori-lang.com serves from new repo

Phase 3 - Cleanup
  └─ 04.1: Remove website/ from ori_lang (+ update sync-version.sh + auto-release.yml)
  └─ 04.2: Update CLAUDE.md governance rules
  Gate: ori_lang has no website code; ori-lang-website deploys independently

Phase 4 - Verification
  └─ 05: Full verification (local dev, CI, all pages, playground, broken links)
```

## Quick Reference

| ID | Title | File | Status |
|----|-------|------|--------|
| 01 | Repository Setup | `section-01-repo-setup.md` | Complete |
| 02 | Path Migration | `section-02-path-migration.md` | Complete |
| 03 | CI/CD Pipeline | `section-03-ci-cd.md` | In Progress |
| 04 | Cleanup & Governance | `section-04-cleanup.md` | In Progress |
| 05 | Verification | `section-05-verification.md` | Not Started |
