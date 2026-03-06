#!/usr/bin/env python3
"""Binary metrics — Section 06.

Computes binary quality metrics from exit codes and optional section sizes.
Does NOT invoke external commands — parses pre-collected output.
"""

from __future__ import annotations

import re
from dataclasses import dataclass
from pathlib import Path


# ---------------------------------------------------------------------------
# Data Model
# ---------------------------------------------------------------------------

@dataclass
class BinaryMetrics:
    eval_exit: int | None
    aot_exit: int | None
    expected: int
    eval_correct: bool
    aot_correct: bool
    paths_match: bool
    hard_fail: bool
    defects: int
    binary_size: int | None = None
    text_size: int | None = None
    rodata_size: int | None = None


# ---------------------------------------------------------------------------
# Section Size Parsing
# ---------------------------------------------------------------------------

_SECTION_RE = re.compile(r'^(\.\S+)\s+(\d+)')


def parse_section_sizes(text: str) -> dict[str, int]:
    """Parse `size -A binary` output into {section_name: size_bytes}."""
    sections: dict[str, int] = {}
    for line in text.splitlines():
        m = _SECTION_RE.match(line.strip())
        if m:
            sections[m.group(1)] = int(m.group(2))
    return sections


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def compute_binary_metrics(
    eval_exit: int | None,
    aot_exit: int | None,
    expected: int,
    sections_text: str | None = None,
) -> BinaryMetrics:
    """Compute binary quality metrics from exit codes and optional section sizes.

    Args:
        eval_exit: Interpreter exit code (None if not run).
        aot_exit: AOT binary exit code (None if compilation failed).
        expected: Expected exit code.
        sections_text: Output of `size -A binary` (optional).
    """
    eval_correct = eval_exit == expected if eval_exit is not None else False
    aot_correct = aot_exit == expected if aot_exit is not None else False
    paths_match = eval_exit == aot_exit if (eval_exit is not None and aot_exit is not None) else False

    defects = 0
    hard_fail = False

    # Wrong eval output
    if eval_exit is not None and not eval_correct:
        defects += 1
        hard_fail = True

    # Wrong AOT output
    if aot_exit is not None and not aot_correct:
        defects += 1
        hard_fail = True

    # Eval/AOT mismatch
    if (eval_exit is not None and aot_exit is not None and not paths_match):
        defects += 3
        hard_fail = True

    # Crash detection (exit code > 128 = signal)
    if eval_exit is not None and eval_exit > 128:
        defects += 3
        hard_fail = True
    if aot_exit is not None and aot_exit > 128:
        defects += 3
        hard_fail = True

    # Compilation failure
    if aot_exit is None:
        defects += 3
        hard_fail = True

    # Section sizes (informational)
    binary_size = None
    text_size = None
    rodata_size = None

    if sections_text:
        sections = parse_section_sizes(sections_text)
        text_size = sections.get(".text")
        rodata_size = sections.get(".rodata")
        if text_size is not None and rodata_size is not None:
            binary_size = text_size + rodata_size

    return BinaryMetrics(
        eval_exit=eval_exit,
        aot_exit=aot_exit,
        expected=expected,
        eval_correct=eval_correct,
        aot_correct=aot_correct,
        paths_match=paths_match,
        hard_fail=hard_fail,
        defects=defects,
        binary_size=binary_size,
        text_size=text_size,
        rodata_size=rodata_size,
    )
