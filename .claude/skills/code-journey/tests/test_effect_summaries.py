"""Tests for effect_summaries.py -- runtime function RC effect table."""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from effect_summaries import (
    RUNTIME_EFFECTS,
    FunctionEffect,
    RcEffect,
    get_effect,
    is_allocation_function,
)


class TestGetEffect:
    def test_known_function(self):
        effect = get_effect("ori_rc_alloc")
        assert effect is not None
        assert effect.return_effect == RcEffect.PLUS_ONE
        assert effect.is_allocation

    def test_unknown_function(self):
        assert get_effect("unknown_function") is None

    def test_user_function(self):
        assert get_effect("_ori_my_func") is None

    def test_cow_pattern(self):
        effect = get_effect("ori_list_push_cow")
        assert effect is not None
        assert effect.return_effect == RcEffect.PLUS_ONE
        assert effect.param_effects == (RcEffect.BORROWED,)

    def test_cow_pattern_requires_ori_prefix(self):
        assert get_effect("something_cow") is None


class TestAllocationFunctions:
    def test_str_from_raw_is_allocation(self):
        assert is_allocation_function("ori_str_from_raw")

    def test_list_alloc_data_is_allocation(self):
        assert is_allocation_function("ori_list_alloc_data")

    def test_rc_inc_is_not_allocation(self):
        assert not is_allocation_function("ori_rc_inc")

    def test_rc_dec_is_not_allocation(self):
        assert not is_allocation_function("ori_rc_dec")

    def test_unknown_is_not_allocation(self):
        assert not is_allocation_function("unknown")


class TestEffectTable:
    def test_rc_inc_has_plus_one_param(self):
        effect = get_effect("ori_rc_inc")
        assert effect is not None
        assert effect.return_effect == RcEffect.NONE
        assert RcEffect.PLUS_ONE in effect.param_effects

    def test_rc_dec_has_minus_one_param(self):
        effect = get_effect("ori_rc_dec")
        assert effect is not None
        assert effect.return_effect == RcEffect.NONE
        assert RcEffect.MINUS_ONE in effect.param_effects

    def test_str_from_raw_returns_plus_one(self):
        effect = get_effect("ori_str_from_raw")
        assert effect is not None
        assert effect.return_effect == RcEffect.PLUS_ONE
        # Parameters are borrowed (src_ptr) and none (len)
        assert effect.param_effects[0] == RcEffect.BORROWED
        assert effect.param_effects[1] == RcEffect.NONE

    def test_buffer_rc_dec_consumes_first_param(self):
        effect = get_effect("ori_buffer_rc_dec")
        assert effect is not None
        assert effect.param_effects[0] == RcEffect.MINUS_ONE

    def test_iter_map_consumes_and_allocates(self):
        effect = get_effect("ori_iter_map")
        assert effect is not None
        assert effect.return_effect == RcEffect.PLUS_ONE  # new iterator
        assert effect.param_effects[0] == RcEffect.MINUS_ONE  # consumes input

    def test_iter_drop_consumes(self):
        effect = get_effect("ori_iter_drop")
        assert effect is not None
        assert effect.return_effect == RcEffect.NONE
        assert effect.param_effects[0] == RcEffect.MINUS_ONE

    def test_set_operations_present(self):
        assert get_effect("ori_set_literal_alloc") is not None
        assert get_effect("ori_set_buffer_rc_dec") is not None
        assert get_effect("ori_set_buffer_drop_unique") is not None

    def test_map_operations_present(self):
        assert get_effect("ori_map_literal_alloc") is not None
        assert get_effect("ori_map_buffer_rc_dec") is not None
        assert get_effect("ori_map_buffer_drop_unique") is not None


class TestEffectTableCompleteness:
    """Verify all allocation functions have matching deallocation patterns."""

    def test_every_allocation_returns_plus_one(self):
        for name, effect in RUNTIME_EFFECTS.items():
            if effect.is_allocation:
                assert effect.return_effect == RcEffect.PLUS_ONE, (
                    f"{name} is marked as allocation but return_effect is "
                    f"{effect.return_effect}"
                )

    def test_every_dealloc_has_minus_one_param(self):
        dealloc_names = [
            "ori_rc_dec", "ori_rc_free",
            "ori_buffer_rc_dec", "ori_buffer_drop_unique",
            "ori_map_buffer_rc_dec", "ori_map_buffer_drop_unique",
            "ori_set_buffer_rc_dec", "ori_set_buffer_drop_unique",
            "ori_iter_drop",
        ]
        for name in dealloc_names:
            effect = get_effect(name)
            assert effect is not None, f"{name} missing from table"
            assert RcEffect.MINUS_ONE in effect.param_effects, (
                f"{name} should have MINUS_ONE param effect"
            )

    def test_frozen_effects(self):
        """FunctionEffect is frozen dataclass -- param_effects is tuple."""
        for name, effect in RUNTIME_EFFECTS.items():
            assert isinstance(effect.param_effects, tuple), (
                f"{name}: param_effects should be tuple, got {type(effect.param_effects)}"
            )
