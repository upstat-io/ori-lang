#!/usr/bin/env python3
"""Attribute metrics -- Section 04.

Deterministically checks which LLVM attributes are applicable to each function
and whether they are correctly applied, producing compliance percentage.

Closure-aware rules (Section 06): functions with indirect calls (closures)
skip fastcc and nounwind applicability checks since these are structurally
impossible. Memory attribute checks are skipped for non-leaf functions.
"""

from __future__ import annotations

import re
from dataclasses import dataclass, field

from ir_parser import Function, Module

# Verified against compiler/ori_llvm/src/codegen/runtime_decl/runtime_functions.rs
NORETURN_FUNCTIONS = frozenset({"ori_panic_cstr", "ori_panic"})
COLD_FUNCTIONS = frozenset({"ori_panic_cstr", "ori_panic"})
RC_MEMORY_FUNCTIONS = frozenset({"ori_rc_inc", "ori_rc_dec"})
NOALIAS_RETURN_FUNCTIONS = frozenset({"ori_rc_alloc"})

# Functions that must NOT be nounwind (they need to unwind for RC cleanup)
MUST_NOT_BE_NOUNWIND = frozenset({"ori_panic_cstr", "ori_panic"})

# Indirect call detection (closures, function pointers)
# LLVM register names can contain dots (e.g., %closure.fn_ptr), so use [\w.]+ not \w+
_INDIRECT_CALL_RE = re.compile(r'(?:call|invoke)\b[^@]*%[\w.]+\s*\(')

# Extract callee name from call/invoke: @name (possibly quoted)
_CALLEE_RE = re.compile(r'@"?([\w.$]+)"?\s*\(')


# ---------------------------------------------------------------------------
# Data Model
# ---------------------------------------------------------------------------

@dataclass
class AttributeCheck:
    attr: str
    applicable: bool
    present: bool
    description: str


@dataclass
class FunctionAttributeMetrics:
    name: str
    checks: list[AttributeCheck]
    not_applicable_reason: str | None = None


@dataclass
class AttributeMetrics:
    per_function: list[FunctionAttributeMetrics]
    total_applicable: int
    total_correct: int
    compliance_pct: float
    has_wrong: bool


# ---------------------------------------------------------------------------
# Closure & Leaf Detection
# ---------------------------------------------------------------------------

def _has_indirect_calls(func: Function) -> bool:
    """True if function contains indirect call/invoke (closure, fn pointer)."""
    return any(
        _INDIRECT_CALL_RE.search(instr.text)
        for block in func.blocks
        for instr in block.instructions
    )


def _is_closure_function(func: Function) -> bool:
    """Detect functions related to closure implementation.

    Patterns:
    - Name contains "$lambda" or "$closure" or "$partial"
    - Function body contains indirect calls (call/invoke through %reg)
    """
    # Both $ and __ prefixed forms: Ori uses __lambda/__partial,
    # other compilers may use $lambda/$partial
    name_hints = ("$lambda", "$closure", "$partial",
                  "__lambda", "__closure", "_partial_")
    if any(hint in func.name for hint in name_hints):
        return True
    return _has_indirect_calls(func)


def _all_callees_nounwind(func: Function, module: Module) -> bool:
    """True if every call/invoke target in this function is nounwind.

    Used for non-leaf functions: if the compiler proved all callees nounwind
    (via two-pass fixed-point analysis), the function itself should be nounwind.
    Returns True for functions with no call/invoke instructions.
    """
    for block in func.blocks:
        for instr in block.instructions:
            if instr.opcode not in ("call", "invoke"):
                continue
            # Indirect calls (closures) — can't determine nounwind
            if _INDIRECT_CALL_RE.search(instr.text):
                return False
            m = _CALLEE_RE.search(instr.text)
            if not m:
                continue
            callee_name = m.group(1)
            # LLVM intrinsics are always nounwind
            if callee_name.startswith("llvm."):
                continue
            callee = module.functions.get(f"@{callee_name}")
            if callee is None:
                # Try quoted form
                callee = module.functions.get(f'@"{callee_name}"')
            if callee and "nounwind" not in callee.attributes:
                return False
    return True


def _is_leaf_function(func: Function, module: Module) -> bool:
    """True if function makes no calls to other user functions.

    Calls to runtime (ori_*) and LLVM intrinsics (llvm.*) don't count.
    """
    for block in func.blocks:
        for instr in block.instructions:
            if instr.opcode not in ("call", "invoke"):
                continue
            # Check if it calls a user function
            text = instr.text
            if "@_ori_" in text:
                return False
            # Indirect calls count as non-leaf
            if _INDIRECT_CALL_RE.search(text):
                return False
    return True


# ---------------------------------------------------------------------------
# Attribute Rules
# ---------------------------------------------------------------------------

def _is_attr_present(func: Function, attr: str) -> bool:
    """Check if an attribute is present on a function."""
    if attr == "fastcc":
        return func.calling_convention == "fastcc"
    return attr in func.attributes


# Each rule: (attr_name, applicable_when, description)
# The applicable_when function takes (func, is_closure, is_leaf) -> bool.
_ATTRIBUTE_RULES: list[tuple[str, callable, str]] = [
    ("fastcc",
     lambda f, c, l: f.is_user_function and not f.is_entry_called and not c,
     "Internal user function (not entry-called, not closure)"),

    # nounwind: handled specially in compute_attribute_metrics() because it
    # needs module-level callee lookup. Placeholder here for ordering.
    ("nounwind",
     lambda f, c, l: False,  # overridden per-function below
     "Leaf functions or non-leaf with all-nounwind callees"),

    ("uwtable",
     lambda f, c, l: f.is_definition and f.raw_name != "@main",
     "User-defined functions (not @main wrapper)"),

    ("noundef",
     lambda f, c, l: f.is_definition and f.return_type != "void",
     "Non-void return type (Ori values are always defined)"),

    ("noreturn",
     lambda f, c, l: f.is_runtime_decl and f.name in NORETURN_FUNCTIONS,
     "Functions that never return (panic/abort)"),

    ("cold",
     lambda f, c, l: f.is_runtime_decl and f.name in COLD_FUNCTIONS,
     "Error/panic path functions"),
]


# ---------------------------------------------------------------------------
# Wrong Attribute Detection
# ---------------------------------------------------------------------------

def _detect_wrong_attributes(func: Function) -> list[str]:
    """Detect attributes that are present but shouldn't be."""
    wrongs: list[str] = []

    # noreturn on a function that has ret instructions
    if "noreturn" in func.attributes:
        has_ret = any(
            instr.opcode == "ret"
            for block in func.blocks
            for instr in block.instructions
        )
        if has_ret:
            wrongs.append("noreturn on function that returns")

    # nounwind on runtime declarations that must unwind
    if func.is_runtime_decl and func.name in MUST_NOT_BE_NOUNWIND:
        if "nounwind" in func.attributes:
            wrongs.append(f"nounwind on {func.name} (must unwind for RC cleanup)")

    return wrongs


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def compute_attribute_metrics(module: Module) -> AttributeMetrics:
    """Compute attribute compliance metrics for all functions."""
    results: list[FunctionAttributeMetrics] = []
    total_applicable = 0
    total_correct = 0
    any_wrong = False

    functions_to_check = [
        f for f in module.functions.values()
        if f.is_definition or f.is_runtime_decl
    ]

    for func in functions_to_check:
        checks: list[AttributeCheck] = []
        is_closure = _is_closure_function(func)
        is_leaf = _is_leaf_function(func, module)
        reason = "closure (indirect calls)" if is_closure else None

        for attr, applicable_fn, desc in _ATTRIBUTE_RULES:
            if attr == "nounwind":
                # nounwind needs module-level callee lookup.
                # Applicable if: definition, not @main, not closure, AND
                # all callees (including runtime) are nounwind.
                # This subsumes the old leaf-only check: leaf functions have
                # no calls, so all callees are vacuously nounwind.
                applicable = (
                    func.is_definition
                    and func.raw_name != "@main"
                    and not is_closure
                    and _all_callees_nounwind(func, module)
                )
            else:
                applicable = applicable_fn(func, is_closure, is_leaf)
            present = _is_attr_present(func, attr)

            if applicable:
                total_applicable += 1
                if present:
                    total_correct += 1

            checks.append(AttributeCheck(
                attr=attr, applicable=applicable, present=present,
                description=desc,
            ))

        # Per-parameter noundef
        for i, pattrs in enumerate(func.param_attributes):
            ptype = func.param_types[i] if i < len(func.param_types) else "?"
            applicable = func.is_definition and ptype not in ("void",)
            present = "noundef" in pattrs

            if applicable:
                total_applicable += 1
                if present:
                    total_correct += 1

            checks.append(AttributeCheck(
                attr=f"noundef(param{i})",
                applicable=applicable,
                present=present,
                description=f"Parameter {i} ({ptype}) is always defined",
            ))

        # Wrong attribute detection
        wrongs = _detect_wrong_attributes(func)
        if wrongs:
            any_wrong = True

        results.append(FunctionAttributeMetrics(
            name=func.raw_name, checks=checks,
            not_applicable_reason=reason,
        ))

    compliance_pct = (
        (total_correct / total_applicable * 100.0) if total_applicable > 0
        else 100.0
    )

    return AttributeMetrics(
        per_function=results,
        total_applicable=total_applicable,
        total_correct=total_correct,
        compliance_pct=round(compliance_pct, 1),
        has_wrong=any_wrong,
    )
