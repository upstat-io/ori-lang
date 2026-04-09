---
section: "03"
title: "Enhance dual-exec-debug.sh"
status: not-started
reviewed: false
goal: "Auto-dump ARC IR and run codegen-audit on mismatch, bridging the gap between 'these differ' and 'here is why'"
success_criteria:
  - "On mismatch, dual-exec-debug.sh auto-dumps ARC IR via arc-dump.sh alongside LLVM IR"
  - "On mismatch, dual-exec-debug.sh runs codegen-audit.sh and displays findings"
  - "self-test.sh updated with test coverage"
inspired_by:
  - "Zig --verbose-llvm-ir — automatically shows relevant IR on compilation failure"
depends_on: []
third_party_review:
  status: none
  updated: null
sections:
  - id: "03.1"
    title: "Add ARC IR and codegen-audit to mismatch diagnostics"
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
**Goal:** When `dual-exec-debug.sh` detects a mismatch between interpreter and AOT, automatically dump ARC IR (which shows the pre-codegen state — often where AIMS bugs are visible) and run codegen-audit (which catches RC/COW/ABI issues statically).

**Success Criteria:**
- [ ] On mismatch, ARC IR is saved to `$tmpdir/diag-arc.txt` and reported
- [ ] On mismatch, `codegen-audit.sh` runs and findings are displayed
- [ ] On match, no extra work is done (no performance penalty for passing cases)
- [ ] Satisfies mission criterion: "dual-exec-debug.sh auto-dumps ARC IR on mismatch"

**Context:** `dual-exec-debug.sh` currently auto-dumps LLVM IR and RC stats on mismatch (lines 240-262), but never dumps ARC IR even though `arc-dump.sh` exists. Many AIMS bugs — wrong RC placement, missing drops, incorrect ownership annotations — are visible in ARC IR before LLVM codegen faithfully replicates them. Adding ARC IR to the mismatch diagnostics bridges the gap from "these outputs differ" to "here is the ARC-level decision that caused the divergence."

**Depends on:** None.

---

## 03.1 Add ARC IR and codegen-audit to mismatch diagnostics

**File(s):** `diagnostics/dual-exec-debug.sh`

Extend the auto-diagnostics block (currently lines 240-265) to include ARC IR and codegen-audit.

- [ ] After the existing LLVM IR dump (line 246), add ARC IR dump:
  ```bash
  # Run arc-dump.sh
  arc_file="$tmpdir/diag-arc.txt"
  if "$SCRIPT_DIR/arc-dump.sh" --raw "$FILE" > "$arc_file" 2>/dev/null; then
      arc_lines=$(wc -l < "$arc_file")
      echo -e "  ARC IR saved to ${arc_file} (${arc_lines} lines)"
  else
      echo -e "  ${C_YELLOW}ARC IR dump failed${C_NC}"
  fi
  ```
- [ ] After the RC stats (line 256), add codegen-audit:
  ```bash
  # Run codegen-audit.sh
  audit_output=$("$SCRIPT_DIR/codegen-audit.sh" "$color_flag" "$FILE" 2>/dev/null) || true
  if [[ -n "$audit_output" ]]; then
      echo -e "  Codegen Audit:"
      echo "$audit_output" | sed 's/^/  │ /'
  fi
  ```
- [ ] Update the script header comment to document the expanded auto-diagnostics
- [ ] Verify: create a test `.ori` file that produces a known mismatch (if one exists in fixtures), or manually verify the output format

- [ ] **Subsection close-out (03.1)** — MANDATORY before starting 03.R:
  - [ ] All tasks above are `[x]` and verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**

---

## 03.R Third Party Review Findings

- None.

---

## 03.N Completion Checklist

- [ ] All subsections (03.1) complete
- [ ] `diagnostics/self-test.sh` passes
- [ ] `timeout 150 ./test-all.sh` green — no regressions
- [ ] `/tpr-review` passed
- [ ] `/impl-hygiene-review` passed
- [ ] **`/improve-tooling` section-close sweep**
