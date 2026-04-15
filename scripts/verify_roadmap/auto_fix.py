"""Auto-fix engine for verify-roadmap.

Section 03.3 of verify-roadmap-redesign plan.

Two-layer architecture:
  1. **Dispatcher** (build_fix_plan): pure ClassifiedFinding -> FixPlan.
     No I/O. Translates SafeFix findings into frontmatter operations.
  2. **Applier** (apply_fixes): orchestrates backup + patcher invocation +
     audit log. Takes a `patcher` callable so tests can stub §03.4's
     apply_patch without circular dependency.

Defense-in-depth invariants (load-bearing):
  - SafetyClass.EXPOSURE_REVIEW findings are REJECTED at apply_fixes entry —
    hard error, not silent skip
  - FM_DECLARED_VS_BODY_DERIVED reaching SafeFix dispatch is REJECTED —
    classifier bug if it reaches us (see §03.1 blind spot #4)
  - parallel: true field is NEVER touched by any fix handler
  - PatchResult(applied=False) DEMOTES the SafeFix to ExposureReview
    in the result (TPR-03-003-codex unapplied-fix surface)
  - All file writes go through the patcher — auto-fix engine does NOT
    write source files directly (only audit + backup files)
"""

from __future__ import annotations

import hashlib
import json
import shutil
from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path
from typing import Callable, Iterable

from scripts.plan_corpus import (
    Finding,
    FindingCategory,
    FindingSubtype,
    SourceKind,
)

from .safety import (
    ClassifiedFinding,
    FmOperation,
    FmOperationKind,
    PAIRING_TAG_PLAN_TO_NAME_RENAME,
    PatchResult,
    PreimageRecord,
    SafetyClass,
)


# ---------------------------------------------------------------------------
# Public types
# ---------------------------------------------------------------------------

@dataclass(frozen=True)
class FixPlan:
    """A planned set of frontmatter operations for one finding.

    Produced by build_fix_plan; consumed by apply_fixes (which forwards
    to the §03.4 patcher).
    """
    finding_id: str
    path: Path
    operations: tuple[FmOperation, ...]
    rationale: str


@dataclass
class FixApplyResult:
    """Outcome of an apply_fixes batch.

    - planned_findings: ClassifiedFindings for which a FixPlan was produced
      (in dry-run, this is the only populated bucket besides skipped)
    - applied_findings: subset that the patcher reported applied=True
    - unapplied_results: PatchResults with applied=False — these get
      surfaced in the report's "unapplied fixes" group (TPR-03-003-codex)
    - skipped_findings: SafeFix findings with no handler (build_fix_plan
      returned None) — defensive bucket, should be empty in production
    """
    planned_findings: list[ClassifiedFinding] = field(default_factory=list)
    applied_findings: list[ClassifiedFinding] = field(default_factory=list)
    unapplied_results: list[PatchResult] = field(default_factory=list)
    skipped_findings: list[ClassifiedFinding] = field(default_factory=list)


class AutoFixError(RuntimeError):
    """Raised when defense-in-depth invariants are violated.

    Examples:
      - ExposureReview finding passed to apply_fixes
      - FM_DECLARED_VS_BODY_DERIVED reaches SafeFix dispatch (classifier bug)
    """


# ---------------------------------------------------------------------------
# Per-finding dispatchers
# ---------------------------------------------------------------------------

def _dispatch_unknown_field(cf: ClassifiedFinding) -> tuple[FmOperation, ...]:
    """SCHEMA_VIOLATION/UNKNOWN_FIELD: handle plan: -> name: rename + remove."""
    desc = cf.finding.description.lower()

    if "plan" in desc:
        if cf.pairing_tag == PAIRING_TAG_PLAN_TO_NAME_RENAME:
            return (FmOperation.make(
                FmOperationKind.RENAME_KEY, old_key="plan", new_key="name",
            ),)
        return (FmOperation.make(FmOperationKind.REMOVE_KEY, key="plan"),)

    return ()


def _dispatch_missing_required_field(
    cf: ClassifiedFinding,
) -> tuple[FmOperation, ...]:
    """SCHEMA_VIOLATION/MISSING_REQUIRED_FIELD: insert safe defaults."""
    desc = cf.finding.description.lower()

    if "reviewed" in desc:
        return (FmOperation.make(
            FmOperationKind.INSERT_KEY,
            key="reviewed",
            value="false",
            after_key="status",
        ),)

    if "third_party_review" in desc:
        # Multi-line YAML value: insert as a structured block
        return (FmOperation.make(
            FmOperationKind.INSERT_KEY,
            key="third_party_review",
            value="\n  status: none\n  updated: null",
            after_key="sections",
        ),)

    return ()


def _dispatch_status_contradiction(
    cf: ClassifiedFinding,
) -> tuple[FmOperation, ...]:
    """STATUS_CONTRADICTION SafeFix: only PLAN_ACTIVE_ALL_SECTIONS_NOT_STARTED.

    FM_DECLARED_VS_BODY_DERIVED is a defense-in-depth panic — see
    classify_safety; the classifier should never produce SafeFix here.
    """
    sub = cf.finding.subtype

    if sub == FindingSubtype.FM_DECLARED_VS_BODY_DERIVED:
        # Defense-in-depth: this should be impossible (classify_safety
        # always returns ExposureReview), but if a buggy classifier or a
        # forged ClassifiedFinding gets here, fail loudly.
        raise AutoFixError(
            "Defense-in-depth: FM_DECLARED_VS_BODY_DERIVED reached SafeFix "
            "dispatch. The classifier returned an incorrect SafetyClass — "
            "this finding must always be ExposureReview (see §03.1 blind spot #4)."
        )

    if sub == FindingSubtype.PLAN_ACTIVE_ALL_SECTIONS_NOT_STARTED:
        return (FmOperation.make(
            FmOperationKind.REPLACE_VALUE,
            key="status",
            new_value="queued",
        ),)

    return ()


def _dispatch_dead_reference(cf: ClassifiedFinding) -> tuple[FmOperation, ...]:
    """DEAD_REFERENCE SafeFix: remove from depends_on list.

    Audit trail goes to fixes-applied.json, NOT inline HTML comments
    (would be re-scanned by HTML_COMMENT_CONVENTION parser — blind spot #8).
    """
    f = cf.finding

    if f.source_kind != SourceKind.EXPLICIT_DEPENDS_ON:
        # Non-depends_on dead refs are ExposureReview; reaching here is
        # unexpected but defensive — return empty
        return ()

    # Extract the dead reference value from the description.
    # Description format: "Dead depends_on entry: <value>" (best-effort).
    item_value = ""
    if ":" in f.description:
        item_value = f.description.rsplit(":", 1)[1].strip()

    return (FmOperation.make(
        FmOperationKind.REMOVE_LIST_ITEM,
        list_key="depends_on",
        item_value=item_value,
    ),)


# ---------------------------------------------------------------------------
# Top-level dispatcher
# ---------------------------------------------------------------------------

_DISPATCHERS: dict[
    tuple[FindingCategory, FindingSubtype],
    Callable[[ClassifiedFinding], tuple[FmOperation, ...]],
] = {
    # SCHEMA_VIOLATION
    (FindingCategory.SCHEMA_VIOLATION, FindingSubtype.UNKNOWN_FIELD):
        _dispatch_unknown_field,
    (FindingCategory.SCHEMA_VIOLATION, FindingSubtype.MISSING_REQUIRED_FIELD):
        _dispatch_missing_required_field,
    # STATUS_CONTRADICTION (PLAN_ACTIVE_... + FM_... defense-in-depth panic)
    (FindingCategory.STATUS_CONTRADICTION,
     FindingSubtype.PLAN_ACTIVE_ALL_SECTIONS_NOT_STARTED):
        _dispatch_status_contradiction,
    (FindingCategory.STATUS_CONTRADICTION,
     FindingSubtype.FM_DECLARED_VS_BODY_DERIVED):
        _dispatch_status_contradiction,
    # DEAD_REFERENCE
    (FindingCategory.DEAD_REFERENCE, FindingSubtype.PLAN_DIRECTORY_NOT_FOUND):
        _dispatch_dead_reference,
    (FindingCategory.DEAD_REFERENCE, FindingSubtype.SECTION_FILE_NOT_FOUND):
        _dispatch_dead_reference,
    (FindingCategory.DEAD_REFERENCE, FindingSubtype.CROSS_PLAN_NAME_NOT_FOUND):
        _dispatch_dead_reference,
}


def build_fix_plan(cf: ClassifiedFinding) -> FixPlan | None:
    """Translate a SafeFix ClassifiedFinding into a FixPlan.

    Returns None when:
      - The finding is ExposureReview (filtered out)
      - The (category, subtype) has no SafeFix handler

    Raises AutoFixError when:
      - FM_DECLARED_VS_BODY_DERIVED reaches SafeFix dispatch
        (classifier bug — defense-in-depth invariant)
    """
    if cf.safety_class != SafetyClass.SAFE_FIX:
        return None

    key = (cf.finding.category, cf.finding.subtype)
    dispatcher = _DISPATCHERS.get(key)
    if dispatcher is None:
        return None

    operations = dispatcher(cf)
    if not operations:
        return None

    return FixPlan(
        finding_id=cf.finding.id,
        path=cf.finding.source,
        operations=operations,
        rationale=cf.rationale,
    )


def build_fix_plans(
    classifieds: Iterable[ClassifiedFinding],
) -> Iterable[FixPlan]:
    """Bulk build_fix_plan; filters Nones (ExposureReview / no handler / sibling-resolved)."""
    for cf in classifieds:
        if cf.safety_class != SafetyClass.SAFE_FIX:
            continue
        # Paired-dedup guard: a finding whose sibling already carries the
        # SafeFix has `resolved_by_sibling` set to the sibling's Finding.id.
        # Applying both halves would double-write (e.g. both rename `plan:`
        # AND insert `name:` when the rename alone resolves the missing-name
        # error). Skip the dependent half.
        if cf.resolved_by_sibling is not None:
            continue
        plan = build_fix_plan(cf)
        if plan is not None:
            yield plan


# ---------------------------------------------------------------------------
# Applier
# ---------------------------------------------------------------------------

# The patcher signature — matches §03.4's apply_patch contract.
PatcherFn = Callable[
    [Path, list[FmOperation], PreimageRecord, Path], PatchResult
]


def _backup_file(source: Path, backups_dir: Path) -> Path | None:
    """Copy source file into backups_dir (mirroring its relative path).

    Returns the backup path, or None if the source doesn't exist.
    """
    if not source.exists():
        return None
    backups_dir.mkdir(parents=True, exist_ok=True)
    # Mirror the source path under backups/, with a content hash suffix
    # so multiple backups of the same file don't overwrite each other.
    digest = hashlib.sha256(source.read_bytes()).hexdigest()[:8]
    safe_name = source.name + f".{digest}.bak"
    backup_path = backups_dir / safe_name
    shutil.copy2(source, backup_path)
    return backup_path


def apply_fixes(
    classifieds: list[ClassifiedFinding],
    *,
    patcher: PatcherFn,
    preimages: dict[Path, PreimageRecord],
    output_dir: Path,
    corpus_root: Path,
    dry_run: bool = False,
) -> FixApplyResult:
    """Apply auto-fixes for SafeFix ClassifiedFindings.

    Args:
        classifieds: ClassifiedFinding records — MUST all be SafeFix.
            ExposureReview findings raise AutoFixError (defense-in-depth).
        patcher: callable matching §03.4's apply_patch contract.
        preimages: PreimageRecord per source file (concurrent-session guard).
        output_dir: where backups/ + fixes-applied.json land.
        corpus_root: the reviewed plans-directory root; forwarded to the
            patcher for path-escape refusal. Paths outside this root are
            refused at the patcher boundary, never written.
        dry_run: if True, plans are computed and reported but not applied.

    Returns FixApplyResult with planned/applied/unapplied/skipped buckets.
    """
    # Defense-in-depth: reject non-SafeFix at the entry boundary
    for cf in classifieds:
        if cf.safety_class != SafetyClass.SAFE_FIX:
            raise AutoFixError(
                f"apply_fixes received an ExposureReview finding "
                f"({cf.finding.id}: {cf.finding.subtype.value}) — "
                "ExposureReview findings must NEVER reach the auto-fix path. "
                "This is a hard invariant; classifier or caller bug."
            )

    result = FixApplyResult()
    audit_entries: list[dict] = []
    backups_dir = output_dir / "backups"

    for cf in classifieds:
        try:
            plan = build_fix_plan(cf)
        except AutoFixError:
            # Re-raise — defense-in-depth panics don't get demoted
            raise

        if plan is None:
            result.skipped_findings.append(cf)
            continue

        result.planned_findings.append(cf)

        if dry_run:
            # Dry run: plan only, no file mutation, no audit log entry
            continue

        # Backup BEFORE invoking patcher
        backup_path = _backup_file(plan.path, backups_dir)

        # Look up preimage (None if caller didn't provide one)
        preimage = preimages.get(plan.path)
        if preimage is None:
            # Defensive: synthesize a "missing preimage" marker that the
            # patcher will reject — better than skipping silently
            preimage = PreimageRecord(
                path=plan.path,
                content_hash="<missing>",
                scan_timestamp=0.0,
            )

        patch_result = patcher(
            plan.path, list(plan.operations), preimage, corpus_root
        )

        if patch_result.applied:
            result.applied_findings.append(cf)
        else:
            # Concurrent-modification: surface as unapplied, do NOT silently drop
            result.unapplied_results.append(patch_result)

        audit_entries.append({
            "finding_id": plan.finding_id,
            "path": str(plan.path),
            "operations": [
                {"kind": op.kind.value, "kwargs": op.kwargs_dict()}
                for op in plan.operations
            ],
            "rationale": plan.rationale,
            "applied": patch_result.applied,
            "reason": patch_result.reason,
            "before_hash": patch_result.before_hash,
            "after_hash": patch_result.after_hash,
            "backup_path": str(backup_path) if backup_path else None,
            "timestamp": datetime.now(timezone.utc).isoformat(),
        })

    # Write audit log (only when we actually attempted fixes)
    if not dry_run and audit_entries:
        output_dir.mkdir(parents=True, exist_ok=True)
        audit_file = output_dir / "fixes-applied.json"
        audit_file.write_text(
            json.dumps({"fixes": audit_entries}, indent=2) + "\n",
            encoding="utf-8",
        )

    return result
