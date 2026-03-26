//! Tests for the interval lattice (`ValueRange`) and `IntWidth` range methods.
//!
//! TDD: written BEFORE implementation — initially these fail (compile errors
//! or assertion failures). Implementation in `mod.rs` makes them pass unchanged.

use super::*;

// ─── join ──────────────────────────────────────────────────────

#[test]
fn join_bottom_identity() {
    let r = ValueRange::Bounded { lo: 0, hi: 10 };
    assert_eq!(r.join(ValueRange::Bottom), r);
    assert_eq!(ValueRange::Bottom.join(r), r);
}

#[test]
fn join_top_absorbing() {
    let r = ValueRange::Bounded { lo: 0, hi: 10 };
    assert_eq!(r.join(ValueRange::Top), ValueRange::Top);
    assert_eq!(ValueRange::Top.join(r), ValueRange::Top);
}

#[test]
fn join_bottom_bottom() {
    assert_eq!(
        ValueRange::Bottom.join(ValueRange::Bottom),
        ValueRange::Bottom
    );
}

#[test]
fn join_top_top() {
    assert_eq!(ValueRange::Top.join(ValueRange::Top), ValueRange::Top);
}

#[test]
fn join_commutative() {
    let a = ValueRange::Bounded { lo: 0, hi: 5 };
    let b = ValueRange::Bounded { lo: 3, hi: 10 };
    assert_eq!(a.join(b), b.join(a));
}

#[test]
fn join_idempotent() {
    let r = ValueRange::Bounded { lo: -10, hi: 20 };
    assert_eq!(r.join(r), r);
}

#[test]
fn join_disjoint_ranges() {
    let a = ValueRange::Bounded { lo: 0, hi: 5 };
    let b = ValueRange::Bounded { lo: 10, hi: 20 };
    assert_eq!(a.join(b), ValueRange::Bounded { lo: 0, hi: 20 });
}

#[test]
fn join_overlapping_ranges() {
    let a = ValueRange::Bounded { lo: 0, hi: 10 };
    let b = ValueRange::Bounded { lo: 5, hi: 15 };
    assert_eq!(a.join(b), ValueRange::Bounded { lo: 0, hi: 15 });
}

/// Semantic pin: join computes the enclosing interval, not intersection.
#[test]
fn join_semantic_pin_enclosing_interval() {
    let a = ValueRange::Bounded { lo: 0, hi: 99 };
    let b = ValueRange::Bounded { lo: 50, hi: 150 };
    assert_eq!(a.join(b), ValueRange::Bounded { lo: 0, hi: 150 });
}

#[test]
fn join_nested_ranges() {
    let outer = ValueRange::Bounded { lo: 0, hi: 100 };
    let inner = ValueRange::Bounded { lo: 20, hi: 80 };
    assert_eq!(outer.join(inner), outer);
}

// ─── meet ──────────────────────────────────────────────────────

#[test]
fn meet_top_identity() {
    let r = ValueRange::Bounded { lo: 0, hi: 10 };
    assert_eq!(r.meet(ValueRange::Top), r);
    assert_eq!(ValueRange::Top.meet(r), r);
}

#[test]
fn meet_bottom_absorbing() {
    let r = ValueRange::Bounded { lo: 0, hi: 10 };
    assert_eq!(r.meet(ValueRange::Bottom), ValueRange::Bottom);
    assert_eq!(ValueRange::Bottom.meet(r), ValueRange::Bottom);
}

#[test]
fn meet_top_top() {
    assert_eq!(ValueRange::Top.meet(ValueRange::Top), ValueRange::Top);
}

#[test]
fn meet_bottom_bottom() {
    assert_eq!(
        ValueRange::Bottom.meet(ValueRange::Bottom),
        ValueRange::Bottom
    );
}

#[test]
fn meet_commutative() {
    let a = ValueRange::Bounded { lo: 0, hi: 10 };
    let b = ValueRange::Bounded { lo: 5, hi: 15 };
    assert_eq!(a.meet(b), b.meet(a));
}

#[test]
fn meet_idempotent() {
    let r = ValueRange::Bounded { lo: -10, hi: 20 };
    assert_eq!(r.meet(r), r);
}

#[test]
fn meet_overlapping_ranges() {
    let a = ValueRange::Bounded { lo: 0, hi: 10 };
    let b = ValueRange::Bounded { lo: 5, hi: 15 };
    assert_eq!(a.meet(b), ValueRange::Bounded { lo: 5, hi: 10 });
}

#[test]
fn meet_disjoint_ranges() {
    let a = ValueRange::Bounded { lo: 0, hi: 5 };
    let b = ValueRange::Bounded { lo: 10, hi: 20 };
    assert_eq!(a.meet(b), ValueRange::Bottom);
}

#[test]
fn meet_nested_ranges() {
    let outer = ValueRange::Bounded { lo: 0, hi: 100 };
    let inner = ValueRange::Bounded { lo: 20, hi: 80 };
    assert_eq!(outer.meet(inner), inner);
}

// ─── fits_in ───────────────────────────────────────────────────

#[test]
fn fits_in_i8_boundaries() {
    assert!(ValueRange::Bounded { lo: -128, hi: 127 }.fits_in(IntWidth::I8));
    assert!(!ValueRange::Bounded { lo: -129, hi: 127 }.fits_in(IntWidth::I8));
    assert!(!ValueRange::Bounded { lo: -128, hi: 128 }.fits_in(IntWidth::I8));
    assert!(ValueRange::Bounded { lo: 0, hi: 0 }.fits_in(IntWidth::I8));
}

#[test]
fn fits_in_i16_boundaries() {
    assert!(ValueRange::Bounded {
        lo: -32768,
        hi: 32767
    }
    .fits_in(IntWidth::I16));
    assert!(!ValueRange::Bounded {
        lo: -32769,
        hi: 32767
    }
    .fits_in(IntWidth::I16));
    assert!(ValueRange::Bounded { lo: 0, hi: 255 }.fits_in(IntWidth::I16));
}

#[test]
fn fits_in_i32_boundaries() {
    assert!(ValueRange::Bounded {
        lo: -2_147_483_648,
        hi: 2_147_483_647
    }
    .fits_in(IntWidth::I32));
    assert!(!ValueRange::Bounded {
        lo: -2_147_483_649,
        hi: 0
    }
    .fits_in(IntWidth::I32));
}

#[test]
fn fits_in_i64_always() {
    assert!(ValueRange::Top.fits_in(IntWidth::I64));
    assert!(ValueRange::Bounded {
        lo: i64::MIN,
        hi: i64::MAX
    }
    .fits_in(IntWidth::I64));
}

#[test]
fn fits_in_top_narrow_widths() {
    assert!(!ValueRange::Top.fits_in(IntWidth::I8));
    assert!(!ValueRange::Top.fits_in(IntWidth::I16));
    assert!(!ValueRange::Top.fits_in(IntWidth::I32));
}

#[test]
fn fits_in_bottom_all_widths() {
    assert!(ValueRange::Bottom.fits_in(IntWidth::I8));
    assert!(ValueRange::Bottom.fits_in(IntWidth::I16));
    assert!(ValueRange::Bottom.fits_in(IntWidth::I32));
    assert!(ValueRange::Bottom.fits_in(IntWidth::I64));
}

// ─── min_width ─────────────────────────────────────────────────

#[test]
fn min_width_i8_range() {
    assert_eq!(
        ValueRange::Bounded { lo: -128, hi: 127 }.min_width(),
        IntWidth::I8
    );
    assert_eq!(
        ValueRange::Bounded { lo: 0, hi: 0 }.min_width(),
        IntWidth::I8
    );
    assert_eq!(
        ValueRange::Bounded { lo: -1, hi: 1 }.min_width(),
        IntWidth::I8
    );
}

#[test]
fn min_width_i16_range() {
    assert_eq!(
        ValueRange::Bounded { lo: -129, hi: 127 }.min_width(),
        IntWidth::I16
    );
    assert_eq!(
        ValueRange::Bounded { lo: 0, hi: 128 }.min_width(),
        IntWidth::I16
    );
    assert_eq!(
        ValueRange::Bounded {
            lo: -32768,
            hi: 32767
        }
        .min_width(),
        IntWidth::I16
    );
}

#[test]
fn min_width_i32_range() {
    assert_eq!(
        ValueRange::Bounded {
            lo: -32769,
            hi: 32767
        }
        .min_width(),
        IntWidth::I32
    );
    assert_eq!(
        ValueRange::Bounded {
            lo: -2_147_483_648,
            hi: 2_147_483_647
        }
        .min_width(),
        IntWidth::I32
    );
}

#[test]
fn min_width_i64_range() {
    assert_eq!(
        ValueRange::Bounded {
            lo: i64::MIN,
            hi: i64::MAX
        }
        .min_width(),
        IntWidth::I64
    );
    assert_eq!(
        ValueRange::Bounded {
            lo: -2_147_483_649,
            hi: 0
        }
        .min_width(),
        IntWidth::I64
    );
}

#[test]
fn min_width_top() {
    assert_eq!(ValueRange::Top.min_width(), IntWidth::I64);
}

#[test]
fn min_width_bottom() {
    assert_eq!(ValueRange::Bottom.min_width(), IntWidth::I8);
}

// ─── is_constant ───────────────────────────────────────────────

#[test]
fn is_constant_single_value() {
    assert_eq!(
        ValueRange::Bounded { lo: 42, hi: 42 }.is_constant(),
        Some(42)
    );
    assert_eq!(
        ValueRange::Bounded { lo: -1, hi: -1 }.is_constant(),
        Some(-1)
    );
    assert_eq!(ValueRange::Bounded { lo: 0, hi: 0 }.is_constant(), Some(0));
}

#[test]
fn is_constant_range() {
    assert_eq!(ValueRange::Bounded { lo: 0, hi: 10 }.is_constant(), None);
}

#[test]
fn is_constant_special() {
    assert_eq!(ValueRange::Bottom.is_constant(), None);
    assert_eq!(ValueRange::Top.is_constant(), None);
}

// ─── overlaps ──────────────────────────────────────────────────

#[test]
fn overlaps_disjoint() {
    let a = ValueRange::Bounded { lo: 0, hi: 5 };
    let b = ValueRange::Bounded { lo: 6, hi: 10 };
    assert!(!a.overlaps(&b));
    assert!(!b.overlaps(&a));
}

#[test]
fn overlaps_touching() {
    let a = ValueRange::Bounded { lo: 0, hi: 5 };
    let b = ValueRange::Bounded { lo: 5, hi: 10 };
    assert!(a.overlaps(&b));
}

#[test]
fn overlaps_nested() {
    let outer = ValueRange::Bounded { lo: 0, hi: 10 };
    let inner = ValueRange::Bounded { lo: 3, hi: 7 };
    assert!(outer.overlaps(&inner));
    assert!(inner.overlaps(&outer));
}

#[test]
fn overlaps_identical() {
    let r = ValueRange::Bounded { lo: 0, hi: 10 };
    assert!(r.overlaps(&r));
}

#[test]
fn overlaps_with_bottom() {
    let r = ValueRange::Bounded { lo: 0, hi: 10 };
    assert!(!r.overlaps(&ValueRange::Bottom));
    assert!(!ValueRange::Bottom.overlaps(&r));
    assert!(!ValueRange::Bottom.overlaps(&ValueRange::Bottom));
}

#[test]
fn overlaps_with_top() {
    let r = ValueRange::Bounded { lo: 0, hi: 10 };
    assert!(r.overlaps(&ValueRange::Top));
    assert!(ValueRange::Top.overlaps(&r));
    assert!(ValueRange::Top.overlaps(&ValueRange::Top));
}

// ─── Default ───────────────────────────────────────────────────

#[test]
fn default_is_top() {
    assert_eq!(ValueRange::default(), ValueRange::Top);
}

// ─── IntWidth::signed_range / unsigned_range ───────────────────

#[test]
fn signed_range_i8() {
    assert_eq!(
        IntWidth::I8.signed_range(),
        ValueRange::Bounded { lo: -128, hi: 127 }
    );
}

#[test]
fn signed_range_i16() {
    assert_eq!(
        IntWidth::I16.signed_range(),
        ValueRange::Bounded {
            lo: -32768,
            hi: 32767
        }
    );
}

#[test]
fn signed_range_i32() {
    assert_eq!(
        IntWidth::I32.signed_range(),
        ValueRange::Bounded {
            lo: -2_147_483_648,
            hi: 2_147_483_647
        }
    );
}

#[test]
fn signed_range_i64_is_top() {
    assert_eq!(IntWidth::I64.signed_range(), ValueRange::Top);
}

#[test]
fn unsigned_range_i8() {
    assert_eq!(
        IntWidth::I8.unsigned_range(),
        ValueRange::Bounded { lo: 0, hi: 255 }
    );
}

#[test]
fn unsigned_range_i16() {
    assert_eq!(
        IntWidth::I16.unsigned_range(),
        ValueRange::Bounded { lo: 0, hi: 65535 }
    );
}

#[test]
fn unsigned_range_i32() {
    assert_eq!(
        IntWidth::I32.unsigned_range(),
        ValueRange::Bounded {
            lo: 0,
            hi: 4_294_967_295
        }
    );
}

#[test]
fn unsigned_range_i64_is_top() {
    assert_eq!(IntWidth::I64.unsigned_range(), ValueRange::Top);
}

// ─── i64 boundary edge cases ───────────────────────────────────

#[test]
fn min_width_i64_min_constant() {
    assert_eq!(
        ValueRange::Bounded {
            lo: i64::MIN,
            hi: i64::MIN
        }
        .min_width(),
        IntWidth::I64
    );
}

#[test]
fn fits_in_i64_boundaries() {
    let full = ValueRange::Bounded {
        lo: i64::MIN,
        hi: i64::MAX,
    };
    assert!(!full.fits_in(IntWidth::I8));
    assert!(!full.fits_in(IntWidth::I16));
    assert!(!full.fits_in(IntWidth::I32));
    assert!(full.fits_in(IntWidth::I64));
}

// ─── RangeAnalysisConfig ───────────────────────────────────────

#[test]
fn config_default_values() {
    let c = RangeAnalysisConfig::default();
    assert_eq!(c.max_iterations, 20);
    assert_eq!(c.max_blocks, 500);
    assert_eq!(c.max_scc_iterations, 10);
    assert_eq!(c.max_total_scc_iterations, 50);
}
