#!/usr/bin/env python3
"""merge-findings.py — merge envelope files into a reviewer-tagged finding list.

Usage:
    merge-findings.py --codex CODEX_ENVELOPE --gemini GEMINI_ENVELOPE \
                      --section SECTION_NUMBER \
                      [--out MERGED_FILE]

Either --codex or --gemini may point to a non-existent file (e.g. when
the circuit breaker skipped a reviewer). The missing reviewer is treated
as having zero findings and the output carries "reviewer_mode": "single".

Output: JSON to stdout (or --out file) with shape:
  {
    "section": "02",
    "reviewer_mode": "dual",           // "dual" or "single"
    "active_reviewers": ["codex", "gemini"],  // which reviewers ran
    "tripped_reviewer": null,           // name of skipped reviewer, or null
    "merged_findings": [ ... ],
    "summary": {
      "codex_findings": 5,
      "gemini_findings": 3,
      "agreements": 2,
      "codex_only": 3,
      "gemini_only": 1,
      "informational": 1,
      "actionable": 7,
      "max_severity": "high"           // highest severity across all findings
    }
  }
"""

import argparse
import json
import os
import sys


def make_id(section, ordinal, reviewer):
    return f"[TPR-{section}-{ordinal:03d}-{reviewer}]"


SEVERITY_ORDER = {"critical": 4, "high": 3, "major": 2, "medium": 2, "minor": 1, "low": 1, "informational": 0}


def load_envelope(path):
    """Load an envelope file, returning empty findings if missing or invalid."""
    if not path or not os.path.isfile(path):
        return {"findings": [], "no_findings": True, "_skipped": True}
    try:
        with open(path) as f:
            env = json.load(f)
        env["_skipped"] = False
        return env
    except (json.JSONDecodeError, OSError):
        return {"findings": [], "no_findings": True, "_skipped": True}


def max_severity(findings):
    """Return the highest severity string across a list of finding entries."""
    best = "informational"
    best_rank = 0
    for entry in findings:
        sev = entry.get("finding", {}).get("severity", "informational")
        rank = SEVERITY_ORDER.get(sev, 0)
        if rank > best_rank:
            best_rank = rank
            best = sev
    return best


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--codex", required=True)
    ap.add_argument("--gemini", required=True)
    ap.add_argument("--section", required=True, help="Section number, e.g. '02'")
    ap.add_argument("--out", default=None)
    args = ap.parse_args()

    codex_env = load_envelope(args.codex)
    gemini_env = load_envelope(args.gemini)

    codex_skipped = codex_env.pop("_skipped", False)
    gemini_skipped = gemini_env.pop("_skipped", False)

    # Determine reviewer mode
    active_reviewers = []
    tripped_reviewer = None
    if not codex_skipped:
        active_reviewers.append("codex")
    else:
        tripped_reviewer = "codex"
    if not gemini_skipped:
        active_reviewers.append("gemini")
    else:
        tripped_reviewer = "gemini"
    reviewer_mode = "dual" if len(active_reviewers) == 2 else "single"

    if not active_reviewers:
        # Both skipped — should not happen (circuit-breaker blocks this upstream)
        result = {
            "section": args.section,
            "reviewer_mode": "none",
            "active_reviewers": [],
            "tripped_reviewer": "both",
            "merged_findings": [],
            "summary": {
                "codex_findings": 0, "gemini_findings": 0,
                "agreements": 0, "codex_only": 0, "gemini_only": 0,
                "informational": 0, "actionable": 0, "max_severity": "informational",
            }
        }
        out = json.dumps(result, indent=2)
        if args.out:
            with open(args.out, "w") as f:
                f.write(out + "\n")
        else:
            sys.stdout.write(out + "\n")
        sys.exit(0)

    # Normalize gemini findings: gemini has been observed to emit "description"
    # instead of "title" (tracked on 2026-04-14 during BUG-04-059 Phase 5 code
    # TPR). Alias "description" → "title" so cross-reviewer matching by
    # (location, title) still works. Also default "title" to empty string for
    # findings that lack both — don't crash the merge on schema drift.
    for f in gemini_env.get("findings", []):
        if "title" not in f:
            f["title"] = f.get("description", "")
    for f in codex_env.get("findings", []):
        if "title" not in f:
            f["title"] = f.get("description", "")

    # Build (location, title) → finding maps for cross-reviewer lookup
    gemini_by_loctitle = {}
    for f in gemini_env.get("findings", []):
        key = (f.get("location", ""), f["title"])
        gemini_by_loctitle[key] = f

    codex_by_loctitle = {}
    for f in codex_env.get("findings", []):
        key = (f.get("location", ""), f["title"])
        codex_by_loctitle[key] = f

    merged = []
    agreements = 0
    codex_only = 0
    gemini_only = 0
    informational = 0

    # First pass: codex findings (in order)
    for i, finding in enumerate(codex_env.get("findings", []), start=1):
        codex_id = make_id(args.section, i, "codex")
        key = (finding["location"], finding["title"])
        if key in gemini_by_loctitle:
            # Find gemini's ordinal for this finding
            gemini_findings = gemini_env.get("findings", [])
            gemini_ordinal = next(
                (j for j, gf in enumerate(gemini_findings, start=1)
                 if (gf["location"], gf["title"]) == key),
                None
            )
            partner_id = make_id(args.section, gemini_ordinal, "gemini") if gemini_ordinal else None
            merged.append({
                "id": codex_id,
                "reviewer": "codex",
                "agreement": True,
                "agreement_partner_id": partner_id,
                "finding": finding,
            })
            agreements += 1
        else:
            merged.append({
                "id": codex_id,
                "reviewer": "codex",
                "agreement": False,
                "agreement_partner_id": None,
                "finding": finding,
            })
            codex_only += 1

    # Second pass: gemini findings (in order). Add gemini-only AND the gemini half of agreements.
    for i, finding in enumerate(gemini_env.get("findings", []), start=1):
        gemini_id = make_id(args.section, i, "gemini")
        key = (finding["location"], finding["title"])
        if key in codex_by_loctitle:
            # This is the gemini half of an agreement. Find codex's ordinal.
            codex_findings = codex_env.get("findings", [])
            codex_ordinal = next(
                (j for j, cf in enumerate(codex_findings, start=1)
                 if (cf["location"], cf["title"]) == key),
                None
            )
            partner_id = make_id(args.section, codex_ordinal, "codex") if codex_ordinal else None
            merged.append({
                "id": gemini_id,
                "reviewer": "gemini",
                "agreement": True,
                "agreement_partner_id": partner_id,
                "finding": finding,
            })
        else:
            merged.append({
                "id": gemini_id,
                "reviewer": "gemini",
                "agreement": False,
                "agreement_partner_id": None,
                "finding": finding,
            })
            gemini_only += 1

    # Count informational findings (non-actionable observations).
    # Count unique informational findings: for agreements, count once (not twice).
    seen_informational = set()
    for entry in merged:
        sev = entry["finding"].get("severity", "")
        if sev == "informational":
            key = (entry["finding"]["location"], entry["finding"]["title"])
            if key not in seen_informational:
                seen_informational.add(key)
                informational += 1

    codex_total = len(codex_env.get("findings", []))
    gemini_total = len(gemini_env.get("findings", []))
    # Actionable = unique findings minus informational.
    # Unique count: agreements (counted once) + codex_only + gemini_only.
    unique_total = agreements + codex_only + gemini_only
    actionable = unique_total - informational

    result = {
        "section": args.section,
        "reviewer_mode": reviewer_mode,
        "active_reviewers": active_reviewers,
        "tripped_reviewer": tripped_reviewer,
        "merged_findings": merged,
        "summary": {
            "codex_findings": codex_total,
            "gemini_findings": gemini_total,
            "agreements": agreements,
            "codex_only": codex_only,
            "gemini_only": gemini_only,
            "informational": informational,
            "actionable": actionable,
            "max_severity": max_severity(merged),
        }
    }

    out = json.dumps(result, indent=2)
    if args.out:
        with open(args.out, "w") as f:
            f.write(out + "\n")
    else:
        sys.stdout.write(out + "\n")
    sys.exit(0)


if __name__ == "__main__":
    main()
