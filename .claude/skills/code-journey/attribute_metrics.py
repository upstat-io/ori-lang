#!/usr/bin/env python3
"""Attribute metrics — Section 04.

Deterministically checks which LLVM attributes are applicable to each function
and whether they are correctly applied, producing compliance percentage.
"""

from __future__ import annotations

from dataclasses import dataclass, field

from ir_parser import Function, Module

# Verified against compiler/ori_llvm/src/codegen/runtime_decl/runtime_functions.rs
NORETURN_FUNCTIONS = frozenset({"ori_panic_cstr", "ori_panic"})
COLD_FUNCTIONS = frozenset({"ori_panic_cstr", "ori_panic"})
RC_MEMORY_FUNCTIONS = frozenset({"ori_rc_inc", "ori_rc_dec"})
NOALIAS_RETURN_FUNCTIONS = frozenset({"ori_rc_alloc"})

# Functions that must NOT be nounwind (they need to unwind for RC cleanup)
MUST_NOT_BE_NOUNWIND = frozenset({"ori_panic_cstr", "ori_panic"})


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


@dataclass
class AttributeMetrics:
    per_function: list[FunctionAttributeMetrics]
    total_applicable: int
    total_correct: int
    compliance_pct: float
    has_wrong: bool


# ---------------------------------------------------------------------------
# Attribute Rules
# ---------------------------------------------------------------------------

def _is_attr_present(func: Function, attr: str) -> bool:
    """Check if an attribute is present on a function.

    For calling conventions (fastcc), checks func.calling_convention.
    For all other attributes, checks func.attributes (resolved set).
    """
    if attr == "fastcc":
        return func.calling_convention == "fastcc"
    return attr in func.attributes


# Each rule: (attr_name, applicable_when, description)
_ATTRIBUTE_RULES: list[tuple[str, callable, str]] = [
    ("fastcc",
     lambda f: f.is_user_function and not f.is_entry_called,
     "Internal user function (not entry-called)"),

    ("nounwind",
     lambda f: f.is_definition,
     "All defined functions (panic paths end in unreachable)"),

    ("uwtable",
     lambda f: f.is_definition and f.raw_name != "@main",
     "User-defined functions (not @main wrapper)"),

    ("noundef",
     lambda f: f.is_definition and f.return_type != "void",
     "Non-void return type (Ori values are always defined)"),

    ("noreturn",
     lambda f: f.is_runtime_decl and f.name in NORETURN_FUNCTIONS,
     "Functions that never return (panic/abort)"),

    ("cold",
     lambda f: f.is_runtime_decl and f.name in COLD_FUNCTIONS,
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

    # Check attribute rules on all functions (definitions + runtime decls)
    functions_to_check = [
        f for f in module.functions.values()
        if f.is_definition or f.is_runtime_decl
    ]

    for func in functions_to_check:
        checks: list[AttributeCheck] = []

        for attr, applicable_fn, desc in _ATTRIBUTE_RULES:
            applicable = applicable_fn(func)
            present = _is_attr_present(func, attr)

            if applicable:
                total_applicable += 1
                if present:
                    total_correct += 1

            checks.append(AttributeCheck(
                attr=attr, applicable=applicable, present=present,
                description=desc,
            ))

        # Also check per-parameter noundef
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

        results.append(FunctionAttributeMetrics(name=func.raw_name, checks=checks))

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
