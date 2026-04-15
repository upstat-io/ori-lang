"""Schema definitions and validation for the seven plan file classes.

Contains FileClass, classify_file, the FILE_CLASS_META registry (pattern /
display-name / schema class / validator / status-enum), all _check_*
helpers, all _validate_* per-schema functions, and validate().

Dataclass schema shapes and status enums are homed in `.schemas` (the SSOT
for schema *shape* and constraint *values*). This module owns the *file
classification* + *validation dispatch* built on top of those shapes.
"""

from __future__ import annotations

import enum
import re
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Callable

from .types import (
    Finding,
    FindingCategory,
    FindingSubtype,
    Outcome,
    Severity,
    PLANS_DIR,
    ROADMAP_DIR,
    FIX_BUG_RE,
    SECTION_FILE_RE,
    OVERVIEW_RE,
    INTRA_PLAN_DEP_RE,
    CROSS_PLAN_DEP_RE,
    FULL_PATH_DEP_RE,
)
from .schemas import (
    PlanIndexSchema,
    PlanSectionSchema,
    RoadmapSectionSchema,
    OverviewSchema,
    BugTrackerSectionSchema,
    FixBugSchema,
    CompletedIndexSchema,
    SubsectionEntry,
    TprInfo,
    PLAN_STATUSES,
    SECTION_STATUSES,
    OVERVIEW_STATUSES,
    FIX_STATUSES,
    TPR_STATUSES,
    SEVERITY_VALUES,
    COMPLETED_STATUSES,
    _schema_required_fields,
    _schema_allowed_fields,
)


# ---------------------------------------------------------------------------
# File classification
# ---------------------------------------------------------------------------


class FileClass(enum.Enum):
    """The seven schema classes for plan files."""
    PLAN_INDEX = "plan_index"
    PLAN_SECTION = "plan_section"
    ROADMAP_SECTION = "roadmap_section"
    OVERVIEW = "overview"
    BUG_TRACKER_SECTION = "bug_tracker_section"
    FIX_BUG = "fix_bug"
    COMPLETED_INDEX = "completed_index"


def classify_file(path: Path) -> FileClass | None:
    """Classify a plan file into one of the seven schema classes.

    Anchors on the first `plans` segment in the resolved path so the
    classifier works both on real PLANS_DIR paths AND on synthetic
    tmp_path corpora used by tests. Falls back to path.parts when no
    `plans` segment exists (exact pre-refactor behavior).
    """
    path = path.resolve()
    if path.is_relative_to(PLANS_DIR):
        parts = path.relative_to(PLANS_DIR).parts
    else:
        full_parts = path.parts
        # Scan for the first 'plans' segment and take everything after it.
        try:
            idx = full_parts.index("plans")
            parts = full_parts[idx + 1:]
        except ValueError:
            parts = full_parts

    name = path.name

    # Completed plan indexes
    if len(parts) >= 2 and parts[0] == "completed" and name == "index.md":
        return FileClass.COMPLETED_INDEX

    # Bug-tracker fix files
    if len(parts) >= 2 and parts[0] == "bug-tracker" and FIX_BUG_RE.match(name):
        return FileClass.FIX_BUG

    # Bug-tracker section files
    if len(parts) >= 2 and parts[0] == "bug-tracker" and SECTION_FILE_RE.match(name):
        return FileClass.BUG_TRACKER_SECTION

    # Roadmap section files
    if len(parts) >= 2 and parts[0] == "roadmap" and SECTION_FILE_RE.match(name):
        return FileClass.ROADMAP_SECTION

    # Overview files
    if OVERVIEW_RE.match(name):
        return FileClass.OVERVIEW

    # Plan index files
    if name == "index.md":
        return FileClass.PLAN_INDEX

    # Plan section files
    if SECTION_FILE_RE.match(name):
        return FileClass.PLAN_SECTION

    return None


def _validate_sections(data: dict, path: Path) -> list[Finding]:
    """Validate each entry in the sections[] list matches SubsectionEntry shape.

    Emits one finding per malformed entry (unknown field, missing required field,
    or enum violation on status). Silently returns [] if sections is absent or
    empty — the required-fields check at the top level handles absence.
    """
    findings: list[Finding] = []
    sections = data.get("sections")
    if sections is None:
        return findings
    if not isinstance(sections, list):
        findings.append(Finding(
            category=FindingCategory.SCHEMA_VIOLATION,
            subtype=FindingSubtype.WRONG_TYPE,
            severity=Severity.HIGH,
            source=path,
            description="sections must be a list",
            recommended_fix="Use sections: [{id: NN.N, title: ..., status: ...}, ...]",
        ))
        return findings
    for idx, entry in enumerate(sections):
        if not isinstance(entry, dict):
            findings.append(Finding(
                category=FindingCategory.SCHEMA_VIOLATION,
                subtype=FindingSubtype.WRONG_TYPE,
                severity=Severity.HIGH,
                source=path,
                description=f"sections[{idx}] must be a mapping, got {type(entry).__name__}",
                recommended_fix="Use a mapping with id/title/status keys",
            ))
            continue
        for required_field in _schema_required_fields(SubsectionEntry):
            if required_field not in entry:
                findings.append(Finding(
                    category=FindingCategory.SCHEMA_VIOLATION,
                    subtype=FindingSubtype.MISSING_REQUIRED_FIELD,
                    severity=Severity.HIGH,
                    source=path,
                    description=f"sections[{idx}] missing required field {required_field!r}",
                    recommended_fix=f"Add {required_field}: ... to the entry",
                    # Path-style target_key for nested fields so future auto-fix
                    # extensions can dispatch structurally instead of parsing
                    # the description prose (TPR-03-005-gemini informational).
                    target_key=f"sections[{idx}].{required_field}",
                ))
        for key in entry:
            if key not in _schema_allowed_fields(SubsectionEntry):
                findings.append(Finding(
                    category=FindingCategory.SCHEMA_VIOLATION,
                    subtype=FindingSubtype.UNKNOWN_FIELD,
                    severity=Severity.MEDIUM,
                    source=path,
                    description=f"sections[{idx}] has unknown field {key!r}",
                    recommended_fix=f"Remove {key!r} or add it to the SubsectionEntry schema",
                    target_key=f"sections[{idx}].{key}",
                ))
        status = entry.get("status")
        if status is not None and status not in SECTION_STATUSES:
            findings.append(Finding(
                category=FindingCategory.SCHEMA_VIOLATION,
                subtype=FindingSubtype.ENUM_OUT_OF_RANGE,
                severity=Severity.MEDIUM,
                source=path,
                description=(
                    f"sections[{idx}].status={status!r} not in {sorted(SECTION_STATUSES)}"
                ),
                recommended_fix=f"Use one of {sorted(SECTION_STATUSES)}",
                target_key=f"sections[{idx}].status",
            ))
    return findings


# ---------------------------------------------------------------------------
# Validation helpers
# ---------------------------------------------------------------------------


def _validate_tpr_info(raw: Any, path: Path) -> tuple[TprInfo | None, list[Finding]]:
    """Validate a third_party_review block."""
    findings: list[Finding] = []
    if raw is None:
        return None, findings
    if not isinstance(raw, dict):
        findings.append(Finding(
            category=FindingCategory.SCHEMA_VIOLATION,
            subtype=FindingSubtype.WRONG_TYPE,
            severity=Severity.HIGH,
            source=path,
            description="third_party_review must be a mapping",
            recommended_fix="Use third_party_review: {status: none, updated: null}",
        ))
        return None, findings
    status = raw.get("status")
    if status is not None:
        status = str(status)
    updated = raw.get("updated")
    if updated is not None:
        updated = str(updated)
    if status and status not in TPR_STATUSES:
        findings.append(Finding(
            category=FindingCategory.SCHEMA_VIOLATION,
            subtype=FindingSubtype.ENUM_OUT_OF_RANGE,
            severity=Severity.HIGH,
            source=path,
            description=f"third_party_review.status={status!r} not in {sorted(TPR_STATUSES)}",
            recommended_fix=f"Use one of: {', '.join(sorted(TPR_STATUSES))}",
        ))
    if status and status != "none" and updated is None:
        findings.append(Finding(
            category=FindingCategory.STATUS_CONTRADICTION,
            subtype=FindingSubtype.TPR_STATUS_WITHOUT_DATE,
            severity=Severity.MEDIUM,
            source=path,
            description=f"third_party_review.status={status!r} but updated is null",
            recommended_fix="Set third_party_review.updated to today's date",
        ))
    if status == "none" and updated is not None and updated != "null":
        findings.append(Finding(
            category=FindingCategory.STATUS_CONTRADICTION,
            subtype=FindingSubtype.TPR_STATUS_NONE_WITH_DATE,
            severity=Severity.LOW,
            source=path,
            description=f"third_party_review.status=none but updated={updated!r}",
            recommended_fix="Set updated to null when status is none",
        ))
    return TprInfo(status=status or "none", updated=updated), findings


def _check_required(
    data: dict, required: list[str], path: Path
) -> list[Finding]:
    findings = []
    for key in required:
        if key not in data:
            findings.append(Finding(
                category=FindingCategory.SCHEMA_VIOLATION,
                subtype=FindingSubtype.MISSING_REQUIRED_FIELD,
                severity=Severity.HIGH,
                source=path,
                description=f"missing required field: {key}",
                recommended_fix=f"Add '{key}:' to frontmatter",
                target_key=key,
            ))
    return findings


def _check_unknown_fields(
    data: dict, allowed: frozenset[str], path: Path
) -> list[Finding]:
    findings = []
    for key in data:
        if key not in allowed:
            findings.append(Finding(
                category=FindingCategory.SCHEMA_VIOLATION,
                subtype=FindingSubtype.UNKNOWN_FIELD,
                severity=Severity.MEDIUM,
                source=path,
                description=f"unknown field: {key!r}",
                recommended_fix=f"Remove '{key}' or check for typos. Allowed: {sorted(allowed)}",
                target_key=key,
            ))
    return findings


def _check_enum(
    data: dict, key: str, allowed: frozenset[str], path: Path
) -> list[Finding]:
    val = data.get(key)
    if val is None:
        return []
    val_str = str(val).strip()
    # Strip inline comments (e.g. "in-progress  # TPR done")
    if "#" in val_str:
        val_str = val_str.split("#")[0].strip()
    if val_str not in allowed:
        return [Finding(
            category=FindingCategory.SCHEMA_VIOLATION,
            subtype=FindingSubtype.ENUM_OUT_OF_RANGE,
            severity=Severity.HIGH,
            source=path,
            description=f"{key}={val!r} not in {sorted(allowed)}",
            recommended_fix=f"Use one of: {', '.join(sorted(allowed))}",
        )]
    return []


def _validate_dep_id(dep: Any, path: Path) -> list[Finding]:
    """Validate a single depends_on entry."""
    if not isinstance(dep, str):
        return [Finding(
            category=FindingCategory.SCHEMA_VIOLATION,
            subtype=FindingSubtype.DEP_ID_MALFORMED,
            severity=Severity.HIGH,
            source=path,
            description=f"depends_on entry must be a string, got {type(dep).__name__}",
            recommended_fix="Use depends_on: [\"01\", \"02\"]",
        )]
    if FULL_PATH_DEP_RE.match(dep):
        return [Finding(
            category=FindingCategory.SCHEMA_VIOLATION,
            subtype=FindingSubtype.DEP_ID_FULL_PATH,
            severity=Severity.HIGH,
            source=path,
            description=f"depends_on uses full path: {dep!r}",
            recommended_fix="Use logical ID (\"01\") or cross-plan (\"plan-name#01\")",
        )]
    if not INTRA_PLAN_DEP_RE.match(dep) and not CROSS_PLAN_DEP_RE.match(dep):
        return [Finding(
            category=FindingCategory.SCHEMA_VIOLATION,
            subtype=FindingSubtype.DEP_ID_MALFORMED,
            severity=Severity.HIGH,
            source=path,
            description=f"malformed depends_on ID: {dep!r}",
            recommended_fix="Use \"NN\" (intra-plan) or \"plan-name#NN\" (cross-plan)",
        )]
    return []


def _validate_depends_on(data: dict, path: Path) -> list[Finding]:
    """Validate the depends_on field if present."""
    raw = data.get("depends_on")
    if raw is None:
        return []
    if isinstance(raw, str):
        return [Finding(
            category=FindingCategory.SCHEMA_VIOLATION,
            subtype=FindingSubtype.DEP_ID_MALFORMED,
            severity=Severity.HIGH,
            source=path,
            description="depends_on must be a list, not a bare string (would iterate chars)",
            recommended_fix=f'Use depends_on: ["{raw}"]',
        )]
    if not isinstance(raw, list):
        return [Finding(
            category=FindingCategory.SCHEMA_VIOLATION,
            subtype=FindingSubtype.WRONG_TYPE,
            severity=Severity.HIGH,
            source=path,
            description=f"depends_on must be a list, got {type(raw).__name__}",
            recommended_fix="Use depends_on: [\"01\"]",
        )]
    findings = []
    for dep in raw:
        findings.extend(_validate_dep_id(dep, path))
    return findings


# ---------------------------------------------------------------------------
# Per-schema validators — required/allowed derived from dataclasses above
# ---------------------------------------------------------------------------


def _validate_plan_index(data: dict, path: Path) -> list[Finding]:
    findings = []
    findings.extend(_check_required(data, _schema_required_fields(PlanIndexSchema), path))
    findings.extend(_check_unknown_fields(data, _schema_allowed_fields(PlanIndexSchema), path))
    findings.extend(_check_enum(data, "status", PLAN_STATUSES, path))
    if data.get("reroute") is False:
        findings.append(Finding(
            category=FindingCategory.SCHEMA_VIOLATION,
            subtype=FindingSubtype.CROSS_FIELD_INVARIANT,
            severity=Severity.MEDIUM,
            source=path,
            description="reroute: false is invalid; omit the field instead",
            recommended_fix="Remove the reroute field entirely",
            target_key="reroute",
        ))
    return findings


def _validate_plan_section(data: dict, path: Path) -> list[Finding]:
    findings = []
    findings.extend(_check_required(data, _schema_required_fields(PlanSectionSchema), path))
    findings.extend(_check_unknown_fields(data, _schema_allowed_fields(PlanSectionSchema), path))
    findings.extend(_check_enum(data, "status", SECTION_STATUSES, path))
    findings.extend(_validate_depends_on(data, path))
    findings.extend(_validate_sections(data, path))
    _, tpr_findings = _validate_tpr_info(data.get("third_party_review"), path)
    findings.extend(tpr_findings)
    return findings


def _validate_roadmap_section(data: dict, path: Path) -> list[Finding]:
    findings = []
    findings.extend(_check_required(data, _schema_required_fields(RoadmapSectionSchema), path))
    findings.extend(_check_unknown_fields(data, _schema_allowed_fields(RoadmapSectionSchema), path))
    findings.extend(_check_enum(data, "status", SECTION_STATUSES, path))
    findings.extend(_validate_depends_on(data, path))
    findings.extend(_validate_sections(data, path))
    if "third_party_review" in data:
        _, tpr_findings = _validate_tpr_info(data.get("third_party_review"), path)
        findings.extend(tpr_findings)
    return findings


def _validate_overview(data: dict, path: Path) -> list[Finding]:
    findings = []
    findings.extend(_check_required(data, _schema_required_fields(OverviewSchema), path))
    findings.extend(_check_unknown_fields(data, _schema_allowed_fields(OverviewSchema), path))
    findings.extend(_check_enum(data, "status", OVERVIEW_STATUSES, path))
    return findings


def _validate_bug_section(data: dict, path: Path) -> list[Finding]:
    findings = []
    findings.extend(_check_required(data, _schema_required_fields(BugTrackerSectionSchema), path))
    findings.extend(_check_unknown_fields(data, _schema_allowed_fields(BugTrackerSectionSchema), path))
    findings.extend(_check_enum(data, "status", SECTION_STATUSES, path))
    return findings


def _validate_fix_bug(data: dict, path: Path) -> list[Finding]:
    findings = []
    findings.extend(_check_required(data, _schema_required_fields(FixBugSchema), path))
    findings.extend(_check_unknown_fields(data, _schema_allowed_fields(FixBugSchema), path))
    findings.extend(_check_enum(data, "status", FIX_STATUSES, path))
    findings.extend(_check_enum(data, "severity", SEVERITY_VALUES, path))
    _, tpr_findings = _validate_tpr_info(data.get("third_party_review"), path)
    findings.extend(tpr_findings)
    return findings


def _validate_completed_index(data: dict, path: Path) -> list[Finding]:
    findings = []
    findings.extend(_check_required(data, _schema_required_fields(CompletedIndexSchema), path))
    findings.extend(_check_unknown_fields(data, _schema_allowed_fields(CompletedIndexSchema), path))
    findings.extend(_check_enum(data, "status", COMPLETED_STATUSES, path))
    return findings


# ---------------------------------------------------------------------------
# §06.2 — `## Intelligence Reconnaissance` body-level validator
# ---------------------------------------------------------------------------
#
# Detection rules (from `plans/query-intel-adoption/section-06-plan-schema-recon.md`
# §06.2 implementation item 6):
#
# * **Missing block** — no `## Intelligence Reconnaissance` header in body.
#   status=not-started → Severity.HIGH + Outcome.WARNING (ERROR under --strict-recon).
#   status=in-progress → Severity.MEDIUM + Outcome.WARNING (unaffected by --strict-recon).
#   status=complete → exempt (0 findings).
#
# * **Graph-unavailable documentation** (header + ISO date + an unavailability
#   phrase) — Severity.LOW + Outcome.WARNING, distinct `RECON_GRAPH_UNAVAILABLE`
#   subtype. PASSES the anti-stub check (not a performative ritual — author
#   ran the availability check and recorded the result).
#
# * **Stub / performative-ritual block** (header present but fails concrete-
#   content checks) — `VALIDATION_BYPASS` subtype, same severity/outcome
#   mapping as missing-block. Triggers on: empty/whitespace, placeholder-
#   only, missing query line, missing date, missing citation, citation
#   marker followed by placeholder within 20 chars ("mixed-placeholder-
#   after-citation"), or a bare `@`-include without a summary paragraph.
#
# Format-coupling contract (§03/§06/§07): ≤500-char bound +
# `[ori]` / `[repo#N]` / `[repo:path]` citation vocabulary +
# `.claude/skills/dual-tpr/compose-intel-summary.md` SSOT. Exact line-level
# formatting may vary per consumer rendering context; anti-stub detection
# enforces the citation-grammar half.

_RECON_HEADER_RE = re.compile(
    r"^## Intelligence Reconnaissance\s*$", re.MULTILINE,
)
_NEXT_SECTION_RE = re.compile(r"^## ", re.MULTILINE)
_HTML_COMMENT_RE = re.compile(r"<!--.*?-->", re.DOTALL)

_RECON_DATE_RE = re.compile(r"\b\d{4}-\d{2}-\d{2}\b")
_RECON_QUERY_RE = re.compile(r"\bscripts/intel-query\.sh\b")
_RECON_ORI_CITATION_RE = re.compile(r"\[ori\]")
# `[rust#123]` or `[repo:path/to/symbol]` — generic pattern matches any
# reference-repo citation without hardcoding the repo list.
_RECON_REPO_CITATION_RE = re.compile(r"\[[a-z][a-z0-9-]*[#:][^\]]+\]")
_RECON_AT_INCLUDE_RE = re.compile(
    r"@\.claude/skills/dual-tpr/compose-intel-summary\.md",
)

# Placeholder tokens (case-insensitive, whole-token). Used both for the
# "only-placeholders" check and the mixed-placeholder-after-citation scan.
_RECON_PLACEHOLDER_PATTERN = (
    r"\bTBD\b|\bTODO\b|\bNONE\b|\bN/?A\b|\bXXX\b|\(empty\)|\.{3}|…"
)
_RECON_PLACEHOLDER_RE = re.compile(_RECON_PLACEHOLDER_PATTERN, re.IGNORECASE)
# `[ori]` OR `[repo#` / `[repo:` — start of any citation marker.
_RECON_MIXED_CITATION_RE = re.compile(
    r"(\[ori\]|\[[a-z][a-z0-9-]*[#:])"
    r"[^\n]{0,20}?"
    r"(" + _RECON_PLACEHOLDER_PATTERN + r")",
    re.IGNORECASE,
)

_RECON_GRAPH_UNAVAIL_PHRASES = (
    "graph was unavailable",
    "intelligence graph unavailable",
    "graph unavailable",
)

_RECON_SSOT_HINT = (
    "See '.claude/skills/create-plan/plan-schema.md' Section File Template and "
    "'.claude/skills/dual-tpr/compose-intel-summary.md' Step D for the query "
    "protocol and summary format ([ori] / [repo#N] / [repo:path] citations, "
    "≤500 chars). If the graph is unavailable, record the unavailability as "
    "freeform prose with an ISO date — do NOT omit the block."
)


def _extract_recon_block(body: str) -> str | None:
    """Extract the `## Intelligence Reconnaissance` block body.

    Returns None if the header is absent. Otherwise returns the text from
    the line AFTER the header to the next `## ` section header or EOF,
    with HTML comments stripped (they are metadata, not content).
    """
    header = _RECON_HEADER_RE.search(body)
    if not header:
        return None
    start = header.end()
    next_section = _NEXT_SECTION_RE.search(body, start)
    end = next_section.start() if next_section else len(body)
    block = body[start:end]
    return _HTML_COMMENT_RE.sub("", block)


def _check_intel_recon_block(
    data: dict, body: str, path: Path, strict_recon: bool = False,
) -> list[Finding]:
    """Body-level validator for PLAN_SECTION recon-block mandate.

    See the `§06.2` header comment for the full detection rules. Returns
    a list with AT MOST ONE finding per invocation — the first failing
    check wins, so consumers see the primary violation rather than a
    cascade.
    """
    status = data.get("status", "")
    # `complete` sections are totally exempt — the retrofit / enforcement
    # surface is `not-started` and `in-progress` only.
    if status == "complete":
        return []

    # Severity + Outcome for missing/stub findings, gated on status.
    if status == "in-progress":
        base_sev = Severity.MEDIUM
        base_outcome = Outcome.WARNING  # --strict-recon does NOT escalate in-progress
    else:
        # not-started (and any unknown status — treat as not-started
        # defensively; the schema validator will emit its own ENUM_OUT_OF_RANGE).
        base_sev = Severity.HIGH
        base_outcome = Outcome.ERROR if strict_recon else Outcome.WARNING

    block = _extract_recon_block(body)
    if block is None:
        return [Finding(
            category=FindingCategory.GAP,
            subtype=FindingSubtype.MISSING_RECON_BLOCK,
            severity=base_sev,
            source=path,
            description=(
                "Section lacks a '## Intelligence Reconnaissance' block. "
                "Every PLAN_SECTION author must record graph queries run + "
                "≤500-char summary + ISO date before implementation."
            ),
            recommended_fix=_RECON_SSOT_HINT,
            outcome=base_outcome,
        )]

    stripped = block.strip()
    has_date = bool(_RECON_DATE_RE.search(block))
    lower = block.lower()

    # Graph-unavailable takes precedence: legitimate documentation shape,
    # distinct from stubs. Requires a date + one of the unavailability
    # phrases so authors can't pass the check with a bare "unavailable"
    # comment.
    if has_date and any(p in lower for p in _RECON_GRAPH_UNAVAIL_PHRASES):
        return [Finding(
            category=FindingCategory.GAP,
            subtype=FindingSubtype.RECON_GRAPH_UNAVAILABLE,
            severity=Severity.LOW,
            source=path,
            description=(
                "Section records graph-unavailable state. This is a distinct, "
                "non-gating finding — the author ran the availability check "
                "and recorded that the graph was down. Fill in full queries "
                "if/when the graph becomes available."
            ),
            recommended_fix=(
                "When `scripts/intel-query.sh status` returns ok, run the "
                "canonical recon queries and update this block with the "
                "results summary and [ori] / [repo#N] citations."
            ),
            outcome=Outcome.WARNING,
        )]

    # Empty / whitespace-only after the header.
    if not stripped:
        return [Finding(
            category=FindingCategory.GAP,
            subtype=FindingSubtype.VALIDATION_BYPASS,
            severity=base_sev,
            source=path,
            description=(
                "'## Intelligence Reconnaissance' header present but body is "
                "empty (performative-ritual stub)."
            ),
            recommended_fix=_RECON_SSOT_HINT,
            outcome=base_outcome,
        )]

    # Only-placeholder tokens: strip all placeholder tokens + whitespace;
    # if nothing remains, the body is a ritual stub.
    placeholder_free = _RECON_PLACEHOLDER_RE.sub("", stripped).strip()
    if not placeholder_free:
        return [Finding(
            category=FindingCategory.GAP,
            subtype=FindingSubtype.VALIDATION_BYPASS,
            severity=base_sev,
            source=path,
            description=(
                "'## Intelligence Reconnaissance' block contains only "
                "placeholder tokens (TBD / TODO / n/a / none / (empty) / ...). "
                "This is a performative-ritual stub — placeholder tokens do "
                "not record real reconnaissance."
            ),
            recommended_fix=_RECON_SSOT_HINT,
            outcome=base_outcome,
        )]

    # Bare `@`-include with no surrounding summary. The @-include is a
    # SOURCE for Claude's prompt, NOT a substitute for a plan-resident
    # snapshot. Strip the include text and see if anything substantive
    # remains.
    if _RECON_AT_INCLUDE_RE.search(block):
        without_at = _RECON_AT_INCLUDE_RE.sub("", stripped).strip()
        if not without_at:
            return [Finding(
                category=FindingCategory.GAP,
                subtype=FindingSubtype.VALIDATION_BYPASS,
                severity=base_sev,
                source=path,
                description=(
                    "'## Intelligence Reconnaissance' block contains only the "
                    "literal `@.claude/skills/dual-tpr/compose-intel-summary.md` "
                    "SSOT reference. The @-include is a SOURCE for Claude's "
                    "prompt — NOT a substitute for the plan-resident snapshot."
                ),
                recommended_fix=(
                    "Add a condensed ≤500-char summary paragraph alongside the "
                    "@-include, with the literal scripts/intel-query.sh commands "
                    "run, an ISO YYYY-MM-DD date, and at least one "
                    "[ori] / [repo#N] / [repo:path] citation marker."
                ),
                outcome=base_outcome,
            )]

    # Concrete-content checks: query line, ISO date, citation marker.
    has_query = bool(_RECON_QUERY_RE.search(block))
    has_citation = bool(
        _RECON_ORI_CITATION_RE.search(block)
        or _RECON_REPO_CITATION_RE.search(block)
    )
    missing: list[str] = []
    if not has_query:
        missing.append("scripts/intel-query.sh command line")
    if not has_date:
        missing.append("ISO date marker (YYYY-MM-DD)")
    if not has_citation:
        missing.append("citation marker ([ori] / [repo#N] / [repo:path])")
    if missing:
        return [Finding(
            category=FindingCategory.GAP,
            subtype=FindingSubtype.VALIDATION_BYPASS,
            severity=base_sev,
            source=path,
            description=(
                f"'## Intelligence Reconnaissance' block is missing: "
                f"{', '.join(missing)}. Performative-ritual stub."
            ),
            recommended_fix=_RECON_SSOT_HINT,
            outcome=base_outcome,
        )]

    # Mixed-placeholder-after-citation — the citation marker provides false
    # cover for a TBD/TODO content line within 20 chars. Example: `[ori] TBD`.
    if _RECON_MIXED_CITATION_RE.search(block):
        return [Finding(
            category=FindingCategory.GAP,
            subtype=FindingSubtype.VALIDATION_BYPASS,
            severity=base_sev,
            source=path,
            description=(
                "'## Intelligence Reconnaissance' has a citation marker "
                "followed by a placeholder token within 20 characters "
                "(mixed-placeholder-after-citation). The citation marker "
                "provides false cover — the content is not real TBD."
            ),
            recommended_fix=(
                "Replace the placeholder after the citation marker with the "
                "actual summary substance. Example: replace `[ori] TBD` with "
                "`[ori] <what the graph query actually returned>`."
            ),
            outcome=base_outcome,
        )]

    # All checks passed — block is complete.
    return []


# ---------------------------------------------------------------------------
# FileClass metadata registry (SSOT for per-class display / pattern / schema /
# validator — queried by docgen.py, validate(), and any other consumer)
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class FileClassMeta:
    """Per-FileClass metadata used by validation, docgen, and discovery.

    `display_name` and `pattern` drive human-readable documentation; `schema_cls`
    is the dataclass SSOT for fields; `validator` performs per-class semantic
    checks on parsed frontmatter; `body_validator` (optional) performs
    body-level checks that need the raw post-frontmatter text, threaded with
    the `strict_recon` enforcement flag.

    The body validator is None for classes that don't need body-level checks.
    Adding a body validator is explicit opt-in — ROADMAP_SECTION /
    BUG_TRACKER_SECTION / FIX_BUG / overview / index classes remain None so
    §06.2's recon-block mandate is confined to PLAN_SECTION.

    Rationale for shape (a) over a parallel body-check phase (shape b):
    one dispatch mechanism avoids duplicate plumbing and keeps the
    class-keyed registry that docgen relies on. See §06.2 Design Decision 5.
    """
    display_name: str
    pattern: str
    schema_cls: type
    validator: Callable[[dict, Path], list[Finding]]
    body_validator: Callable[[dict, str, Path, bool], list[Finding]] | None = None


FILE_CLASS_META: dict[FileClass, FileClassMeta] = {
    FileClass.PLAN_INDEX: FileClassMeta(
        "Plan Index", "plans/*/index.md", PlanIndexSchema, _validate_plan_index),
    FileClass.PLAN_SECTION: FileClassMeta(
        "Plan Section", "plans/*/section-*.md", PlanSectionSchema, _validate_plan_section,
        body_validator=_check_intel_recon_block),
    FileClass.ROADMAP_SECTION: FileClassMeta(
        "Roadmap Section", "plans/roadmap/section-*.md", RoadmapSectionSchema,
        _validate_roadmap_section),
    FileClass.OVERVIEW: FileClassMeta(
        "Overview", "plans/*/00-overview.md", OverviewSchema, _validate_overview),
    FileClass.BUG_TRACKER_SECTION: FileClassMeta(
        "Bug Tracker Section", "plans/bug-tracker/section-*.md",
        BugTrackerSectionSchema, _validate_bug_section),
    FileClass.FIX_BUG: FileClassMeta(
        "Fix Bug", "plans/bug-tracker/fix-BUG-*.md", FixBugSchema, _validate_fix_bug),
    FileClass.COMPLETED_INDEX: FileClassMeta(
        "Completed Index", "plans/completed/*/index.md", CompletedIndexSchema,
        _validate_completed_index),
}


def validate(
    file_class: FileClass,
    data: dict,
    path: Path,
    *,
    body: str = "",
    strict_recon: bool = False,
) -> list[Finding]:
    """Validate parsed frontmatter AND body text against the file-class schema.

    Runs the frontmatter `validator` first, then (if present) the
    `body_validator`, concatenating findings in that order. `body` and
    `strict_recon` are keyword-only with defaults — the 3-positional form
    `validate(fc, data, path)` remains the stable public contract for
    frontmatter-only callers (tests, ad-hoc invocations). The body-aware
    path is opt-in via keyword.

    Callers: `discovery.load_and_validate` — the single production call
    site — passes both keywords. Frontmatter-only tests skip them.
    """
    meta = FILE_CLASS_META.get(file_class)
    if meta is None:
        return []
    findings = list(meta.validator(data, path))
    if meta.body_validator is not None:
        findings.extend(meta.body_validator(data, body, path, strict_recon))
    return findings
