//! Matrix tests for closure capture composition.
//!
//! Per §04.2 `success_criteria` + Test Strategy, the matrix covers the four
//! capture-shape variants enumerated in the §04.2 body:
//!   (1) Capture-by-value of an Owned binding → `owned_fields[i]` populated.
//!   (2) Capture-by-reference → `borrowed_fields[i]` populated; owned untouched.
//!   (3) Captures-of-captures (nested closures) → outer's `owned_field` carries
//!       the inner closure's `Idx`; inner's burden registered separately and
//!       resolved at codegen via `TypeRegistry::burden(inner_idx)`.
//!   (4) Capture-of-projection → treated as `borrowed_field` with parent's
//!       lifetime; the projection itself does not own — parent owns.
//!
//! Plus boundary pins for `compiled_drop` `FnSym` uniqueness per closure `Idx` +
//! the per-`Idx` mangling key shared with §04.1's recursive-type drop functions
//! per `_ori_drop$<idx_raw>` at `drop_gen.rs:45`.

use super::{compose_closure_burden_spec, mint_closure_drop_fn_sym, ClosureCapture};
use crate::registry::burden::UserBurdenSpec;
use crate::registry::burden_compose::scc::mint_compiled_drop_fn_sym;
use crate::Idx;

/// Synthetic closure `Idx` in the dynamic range (`FIRST_DYNAMIC` = 64). Avoids
/// aliasing pre-interned primitive slots (`types.md §TY-5`).
fn closure_idx(raw: u32) -> Idx {
    Idx::from_raw(Idx::FIRST_DYNAMIC + raw)
}

// ─── (1) Capture-by-value — single Owned binding ─────────────────────────

#[test]
fn closure_with_single_owned_str_capture_populates_one_owned_field() {
    // §04.2 success_criterion: capture-by-value of Owned binding produces
    // owned_fields entry with field_type = captured binding's resolved Idx.
    let ci = closure_idx(0);
    let captures = [ClosureCapture {
        field_index: 0,
        field_type: Idx::STR,
    }];
    let spec = compose_closure_burden_spec(ci, &captures, &[]);

    assert!(
        spec.self_heap_alloc,
        "closure env MUST be heap-allocated (self_heap_alloc: true)",
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
    // §04.2 success_criterion: field_index corresponds to caller's argument
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

// ─── (2) Capture-by-reference — single Borrowed binding ──────────────────

#[test]
fn closure_with_single_borrowed_capture_populates_borrowed_fields_not_owned() {
    // §04.2 success_criterion: capture-by-reference is borrow; stored in
    // borrowed_fields[i]; no drop on env field (borrows do not own).
    let ci = closure_idx(2);
    let captures = [ClosureCapture {
        field_index: 0,
        field_type: Idx::STR,
    }];
    let spec = compose_closure_burden_spec(ci, &[], &captures);

    assert!(spec.self_heap_alloc);
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

// ─── (3) Mixed — owned + borrowed captures in the same closure ───────────

#[test]
fn closure_with_mixed_owned_and_borrowed_captures_populates_both_field_sets() {
    // §04.2 success_criterion: a single closure can mix capture modes — each
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

// ─── (4) Captures-of-captures — nested closure as a captured field ───────

#[test]
fn closure_capturing_another_closure_carries_inner_idx_in_owned_field() {
    // §04.2 success_criterion: outer env field IS Closure<...> with its OWN
    // compiled_drop. Recursion handled identically to recursive types per
    // §04.1 — outer closure's drop body recursively invokes inner closure's
    // compiled_drop via the inner field's UserBurdenSpec lookup at codegen.
    //
    // Composition records the inner closure's Idx in owned_fields[i].field_type;
    // codegen resolves the inner burden via SEPARATE TypeRegistry::burden lookup.
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
        "outer's owned_field MUST carry the inner closure's Idx — codegen resolves the nested burden via TypeRegistry::burden(inner)",
    );

    // Inner closure has its own (distinct) burden + compiled_drop FnSym.
    let inner_captures = [ClosureCapture {
        field_index: 0,
        field_type: Idx::STR,
    }];
    let inner_spec = compose_closure_burden_spec(inner, &inner_captures, &[]);
    assert_ne!(
        outer_spec.compiled_drop, inner_spec.compiled_drop,
        "outer and inner closures MUST get distinct compiled_drop FnSyms (per-Idx mangling per drop_gen.rs:45)",
    );
}

// ─── (5) Capture-of-projection — borrowed_field with parent lifetime ─────

#[test]
fn closure_capturing_projection_uses_borrowed_field_with_parent_idx() {
    // §04.2 success_criterion: capture of projection treated as borrowed_fields
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

// ─── compiled_drop FnSym key invariant ────────────────────────────────────

#[test]
fn closure_compiled_drop_fn_sym_matches_per_idx_mangling_key() {
    // §04.2 + §04.1 mangling contract: closure drop functions share the
    // `_ori_drop$<idx_raw>` mangling key with recursive-type drop functions
    // per `drop_gen.rs:45`. mint_closure_drop_fn_sym MUST delegate to
    // scc::mint_compiled_drop_fn_sym so the per-Idx distinctness contract
    // (`raw + 1`, saturating) holds identically.
    let ci = closure_idx(7);
    let captures = [ClosureCapture {
        field_index: 0,
        field_type: Idx::STR,
    }];
    let spec = compose_closure_burden_spec(ci, &captures, &[]);
    let expected = mint_compiled_drop_fn_sym(ci);
    assert_eq!(
        spec.compiled_drop,
        Some(expected),
        "closure compiled_drop MUST follow the Idx-derived mangling shared with §04.1",
    );
    let direct = mint_closure_drop_fn_sym(ci);
    assert_eq!(
        direct, expected,
        "mint_closure_drop_fn_sym MUST delegate to scc::mint_compiled_drop_fn_sym",
    );
}

#[test]
fn closure_drop_fn_syms_for_distinct_closures_are_distinct() {
    // §04.2 + §04.1 per-Idx distinctness: two closures with distinct Idx values
    // MUST get distinct compiled_drop FnSyms even when their capture shapes are
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
        spec_a.compiled_drop, spec_b.compiled_drop,
        "distinct closure Idx values MUST yield distinct compiled_drop FnSyms",
    );
}

// ─── Non-capturing closure — empty owned_fields + empty borrowed_fields ──

#[test]
fn closure_with_zero_captures_yields_empty_field_sets_but_still_heap_alloc() {
    // §04.2 + arc.md §RT-2 closure env-header layout: even a closure with zero
    // captures has self_heap_alloc=true at the spec level — the env-header is
    // the load-bearing allocation site for the drop_fn slot. The codegen path
    // emit_rc_dec_closure (rc_ops.rs:266-268) short-circuits when env_ptr is
    // constant null (non-capturing); composition records the heap-alloc
    // disposition uniformly + the codegen short-circuit owns the runtime
    // optimization.
    let ci = closure_idx(10);
    let spec = compose_closure_burden_spec(ci, &[], &[]);

    assert!(
        spec.self_heap_alloc,
        "self_heap_alloc records the env-header disposition; non-capturing closures get the null-env short-circuit at runtime, not at spec-composition time",
    );
    assert!(spec.owned_fields.is_empty());
    assert!(spec.borrowed_fields.is_empty());
    assert!(spec.compiled_drop.is_some());
    assert!(spec.user_drop.is_none());
}

// ─── Default-shape invariants ─────────────────────────────────────────────

#[test]
fn closure_burden_default_invariants_no_variants_no_element() {
    // §04.2 success_criterion: element_burden: None; variant_burdens: empty.
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

// ─── Borrow-check-refinement sync — partition tracks classification ──────

#[test]
fn closure_owned_borrowed_partition_is_a_pure_function_of_classification_input() {
    // §04.2 borrow-check-refinement-sync verification (no-drift):
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
    let cap = ClosureCapture {
        field_index: 0,
        field_type: Idx::STR,
    };

    let as_owned = compose_closure_burden_spec(ci, &[cap.clone()], &[]);
    assert_eq!(as_owned.owned_fields.len(), 1);
    assert!(
        as_owned.borrowed_fields.is_empty(),
        "owned-classified capture leaves zero residue in borrowed_fields",
    );

    let as_borrowed = compose_closure_burden_spec(ci, &[], &[cap]);
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
    // self_heap_alloc=false, compiled_drop=None — both differ for closures).
    let ci = closure_idx(12);
    let spec = compose_closure_burden_spec(ci, &[], &[]);
    assert_ne!(spec, UserBurdenSpec::default());
}
