---
section: "06"
title: "LSP Disposition (tools/ori-lsp Atomic Deletion)"
status: not-started
reviewed: false
goal: "Delete `tools/ori-lsp/` (provably unrevivable — imports `oric::ast`, `oric::lexer`, `oric::parser::Parser`, `oric::type_check()`, `oric::format::format()` that no longer exist in the current oric public API per Pass 1 Agent 3's `cargo check` verification) atomically across tier-1 breaking references (Cargo.toml workspace exclude, scripts/sync-version.sh, .github/workflows/auto-release.yml), tier-2 stale comments (.claude/rules/cargo.md, docs/development/versioning.md, compiler/oric/src/lib.rs), and tier-3 full docs rewrites (docs/tooling/lsp/design/ as future-tense architectural vision per user's pre-approved direction)."
success_criteria:
  - "`ls tools/ori-lsp` returns 'No such file or directory' (two-step deletion removes BOTH tracked and untracked files — Finding 3 fix)"
  - "§06.6 commit uses EXPLICIT path staging (no `git add -A`) with worktree cleanliness gate (Finding 4 fix)"
  - "`cargo check --workspace` still passes after Cargo.toml:47-50 edit (workspace exclude removed)"
  - "`./scripts/sync-version.sh --check` passes post-edit (line 187-188 removed)"
  - "`.github/workflows/auto-release.yml` no longer references `tools/ori-lsp/Cargo.toml` in the git add list (line 106-107)"
  - "No Tier-2 stale references remain: grep for `tools/ori-lsp` and `ori-lsp` returns zero matches in `.claude/rules/cargo.md`, `docs/development/versioning.md`, `compiler/oric/src/lib.rs` except approved narrative history references"
  - "`docs/tooling/lsp/design/index.md` and `02-architecture/index.md` rewritten in future tense describing the canonical future home at `compiler/ori_lsp/` — no references to deleted `oric::ast`/`oric::lexer`/`oric::parser::Parser`/`oric::type_check`/`oric::format::format` APIs"
  - "Formatter docs (`docs/tooling/formatter/design/index.md:126,130`, `04-implementation/index.md:436,439`, `integration.md:70,135`) updated to future tense ('when the LSP server is implemented')"
  - "`plans/roadmap/section-22-tooling.md:150-156` LSP subsection updated to reflect deletion of prototype and clarify `compiler/ori_lsp/` is reserved canonical home"
  - "`docs/ori_lang/proposals/approved/lsp-implementation-proposal.md` + `drafts/mcp-semantic-tooling-proposal.md:48` stale references updated"
  - "`./test-all.sh` green post-deletion"
  - "`cargo check --workspace && cargo clippy --workspace --all-targets` both green"
  - "Cargo.toml edit executed via pre-approved user permission (captured in §06.2 as explicit confirmation of the pre-approval from §00 overview Step 9)"
  - "Satisfies mission criterion: '**LSP deletion is atomic**'"
inspired_by:
  - "rustc `rustc_driver_impl` extraction — atomic multi-file refactor with same-commit docs updates"
  - "Swift SIL optimizer pass removal — stale docs rewritten in future tense before deletion lands"
depends_on: ["01", "05"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "06.1"
    title: "Pre-Deletion Re-verification (Unrevivability Proof)"
    status: not-started
  - id: "06.2"
    title: "Tier 1: Critical Reference Updates"
    status: not-started
  - id: "06.3"
    title: "Tier 2: Stale Comment & Doc Reference Updates"
    status: not-started
  - id: "06.4"
    title: "Tier 3: LSP Design Docs Future-Tense Rewrite"
    status: not-started
  - id: "06.5"
    title: "Tier 3 Continued: Formatter + Proposals + Roadmap Updates"
    status: not-started
  - id: "06.6"
    title: "tools/ori-lsp Deletion + Atomic Commit"
    status: not-started
  - id: "06.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "06.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 06: LSP Disposition (tools/ori-lsp Atomic Deletion)

**Status:** Not Started
**Goal:** Execute the provably-correct deletion of `tools/ori-lsp/` as a single atomic operation across the 20+ files that reference it. The prototype is uncompilable (Pass 1 Agent 3 ran `cargo check` and confirmed `oric::ast`, `oric::lexer`, `oric::parser::Parser`, `oric::type_check()`, and `oric::format::format()` all produce E0432/E0433/E0425 errors because those modules/functions no longer exist in the current oric public API). It is NOT "stale"; it is **literally dead code**. The roadmap at `plans/roadmap/section-22-tooling.md:150-156` already claims `compiler/ori_lsp/` as the canonical future home for a future LSP implementation — this plan preserves that claim by rewriting the design docs in future tense rather than deleting them.

**Success Criteria:**

- [ ] `tools/ori-lsp/` directory absent
- [ ] `cargo check --workspace` passes post-edit
- [ ] `cargo clippy --workspace --all-targets` passes post-edit
- [ ] `./scripts/sync-version.sh --check` passes post-edit
- [ ] `.github/workflows/auto-release.yml` does not reference `tools/ori-lsp/Cargo.toml`
- [ ] No tier-2 stale references to `tools/ori-lsp` in `.claude/rules/cargo.md`, `docs/development/versioning.md`, `compiler/oric/src/lib.rs`
- [ ] Tier-3 LSP design docs rewritten in future tense
- [ ] Tier-3 formatter docs updated to future-tense LSP references
- [ ] Tier-3 roadmap + proposals updated
- [ ] `./test-all.sh` green
- [ ] Single atomic commit: `refactor(plan): delete tools/ori-lsp prototype; reserve compiler/ori_lsp/ as canonical future home`
- [ ] Satisfies mission criterion: "**LSP deletion is atomic**"

**Context:** Pass 1 Agent 3 did the unrevivability proof empirically. When the agent ran `cargo check --manifest-path tools/ori-lsp/Cargo.toml`, the output was:

```
error[E0432]: unresolved imports `oric::ast`, `oric::lexer`
error[E0433]: failed to resolve: could not find `Parser` in `parser`
error[E0433]: could not find `format` in `oric`
error[E0425]: cannot find function `type_check` in crate `oric`
```

The current oric crate exports (per `compiler/oric/src/lib.rs:100-107`) only the Salsa-based query interface — `CompilerDb`, `Db`, `EvalOutput`, `ModuleEvalResult`, `SourceFile`, `evaluated` query. The flat `ast`/`lexer`/`parser::Parser`/`type_check`/`format::format` API that `tools/ori-lsp` was built against was removed during the big compiler-internals refactor (exact commit unknown but irrelevant — CLAUDE.md §NEVER investigate "pre-existing?" applies: it does not matter when it broke, only that it IS broken). Reviving it would be a full rewrite, not a fix.

User pre-approved two items relevant to this section (recorded in `00-overview.md` Step 9 review):
1. **Cargo.toml edit** — removing the workspace exclude for `tools/ori-lsp` (lines 47-50) is pre-approved per `.claude/rules/cargo.md`. §06.2 still RECORDS the edit as per-policy (shows the diff in a checklist item) but does NOT re-request approval at execution time.
2. **Doc rewrite approach** — tier-3 rewrites use **future-tense architectural vision** style (option "Rewrite as future-tense architectural vision" from the mission approval AskUserQuestion). Docs describe what the `compiler/ori_lsp/` canonical-home crate WILL look like when implemented, without implying any current implementation exists.

**Reference implementations:**
- **rustc** (`rustc_driver_impl` extraction, 2024): when the driver was extracted into its own crate, all docs and roadmap items referencing the old location were updated in the same commit as the extraction.
- **Swift** (`lib/SILOptimizer/Utils/ArrayOpt.cpp` deletion, circa Swift 5.9): when a stale SIL optimizer utility was deleted, the docs that described it were rewritten as "historical note — this utility was retired when X replaced it" rather than simply deleted, preserving context for future maintainers.

**Depends on:** §01 (baseline proves pre-deletion state), §05 (phantom `.gitkeep` already removed there — this section does NOT duplicate that work).

---

## 06.1 Pre-Deletion Re-verification (Unrevivability Proof)

**File(s):** No edits — verification only

Re-verify that `tools/ori-lsp/` is still unrevivable at plan-execution time. Pass 1 Agent 3 did this verification, but CLAUDE.md §Stabilization Discipline requires fresh verification at execution time — if someone revived the LSP between research and execution (unlikely but possible), the deletion must not proceed.

- [ ] Verify `tools/ori-lsp/` still exists:
  ```bash
  cd /home/eric/projects/ori_lang
  ls tools/ori-lsp/
  # Expected: Cargo.toml Cargo.lock src target
  ```

- [ ] Verify it's still workspace-excluded:
  ```bash
  sed -n '45,55p' Cargo.toml
  # Expected: lines 47-50 comment + exclude entry for tools/ori-lsp
  ```

- [ ] Re-run `cargo check` on the prototype (expected to fail):
  ```bash
  cargo check --manifest-path tools/ori-lsp/Cargo.toml 2>&1 | tee /tmp/ori-lsp-cargo-check.log | tail -20
  echo "Exit: $?"
  ```
  - [ ] Expected: non-zero exit with E0432/E0433/E0425 errors
  - [ ] If `cargo check` SUCCEEDS: STOP. The prototype was revived. Re-evaluate whether §06 should still delete it. This likely means someone did an out-of-plan revive effort; discuss with user before proceeding.

- [ ] Confirm the missing APIs:
  ```bash
  grep -n 'oric::ast\|oric::lexer\|oric::parser::Parser\|oric::type_check\|oric::format' tools/ori-lsp/src/*.rs 2>&1
  # Expected: matches showing which LOC use which dead API
  ```

- [ ] Check the current oric public surface:
  ```bash
  sed -n '95,115p' compiler/oric/src/lib.rs
  # Expected: re-exports CompilerDb, Db, EvalOutput, ModuleEvalResult, SourceFile, evaluated
  # NO mention of ast, lexer, parser::Parser, type_check, format::format
  ```

- [ ] **Subsection close-out (06.1)** — MANDATORY before starting 06.2:
  - [ ] Unrevivability confirmed empirically via cargo check
  - [ ] Missing APIs enumerated
  - [ ] Current oric public surface captured
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — reflect
        on the cargo-check verification approach. Is there a
        `scripts/dev/verify-excluded-crate.sh <path>` helper that takes an
        excluded crate path and reports "still compiles" / "doesn't compile
        with these specific errors"? Future reorgs that delete excluded
        crates will need this exact verification. If yes, add and commit
        via `build(scripts): add verify-excluded-crate.sh — surfaced by
        project-reorganization/section-06.1 retrospective`. If this is a
        genuinely one-time check: document that.

---

## 06.2 Tier 1: Critical Reference Updates

**File(s):** `Cargo.toml`, `scripts/sync-version.sh`, `.github/workflows/auto-release.yml`

These 3 files MUST be updated atomically with the deletion — if deletion happens first, release workflows break; if reference updates happen first, Cargo still tries to exclude a non-existent directory. They all land in the same commit as the actual `rm -rf` in §06.6.

- [ ] **Cargo.toml:47-50 edit (USER PRE-APPROVED per §00 overview Step 9)**:
  ```bash
  sed -n '45,55p' Cargo.toml
  ```
  Confirm lines 47-50 match:
  ```toml
  # Excluded crates (not part of workspace at all):
  # - ori-lsp: Not yet updated to use new compiler
  exclude = [
      "tools/ori-lsp",
  ]
  ```

  Use the `Edit` tool to remove the exclude block entirely (the comment at line 47 and the `exclude = [...]` array). If removing the `exclude = []` entry leaves no other `exclude` usage, remove the whole stanza:
  ```
  old_string:
  # Excluded crates (not part of workspace at all):
  # - ori-lsp: Not yet updated to use new compiler
  exclude = [
      "tools/ori-lsp",
  ]

  new_string:
  (empty — remove the whole block)
  ```

  - [ ] **Pre-approval recorded**: user approved this Cargo.toml edit in the §00 overview Step 9 review (see overview's "consultation history" section)
  - [ ] Verify edit: `sed -n '45,55p' Cargo.toml` no longer shows the exclude block
  - [ ] Verify Cargo.toml still parses: `cargo check --workspace 2>&1 | tail -10` — should compile normally. If Cargo complains about the removed exclude section, it means other exclude entries needed — unlikely, but investigate.

- [ ] **scripts/sync-version.sh:187-188 edit**:
  ```bash
  sed -n '180,200p' scripts/sync-version.sh
  ```
  Confirm lines 187-188 match:
  ```bash
      # ori-lsp (excluded from workspace)
      update_cargo_version "$ROOT_DIR/tools/ori-lsp/Cargo.toml" "$cargo_version" || failed=true
  ```
  Use `Edit` to remove those two lines. Remove BOTH the comment line and the function call.
  - [ ] Verify: `grep -n 'ori-lsp' scripts/sync-version.sh` returns empty
  - [ ] Verify script still parses: `bash -n scripts/sync-version.sh && echo OK`

- [ ] **.github/workflows/auto-release.yml:106-107 edit**:
  ```bash
  sed -n '100,115p' .github/workflows/auto-release.yml
  ```
  The git add line spans multiple entries:
  ```yaml
          git add BUILD_NUMBER Cargo.toml compiler/ori_llvm/Cargo.toml compiler/ori_rt/Cargo.toml \
                  tools/ori-lsp/Cargo.toml editors/vscode-ori/package.json
  ```
  Use `Edit` to remove the `tools/ori-lsp/Cargo.toml ` substring (and fix up the continuation backslash if needed). Resulting lines:
  ```yaml
          git add BUILD_NUMBER Cargo.toml compiler/ori_llvm/Cargo.toml compiler/ori_rt/Cargo.toml \
                  editors/vscode-ori/package.json
  ```
  - [ ] Verify: `grep -n 'ori-lsp' .github/workflows/auto-release.yml` returns empty
  - [ ] Verify YAML syntax: `python3 -c 'import yaml; yaml.safe_load(open(".github/workflows/auto-release.yml"))' && echo OK`

- [ ] Run `./scripts/sync-version.sh --check` to prove the post-edit script still works:
  ```bash
  ./scripts/sync-version.sh --check 2>&1 | tail -10
  ```
  - [ ] Exit code 0
  - [ ] No "file not found" errors

- [ ] **Subsection close-out (06.2)** — MANDATORY before starting 06.3:
  - [ ] All 3 tier-1 files edited and verified
  - [ ] `cargo check --workspace` passes
  - [ ] `./scripts/sync-version.sh --check` passes
  - [ ] YAML still parses
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — reflect
        on the tier-1 atomic-update pattern. Was there friction in the
        "edit, then verify downstream consumers parse" flow? A
        `scripts/dev/atomic-multi-file-edit.sh` helper that takes a set of
        file edits + verification commands and rolls back on failure would
        be overengineered for this specific case (each edit is trivial),
        but the concept is valid for larger refactors. Document:
        "Retrospective 06.2: no tooling gaps — per-file Edit + verify cycle
        is adequate at this scale."

---

## 06.3 Tier 2: Stale Comment & Doc Reference Updates

**File(s):** `.claude/rules/cargo.md`, `docs/development/versioning.md`, `compiler/oric/src/lib.rs`

These files contain stale comments that mention `tools/ori-lsp` as a currently-excluded crate. After deletion, those comments are lies (the crate doesn't exist anymore). Updating them is tier-2 because a stale comment doesn't break the build — but it does mislead readers and constitutes `DRIFT under SSOT` per `impl-hygiene.md`.

- [ ] **`.claude/rules/cargo.md:1`** (the header):
  ```bash
  sed -n '1,10p' .claude/rules/cargo.md
  ```
  Pass 1 Agent 3 noted this file mentions `ori-lsp` in line 1-ish. Read the actual context and update to remove the `tools/ori-lsp` mention. If the text says something like "ori-lsp is fully excluded from the workspace because it hasn't been updated to the new compiler API", update to either remove the mention entirely or add a brief historical note: "tools/ori-lsp was deleted in {commit hash} — LSP work will live at compiler/ori_lsp/ when implemented."
  - [ ] Preferred: remove the mention, since the future-home claim lives in the roadmap
  - [ ] Verify: `grep -n 'ori-lsp\|ori_lsp' .claude/rules/cargo.md` returns zero matches

- [ ] **`docs/development/versioning.md:67`** (table row):
  ```bash
  sed -n '63,75p' docs/development/versioning.md
  ```
  Expected: a table row `| tools/ori-lsp/Cargo.toml | CalVer → Cargo (excluded from workspace) |` or similar. Delete that row.
  - [ ] Verify: `grep -n 'ori-lsp\|ori_lsp' docs/development/versioning.md` returns zero matches
  - [ ] Note: §02 already fixed `versioning.md:53` (website path). This is a DIFFERENT line on the same file — verify §02's fix is still present post-edit.

- [ ] **`compiler/oric/src/lib.rs:96`** (comment):
  ```bash
  sed -n '90,105p' compiler/oric/src/lib.rs
  ```
  Expected: a comment like `// Re-exports: only types actually consumed by external crates (ori-lsp, benches)`. Update to remove the `ori-lsp` mention. Options:
  - Simpler: `// Re-exports: only types actually consumed by external crates (benches)`
  - Most accurate: `// Re-exports: only types actually consumed by external crates`
  - [ ] Use `Edit` tool with the exact old_string; verify the diff shows only the one-line change
  - [ ] Verify: `grep -n 'ori-lsp\|ori_lsp' compiler/oric/src/lib.rs` returns zero matches
  - [ ] Verify `cargo check -p oric` still passes (the comment change cannot affect compilation, but double-check)

- [ ] **Subsection close-out (06.3)** — MANDATORY before starting 06.4:
  - [ ] All 3 tier-2 files updated
  - [ ] Zero `tools/ori-lsp` references remain in those files
  - [ ] `cargo check -p oric` green
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — reflect
        on the "find stale comment, delete/rewrite" pattern. Is there a
        tool that surfaces "comments that reference deleted code paths"?
        Probably not feasible without a full semantic analysis, but worth
        documenting the gap. Document: "Retrospective 06.3: stale-comment
        detection is an open problem — no feasible tooling at this scale
        without semantic analysis; manual per-reference review is the
        current state of the art."

---

## 06.4 Tier 3: LSP Design Docs Future-Tense Rewrite

**File(s):** `docs/tooling/lsp/design/index.md`, `docs/tooling/lsp/design/02-architecture/index.md`, `docs/tooling/lsp/design/02-architecture/wasm.md`, `docs/tooling/lsp/design/04-integration/editors.md`, and any other `docs/tooling/lsp/design/**/*.md` files that reference `tools/ori-lsp` or the dead API surface.

**User-approved direction (from §00 overview Step 9):** "Rewrite as future-tense architectural vision — the docs become 'this is what the FUTURE compiler/ori_lsp crate will look like when implemented'. Current content describes an unimplemented crate."

This is the biggest work item in §06. The docs are not just stale-reference-updates — they need their prose rewritten to be factually accurate (the LSP does not exist today) while preserving the architectural thinking (the design is good, it just needs the future implementation to be built against the current oric query API).

- [ ] **`docs/tooling/lsp/design/index.md` — full review + rewrite**:
  ```bash
  wc -l docs/tooling/lsp/design/index.md
  cat docs/tooling/lsp/design/index.md
  ```
  Identify each paragraph that asserts current implementation (e.g., "The LSP server is implemented at `tools/ori-lsp/`..."). Rewrite each to future tense:
  - "The LSP server is implemented at `tools/ori-lsp/` using tower-lsp..." → "The future LSP server will live at `compiler/ori_lsp/` (reserved canonical home, currently unpopulated). When implemented, it will use tower-lsp..."
  - "✅ Diagnostics (lex, parse, and type errors via `oric::type_check()`)" → "Planned: diagnostics via the current oric Salsa query API (`CompilerDb::evaluated` + lexer/parser/type-checker queries); the old flat API (`oric::type_check()`) was removed and the future LSP will need to integrate with the query interface instead."
  - Any dependency-diagram or code sample that uses `oric::ast`/`oric::lexer`/`oric::parser::Parser`/`oric::type_check`/`oric::format::format` → either rewrite against the current API or mark as "pseudo-code — needs re-design against query interface when implementation begins."
  - Add a header banner at the top of `index.md`: `> **Status:** Future work. The prototype at `tools/ori-lsp/` was deleted on {date} because it was built against a removed compiler API (see `compiler/oric/src/lib.rs` for the current public surface). When the LSP is re-implemented, it will live at `compiler/ori_lsp/`.`
  - [ ] Use `Edit` or `Write` on the file. `Write` is cleaner for a full rewrite if the diff is substantial (>30% of content changes).

- [ ] **`docs/tooling/lsp/design/02-architecture/index.md` — full review + rewrite**:
  Same treatment as index.md. The architecture doc has detailed code examples using the dead API — those either get rewritten against the query API (hard, would require deep design work) or marked as "pseudo-code illustrating concept; concrete API TBD during implementation."
  - Add the same future-tense banner at the top
  - Rewrite the line at `docs/tooling/lsp/design/02-architecture/index.md:10` ("Note: The crate is located at `tools/ori-lsp/`, not `compiler/ori_lsp/`") → "Note: The crate's canonical future home is `compiler/ori_lsp/` (currently reserved but unpopulated). A prior prototype at `tools/ori-lsp/` was deleted — see section-06 of plans/project-reorganization/ for context."
  - [ ] Rewrite all code examples using `oric::ast`, `lexer::tokenize`, `parser::Parser::new`, etc. to either remove them or mark them as illustrative pseudo-code

- [ ] **`docs/tooling/lsp/design/02-architecture/wasm.md:33`**:
  ```bash
  sed -n '25,45p' docs/tooling/lsp/design/02-architecture/wasm.md
  ```
  Change "From `tools/ori-lsp/`" to "From the future `compiler/ori_lsp/` crate (reserved canonical home)". WASM build guidance is still technically valid — the `wasm-pack` / `wasm-bindgen` commands will apply to any Rust crate.

- [ ] **`docs/tooling/lsp/design/04-integration/editors.md`**:
  ```bash
  grep -n 'ori-lsp\|ori_lsp\|tools/ori-lsp' docs/tooling/lsp/design/04-integration/editors.md
  ```
  Pass 1 identified multiple references (lines 24-26, 334-345, etc.). Update each:
  - Binary filenames like `ori_lsp-linux`, `ori_lsp-macos` — keep (these are the eventual binary names, agnostic to crate location)
  - Build commands like `cargo build -p ori_lsp` — update to "cargo build -p ori_lsp (when the crate is implemented at compiler/ori_lsp/)"
  - References to `tools/ori-lsp/` path → `compiler/ori_lsp/`

- [ ] **`docs/tooling/lsp/design/04-integration/index.md`**:
  Similar treatment — update all path references, keep binary-name and protocol content which is agnostic to crate location.

- [ ] **`docs/tooling/lsp/design/04-integration/playground.md:101`**:
  Preserve the WASM import line (`ori_lsp.js`) since the future binary name is still expected to be `ori_lsp`.

- [ ] **`docs/tooling/lsp/design/01-protocol/**`** and **`03-features/**`**:
  Scan for `tools/ori-lsp` mentions:
  ```bash
  rg -n 'tools/ori-lsp\|`ori-lsp`\|`tools/ori-lsp`' docs/tooling/lsp/design/01-protocol/ docs/tooling/lsp/design/03-features/
  ```
  Update each reference to future tense or remove.

- [ ] Global post-check — no tier-3 LSP design doc mentions `tools/ori-lsp`:
  ```bash
  rg -n 'tools/ori-lsp' docs/tooling/lsp/design/
  # Expected: zero matches
  ```

- [ ] **Subsection close-out (06.4)** — MANDATORY before starting 06.5:
  - [ ] Every file in `docs/tooling/lsp/design/` has been reviewed
  - [ ] No `tools/ori-lsp` references remain
  - [ ] Code examples using dead APIs are either removed or explicitly marked as pseudo-code
  - [ ] Future-tense banners added to `index.md` and `02-architecture/index.md`
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — reflect
        on the large-scale doc rewrite journey. Was there friction in
        keeping track of which paragraphs were "fact-level stale" vs
        "architecture-level still valid"? Is there a doc-review-checklist
        pattern that would help? Probably: a
        `scripts/dev/doc-rewrite-audit.sh <dir> <deprecated-path>` that
        greps for the deprecated path and lists every hit with line
        context, so the reviewer can systematically decide each hit. If
        yes, build it now and commit via `build(scripts): add
        doc-rewrite-audit.sh — surfaced by project-reorganization/
        section-06.4 retrospective`. This is broadly valuable for any
        future doc rewrite.

---

## 06.5 Tier 3 Continued: Formatter + Proposals + Roadmap Updates

**File(s):** `docs/tooling/formatter/design/index.md`, `docs/tooling/formatter/design/04-implementation/index.md`, `docs/tooling/formatter/integration.md`, `docs/ori_lang/proposals/approved/lsp-implementation-proposal.md`, `docs/ori_lang/proposals/drafts/mcp-semantic-tooling-proposal.md`, `plans/roadmap/section-22-tooling.md`

Pass 1 Agent 3 found stale `ori_lsp` / `tools/ori-lsp` references in these files beyond the LSP design docs subtree. Each gets a small targeted update.

- [ ] **`docs/tooling/formatter/design/index.md:126, 130`** — "`ori-lsp`" and "Same `ori_lsp` binary serves VS Code..." references:
  ```bash
  sed -n '120,135p' docs/tooling/formatter/design/index.md
  ```
  Update to future tense: "When the LSP server is implemented at `compiler/ori_lsp/`, it will serve formatting via the `textDocument/formatting` request. The formatter crate (`ori_fmt`) is designed to be consumed by that future LSP through the Salsa query interface."
  - [ ] Verify: `grep -n 'ori-lsp\|ori_lsp' docs/tooling/formatter/design/index.md` returns only future-tense mentions or zero

- [ ] **`docs/tooling/formatter/design/04-implementation/index.md:436, 439`** — "// In ori_lsp" comment and nearby:
  ```bash
  sed -n '430,445p' docs/tooling/formatter/design/04-implementation/index.md
  ```
  Update to future tense. Keep the architectural intent — the formatter is designed to be callable from an LSP context — but mark it as "future integration point."

- [ ] **`docs/tooling/formatter/integration.md:70, 135`** — "When `ori_lsp` is available..." references:
  ```bash
  sed -n '65,80p' docs/tooling/formatter/integration.md
  sed -n '130,145p' docs/tooling/formatter/integration.md
  ```
  Already uses "when available" phrasing — confirm the tense is future and clarify that the future LSP will live at `compiler/ori_lsp/`.

- [ ] **`docs/ori_lang/proposals/approved/lsp-implementation-proposal.md`** — references to `plans/ori_lsp/` and `compiler/ori_lsp/`:
  ```bash
  grep -n 'ori_lsp\|ori-lsp\|tools/ori-lsp' docs/ori_lang/proposals/approved/lsp-implementation-proposal.md
  ```
  This is an approved proposal. Updates should be conservative — add a header note clarifying that implementation is pending, update specific-path references to reflect the post-§06 state, but do NOT rewrite the proposal's design content.
  - Add header note at top: `> **Implementation status (as of {date}):** Prototype at `tools/ori-lsp/` was deleted (see plans/project-reorganization/section-06). Canonical future home: `compiler/ori_lsp/` (reserved but unpopulated). This proposal's design remains approved and will guide future implementation.`

- [ ] **`docs/ori_lang/proposals/drafts/mcp-semantic-tooling-proposal.md:48`** — "ori-lsp (long-running, warm Salsa DB)" reference:
  ```bash
  sed -n '45,55p' docs/ori_lang/proposals/drafts/mcp-semantic-tooling-proposal.md
  ```
  Update to "the future LSP (planned for `compiler/ori_lsp/`, long-running, warm Salsa DB)" or similar.

- [ ] **`plans/roadmap/section-22-tooling.md:150-156`** — LSP subsection header:
  ```bash
  sed -n '145,165p' plans/roadmap/section-22-tooling.md
  ```
  This is the canonical-home declaration. Update to reflect the §06 cleanup:
  - Section should say something like: "**22.2 LSP** — canonical home reserved at `compiler/ori_lsp/` (currently empty). A prior prototype at `tools/ori-lsp/` was deleted in plans/project-reorganization/section-06 because it was built against a removed compiler API. Re-implementation will use the current oric Salsa query interface."
  - Keep the roadmap commitment that LSP is a future tooling section
  - [ ] Do NOT move or rename the subsection — keep the 22.2 anchor stable so other cross-refs survive

- [ ] Global post-check — zero stale `tools/ori-lsp` references:
  ```bash
  rg -n 'tools/ori-lsp' . 2>&1 \
    | grep -v '^plans/project-reorganization/' \
    | grep -v '^tools/ori-lsp' \
    | grep -v '^target/' \
    | grep -v '^target-llvm/'
  # Expected: empty output (all references either in this plan itself, inside tools/ori-lsp itself which is about to be deleted, or in build-output dirs)
  ```
  - [ ] If any surviving reference: STOP, investigate, update before proceeding

- [ ] **Subsection close-out (06.5)** — MANDATORY before starting 06.6:
  - [ ] All 6 tier-3 files beyond the design subtree updated
  - [ ] Global grep returns zero stale references
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — reflect
        on the scattered-reference sweep. This was a mini-version of what
        §08 will do at much larger scale (747 refs). Is the
        `scripts/dev/plan-ref-sweep.sh` helper from §02.3's retrospective
        (if built) applicable here? Verify its reusability. If no helper
        was built in §02.3, is this the right time to build it — given
        §08 will need it too? Consider building now. Commit via
        `build(scripts): add plan-ref-sweep.sh — surfaced by
        project-reorganization/section-06.5 retrospective` (if not already
        done). Otherwise: document decision.

---

## 06.6 tools/ori-lsp Deletion + Atomic Commit

**File(s):** `tools/ori-lsp/` (DELETED)

Now delete the actual `tools/ori-lsp/` directory and commit everything from §06 as a single atomic commit.

- [ ] Final sanity check before deletion:
  ```bash
  cd /home/eric/projects/ori_lang
  # 1. All tier-1/2/3 edits in place
  git status --short | head -40

  # 2. Cargo still parses
  cargo check --workspace 2>&1 | tail -5

  # 3. Test suite still green (pre-deletion — we haven't deleted the prototype yet)
  timeout 150 ./test-all.sh 2>&1 | tail -10
  ```
  - [ ] All checks pass

- [ ] Delete the prototype directory.
  **CRITICAL FINDING (Phase 2 dual-source review, 2026-04-11):** `git rm -rf` only removes TRACKED files. The `tools/ori-lsp/` directory has untracked residue:
  - `tools/ori-lsp/target/` — cargo build output (created by any prior `cargo check` / `cargo build` in the prototype subtree)
  - Potentially `tools/ori-lsp/Cargo.lock` modifications

  A bare `git rm -rf tools/ori-lsp/` would leave an on-disk `tools/ori-lsp/` directory surviving with the untracked `target/` inside it, causing §06.N success criterion `ls tools/ori-lsp` returns "No such file or directory" to FAIL. The fix is to do BOTH operations in sequence:
  ```bash
  # Step 1: git rm -rf removes tracked files from index (Cargo.toml, Cargo.lock, src/*.rs)
  git rm -rf tools/ori-lsp/
  
  # Step 2: plain rm -rf removes remaining untracked files (target/, any editor scratch files)
  # and the now-empty directory itself
  rm -rf tools/ori-lsp/
  ```
  - Note: `tools/` directory itself stays (other tools may live there in the future).
  - Note: the two-step pattern is required because `git rm` only touches the index; `rm` only touches the working tree. Neither alone is sufficient when untracked files live inside the deletion target.

- [ ] Verify deletion:
  ```bash
  ls tools/ 2>&1
  # Expected: tools/ still exists but ori-lsp is gone
  ls tools/ori-lsp/ 2>&1 || echo "absent"
  # Expected: "absent" (directory is fully gone, not just emptied of tracked files)
  git ls-files tools/ori-lsp/ 2>&1
  # Expected: empty (no tracked files)
  ```

- [ ] Post-deletion verification — everything STILL compiles/runs:
  ```bash
  cargo check --workspace 2>&1 | tail -10
  # Expected: compiles cleanly
  cargo clippy --workspace --all-targets 2>&1 | tail -10
  # Expected: no new clippy errors
  ./scripts/sync-version.sh --check 2>&1 | tail -5
  # Expected: all versions in sync
  timeout 150 ./test-all.sh 2>&1 | tail -10
  # Expected: green
  ```

- [ ] **Pre-commit worktree cleanliness gate (Phase 2 LEAK finding, 2026-04-11):** Before staging, verify the working tree contains ONLY the expected §06 edits. `git add -A` is banned for this commit because it would silently pick up unrelated in-flight edits (plan files from concurrent sections, contributor scratch files, etc.). Enumerate expected staged paths and abort if unexpected dirty files are present:
  ```bash
  cd /home/eric/projects/ori_lang
  
  # Expected paths for §06 commit — list every file this section was supposed to touch:
  # Tier 1: Cargo.toml, scripts/sync-version.sh, .github/workflows/auto-release.yml
  # Tier 2: .claude/rules/cargo.md, docs/development/versioning.md, compiler/oric/src/lib.rs
  # Tier 3 design: docs/tooling/lsp/design/index.md, 02-architecture/*.md, 04-integration/*.md, etc.
  # Tier 3 formatter: docs/tooling/formatter/design/index.md, 04-implementation/index.md, integration.md
  # Tier 3 proposals/roadmap: docs/ori_lang/proposals/approved/lsp-implementation-proposal.md,
  #                           docs/ori_lang/proposals/drafts/mcp-semantic-tooling-proposal.md,
  #                           plans/roadmap/section-22-tooling.md
  # Deletion: tools/ori-lsp/* (all entries)
  # Plan sync: plans/project-reorganization/section-06-lsp-disposition.md,
  #            plans/project-reorganization/00-overview.md,
  #            plans/project-reorganization/index.md (if updated)
  
  git status --porcelain \
    | grep -vE '^[ ADMR]. (Cargo\.toml|scripts/sync-version\.sh|\.github/workflows/auto-release\.yml)$' \
    | grep -vE '^[ ADMR]. (\.claude/rules/cargo\.md|docs/development/versioning\.md|compiler/oric/src/lib\.rs)$' \
    | grep -vE '^[ ADMR]. docs/tooling/lsp/design/' \
    | grep -vE '^[ ADMR]. docs/tooling/formatter/' \
    | grep -vE '^[ ADMR]. docs/ori_lang/proposals/' \
    | grep -vE '^[ ADMR]. plans/roadmap/section-22-tooling\.md$' \
    | grep -vE '^[ ADMR]. plans/project-reorganization/' \
    | grep -vE '^D. tools/ori-lsp/' \
    | grep -vE '^ D tools/ori-lsp/' \
    || echo "CLEAN — only expected §06 paths are dirty"
  ```
  - [ ] Output is "CLEAN — only expected §06 paths are dirty"
  - [ ] If ANY unexpected path appears: STOP. Investigate whether it's a concurrent edit from another section (shelve it first) or a legitimate §06 edit the allow-list missed (add to the allow-list)

- [ ] Commit everything from §06 as a single atomic commit — use EXPLICIT path staging, NOT `git add -A` (banned per Phase 2 LEAK finding):
  ```bash
  cd /home/eric/projects/ori_lang
  
  # Stage tier-1 files explicitly (Cargo.toml requires user pre-approval per §06.2)
  git add Cargo.toml
  git add scripts/sync-version.sh
  git add .github/workflows/auto-release.yml
  
  # Stage tier-2 files explicitly
  git add .claude/rules/cargo.md
  git add docs/development/versioning.md
  git add compiler/oric/src/lib.rs
  
  # Stage tier-3 LSP design doc tree
  git add docs/tooling/lsp/design/
  
  # Stage tier-3 formatter integration docs
  git add docs/tooling/formatter/design/index.md
  git add docs/tooling/formatter/design/04-implementation/index.md
  git add docs/tooling/formatter/integration.md
  
  # Stage tier-3 proposals + roadmap
  git add docs/ori_lang/proposals/approved/lsp-implementation-proposal.md
  git add docs/ori_lang/proposals/drafts/mcp-semantic-tooling-proposal.md
  git add plans/roadmap/section-22-tooling.md
  
  # Stage the deletion (git rm already updated the index, but `git add` on the path
  # ensures any residual untracked children are not accidentally added; git rm -rf
  # already removed tracked children; the plain rm -rf removed untracked children)
  git add -u tools/ori-lsp/ 2>/dev/null || true
  
  # Stage the plan sync updates for §06 (status → complete)
  git add plans/project-reorganization/section-06-lsp-disposition.md
  git add plans/project-reorganization/00-overview.md
  git add plans/project-reorganization/index.md
  
  # Sanity check — show what's staged before committing
  git status --short | head -50
  
  git commit -m "refactor(plan): delete tools/ori-lsp prototype; reserve compiler/ori_lsp/ as canonical future home

The tools/ori-lsp/ prototype was provably unrevivable — it imports
oric::ast, oric::lexer, oric::parser::Parser, oric::type_check(), and
oric::format::format(), none of which exist in the current oric public
API (which is Salsa-query-based). 'cargo check' emits E0432/E0433/E0425.

This commit atomically deletes the prototype AND updates every file
that references it, across three tiers:

Tier 1 (deletion would break without these):
- Cargo.toml:47-50 (workspace exclude removed — user pre-approved per cargo.md)
- scripts/sync-version.sh:187-188 (stop trying to sync a dead file)
- .github/workflows/auto-release.yml:106-107 (stop trying to git add a dead file)

Tier 2 (stale references):
- .claude/rules/cargo.md (removed ori-lsp mention)
- docs/development/versioning.md (removed table row)
- compiler/oric/src/lib.rs:96 (updated re-export comment)

Tier 3 (future-tense rewrites per user-approved direction):
- docs/tooling/lsp/design/ (index.md, 02-architecture/, 04-integration/ —
  future-tense architectural vision, code examples marked pseudo-code)
- docs/tooling/formatter/design/ (formatter↔LSP integration points updated)
- docs/ori_lang/proposals/approved/lsp-implementation-proposal.md (header note added)
- docs/ori_lang/proposals/drafts/mcp-semantic-tooling-proposal.md:48
- plans/roadmap/section-22-tooling.md:150-156 (22.2 LSP subsection updated)

The compiler/ori_lsp/ directory reservation is preserved via the
plans/roadmap/section-22-tooling.md commitment. The .gitkeep phantom was
already deleted in section-05.

Refs: plans/project-reorganization/section-06-lsp-disposition.md"
  ```

- [ ] Final verification:
  ```bash
  git log --oneline -1
  git show --stat HEAD | head -40
  rg -n 'tools/ori-lsp' . 2>&1 | grep -v '^plans/project-reorganization/' || echo "clean"
  ```
  - [ ] Commit landed
  - [ ] Stat shows ~20 file changes (tier-1 edits + tier-2 edits + tier-3 rewrites + deletion)
  - [ ] Zero surviving references to `tools/ori-lsp` outside this plan dir

- [ ] **Subsection close-out (06.6)** — MANDATORY before 06.R:
  - [ ] `tools/ori-lsp/` directory gone
  - [ ] Workspace compiles cleanly
  - [ ] All tier-1/2/3 changes landed atomically
  - [ ] Commit hash recorded
  - [ ] `./test-all.sh` green
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — reflect
        on the atomic-deletion execution. Was the "stage everything, verify,
        commit" workflow smooth? Is there friction in remembering to run
        cargo check + clippy + sync-version --check + test-all post-edit
        each time? A `scripts/dev/verify-everything.sh` helper that runs
        all four checks in sequence and reports pass/fail per tool would
        be broadly useful for post-refactor verification — far beyond this
        plan. Strongly consider building now. Commit via
        `build(scripts): add verify-everything.sh — surfaced by
        project-reorganization/section-06.6 retrospective`. Otherwise
        document the decision.

---

## 06.R Third Party Review Findings

<!-- Reserved for dual-source /tpr-review findings. -->

- None.

---

## 06.N Completion Checklist

- [ ] `tools/ori-lsp/` absent
- [ ] All tier-1 files updated (Cargo.toml, sync-version.sh, auto-release.yml)
- [ ] All tier-2 files updated (.claude/rules/cargo.md, versioning.md, oric/src/lib.rs)
- [ ] All tier-3 LSP design docs rewritten in future tense
- [ ] All tier-3 formatter/proposals/roadmap references updated
- [ ] `cargo check --workspace` green
- [ ] `cargo clippy --workspace --all-targets` green
- [ ] `./scripts/sync-version.sh --check` green
- [ ] `./test-all.sh` green
- [ ] `rg 'tools/ori-lsp' .` returns zero matches outside this plan directory
- [ ] Single atomic commit lands
- [ ] Plan annotation cleanup: N/A (the edit to `compiler/oric/src/lib.rs:96` is a comment change, not a plan annotation)
- [ ] **Plan sync** — update plan metadata:
  - [ ] This section's frontmatter `status` → `complete`
  - [ ] `00-overview.md` Quick Reference table for §06 → `Complete`
  - [ ] `00-overview.md` mission success criterion "LSP deletion is atomic" → checked
  - [ ] `00-overview.md` Known Bugs row for `tools/ori-lsp unrevivable` → `Fixed`
  - [ ] `00-overview.md` Known Bugs row for `compiler/ori_lsp/.gitkeep phantom` → `Fixed` (cross-reference §05)
  - [ ] `index.md` updated
- [ ] `/tpr-review` passed — HIGH-complexity section with 20+ file changes; review should check: (a) no tier-1 file missed, (b) docs rewrites are coherent future-tense not just tense-changed, (c) zero stale refs escaped the sweep
- [ ] `/impl-hygiene-review` passed (AFTER TPR clean) — specifically check SSOT compliance (no duplicated LSP status claims across the 20+ updated files), DRIFT (all references now consistent), and BLOAT (did the docs rewrite add unnecessary content beyond the fact update?)
- [ ] `/improve-tooling` **section-close sweep** — per-subsection retrospectives (06.1-06.6) should already be committed. Cross-subsection pattern check: did ANY subsection reveal that the 20-file atomic-edit workflow needs better tooling? The `verify-everything.sh` helper from 06.6 is likely the most broadly valuable. Verify it was built. Also check: was the "tier 1 / tier 2 / tier 3" classification useful enough that future plans should adopt it? If yes, document in the plan schema (`.claude/skills/create-plan/plan-schema.md`). If not: "Section-06 close sweep: per-subsection retrospectives covered the verification and ref-sweep tooling; the tier classification was plan-local and does not generalize."

**Exit Criteria:** `ls tools/` shows `tools/` still exists but `ori-lsp/` is gone. `cargo check --workspace && cargo clippy --workspace --all-targets && ./scripts/sync-version.sh --check && ./test-all.sh` all exit 0. `rg 'tools/ori-lsp' .` returns only this plan's own references. A single commit (`refactor(plan): delete tools/ori-lsp prototype; reserve compiler/ori_lsp/ as canonical future home`) contains ~20 file changes. Mission success criterion "LSP deletion is atomic" is satisfied.
