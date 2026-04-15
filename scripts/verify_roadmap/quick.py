"""Quick-mode runner for verify-roadmap.

Section 03.5 of verify-roadmap-redesign plan.

`--quick` mode is the lightweight pre-check called by `/continue-roadmap`'s
roadmap-scan.sh BEFORE selecting the next section to work on. It runs ONLY
the read-only DAG checks that don't require git activity signals or
expensive shared-subsystem analysis:

  - BLOCKED   (classify_blocked)
  - DEAD_REFERENCE (classify_dead_reference)

Explicitly NOT included in --quick (per §03.5 blind spot #9):
  - CONFLICT  — requires O(N^2) shared-subsystem analysis
  - STATUS_CONTRADICTION  — requires body scanning
  - SUPERSEDED  — requires reroute resolution + git
  - MISSING_DEPENDENCY  — requires full prose scan

`--quick` mode passes `context=None` to classify_safety, which returns
ExposureReview for ALL findings (per §03.1). This is intentional —
`--quick` is a pre-check surface, never a write-back trigger. Auto-fix
only runs in `--full` mode.
"""

from __future__ import annotations

from pathlib import Path

from scripts.plan_corpus import discover_corpus
from scripts.plan_corpus.dag import (
    build_dag,
    classify_blocked,
    classify_dead_reference,
)

from .report import Report, ReportMode
from .safety import (
    ClassifiedFinding,
    classify_safety,
)


def run_quick(plans_root: Path | None = None) -> Report:
    """Run --quick mode and return a Report.

    Steps:
      1. Discover the plan corpus (parses frontmatter into corpus buckets;
         emits PARSE_ERROR + GAP findings via corpus.gaps)
      2. Build DAG from corpus
      3. Run classify_blocked + classify_dead_reference
      4. classify_safety with context=None -> all ExposureReview
      5. Wrap in a QUICK Report (report.py renderers will omit safety_class)

    Returns:
      Report(mode=QUICK, findings=..., unapplied_fixes=())
    """
    # Step 1: discover corpus (parses frontmatter into buckets;
    # PARSE_ERROR + GAP findings land in corpus.gaps).
    corpus = discover_corpus(plans_root)

    # Step 2: build DAG directly from corpus (no need to re-validate;
    # discover_corpus already parsed every file)
    dag = build_dag(corpus)

    # Step 3: run only the read-only DAG classifiers (BLOCKED + DEAD_REFERENCE)
    # plus surface any parse/discovery findings already in corpus.gaps.
    findings: list = list(corpus.gaps)
    findings.extend(classify_blocked(dag, corpus))
    findings.extend(classify_dead_reference(dag, corpus))

    # Step 4: classify_safety with context=None (all -> ExposureReview)
    classifieds: list[ClassifiedFinding] = [
        classify_safety(f, context=None)
        for f in findings
    ]

    return Report(
        mode=ReportMode.QUICK,
        findings=tuple(classifieds),
        unapplied_fixes=(),
    )
