#!/usr/bin/env python3
"""Instruction efficiency metrics — Section 02.

Computes the instruction ratio (actual/ideal) deterministically by identifying
unjustified overhead patterns in LLVM IR. The "ideal" is defined as:
    ideal = actual - unjustified
where "unjustified" counts instructions that serve no purpose.

Key design principle: overflow checking, panic paths, and ABI compliance are
JUSTIFIED overhead in Ori — they are part of the ideal, not bloat above it.
"""

from __future__ import annotations

import re
from dataclasses import dataclass, field

from ir_parser import Function, Module, BasicBlock, Instruction
from ir_utils import is_redundant_unconditional_branch, is_trivial_phi

_NORETURN_CALL_RE = re.compile(r'call\s+void\s+@ori_panic')


# ---------------------------------------------------------------------------
# Data Model
# ---------------------------------------------------------------------------

@dataclass
class FunctionMetrics:
    name: str
    actual: int
    unjustified: int
    ideal: int            # actual - unjustified (>= 1)
    ratio: float          # actual / ideal (>= 1.0)
    verdict: str          # OPTIMAL, NEAR-OPTIMAL, ACCEPTABLE, BLOATED, WASTEFUL


@dataclass
class InstructionMetrics:
    per_function: list[FunctionMetrics]
    avg_ratio: float      # Weighted average across user functions
    max_ratio: float      # Worst function ratio
    total_unjustified: int  # Sum across all user functions


# ---------------------------------------------------------------------------
# Unjustified Pattern Detectors
# ---------------------------------------------------------------------------

def _is_dead_after_noreturn(
    instr: Instruction, idx: int, block: BasicBlock
) -> bool:
    """True if this instruction follows a noreturn call in the same block.

    After `call void @ori_panic_cstr(...)` + `unreachable`, any further
    instructions in the block are dead code.
    """
    if idx == 0:
        return False
    prev = block.instructions[idx - 1]
    # After unreachable, everything is dead
    if prev.opcode == 'unreachable':
        return True
    # After a noreturn call (but before unreachable), also dead
    if prev.opcode == 'call' and _NORETURN_CALL_RE.search(prev.text):
        return instr.opcode != 'unreachable'
    return False


def _is_unnecessary_alloca_chain(
    instr: Instruction, idx: int, block: BasicBlock
) -> bool:
    """Conservative: flag alloca followed immediately by store+load in same block.

    Pattern: alloca → store to it → load from it, with no other uses.
    Only checks same-block, consecutive instructions (conservative heuristic).
    """
    if instr.opcode != 'alloca' or instr.result is None:
        return False

    alloca_reg = instr.result
    remaining = block.instructions[idx + 1:]
    stores = 0
    loads = 0
    other_uses = 0

    for later in remaining:
        if alloca_reg in later.text:
            if later.opcode == 'store':
                stores += 1
            elif later.opcode == 'load':
                loads += 1
            else:
                other_uses += 1

    return stores == 1 and loads == 1 and other_uses == 0


def compute_unjustified(func: Function) -> int:
    """Count instructions that shouldn't be in the IR."""
    unjustified = 0
    for idx, block in enumerate(func.blocks):
        next_block = func.blocks[idx + 1] if idx + 1 < len(func.blocks) else None

        if is_redundant_unconditional_branch(block, next_block):
            unjustified += 1

        for i, instr in enumerate(block.instructions):
            if is_trivial_phi(instr):
                unjustified += 1
            if _is_dead_after_noreturn(instr, i, block):
                unjustified += 1
            if _is_unnecessary_alloca_chain(instr, i, block):
                unjustified += 1

    return unjustified


# ---------------------------------------------------------------------------
# Ratio Calculation
# ---------------------------------------------------------------------------

def _verdict(ratio: float) -> str:
    if ratio == 1.0:
        return "OPTIMAL"
    if ratio <= 1.50:
        return "NEAR-OPTIMAL"
    if ratio <= 2.50:
        return "ACCEPTABLE"
    if ratio <= 5.00:
        return "BLOATED"
    return "WASTEFUL"


def compute_instruction_metrics(module: Module) -> InstructionMetrics:
    """Compute instruction efficiency metrics for all user functions."""
    results: list[FunctionMetrics] = []
    total_actual = 0
    total_ideal = 0
    max_ratio = 1.0

    for func in module.user_functions():
        actual = func.instruction_count
        unjustified = compute_unjustified(func)
        ideal = max(1, actual - unjustified)
        ratio = actual / ideal if ideal > 0 else 1.0

        results.append(FunctionMetrics(
            name=func.raw_name,
            actual=actual,
            unjustified=unjustified,
            ideal=ideal,
            ratio=round(ratio, 4),
            verdict=_verdict(ratio),
        ))

        total_actual += actual
        total_ideal += ideal
        max_ratio = max(max_ratio, ratio)

    avg_ratio = total_actual / total_ideal if total_ideal > 0 else 1.0

    return InstructionMetrics(
        per_function=results,
        avg_ratio=round(avg_ratio, 4),
        max_ratio=round(max_ratio, 4),
        total_unjustified=sum(fm.unjustified for fm in results),
    )
