use super::compose_burden_for_idx;
use crate::{Idx, Pool, TypeRegistry};

/// A tuple with a heap element composes the anonymous-struct burden: the
/// str slot is an owned field (positional path), the scalar slot omitted.
#[test]
fn tuple_with_heap_element_composes_owned_field_burden() {
    let mut pool = Pool::new();
    let tuple = pool.tuple(&[Idx::STR, Idx::INT]);
    let registry = TypeRegistry::new();
    let Some(spec) = compose_burden_for_idx(&pool, &registry, tuple) else {
        panic!("a (str, int) tuple must compose an owned-field burden");
    };
    assert_eq!(spec.owned_fields.len(), 1, "one owned field: the str slot");
    assert_eq!(
        spec.owned_fields[0].field_path,
        vec![0],
        "the owned field is positional slot 0"
    );
    assert_eq!(spec.owned_fields[0].field_type, Idx::STR);
}

/// An all-scalar tuple carries no burden (no owned fields to release).
#[test]
fn all_scalar_tuple_composes_no_burden() {
    let mut pool = Pool::new();
    let tuple = pool.tuple(&[Idx::INT, Idx::BOOL]);
    let registry = TypeRegistry::new();
    assert!(
        compose_burden_for_idx(&pool, &registry, tuple).is_none(),
        "an (int, bool) tuple owes no release"
    );
}
