//! `lookup_burden` dispatch tests — builtin path (`&'static`), user path
//! (`TypeRegistry`-borrowed), miss paths (`None`), and `Burden` trait
//! parity across the two partition variants.

use ori_ir::Span;
use ori_registry::burden::table::{
    BurdenRegistry, TYPE_ID_BOOL, TYPE_ID_CHANNEL, TYPE_ID_INT, TYPE_ID_STR, TYPE_PARAM_T,
};
use ori_types::burden::{UserBurdenSpec, UserOwnedField};
use ori_types::burden_compose::compose_user_burden;
use ori_types::{FieldDef, Idx, Pool, TypeRegistry, Visibility};

use super::{idx_to_type_ref, lookup_burden};
use crate::lower::burden::{Burden, BurdenRef, TypeRef};
use crate::lower::test_utils::{registered_struct_with_burden, test_name};
use ori_registry::burden::table::burden_type_id;
use ori_registry::TypeTag;

fn lookup_required<'a>(ty: TypeRef, registry: &'a TypeRegistry, label: &'a str) -> BurdenRef<'a> {
    match lookup_burden(ty, registry) {
        Some(burden) => burden,
        None => panic!("expected burden lookup hit for {label}"),
    }
}

#[test]
fn lookup_builtin_int_returns_static_burden_ref() {
    let registry = TypeRegistry::new();
    let burden = lookup_required(TypeRef::Builtin(TYPE_ID_INT), &registry, "int");
    match burden {
        BurdenRef::Builtin(_) => {}
        BurdenRef::User(_) => panic!("Builtin lookup must return Builtin variant"),
    }
    // Scalar primitive — empty owned/borrowed/variant lists.
    assert!(
        burden.owned_fields().next().is_none(),
        "int has no owned fields",
    );
    assert!(burden.user_drop().is_none(), "int has no user drop",);
}

#[test]
fn lookup_builtin_str_returns_static_burden_ref() {
    let registry = TypeRegistry::new();
    let burden = lookup_required(TypeRef::Builtin(TYPE_ID_STR), &registry, "str");
    match burden {
        BurdenRef::Builtin(_) => {}
        BurdenRef::User(_) => panic!("str must dispatch through builtin path"),
    }
}

#[test]
fn lookup_builtin_bool_returns_static_burden_ref() {
    let registry = TypeRegistry::new();
    let result = lookup_burden(TypeRef::Builtin(TYPE_ID_BOOL), &registry);
    assert!(matches!(result, Some(BurdenRef::Builtin(_))));
}

#[test]
fn lookup_user_returns_registry_borrowed_burden_ref() {
    let mut registry = TypeRegistry::new();
    let user_idx = Idx::from_raw(1024);
    let user_spec = UserBurdenSpec {
        self_heap_alloc: true,
        owned_fields: vec![UserOwnedField {
            field_path: vec![0],
            field_type: Idx::STR,
        }],
        ..UserBurdenSpec::default()
    };
    registered_struct_with_burden(&mut registry, "Holder", user_idx, Some(user_spec));

    let burden = lookup_required(TypeRef::User(user_idx), &registry, "Holder user_idx");
    match burden {
        BurdenRef::User(spec) => {
            assert!(spec.self_heap_alloc);
            assert_eq!(spec.owned_fields.len(), 1);
            assert_eq!(spec.owned_fields[0].field_type, Idx::STR);
        }
        BurdenRef::Builtin(_) => panic!("User lookup must return User variant"),
    }
}

#[test]
fn lookup_user_unregistered_returns_none() {
    let registry = TypeRegistry::new();
    let unregistered = Idx::from_raw(9999);
    let result = lookup_burden(TypeRef::User(unregistered), &registry);
    assert!(
        result.is_none(),
        "unregistered user idx must lookup as None"
    );
}

#[test]
fn lookup_user_registered_without_burden_returns_none() {
    let mut registry = TypeRegistry::new();
    let user_idx = Idx::from_raw(2048);
    registered_struct_with_burden(&mut registry, "NoBurden", user_idx, None);

    let result = lookup_burden(TypeRef::User(user_idx), &registry);
    assert!(
        result.is_none(),
        "registered type with burden=None must lookup as None",
    );
}

#[test]
fn dispatch_parity_owned_fields_across_partition() {
    // Build a user spec structurally equivalent to a builtin's empty shape:
    // both yield an empty owned-fields iterator. Burden trait method dispatch
    // must produce equivalent observations regardless of partition side.
    let mut registry = TypeRegistry::new();
    let user_idx = Idx::from_raw(4096);
    let user_spec = UserBurdenSpec::default(); // empty across all dims
    registered_struct_with_burden(&mut registry, "EmptyUser", user_idx, Some(user_spec));

    let builtin = lookup_required(TypeRef::Builtin(TYPE_ID_INT), &registry, "int");
    let user = lookup_required(TypeRef::User(user_idx), &registry, "EmptyUser user_idx");

    let builtin_owned: Vec<_> = builtin.owned_fields().collect();
    let user_owned: Vec<_> = user.owned_fields().collect();
    assert_eq!(
        builtin_owned.len(),
        user_owned.len(),
        "parity: both partitions report zero owned fields for empty shape",
    );
    assert_eq!(
        builtin.self_heap_alloc(),
        user.self_heap_alloc(),
        "parity: empty shapes agree on self_heap_alloc",
    );
    assert_eq!(
        builtin.user_drop(),
        user.user_drop(),
        "parity: empty shapes agree on user_drop",
    );
}

#[test]
fn dispatch_parity_borrowed_fields_zero_arity() {
    let mut registry = TypeRegistry::new();
    let user_idx = Idx::from_raw(8192);
    let user_spec = UserBurdenSpec::default();
    registered_struct_with_burden(&mut registry, "EmptyU2", user_idx, Some(user_spec));

    let builtin = lookup_required(TypeRef::Builtin(TYPE_ID_INT), &registry, "int");
    let user = lookup_required(TypeRef::User(user_idx), &registry, "EmptyU2 user_idx");

    assert!(
        builtin.borrowed_fields().next().is_none(),
        "builtin int has zero borrowed fields",
    );
    assert!(
        user.borrowed_fields().next().is_none(),
        "user empty spec has zero borrowed fields",
    );
}

#[test]
fn lookup_user_idx_partition_does_not_query_builtin_table() {
    // A user Idx whose raw value happens to match a builtin TypeId's raw value
    // MUST NOT route through BurdenRegistry::lookup_builtin. The partition is
    // the TypeRef variant, not the raw value.
    let mut registry = TypeRegistry::new();
    // Idx::from_raw(0) is Idx::INT — same raw value as TYPE_ID_INT's discriminant.
    // The TypeRef::User variant ensures we hit TypeRegistry, not BURDEN_TABLE.
    let collision_idx = Idx::INT;
    registered_struct_with_burden(&mut registry, "CollisionRaw", collision_idx, None);

    let result = lookup_burden(TypeRef::User(collision_idx), &registry);
    assert!(
        result.is_none(),
        "User-tagged collision_idx with burden=None routes through TypeRegistry, not BURDEN_TABLE",
    );

    // Same raw value, Builtin-tagged: routes through BURDEN_TABLE, finds int.
    let builtin_result = lookup_burden(TypeRef::Builtin(TYPE_ID_INT), &registry);
    assert!(matches!(builtin_result, Some(BurdenRef::Builtin(_))));
}

// FFI exclusion contract — structural tests.
// Spec: Annex E §FFI.

#[test]
fn unannotated_ffi_empty_burden_yields_zero_owned_fields_and_no_user_drop() {
    // Test 1 (STRUCTURAL): unannotated opaque FFI types are represented by
    // the empty BuiltinBurdenSpec — semantically `CPtr` / `JsValue` /
    // `JsPromise<T>` / `extern "c"` types without `#free`. The empty spec
    // reports zero owned fields, zero borrowed fields, zero variant
    // burdens, no user drop. Spec: Annex E §AIMS RL-31 Sufficient-Noalias
    // Rule clause 8 — empty `BurdenSpec` ≠ memory burden ≠ noalias
    // eligible.
    let burden_ref = BurdenRef::Builtin(&ori_registry::burden::EMPTY_BURDEN_SPEC);
    assert!(
        burden_ref.owned_fields().next().is_none(),
        "empty BuiltinBurdenSpec must have zero owned fields",
    );
    assert!(
        burden_ref.borrowed_fields().next().is_none(),
        "empty BuiltinBurdenSpec must have zero borrowed fields",
    );
    assert!(
        burden_ref.variant_burdens().next().is_none(),
        "empty BuiltinBurdenSpec must have zero variant burdens",
    );
    assert!(
        burden_ref.user_drop().is_none(),
        "empty BuiltinBurdenSpec must have no user drop",
    );
    assert!(
        burden_ref.compiled_drop().is_none(),
        "empty BuiltinBurdenSpec must have no compiled drop",
    );
    assert!(
        !burden_ref.self_heap_alloc(),
        "empty BuiltinBurdenSpec must not declare self heap allocation",
    );
    assert!(
        burden_ref.element_burden().is_none(),
        "empty BuiltinBurdenSpec must have no element burden",
    );
}

// ───.B Channel<T> drop-glue reachability via wrapper ────────
//
// These tests exercise the END-TO-END drop-glue pathway for `Channel<T>`:
// the BURDEN_TABLE template is composed via `compose_user_burden` at
// monomorphization, registered against a monomorphized `Idx` via
// `TypeRegistry::register_user_burden`, and looked up via the
// wrapper surface (`lookup_burden`). The walk MUST reveal a path to T's
// burden when T has one (e.g., `Channel<str>` → str's heap allocation);
// no path when T is empty-burden (e.g., `Channel<int>`).
//
// The wrapper-walk completeness pin proves the `element_burden` slot is
// load-bearing for drop-glue: stripping it collapses the reachability
// path, breaking drop emission for buffered T elements.

fn channel_template() -> &'static ori_registry::burden::BuiltinBurdenSpec {
    match BurdenRegistry::lookup_builtin(TYPE_ID_CHANNEL) {
        Some(spec) => spec,
        None => {
            panic!("Channel<T> template missing from BURDEN_TABLE —.B template regression")
        }
    }
}

fn register_user_struct_slot(registry: &mut TypeRegistry, name: &str, idx: Idx) {
    let fields = vec![FieldDef {
        name: test_name("payload"),
        ty: Idx::INT,
        span: Span::DUMMY,
        visibility: Visibility::Public,
    }];
    registry.register_struct(
        test_name(name),
        idx,
        vec![],
        fields,
        Span::DUMMY,
        Visibility::Public,
        0,
        None,
        None,
    );
}

#[test]
fn channel_builtin_template_lookup_returns_static_burden_ref() {
    // Positive: TypeRef::Builtin(TYPE_ID_CHANNEL) routes through
    // `BurdenRegistry::lookup_builtin`, returning a `&'static
    // BuiltinBurdenSpec` matching the.B template. The template's
    // `element_burden` is the TYPE_PARAM_T placeholder — composition
    // substitutes it to a concrete user `Idx` before drop-glue lookup.
    let registry = TypeRegistry::new();
    let result = lookup_burden(TypeRef::Builtin(TYPE_ID_CHANNEL), &registry);
    match result {
        Some(BurdenRef::Builtin(spec)) => {
            assert!(
                spec.self_heap_alloc,
                "Channel<T> template advertises heap allocation"
            );
            assert_eq!(
                spec.element_burden,
                Some(TYPE_PARAM_T),
                "Channel<T> template exposes the type-param placeholder"
            );
        }
        Some(BurdenRef::User(_)) => panic!("Channel TYPE_ID must route through Builtin partition"),
        None => panic!("Channel TYPE_ID has a BURDEN_TABLE entry; lookup must hit"),
    }
}

#[test]
fn channel_str_composed_spec_walked_via_wrapper_reveals_str_burden_path() {
    // Positive (drop-glue reachability):
    // composing `Channel<str>` via the mechanism, registering it,
    // and walking the resulting `UserBurdenSpec` via the wrapper
    // (`lookup_burden`) reveals a path from the Channel handle to T's
    // burden via `element_burden = Some(Idx::STR)`. When drop-glue
    // emission walks the spec, it sees Idx::STR and emits the appropriate
    // RcDec on each buffered str element.
    let pool = Pool::new();
    let mut registry = TypeRegistry::new();
    let channel_str_idx = Idx::from_raw(9001);
    register_user_struct_slot(&mut registry, "ChannelStr", channel_str_idx);

    let composed = compose_user_burden(channel_template(), &[Idx::STR], &pool, &registry);
    registry.register_user_burden(channel_str_idx, composed);

    let Some(burden) = lookup_burden(TypeRef::User(channel_str_idx), &registry) else {
        panic!("Channel<str> spec MUST be looked up via TypeRef::User after registration");
    };

    // The wrapper walk surfaces self_heap_alloc + element_burden.
    assert!(
        burden.self_heap_alloc(),
        "Channel<str> handle is heap-allocated"
    );
    let Some(element) = burden.element_burden() else {
        panic!("Channel<str>: element_burden carries Idx::STR after composition");
    };
    match element {
        TypeRef::User(idx) => assert_eq!(
            idx,
            Idx::STR,
            "Channel<str>'s element_burden points at Idx::STR"
        ),
        TypeRef::Builtin(_) => panic!(
            "composed Channel<str>'s element_burden routes through User partition (composition stamps the concrete pool Idx into UserBurdenSpec)"
        ),
    }
}

#[test]
fn channel_int_composed_spec_walked_via_wrapper_reveals_int_burden_path() {
    // Positive: Channel<int> composes with `element_burden = Some(Idx::INT)`.
    // The element points at `int` (empty primitive burden) — drop-glue
    // walks the path but emits no RcDec since int has no heap allocation.
    // The path itself MUST be present; absence would defeat reachability.
    let pool = Pool::new();
    let mut registry = TypeRegistry::new();
    let channel_int_idx = Idx::from_raw(9002);
    register_user_struct_slot(&mut registry, "ChannelInt", channel_int_idx);

    let composed = compose_user_burden(channel_template(), &[Idx::INT], &pool, &registry);
    registry.register_user_burden(channel_int_idx, composed);

    let Some(burden) = lookup_burden(TypeRef::User(channel_int_idx), &registry) else {
        panic!("Channel<int> spec MUST be looked up via TypeRef::User after registration");
    };

    let Some(element) = burden.element_burden() else {
        panic!("Channel<int>: element_burden present even when T has empty burden");
    };
    match element {
        TypeRef::User(idx) => assert_eq!(
            idx,
            Idx::INT,
            "Channel<int>'s element_burden points at Idx::INT"
        ),
        TypeRef::Builtin(_) => panic!(
            "composed Channel<int>'s element_burden routes through User partition (composition stamps Idx::INT into UserBurdenSpec)"
        ),
    }
}

#[test]
fn negative_pin_channel_without_element_burden_loses_drop_glue_reachability() {
    // Negative pin (wrapper-walk completeness): a regression that
    // accidentally drops the `element_burden` slot from the Channel<T>
    // composition produces a `UserBurdenSpec` whose walk returns NO T
    // burden node — drop-glue cannot reach buffered T elements when the
    // refcount reaches zero. This test models the regression by
    // constructing the broken spec directly and asserting the wrapper
    // walk returns None for `element_burden`.
    let mut registry = TypeRegistry::new();
    let broken_idx = Idx::from_raw(9003);
    register_user_struct_slot(&mut registry, "BrokenChannel", broken_idx);

    // Broken spec: self_heap_alloc preserved but element_burden missing.
    let broken_spec = UserBurdenSpec {
        self_heap_alloc: true,
        owned_fields: vec![],
        borrowed_fields: vec![],
        variant_burdens: vec![],
        element_burden: None,
        compiled_drop: None,
        user_drop: None,
    };
    registry.register_user_burden(broken_idx, broken_spec);

    let Some(burden) = lookup_burden(TypeRef::User(broken_idx), &registry) else {
        panic!("regression spec MUST still be looked up via TypeRef::User");
    };

    assert!(
        burden.element_burden().is_none(),
        "without element_burden the wrapper walk has no path to T's burden — drop-glue regression"
    );
    // The handle's own heap allocation is still tracked; only T's
    // reachability is lost.
    assert!(burden.self_heap_alloc());
}

#[test]
fn semantic_pin_channel_str_distinguishable_from_channel_int_via_wrapper() {
    // Semantic pin: the wrapper walk produces structurally DISTINCT
    // observations for Channel<str> and Channel<int>. Catches a
    // regression where Channel composition would accidentally collapse
    // T into an opaque sentinel that hides type identity from drop-glue
    // emission.
    let pool = Pool::new();
    let mut registry = TypeRegistry::new();
    let chan_str_idx = Idx::from_raw(9010);
    let chan_int_idx = Idx::from_raw(9011);
    register_user_struct_slot(&mut registry, "ChanStr", chan_str_idx);
    register_user_struct_slot(&mut registry, "ChanInt", chan_int_idx);

    let str_spec = compose_user_burden(channel_template(), &[Idx::STR], &pool, &registry);
    let int_spec = compose_user_burden(channel_template(), &[Idx::INT], &pool, &registry);
    registry.register_user_burden(chan_str_idx, str_spec);
    registry.register_user_burden(chan_int_idx, int_spec);

    let Some(str_burden) = lookup_burden(TypeRef::User(chan_str_idx), &registry) else {
        panic!("Channel<str> spec missing");
    };
    let Some(int_burden) = lookup_burden(TypeRef::User(chan_int_idx), &registry) else {
        panic!("Channel<int> spec missing");
    };

    let str_elem = match str_burden.element_burden() {
        Some(TypeRef::User(idx)) => idx,
        Some(TypeRef::Builtin(_)) => panic!("Channel<str> element routes through User"),
        None => panic!("Channel<str> element_burden must be Some"),
    };
    let int_elem = match int_burden.element_burden() {
        Some(TypeRef::User(idx)) => idx,
        Some(TypeRef::Builtin(_)) => panic!("Channel<int> element routes through User"),
        None => panic!("Channel<int> element_burden must be Some"),
    };
    assert_ne!(
        str_elem, int_elem,
        "wrapper walk of Channel<str> and Channel<int> yields distinct element TypeIds"
    );
}

#[test]
fn annotated_ffi_extern_type_user_drop_carries_free_symbol() {
    // Test 2 (STRUCTURAL): an extern type declared with `#free(symbol)`
    // gets an explicit UserBurdenSpec with user_drop = Some(FnSym) and
    // empty field/variant lists. Spec: Annex E §FFI — Owned-positioned
    // extern values drop via the named free function.
    //
    // The canonical syntax target is the literal form
    // `extern "c" from "libsqlite" #free(sqlite3_close) { type DbHandle }`;
    // extern-type AST declarations remain target-only per the module-doc
    // Status section. This test pins the burden shape directly via
    // `compute_extern_type_burden` — the building block downstream.
    use core::num::NonZeroU32;
    use ori_registry::burden::FnSym;

    let symbol_raw = 17u32;
    let Some(nz) = NonZeroU32::new(symbol_raw) else {
        panic!("symbol raw must be nonzero (literal 17)");
    };
    let expected_fn_sym = FnSym::new(nz);

    let user_spec = UserBurdenSpec {
        self_heap_alloc: false,
        owned_fields: vec![],
        borrowed_fields: vec![],
        variant_burdens: vec![],
        element_burden: None,
        compiled_drop: None,
        user_drop: Some(expected_fn_sym),
    };

    let mut registry = TypeRegistry::new();
    let extern_type_idx = Idx::from_raw(16384);
    registered_struct_with_burden(&mut registry, "DbHandle", extern_type_idx, Some(user_spec));

    let burden_ref = lookup_required(
        TypeRef::User(extern_type_idx),
        &registry,
        "DbHandle extern type",
    );
    match burden_ref {
        BurdenRef::User(spec) => {
            assert_eq!(
                spec.user_drop,
                Some(expected_fn_sym),
                "annotated extern type must carry user_drop = Some(FnSym)",
            );
            assert!(
                spec.owned_fields.is_empty(),
                "annotated extern type opaque payload has no owned fields",
            );
            assert!(
                spec.borrowed_fields.is_empty(),
                "annotated extern type opaque payload has no borrowed fields",
            );
            assert!(
                spec.variant_burdens.is_empty(),
                "annotated extern type opaque payload has no variant burdens",
            );
        }
        BurdenRef::Builtin(_) => {
            panic!("annotated extern type must dispatch through User partition");
        }
    }
}

// `idx_to_type_ref` classifier tests — pin the `Idx → TypeTag → burden_type_id`
// translation chain. Idx and TypeTag have DIFFERENT orderings (e.g., Idx::STR=3
// vs TypeTag::Str discriminant=10) — naive raw-value punning would silently
// dispatch the wrong burden entry.

#[test]
fn idx_to_type_ref_int_dispatches_builtin() {
    let registry = TypeRegistry::new();
    let ty = idx_to_type_ref(Idx::INT, &registry);
    assert_eq!(
        ty,
        TypeRef::Builtin(burden_type_id(TypeTag::Int)),
        "Idx::INT (raw 0) maps to TypeTag::Int (discriminant 0)",
    );
}

#[test]
fn idx_to_type_ref_str_dispatches_builtin_off_ordering() {
    // Idx::STR is at raw 3; TypeTag::Str is at discriminant 10. Naive raw
    // pass-through would mis-dispatch (pick TypeTag::Char's burden); explicit
    // match must translate correctly.
    let registry = TypeRegistry::new();
    let ty = idx_to_type_ref(Idx::STR, &registry);
    assert_eq!(
        ty,
        TypeRef::Builtin(burden_type_id(TypeTag::Str)),
        "Idx::STR (raw 3) MUST map to TypeTag::Str (discriminant 10), not Char",
    );
}

#[test]
fn idx_to_type_ref_never_dispatches_builtin() {
    let registry = TypeRegistry::new();
    let ty = idx_to_type_ref(Idx::NEVER, &registry);
    assert_eq!(
        ty,
        TypeRef::Builtin(burden_type_id(TypeTag::Never)),
        "Idx::NEVER (raw 7) maps to TypeTag::Never (discriminant 6)",
    );
}

#[test]
fn idx_to_type_ref_dynamic_dispatches_user() {
    // Negative pin: any Idx >= FIRST_DYNAMIC (64) MUST go through User
    // partition, not Builtin.
    let registry = TypeRegistry::new();
    let dynamic_idx = Idx::from_raw(1024);
    let ty = idx_to_type_ref(dynamic_idx, &registry);
    assert_eq!(
        ty,
        TypeRef::User(dynamic_idx),
        "dynamic Idx (>= FIRST_DYNAMIC) MUST route through User partition",
    );
}

#[test]
fn idx_to_type_ref_error_dispatches_user() {
    // Idx::ERROR (raw 8) is poison per -3; no burden entry in
    // BURDEN_TABLE. Must route to User (where TypeRegistry::burden returns
    // None for the unregistered ERROR slot).
    let registry = TypeRegistry::new();
    let ty = idx_to_type_ref(Idx::ERROR, &registry);
    assert_eq!(
        ty,
        TypeRef::User(Idx::ERROR),
        "Idx::ERROR (raw 8, poison) MUST route through User (no BURDEN_TABLE entry)",
    );
}
