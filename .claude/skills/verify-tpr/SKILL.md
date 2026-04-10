---
name: verify-tpr
model: sonnet
description: "Triage TPR findings in a plan section with full codebase validation. Invoked by /continue-roadmap when third_party_review.status is 'findings'. Validates each finding against actual code, spec, and plan — accepts or rejects with rigor."
argument-hint: "<section-file-path>"
---

# Verify TPR Findings

Triage all unchecked Third Party Review findings in a plan section file. This skill is invoked by `/continue-roadmap` (Step 1.9) when a section has `third_party_review.status: findings`.

**This skill runs on Opus** because TPR triage requires deep codebase validation, spec cross-referencing, and judgment about whether findings are factually correct vs. rationalized dismissals.

## Arguments

- `<section-file-path>` — path to the plan section file containing the TPR findings block (e.g., `plans/repr-opt/section-07-enum-repr.md`)

## Workflow

### Step 1: Read CLAUDE.md

Read `CLAUDE.md` in full. The zero-deferral and no-rationalization rules are load-bearing for triage decisions.

### Step 2: Read the Section File

Read the entire section file. Identify:
- The `## {NN}.R Third Party Review Findings` block
- All unchecked `- [ ]` findings (these are the items to triage)
- The section's frontmatter (`third_party_review.status`, `status`, etc.)

If there are no unchecked findings, report "No open TPR findings" and exit.

### Step 3: Triage Each Finding

Process findings in priority order: `[high]` -> `[medium]` -> `[low]`.

For EACH unchecked finding:

1. **Read the referenced file(s)** — the finding cites specific file paths and line numbers. Read those files. Do not triage from memory or the finding text alone.

2. **Validate against the codebase** — does the issue described in the finding actually exist?
   - Check the specific code paths mentioned
   - Check if the issue has already been fixed since the finding was filed
   - Check the spec (`docs/ori_lang/v2026/spec/`) if the finding concerns language semantics
   - Check tests — are there existing tests that cover this? Are they sufficient?

3. **Determine: ACCEPT or REJECT**

   **ACCEPT** if the finding identifies a real issue (even if partially):
   - The described weakness/bug/gap exists in the codebase
   - Tests are weaker than claimed (e.g., structural assertions where behavioral tests are possible)
   - A code pattern is genuinely problematic

   **REJECT** only if the finding is **factually incorrect**:
   - The described issue does not exist in the code (file changed since review, finding misread the code)
   - The test the finding claims is missing actually exists
   - The behavior the finding flags as wrong is actually spec-correct

   **BANNED rejection reasons** (these are NEVER valid):
   - "Not related to current plan" / "out of scope"
   - "Pre-existing" / "was already like that"
   - "Architectural limitation"
   - "Conservative/safe approach"
   - "Future improvement"
   - "Not our problem"

4. **For ACCEPTED findings — determine resolution path**:

   **Can fix now** (the issue is concrete and the fix is clear):
   - Fix the code, add/strengthen tests
   - Mark the finding resolved with what was done:
     ```markdown
     - [x] `[TPR-NN-XXX][severity]` `path` — Description.
       Resolved: Fixed on YYYY-MM-DD. [What was done — code change + tests added].
     ```

   **Genuinely blocked** (e.g., depends on a gate flag, requires infrastructure that doesn't exist yet):
   - Verify the blocker is real by checking the codebase (not just trusting the finding's claim)
   - Create a concrete `- [ ]` task in the appropriate subsection with a `<!-- blocked-by:X -->` anchor
   - Mark the finding resolved with the anchor:
     ```markdown
     - [x] `[TPR-NN-XXX][severity]` `path` — Description.
       Resolved: Validated on YYYY-MM-DD. Blocked by [specific gate/dependency]. Created concrete task in {NN}.{M}: "[task description]" with blocked-by anchor. Will be unblocked when [condition].
     ```
   - **CRITICAL**: The blocker must be real and verifiable. "Blocked by future work" without a specific gate or dependency is deferral, not a blocker.

   **Requires investigation** (finding may be valid but you need to dig deeper):
   - Do the investigation NOW. Do not mark as resolved without understanding.
   - If investigation reveals the issue is real: fix or create blocked task (above)
   - If investigation reveals the issue doesn't exist: reject with evidence

5. **For REJECTED findings**:
   ```markdown
   - [x] `[TPR-NN-XXX][severity]` `path` — Description.
     Resolved: Rejected on YYYY-MM-DD. [Evidence that the issue does not exist — cite specific file:line, test name, or spec clause].
   ```

### Step 4: Update Frontmatter

After all findings are triaged:

- Update `third_party_review.updated` to today's date
- If ALL findings were rejected (no new `- [ ]` items created):
  - Set `third_party_review.status: resolved`
- If ANY accepted findings created new `- [ ]` implementation items:
  - **Keep** `third_party_review.status: findings` — do NOT set to `resolved`
  - Status transitions to `resolved` only when accepted tasks are complete and revalidated
- Section `status` stays `in-progress` while `third_party_review.status: findings`

### Step 5: Report

Output a summary for the calling skill:

```
TPR Triage Complete: <section-file>
  Findings: N total (H high, M medium, L low)
  Accepted: X (Y fixed now, Z blocked with anchors)
  Rejected: R (with evidence)
  New tasks created: [list subsection IDs where tasks were added]
  Frontmatter: third_party_review.status = <new status>
  Section status: <status>
```

## Status Rules

- A section CANNOT be `complete` while unchecked TPR items exist
- `third_party_review.status: findings` forces section `status` to `in-progress`
- All findings must be triaged before any new implementation work begins in that section

## Quality Standard

The entire point of this skill running on Opus is to prevent soft accepts — findings marked `[x]` with deferral language instead of actual fixes or properly-anchored blocked tasks. Every resolution must either change code or point to a concrete, verifiable blocker. "We'll handle it later" is not a resolution.
