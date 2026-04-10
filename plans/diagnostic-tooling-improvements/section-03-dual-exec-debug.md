---
section: "03"
title: "Enhance dual-exec-debug.sh"
status: not-started
reviewed: true
goal: "Auto-dump ARC IR and run codegen-audit on mismatch, export ORI_BIN for child script consistency, add ARC dump to build-failure branch, bridge the gap between 'these differ' and 'here is why'"
success_criteria:
  - "On mismatch, dual-exec-debug.sh auto-dumps ARC IR via arc-dump.sh alongside LLVM IR"
  - "On mismatch, dual-exec-debug.sh runs codegen-audit.sh and displays findings with proper exit-code handling"
  - "ORI_BIN is exported before calling any child script so all helpers use the same binary"
  - "On build failure, ARC IR is still captured (arc-dump.sh can capture pre-codegen IR even when LLVM fails)"
  - "codegen-audit.sh temp artifact side-effect is neutralized when called from dual-exec-debug.sh"
  - "self-test.sh updated with mismatch-path test coverage (not just match-path)"
inspired_by:
  - "Zig --verbose-llvm-ir — automatically shows relevant IR on compilation failure"
  - "diagnose-aot.sh — canonical pattern for ORI_BIN export, codegen-audit exit-code handling, ARC dump"
depends_on: []
third_party_review:
  status: resolved
  updated: 2026-04-09
sections:
  - id: "03.1"
    title: "Export ORI_BIN for child script consistency"
    status: not-started
  - id: "03.2"
    title: "Add ARC IR dump to build-failure branch"
    status: not-started
  - id: "03.3"
    title: "Add ARC IR and codegen-audit to mismatch diagnostics"
    status: not-started
  - id: "03.4"
    title: "Add mismatch-path self-test coverage"
    status: not-started
  - id: "03.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "03.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 03: Enhance dual-exec-debug.sh

**Status:** Not Started
**Goal:** When `dual-exec-debug.sh` detects a mismatch between interpreter and AOT, automatically dump ARC IR (which shows the pre-codegen state — often where AIMS bugs are visible) and run codegen-audit (which catches RC/COW/ABI issues statically). Also fix the `ORI_BIN` export leak, add ARC IR capture on build failure, and add mismatch-path self-test coverage.

**Success Criteria:**
- [ ] `ORI_BIN` is exported immediately after `find_ori_bin` so child scripts (`arc-dump.sh`, `codegen-audit.sh`) resolve the same binary
- [ ] On build failure, ARC IR is captured and saved (arc-dump.sh captures IR before codegen)
- [ ] On mismatch, ARC IR is saved to `$tmpdir/diag-arc.txt` and reported
- [ ] On mismatch, `codegen-audit.sh` runs with proper exit-code handling (0=clean, 1=findings, 2=infra-failure) — not `|| true`
- [ ] `codegen-audit.sh` does not leave temp artifacts in the user's working directory when called from this script
- [ ] Build-failure exit code is 2 (not 1), matching the documented exit contract
- [ ] `--keep-temp` flag preserves diagnostic artifacts on mismatch for user inspection
- [ ] On match, no extra work is done (no performance penalty for passing cases)
- [ ] `self-test.sh` has mismatch-path AND build-failure-path tests using a deterministic harness (no reliance on real compiler bugs)
- [ ] Satisfies mission criterion: "dual-exec-debug.sh auto-dumps ARC IR on mismatch"

**Context:** `dual-exec-debug.sh` currently auto-dumps LLVM IR and RC stats on mismatch (lines 240-262), but never dumps ARC IR even though `arc-dump.sh` exists. Many AIMS bugs — wrong RC placement, missing drops, incorrect ownership annotations — are visible in ARC IR before LLVM codegen faithfully replicates them. Adding ARC IR to the mismatch diagnostics bridges the gap from "these outputs differ" to "here is the ARC-level decision that caused the divergence."

Additionally, the script has a binary-consistency leak: it calls `find_ori_bin` at line 31 to set `$ORI`, but child scripts (`arc-dump.sh` calls `find_any_ori_bin`, `codegen-audit.sh` calls `find_ori_bin`) each re-resolve independently. Without `export ORI_BIN="$ORI"`, child scripts may pick a different binary (e.g., debug vs release). `diagnose-aot.sh` already fixed this exact class of bug at line 204.

**Depends on:** None.

---

## 03.1 Export ORI_BIN for child script consistency

**File(s):** `diagnostics/dual-exec-debug.sh`

**Pattern:** Follow `diagnose-aot.sh` line 204 exactly.

**Why:** `dual-exec-debug.sh` calls `find_ori_bin` at line 31, which sets `$ORI`. But `arc-dump.sh` (called in 03.2/03.3) uses `find_any_ori_bin()`, and `codegen-audit.sh` (called in 03.3) uses `find_ori_bin()`. Both check `$ORI_BIN` env var first. Without exporting, child scripts re-resolve independently and may choose a different binary (e.g., release when the parent chose debug). `diagnose-aot.sh` already has the canonical fix at line 204: `export ORI_BIN="$ORI"`.

- [ ] Immediately after `find_ori_bin` (line 31), add:
  ```bash
  # Export ORI_BIN so child scripts (arc-dump.sh, codegen-audit.sh) use the
  # same binary we resolved, rather than re-resolving via their own
  # find_ori_bin/find_any_ori_bin which may choose a different build profile.
  export ORI_BIN="$ORI"
  ```
- [ ] Verify: with both debug and release binaries present, confirm child scripts use the same binary as the parent (add `echo "Using: $ORI" >&2` temporarily to both parent and child, run, compare paths)

- [ ] **Subsection close-out (03.1)** — MANDATORY before starting 03.2:
  - [ ] All tasks above are `[x]` and verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**

---

## 03.2 Add ARC IR dump to build-failure branch

**File(s):** `diagnostics/dual-exec-debug.sh`

**Why:** The build-failure branch (lines 158-169) currently exits without any IR capture. But `arc-dump.sh` can capture ARC IR even when the build fails — ARC IR is emitted before LLVM codegen, so if codegen fails (e.g., LLVM IR verification error), the ARC IR is still available. This is valuable because build failures often stem from ARC-level issues that are visible in the ARC IR.

**Pattern:** `arc-dump.sh` lines 115-139 already handle build failure gracefully — it captures IR before the failure and prints a warning. Lean on this behavior.

- [ ] **Fix build-failure exit code** (DRIFT: lines 22-25 document `exit 2 = usage error or infrastructure failure`, but line 168 exits 1 on build failure — conflating "build failed" with "mismatch"). Change `exit 1` at line 168 to `exit 2` to match the documented exit contract. Build failure is an infrastructure failure, not a backend mismatch.
- [ ] Inside the build-failure block (after line 159 `echo -e "  ${C_RED}Build failed${C_NC}:"`), before the Summary section, add ARC IR capture:
  ```bash
  # Attempt ARC IR capture even on build failure — ARC IR is emitted
  # before codegen, so it may be available even when LLVM fails.
  arc_file="$tmpdir/diag-arc.txt"
  if "$SCRIPT_DIR/arc-dump.sh" --raw "$FILE" > "$arc_file" 2>/dev/null; then
      arc_lines=$(wc -l < "$arc_file")
      echo -e "  ARC IR saved to ${arc_file} (${arc_lines} lines)"
  else
      echo -e "  ${C_YELLOW}ARC IR dump unavailable${C_NC}"
  fi
  ```
- [ ] **Ordering dependency note:** This uses `$SCRIPT_DIR` (set at line 29) and `$tmpdir` (set at line 96) — both available before line 158. No `color_flag` needed here (color variables `C_YELLOW`/`C_NC` are set at lines 84-93, before the build step).
- [ ] Verify: intentionally break an `.ori` file at the codegen level (e.g., a type that causes LLVM IR verification failure) and confirm: (a) ARC IR is captured in the build-failure output, (b) exit code is 2 (not 1)

- [ ] **Subsection close-out (03.2)** — MANDATORY before starting 03.3:
  - [ ] All tasks above are `[x]` and verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**

---

## 03.3 Add ARC IR and codegen-audit to mismatch diagnostics

**File(s):** `diagnostics/dual-exec-debug.sh`

Extend the auto-diagnostics block (currently lines 240-265) to include ARC IR dump and codegen-audit run.

**Ordering dependency:** `color_flag` is initialized at line 254 inside the mismatch block, after the LLVM IR dump (line 246) and before RC stats (line 256). The new ARC IR dump must go after the LLVM IR dump but does NOT need `color_flag` (uses `--raw`). The codegen-audit call needs `color_flag` and must go after RC stats (line 256), where `color_flag` is already available. Do NOT reorder the existing LLVM IR dump or RC stats — `color_flag` initialization at line 254 is a dependency for both RC stats and codegen-audit.

- [ ] After the existing LLVM IR dump (line 251, after the `fi`), add ARC IR dump:
  ```bash
  # Run arc-dump.sh — ARC IR shows pre-codegen state where AIMS bugs are visible
  arc_file="$tmpdir/diag-arc.txt"
  if "$SCRIPT_DIR/arc-dump.sh" --raw "$FILE" > "$arc_file" 2>/dev/null; then
      arc_lines=$(wc -l < "$arc_file")
      echo -e "  ARC IR saved to ${arc_file} (${arc_lines} lines)"
  else
      echo -e "  ${C_YELLOW}ARC IR dump failed${C_NC}"
  fi
  ```

- [ ] After the RC stats block (line 262, after the `fi`), add codegen-audit with **proper exit-code handling** (follow `diagnose-aot.sh` lines 371-408 pattern — NOT `|| true`):
  ```bash
  # Run codegen-audit.sh — static RC/COW/ABI analysis
  # Exit codes: 0=clean, 1=findings detected, 2=compilation/infra failure
  audit_file="$tmpdir/codegen_audit.txt"
  "$SCRIPT_DIR/codegen-audit.sh" "$color_flag" "$FILE" > "$audit_file" 2>&1
  audit_exit=$?
  case $audit_exit in
      0)
          echo -e "  Codegen Audit: ${C_GREEN}clean${C_NC}"
          ;;
      1)
          echo -e "  Codegen Audit:"
          sed 's/^/  │ /' "$audit_file"
          ;;
      2)
          echo -e "  ${C_YELLOW}Codegen audit failed (infrastructure error)${C_NC}"
          ;;
  esac
  ```
  **Why not `|| true`?** `codegen-audit.sh` has a 3-level exit contract (0=clean, 1=findings, 2=infra-failure). Using `|| true` swallows all exit codes, making it impossible to distinguish "found RC issues" from "codegen-audit itself broke." `diagnose-aot.sh` lines 371-408 use the proper `case` pattern — this section must match.

- [ ] **Neutralize codegen-audit.sh temp artifact side-effect:** `codegen-audit.sh` lines 147-148 create a binary at `${FILE%.ori}` (in the user's working directory, outside `$tmpdir`) and then `rm -f` it. This is a side effect that writes outside `$tmpdir`. To prevent this, pass the `--no-color` flag is not sufficient — the artifact comes from `ori build "$FILE"` without `-o`. Two options:
  - **Option A (preferred):** Fix `codegen-audit.sh` to build to `$tmpdir` instead of defaulting to `${FILE%.ori}`. This eliminates the side effect at the source for ALL callers. Add `-o "$tmpdir/audit_binary"` to the `ori build` call at line 144, and remove the cleanup at lines 147-148.
  - **Option B (if Option A has wider blast radius):** Document the side effect in a code comment and accept it — the `rm -f` cleanup is immediate.
  - [ ] Implement Option A in `codegen-audit.sh` (line 144: add `-o "$tmpdir/audit_binary"`, remove lines 147-148's `BINARY` cleanup). Verify `self-test.sh` still passes.

- [ ] **Add `--keep-temp` flag** to preserve diagnostic artifacts on mismatch. Currently `trap 'rm -rf "$tmpdir"' EXIT` (line 97) deletes all artifacts before the user can inspect the "saved to" paths printed in the mismatch output. Add:
  - A `--keep-temp` option in the argument parser (after `--verbose`, line 41) that sets `KEEP_TEMP=1`
  - On mismatch with `KEEP_TEMP=1`, copy diagnostic artifacts (`diag-ir.ll`, `diag-arc.txt`, `codegen_audit.txt`) to the working directory (e.g., `.ori-debug/`) and print the preserved paths
  - On mismatch without `--keep-temp`, print a hint: `"(use --keep-temp to preserve diagnostic files)"`
  - Update the EXIT trap to skip cleanup when `KEEP_TEMP=1`
  - Document `--keep-temp` in the Options section of the header comment
- [ ] Update the script header comment (lines 1-26) to document the expanded auto-diagnostics:
  - Add `arc-dump.sh` to the "Requires:" line (currently lists `ir-dump.sh, rc-stats.sh`)
  - Add `codegen-audit.sh` to the "Requires:" line
  - Add `--keep-temp` to the Options list
  - Add a line to the description noting: "On mismatch, also dumps ARC IR and runs codegen-audit."

- [ ] Verify the full mismatch output format: create or use a test `.ori` file that produces a known mismatch, run `dual-exec-debug.sh`, confirm the output shows LLVM IR, ARC IR, RC stats, and codegen-audit in that order

- [ ] **Subsection close-out (03.3)** — MANDATORY before starting 03.4:
  - [ ] All tasks above are `[x]` and verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**

---

## 03.4 Add mismatch-path self-test coverage

**File(s):** `diagnostics/self-test.sh`, `diagnostics/fixtures/` (new fixture)

**Why:** `self-test.sh` lines 261-266 only run `dual-exec-debug.sh` on match-path fixtures (`simple.ori`, `clean.ori` — both produce matching interpreter/AOT results). The new ARC IR dump and codegen-audit code in the mismatch block (03.3) has ZERO automated test coverage. A regression in the mismatch output format would ship behind a green self-test.

**Note on Section 06 coordination:** Section 06 creates failure-mode fixtures and expands self-test coverage. The mismatch fixture created here is specific to dual-exec-debug.sh's mismatch path — it needs a program where interpreter and AOT produce DIFFERENT output (not a program that fails to compile). This is distinct from Section 06's leak/double-free failure fixtures. If Section 06 creates a fixture that also triggers a dual-exec mismatch, it can be reused — but this section must not DEPEND on Section 06 (they are independent per the dependency graph).

- [ ] **Create a deterministic mismatch harness** (NOT a real compiler-bug fixture). Strategy: use an `ORI_BIN` wrapper script that produces different behavior for `run` vs `build`. `_common.sh` lines 23-35 honor `ORI_BIN` before auto-discovery, so setting `ORI_BIN` to a wrapper gives complete control over divergence.
  - Create `diagnostics/fixtures/mismatch-wrapper.sh` — a shell script that:
    - **Default: pass-through to real `ori`** — all commands, flags, and env-driven build modes (including `_common.sh`'s `build /dev/null` probe, `ORI_DUMP_AFTER_ARC=1`, `--emit=llvm-ir`, etc.) delegate to the real `ori` binary. This is essential because `dual-exec-debug.sh`'s helper scripts (`arc-dump.sh`, `codegen-audit.sh`, `ir-dump.sh`) invoke the binary in multiple modes during the mismatch diagnostics block.
    - **Override `ori run <file>`**: prints "INTERP" to stdout and exits 0 (forces interpreter output divergence)
    - **Override `ori build <file> -o <out>`** (plain build only): creates a small binary that prints "AOT" to stdout (or delegates to real `ori` and patches the binary's output — simplest approach: delegate to real `ori build`, then replace the resulting binary with a wrapper that prints "AOT")
    - The wrapper MUST resolve the real `ori` binary path at startup (e.g., from `$REAL_ORI` env var set by the self-test, or by finding it via the same `_common.sh` logic minus `ORI_BIN` override)
    - This guarantees stdout mismatch ("INTERP" vs "AOT") while keeping all helper diagnostics functional
  - Create `diagnostics/fixtures/mismatch.ori` — a minimal valid `.ori` program (e.g., `@main () -> void = print(msg: "hello")`) used as the input file. It doesn't need to actually produce a mismatch — the wrapper handles that.
- [ ] **Add build-failure self-tests** (split into two pins per TPR-03-002-codex):
  - **Pre-lowering failure pin** (syntax error → no ARC IR available):
    - Create `diagnostics/fixtures/build-fail-parse.ori` — a file with a syntax error that fails at parse time (before ARC lowering). `arc-dump.sh` cannot produce ARC IR for this case.
    - Add self-test: assert exit code 2 (infrastructure failure, not mismatch)
    - Add self-test: assert output contains "ARC IR dump unavailable" (NOT "ARC IR saved to")
  - **Post-lowering failure pin** (codegen failure → ARC IR IS available):
    - Create `diagnostics/fixtures/build-fail-codegen-wrapper.sh` — an `ORI_BIN` wrapper script that:
      - **Passes through all commands to real `ori` by default** (same pass-through contract as `mismatch-wrapper.sh`)
      - **On `ori build <file> -o <out>`**: delegates the build to real `ori` but forces it to fail after ARC lowering has completed. Strategy: run real `ori build` with `ORI_DUMP_AFTER_ARC=1` to capture ARC IR, then exit 1 (simulating codegen failure). The ARC IR is already emitted to stderr/file by the real `ori` before the wrapper kills the process.
      - This guarantees: (a) `arc-dump.sh` called separately WILL produce ARC IR (the program is valid), (b) `dual-exec-debug.sh` sees build failure exit code, (c) the 03.2 ARC capture code runs and finds available ARC IR
    - Create `diagnostics/fixtures/build-fail-codegen.ori` — a valid `.ori` program (same as `mismatch.ori` or similar). Does not need to actually trigger a codegen failure — the wrapper handles that.
    - Add self-test: `env ORI_BIN="$FIXTURES_DIR/build-fail-codegen-wrapper.sh"` + assert exit code 2
    - Add self-test: assert output contains "ARC IR saved to" (proves the 03.2 capture behavior)
- [ ] Add mismatch-path tests to `self-test.sh` in the `dual-exec-debug.sh` section (after line 266):
  ```bash
  # Mismatch path: verify auto-diagnostics output (uses ORI_BIN wrapper for deterministic divergence)
  run_test_exit_code "mismatch harness triggers mismatch (exit 1)" 1 \
      env ORI_BIN="$FIXTURES_DIR/mismatch-wrapper.sh" "$SCRIPT_DIR/dual-exec-debug.sh" --no-color "$FIXTURES_DIR/mismatch.ori"
  run_test_output_contains "mismatch auto-dumps ARC IR" "ARC IR saved to" \
      env ORI_BIN="$FIXTURES_DIR/mismatch-wrapper.sh" "$SCRIPT_DIR/dual-exec-debug.sh" --no-color "$FIXTURES_DIR/mismatch.ori"
  run_test_output_contains "mismatch runs codegen-audit" "Codegen Audit" \
      env ORI_BIN="$FIXTURES_DIR/mismatch-wrapper.sh" "$SCRIPT_DIR/dual-exec-debug.sh" --no-color "$FIXTURES_DIR/mismatch.ori"

  # Build-failure path (pre-lowering): exit 2, ARC IR unavailable
  run_test_exit_code "pre-lowering build failure exits 2" 2 \
      "$SCRIPT_DIR/dual-exec-debug.sh" --no-color "$FIXTURES_DIR/build-fail-parse.ori"
  run_test_output_contains "pre-lowering failure shows ARC unavailable" "ARC IR dump unavailable" \
      "$SCRIPT_DIR/dual-exec-debug.sh" --no-color "$FIXTURES_DIR/build-fail-parse.ori"

  # Build-failure path (post-lowering, wrapper-based): exit 2, ARC IR IS captured
  run_test_exit_code "post-lowering build failure exits 2" 2 \
      env ORI_BIN="$FIXTURES_DIR/build-fail-codegen-wrapper.sh" "$SCRIPT_DIR/dual-exec-debug.sh" --no-color "$FIXTURES_DIR/build-fail-codegen.ori"
  run_test_output_contains "post-lowering failure captures ARC IR" "ARC IR saved to" \
      env ORI_BIN="$FIXTURES_DIR/build-fail-codegen-wrapper.sh" "$SCRIPT_DIR/dual-exec-debug.sh" --no-color "$FIXTURES_DIR/build-fail-codegen.ori"
  ```
- [ ] Update the fixture existence check at the top of `self-test.sh` (line 181) to include `mismatch.ori`, `mismatch-wrapper.sh`, `build-fail-parse.ori`, `build-fail-codegen.ori`, and `build-fail-codegen-wrapper.sh`
- [ ] Verify: `diagnostics/self-test.sh --verbose` passes with all new self-test entries

- [ ] **Subsection close-out (03.4)** — MANDATORY before starting 03.R:
  - [ ] All tasks above are `[x]` and verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**

---

## 03.R Third Party Review Findings

- [x] `[TPR-03-001-codex][high]` `section-03-dual-exec-debug.md:97` — Build-failure branch exits 1, conflating with mismatch. No build-failure self-test.
  Resolved: Fixed on 2026-04-09. Added exit-code fix item to 03.2 and build-failure self-test to 03.4.
- [x] `[TPR-03-002-codex][high]` `section-03-dual-exec-debug.md:188` — Mismatch fixture depends on real compiler bug. Replace with deterministic ORI_BIN wrapper harness.
  Resolved: Fixed on 2026-04-09. Rewrote 03.4 with deterministic mismatch-wrapper.sh strategy using ORI_BIN.
- [x] `[TPR-03-003-codex][low]` `section-07-integration.md:132` — Section 07.4 only mentions ARC IR, not codegen-audit.
  Resolved: Fixed on 2026-04-09. Strengthened 03.N checklist reference with specific lines and files to update.
- [x] `[TPR-03-001-gemini][medium]` `section-03-dual-exec-debug.md:120` — Diagnostic artifacts deleted by EXIT trap before user can inspect.
  Resolved: Fixed on 2026-04-09. Added --keep-temp flag item to 03.3 with artifact persistence strategy.
- [x] `[TPR-03-002-gemini][medium]` `section-03-dual-exec-debug.md:188` — Mismatch fixture strategy is fragile.
  Resolved: Fixed on 2026-04-09. Same fix as [TPR-03-002-codex] (deterministic harness).
- [x] `[TPR-03-003-gemini][low]` `section-03-dual-exec-debug.md:158` — Validates Option A for codegen-audit.sh tmpdir is safe.
  Resolved: Non-actionable (confirms existing plan approach).
- [x] `[TPR-03-001-codex][high]` (iter 2) `section-03-dual-exec-debug.md:198` — Mismatch wrapper needs pass-through for all non-overridden commands.
  Resolved: Fixed on 2026-04-09. Rewrote wrapper spec to default pass-through with explicit override-only contract.
- [x] `[TPR-03-002-codex][medium]` (iter 2) `section-03-dual-exec-debug.md:205` — Build-failure self-test needs pre-lowering AND post-lowering pins.
  Resolved: Fixed on 2026-04-09. Split into two fixture/test pairs: parse-fail (no ARC IR) and codegen-fail (ARC IR captured).
- [x] `[TPR-03-003-codex][medium]` (iter 2) `section-07-integration.md:132` — Section 07.4 itself needs the new items, not just 03.N reference.
  Resolved: Fixed on 2026-04-09. Made 03.N a hard GATE + edited Section 07.4 directly (line 132) to include auto codegen-audit, --keep-temp, build-failure ARC IR capture.
- [x] `[TPR-03-001-codex][medium]` (iter 3) `section-03-dual-exec-debug.md:265` — Section 07.4 docs still only ARC IR.
  Resolved: Fixed on 2026-04-09. Edited Section 07.4 line 132 directly to own all four new doc surfaces.
- [x] `[TPR-03-002-codex][medium]` (iter 3) `section-03-dual-exec-debug.md:211` — Post-lowering build-failure pin not fully deterministic.
  Resolved: Fixed on 2026-04-09. Promoted wrapper-based approach to primary with concrete artifact (build-fail-codegen-wrapper.sh).

---

## 03.N Completion Checklist

- [ ] All subsections (03.1, 03.2, 03.3, 03.4) complete
- [ ] `ORI_BIN` is exported before any child script invocation
- [ ] Build-failure branch captures ARC IR
- [ ] Mismatch block includes ARC IR dump, codegen-audit with proper exit-code handling
- [ ] `codegen-audit.sh` temp artifact side-effect fixed (builds to `$tmpdir`)
- [ ] `diagnostics/self-test.sh` passes (including mismatch-path tests)
- [ ] `timeout 150 ./test-all.sh` green — no regressions
- [ ] **GATE: Section 07.4 docs updated BEFORE Section 03 can close.** `section-07-integration.md` lines 127-135 currently only mention ARC IR dump — the implementer MUST edit Section 07.4 to explicitly include: (1) auto codegen-audit on mismatch, (2) `--keep-temp` flag, (3) build-failure ARC IR capture. Also update `diagnostics/README.md` lines 37-44 (currently says mismatch only runs `ir-dump.sh` and `rc-stats.sh`) to include `arc-dump.sh` and `codegen-audit.sh`. This is not a "reminder" — Section 03 CANNOT be marked complete until Section 07.4 owns these doc items in its own checklist. <!-- unblocks:07.4 -->
- [ ] `/tpr-review` passed
- [ ] `/impl-hygiene-review` passed
- [ ] **`/improve-tooling` section-close sweep**
