#!/usr/bin/env python3
"""Tier 0 static analysis for plan verification.

Performs deterministic, mechanical checks on plan files — no LLM needed.
Outputs structured JSON for consumption by Tier 1 (Sonnet) and Tier 2 (Opus).

Usage:
    plan-audit.py <plan-dir>                    # audit with human-readable output
    plan-audit.py <plan-dir> --json             # JSON output for agent consumption
    plan-audit.py <plan-dir> --fix-safe         # auto-fix deterministic metadata
    plan-audit.py <plan-dir> --fix-safe --apply # actually write fixes (default: dry-run)
    plan-audit.py <plan-dir> --verify           # post-edit structural validation
    plan-audit.py <plan-dir> --check status,paths  # run specific checks only

Checks (category codes follow impl-hygiene.md vocabulary):
    STATUS_DRIFT     — frontmatter status vs checkbox counts disagree
    DEAD_PATH        — referenced file path does not exist on disk
    STALE_LINE_REF   — file:line reference points to wrong content
    BROKEN_DEP       — depends_on references nonexistent section
    CRITERIA_ORPHAN  — mission criterion has no section mapping (heuristic)
    DEFERRAL_LANG    — soft deferral language in checklist items
    SIZE_VIOLATION   — section has <3 or >20 checklist items
    DUPLICATE_ITEM   — same checklist text appears in multiple sections
    REVIEWED_GATE    — reviewed:false on complete section (or vice versa)
    SCHEMA_VIOLATION — missing required frontmatter fields
    COMPLETION_TPL   — missing mandatory completion checklist items
    FILE_CONTENTION  — multiple sections reference the same source file
    BLOAT_RISK       — referenced source file already near 500-line limit
    SUB_STATUS_DRIFT — subsection frontmatter vs checkbox counts disagree
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
from collections import Counter, defaultdict
from dataclasses import asdict, dataclass, field
from pathlib import Path

# Add parent dir so we can import planlib from the same directory
sys.path.insert(0, str(Path(__file__).resolve().parent))
import planlib

# ─── Finding model ───────────────────────────────────────────────────────────


@dataclass
class Finding:
    """A single audit finding."""
    check: str                 # STATUS_DRIFT, DEAD_PATH, etc.
    category: str              # DRIFT, GAP, WASTE, LEAK, BLOAT, EXPOSURE
    severity: str              # critical, major, minor
    confidence: str            # deterministic, heuristic
    location: str              # section-04.md:42 or plan-level
    message: str
    fix_safe: bool = False     # can --fix-safe auto-correct this?
    detail: str = ""           # additional context for agents


# ─── Check implementations ───────────────────────────────────────────────────


def check_status_drift(plan: planlib.PlanInfo) -> list[Finding]:
    """Compare frontmatter status against actual checkbox state."""
    findings = []
    for s in plan.sections:
        mismatch = s.status_mismatch()
        if mismatch:
            findings.append(Finding(
                check="STATUS_DRIFT", category="DRIFT", severity="major",
                confidence="deterministic", location=s.filename,
                message=f"Section {s.number} '{s.title}': {mismatch}",
                fix_safe=True,
                detail=f"checked={s.checked}/{s.total}, status={s.status}",
            ))
        # Check subsection status drift
        for sub in s.subsections:
            sub_mismatch = sub.status_mismatch()
            if sub_mismatch:
                findings.append(Finding(
                    check="SUB_STATUS_DRIFT", category="DRIFT", severity="minor",
                    confidence="deterministic", location=f"{s.filename}:§{sub.id}",
                    message=f"Subsection {sub.id} '{sub.title}': {sub_mismatch}",
                    fix_safe=True,
                    detail=f"checked={sub.checked}/{sub.total}, status={sub.status}",
                ))
    return findings


def check_dead_paths(plan: planlib.PlanInfo) -> list[Finding]:
    """Check that referenced file paths exist on disk."""
    findings = []
    for s in plan.sections:
        for ref in s.extract_file_refs():
            full = planlib.REPO_ROOT / ref
            if not full.exists():
                findings.append(Finding(
                    check="DEAD_PATH", category="GAP", severity="major",
                    confidence="deterministic", location=s.filename,
                    message=f"Referenced path `{ref}` does not exist",
                ))
    return findings


def check_stale_line_refs(plan: planlib.PlanInfo) -> list[Finding]:
    """Check that file:line references point to plausible content."""
    findings = []
    for s in plan.sections:
        for ref_path, lineno in s.extract_file_line_refs():
            full = planlib.REPO_ROOT / ref_path
            if not full.exists():
                continue  # Already caught by DEAD_PATH
            try:
                lines = full.read_text(encoding="utf-8", errors="replace").splitlines()
                if lineno > len(lines):
                    findings.append(Finding(
                        check="STALE_LINE_REF", category="DRIFT", severity="minor",
                        confidence="deterministic", location=s.filename,
                        message=f"`{ref_path}:{lineno}` — file only has {len(lines)} lines",
                    ))
            except OSError:
                pass
    return findings


def check_broken_deps(plan: planlib.PlanInfo) -> list[Finding]:
    """Check that depends_on references point to existing sections."""
    findings = []
    section_nums = {s.number for s in plan.sections}
    # Also include zero-padded variants
    section_nums_norm = {s.number.lstrip("0") or "0" for s in plan.sections}
    for s in plan.sections:
        for dep in s.depends_on:
            dep_norm = dep.lstrip("0") or "0"
            if dep not in section_nums and dep_norm not in section_nums_norm:
                findings.append(Finding(
                    check="BROKEN_DEP", category="GAP", severity="critical",
                    confidence="deterministic", location=s.filename,
                    message=f"Section {s.number} depends_on '{dep}' — no such section exists",
                ))
    return findings


def check_criteria_orphans(plan: planlib.PlanInfo) -> list[Finding]:
    """Basic heuristic: check if mission criteria text appears in any section body."""
    findings = []
    if not plan.overview or not plan.overview.mission_criteria:
        return findings
    for criterion in plan.overview.mission_criteria:
        # Extract key words (skip short words) for fuzzy matching
        words = [w.lower() for w in re.findall(r"\w{4,}", criterion)]
        if len(words) < 2:
            continue
        found = False
        for s in plan.sections:
            body_lower = s.body_text.lower()
            matches = sum(1 for w in words if w in body_lower)
            if matches >= len(words) * 0.5:
                found = True
                break
        if not found:
            findings.append(Finding(
                check="CRITERIA_ORPHAN", category="GAP", severity="minor",
                confidence="heuristic",
                location="00-overview.md",
                message=f"Mission criterion may have no section coverage: '{criterion[:80]}...'",
                detail="Heuristic: <50% keyword overlap with any section body",
            ))
    return findings


def check_deferral_language(plan: planlib.PlanInfo) -> list[Finding]:
    """Scan for soft deferral language in plan text."""
    findings = []
    for s in plan.sections:
        for lineno, word, context in s.extract_deferral_language():
            findings.append(Finding(
                check="DEFERRAL_LANG", category="WASTE", severity="minor",
                confidence="heuristic", location=f"{s.filename}:{lineno}",
                message=f"Deferral language '{word}' in: {context[:100]}",
            ))
    return findings


def check_size_violations(plan: planlib.PlanInfo) -> list[Finding]:
    """Flag sections with too few (<3) or too many (>20) top-level items."""
    findings = []
    for s in plan.sections:
        top_level = [i for i in s.flat_items if i.indent == 0]
        if len(top_level) < 3 and s.status != "complete":
            findings.append(Finding(
                check="SIZE_VIOLATION", category="GAP", severity="minor",
                confidence="deterministic", location=s.filename,
                message=f"Section {s.number} has only {len(top_level)} top-level items (minimum 3)",
            ))
        if len(top_level) > 20:
            findings.append(Finding(
                check="SIZE_VIOLATION", category="BLOAT", severity="major",
                confidence="deterministic", location=s.filename,
                message=f"Section {s.number} has {len(top_level)} top-level items (maximum 20) — consider splitting",
            ))
    return findings


def check_duplicate_items(plan: planlib.PlanInfo) -> list[Finding]:
    """Find identical checklist text across different sections."""
    findings = []
    seen: dict[str, str] = {}  # normalized content → first section filename
    for s in plan.sections:
        for item in s.flat_items:
            # Normalize: lowercase, strip whitespace and markdown formatting
            norm = re.sub(r"\s+", " ", item.content.lower().strip())
            norm = re.sub(r"\*\*|\`|_", "", norm)
            if len(norm) < 20:
                continue  # Skip very short items
            if norm in seen and seen[norm] != s.filename:
                findings.append(Finding(
                    check="DUPLICATE_ITEM", category="WASTE", severity="minor",
                    confidence="deterministic", location=s.filename,
                    message=f"Duplicate item also in {seen[norm]}: '{item.content[:80]}'",
                ))
            else:
                seen[norm] = s.filename
    return findings


def check_reviewed_gate(plan: planlib.PlanInfo) -> list[Finding]:
    """Check for reviewed/status mismatches."""
    findings = []
    for s in plan.sections:
        if s.reviewed is None:
            findings.append(Finding(
                check="REVIEWED_GATE", category="GAP", severity="minor",
                confidence="deterministic", location=s.filename,
                message=f"Section {s.number} missing `reviewed` field in frontmatter",
                fix_safe=True,
            ))
        elif s.status == "complete" and s.reviewed is False:
            findings.append(Finding(
                check="REVIEWED_GATE", category="DRIFT", severity="minor",
                confidence="deterministic", location=s.filename,
                message=f"Section {s.number} is complete but reviewed: false",
            ))
    return findings


def check_schema_violations(plan: planlib.PlanInfo) -> list[Finding]:
    """Check required frontmatter fields exist."""
    findings = []
    required_section = ["section", "title", "status"]
    for s in plan.sections:
        for fld in required_section:
            if fld not in s.frontmatter:
                findings.append(Finding(
                    check="SCHEMA_VIOLATION", category="GAP", severity="critical",
                    confidence="deterministic", location=s.filename,
                    message=f"Missing required frontmatter field: {fld}",
                ))
    if plan.overview:
        required_overview = ["plan", "title", "status"]
        for fld in required_overview:
            if fld not in plan.overview.frontmatter:
                # Some overviews have no frontmatter at all (roadmap) — skip
                if plan.overview.frontmatter:
                    findings.append(Finding(
                        check="SCHEMA_VIOLATION", category="GAP", severity="major",
                        confidence="deterministic", location="00-overview.md",
                        message=f"Missing required overview field: {fld}",
                    ))
    return findings


def check_completion_template(plan: planlib.PlanInfo) -> list[Finding]:
    """Check sections have mandatory completion checklist items."""
    findings = []
    mandatory_patterns = [
        (r"test-all\.sh", "test-all.sh regression check"),
        (r"tpr[_-]review|/tpr-review|third.party.review", "TPR review"),
        (r"hygiene[_-]review|/impl-hygiene", "impl-hygiene review"),
    ]
    for s in plan.sections:
        if s.status == "complete":
            continue  # Don't flag already-completed sections
        # Check if section has a completion checklist subsection
        has_completion = any(
            sub.id.endswith(".N") or "completion" in sub.title.lower()
            for sub in s.subsections
        )
        if not has_completion and s.total > 0:
            findings.append(Finding(
                check="COMPLETION_TPL", category="GAP", severity="major",
                confidence="deterministic", location=s.filename,
                message=f"Section {s.number} has no Completion Checklist subsection (NN.N)",
            ))
            continue

        # Check for mandatory items in body text
        body_lower = s.body_text.lower()
        for pattern, desc in mandatory_patterns:
            if not re.search(pattern, body_lower, re.IGNORECASE):
                findings.append(Finding(
                    check="COMPLETION_TPL", category="GAP", severity="minor",
                    confidence="heuristic", location=s.filename,
                    message=f"Section {s.number} completion checklist may be missing: {desc}",
                ))
    return findings


def check_file_contention(plan: planlib.PlanInfo) -> list[Finding]:
    """Detect when multiple sections reference the same source file."""
    findings = []
    file_to_sections: dict[str, list[str]] = defaultdict(list)
    for s in plan.sections:
        for ref in s.extract_file_refs():
            file_to_sections[ref].append(f"{s.number}")

    for ref, sections in file_to_sections.items():
        if len(sections) > 1:
            findings.append(Finding(
                check="FILE_CONTENTION", category="DRIFT", severity="minor",
                confidence="deterministic", location="plan-level",
                message=f"`{ref}` referenced by sections: {', '.join(sections)} — potential ordering dependency",
                detail=f"Sections touching same file may need explicit depends_on",
            ))
    return findings


def check_bloat_risk(plan: planlib.PlanInfo) -> list[Finding]:
    """Flag referenced source files already near the 500-line limit."""
    findings = []
    checked: set[str] = set()
    for s in plan.sections:
        if s.status == "complete":
            continue
        for ref in s.extract_file_refs():
            if ref in checked:
                continue
            checked.add(ref)
            full = planlib.REPO_ROOT / ref
            if not full.exists():
                continue
            try:
                line_count = len(full.read_text(encoding="utf-8", errors="replace").splitlines())
                if line_count >= 450:
                    findings.append(Finding(
                        check="BLOAT_RISK", category="BLOAT", severity="major",
                        confidence="deterministic", location=s.filename,
                        message=f"`{ref}` is {line_count} lines — at or near 500-line limit",
                        detail="Plan should include a split/extract task if adding code to this file",
                    ))
                elif line_count >= 350:
                    findings.append(Finding(
                        check="BLOAT_RISK", category="BLOAT", severity="minor",
                        confidence="deterministic", location=s.filename,
                        message=f"`{ref}` is {line_count} lines — approaching 500-line limit",
                    ))
            except OSError:
                pass
    return findings


# ─── Check registry ──────────────────────────────────────────────────────────

ALL_CHECKS: dict[str, tuple[str, type]] = {
    "status":      ("STATUS_DRIFT + SUB_STATUS_DRIFT", check_status_drift),
    "paths":       ("DEAD_PATH", check_dead_paths),
    "lines":       ("STALE_LINE_REF", check_stale_line_refs),
    "deps":        ("BROKEN_DEP", check_broken_deps),
    "criteria":    ("CRITERIA_ORPHAN", check_criteria_orphans),
    "deferral":    ("DEFERRAL_LANG", check_deferral_language),
    "size":        ("SIZE_VIOLATION", check_size_violations),
    "duplicates":  ("DUPLICATE_ITEM", check_duplicate_items),
    "reviewed":    ("REVIEWED_GATE", check_reviewed_gate),
    "schema":      ("SCHEMA_VIOLATION", check_schema_violations),
    "completion":  ("COMPLETION_TPL", check_completion_template),
    "contention":  ("FILE_CONTENTION", check_file_contention),
    "bloat":       ("BLOAT_RISK", check_bloat_risk),
}


# ─── Fix-safe implementation ─────────────────────────────────────────────────


def apply_status_fix(section: planlib.SectionInfo, dry_run: bool = True) -> str | None:
    """Fix status drift by updating frontmatter status to match checkboxes."""
    if section.total == 0:
        return None
    if section.unchecked == 0:
        new_status = "complete"
    elif section.checked > 0:
        new_status = "in-progress"
    else:
        new_status = "not-started"

    if new_status == section.status:
        return None

    if dry_run:
        return f"Would fix {section.filename}: status {section.status} → {new_status}"

    text = planlib.read_text(section.path)
    # Replace status in frontmatter
    updated = re.sub(
        r"^(status:\s*).+$",
        f"\\g<1>{new_status}",
        text,
        count=1,
        flags=re.MULTILINE,
    )
    section.path.write_text(updated, encoding="utf-8")
    return f"Fixed {section.filename}: status {section.status} → {new_status}"


# ─── Output formatting ───────────────────────────────────────────────────────


def findings_to_json(findings: list[Finding], plan: planlib.PlanInfo) -> dict:
    """Build the JSON packet for agent consumption."""
    # Summary
    by_severity = Counter(f.severity for f in findings)
    by_check = Counter(f.check for f in findings)
    by_category = Counter(f.category for f in findings)

    # Extract deduped symbol list for Tier 1 LSP verification
    all_symbols: set[str] = set()
    all_file_refs: set[str] = set()
    all_file_line_refs: list[dict] = []
    for s in plan.sections:
        all_symbols.update(s.extract_symbols())
        all_file_refs.update(s.extract_file_refs())
        for path, line in s.extract_file_line_refs():
            all_file_line_refs.append({"path": path, "line": line, "section": s.number})

    # Section manifest
    section_manifest = []
    for s in plan.sections:
        section_manifest.append({
            "number": s.number,
            "title": s.title,
            "status": s.status,
            "reviewed": s.reviewed,
            "checked": s.checked,
            "total": s.total,
            "pct": s.pct,
            "depends_on": s.depends_on,
            "subsection_count": len(s.subsections),
            "file_refs_count": len(s.extract_file_refs()),
        })

    return {
        "plan": plan.name,
        "plan_dir": str(plan.dir),
        "overview_status": plan.overview.status if plan.overview else None,
        "total_sections": len(plan.sections),
        "total_items": plan.total_items,
        "total_checked": plan.total_checked,
        "completion_pct": plan.pct,
        "summary": {
            "total_findings": len(findings),
            "by_severity": dict(by_severity),
            "by_check": dict(by_check),
            "by_category": dict(by_category),
        },
        "findings": [
            {
                "check": f.check,
                "category": f.category,
                "severity": f.severity,
                "confidence": f.confidence,
                "location": f.location,
                "message": f.message,
                "fix_safe": f.fix_safe,
                "detail": f.detail,
            }
            for f in findings
        ],
        "section_manifest": section_manifest,
        "symbols": sorted(all_symbols),
        "file_refs": sorted(all_file_refs),
        "file_line_refs": all_file_line_refs,
    }


def print_human(findings: list[Finding], plan: planlib.PlanInfo) -> None:
    """Human-readable output."""
    if not findings:
        print(f"plan-audit: {plan.name} — CLEAN (0 findings)")
        return

    by_severity = Counter(f.severity for f in findings)
    print(f"plan-audit: {plan.name} — {len(findings)} findings "
          f"({by_severity.get('critical', 0)} critical, "
          f"{by_severity.get('major', 0)} major, "
          f"{by_severity.get('minor', 0)} minor)")
    print()

    # Group by severity
    for sev in ["critical", "major", "minor"]:
        group = [f for f in findings if f.severity == sev]
        if not group:
            continue
        print(f"  [{sev.upper()}]")
        for f in group:
            prefix = "[fix-safe] " if f.fix_safe else ""
            print(f"    {prefix}[{f.check}] {f.location}: {f.message}")
            if f.detail:
                print(f"      {f.detail}")
        print()

    # Summary stats
    print(f"  Plan: {plan.total_checked}/{plan.total_items} items checked ({plan.pct}%)")
    for s in plan.sections:
        status_mark = "[x]" if s.status == "complete" else "[ ]" if s.status == "not-started" else "[~]"
        print(f"    {status_mark} {s.number}: {s.title} ({s.checked}/{s.total}, {s.status})")


# ─── Main ────────────────────────────────────────────────────────────────────


def main() -> int:
    parser = argparse.ArgumentParser(description="Tier 0 static analysis for plan verification")
    parser.add_argument("plan_dir", help="Path to plan directory")
    parser.add_argument("--json", action="store_true", help="JSON output for agent consumption")
    parser.add_argument("--fix-safe", action="store_true", help="Auto-fix deterministic metadata")
    parser.add_argument("--apply", action="store_true", help="Actually write fixes (default: dry-run)")
    parser.add_argument("--verify", action="store_true", help="Post-edit structural validation (stricter)")
    parser.add_argument("--check", help="Comma-separated check names to run (default: all)")
    parser.add_argument("--no-color", action="store_true", help="Disable color output")

    args = parser.parse_args()

    plan_path = Path(args.plan_dir).resolve()
    if not plan_path.is_dir():
        print(f"Error: {args.plan_dir} is not a directory", file=sys.stderr)
        return 2

    # Load plan
    plan = planlib.load_plan(plan_path)
    if not plan.sections:
        print(f"Warning: no section files found in {plan_path}", file=sys.stderr)

    # Determine which checks to run
    if args.check:
        check_names = [c.strip() for c in args.check.split(",")]
        for name in check_names:
            if name not in ALL_CHECKS:
                print(f"Error: unknown check '{name}'. Available: {', '.join(ALL_CHECKS)}", file=sys.stderr)
                return 2
    else:
        check_names = list(ALL_CHECKS.keys())

    # Run fix-safe first if requested
    if args.fix_safe:
        fixes = []
        for s in plan.sections:
            result = apply_status_fix(s, dry_run=not args.apply)
            if result:
                fixes.append(result)
        if fixes:
            for fix in fixes:
                print(fix, file=sys.stderr)
            if args.apply:
                # Reload plan after fixes
                plan = planlib.load_plan(plan_path)

    # Run checks
    all_findings: list[Finding] = []
    for name in check_names:
        _, check_fn = ALL_CHECKS[name]
        all_findings.extend(check_fn(plan))

    # In verify mode, stricter thresholds
    if args.verify:
        critical = [f for f in all_findings if f.severity == "critical"]
        if critical:
            print(f"VERIFY FAILED: {len(critical)} critical findings", file=sys.stderr)

    # Output
    if args.json:
        data = findings_to_json(all_findings, plan)
        json.dump(data, sys.stdout, indent=2)
        print()
    else:
        print_human(all_findings, plan)

    # Exit code: 0 = clean, 1 = findings, 2 = error
    if any(f.severity == "critical" for f in all_findings):
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
