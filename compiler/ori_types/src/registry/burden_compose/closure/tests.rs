//! Matrix tests for closure capture composition.
//!
//! The matrix covers the four
//! capture-shape variants:
//!   (1) Capture-by-value of an Owned binding → `owned_fields[i]` populated.
//!   (2) Capture-by-reference → `borrowed_fields[i]` populated; owned untouched.
//!   (3) Captures-of-captures (nested closures) → outer's `owned_field` carries
//!       the inner closure's `Idx`; inner's burden registered separately and
//!       resolved through `TypeRegistry::burden(inner_idx)`.
//!   (4) Capture-of-projection → treated as `borrowed_field` with parent's
//!       lifetime; the projection itself does not own — parent owns.
//!
//! Plus boundary pins for stable `drop_operation` identity uniqueness per
//! closure `Idx` and the shared recursive-type identity space.

use super::{compose_closure_burden_spec, mint_closure_drop_operation, ClosureCapture};
use crate::registry::burden::UserBurdenSpec;
use crate::registry::burden_compose::scc::mint_drop_operation_sym;
use crate::Idx;

/// Synthetic closure `Idx` in the dynamic range (`FIRST_DYNAMIC` = 64). Avoids
/// aliasing pre-interned primitive slots (TY-5: pre-interned primitives occupy
/// the reserved Idx range below `FIRST_DYNAMIC`).
fn closure_idx(raw: u32) -> Idx {
    Idx::from_raw(Idx::FIRST_DYNAMIC + raw)
}

// (1) Capture-by-value — single Owned binding

#[test]
fn closure_with_single_owned_str_capture_populates_one_owned_field() {
    // success criterion: capture-by-value of Owned binding produces
    // owned_fields entry with field_type = captured binding's resolved Idx.
    let ci = closure_idx(0);
    let captures = [ClosureCapture {
        field_index: 0,
        field_type: Idx::STR,
    }];
    let spec = compose_closure_burden_spec(ci, &captures, &[]);

    assert!(
        spec.self_owned_identity,
        "a captured environment must introduce its own logical identity",
    );
    assert_eq!(
        spec.owned_fields.len(),
        1,
        "single capture-by-value → exactly one owned_fields entry",
    );
    assert_eq!(spec.owned_fields[0].field_type, Idx::STR);
    assert_eq!(spec.owned_fields[0].field_path, vec![0u32]);
    assert!(spec.borrowed_fields.is_empty());
    assert_eq!(spec.element_burden, None);
    assert!(spec.variant_burdens.is_empty());
    assert!(
        spec.user_drop.is_none(),
        "closures cannot have user @drop per drop-trait-proposal.md §Auto-derive",
    );
}

#[test]
fn closure_with_multiple_owned_captures_preserves_capture_order() {
    // success criterion: field_index corresponds to caller's argument
    // order at the PartialApply site. Captures supplied in order [STR, INT,
    // STR] MUST land in owned_fields[0..3] with the same Idx sequence.
    let ci = closure_idx(1);
    let captures = [
        ClosureCapture {
            field_index: 0,
            field_type: Idx::STR,
        },
        ClosureCapture {
            field_index: 1,
            field_type: Idx::INT,
        },
        ClosureCapture {
            field_index: 2,
            field_type: Idx::STR,
        },
    ];
    let spec = compose_closure_burden_spec(ci, &captures, &[]);

    assert_eq!(spec.owned_fields.len(), 3);
    let types: Vec<Idx> = spec.owned_fields.iter().map(|f| f.field_type).collect();
    assert_eq!(types, vec![Idx::STR, Idx::INT, Idx::STR]);
    let paths: Vec<&Vec<u32>> = spec.owned_fields.iter().map(|f| &f.field_path).collect();
    assert_eq!(paths, vec![&vec![0u32], &vec![1u32], &vec![2u32]]);
}

// (2) Capture-by-reference — single Borrowed binding

#[test]
fn closure_with_single_borrowed_capture_populates_borrowed_fields_not_owned() {
    // success criterion: capture-by-reference is borrow; stored in
    // borrowed_fields[i]; no drop on env field (borrows do not own).
    let ci = closure_idx(2);
    let captures = [ClosureCapture {
        field_index: 0,
        field_type: Idx::STR,
    }];
    let spec = compose_closure_burden_spec(ci, &[], &captures);

    assert!(spec.self_owned_identity);
    assert!(
        spec.owned_fields.is_empty(),
        "capture-by-reference does not populate owned_fields",
    );
    assert_eq!(
        spec.borrowed_fields.len(),
        1,
        "single capture-by-reference → exactly one borrowed_fields entry",
    );
    assert_eq!(spec.borrowed_fields[0].field_type, Idx::STR);
    assert_eq!(spec.borrowed_fields[0].field_path, vec![0u32]);
}

// (3) Mixed — owned + borrowed captures in the same closure

#[test]
fn closure_with_mixed_owned_and_borrowed_captures_populates_both_field_sets() {
    // success criterion: a single closure can mix capture modes — each
    // capture's field_index in its respective field set.
    let ci = closure_idx(3);
    let owned = [ClosureCapture {
        field_index: 0,
        field_type: Idx::STR,
    }];
    let borrowed = [ClosureCapture {
        field_index: 1,
        field_type: Idx::INT,
    }];
    let spec = compose_closure_burden_spec(ci, &owned, &borrowed);

    assert_eq!(spec.owned_fields.len(), 1);
    assert_eq!(spec.owned_fields[0].field_type, Idx::STR);
    assert_eq!(spec.borrowed_fields.len(), 1);
    assert_eq!(spec.borrowed_fields[0].field_type, Idx::INT);
}

// (4) Captures-of-captures — nested closure as a captured field

#[test]
fn closure_capturing_another_closure_carries_inner_idx_in_owned_field() {
    // success criterion: outer env field IS Closure<...> with its OWN
    // drop_operation. Recursion is handled identically to recursive types:
    // the outer environment's logical cleanup traverses the inner closure's
    // drop_operation via its UserBurdenSpec.
    //
    // Composition records the inner closure's Idx in owned_fields[i].field_type;
    // the consumer resolves the inner burden via a separate registry lookup.
    let outer = closure_idx(4);
    let inner = closure_idx(5);
    let captures = [ClosureCapture {
        field_index: 0,
        field_type: inner, // outer captures inner closure by value
    }];
    let outer_spec = compose_closure_burden_spec(outer, &captures, &[]);

    assert_eq!(outer_spec.owned_fields.len(), 1);
    assert_eq!(
        outer_spec.owned_fields[0].field_type, inner,
        "outer's owned field must carry the inner closure's stable type identity",
    );

    // The inner closure has its own distinct burden and cleanup identity.
    let inner_captures = [ClosureCapture {
        field_index: 0,
        field_type: Idx::STR,
    }];
    let inner_spec = compose_closure_burden_spec(inner, &inner_captures, &[]);
    assert_ne!(
        outer_spec.drop_operation, inner_spec.drop_operation,
        "outer and inner closures must get distinct cleanup identities",
    );
}

// (5) Capture-of-projection — borrowed_field with parent lifetime

#[test]
fn closure_capturing_projection_uses_borrowed_field_with_parent_idx() {
    // success criterion: capture of projection treated as borrowed_fields
    // entry — the projection itself does not own; parent owns. The field_type
    // is the projected field's resolved Idx, not the parent's.
    let ci = closure_idx(6);
    // Parent type's projected field (e.g., `p.a` where p: Pair { a: int, b: int })
    // — capture-of-projection borrows that field with parent's lifetime tied
    // by the borrow-inference machinery downstream of typeck.
    let captures = [ClosureCapture {
        field_index: 0,
        field_type: Idx::INT, // projected field's resolved type
    }];
    let spec = compose_closure_burden_spec(ci, &[], &captures);

    assert!(spec.owned_fields.is_empty());
    assert_eq!(spec.borrowed_fields.len(), 1);
    assert_eq!(
        spec.borrowed_fields[0].field_type,
        Idx::INT,
        "capture-of-projection records the projected field's type, not parent's",
    );
}

// Stable cleanup-operation identity

#[test]
fn closure_drop_operation_matches_per_idx_identity_key() {
    // Closure and recursive-type cleanup operations share one stable identity
    // space. Physical projections decide whether and how to name a helper.
    let ci = closure_idx(7);
    let captures = [ClosureCapture {
        field_index: 0,
        field_type: Idx::STR,
    }];
    let spec = compose_closure_burden_spec(ci, &captures, &[]);
    let expected = mint_drop_operation_sym(ci);
    assert_eq!(
        spec.drop_operation,
        Some(expected),
        "closure cleanup must follow the shared Idx-derived identity",
    );
    let direct = mint_closure_drop_operation(ci);
    assert_eq!(
        direct, expected,
        "closure cleanup identity must use the shared operation identity space",
    );
}

#[test]
fn closure_drop_operations_for_distinct_closures_are_distinct() {
    // per-Idx distinctness: two closures with distinct Idx values
    // MUST get distinct cleanup identities even when their capture shapes are
    // structurally identical.
    let a = closure_idx(8);
    let b = closure_idx(9);
    let captures = [ClosureCapture {
        field_index: 0,
        field_type: Idx::STR,
    }];
    let spec_a = compose_closure_burden_spec(a, &captures, &[]);
    let spec_b = compose_closure_burden_spec(b, &captures, &[]);
    assert_ne!(
        spec_a.drop_operation, spec_b.drop_operation,
        "distinct closure Idx values must yield distinct cleanup identities",
    );
}

// Non-capturing closure — empty owned_fields + empty borrowed_fields

#[test]
fn closure_with_zero_captures_keeps_conservative_callable_identity() {
    // The shared burden remains conservative because this producer does not
    // yet carry the later closure-site discharge proof. A physical plan may
    // erase storage and cleanup only after consuming that exact proof.
    let ci = closure_idx(10);
    let spec = compose_closure_burden_spec(ci, &[], &[]);

    assert!(spec.self_owned_identity);
    assert!(spec.owned_fields.is_empty());
    assert!(spec.borrowed_fields.is_empty());
    assert!(spec.drop_operation.is_some());
    assert!(spec.user_drop.is_none());
}

// Default-shape invariants

#[test]
fn closure_burden_default_invariants_no_variants_no_element() {
    // success criterion: element_burden: None; variant_burdens: empty.
    // Closures are neither collections nor sums.
    let ci = closure_idx(11);
    let owned = [ClosureCapture {
        field_index: 0,
        field_type: Idx::STR,
    }];
    let spec = compose_closure_burden_spec(ci, &owned, &[]);

    assert_eq!(spec.element_burden, None, "closures have no element burden");
    assert!(
        spec.variant_burdens.is_empty(),
        "closures have no variant burdens",
    );
}

// Borrow-check-refinement sync — partition tracks classification

#[test]
fn closure_owned_borrowed_partition_is_a_pure_function_of_classification_input() {
    // borrow-check-refinement-sync verification (no-drift):
    //
    // `compose_closure_burden_spec` is a PURE mapping of its `owned_captures`
    // / `borrowed_captures` inputs — it does NOT classify captures itself.
    // The owned/borrowed partition in the composed spec is therefore exactly
    // whatever classification the caller supplied. A capture that the borrow
    // checker refines from by-value (owned) to by-reference (borrowed) lands
    // in `borrowed_fields` IFF the caller feeds it as a borrowed capture —
    // there is no internal owned-default that a refinement could leave stale.
    //
    // Two compositions of the SAME capture (`Idx::STR` at field 0) under the
    // two opposite classifications: owned-input → owned_fields only;
    // borrowed-input → borrowed_fields only. The partition tracks the input
    // with zero residue in the other set. Wiring the composer at a site that
    // supplies the post-borrow-check classification therefore yields a
    // post-borrow-check-correct partition — no drift is possible from the
    // composer itself.
    let ci = closure_idx(13);
    let cap = [ClosureCapture {
        field_index: 0,
        field_type: Idx::STR,
    }];

    let as_owned = compose_closure_burden_spec(ci, &cap, &[]);
    assert_eq!(as_owned.owned_fields.len(), 1);
    assert!(
        as_owned.borrowed_fields.is_empty(),
        "owned-classified capture leaves zero residue in borrowed_fields",
    );

    let as_borrowed = compose_closure_burden_spec(ci, &[], &cap);
    assert!(
        as_borrowed.owned_fields.is_empty(),
        "borrowed-classified capture leaves zero residue in owned_fields — a \
         by-value→by-reference refinement carried through the input does NOT \
         leave a stale owned_fields entry that would emit a spurious dec",
    );
    assert_eq!(as_borrowed.borrowed_fields.len(), 1);
    assert_eq!(as_borrowed.borrowed_fields[0].field_type, Idx::STR);
}

#[test]
fn closure_burden_does_not_inherit_default_user_burden_spec() {
    // Defensive: confirm the composed spec is NOT structurally equal to
    // UserBurdenSpec::default() even at zero captures (default carries
    // self_owned_identity=false, drop_operation=None — both differ for closures).
    let ci = closure_idx(12);
    let spec = compose_closure_burden_spec(ci, &[], &[]);
    assert_ne!(spec, UserBurdenSpec::default());
}
