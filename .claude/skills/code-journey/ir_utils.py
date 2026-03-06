#!/usr/bin/env python3
"""Shared IR analysis utilities for instruction metrics (§02) and control flow metrics (§05).

Contains detection functions for patterns that affect both scoring dimensions:
- Redundant unconditional branches (counted as unjustified + CF defect)
- Trivial phi nodes (counted as unjustified + CF defect)

Depends on ir_parser data model only. No circular dependencies.
"""

from __future__ import annotations

import re

from ir_parser import BasicBlock, Instruction

_BR_LABEL_RE = re.compile(r'br\s+label\s+%(\S+)')
_BR_COND_RE = re.compile(
    r'br\s+i1\s+\S+,\s*label\s+%(\S+),\s*label\s+%(\S+)'
)
_PHI_VALUES_RE = re.compile(r'\[\s*([^,\]]+?)\s*,')
_SWITCH_LABELS_RE = re.compile(r'label\s+%(\S+)')


def extract_branch_targets(text: str) -> list[str]:
    """Extract branch target labels from a br or switch instruction.

    Returns list of target label names (without %).
    - Unconditional br: ["target"]
    - Conditional br: ["true_target", "false_target"]
    - Switch: ["default", "case0", "case1", ...]
    """
    # Conditional branch
    cond = _BR_COND_RE.search(text)
    if cond:
        return [cond.group(1), cond.group(2)]

    # Unconditional branch
    uncond = _BR_LABEL_RE.search(text)
    if uncond:
        return [uncond.group(1)]

    # Switch instruction
    if text.strip().startswith('switch'):
        return _SWITCH_LABELS_RE.findall(text)

    return []


def extract_phi_values(text: str) -> list[str]:
    """Extract incoming values from a phi instruction.

    phi i64 [ %val1, %block1 ], [ %val2, %block2 ] → ["%val1", "%val2"]
    """
    return _PHI_VALUES_RE.findall(text)


def is_redundant_unconditional_branch(
    block: BasicBlock, next_block: BasicBlock | None
) -> bool:
    """True if block ends with `br label %X` where X is the next block.

    This is a fall-through jump that serves no purpose.
    """
    if not block.instructions:
        return False
    last = block.instructions[-1]
    if last.opcode != 'br':
        return False
    # Must be unconditional (no i1 condition)
    if 'i1' in last.text:
        return False
    targets = extract_branch_targets(last.text)
    return (len(targets) == 1
            and next_block is not None
            and targets[0] == next_block.label)


def is_trivial_phi(instr: Instruction) -> bool:
    """True if phi has single incoming value or all values are the same."""
    if instr.opcode != 'phi':
        return False
    values = extract_phi_values(instr.text)
    return len(values) <= 1 or len(set(values)) == 1


def is_same_target_conditional(instr: Instruction) -> bool:
    """True if conditional br has both targets the same (semantic incorrectness)."""
    if instr.opcode != 'br' or 'i1' not in instr.text:
        return False
    targets = extract_branch_targets(instr.text)
    return len(targets) == 2 and targets[0] == targets[1]
