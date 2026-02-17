use super::*;
use ori_ir::{ExprId, Name, Span};

fn test_name(s: &str) -> Name {
    Name::from_raw(
        s.as_bytes()
            .iter()
            .fold(0u32, |acc, &b| acc.wrapping_add(u32::from(b))),
    )
}

fn test_span() -> Span {
    Span::DUMMY
}

fn test_expr() -> ExprId {
    ExprId::new(0)
}

#[test]
fn register_and_lookup_trait() {
    let mut registry = TraitRegistry::new();

    let name = test_name("Display");
    let idx = Idx::from_raw(200);

    let mut methods = FxHashMap::default();
    methods.insert(
        test_name("fmt"),
        TraitMethodDef {
            name: test_name("fmt"),
            signature: Idx::from_raw(300),
            has_default: false,
            default_body: None,
            span: test_span(),
        },
    );

    registry.register_trait(TraitEntry {
        name,
        idx,
        type_params: vec![],
        super_traits: vec![],
        methods,
        assoc_types: FxHashMap::default(),
        object_safety_violations: vec![],
        span: test_span(),
    });

    // Lookup by name
    let entry = registry.get_trait_by_name(name).expect("should find trait");
    assert_eq!(entry.name, name);
    assert_eq!(entry.idx, idx);

    // Lookup by idx
    let entry = registry.get_trait_by_idx(idx).expect("should find trait");
    assert_eq!(entry.name, name);

    // Lookup method
    let method = registry
        .trait_method(idx, test_name("fmt"))
        .expect("should find method");
    assert!(!method.has_default);
}

#[test]
fn register_and_lookup_impl() {
    let mut registry = TraitRegistry::new();

    // Register a trait first
    let trait_name = test_name("Show");
    let trait_idx = Idx::from_raw(200);

    registry.register_trait(TraitEntry {
        name: trait_name,
        idx: trait_idx,
        type_params: vec![],
        super_traits: vec![],
        methods: FxHashMap::default(),
        assoc_types: FxHashMap::default(),
        object_safety_violations: vec![],
        span: test_span(),
    });

    // Register an impl
    let self_type = Idx::INT;
    let mut methods = FxHashMap::default();
    methods.insert(
        test_name("show"),
        ImplMethodDef {
            name: test_name("show"),
            signature: Idx::from_raw(301),
            has_self: true,
            body: test_expr(),
            span: test_span(),
        },
    );

    let impl_idx = registry.register_impl(ImplEntry {
        trait_idx: Some(trait_idx),
        self_type,
        type_params: vec![],
        methods,
        assoc_types: FxHashMap::default(),
        where_clause: vec![],
        specificity: ImplSpecificity::Concrete,
        span: test_span(),
    });

    assert_eq!(impl_idx, 0);

    // Find impl by type
    let impls: Vec<_> = registry.impls_for_type(self_type).collect();
    assert_eq!(impls.len(), 1);
    assert_eq!(impls[0].trait_idx, Some(trait_idx));

    // Find impl by trait
    let impls: Vec<_> = registry.impls_of_trait(trait_idx).collect();
    assert_eq!(impls.len(), 1);
    assert_eq!(impls[0].self_type, self_type);

    // Find specific impl
    let (idx, entry) = registry
        .find_impl(trait_idx, self_type)
        .expect("should find impl");
    assert_eq!(idx, 0);
    assert!(entry.methods.contains_key(&test_name("show")));
}

#[test]
fn inherent_impl() {
    let mut registry = TraitRegistry::new();

    let self_type = Idx::from_raw(100);
    let mut methods = FxHashMap::default();
    methods.insert(
        test_name("len"),
        ImplMethodDef {
            name: test_name("len"),
            signature: Idx::from_raw(400),
            has_self: true,
            body: test_expr(),
            span: test_span(),
        },
    );

    registry.register_impl(ImplEntry {
        trait_idx: None, // Inherent impl
        self_type,
        type_params: vec![],
        methods,
        assoc_types: FxHashMap::default(),
        where_clause: vec![],
        specificity: ImplSpecificity::Concrete,
        span: test_span(),
    });

    // Find inherent impl
    let (impl_idx, entry) = registry
        .inherent_impl(self_type)
        .expect("should find inherent impl");
    assert_eq!(impl_idx, 0);
    assert!(entry.trait_idx.is_none());

    // Check has_inherent_impl
    assert!(registry.has_inherent_impl(self_type));
    assert!(!registry.has_inherent_impl(Idx::INT));
}

#[test]
fn method_lookup_priority() {
    let mut registry = TraitRegistry::new();

    let self_type = Idx::from_raw(100);
    let trait_idx = Idx::from_raw(200);
    let method_name = test_name("foo");

    // Register trait
    registry.register_trait(TraitEntry {
        name: test_name("Trait"),
        idx: trait_idx,
        type_params: vec![],
        super_traits: vec![],
        methods: FxHashMap::default(),
        assoc_types: FxHashMap::default(),
        object_safety_violations: vec![],
        span: test_span(),
    });

    // Register inherent impl with method "foo"
    let mut inherent_methods = FxHashMap::default();
    inherent_methods.insert(
        method_name,
        ImplMethodDef {
            name: method_name,
            signature: Idx::from_raw(300),
            has_self: true,
            body: test_expr(),
            span: test_span(),
        },
    );

    registry.register_impl(ImplEntry {
        trait_idx: None,
        self_type,
        type_params: vec![],
        methods: inherent_methods,
        assoc_types: FxHashMap::default(),
        where_clause: vec![],
        specificity: ImplSpecificity::Concrete,
        span: test_span(),
    });

    // Register trait impl with same method "foo"
    let mut trait_methods = FxHashMap::default();
    trait_methods.insert(
        method_name,
        ImplMethodDef {
            name: method_name,
            signature: Idx::from_raw(400),
            has_self: true,
            body: test_expr(),
            span: test_span(),
        },
    );

    registry.register_impl(ImplEntry {
        trait_idx: Some(trait_idx),
        self_type,
        type_params: vec![],
        methods: trait_methods,
        assoc_types: FxHashMap::default(),
        where_clause: vec![],
        specificity: ImplSpecificity::Concrete,
        span: test_span(),
    });

    // Lookup should find inherent method first
    let lookup = registry
        .lookup_method(self_type, method_name)
        .expect("should find method");
    assert!(lookup.is_inherent());
    assert_eq!(lookup.method().signature, Idx::from_raw(300));
}

#[test]
fn coherence_check() {
    let mut registry = TraitRegistry::new();

    let trait_idx = Idx::from_raw(200);
    let self_type = Idx::INT;

    registry.register_trait(TraitEntry {
        name: test_name("Trait"),
        idx: trait_idx,
        type_params: vec![],
        super_traits: vec![],
        methods: FxHashMap::default(),
        assoc_types: FxHashMap::default(),
        object_safety_violations: vec![],
        span: test_span(),
    });

    // No impl yet
    assert!(!registry.has_impl(trait_idx, self_type));

    // Register impl
    registry.register_impl(ImplEntry {
        trait_idx: Some(trait_idx),
        self_type,
        type_params: vec![],
        methods: FxHashMap::default(),
        assoc_types: FxHashMap::default(),
        where_clause: vec![],
        specificity: ImplSpecificity::Concrete,
        span: test_span(),
    });

    // Now has impl
    assert!(registry.has_impl(trait_idx, self_type));
}

#[test]
fn associated_types() {
    let mut registry = TraitRegistry::new();

    let trait_name = test_name("Iterator");
    let trait_idx = Idx::from_raw(200);
    let item_name = test_name("Item");

    let mut assoc_types = FxHashMap::default();
    assoc_types.insert(
        item_name,
        TraitAssocTypeDef {
            name: item_name,
            bounds: vec![],
            default: None,
            span: test_span(),
        },
    );

    registry.register_trait(TraitEntry {
        name: trait_name,
        idx: trait_idx,
        type_params: vec![],
        super_traits: vec![],
        methods: FxHashMap::default(),
        assoc_types,
        object_safety_violations: vec![],
        span: test_span(),
    });

    // Lookup associated type
    let assoc = registry
        .trait_assoc_type(trait_idx, item_name)
        .expect("should find assoc type");
    assert_eq!(assoc.name, item_name);
    assert!(assoc.default.is_none());
}

// =============================================================================
// Super-trait tracking
// =============================================================================

/// Helper to register a minimal trait with the given super-traits.
fn register_simple_trait(
    registry: &mut TraitRegistry,
    name: &str,
    idx: Idx,
    super_traits: Vec<Idx>,
    methods: FxHashMap<Name, TraitMethodDef>,
) {
    registry.register_trait(TraitEntry {
        name: test_name(name),
        idx,
        type_params: vec![],
        super_traits,
        methods,
        assoc_types: FxHashMap::default(),
        object_safety_violations: vec![],
        span: test_span(),
    });
}

#[test]
fn all_super_traits_linear_chain() {
    let mut registry = TraitRegistry::new();

    // A (no parents) -> B: A -> C: B
    let a_idx = Idx::from_raw(100);
    let b_idx = Idx::from_raw(101);
    let c_idx = Idx::from_raw(102);

    register_simple_trait(&mut registry, "A", a_idx, vec![], FxHashMap::default());
    register_simple_trait(&mut registry, "B", b_idx, vec![a_idx], FxHashMap::default());
    register_simple_trait(&mut registry, "C", c_idx, vec![b_idx], FxHashMap::default());

    // C's all_super_traits should be [B, A]
    let supers = registry.all_super_traits(c_idx);
    assert_eq!(supers.len(), 2);
    assert!(supers.contains(&b_idx));
    assert!(supers.contains(&a_idx));

    // B's all_super_traits should be [A]
    let supers = registry.all_super_traits(b_idx);
    assert_eq!(supers, vec![a_idx]);

    // A has no super-traits
    assert!(registry.all_super_traits(a_idx).is_empty());
}

#[test]
fn all_super_traits_diamond() {
    let mut registry = TraitRegistry::new();

    //     A
    //    / \
    //   B   C
    //    \ /
    //     D
    let a_idx = Idx::from_raw(100);
    let b_idx = Idx::from_raw(101);
    let c_idx = Idx::from_raw(102);
    let d_idx = Idx::from_raw(103);

    register_simple_trait(&mut registry, "A", a_idx, vec![], FxHashMap::default());
    register_simple_trait(&mut registry, "B", b_idx, vec![a_idx], FxHashMap::default());
    register_simple_trait(&mut registry, "C", c_idx, vec![a_idx], FxHashMap::default());
    register_simple_trait(
        &mut registry,
        "D",
        d_idx,
        vec![b_idx, c_idx],
        FxHashMap::default(),
    );

    // D should have B, C, A (deduplicated — A appears only once)
    let supers = registry.all_super_traits(d_idx);
    assert_eq!(supers.len(), 3);
    assert!(supers.contains(&b_idx));
    assert!(supers.contains(&c_idx));
    assert!(supers.contains(&a_idx));
}

#[test]
fn collected_methods_deduplication() {
    let mut registry = TraitRegistry::new();

    // A has method "foo"
    let a_idx = Idx::from_raw(100);
    let b_idx = Idx::from_raw(101);
    let foo_name = test_name("foo");

    let mut a_methods = FxHashMap::default();
    a_methods.insert(
        foo_name,
        TraitMethodDef {
            name: foo_name,
            signature: Idx::from_raw(300),
            has_default: true,
            default_body: Some(test_expr()),
            span: test_span(),
        },
    );

    register_simple_trait(&mut registry, "A", a_idx, vec![], a_methods);

    // B: A also has "foo" (override) and "bar"
    let bar_name = test_name("bar");
    let mut b_methods = FxHashMap::default();
    b_methods.insert(
        foo_name,
        TraitMethodDef {
            name: foo_name,
            signature: Idx::from_raw(400),
            has_default: true,
            default_body: Some(test_expr()),
            span: test_span(),
        },
    );
    b_methods.insert(
        bar_name,
        TraitMethodDef {
            name: bar_name,
            signature: Idx::from_raw(401),
            has_default: false,
            default_body: None,
            span: test_span(),
        },
    );

    register_simple_trait(&mut registry, "B", b_idx, vec![a_idx], b_methods);

    // B's collected_methods: foo from B (override), bar from B, NOT foo from A
    let methods = registry.collected_methods(b_idx);
    assert_eq!(methods.len(), 2);

    let foo_entry = methods
        .iter()
        .find(|(name, _, _)| *name == foo_name)
        .expect("foo method should exist in collected methods");
    // foo should come from B, not A
    assert_eq!(foo_entry.1, b_idx);
    assert_eq!(foo_entry.2.signature, Idx::from_raw(400));

    let bar_entry = methods
        .iter()
        .find(|(name, _, _)| *name == bar_name)
        .expect("bar method should exist in collected methods");
    assert_eq!(bar_entry.1, b_idx);
}
