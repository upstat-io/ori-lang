"""Findings report format for verify-roadmap.

Section 03.2 of verify-roadmap-redesign plan.

Generates JSON + markdown + console output from ClassifiedFindings.
Surfaces unapplied fixes (PatchResult(applied=False)) as a distinct
group — never silently drops them.

Design invariants:
  1. Imports Finding from plan_corpus (no shadow types)
  2. Imports ClassifiedFinding/SafetyClass from .safety (local SSOT)
  3. --quick mode omits safety_class/rationale (classification not run)
  4. Severity ordering: critical > high > medium > low (rendered first)
  5. Within severity, ExposureReview rendered before SafeFix
  6. Exit code: 0 clean, 1 findings present, 2 critical findings present
"""

from __future__ import annotations

import enum
import json
from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path
from typing import Iterable

from scripts.plan_corpus import Severity

from .safety import (
    ClassifiedFinding,
    PatchResult,
    SafetyClass,
)


# ---------------------------------------------------------------------------
# Modes & top-level Report dataclass
# ---------------------------------------------------------------------------

class ReportMode(enum.Enum):
    """Report generation mode.

    FULL: all classifiers run, classify_safety populated, auto-fix may apply
    QUICK: only DAG read-only checks run, no classification (all ExposureReview)
    """
    FULL = "full"
    QUICK = "quick"


@dataclass(frozen=True)
class Report:
    """Aggregated findings ready for rendering."""
    mode: ReportMode
    findings: tuple[ClassifiedFinding, ...] = ()
    unapplied_fixes: tuple[PatchResult, ...] = ()


def generate_report(
    classified: Iterable[ClassifiedFinding],
    unapplied: Iterable[PatchResult],
    mode: ReportMode,
) -> Report:
    """Wrap classified findings + unapplied fixes into a Report."""
    return Report(
        mode=mode,
        findings=tuple(classified),
        unapplied_fixes=tuple(unapplied),
    )


# ---------------------------------------------------------------------------
# Sorting helpers — canonical ordering for renderers
# ---------------------------------------------------------------------------

def _sort_findings(findings: Iterable[ClassifiedFinding]) -> list[ClassifiedFinding]:
    """Sort by severity (CRITICAL first), then ExposureReview before SafeFix.

    Within each (severity, safety_class) group, order by category then subtype.
    """
    def key(cf: ClassifiedFinding):
        f = cf.finding
        # Higher severity first -> negate
        sev_key = -int(f.severity)
        # ExposureReview (1) before SafeFix (0) -> use 0 for review, 1 for safe
        safety_key = 0 if cf.safety_class == SafetyClass.EXPOSURE_REVIEW else 1
        return (sev_key, safety_key, f.category.value, f.subtype.value, str(f.source))
    return sorted(findings, key=key)


def _group_by_severity(
    findings: Iterable[ClassifiedFinding],
) -> dict[Severity, list[ClassifiedFinding]]:
    """Group sorted findings by severity (preserving ordering from _sort_findings)."""
    groups: dict[Severity, list[ClassifiedFinding]] = {}
    for cf in findings:
        groups.setdefault(cf.finding.severity, []).append(cf)
    return groups


# ---------------------------------------------------------------------------
# JSON renderer
# ---------------------------------------------------------------------------

def render_json(report: Report, corpus_size: int | None = None) -> dict:
    """Render the report as a JSON-serializable dict.

    In QUICK mode, omits safety_class and rationale from each finding entry —
    classification was not performed.
    """
    findings_out: list[dict] = []
    for cf in _sort_findings(report.findings):
        entry: dict = {
            "finding": cf.finding.to_json(),
        }
        if report.mode == ReportMode.FULL:
            entry["safety_class"] = cf.safety_class.value
            entry["rationale"] = cf.rationale
            entry["resolved_by_sibling"] = cf.resolved_by_sibling
        findings_out.append(entry)

    unapplied_out: list[dict] = []
    for pr in report.unapplied_fixes:
        unapplied_out.append({
            "applied": pr.applied,
            "reason": pr.reason,
            "finding_id": pr.finding_id,
            "path": str(pr.path),
            "before_hash": pr.before_hash,
            "after_hash": pr.after_hash,
        })

    metadata = {
        "timestamp": datetime.now(timezone.utc).isoformat(),
        "mode": report.mode.value,
        "total_findings": len(findings_out),
        "total_unapplied_fixes": len(unapplied_out),
    }
    if corpus_size is not None:
        metadata["corpus_size"] = corpus_size

    return {
        "metadata": metadata,
        "findings": findings_out,
        "unapplied_fixes": unapplied_out,
    }


# ---------------------------------------------------------------------------
# Markdown renderer
# ---------------------------------------------------------------------------

_SEVERITY_ORDER = (Severity.CRITICAL, Severity.HIGH, Severity.MEDIUM, Severity.LOW)


def render_markdown(report: Report) -> str:
    """Render the report as markdown, grouped by severity then safety class."""
    lines: list[str] = []
    mode_label = report.mode.value.upper()
    lines.append(f"# Verify-Roadmap Findings ({mode_label} mode)")
    lines.append("")

    # Summary table
    by_severity: dict[Severity, int] = {s: 0 for s in _SEVERITY_ORDER}
    by_safety: dict[SafetyClass, int] = {
        SafetyClass.EXPOSURE_REVIEW: 0,
        SafetyClass.SAFE_FIX: 0,
    }
    for cf in report.findings:
        by_severity[cf.finding.severity] = by_severity.get(cf.finding.severity, 0) + 1
        by_safety[cf.safety_class] = by_safety.get(cf.safety_class, 0) + 1

    lines.append("## Summary")
    lines.append("")
    lines.append("| Metric | Count |")
    lines.append("|--------|-------|")
    lines.append(f"| Total findings | {len(report.findings)} |")
    for sev in _SEVERITY_ORDER:
        lines.append(f"| {sev.name.capitalize()} | {by_severity[sev]} |")
    if report.mode == ReportMode.FULL:
        lines.append(
            f"| ExposureReview | {by_safety[SafetyClass.EXPOSURE_REVIEW]} |"
        )
        lines.append(f"| SafeFix | {by_safety[SafetyClass.SAFE_FIX]} |")
    lines.append(f"| Unapplied fixes | {len(report.unapplied_fixes)} |")
    lines.append("")

    # Findings grouped by severity then safety class
    sorted_findings = _sort_findings(report.findings)
    grouped = _group_by_severity(sorted_findings)
    for sev in _SEVERITY_ORDER:
        bucket = grouped.get(sev, [])
        if not bucket:
            continue
        lines.append(f"## {sev.name.capitalize()} Severity")
        lines.append("")
        # Sub-group by safety class within severity (ExposureReview first)
        if report.mode == ReportMode.FULL:
            for sc in (SafetyClass.EXPOSURE_REVIEW, SafetyClass.SAFE_FIX):
                sub = [cf for cf in bucket if cf.safety_class == sc]
                if not sub:
                    continue
                label = "ExposureReview" if sc == SafetyClass.EXPOSURE_REVIEW else "SafeFix"
                lines.append(f"### {label}")
                lines.append("")
                for cf in sub:
                    lines.append(_render_md_finding(cf, full=True))
                    lines.append("")
        else:
            # QUICK mode: no safety subdivision
            for cf in bucket:
                lines.append(_render_md_finding(cf, full=False))
                lines.append("")

    # Unapplied fixes section
    if report.unapplied_fixes:
        lines.append("## Unapplied Fixes")
        lines.append("")
        lines.append(
            "These fixes were intended but could not safely complete "
            "(e.g., concurrent modification, malformed file)."
        )
        lines.append("")
        for pr in report.unapplied_fixes:
            lines.append(f"- **{pr.finding_id}** `{pr.path}` — {pr.reason}")
        lines.append("")

    return "\n".join(lines)


def _render_md_finding(cf: ClassifiedFinding, full: bool) -> str:
    """Render a single ClassifiedFinding as a markdown bullet."""
    f = cf.finding
    parts = [f"- **[{f.id}][{f.severity.name.lower()}]** `{f.source}`"]
    if f.source_line is not None:
        parts[0] += f":{f.source_line}"
    parts[0] += f" — {f.description}"
    if f.recommended_fix:
        parts.append(f"  - Fix: {f.recommended_fix}")
    if full:
        label = (
            "ExposureReview" if cf.safety_class == SafetyClass.EXPOSURE_REVIEW
            else "SafeFix"
        )
        parts.append(f"  - Safety: {label} — {cf.rationale}")
        if cf.resolved_by_sibling:
            parts.append(f"  - Resolved by sibling: {cf.resolved_by_sibling}")
    return "\n".join(parts)


# ---------------------------------------------------------------------------
# Console renderer
# ---------------------------------------------------------------------------

# ANSI color codes (used only when color=True and stdout is a TTY)
_C_RESET = "\033[0m"
_C_RED = "\033[31m"
_C_YELLOW = "\033[33m"
_C_BLUE = "\033[34m"
_C_GRAY = "\033[90m"
_C_BOLD = "\033[1m"

_SEVERITY_COLOR = {
    Severity.CRITICAL: _C_RED,
    Severity.HIGH: _C_RED,
    Severity.MEDIUM: _C_YELLOW,
    Severity.LOW: _C_BLUE,
}


def render_console(report: Report, color: bool = True) -> str:
    """Render the report as one-line-per-finding console output.

    SafeFix findings prefixed with [auto], ExposureReview with [review],
    unapplied fixes with [UNAPPLIED]. In QUICK mode, no classification
    prefix is emitted.
    """
    lines: list[str] = []
    mode_label = report.mode.value.upper()
    if color:
        lines.append(f"{_C_BOLD}verify-roadmap [{mode_label}]{_C_RESET}")
    else:
        lines.append(f"verify-roadmap [{mode_label}]")

    if not report.findings and not report.unapplied_fixes:
        lines.append("  (no findings)")
        return "\n".join(lines)

    for cf in _sort_findings(report.findings):
        f = cf.finding
        sev = f.severity.name.lower()
        prefix = ""
        if report.mode == ReportMode.FULL:
            if cf.safety_class == SafetyClass.SAFE_FIX:
                prefix = "[auto]"
            else:
                prefix = "[review]"

        loc = f"{f.source}"
        if f.source_line is not None:
            loc += f":{f.source_line}"

        line = f"  {prefix} [{sev}] {loc} — {f.description}".strip()
        if color:
            c = _SEVERITY_COLOR.get(f.severity, "")
            line = f"{c}{line}{_C_RESET}" if c else line
        lines.append(line)

    for pr in report.unapplied_fixes:
        line = f"  [UNAPPLIED] {pr.path} — {pr.reason} (finding {pr.finding_id})"
        if color:
            line = f"{_C_GRAY}{line}{_C_RESET}"
        lines.append(line)

    return "\n".join(lines)


# ---------------------------------------------------------------------------
# Exit code semantics
# ---------------------------------------------------------------------------

def exit_code_for_findings(report: Report) -> int:
    """Compute the exit code for a report.

    0 = clean (no findings, no unapplied fixes)
    1 = findings present (low/medium/high) or unapplied fixes
    2 = at least one critical finding
    """
    has_critical = any(
        cf.finding.severity == Severity.CRITICAL
        for cf in report.findings
    )
    if has_critical:
        return 2
    if report.findings or report.unapplied_fixes:
        return 1
    return 0


# ---------------------------------------------------------------------------
# write_reports — file I/O
# ---------------------------------------------------------------------------

def write_reports(
    report: Report,
    output_dir: Path,
    corpus_size: int | None = None,
) -> None:
    """Write JSON and markdown reports to output_dir.

    Creates the directory if needed. Files written: findings.json, findings.md.
    These should NOT be committed (use build/verify-roadmap/ as the canonical
    output location).
    """
    output_dir.mkdir(parents=True, exist_ok=True)
    json_data = render_json(report, corpus_size=corpus_size)
    (output_dir / "findings.json").write_text(
        json.dumps(json_data, indent=2) + "\n",
        encoding="utf-8",
    )
    (output_dir / "findings.md").write_text(
        render_markdown(report) + "\n",
        encoding="utf-8",
    )
