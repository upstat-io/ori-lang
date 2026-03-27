---
section: "03"
title: "CI cross-platform timeout"
status: complete
reviewed: false
goal: "Keep CI runtime limits high enough for the slowest supported platform without masking genuine hangs."
depends_on: []
third_party_review:
  status: none
  updated: null
sections:
  - id: "03.1"
    title: "Timeout increase"
    status: complete
  - id: "03.R"
    title: "Third Party Review Findings"
    status: complete
  - id: "03.N"
    title: "Completion Checklist"
    status: complete
---

# Section 03: CI cross-platform timeout

**Status:** Complete
**Goal:** The CI workflow allows the Windows job enough wall-clock time to finish under normal load while still failing real deadlocks or hangs.

**Context:** The plan tracked a cross-platform timeout issue alongside the two macOS AOT failures. That workflow adjustment was already applied in `.github/workflows/ci.yml`, and the current review did not uncover any follow-up defects in this section.

**Depends on:** None.

---

## 03.1 Timeout increase

**File(s):** `.github/workflows/ci.yml`

The workflow timeout increase from `10` to `30` minutes is already landed for the affected CI job.

- [x] Increase the relevant CI timeout to `30` minutes.
- [x] Keep the change scoped to workflow configuration.

---

## 03.R Third Party Review Findings

- None.

---

## 03.N Completion Checklist

- [x] `.github/workflows/ci.yml` reflects the `30` minute timeout.
- [x] This section has no open third-party review findings.

**Exit Criteria:** The workflow timeout update is present in the repository and there are no outstanding review findings against this section.
