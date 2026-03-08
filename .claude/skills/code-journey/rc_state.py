#!/usr/bin/env python3
"""CFG-aware RC state tracking -- Section 02.

Forward-only dataflow analysis tracking per-SSA-value RC state through
the control flow graph. Eliminates false positives from conditional RC
paths (SSO gates, null guards, error branches).

Inspired by Swift's TopDownRefCountState (lib/SILOptimizer/ARC/RefCountState.h),
simplified to forward-only analysis with conservative merge at CFG joins.
This is the MVP (Phase 1a) -- sufficient for Ori's simple branching patterns.

Requires Python 3.10+ for union type syntax.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from enum import Enum

from effect_summaries import RcEffect, get_effect
from ir_parser import BasicBlock, Function
from ir_parser_internal import INVOKE_TARGETS_RE
from ir_utils import extract_branch_targets


# ---------------------------------------------------------------------------
# State Machine
# ---------------------------------------------------------------------------

class RcState(Enum):
    """State of a single RC-managed value."""
    UNKNOWN = "unknown"
    LIVE = "live"               # Known RC >= 1 (allocated or incremented)
    INCREMENTED = "incremented"  # Explicitly incremented
    DECREMENTED = "decremented"  # Explicitly decremented
    CONSUMED = "consumed"        # Ownership transferred


@dataclass
class ValueState:
    """RC state for a single SSA value."""
    state: RcState = RcState.UNKNOWN
    inc_count: int = 0
    dec_count: int = 0
    is_conditional: bool = False

    def apply_inc(self) -> None:
        self.inc_count += 1
        if self.state in (RcState.UNKNOWN, RcState.DECREMENTED):
            self.state = RcState.INCREMENTED
        elif self.state == RcState.LIVE:
            self.state = RcState.INCREMENTED

    def apply_dec(self) -> None:
        self.dec_count += 1
        if self.state == RcState.INCREMENTED:
            self.state = RcState.LIVE  # inc/dec cancel
        else:
            self.state = RcState.DECREMENTED

    def apply_alloc(self) -> None:
        self.inc_count += 1
        self.state = RcState.LIVE

    @property
    def balanced(self) -> bool:
        return self.inc_count == self.dec_count


def merge_value_states(states: list[ValueState]) -> ValueState:
    """Merge states from multiple predecessor blocks (conservative)."""
    if not states:
        return ValueState()
    if len(states) == 1:
        return ValueState(
            state=states[0].state,
            inc_count=states[0].inc_count,
            dec_count=states[0].dec_count,
            is_conditional=states[0].is_conditional,
        )

    # Conservative merge: take min of inc/dec (some paths may not execute RC ops)
    min_inc = min(s.inc_count for s in states)
    max_inc = max(s.inc_count for s in states)
    min_dec = min(s.dec_count for s in states)
    max_dec = max(s.dec_count for s in states)

    # If different paths have different counts, RC ops are conditional
    is_conditional = (min_inc != max_inc) or (min_dec != max_dec)

    merged_state = RcState.UNKNOWN
    all_states = {s.state for s in states}
    if len(all_states) == 1:
        merged_state = all_states.pop()
    elif RcState.CONSUMED in all_states:
        merged_state = RcState.CONSUMED
    elif RcState.LIVE in all_states:
        merged_state = RcState.LIVE

    return ValueState(
        state=merged_state,
        inc_count=min_inc,
        dec_count=min_dec,
        is_conditional=is_conditional or any(s.is_conditional for s in states),
    )


# ---------------------------------------------------------------------------
# CFG Construction
# ---------------------------------------------------------------------------

def build_cfg(func: Function) -> dict[str, list[str]]:
    """Build successor map from basic blocks.

    Handles br, switch, invoke, and ret terminators.
    """
    cfg: dict[str, list[str]] = {b.label: [] for b in func.blocks}

    for block in func.blocks:
        if not block.instructions:
            continue

        # Check all instructions for invoke (which creates edges but isn't
        # necessarily the last instruction parsed as a single unit)
        for instr in block.instructions:
            if instr.opcode == "invoke":
                m = INVOKE_TARGETS_RE.search(instr.text)
                if m:
                    for target in (m.group(1), m.group(2)):
                        if target in cfg and target not in cfg[block.label]:
                            cfg[block.label].append(target)

        # Check the last instruction for br/switch
        last = block.instructions[-1]
        if last.opcode in ("br", "switch"):
            targets = extract_branch_targets(last.text)
            for target in targets:
                if target in cfg and target not in cfg[block.label]:
                    cfg[block.label].append(target)

    return cfg


def build_predecessor_map(cfg: dict[str, list[str]]) -> dict[str, list[str]]:
    """Invert successor map to get predecessors."""
    preds: dict[str, list[str]] = {label: [] for label in cfg}
    for label, succs in cfg.items():
        for succ in succs:
            if succ in preds:
                preds[succ].append(label)
    return preds


def compute_rpo(cfg: dict[str, list[str]], entry: str) -> list[str]:
    """Compute reverse-postorder traversal of CFG."""
    visited: set[str] = set()
    postorder: list[str] = []

    def dfs(label: str) -> None:
        if label in visited:
            return
        visited.add(label)
        for succ in cfg.get(label, []):
            dfs(succ)
        postorder.append(label)

    dfs(entry)
    return list(reversed(postorder))


# ---------------------------------------------------------------------------
# Callee extraction (duplicated from arc_metrics to avoid circular import)
# ---------------------------------------------------------------------------

import re
_CALLEE_RE = re.compile(r'(?:call|invoke)\b[^@]*@(?:"([^"]+)"|([\w.$]+))\s*\(')


def _extract_callee(text: str) -> str | None:
    m = _CALLEE_RE.search(text)
    if not m:
        return None
    return m.group(1) or m.group(2)


# ---------------------------------------------------------------------------
# Block-Level Analysis
# ---------------------------------------------------------------------------

def analyze_block(
    block: BasicBlock,
    entry_state: dict[str, ValueState],
) -> dict[str, ValueState]:
    """Process one basic block, returning exit state.

    Tracks RC state transitions for each SSA value that participates
    in RC operations.
    """
    # Copy entry state
    state: dict[str, ValueState] = {
        k: ValueState(
            state=v.state, inc_count=v.inc_count,
            dec_count=v.dec_count, is_conditional=v.is_conditional,
        )
        for k, v in entry_state.items()
    }

    for instr in block.instructions:
        callee = _extract_callee(instr.text)
        if not callee:
            continue

        effect = get_effect(callee)
        if not effect:
            continue

        # Return value allocation
        if effect.return_effect == RcEffect.PLUS_ONE and instr.result:
            vs = state.setdefault(instr.result, ValueState())
            vs.apply_alloc()

        # Parameter effects (need to extract args from instruction text)
        # For simplicity, track by callee function name pattern
        if effect.param_effects:
            for i, pe in enumerate(effect.param_effects):
                if pe == RcEffect.PLUS_ONE:
                    arg = _extract_nth_arg(instr.text, i)
                    if arg:
                        vs = state.setdefault(arg, ValueState())
                        vs.apply_inc()
                elif pe == RcEffect.MINUS_ONE:
                    arg = _extract_nth_arg(instr.text, i)
                    if arg:
                        vs = state.setdefault(arg, ValueState())
                        vs.apply_dec()

    return state


def _extract_nth_arg(call_text: str, n: int) -> str | None:
    """Extract the nth argument from a call/invoke instruction."""
    paren = call_text.find('(')
    if paren < 0:
        return None

    # Find matching closing paren
    depth = 0
    args_start = paren + 1
    for i in range(paren, len(call_text)):
        if call_text[i] == '(':
            depth += 1
        elif call_text[i] == ')':
            depth -= 1
            if depth == 0:
                args_end = i
                break
    else:
        return None

    args_text = call_text[args_start:args_end]
    if not args_text.strip():
        return None

    # Split by commas (respecting nested parens)
    args: list[str] = []
    current = []
    depth = 0
    for ch in args_text:
        if ch == '(':
            depth += 1
        elif ch == ')':
            depth -= 1
        elif ch == ',' and depth == 0:
            args.append(''.join(current).strip())
            current = []
            continue
        current.append(ch)
    if current:
        args.append(''.join(current).strip())

    if n >= len(args):
        return None

    # Extract the SSA value from "type value" (e.g., "ptr %x" -> "%x")
    parts = args[n].split()
    return parts[-1] if parts else None


# ---------------------------------------------------------------------------
# Function-Level Analysis
# ---------------------------------------------------------------------------

@dataclass
class CfgRcResult:
    """Result of CFG-aware RC analysis for a function."""
    total_inc: int = 0
    total_dec: int = 0
    balanced: bool = True
    conditional_ops: int = 0
    values: dict[str, ValueState] = field(default_factory=dict)


def analyze_function_rc(func: Function) -> CfgRcResult:
    """Perform forward CFG-aware RC analysis on a function.

    Returns per-value RC state information including whether operations
    are conditional (behind branches).
    """
    if not func.blocks:
        return CfgRcResult()

    cfg = build_cfg(func)
    preds = build_predecessor_map(cfg)
    entry = func.blocks[0].label
    rpo = compute_rpo(cfg, entry)

    # Block label -> block
    block_map = {b.label: b for b in func.blocks}

    # Block label -> exit state
    exit_states: dict[str, dict[str, ValueState]] = {}

    # Forward analysis in RPO (single pass for acyclic, max 10 for loops)
    for iteration in range(10):
        changed = False

        for label in rpo:
            block = block_map.get(label)
            if block is None:
                continue

            # Merge predecessor exit states
            pred_labels = preds.get(label, [])
            if not pred_labels:
                entry_state: dict[str, ValueState] = {}
            elif len(pred_labels) == 1:
                entry_state = {
                    k: ValueState(
                        state=v.state, inc_count=v.inc_count,
                        dec_count=v.dec_count, is_conditional=v.is_conditional,
                    )
                    for k, v in exit_states.get(pred_labels[0], {}).items()
                }
            else:
                # Merge from multiple predecessors
                all_keys: set[str] = set()
                for pl in pred_labels:
                    all_keys.update(exit_states.get(pl, {}).keys())

                entry_state = {}
                for key in all_keys:
                    incoming = [
                        exit_states[pl][key]
                        for pl in pred_labels
                        if key in exit_states.get(pl, {})
                    ]
                    if len(incoming) < len(pred_labels):
                        # Value not present in all predecessors = conditional
                        incoming.append(ValueState(is_conditional=True))
                    entry_state[key] = merge_value_states(incoming)

            new_exit = analyze_block(block, entry_state)

            old_exit = exit_states.get(label, {})
            if new_exit != old_exit:
                exit_states[label] = new_exit
                changed = True

        if not changed:
            break

    # Collect results from all exit states (last block or ret blocks)
    all_values: dict[str, ValueState] = {}
    for label, block_state in exit_states.items():
        block = block_map.get(label)
        if block is None:
            continue
        # Check if this block has a ret instruction (terminal)
        has_ret = any(i.opcode == "ret" for i in block.instructions)
        if has_ret or not cfg.get(label):  # Terminal block
            for key, vs in block_state.items():
                if key not in all_values:
                    all_values[key] = vs
                else:
                    all_values[key] = merge_value_states([all_values[key], vs])

    total_inc = sum(vs.inc_count for vs in all_values.values())
    total_dec = sum(vs.dec_count for vs in all_values.values())
    conditional_ops = sum(1 for vs in all_values.values() if vs.is_conditional)

    return CfgRcResult(
        total_inc=total_inc,
        total_dec=total_dec,
        balanced=total_inc == total_dec,
        conditional_ops=conditional_ops,
        values=all_values,
    )
