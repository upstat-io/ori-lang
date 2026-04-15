"""Paired-finding deduplication for verify-roadmap.

Section 03.1 of verify-roadmap-redesign plan (resolved_by_sibling wiring).

Some findings come in semantically-linked pairs: if one half is auto-fixed,
the other half's root cause disappears with it. The paired-dedup pass runs
AFTER `classify_safety()` on the full classified-finding list and marks the
dependent half with `resolved_by_sibling = <independent-half's Finding.id>`
so the auto-fix engine skips it.

Currently handled pairs:

  - plan: → name: rename on PlanIndexSchema
      Independent: UNKNOWN_FIELD(plan) classified SafeFix rename.
      Dependent:   MISSING_REQUIRED_FIELD(name) on the same file.
      Semantics:   renaming `plan:` to `name:` also resolves the missing-
                   name finding; applying both halves double-writes.

Design invariants (parallel to safety.py):
  1. pair_resolved_by_sibling is PURE — no I/O, no subprocess calls.
  2. The pass is run by the CLI (`quick.py` / future `full.py`) after
     `classify_safety` has produced ClassifiedFinding records; classifier
     internals never call it (classify_safety is single-finding, pairing
     is cross-finding).
  3. Returns NEW ClassifiedFinding instances (dataclass replace) — never
     mutates inputs; inputs remain frozen.
  4. Only the dependent half gets `resolved_by_sibling` set. The
     independent half carries the SafeFix that actually runs.
"""

from __future__ import annotations

from collections import defaultdict
from dataclasses import replace
from pathlib import Path

from scripts.plan_corpus import FindingCategory, FindingSubtype

from .safety import ClassifiedFinding, SafetyClass, _is_plan_index


_RENAME_RATIONALE_PREFIX = "PlanIndexSchema: rename plan: to name:"


def pair_resolved_by_sibling(
    classifieds: list[ClassifiedFinding],
) -> list[ClassifiedFinding]:
    """Mark sibling-resolved findings so auto_fix skips them.

    For each PlanIndex file, if a SafeFix UNKNOWN_FIELD(plan) rename exists
    alongside a MISSING_REQUIRED_FIELD(name), the MISSING_REQUIRED_FIELD is
    returned with `resolved_by_sibling=<rename_finding.id>`. All other
    ClassifiedFindings are returned unchanged (identity preserved).

    Returns a new list — inputs are not mutated (ClassifiedFinding is
    frozen).
    """
    by_file: dict[Path, list[ClassifiedFinding]] = defaultdict(list)
    for cf in classifieds:
        by_file[cf.finding.source].append(cf)

    out: list[ClassifiedFinding] = []
    for cf in classifieds:
        sibling_id = _find_rename_sibling(cf, by_file[cf.finding.source])
        if sibling_id is not None:
            out.append(replace(cf, resolved_by_sibling=sibling_id))
        else:
            out.append(cf)
    return out


def _find_rename_sibling(
    cf: ClassifiedFinding,
    same_file: list[ClassifiedFinding],
) -> str | None:
    """Return the rename-sibling Finding.id if cf is its dependent half."""
    f = cf.finding

    # Only MISSING_REQUIRED_FIELD(name) on PlanIndex can be sibling-resolved.
    if (
        f.category != FindingCategory.SCHEMA_VIOLATION
        or f.subtype != FindingSubtype.MISSING_REQUIRED_FIELD
    ):
        return None
    if "name" not in f.description.lower():
        return None
    if not _is_plan_index(f.source):
        return None

    for other in same_file:
        if other is cf:
            continue
        of = other.finding
        if (
            of.category == FindingCategory.SCHEMA_VIOLATION
            and of.subtype == FindingSubtype.UNKNOWN_FIELD
            and other.safety_class == SafetyClass.SAFE_FIX
            and other.rationale.startswith(_RENAME_RATIONALE_PREFIX)
        ):
            return of.id

    return None
