use ori_ir::Name;

use super::*;

fn unwrap_burden(burden: Option<UserBurdenSpec>) -> UserBurdenSpec {
    match burden {
        Some(burden) => burden,
        None => panic!("expected Some(UserBurdenSpec)"),
    }
}

fn unwrap_fn_sym(user_drop: Option<FnSym>) -> FnSym {
    match user_drop {
        Some(symbol) => symbol,
        None => panic!("expected user_drop = Some(FnSym)"),
    }
}

#[test]
fn extern_burden_with_free_fn_carries_user_drop() {
    let raw = 42u32;
    let name = Name::from_raw(raw);
    let burden = unwrap_burden(compute_extern_type_burden(Some(name)));
    assert!(!burden.self_owned_identity);
    assert!(burden.owned_fields.is_empty());
    assert!(burden.borrowed_fields.is_empty());
    assert!(burden.variant_burdens.is_empty());
    assert!(burden.element_burden.is_none());
    assert!(burden.drop_operation.is_none());
    let fn_sym = unwrap_fn_sym(burden.user_drop);
    assert_eq!(fn_sym.get().get(), raw);
}

#[test]
fn extern_burden_without_free_fn_returns_none() {
    let burden = compute_extern_type_burden(None);
    assert!(burden.is_none());
}
