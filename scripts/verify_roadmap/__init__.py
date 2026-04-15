"""Verify-roadmap skill: write-back policy layer on top of plan_corpus.

This package owns:
  - Safety taxonomy (SafetyClass, ClassifiedFinding) — Section 03.1
  - Report format (JSON, markdown, console) — Section 03.2
  - Auto-fix engine — Section 03.3
  - Frontmatter text patcher — Section 03.4
  - Continue-roadmap integration — Section 03.5

plan_corpus produces factual Finding records (no policy).
This package classifies and acts on those findings.
plan_corpus MUST NEVER import from this package.
"""

from __future__ import annotations

from .safety import (
    SafetyClass,
    ClassifiedFinding,
    WriteBackContext,
    PreimageRecord,
    classify_safety,
)

__all__ = [
    "SafetyClass",
    "ClassifiedFinding",
    "WriteBackContext",
    "PreimageRecord",
    "classify_safety",
]
