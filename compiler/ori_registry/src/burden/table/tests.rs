//! Tests for `BURDEN_TABLE` + `BurdenRegistry::lookup_builtin`.
//!
//! Success criteria: every builtin `TypeId` in `BURDEN_TABLE` has the correct
//! template; lookup misses return `None`; `BurdenRegistry::lookup_builtin`
//! on user-type ids returns `None`.

use core::num::NonZeroU32;

use super::*;
use crate::TypeTag;

// Helper: looks up a TypeId we know is in the table, with a useful panic
// message when missing. Used in place of `.expect()` (clippy::expect_used
// is workspace-denied).
fn lookup_required(id: TypeId) -> &'static BuiltinBurdenSpec {
    match BurdenRegistry::lookup_builtin(id) {
        Some(spec) => spec,
        None => panic!("missing BURDEN_TABLE entry for {id:?}"),
    }
}

// Const-promoted NonZeroU32 literals for negative-lookup tests; avoids
// `NonZeroU32::new(N).expect(...)` at runtime (clippy::expect_used denied).
const NZ_100: NonZeroU32 = match NonZeroU32::new(100) {
    Some(n) => n,
    None => unreachable!(),
};

const NZ_9000: NonZeroU32 = match NonZeroU32::new(9_000) {
    Some(n) => n,
    None => unreachable!(),
};

#[test]
fn primitives_and_range_have_empty_burden() {
    for id in [
        TYPE_ID_INT,
        TYPE_ID_FLOAT,
        TYPE_ID_BOOL,
        TYPE_ID_CHAR,
        TYPE_ID_BYTE,
        TYPE_ID_UNIT,
        TYPE_ID_NEVER,
        TYPE_ID_DURATION,
        TYPE_ID_SIZE,
        TYPE_ID_ORDERING,
        TYPE_ID_RANGE,
    ] {
        let spec = lookup_required(id);
        assert!(
            !spec.self_heap_alloc,
            "{id:?}: self_heap_alloc must be false"
        );
        assert!(
            spec.owned_fields.is_empty(),
            "{id:?}: owned_fields must be empty"
        );
        assert!(
            spec.borrowed_fields.is_empty(),
            "{id:?}: borrowed_fields must be empty"
        );
        assert!(
            spec.variant_burdens.is_empty(),
            "{id:?}: variant_burdens must be empty"
        );
        assert!(
            spec.element_burden.is_none(),
            "{id:?}: element_burden must be None"
        );
        assert!(
            spec.compiled_drop.is_none(),
            "{id:?}: compiled_drop must be None"
        );
        assert!(spec.user_drop.is_none(), "{id:?}: user_drop must be None");
    }
}

#[test]
fn str_is_heap_with_no_element_burden() {
    let spec = lookup_required(TYPE_ID_STR);
    assert!(spec.self_heap_alloc);
    assert_eq!(spec.element_burden, None);
    assert!(spec.owned_fields.is_empty());
    assert!(spec.variant_burdens.is_empty());
}

#[test]
fn collections_carry_type_param_placeholder() {
    for id in [TYPE_ID_LIST, TYPE_ID_MAP, TYPE_ID_SET] {
        let spec = lookup_required(id);
        assert!(spec.self_heap_alloc, "{id:?}: must self_heap_alloc");
        assert_eq!(
            spec.element_burden,
            Some(TYPE_PARAM_T),
            "{id:?}: element_burden placeholder"
        );
        assert!(
            spec.owned_fields.is_empty(),
            "{id:?}: owned_fields template empty"
        );
        assert!(spec.variant_burdens.is_empty(), "{id:?}: not a sum type");
    }
}

#[test]
fn option_variants_have_correct_transfers() {
    let spec = lookup_required(TYPE_ID_OPTION);
    assert!(!spec.self_heap_alloc);
    assert!(spec.element_burden.is_none());
    assert_eq!(spec.variant_burdens.len(), 2);

    let none = &spec.variant_burdens[0];
    assert_eq!(none.variant_id, OPTION_VARIANT_NONE);
    assert!(none.transfers_on_match.is_empty());
    assert!(none.retained_owned.is_empty());

    let some = &spec.variant_burdens[1];
    assert_eq!(some.variant_id, OPTION_VARIANT_SOME);
    assert_eq!(some.transfers_on_match.len(), 1);
    let transfer = some.transfers_on_match[0];
    assert_eq!(transfer.binding_index, 0);
    assert_eq!(transfer.field_type, TYPE_PARAM_T);
    assert_eq!(transfer.transfer_kind, TransferKind::Move);
    assert!(transfer.source_field_path.is_empty());
}

#[test]
fn result_variants_have_correct_transfers() {
    let spec = lookup_required(TYPE_ID_RESULT);
    assert_eq!(spec.variant_burdens.len(), 2);

    let ok = &spec.variant_burdens[0];
    assert_eq!(ok.variant_id, RESULT_VARIANT_OK);
    assert_eq!(ok.transfers_on_match.len(), 1);
    assert_eq!(ok.transfers_on_match[0].field_type, TYPE_PARAM_T);
    assert_eq!(ok.transfers_on_match[0].transfer_kind, TransferKind::Move);

    let err = &spec.variant_burdens[1];
    assert_eq!(err.variant_id, RESULT_VARIANT_ERR);
    assert_eq!(err.transfers_on_match.len(), 1);
    assert_eq!(err.transfers_on_match[0].field_type, TYPE_PARAM_E);
    assert_eq!(err.transfers_on_match[0].transfer_kind, TransferKind::Move);
}

#[test]
fn lookup_returns_none_for_unknown_type_id() {
    // 100 is well outside the active TypeTag range (currently 23 builtins)
    // and well below the type-param sentinel ceiling (u32::MAX).
    assert!(BurdenRegistry::lookup_builtin(TypeId::new(NZ_100)).is_none());
}

#[test]
fn lookup_returns_none_for_high_unmapped_id() {
    // 9000 is also outside the table — guards against off-by-one shifts.
    assert!(BurdenRegistry::lookup_builtin(TypeId::new(NZ_9000)).is_none());
}

#[test]
fn burden_table_has_expected_entry_count() {
    // 10 primitives + Range + Str + List + Map + Set + Option + Result = 17.
    // Remaining builtins (Error, Tuple, Channel, Function, Iterator,
    // DoubleEndedIterator) are composition-layer territory and intentionally
    // absent from BURDEN_TABLE.
    assert_eq!(BURDEN_TABLE.len(), 17);
}

#[test]
fn burden_table_has_no_duplicate_type_ids() {
    for (i, (id_i, _)) in BURDEN_TABLE.iter().enumerate() {
        for (id_j, _) in &BURDEN_TABLE[i + 1..] {
            assert_ne!(id_i, id_j, "duplicate TypeId in BURDEN_TABLE: {id_i:?}");
        }
    }
}

#[test]
fn out_of_scope_builtins_return_none() {
    // Generic + non-generic builtins whose monomorphized burdens are owned
    // by the composition layer — they intentionally have NO entry in
    // BURDEN_TABLE, so the lookup helper returns None.
    for id in [
        TYPE_ID_ERROR,
        TYPE_ID_TUPLE,
        TYPE_ID_CHANNEL,
        TYPE_ID_FUNCTION,
        TYPE_ID_ITERATOR,
        TYPE_ID_DOUBLE_ENDED_ITERATOR,
    ] {
        assert!(
            BurdenRegistry::lookup_builtin(id).is_none(),
            "{id:?} unexpectedly populated in BURDEN_TABLE — owned by composition layer"
        );
    }
}

#[test]
fn type_ids_align_with_typetag_discriminants() {
    // Pins the mechanical-derivation invariant for every TypeTag variant:
    // `burden_type_id(tag)` equals the corresponding `TYPE_ID_<NAME>` AND
    // numerically equals `tag as u32 + 1`. Iterates the canonical
    // `TypeTag::all()` slice — adding a TypeTag variant forces this match
    // to grow a new arm (compiler-enforced exhaustiveness), eliminating
    // the parallel-list drift a hardcoded array would suffer.
    for tag in TypeTag::all() {
        let derived = burden_type_id(*tag);
        let expected_const = match tag {
            TypeTag::Int => TYPE_ID_INT,
            TypeTag::Float => TYPE_ID_FLOAT,
            TypeTag::Bool => TYPE_ID_BOOL,
            TypeTag::Char => TYPE_ID_CHAR,
            TypeTag::Byte => TYPE_ID_BYTE,
            TypeTag::Unit => TYPE_ID_UNIT,
            TypeTag::Never => TYPE_ID_NEVER,
            TypeTag::Duration => TYPE_ID_DURATION,
            TypeTag::Size => TYPE_ID_SIZE,
            TypeTag::Ordering => TYPE_ID_ORDERING,
            TypeTag::Str => TYPE_ID_STR,
            TypeTag::Error => TYPE_ID_ERROR,
            TypeTag::List => TYPE_ID_LIST,
            TypeTag::Map => TYPE_ID_MAP,
            TypeTag::Set => TYPE_ID_SET,
            TypeTag::Range => TYPE_ID_RANGE,
            TypeTag::Tuple => TYPE_ID_TUPLE,
            TypeTag::Option => TYPE_ID_OPTION,
            TypeTag::Result => TYPE_ID_RESULT,
            TypeTag::Channel => TYPE_ID_CHANNEL,
            TypeTag::Function => TYPE_ID_FUNCTION,
            TypeTag::Iterator => TYPE_ID_ITERATOR,
            TypeTag::DoubleEndedIterator => TYPE_ID_DOUBLE_ENDED_ITERATOR,
        };
        assert_eq!(
            derived, expected_const,
            "burden_type_id({tag:?}) != TYPE_ID_{tag:?}"
        );
        let expected_disc = match NonZeroU32::new((*tag as u32) + 1) {
            Some(n) => TypeId::new(n),
            None => unreachable!(),
        };
        assert_eq!(
            derived,
            expected_disc,
            "TypeTag::{tag:?} discriminant {disc} + 1 mismatch",
            disc = *tag as u32
        );
    }
}

/// Categorizes how a `TypeTag` participates in `BURDEN_TABLE`.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum TableClassification {
    /// Template entry shipped in `BURDEN_TABLE` (lookup returns `Some`).
    InTable,
    /// Monomorphized entries owned by the composition layer; not present in
    /// `BURDEN_TABLE` (lookup returns `None`).
    DeferredToComposition,
}

#[test]
fn typetag_classification_is_exhaustive_and_consistent() {
    // Pins the §01.2 vs composition-layer partition for every `TypeTag`
    // variant. Adding a new `TypeTag` forces an arm here (compiler-enforced
    // exhaustiveness) — the author must explicitly choose InTable or
    // DeferredToComposition, and the assertion below cross-checks that
    // choice against the actual `BURDEN_TABLE` state.
    for tag in TypeTag::all() {
        let classification = match tag {
            TypeTag::Int
            | TypeTag::Float
            | TypeTag::Bool
            | TypeTag::Char
            | TypeTag::Byte
            | TypeTag::Unit
            | TypeTag::Never
            | TypeTag::Duration
            | TypeTag::Size
            | TypeTag::Ordering
            | TypeTag::Str
            | TypeTag::List
            | TypeTag::Map
            | TypeTag::Set
            | TypeTag::Range
            | TypeTag::Option
            | TypeTag::Result => TableClassification::InTable,
            TypeTag::Error
            | TypeTag::Tuple
            | TypeTag::Channel
            | TypeTag::Function
            | TypeTag::Iterator
            | TypeTag::DoubleEndedIterator => TableClassification::DeferredToComposition,
        };
        let has_entry = BurdenRegistry::lookup_builtin(burden_type_id(*tag)).is_some();
        match classification {
            TableClassification::InTable => assert!(
                has_entry,
                "{tag:?} classified InTable but missing from BURDEN_TABLE"
            ),
            TableClassification::DeferredToComposition => assert!(
                !has_entry,
                "{tag:?} classified DeferredToComposition but present in BURDEN_TABLE"
            ),
        }
    }
}

// Const-context construction sanity (compile-time, not runtime).
const _CONST_ASSERT_TABLE_NONEMPTY: () = {
    assert!(!BURDEN_TABLE.is_empty());
};
