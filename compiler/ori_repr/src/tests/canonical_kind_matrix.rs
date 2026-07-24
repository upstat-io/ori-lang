use super::*;

#[test]
fn canonical_returns_none_for_borrowed() {
    use ori_types::{LifetimeId, Tag};

    let mut pool = Pool::new();
    let borrowed_idx = pool.borrowed(Idx::INT, LifetimeId::from_raw(1));
    assert_eq!(pool.tag(borrowed_idx), Tag::Borrowed);
    let mut cache = rustc_hash::FxHashMap::default();
    assert!(
        canonical_cached(&pool, borrowed_idx, &mut cache).is_none(),
        "Borrowed must return None, not panic"
    );
}

#[test]
fn canonical_returns_none_for_projection() {
    use ori_types::Tag;

    let mut pool = Pool::new();
    let proj_idx = pool.intern(Tag::Projection, 0);
    let mut cache = rustc_hash::FxHashMap::default();
    assert!(
        canonical_cached(&pool, proj_idx, &mut cache).is_none(),
        "Projection must return None, not panic"
    );
}

#[test]
fn canonical_returns_none_for_module_ns() {
    use ori_types::Tag;

    let mut pool = Pool::new();
    let ns_idx = pool.intern(Tag::ModuleNs, 0);
    let mut cache = rustc_hash::FxHashMap::default();
    assert!(
        canonical_cached(&pool, ns_idx, &mut cache).is_none(),
        "ModuleNs must return None, not panic"
    );
}

#[test]
fn canonical_repr_type_kind_matrix() {
    let mut pool = Pool::new();
    assert_primitive_canonical_reprs(&pool);
    assert_container_canonical_reprs(&mut pool);
    assert_complex_canonical_reprs(&mut pool);
    assert_non_codegen_reprs(&mut pool);
}

fn assert_primitive_canonical_reprs(pool: &Pool) {
    let cases = [
        (
            Idx::INT,
            MachineRepr::Int {
                width: IntWidth::I64,
                signed: true,
            },
            "Int canonical repr",
        ),
        (
            Idx::FLOAT,
            MachineRepr::Float {
                width: FloatWidth::F64,
            },
            "Float canonical repr",
        ),
        (Idx::BOOL, MachineRepr::Bool, "Bool canonical repr"),
        (
            Idx::STR,
            MachineRepr::FatPointer(FatRepr::Str),
            "Str canonical repr",
        ),
        (Idx::CHAR, MachineRepr::Char, "Char canonical repr"),
        (Idx::BYTE, MachineRepr::Byte, "Byte canonical repr"),
        (Idx::UNIT, MachineRepr::Unit, "Unit canonical repr"),
        (Idx::NEVER, MachineRepr::Never, "Never canonical repr"),
        (
            Idx::DURATION,
            MachineRepr::Duration,
            "Duration canonical repr",
        ),
        (Idx::SIZE, MachineRepr::Size, "Size canonical repr"),
        (
            Idx::ORDERING,
            MachineRepr::Ordering,
            "Ordering canonical repr",
        ),
    ];
    for (idx, expected, message) in cases {
        assert_eq!(canonical(pool, idx), expected, "{message}");
    }
    let mut cache = rustc_hash::FxHashMap::default();
    assert!(
        canonical_cached(pool, Idx::ERROR, &mut cache).is_none(),
        "Error has no canonical repr"
    );
}

fn assert_container_canonical_reprs(pool: &mut Pool) {
    use ori_types::LifetimeId;

    let list = pool.list(Idx::INT);
    assert!(matches!(
        canonical(pool, list),
        MachineRepr::FatPointer(FatRepr::Collection { .. })
    ));
    let option = pool.option(Idx::INT);
    assert!(matches!(canonical(pool, option), MachineRepr::Enum(_)));
    let set = pool.set(Idx::STR);
    assert!(matches!(
        canonical(pool, set),
        MachineRepr::FatPointer(FatRepr::Collection { .. })
    ));
    let channel = pool.channel(Idx::INT);
    assert_eq!(canonical(pool, channel), MachineRepr::OpaquePtr);
    let range = pool.range(Idx::INT);
    assert_eq!(canonical(pool, range), MachineRepr::Range);
    let iterator = pool.iterator(Idx::INT);
    assert_eq!(canonical(pool, iterator), MachineRepr::UnmanagedPtr);
    let double_ended = pool.double_ended_iterator(Idx::INT);
    assert_eq!(canonical(pool, double_ended), MachineRepr::UnmanagedPtr);
    let map = pool.map(Idx::STR, Idx::INT);
    assert!(matches!(
        canonical(pool, map),
        MachineRepr::FatPointer(FatRepr::Map { .. })
    ));
    let result = pool.result(Idx::INT, Idx::STR);
    assert!(matches!(canonical(pool, result), MachineRepr::Enum(_)));
    let borrowed = pool.borrowed(Idx::INT, LifetimeId::from_raw(1));
    let mut cache = rustc_hash::FxHashMap::default();
    assert!(canonical_cached(pool, borrowed, &mut cache).is_none());
}

fn assert_complex_canonical_reprs(pool: &mut Pool) {
    use ori_types::EnumVariant;

    let function = pool.function1(Idx::INT, Idx::BOOL);
    assert!(matches!(canonical(pool, function), MachineRepr::Closure(_)));
    let tuple = pool.pair(Idx::INT, Idx::BOOL);
    assert!(matches!(canonical(pool, tuple), MachineRepr::Tuple(_)));
    let struct_name = Name::new(0, 500);
    let struct_idx = pool.struct_type(struct_name, &[(Name::new(0, 501), Idx::INT)]);
    assert!(matches!(
        canonical(pool, struct_idx),
        MachineRepr::Struct(_)
    ));
    let enum_idx = pool.enum_type(
        Name::new(0, 600),
        &[
            EnumVariant {
                name: Name::new(0, 601),
                field_types: vec![],
            },
            EnumVariant {
                name: Name::new(0, 602),
                field_types: vec![Idx::INT],
            },
        ],
    );
    assert!(matches!(canonical(pool, enum_idx), MachineRepr::Enum(_)));

    let named = pool.named(Name::new(0, 700));
    pool.set_resolution(named, struct_idx);
    assert!(matches!(canonical(pool, named), MachineRepr::Struct(_)));
    let applied = pool.applied(Name::new(0, 800), &[Idx::INT]);
    pool.set_resolution(applied, Idx::INT);
    assert_eq!(
        canonical(pool, applied),
        MachineRepr::Int {
            width: IntWidth::I64,
            signed: true,
        }
    );
    let alias = pool.named(Name::new(0, 900));
    pool.set_resolution(alias, Idx::FLOAT);
    assert_eq!(
        canonical(pool, alias),
        MachineRepr::Float {
            width: FloatWidth::F64,
        }
    );
}

fn assert_non_codegen_reprs(pool: &mut Pool) {
    use ori_types::Tag;

    let mut indices = vec![pool.fresh_var()];
    indices.push(pool.intern(Tag::BoundVar, 0));
    indices.push(pool.rigid_var(Name::new(0, 999)));
    indices.push(pool.scheme(&[0], Idx::INT));
    indices.push(pool.intern(Tag::Projection, 0));
    indices.push(pool.intern(Tag::ModuleNs, 0));
    indices.push(pool.intern(Tag::Infer, 0));
    indices.push(pool.intern(Tag::SelfType, 0));

    let mut cache = rustc_hash::FxHashMap::default();
    for idx in indices {
        assert!(canonical_cached(pool, idx, &mut cache).is_none());
    }
}
