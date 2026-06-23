use super::*;

#[test]
fn from_name_maps_every_c_star_kind() {
    // Full c_* inventory (count=9 incl CPtr) — the FFI type dimension.
    let cases = [
        ("c_char", CAbiKind::CChar),
        ("c_short", CAbiKind::CShort),
        ("c_int", CAbiKind::CInt),
        ("c_long", CAbiKind::CLong),
        ("c_longlong", CAbiKind::CLongLong),
        ("c_size", CAbiKind::CSize),
        ("c_float", CAbiKind::CFloat),
        ("c_double", CAbiKind::CDouble),
        ("CPtr", CAbiKind::CPtr),
    ];
    let mut count = 0;
    for (name, kind) in cases {
        assert_eq!(CAbiKind::from_name(name), Some(kind), "name {name}");
        count += 1;
    }
    // Self-verifying matrix completeness — no c_* cell silently skipped.
    assert_eq!(count, 9, "the full c_* inventory must be 9 entries");
}

#[test]
fn from_name_rejects_non_ffi_names() {
    // Width-leakage guard: non-FFI names carry no CAbiKind.
    assert_eq!(CAbiKind::from_name("int"), None);
    assert_eq!(CAbiKind::from_name("float"), None);
    assert_eq!(CAbiKind::from_name("MyStruct"), None);
    assert_eq!(CAbiKind::from_name(""), None);
}

#[test]
fn float_kinds_are_exactly_cfloat_cdouble() {
    // is_float drives the FLOAT-vs-INT concrete resolution at every FFI site.
    // Pin it per-variant so an is_float regression (e.g. always-false) that
    // silently lowers a C `double`/`float` to i64 fails here.
    assert!(CAbiKind::CFloat.is_float());
    assert!(CAbiKind::CDouble.is_float());
    // Integer kinds:
    for k in [
        CAbiKind::CChar,
        CAbiKind::CShort,
        CAbiKind::CInt,
        CAbiKind::CLong,
        CAbiKind::CLongLong,
        CAbiKind::CSize,
        CAbiKind::CPtr,
    ] {
        assert!(!k.is_float(), "{k:?} must not be a float kind");
    }
}

#[test]
fn every_all_entry_round_trips_through_c_name() {
    // Iterate the canonical inventory (not a hand-listed copy): every ALL entry
    // maps to a name that from_name inverts back to the same variant. Ties ALL,
    // c_name, and from_name into one mutually-consistent set — a new ALL entry
    // is covered automatically.
    for k in CAbiKind::ALL {
        assert_eq!(CAbiKind::from_name(k.c_name()), Some(k), "{k:?}");
    }
    assert_eq!(
        CAbiKind::ALL.len(),
        9,
        "the full c_* inventory must be 9 entries"
    );
}

#[test]
fn target_variant_kinds_are_exactly_clong_csize_cptr() {
    // Only these three are target-resolved by ori_repr.
    assert!(CAbiKind::CLong.is_target_variant());
    assert!(CAbiKind::CSize.is_target_variant());
    assert!(CAbiKind::CPtr.is_target_variant());
    // Target-invariant kinds:
    for k in [
        CAbiKind::CChar,
        CAbiKind::CShort,
        CAbiKind::CInt,
        CAbiKind::CLongLong,
        CAbiKind::CFloat,
        CAbiKind::CDouble,
    ] {
        assert!(!k.is_target_variant(), "{k:?} must be target-invariant");
    }
}
