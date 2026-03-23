#!/usr/bin/env python3
"""Control flow metrics — Section 05.

Counts control flow defects (empty blocks, redundant branches, trivial phi nodes)
and detects semantic incorrectness (unreachable blocks, same-target conditionals).
"""

from __future__ import annotations

from dataclasses import dataclass

from ir_parser import Function, Module, BasicBlock
from ir_utils import (
    extract_branch_targets,
    is_redundant_unconditional_branch,
    is_same_target_conditional,
    is_trivial_phi,
)


# ---------------------------------------------------------------------------
# Data Model
# ---------------------------------------------------------------------------

@dataclass
class FunctionCfMetrics:
    name: str
    block_count: int
    empty_blocks: int
    redundant_branches: int
    trivial_phis: int
    total_defects: int


@dataclass
class ControlFlowMetrics:
    per_function: list[FunctionCfMetrics]
    total_defects: int
    is_incorrect: bool


# ---------------------------------------------------------------------------
# Defect Detection
# ---------------------------------------------------------------------------

def _is_empty_block(block: BasicBlock) -> bool:
    """A block whose only instruction is an unconditional br."""
    return (len(block.instructions) == 1
            and block.instructions[0].opcode == "br"
            and "i1" not in block.instructions[0].text)


def _build_predecessor_map(func: Function) -> dict[str, list[str]]:
    """Build a map of block_label -> [predecessor_labels]."""
    preds: dict[str, list[str]] = {b.label: [] for b in func.blocks}
    for block in func.blocks:
        if not block.instructions:
            continue
        last = block.instructions[-1]
        targets = extract_branch_targets(last.text)
        for target in targets:
            if target in preds:
                preds[target].append(block.label)
    return preds


def _has_unreachable_blocks(func: Function) -> bool:
    """True if any non-entry block has no predecessors."""
    if len(func.blocks) <= 1:
        return False
    preds = _build_predecessor_map(func)
    entry = func.blocks[0].label
    for block in func.blocks:
        if block.label == entry:
            continue
        if not preds.get(block.label):
            return True
    return False


def _has_same_target_conditional(func: Function) -> bool:
    """True if any conditional branch has both targets the same."""
    for block in func.blocks:
        for instr in block.instructions:
            if is_same_target_conditional(instr):
                return True
    return False


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def compute_control_flow_metrics(module: Module) -> ControlFlowMetrics:
    """Compute control flow defect metrics for all user functions."""
    results: list[FunctionCfMetrics] = []
    total_defects = 0
    any_incorrect = False

    for func in module.user_functions():
        # Entry block (first block) is excluded from empty-block count — a
        # preheader block (`br label %header` to a phi-bearing loop header)
        # is structurally necessary for the phi to distinguish initial values
        # from loop-carried values.
        empty = sum(
            1 for i, b in enumerate(func.blocks)
            if _is_empty_block(b) and i != 0
        )
        # Entry block (i==0) preheaders are excluded — same rationale
        # as for empty blocks (structural loop preheaders).
        redundant = sum(
            1 for i, b in enumerate(func.blocks)
            if i != 0 and is_redundant_unconditional_branch(
                b, func.blocks[i + 1] if i + 1 < len(func.blocks) else None
            )
        )
        trivial = sum(
            1 for b in func.blocks
            for instr in b.instructions
            if is_trivial_phi(instr)
        )

        defects = empty + redundant + trivial
        total_defects += defects

        if _has_unreachable_blocks(func) or _has_same_target_conditional(func):
            any_incorrect = True

        results.append(FunctionCfMetrics(
            name=func.raw_name,
            block_count=len(func.blocks),
            empty_blocks=empty,
            redundant_branches=redundant,
            trivial_phis=trivial,
            total_defects=defects,
        ))

    return ControlFlowMetrics(
        per_function=results,
        total_defects=total_defects,
        is_incorrect=any_incorrect,
    )
