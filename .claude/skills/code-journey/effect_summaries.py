#!/usr/bin/env python3
"""Runtime function RC effect summaries for Code Journey metrics.

Maps Ori runtime function names to their reference-counting effects,
following the Clang RetainCountChecker pattern (RetainSummaryManager).

Each entry specifies:
- return_effect: Does the return value carry a +1 RC?
- param_effects: Does any parameter get consumed (-1) or incremented (+1)?
- is_allocation: Does this function create a new RC-managed object?

Verified against compiler/ori_rt/src/ and
compiler/ori_llvm/src/codegen/runtime_decl/runtime_functions.rs

Requires Python 3.10+ for union type syntax.
"""

from __future__ import annotations

from dataclasses import dataclass
from enum import Enum


class RcEffect(Enum):
    """RC effect of a function on a value."""
    NONE = "none"           # No RC effect
    PLUS_ONE = "+1"         # Allocates / returns owned (+1 RC)
    MINUS_ONE = "-1"        # Consumes / decrements (-1 RC)
    BORROWED = "borrowed"   # Borrows (no RC change, must not be freed)


@dataclass(frozen=True)
class FunctionEffect:
    """RC effects of a runtime function."""
    return_effect: RcEffect       # Effect on the return value
    param_effects: tuple[RcEffect, ...]  # Effect on each parameter (by index)
    is_allocation: bool = False   # True if this function creates a new RC object


def _eff(
    ret: RcEffect,
    params: list[RcEffect],
    *,
    alloc: bool = False,
) -> FunctionEffect:
    """Shorthand constructor."""
    return FunctionEffect(ret, tuple(params), alloc)


_N = RcEffect.NONE
_P = RcEffect.PLUS_ONE
_M = RcEffect.MINUS_ONE
_B = RcEffect.BORROWED


# ---------------------------------------------------------------------------
# Effect Table
# ---------------------------------------------------------------------------
# WARNING (scalability): ~45 entries (~300 lines). Each new
# `pub extern "C" fn ori_*` in ori_rt needs a manual entry.
# The sync test (Section 01.4) catches missing entries. If the table
# grows past ~60 entries, consider auto-generating from ori_rt with
# an #[rc_effect(...)] attribute or build script.

RUNTIME_EFFECTS: dict[str, FunctionEffect] = {
    # --- Allocation (return +1) ---
    "ori_rc_alloc": _eff(_P, [_N, _N], alloc=True),          # size, align
    # Note: ori_str_from_raw returns OriStr struct {len, cap, data} via sret.
    # The +1 RC is on the embedded data pointer, not the struct itself.
    # SSO strings (<=23 bytes) have no heap allocation (data ptr is null).
    "ori_str_from_raw": _eff(_P, [_B, _N], alloc=True),      # src_ptr, len
    # Note: ori_str_concat also returns OriStr struct via sret.
    # Params are *const OriStr (pointers to struct, not to data).
    "ori_str_concat": _eff(_P, [_B, _B], alloc=True),        # a, b
    "ori_list_alloc_data": _eff(_P, [_N, _N], alloc=True),   # capacity, elem_size
    # Note: ori_map_alloc does NOT exist. Map allocation uses
    # ori_map_literal_alloc(count, key_size, val_size, out_cap) -> ptr.
    "ori_map_literal_alloc": _eff(_P, [_N] * 4, alloc=True),

    # --- Deallocation (param -1) ---
    "ori_rc_dec": _eff(_N, [_M, _N]),                         # data_ptr, drop_fn
    # ori_rc_free is raw memory deallocation, NOT an RC operation.
    # It's called from drop functions (invoked by ori_rc_dec at RC=0).
    # The RC decrement already happened in ori_rc_dec; ori_rc_free just
    # frees the backing memory. Counting it as -1 double-counts the release.
    "ori_rc_free": _eff(_N, [_N, _N, _N]),                    # data_ptr, size, align
    # (data, len, cap, elem_size, elem_dec_fn) -- 5 params
    "ori_buffer_rc_dec": _eff(_N, [_M] + [_N] * 4),
    "ori_buffer_drop_unique": _eff(_N, [_M] + [_N] * 4),
    # (data, cap, len, key_size, val_size, key_dec_fn, val_dec_fn) -- 7 params
    "ori_map_buffer_rc_dec": _eff(_N, [_M] + [_N] * 6),
    "ori_map_buffer_drop_unique": _eff(_N, [_M] + [_N] * 6),

    # --- Increment (+1 on param, no return effect) ---
    "ori_rc_inc": _eff(_N, [_P]),                             # data_ptr
    "ori_list_rc_inc": _eff(_N, [_P, _N]),                    # data_ptr, cap

    # --- String methods that return new OriStr (possible +1) ---
    # Each returns OriStr struct; +1 on embedded data ptr if non-SSO.
    "ori_str_from_int": _eff(_P, [_N], alloc=True),
    "ori_str_from_bool": _eff(_P, [_N], alloc=True),
    "ori_str_from_float": _eff(_P, [_N], alloc=True),
    "ori_str_substring": _eff(_P, [_B, _N, _N], alloc=True),
    "ori_str_trim": _eff(_P, [_B], alloc=True),
    "ori_str_to_uppercase": _eff(_P, [_B], alloc=True),
    "ori_str_to_lowercase": _eff(_P, [_B], alloc=True),
    "ori_str_replace": _eff(_P, [_B, _B, _B], alloc=True),
    "ori_str_repeat": _eff(_P, [_B, _N], alloc=True),
    "ori_str_push_char": _eff(_P, [_B, _N], alloc=True),

    # --- Format functions (return OriStr, possible +1) ---
    "ori_format_int": _eff(_P, [_N, _B, _N], alloc=True),
    "ori_format_float": _eff(_P, [_N, _B, _N], alloc=True),
    "ori_format_str": _eff(_P, [_B, _B, _N], alloc=True),
    "ori_format_bool": _eff(_P, [_N, _B, _N], alloc=True),
    "ori_format_char": _eff(_P, [_N, _B, _N], alloc=True),

    # --- String empty (returns OriStr struct, SSO — no heap alloc but still +1 conceptually) ---
    "ori_str_empty": _eff(_P, [], alloc=True),

    # --- Set allocation ---
    "ori_set_literal_alloc": _eff(_P, [_N, _N, _N], alloc=True),
    "ori_set_empty": _eff(_P, [], alloc=True),

    # --- List allocation (additional) ---
    "ori_list_empty": _eff(_P, [], alloc=True),

    # --- Set RC operations ---
    # (data, cap, len, elem_size, hash_fn, elem_dec_fn) -- 6 params
    "ori_set_buffer_rc_dec": _eff(_N, [_M] + [_N] * 5),
    "ori_set_buffer_drop_unique": _eff(_N, [_M] + [_N] * 5),

    # --- Iterator sources (allocate heap-backed iterator state) ---
    # Iterator sources consume (take ownership of) the pre-incremented
    # collection reference. The ori_list_rc_inc/etc. before creation gives
    # the iterator its own reference; the iterator releases it on drop.
    # param 0 = collection data pointer (consumed via ownership transfer).
    "ori_iter_from_list": _eff(_P, [_M, _N, _N, _N, _N], alloc=True),  # data, len, cap, elem_size, elem_dec_fn
    "ori_iter_from_range": _eff(_P, [_N] * 4, alloc=True),   # start, end, step, incl (no RC objects)
    "ori_iter_from_str": _eff(_P, [_M], alloc=True),          # str data (pre-inc'd)
    "ori_iter_from_map": _eff(_P, [_M, _N, _N, _N], alloc=True),  # data, cap, key_size, val_size (pre-inc'd)

    # --- Iterator adapters (consume input iterator, return new) ---
    "ori_iter_map": _eff(_P, [_M, _B, _N], alloc=True),
    "ori_iter_filter": _eff(_P, [_M, _B, _N], alloc=True),
    "ori_iter_take": _eff(_P, [_M, _N], alloc=True),
    "ori_iter_skip": _eff(_P, [_M, _N], alloc=True),
    "ori_iter_enumerate": _eff(_P, [_M], alloc=True),
    "ori_iter_zip": _eff(_P, [_M, _M, _N], alloc=True),
    "ori_iter_chain": _eff(_P, [_M, _M], alloc=True),

    # --- Iterator consumers (consume input iterator, no new allocation) ---
    "ori_iter_drop": _eff(_N, [_M]),

    # --- Catch/recover (returns OriStr) ---
    "ori_catch_recover": _eff(_P, [], alloc=True),
}


def get_effect(func_name: str) -> FunctionEffect | None:
    """Look up the RC effect of a runtime function.

    Returns None for unknown functions (user-defined or unrecognized runtime).
    COW functions (ori_*_cow) are handled by pattern matching.
    """
    if func_name in RUNTIME_EFFECTS:
        return RUNTIME_EFFECTS[func_name]
    # COW pattern: ori_{type}_{op}_cow
    if func_name.startswith("ori_") and func_name.endswith("_cow"):
        return FunctionEffect(
            return_effect=RcEffect.PLUS_ONE,
            param_effects=(_B,),  # input is borrowed
        )
    return None


def is_allocation_function(func_name: str) -> bool:
    """Check if a function is known to allocate (return +1)."""
    effect = get_effect(func_name)
    return effect is not None and effect.is_allocation
