"""DepId resolution and schema reference markdown generation.

Contains resolve_dep, _find_section_file, and generate_schema_reference.

The schema reference is derived from the `@dataclass` schemas in
`schemas.py` via `dataclasses.fields()` and the `FILE_CLASS_META` registry
in `schema.py` — no parallel allowlist of required/optional field names
or per-class (pattern, display_name, schema_cls) tuples. Updating a
dataclass or adding a FileClass entry regenerates the docs on the next
`docgen --check` run.
"""

from __future__ import annotations

from pathlib import Path

from .types import (
    Finding,
    FindingCategory,
    FindingSubtype,
    Severity,
    INTRA_PLAN_DEP_RE,
    CROSS_PLAN_DEP_RE,
    _CATEGORY_SUBTYPES,
)
from .schemas import (
    PLAN_STATUSES,
    SECTION_STATUSES,
    OVERVIEW_STATUSES,
    FIX_STATUSES,
    TPR_STATUSES,
    COMPLETED_STATUSES,
    _schema_required_fields,
    _schema_allowed_fields,
)
from .schema import FILE_CLASS_META
from .discovery import Corpus


# ---------------------------------------------------------------------------
# DepId resolution
# ---------------------------------------------------------------------------


def resolve_dep(
    dep_id: str, current_plan_dir: Path, corpus: Corpus
) -> Path | Finding:
    """Resolve a depends_on ID to a concrete section file path."""
    m = CROSS_PLAN_DEP_RE.match(dep_id)
    if m:
        plan_name, section_id = m.group(1), m.group(2)
        target_dir = corpus.name_index.get(plan_name)
        if target_dir is None:
            return Finding(
                category=FindingCategory.DEAD_REFERENCE,
                subtype=FindingSubtype.CROSS_PLAN_NAME_NOT_FOUND,
                severity=Severity.HIGH,
                source=current_plan_dir / "index.md",
                description=f"cross-plan dep references unknown plan name: {plan_name!r}",
                recommended_fix=f"Known plan names: {sorted(corpus.name_index.keys())}",
            )
        return _find_section_file(target_dir, section_id)

    if INTRA_PLAN_DEP_RE.match(dep_id):
        return _find_section_file(current_plan_dir, dep_id)

    return Finding(
        category=FindingCategory.SCHEMA_VIOLATION,
        subtype=FindingSubtype.DEP_ID_MALFORMED,
        severity=Severity.HIGH,
        source=current_plan_dir / "index.md",
        description=f"malformed dep ID: {dep_id!r}",
        recommended_fix="Use \"NN\" or \"plan-name#NN\"",
    )


def _find_section_file(plan_dir: Path, section_id: str) -> Path | Finding:
    """Find a section file by ID within a plan directory."""
    candidates = list(plan_dir.glob(f"section-{section_id}*.md"))
    if not candidates:
        padded = section_id.zfill(2)
        candidates = list(plan_dir.glob(f"section-{padded}*.md"))
    if candidates:
        return candidates[0]
    return Finding(
        category=FindingCategory.DEAD_REFERENCE,
        subtype=FindingSubtype.SECTION_FILE_NOT_FOUND,
        severity=Severity.HIGH,
        source=plan_dir,
        description=f"no section file matching section-{section_id}*.md in {plan_dir}",
        recommended_fix="Check the section ID or create the missing file",
    )


# ---------------------------------------------------------------------------
# Schema Reference generation
# ---------------------------------------------------------------------------


def _partition_fields(cls) -> tuple[list[str], list[str]]:
    """Split a dataclass's fields into (required, optional) by default presence.

    Delegates to the canonical schemas.py helpers to avoid LEAK:algorithmic-duplication.
    Required fields preserve declaration order; optional fields are sorted alphabetically
    for stable markdown diffs.
    """
    required = _schema_required_fields(cls)
    all_fields = _schema_allowed_fields(cls)
    optional = sorted(all_fields - set(required))
    return required, optional


def generate_schema_reference() -> str:
    """Generate markdown documentation from the dataclass schemas."""
    lines = [
        "<!-- GENERATED from scripts/plan_corpus/ — do not edit -->",
        "",
        "# Plan Schema Reference",
        "",
        "Auto-generated from Python dataclass definitions in `scripts/plan_corpus/schemas.py`.",
        "",
        "## Status Enums",
        "",
        f"- **Plan statuses**: {', '.join(sorted(PLAN_STATUSES))}",
        f"- **Section statuses**: {', '.join(sorted(SECTION_STATUSES))}",
        f"- **Overview statuses**: {', '.join(sorted(OVERVIEW_STATUSES))}",
        f"- **Fix statuses**: {', '.join(sorted(FIX_STATUSES))}",
        f"- **TPR statuses**: {', '.join(sorted(TPR_STATUSES))}",
        f"- **Completed plan statuses**: {', '.join(sorted(COMPLETED_STATUSES))}",
        "",
        "## File Classes",
        "",
    ]

    # Render in FILE_CLASS_META declaration order — matches FileClass enum order,
    # which matches the classify_file() decision order.
    for _file_class, meta in FILE_CLASS_META.items():
        required, optional = _partition_fields(meta.schema_cls)
        lines.append(f"### {meta.display_name}")
        lines.append(f"**Pattern**: `{meta.pattern}`")
        lines.append(f"**Required**: {', '.join(f'`{r}`' for r in required)}")
        if optional:
            lines.append(f"**Optional**: {', '.join(f'`{o}`' for o in optional)}")
        lines.append("")

    lines.append("## Finding Categories")
    lines.append("")
    for cat in FindingCategory:
        subtypes = sorted(s.value for s in _CATEGORY_SUBTYPES.get(cat, []))
        lines.append(f"### {cat.value}")
        for st in subtypes:
            lines.append(f"- `{st}`")
        lines.append("")

    return "\n".join(lines)
