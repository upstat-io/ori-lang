# Phase 5 — Compile & Present Findings

Read by a Sonnet sub-agent dispatched from `/impl-hygiene-review`. Not a registered skill. Formats the merged findings (Phase 3 + any Phase 4 additions/rejections) into the standard report template. Mechanical-writing — decisions are already made.

Consumes `/tmp/impl-hygiene-{run}/phase-{3,4}.json`. Writes `/tmp/impl-hygiene-{run}/phase-5.json` with the final report text and a structured count of findings by category/severity.

---

### Phase 5: Compile & Present Findings

Collect the findings from all passes across all review units. This is synthesis, not just concatenation.

#### 5a. Deduplicate

Same violation caught by multiple passes → keep the deepest analysis, drop the others.

#### 5b. Cross-Reference

Look for patterns across findings:
- **Cluster analysis**: 5+ findings in one module = design problem (escalate to architectural review)
- **3+ LEAKs in one module** = systemic side logic; the module lacks a canonical dispatch/query point
- **Same algorithm duplicated across N files** = missing abstraction (report as a single finding, not N findings)
- **Cross-backend findings** = highest priority; these drift silently

#### 5c. Severity Calibration

Apply default severities from the finding categories, then adjust:
- **LEAK:algorithmic-duplication** across 3+ sites → Critical (not just because it's a LEAK, but because the blast radius of protocol change is proportional to copy count)
- **Cross-backend LEAKs** → always Critical (eval ↔ LLVM dispatch drift is a correctness risk, not just a maintainability concern)
- Findings tagged `[TP-CONFIRMED-codex]`, `[TP-CONFIRMED-gemini]`, or `[TP-CONFIRMED-both]` → keep severity (with `-both` agreement a strong correctness signal that may warrant a priority boost inside the same severity tier). Findings tagged `[TP-SURFACED-codex]` or `[TP-SURFACED-gemini]` → bump severity one level (an independent reviewer caught what you missed, which means it's less obvious and more likely to be missed again). Per trust tiers, every `-gemini` bump is conditional on you having independently verified the claim against actual code.

#### 5d. Present to User

Present findings organized by category and severity, with a summary preamble:

```
## Hygiene Review: [scope]

**Scope**: [crates/boundaries reviewed]
**Passes**: LEAK/SSOT, Algorithmic DRY, Boundary/Flow, Surface Hygiene
**Third-party cross-check**: [Yes/No] — [N confirmed, M surfaced]
**Finding counts**: [N LEAK, N DRIFT, N GAP, N WASTE, N EXPOSURE, N BLOAT]

### Active Plan Context
[Plans read and their relevance]

### Critical Findings (LEAKs)
...

### Major Findings (DRIFT, GAP)
...

### Minor Findings (WASTE, EXPOSURE, BLOAT)
...
```


---

## Cleanup

- [ ] Run `./test-all.sh` to verify no behavior changes
- [ ] Run `./clippy-all.sh` to verify no regressions
- [ ] Delete this plan directory: `rm -rf plans/hygiene-{name}/`
```

Hygiene fix plans are disposable — they exist to track the fixes, then get deleted when complete.

