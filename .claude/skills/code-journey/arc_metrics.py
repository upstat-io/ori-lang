#!/usr/bin/env python3
"""ARC metrics -- Section 03.

Counts RC operations per function, detects violations (unbalanced pairs,
scalar RC, wasted pairs), and produces the weighted violation count for score.py.

Uses effect_summaries to account for implicit allocations by runtime functions
(e.g., ori_str_from_raw returns +1 RC internally, invisible to naive regex).

Module-level balance check (Section 03) detects ownership transfers: when the
module as a whole is balanced, per-function imbalances are likely ownership
transfers (e.g., closure env allocated in one function, freed in another).
"""

from __future__ import annotations

import re
from dataclasses import dataclass, field

from effect_summaries import RcEffect, get_effect, is_allocation_function
from ir_parser import BasicBlock, Function, Module

# RC operation patterns (verified against ori_llvm runtime_functions.rs)
# Match both call and invoke instructions.
_RC_INC_RE = re.compile(
    r'(?:call|invoke)\b.*@"?ori_rc_inc"?\b'
    r'|(?:call|invoke)\b.*@"?ori_list_rc_inc"?\b'
)
_RC_DEC_RE = re.compile(
    r'(?:call|invoke)\b.*@"?ori_rc_dec"?\b'
    r'|(?:call|invoke)\b.*@"?ori_buffer_rc_dec"?\b'
    r'|(?:call|invoke)\b.*@"?ori_buffer_drop_unique"?\b'
    r'|(?:call|invoke)\b.*@"?ori_map_buffer_rc_dec"?\b'
    r'|(?:call|invoke)\b.*@"?ori_map_buffer_drop_unique"?\b'
    r'|(?:call|invoke)\b.*@"?ori_set_buffer_rc_dec"?\b'
    r'|(?:call|invoke)\b.*@"?ori_set_buffer_drop_unique"?\b'
    r'|(?:call|invoke)\b.*@"?ori_iter_drop"?\b'
)
# NOTE: ori_rc_free is NOT an RC operation — it is raw memory deallocation
# called from within drop functions (which are themselves called by ori_rc_dec
# when refcount reaches zero). Counting both ori_rc_dec and ori_rc_free
# double-counts the same release operation.

# Extract callee function name from call/invoke instructions.
# Handles both bare @name( and quoted @"name"( forms.
_CALLEE_RE = re.compile(r'(?:call|invoke)\b[^@]*@(?:"([^"]+)"|([\w.$]+))\s*\(')

# Closure-related function name patterns (both $ and __ prefixed forms)
_CLOSURE_NAME_PARTS = ("$lambda", "$closure", "$partial", "_env",
                       "__lambda", "__closure", "_partial_")

# Collection drop functions that clean up elements internally (opaque to
# static call-site counting). A function calling these has element cleanup
# that is invisible to the per-function RC balance analysis.
_COLLECTION_DROP_FUNCTIONS = frozenset({
    "ori_buffer_drop_unique",
    "ori_buffer_rc_dec",
    "ori_map_buffer_drop_unique",
    "ori_map_buffer_rc_dec",
    "ori_set_buffer_drop_unique",
    "ori_set_buffer_rc_dec",
})


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
    has_collection_drop: bool = False
    is_ownership_transfer: bool = False


@dataclass
class ArcMetrics:
    per_function: list[FunctionArcMetrics]
    total_violations: int
    has_unbalanced: bool
    has_scalar_rc: bool
    module_balanced: bool = False
    ownership_transfers: int = 0


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def _extract_callee(text: str) -> str | None:
    """Extract the callee function name from a call or invoke instruction."""
    m = _CALLEE_RE.search(text)
    if not m:
        return None
    return m.group(1) or m.group(2)


def _is_closure_related(func_name: str) -> bool:
    """Check if a function name suggests closure/lambda involvement."""
    return any(part in func_name for part in _CLOSURE_NAME_PARTS)


# ---------------------------------------------------------------------------
# Detection
# ---------------------------------------------------------------------------

def _is_landingpad_block(block: BasicBlock) -> bool:
    """Check if a block is a landingpad (exception cleanup) block.

    Landingpad blocks are alternative execution paths that run during
    stack unwinding instead of the normal continuation. Their RC
    operations must not be counted alongside normal-path operations
    because they are mutually exclusive paths.
    """
    return (len(block.instructions) > 0
            and block.instructions[0].opcode == "landingpad")


def _block_terminates_with(block: BasicBlock, opcode: str) -> bool:
    """Check if a block's last instruction has the given opcode."""
    if not block.instructions:
        return False
    return block.instructions[-1].opcode == opcode


def _extract_branch_targets(block: BasicBlock) -> list[str]:
    """Extract branch target labels from the last instruction of a block."""
    if not block.instructions:
        return []
    text = block.instructions[-1].text
    # Match: br label %target, br i1 %cond, label %t, label %f
    return re.findall(r'label\s+%([^\s,)]+)', text)


def _find_unwind_blocks(func: Function) -> set[str]:
    """Find all blocks in unwind cleanup regions.

    A block is in the unwind region if:
    - It starts with `landingpad` (exception entry), OR
    - It terminates with `resume` (exception exit), OR
    - It is an intermediate block between landingpad and resume
      (reachable only from unwind blocks, all successors are unwind blocks)

    This handles the common pattern:
        bb2 (landingpad) → rc_dec.heap → rc_dec.sso_skip (resume)
    where all three blocks should be excluded from RC counting.
    """
    unwind_labels: set[str] = set()
    # First pass: find landingpad blocks and resume blocks
    for block in func.blocks:
        if _is_landingpad_block(block) or _block_terminates_with(block, "resume"):
            unwind_labels.add(block.label)

    # Second pass: find blocks whose ALL branch targets are unwind blocks.
    # These are intermediate blocks in the unwind cleanup chain.
    # Iterate until no more blocks are added (handles chains of any length).
    changed = True
    while changed:
        changed = False
        for block in func.blocks:
            if block.label in unwind_labels:
                continue
            targets = _extract_branch_targets(block)
            if targets and all(t in unwind_labels for t in targets):
                unwind_labels.add(block.label)
                changed = True

    return unwind_labels


def _count_rc_ops(func: Function) -> tuple[int, int]:
    """Count RC inc and dec operations, including implicit allocations.

    Uses two layers:
    1. Effect summaries for known runtime functions (most accurate)
    2. Regex fallback for unknown functions (backward compatible)

    Excludes all unwind cleanup blocks (landingpad, resume, and
    intermediate blocks reachable only from landingpad paths), which are
    alternative execution paths mutually exclusive with normal continuation.
    """
    unwind_blocks = _find_unwind_blocks(func)
    inc = 0
    dec = 0
    for block in func.blocks:
        if block.label in unwind_blocks:
            continue
        for instr in block.instructions:
            callee = _extract_callee(instr.text)
            if callee:
                effect = get_effect(callee)
                if effect:
                    # Use effect summary (accurate, handles opaque allocations)
                    if effect.return_effect == RcEffect.PLUS_ONE:
                        inc += 1
                    for pe in effect.param_effects:
                        if pe == RcEffect.PLUS_ONE:
                            inc += 1
                        elif pe == RcEffect.MINUS_ONE:
                            dec += 1
                    continue  # Effect summary handled this call

            # Fallback: regex matching for calls without effect summaries
            if _RC_INC_RE.search(instr.text):
                inc += 1
            if _RC_DEC_RE.search(instr.text):
                dec += 1
    return inc, dec


def _detect_scalar_rc(func: Function) -> bool:
    """Heuristic: RC calls in a function with only scalar params and no alloca.

    If a function takes only i64/double/i1 params and has no alloca of
    pointer-containing structs, any RC call is likely operating on scalars.

    Zero-param functions are excluded: they commonly work with heap objects
    via return values from callees (e.g., @main calling make_adder).
    """
    if not func.param_types:
        return False

    has_rc = any(
        _RC_INC_RE.search(instr.text) or _RC_DEC_RE.search(instr.text)
        for block in func.blocks if not _is_landingpad_block(block)
        for instr in block.instructions
    )
    if not has_rc:
        return False

    scalar_types = {'i64', 'double', 'i1', 'i32', 'float'}
    all_params_scalar = all(pt in scalar_types for pt in func.param_types)
    has_alloca = any(
        instr.opcode == 'alloca'
        for block in func.blocks if not _is_landingpad_block(block)
        for instr in block.instructions
    )

    return all_params_scalar and not has_alloca


def _detect_wasted_pairs(func: Function) -> int:
    """Count consecutive inc+dec on the same value in the same block."""
    count = 0
    for block in func.blocks:
        if _is_landingpad_block(block):
            continue
        instrs = block.instructions
        for i in range(len(instrs) - 1):
            curr = instrs[i]
            nxt = instrs[i + 1]
            if (_RC_INC_RE.search(curr.text) and _RC_DEC_RE.search(nxt.text)):
                inc_args = _extract_first_arg(curr.text)
                dec_args = _extract_first_arg(nxt.text)
                if inc_args and inc_args == dec_args:
                    count += 1
    return count


def _has_collection_drop(func: Function) -> bool:
    """Check if a function calls collection drop/cleanup functions.

    Functions that call buffer_drop_unique, buffer_rc_dec, etc. handle
    element cleanup internally (invoking elem_dec_fn N times for N elements).
    This cleanup is opaque to static call-site counting, creating an
    apparent surplus of N element incs without matching decs.
    """
    for block in func.blocks:
        for instr in block.instructions:
            callee = _extract_callee(instr.text)
            if callee and callee in _COLLECTION_DROP_FUNCTIONS:
                return True
    return False


def _extract_first_arg(call_text: str) -> str | None:
    """Extract the first argument from a call instruction."""
    paren = call_text.find('(')
    if paren < 0:
        return None
    after = call_text[paren + 1:]
    end = min(
        (after.find(',') if ',' in after else len(after)),
        (after.find(')') if ')' in after else len(after)),
    )
    arg = after[:end].strip()
    parts = arg.split()
    return parts[-1] if parts else None


def _find_ownership_transfers(
    results: list[FunctionArcMetrics],
) -> list[tuple[str, str]]:
    """Find (producer, consumer) pairs for ownership transfer detection.

    A producer has inc > dec (creates RC objects it doesn't free).
    A consumer has dec > inc (frees RC objects it didn't create).

    Detection strategies (in priority order):
    1. Surplus/deficit pairing: producers pair with consumers when totals
       match within tolerance (±1 for non-capturing closure constants).
    2. Single-surplus producers: functions with surplus exactly +1 are
       almost always returning a heap-allocated value to their caller
       (standard ownership transfer). These are marked as transfers even
       without a matching consumer, because the caller absorbs the dec
       within its own balanced operations, and element cleanup functions
       (elem_dec) are called dynamically N times but show only 1 static
       call site.
    """
    producers: list[FunctionArcMetrics] = []
    consumers: list[FunctionArcMetrics] = []
    for fm in results:
        if not fm.balanced:
            if fm.rc_inc > fm.rc_dec:
                producers.append(fm)
            else:
                consumers.append(fm)

    if not producers and not consumers:
        return []

    pairs: list[tuple[str, str]] = []
    remaining_surplus = sum(fm.rc_inc - fm.rc_dec for fm in producers)
    remaining_deficit = sum(fm.rc_dec - fm.rc_inc for fm in consumers)

    # Strategy 1: Surplus/deficit pairing with tolerance
    if abs(remaining_surplus - remaining_deficit) <= 1:
        for p in producers:
            for c in consumers:
                pairs.append((p.name, c.name))
        return pairs

    # Strategy 2: Single-surplus producers (ownership transfer pattern)
    # A function with surplus exactly +1 returns one heap-allocated value
    # to its caller. This is the standard ownership transfer pattern —
    # the caller handles the dec. Mark these as transfers unconditionally.
    single_surplus_producers = [
        p for p in producers if (p.rc_inc - p.rc_dec) == 1
    ]
    if single_surplus_producers:
        for p in single_surplus_producers:
            if consumers:
                for c in consumers:
                    pairs.append((p.name, c.name))
            else:
                pairs.append((p.name, "<caller>"))

    # Strategy 3: Collection element cleanup (opaque element drops)
    # When a function creates N elements for a collection and drops the
    # collection via buffer_drop_unique/buffer_rc_dec, the element cleanup
    # (N decs via elem_dec_fn) happens internally and is invisible to
    # static call-site counting. The function shows surplus = N elements.
    # Mark these as ownership transfers if the function has a collection
    # drop call.
    for p in producers:
        if p.is_ownership_transfer:
            continue  # Already handled by strategy 1 or 2
        if p.has_collection_drop:
            pairs.append((p.name, "<collection_elements>"))

    return pairs


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def compute_arc_metrics(module: Module) -> ArcMetrics:
    """Compute ARC metrics for all user functions."""
    results: list[FunctionArcMetrics] = []
    any_unbalanced = False
    any_scalar = False

    for func in module.user_functions():
        inc, dec = _count_rc_ops(func)
        balanced = inc == dec
        scalar = _detect_scalar_rc(func)
        wasted = _detect_wasted_pairs(func)
        has_coll_drop = _has_collection_drop(func)

        if not balanced:
            any_unbalanced = True
        if scalar:
            any_scalar = True

        results.append(FunctionArcMetrics(
            name=func.raw_name,
            rc_inc=inc,
            rc_dec=dec,
            balanced=balanced,
            has_scalar_rc=scalar,
            wasted_pairs=wasted,
            has_collection_drop=has_coll_drop,
        ))

    # Module-level balance check
    total_inc = sum(f.rc_inc for f in results)
    total_dec = sum(f.rc_dec for f in results)
    module_balanced = total_inc == total_dec
    ownership_transfer_count = 0

    # Detect ownership transfers: always try, even if module isn't
    # perfectly balanced. The tolerance in _find_ownership_transfers
    # handles non-capturing closures (off by 1).
    if any_unbalanced:
        pairs = _find_ownership_transfers(results)
        if pairs:
            paired_names = set()
            for p, c in pairs:
                paired_names.add(p)
                paired_names.add(c)
            for fm in results:
                if not fm.balanced and fm.name in paired_names:
                    fm.is_ownership_transfer = True
                    ownership_transfer_count += 1

    # After ownership transfer detection, check for unexplained imbalances.
    # Explained transfers (producer→consumer pairs) are not leaks/double-frees.
    any_unexplained_unbalanced = any(
        not fm.balanced and not fm.is_ownership_transfer
        for fm in results
    )

    # Compute violations with ownership transfer downgrade
    total_violations = 0
    for fm in results:
        violations = 0
        if not fm.balanced:
            if fm.is_ownership_transfer:
                # Ownership transfer: no penalty — producer→consumer pairs
                # are explained cross-function ownership, not leaks
                pass
            else:
                violations += abs(fm.rc_inc - fm.rc_dec) * 3
        if fm.has_scalar_rc:
            violations += 5
        violations += fm.wasted_pairs * 1
        total_violations += violations

    return ArcMetrics(
        per_function=results,
        total_violations=total_violations,
        has_unbalanced=any_unexplained_unbalanced,
        has_scalar_rc=any_scalar,
        module_balanced=module_balanced,
        ownership_transfers=ownership_transfer_count,
    )
