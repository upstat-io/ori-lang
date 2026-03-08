---
section: "03"
title: "CI/CD Pipeline"
status: not-started
goal: "ori-lang-website deploys automatically via GitHub Actions, triggered by both website changes and ori_lang content changes"
depends_on: ["01", "02"]
sections:
  - id: "03.1"
    title: "Website Deploy Workflow"
    status: not-started
  - id: "03.2"
    title: "Compiler Notify Workflow"
    status: not-started
  - id: "03.3"
    title: "Version Sync Strategy"
    status: not-started
  - id: "03.4"
    title: "GitHub Pages Configuration & DNS Switchover"
    status: not-started
  - id: "03.5"
    title: "Completion Checklist"
    status: not-started
---

# Section 03: CI/CD Pipeline

**Status:** Not Started
**Goal:** The ori-lang-website repo deploys to GitHub Pages automatically when (a) website code is pushed to main, or (b) ori_lang content (docs, spec, plans, blog) changes on master.

**Context:** The current `deploy-website.yml` in ori_lang handles everything: Rust toolchain, wasm-pack, bun, changelog, test results, GitHub Pages. It triggers on `workflow_run` from "Auto Release" (`types: [completed]`, master branch) with an `if` guard that allows `success` or `skipped` conclusions, plus `workflow_dispatch`. It does NOT trigger on direct push. The new setup splits this: ori_lang fires a `repository_dispatch` event, and ori-lang-website handles the full build.

**Reference implementations:**
- **warpkit** `.github/workflows/notify-website.yml`: Fires `repository_dispatch` on docs/guide changes
- **warpkit-website** `.github/workflows/deploy.yml`: Clones warpkit, symlinks to `../warpkit`, builds Astro, deploys to GitHub Pages

**Depends on:** Sections 01, 02 (repo must exist with correct paths before CI can work).

---

## 03.1 Website Deploy Workflow

**File(s):** `ori-lang-website/.github/workflows/deploy.yml`

Create the deploy workflow modeled on warpkit-website's, but with the additional WASM build step and test results artifact download.

- [ ] Create `.github/workflows/deploy.yml` with triggers:
  ```yaml
  on:
    push:
      branches: [main]
    repository_dispatch:
      types: [ori-lang-content-updated]
    workflow_dispatch:
  ```

- [ ] Checkout website repo (default action)

- [ ] Checkout ori_lang repo into workspace:
  ```yaml
  - name: Checkout ori_lang repo
    uses: actions/checkout@v4
    with:
      repository: upstat-io/ori-lang
      path: ori_lang
      fetch-depth: 0  # Full history for changelog generation
      token: ${{ secrets.ORI_LANG_PAT }}
  ```
  **WARNING:** The `${{ secrets.ORI_LANG_PAT }}` is **required** if `ori-lang` is a private repo. The default `github.token` only has access to the repo that triggered the workflow (ori-lang-website). Do NOT use a `|| github.token` fallback -- it will silently fail with a 404 on private repos. Create a PAT (or GitHub App token) with `contents:read` on `upstat-io/ori-lang` and store it as the `ORI_LANG_PAT` secret in ori-lang-website.

- [ ] Create symlink so relative paths resolve:
  ```yaml
  - name: Symlink ori_lang for content collections
    run: ln -s "$GITHUB_WORKSPACE/ori_lang" "$GITHUB_WORKSPACE/../ori_lang"
  ```

- [ ] Fetch test results artifact from ori_lang CI:
  ```yaml
  - name: Fetch test results from CI
    uses: dawidd6/action-download-artifact@v6
    with:
      workflow: ci.yml
      repo: upstat-io/ori-lang
      branch: master
      name: test-results
      path: public
      if_no_artifact_found: warn
      github_token: ${{ secrets.ORI_LANG_PAT }}
    continue-on-error: true
  ```
  **Note:** The `github_token` parameter must use the same `ORI_LANG_PAT` that has access to `upstat-io/ori-lang`. Without it, artifact download from a private repo will fail silently (caught by `continue-on-error`).

- [ ] Validate test results (create fallback JSON if not found)

- [ ] Setup Rust toolchain + wasm-pack for WASM build (use `env.RUST_VERSION` at workflow level for easy updates):
  ```yaml
  env:
    RUST_VERSION: "1.93.1"
  ```
  ```yaml
  - name: Setup Rust
    uses: dtolnay/rust-toolchain@stable
    with:
      toolchain: ${{ env.RUST_VERSION }}
      targets: wasm32-unknown-unknown

  - name: Cache Rust dependencies
    uses: Swatinem/rust-cache@v2
    with:
      workspaces: playground-wasm -> playground-wasm/target
      cache-on-failure: true

  - name: Install wasm-pack
    uses: jetli/wasm-pack-action@v0.4.0
  ```

- [ ] Derive build number from ori_lang:
  ```yaml
  - name: Derive build number
    run: cd ori_lang && ./scripts/bump-build.sh
  ```

- [ ] Build WASM playground:
  ```yaml
  - name: Build WASM
    run: |
      cd playground-wasm
      wasm-pack build --target web --out-dir pkg --release
  ```

- [ ] Setup Bun, install deps, build:
  ```yaml
  - name: Setup Bun
    uses: oven-sh/setup-bun@v2

  - name: Install dependencies
    run: bun install

  - name: Build website
    run: bun run build
  ```
  **Note:** `bun run build` triggers the `prebuild` hook in `package.json`, which handles: copying `install.sh`, running `copy-wasm.sh`, `generate-changelog.sh`, and `build-tutorial.mjs`. The WASM build step (above) MUST complete before this step so that `copy-wasm.sh` can find `playground-wasm/pkg/`. The current CI in ori_lang runs these as separate steps AND via prebuild (redundant); the new pipeline relies solely on prebuild, which is cleaner.

  **WARNING — prebuild ordering:** The `prebuild` script in `package.json` runs: `cp ../ori_lang/install.sh public/install.sh && bash scripts/copy-wasm.sh && bash scripts/generate-changelog.sh && node scripts/build-tutorial.mjs`. This means:
  1. The `install.sh` copy requires the ori_lang symlink/checkout to already exist
  2. The `copy-wasm.sh` step requires `playground-wasm/pkg/` to already be built (WASM step above)
  3. The `generate-changelog.sh` step requires git history in ori_lang (ensured by `fetch-depth: 0`)

  All three preconditions are satisfied by the workflow steps above, BUT only if the steps are ordered correctly. The `bun run build` step must be LAST after all setup steps.

- [ ] Add `permissions` block for GitHub Pages deployment:
  ```yaml
  permissions:
    contents: read
    pages: write
    id-token: write
    actions: read  # Needed to download test-results artifact from ori_lang CI
  ```

- [ ] Add concurrency group to prevent parallel deploys:
  ```yaml
  concurrency:
    group: pages
    cancel-in-progress: true
  ```

- [ ] Upload artifact and deploy to GitHub Pages (same as current)

---

## 03.2 Compiler Notify Workflow

**File(s):** `ori_lang/.github/workflows/notify-website.yml`

Create a workflow in ori_lang that triggers a website rebuild when content changes. Modeled on warpkit's `notify-website.yml`.

- [ ] Create `.github/workflows/notify-website.yml`:
  ```yaml
  name: Notify website of content change

  on:
    push:
      branches: [master]
      paths:
        - 'docs/**'
        - 'plans/**'
        - 'blog/**'
        - 'library/std/**'
        - 'install.sh'
        - 'BUILD_NUMBER'

  jobs:
    notify:
      runs-on: ubuntu-latest
      steps:
        - name: Trigger website rebuild
          run: |
            gh api repos/upstat-io/ori-lang-website/dispatches \
              -f event_type=ori-lang-content-updated
          env:
            GH_TOKEN: ${{ secrets.WEBSITE_PAT }}
  ```

- [ ] Ensure `WEBSITE_PAT` secret exists in ori_lang repo settings (PAT with repo access to ori-lang-website)

**Decision point:** The current `deploy-website.yml` triggers on `workflow_run` from "Auto Release" (which bumps `BUILD_NUMBER` and updates versions). Options for the new setup:
1. **Option A (recommended)**: Trigger `notify-website.yml` on `push` to `master` with path filters (as shown above). The `BUILD_NUMBER` path filter catches Auto Release commits.
2. **Option B**: Also trigger on `workflow_run` from Auto Release. This adds complexity for no benefit since `BUILD_NUMBER` is already in the path filter.

---

## 03.3 Version Sync Strategy

**File(s):** `ori_lang/scripts/sync-version.sh`, `ori-lang-website/` (new mechanism needed)

Currently, `sync-version.sh` updates three files inside `website/`:
- `website/playground-wasm/Cargo.toml` (Cargo version)
- `website/src/layouts/BaseLayout.astro` (softwareVersion)
- `website/package.json` (NPM version)

And `auto-release.yml` explicitly `git add`s these files in the version-bump commit.

After extraction, these files live in `ori-lang-website/` — a separate repo. Options:

- [ ] **Option A (recommended)**: Remove website paths from `sync-version.sh` and `auto-release.yml`. Instead, the ori-lang-website deploy workflow reads `BUILD_NUMBER` from the cloned ori_lang repo at build time and injects the version dynamically. This avoids cross-repo commits entirely.
  - Add a build step in `deploy.yml`: derive version from `ori_lang/BUILD_NUMBER` and update `BaseLayout.astro` + `package.json` inline before `bun run build`
  - `playground-wasm/Cargo.toml` version is cosmetic (not published to crates.io) — can stay static or be updated at build time
- [ ] **Option B**: Create a separate `sync-version.yml` workflow in ori-lang-website that is triggered by `repository_dispatch` with the version payload, commits the version bump, and then triggers deploy. More complex but keeps version numbers in git history.

**Whichever option is chosen, `sync-version.sh` and `auto-release.yml` in ori_lang MUST be updated to remove the website paths (see Section 04.1).**

---

## 03.4 GitHub Pages Configuration & DNS Switchover

**CNAME/DNS Switchover Strategy:**

GitHub Pages allows only ONE repo to have a given custom domain at a time. Switching `ori-lang.com` from `ori_lang` to `ori-lang-website` requires careful sequencing to minimize downtime.

- [ ] **Step 1**: Verify the ori-lang-website deploy works WITHOUT custom domain first (use the default `*.github.io` URL)
- [ ] **Step 2**: Remove custom domain `ori-lang.com` from ori_lang repo settings (GitHub Pages > Custom domain > clear)
- [ ] **Step 3**: Immediately configure custom domain `ori-lang.com` in ori-lang-website repo settings
- [ ] **Step 4**: Verify the `CNAME` file in `ori-lang-website/public/CNAME` contains `ori-lang.com`
- [ ] **Step 5**: DNS CNAME record should NOT need changes (it already points to GitHub Pages: `upstat-io.github.io` or similar). However, verify:
  - `dig CNAME ori-lang.com` should show GitHub Pages
  - If the CNAME points specifically to `upstat-io.github.io` (org-level), no change needed
  - If it points to a repo-specific URL, update may be required
- [ ] **Step 6**: Wait for DNS propagation (usually <5 minutes for CNAME; up to 24h for some resolvers)
- [ ] **Step 7**: Verify HTTPS works on `ori-lang.com` (GitHub auto-provisions Let's Encrypt cert; may take a few minutes)
- [ ] **Step 8**: Disable GitHub Pages entirely on ori_lang repo (Settings > Pages > Source: None)

**Expected downtime**: 1-5 minutes between Step 2 and Step 3. Schedule during low-traffic period.

---

## 03.5 Completion Checklist

- [ ] Push to `main` on ori-lang-website triggers a successful deploy
- [ ] Content change in ori_lang fires `repository_dispatch` to ori-lang-website
- [ ] Website deploys with all pages, playground, changelog, and test results
- [ ] `ori-lang.com` serves content from ori-lang-website GitHub Pages
- [ ] `workflow_dispatch` works for manual rebuilds

**Exit Criteria:** Push a test commit to ori_lang docs, verify the website rebuilds automatically and `ori-lang.com` shows the updated content.
