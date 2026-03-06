#!/usr/bin/env python3
"""ARC metrics — Section 03.

Counts RC operations per function, detects violations (unbalanced pairs,
scalar RC, wasted pairs), and produces the weighted violation count for score.py.
"""

from __future__ import annotations

import re
from dataclasses import dataclass

from ir_parser import Function, Module

# RC operation patterns (verified against ori_llvm runtime_functions.rs)
_RC_INC_RE = re.compile(r'call.*@ori_rc_inc|call.*@ori_list_rc_inc')
_RC_DEC_RE = re.compile(
    r'call.*@ori_rc_dec'
    r'|call.*@ori_rc_free'
    r'|call.*@ori_buffer_rc_dec'
    r'|call.*@ori_buffer_drop_unique'
    r'|call.*@ori_map_buffer_drop_unique'
)
# Also check for invoke (should not appear for RC functions, but detect if it does)
_RC_INVOKE_RE = re.compile(r'invoke.*@ori_rc_inc|invoke.*@ori_rc_dec')


# ---------------------------------------------------------------------------
# Data Model
# ---------------------------------------------------------------------------

@dataclass
class FunctionArcMetrics:
    name: str
    rc_inc: int
    rc_dec: int
    balanced: bool
    has_scalar_rc: bool
    wasted_pairs: int


@dataclass
class ArcMetrics:
    per_function: list[FunctionArcMetrics]
    total_violations: int
    has_unbalanced: bool
    has_scalar_rc: bool


# ---------------------------------------------------------------------------
# Detection
# ---------------------------------------------------------------------------

def _count_rc_ops(func: Function) -> tuple[int, int]:
    """Count RC inc and dec operations in a function."""
    inc = 0
    dec = 0
    for block in func.blocks:
        for instr in block.instructions:
            if _RC_INC_RE.search(instr.text):
                inc += 1
            if _RC_DEC_RE.search(instr.text):
                dec += 1
    return inc, dec


def _detect_scalar_rc(func: Function) -> bool:
    """Heuristic: RC calls in a function with only scalar params and no alloca.

    If a function takes only i64/double/i1 params and has no alloca of
    pointer-containing structs, any RC call is likely operating on scalars.
    """
    has_rc = any(
        _RC_INC_RE.search(instr.text) or _RC_DEC_RE.search(instr.text)
        for block in func.blocks
        for instr in block.instructions
    )
    if not has_rc:
        return False

    scalar_types = {'i64', 'double', 'i1', 'i32', 'float'}
    all_params_scalar = all(pt in scalar_types for pt in func.param_types)
    has_alloca = any(
        instr.opcode == 'alloca'
        for block in func.blocks
        for instr in block.instructions
    )

    return all_params_scalar and not has_alloca


def _detect_wasted_pairs(func: Function) -> int:
    """Count consecutive inc+dec on the same value in the same block."""
    count = 0
    for block in func.blocks:
        instrs = block.instructions
        for i in range(len(instrs) - 1):
            curr = instrs[i]
            nxt = instrs[i + 1]
            if (_RC_INC_RE.search(curr.text) and _RC_DEC_RE.search(nxt.text)):
                # Check if operating on the same pointer (first arg after @func_name)
                inc_args = _extract_first_arg(curr.text)
                dec_args = _extract_first_arg(nxt.text)
                if inc_args and inc_args == dec_args:
                    count += 1
    return count


def _extract_first_arg(call_text: str) -> str | None:
    """Extract the first argument from a call instruction."""
    paren = call_text.find('(')
    if paren < 0:
        return None
    after = call_text[paren + 1:]
    # First arg up to comma or closing paren
    end = min(
        (after.find(',') if ',' in after else len(after)),
        (after.find(')') if ')' in after else len(after)),
    )
    arg = after[:end].strip()
    # Strip type prefix (e.g., "ptr %x" → "%x")
    parts = arg.split()
    return parts[-1] if parts else None


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def compute_arc_metrics(module: Module) -> ArcMetrics:
    """Compute ARC metrics for all user functions."""
    results: list[FunctionArcMetrics] = []
    total_violations = 0
    any_unbalanced = False
    any_scalar = False

    for func in module.user_functions():
        inc, dec = _count_rc_ops(func)
        balanced = inc == dec
        scalar = _detect_scalar_rc(func)
        wasted = _detect_wasted_pairs(func)

        if not balanced:
            any_unbalanced = True
        if scalar:
            any_scalar = True

        # Violation weights: unbalanced=3, scalar=5, wasted=1
        violations = 0
        if not balanced:
            violations += abs(inc - dec) * 3
        if scalar:
            violations += 5
        violations += wasted * 1

        total_violations += violations

        results.append(FunctionArcMetrics(
            name=func.raw_name,
            rc_inc=inc,
            rc_dec=dec,
            balanced=balanced,
            has_scalar_rc=scalar,
            wasted_pairs=wasted,
        ))

    return ArcMetrics(
        per_function=results,
        total_violations=total_violations,
        has_unbalanced=any_unbalanced,
        has_scalar_rc=any_scalar,
    )
