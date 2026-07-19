use ori_ir::{DerivedImplId, DerivedTrait, Span};

use super::*;

fn accepted_derive(id: u32) -> crate::AcceptedDerivedImpl {
    crate::AcceptedDerivedImpl {
        id: DerivedImplId::new(id),
        owner_name: Name::from_raw(1),
        owner_type: Idx::INT,
        trait_type: Idx::INT,
        trait_kind: DerivedTrait::Eq,
        method_name: Name::from_raw(2),
        signature: crate::FunctionSig::simple(Name::from_raw(2), vec![Idx::INT], Idx::BOOL),
        span: Span::DUMMY,
    }
}

#[test]
#[should_panic(expected = "accepted derived implementation identities must be unique")]
fn duplicate_accepted_derive_id_panics_during_finish_validation() {
    sort_and_validate_accepted_derives(vec![accepted_derive(0), accepted_derive(0)]);
}

#[test]
fn index_dispatch_error_and_deferred_observations_refine_to_selected() {
    let expr = ori_ir::ExprId::new(3);
    let producer = crate::MethodProducer::Impl(crate::ImplMethodId::new(1, expr));
    let (producers, dispatch) = normalize_index_dispatch(vec![
        (expr, crate::IndexDispatchSelection::Error),
        (expr, crate::IndexDispatchSelection::Deferred),
        (
            expr,
            crate::IndexDispatchSelection::Selected(producer.clone()),
        ),
        (expr, crate::IndexDispatchSelection::Error),
    ]);

    assert_eq!(producers, vec![producer]);
    assert_eq!(
        dispatch,
        vec![(
            expr,
            ori_ir::canon::IndexDispatch::Selected(crate::MethodProducerId::new(0))
        )]
    );
}

#[test]
#[should_panic(expected = "one index expression cannot select two semantic dispatch routes")]
fn index_dispatch_conflicting_concrete_routes_panic() {
    let expr = ori_ir::ExprId::new(3);
    let _ = normalize_index_dispatch(vec![
        (expr, crate::IndexDispatchSelection::Builtin),
        (expr, crate::IndexDispatchSelection::Deferred),
    ]);
}
