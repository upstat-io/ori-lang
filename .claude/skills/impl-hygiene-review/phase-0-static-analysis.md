# Phase 0 — Automated Static Analysis

Read by a Sonnet sub-agent dispatched from `/impl-hygiene-review`. Not a registered skill. Runs the deterministic tooling in this skill directory (`hygiene-lint.py`, `enum-drift.py`, etc.) and captures the output for downstream phases. No AI judgment required at this phase.

Writes `/tmp/impl-hygiene-{run}/phase-0.json` with: tool outputs by name, auto-fixed count, surface findings that remained, and paths to raw logs.

---

### Phase 0: Automated Static Analysis (Run Tools First)

Before AI review begins, run the automated hygiene tools to handle all deterministic, pattern-based checks. This eliminates 60-70% of surface-level findings from AI context, freeing it for LEAK/SSOT/algorithmic DRY analysis.

**All tools are in this skill's folder** (`.claude/skills/impl-hygiene-review/`).

#### 0a. Run hygiene-lint.py (surface checks — ~2s)

```bash
# Full scan of review scope:
hygiene-lint.py --scope <review-paths> --summary

# Detailed findings:
hygiene-lint.py --scope <review-paths>

# Auto-fix banners and commented-out code:
hygiene-lint.py --scope <review-paths> --fix --apply
```

Covers 15 project-specific checks (clippy handles the rest): file-length, fn-length, nesting-depth, test-ephemeral, test-weak, banners, commented-code, bare-todo, catch-all-arms, string-identity, lib-bodies, deny-unsafe, ignore-tracking, phase-bleeding, swallowed-error.

#### 0b. Run enum-drift.py (cross-crate enum coverage — ~0.5s)

```bash
# All known IR enums:
enum-drift.py --summary

# Specific enum:
enum-drift.py --enum CanExpr TypeTag
```

Detects match arms missing for enum variants across crate boundaries — the most dangerous drift pattern (Rust's exhaustive match only catches within a single crate).

#### 0c. Run plan-annotations.py (stale plan refs — ~1s)

```bash
plan-annotations.sh --scope <review-paths> --cleanup-only
```

Already integrated — classifies plan annotations as stale/active/orphan.

#### 0d. Review tool output, apply auto-fixes

1. Apply auto-fixes: `hygiene-fix.py --scope <review-paths> --apply`
2. Review remaining findings — these feed into Phase 3 Pass 4 (skip manual checks already covered by tools)
3. Tool-reported findings that need AI judgment (e.g., test-weak false positives, string-identity in legitimate contexts) get verified during Pass 4

**After Phase 0**: Passes 1-3 of Phase 3 (LEAK/DRY/Boundary) proceed unchanged — these require AI judgment. Pass 4 (Surface Hygiene) is substantially shorter because the tools already caught the mechanical violations.

