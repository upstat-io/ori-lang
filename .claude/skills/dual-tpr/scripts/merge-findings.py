#!/usr/bin/env python3
"""merge-findings.py — merge two envelope files into a reviewer-tagged finding list.

Usage:
    merge-findings.py --codex CODEX_ENVELOPE --gemini GEMINI_ENVELOPE \
                      --section SECTION_NUMBER \
                      [--out MERGED_FILE]

Reads both envelope files, produces a merged finding list with:
  - Reviewer-tagged IDs: [TPR-SECTION-ORDINAL-codex|gemini]
  - Independent ordinal sequences per reviewer
  - Strict (location, title) agreement detection (annotation only)

Output: JSON to stdout (or --out file) with shape:
  {
    "section": "02",
    "merged_findings": [
      {
        "id": "[TPR-02-001-codex]",
        "reviewer": "codex",
        "agreement": true,  # or false
        "agreement_partner_id": "[TPR-02-001-gemini]",  # null if agreement=false
        "finding": { ...the original finding object from the codex envelope... }
      },
      ...
    ],
    "summary": {
      "codex_findings": 5,
      "gemini_findings": 3,
      "agreements": 2,
      "codex_only": 3,
      "gemini_only": 1
    }
  }
"""

import argparse
import json
import sys


def make_id(section, ordinal, reviewer):
    return f"[TPR-{section}-{ordinal:03d}-{reviewer}]"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--codex", required=True)
    ap.add_argument("--gemini", required=True)
    ap.add_argument("--section", required=True, help="Section number, e.g. '02'")
    ap.add_argument("--out", default=None)
    args = ap.parse_args()

    with open(args.codex) as f:
        codex_env = json.load(f)
    with open(args.gemini) as f:
        gemini_env = json.load(f)

    # Build (location, title) → finding maps for cross-reviewer lookup
    gemini_by_loctitle = {}
    for f in gemini_env.get("findings", []):
        key = (f["location"], f["title"])
        gemini_by_loctitle[key] = f

    codex_by_loctitle = {}
    for f in codex_env.get("findings", []):
        key = (f["location"], f["title"])
        codex_by_loctitle[key] = f

    merged = []
    agreements = 0
    codex_only = 0
    gemini_only = 0

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

    result = {
        "section": args.section,
        "merged_findings": merged,
        "summary": {
            "codex_findings": len(codex_env.get("findings", [])),
            "gemini_findings": len(gemini_env.get("findings", [])),
            "agreements": agreements,
            "codex_only": codex_only,
            "gemini_only": gemini_only,
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
