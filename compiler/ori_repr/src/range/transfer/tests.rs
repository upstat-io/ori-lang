//! Tests for transfer functions.
//!
//! TDD: written alongside implementation. Covers arithmetic, bitwise,
//! built-in ranges, and the primop dispatcher.

use super::*;
use crate::range::ValueRange::Bottom;

// ─── range_add ─────────────────────────────────────────────────

#[test]
fn add_positive_bounded() {
    assert_eq!(
        range_add(Bounded { lo: 0, hi: 10 }, Bounded { lo: 0, hi: 10 }),
        Bounded { lo: 0, hi: 20 }
    );
}

/// Semantic pin: add propagation.
#[test]
fn add_semantic_pin() {
    assert_eq!(
        range_add(Bounded { lo: 0, hi: 10 }, Bounded { lo: 0, hi: 10 }),
        Bounded { lo: 0, hi: 20 }
    );
}

#[test]
fn add_mixed_sign() {
    assert_eq!(
        range_add(Bounded { lo: -5, hi: 5 }, Bounded { lo: 1, hi: 3 }),
        Bounded { lo: -4, hi: 8 }
    );
}

#[test]
fn add_bottom_propagates() {
    assert_eq!(range_add(Bottom, Bounded { lo: 0, hi: 10 }), Bottom);
    assert_eq!(range_add(Bounded { lo: 0, hi: 10 }, Bottom), Bottom);
}

#[test]
fn add_top_absorbs() {
    assert_eq!(range_add(Top, Bounded { lo: 0, hi: 10 }), Top);
}

#[test]
fn add_overflow_returns_top() {
    assert_eq!(
        range_add(
            Bounded {
                lo: i64::MAX - 1,
                hi: i64::MAX
            },
            Bounded { lo: 1, hi: 2 }
        ),
        Top
    );
}

// ─── range_sub ─────────────────────────────────────────────────

#[test]
fn sub_basic() {
    assert_eq!(
        range_sub(Bounded { lo: 5, hi: 15 }, Bounded { lo: 1, hi: 3 }),
        Bounded { lo: 2, hi: 14 }
    );
}

#[test]
fn sub_bottom_propagates() {
    assert_eq!(range_sub(Bottom, Bounded { lo: 0, hi: 10 }), Bottom);
}

#[test]
fn sub_overflow_returns_top() {
    assert_eq!(
        range_sub(
            Bounded {
                lo: i64::MIN,
                hi: i64::MIN + 1
            },
            Bounded { lo: 1, hi: 2 }
        ),
        Top
    );
}

// ─── range_mul ─────────────────────────────────────────────────

#[test]
fn mul_positive_positive() {
    assert_eq!(
        range_mul(Bounded { lo: 2, hi: 3 }, Bounded { lo: 4, hi: 5 }),
        Bounded { lo: 8, hi: 15 }
    );
}

#[test]
fn mul_positive_negative() {
    assert_eq!(
        range_mul(Bounded { lo: 2, hi: 3 }, Bounded { lo: -5, hi: -4 }),
        Bounded { lo: -15, hi: -8 }
    );
}

#[test]
fn mul_negative_negative() {
    assert_eq!(
        range_mul(Bounded { lo: -3, hi: -2 }, Bounded { lo: -5, hi: -4 }),
        Bounded { lo: 8, hi: 15 }
    );
}

#[test]
fn mul_spanning_zero() {
    assert_eq!(
        range_mul(Bounded { lo: -2, hi: 3 }, Bounded { lo: -1, hi: 4 }),
        Bounded { lo: -8, hi: 12 }
    );
}

#[test]
fn mul_zero_times_anything() {
    assert_eq!(
        range_mul(Bounded { lo: 0, hi: 0 }, Bounded { lo: -100, hi: 100 }),
        Bounded { lo: 0, hi: 0 }
    );
}

#[test]
fn mul_overflow_returns_top() {
    assert_eq!(
        range_mul(
            Bounded {
                lo: i64::MAX,
                hi: i64::MAX
            },
            Bounded { lo: 2, hi: 2 }
        ),
        Top
    );
}

#[test]
fn mul_bottom_propagates() {
    assert_eq!(range_mul(Bottom, Bounded { lo: 1, hi: 2 }), Bottom);
}

// ─── range_div ─────────────────────────────────────────────────

#[test]
fn div_positive_positive() {
    assert_eq!(
        range_div(Bounded { lo: 10, hi: 20 }, Bounded { lo: 2, hi: 5 }),
        Bounded { lo: 2, hi: 10 }
    );
}

#[test]
fn div_by_zero_range_returns_top() {
    assert_eq!(
        range_div(Bounded { lo: 1, hi: 10 }, Bounded { lo: -1, hi: 1 }),
        Top
    );
}

#[test]
fn div_by_zero_only_returns_top() {
    assert_eq!(
        range_div(Bounded { lo: 1, hi: 10 }, Bounded { lo: 0, hi: 0 }),
        Top
    );
}

#[test]
fn div_negative_positive() {
    // -10/1=-10, -10/5=-2, -2/1=-2, -2/5=0 → min=-10, max=0
    assert_eq!(
        range_div(Bounded { lo: -10, hi: -2 }, Bounded { lo: 1, hi: 5 }),
        Bounded { lo: -10, hi: 0 }
    );
}

// ─── range_div — i64::MIN / -1 overflow (TPR-03-004) ──────────

#[test]
fn div_i64_min_by_neg1_returns_top() {
    // i64::MIN / -1 overflows — must not panic, must return Top.
    // Semantic pin: this test ONLY passes if checked division is used.
    assert_eq!(
        range_div(
            Bounded {
                lo: i64::MIN,
                hi: i64::MIN
            },
            Bounded { lo: -1, hi: -1 }
        ),
        Top
    );
}

#[test]
fn div_range_containing_i64_min_and_neg1_returns_top() {
    // Range where one corner hits i64::MIN / -1.
    assert_eq!(
        range_div(
            Bounded {
                lo: i64::MIN,
                hi: -1
            },
            Bounded { lo: -5, hi: -1 }
        ),
        Top
    );
}

#[test]
fn div_i64_min_by_positive_no_overflow() {
    // i64::MIN / 2 = valid, should produce a bounded result.
    assert_eq!(
        range_div(
            Bounded {
                lo: i64::MIN,
                hi: i64::MIN
            },
            Bounded { lo: 2, hi: 2 }
        ),
        Bounded {
            lo: i64::MIN / 2,
            hi: i64::MIN / 2
        }
    );
}

#[test]
fn floordiv_i64_min_by_neg1_returns_top() {
    // range_floordiv delegates to range_div — same overflow applies.
    assert_eq!(
        range_floordiv(
            Bounded {
                lo: i64::MIN,
                hi: i64::MIN
            },
            Bounded { lo: -1, hi: -1 }
        ),
        Top
    );
}

// ─── range_mod ─────────────────────────────────────────────────

#[test]
fn mod_basic() {
    let r = range_mod(Bounded { lo: 0, hi: 100 }, Bounded { lo: 7, hi: 7 });
    assert_eq!(r, Bounded { lo: -6, hi: 6 });
}

#[test]
fn mod_divisor_spans_zero_returns_top() {
    assert_eq!(
        range_mod(Bounded { lo: 0, hi: 10 }, Bounded { lo: -1, hi: 1 }),
        Top
    );
}

// ─── range_neg ─────────────────────────────────────────────────

#[test]
fn neg_basic() {
    assert_eq!(
        range_neg(Bounded { lo: -5, hi: 10 }),
        Bounded { lo: -10, hi: 5 }
    );
}

#[test]
fn neg_i64_min_returns_top() {
    assert_eq!(
        range_neg(Bounded {
            lo: i64::MIN,
            hi: 0
        }),
        Top
    );
}

#[test]
fn neg_bottom() {
    assert_eq!(range_neg(Bottom), Bottom);
}

// ─── range_abs ─────────────────────────────────────────────────

#[test]
fn abs_all_positive() {
    assert_eq!(
        range_abs(Bounded { lo: 3, hi: 10 }),
        Bounded { lo: 3, hi: 10 }
    );
}

#[test]
fn abs_all_negative() {
    assert_eq!(
        range_abs(Bounded { lo: -10, hi: -3 }),
        Bounded { lo: 3, hi: 10 }
    );
}

#[test]
fn abs_spanning_zero() {
    assert_eq!(
        range_abs(Bounded { lo: -7, hi: 3 }),
        Bounded { lo: 0, hi: 7 }
    );
}

#[test]
fn abs_i64_min_returns_top() {
    assert_eq!(
        range_abs(Bounded {
            lo: i64::MIN,
            hi: 0
        }),
        Top
    );
}

// ─── Bitwise ───────────────────────────────────────────────────

#[test]
fn bitand_non_negative() {
    assert_eq!(
        range_bitand(Bounded { lo: 0, hi: 255 }, Bounded { lo: 0, hi: 15 }),
        Bounded { lo: 0, hi: 15 }
    );
}

#[test]
fn bitand_negative_returns_top() {
    assert_eq!(
        range_bitand(Bounded { lo: -1, hi: 10 }, Bounded { lo: 0, hi: 10 }),
        Top
    );
}

#[test]
fn shl_basic() {
    assert_eq!(
        range_shl(Bounded { lo: 1, hi: 1 }, Bounded { lo: 3, hi: 3 }),
        Bounded { lo: 8, hi: 8 }
    );
}

#[test]
fn shl_negative_shift_returns_top() {
    assert_eq!(
        range_shl(Bounded { lo: 1, hi: 1 }, Bounded { lo: -1, hi: 3 }),
        Top
    );
}

#[test]
fn shl_shift_ge_64_returns_top() {
    assert_eq!(
        range_shl(Bounded { lo: 1, hi: 1 }, Bounded { lo: 0, hi: 64 }),
        Top
    );
}

#[test]
fn shr_basic() {
    // 8>>2=2, 16>>1=8 → [2, 8]
    assert_eq!(
        range_shr(Bounded { lo: 8, hi: 16 }, Bounded { lo: 1, hi: 2 }),
        Bounded { lo: 2, hi: 8 }
    );
}

#[test]
fn bitnot_positive_range() {
    // ~[5, 10] = [-11, -6]
    assert_eq!(
        range_bitnot(Bounded { lo: 5, hi: 10 }),
        Bounded { lo: -11, hi: -6 }
    );
}

#[test]
fn bitnot_bottom() {
    assert_eq!(range_bitnot(Bottom), Bottom);
}

// ─── range_bitnot — i64::MIN overflow (TPR-03-005) ────────────

#[test]
fn bitnot_i64_min_returns_top() {
    // ~i64::MIN requires negating i64::MIN which overflows.
    // Semantic pin: this test ONLY passes if checked negation is used.
    assert_eq!(
        range_bitnot(Bounded {
            lo: i64::MIN,
            hi: i64::MIN
        }),
        Top
    );
}

#[test]
fn bitnot_range_containing_i64_min_returns_top() {
    // lo = i64::MIN → negation overflows.
    assert_eq!(
        range_bitnot(Bounded {
            lo: i64::MIN,
            hi: -1
        }),
        Top
    );
}

#[test]
fn bitnot_i64_max_returns_bounded() {
    // ~i64::MAX = -i64::MAX - 1 = i64::MIN. Valid, no overflow.
    assert_eq!(
        range_bitnot(Bounded {
            lo: i64::MAX,
            hi: i64::MAX
        }),
        Bounded {
            lo: i64::MIN,
            hi: i64::MIN
        }
    );
}

#[test]
fn bitnot_negative_range() {
    // ~[-10, -5] = [4, 9]
    assert_eq!(
        range_bitnot(Bounded { lo: -10, hi: -5 }),
        Bounded { lo: 4, hi: 9 }
    );
}

// ─── Literals & built-in ranges ────────────────────────────────

#[test]
fn literal_exact() {
    assert_eq!(range_literal(42), Bounded { lo: 42, hi: 42 });
    assert_eq!(range_literal(-1), Bounded { lo: -1, hi: -1 });
}

#[test]
fn len_non_negative() {
    assert_eq!(
        range_len(),
        Bounded {
            lo: 0,
            hi: i64::MAX
        }
    );
}

#[test]
fn byte_to_int_range() {
    assert_eq!(range_byte_to_int(), Bounded { lo: 0, hi: 255 });
}

#[test]
fn char_to_int_range() {
    assert_eq!(
        range_char_to_int(),
        Bounded {
            lo: 0,
            hi: 0x10_FFFF
        }
    );
}

// ─── transfer_primop dispatcher ────────────────────────────────

#[test]
fn primop_add_routes_correctly() {
    let mut ranges = FxHashMap::default();
    let v0 = ArcVarId::new(0);
    let v1 = ArcVarId::new(1);
    ranges.insert(v0, Bounded { lo: 0, hi: 10 });
    ranges.insert(v1, Bounded { lo: 5, hi: 15 });
    let result = transfer_primop(PrimOp::Binary(BinaryOp::Add), &[v0, v1], &ranges);
    assert_eq!(result, Bounded { lo: 5, hi: 25 });
}

#[test]
fn primop_neg_routes_correctly() {
    let mut ranges = FxHashMap::default();
    let v0 = ArcVarId::new(0);
    ranges.insert(v0, Bounded { lo: 3, hi: 7 });
    let result = transfer_primop(PrimOp::Unary(UnaryOp::Neg), &[v0], &ranges);
    assert_eq!(result, Bounded { lo: -7, hi: -3 });
}

#[test]
fn primop_comparison_returns_bool_range() {
    let ranges = FxHashMap::default();
    let result = transfer_primop(PrimOp::Binary(BinaryOp::Lt), &[], &ranges);
    assert_eq!(result, Bounded { lo: 0, hi: 1 });
}

#[test]
fn primop_matmul_returns_top() {
    let ranges = FxHashMap::default();
    let result = transfer_primop(PrimOp::Binary(BinaryOp::MatMul), &[], &ranges);
    assert_eq!(result, Top);
}

#[test]
fn primop_try_returns_top() {
    let ranges = FxHashMap::default();
    let result = transfer_primop(PrimOp::Unary(UnaryOp::Try), &[], &ranges);
    assert_eq!(result, Top);
}

// ─── range_floordiv soundness (TPR-03-008) ────────────────────

#[test]
fn floordiv_mixed_sign_exact() {
    // -1 div 2: floor = -1, trunc = 0. Range must contain -1.
    // Semantic pin: ONLY passes with floor division (not truncating).
    assert_eq!(
        range_floordiv(Bounded { lo: -1, hi: -1 }, Bounded { lo: 2, hi: 2 }),
        Bounded { lo: -1, hi: -1 }
    );
}

#[test]
fn floordiv_mixed_sign_exact_2() {
    // -7 div 2: floor = -4, trunc = -3.
    assert_eq!(
        range_floordiv(Bounded { lo: -7, hi: -7 }, Bounded { lo: 2, hi: 2 }),
        Bounded { lo: -4, hi: -4 }
    );
}

#[test]
fn floordiv_same_sign_matches_truncating() {
    // 7 div 2: floor = 3, trunc = 3 — same for same-sign.
    assert_eq!(
        range_floordiv(Bounded { lo: 7, hi: 7 }, Bounded { lo: 2, hi: 2 }),
        Bounded { lo: 3, hi: 3 }
    );
}

#[test]
fn floordiv_negative_by_negative_same_sign() {
    // -7 div -2: floor = 3, trunc = 3 — same for same-sign.
    assert_eq!(
        range_floordiv(Bounded { lo: -7, hi: -7 }, Bounded { lo: -2, hi: -2 }),
        Bounded { lo: 3, hi: 3 }
    );
}

#[test]
fn floordiv_mixed_sign_range() {
    // [-7, -1] div [2, 5]:
    // Corners: -7 div 2=-4, -7 div 5=-2, -1 div 2=-1, -1 div 5=-1
    // min=-4, max=-1
    assert_eq!(
        range_floordiv(Bounded { lo: -7, hi: -1 }, Bounded { lo: 2, hi: 5 }),
        Bounded { lo: -4, hi: -1 }
    );
}

#[test]
fn floordiv_by_zero_range_returns_top() {
    assert_eq!(
        range_floordiv(Bounded { lo: 1, hi: 10 }, Bounded { lo: -1, hi: 1 }),
        Top
    );
}

#[test]
fn floordiv_positive_by_positive_range() {
    // [10, 20] div [2, 5]: same as truncating for positive/positive.
    // 10 div 5=2, 10 div 2=5, 20 div 5=4, 20 div 2=10 → [2, 10]
    assert_eq!(
        range_floordiv(Bounded { lo: 10, hi: 20 }, Bounded { lo: 2, hi: 5 }),
        Bounded { lo: 2, hi: 10 }
    );
}

#[test]
fn floordiv_bottom_propagates() {
    assert_eq!(range_floordiv(Bottom, Bounded { lo: 1, hi: 2 }), Bottom);
    assert_eq!(range_floordiv(Bounded { lo: 1, hi: 2 }, Bottom), Bottom);
}

// ─── range_shr sign-aware monotonicity (TPR-03-009) ───────────

#[test]
fn shr_negative_range_with_shift_range() {
    // [-8, -1] >> [1, 2]:
    // -8>>1=-4, -8>>2=-2, -1>>1=-1, -1>>2=-1
    // min=-4, max=-1
    // Semantic pin: ONLY passes with sign-aware corner computation.
    assert_eq!(
        range_shr(Bounded { lo: -8, hi: -1 }, Bounded { lo: 1, hi: 2 }),
        Bounded { lo: -4, hi: -1 }
    );
}

#[test]
fn shr_mixed_sign_range() {
    // [-8, 16] >> [1, 2]:
    // -8>>1=-4, -8>>2=-2, 16>>1=8, 16>>2=4
    // min=-4, max=8
    assert_eq!(
        range_shr(Bounded { lo: -8, hi: 16 }, Bounded { lo: 1, hi: 2 }),
        Bounded { lo: -4, hi: 8 }
    );
}

#[test]
fn shr_negative_exact_shift() {
    // -16 >> 2 = -4 (arithmetic)
    assert_eq!(
        range_shr(Bounded { lo: -16, hi: -16 }, Bounded { lo: 2, hi: 2 }),
        Bounded { lo: -4, hi: -4 }
    );
}

#[test]
fn shr_positive_range_unchanged() {
    // Positive ranges: existing behavior should be preserved.
    // [8, 16] >> [1, 2]: 8>>2=2, 16>>1=8 → [2, 8]
    assert_eq!(
        range_shr(Bounded { lo: 8, hi: 16 }, Bounded { lo: 1, hi: 2 }),
        Bounded { lo: 2, hi: 8 }
    );
}
