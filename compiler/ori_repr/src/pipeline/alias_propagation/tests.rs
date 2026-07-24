use ori_ir::Name;

use crate::plan::NarrowingPolicy;
use crate::repr::IntWidth;

use super::*;

fn narrowed_struct(pool: &Pool, idx: Idx) -> MachineRepr {
    let MachineRepr::Struct(mut repr) = crate::canonical::canonical(pool, idx) else {
        panic!("test type must have a struct representation");
    };
    repr.fields[0].repr = MachineRepr::Int {
        width: IntWidth::I8,
        signed: true,
    };
    MachineRepr::Struct(repr)
}

#[test]
fn layout_alias_propagation_does_not_cross_nominal_struct_names() {
    let mut pool = Pool::new();
    let field = Name::new(0, 1);
    let source = pool.struct_type(Name::new(0, 2), &[(field, Idx::INT)]);
    let target = pool.struct_type(Name::new(0, 3), &[(field, Idx::INT)]);

    let source_repr = narrowed_struct(&pool, source);
    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    plan.set_repr(
        target,
        ReprDecision {
            source: DecisionSource::Canonical,
            type_idx: target,
            repr: crate::canonical::canonical(&pool, target),
            reason: DecisionReason::Canonical,
        },
    );

    let aliases = LayoutAliasIndex::new(&pool);
    propagate_layout_to_aliases(&mut plan, &pool, &aliases, source, &source_repr);

    let Some(MachineRepr::Struct(target_repr)) = plan.get_repr(target) else {
        panic!("target must retain a struct representation");
    };
    assert_eq!(
        target_repr.fields[0].repr,
        MachineRepr::Int {
            width: IntWidth::I64,
            signed: true,
        }
    );
}

#[test]
fn layout_alias_propagation_retains_same_name_struct_aliases() {
    let mut pool = Pool::new();
    let struct_name = Name::new(0, 4);
    let field = Name::new(0, 5);
    let source_field = pool.named(Name::new(0, 6));
    let target_field = pool.named(Name::new(0, 7));
    pool.set_resolution(source_field, Idx::INT);
    pool.set_resolution(target_field, Idx::INT);
    let source = pool.struct_type(struct_name, &[(field, source_field)]);
    let target = pool.struct_type(struct_name, &[(field, target_field)]);

    let source_repr = narrowed_struct(&pool, source);
    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    plan.set_repr(
        target,
        ReprDecision {
            source: DecisionSource::Canonical,
            type_idx: target,
            repr: crate::canonical::canonical(&pool, target),
            reason: DecisionReason::Canonical,
        },
    );

    let aliases = LayoutAliasIndex::new(&pool);
    propagate_layout_to_aliases(&mut plan, &pool, &aliases, source, &source_repr);

    assert_eq!(plan.get_repr(target), Some(&source_repr));
}

#[test]
fn layout_alias_index_resolves_nested_child_aliases() {
    let mut pool = Pool::new();
    let struct_name = Name::new(0, 8);
    let field = Name::new(0, 9);
    let source_leaf = pool.named(Name::new(0, 10));
    let target_leaf = pool.named(Name::new(0, 11));
    let source_tuple = pool.tuple(&[source_leaf]);
    let target_tuple = pool.tuple(&[target_leaf]);
    pool.set_resolution(source_leaf, Idx::INT);
    pool.set_resolution(target_leaf, Idx::INT);
    let source = pool.struct_type(struct_name, &[(field, source_tuple)]);
    let target = pool.struct_type(struct_name, &[(field, target_tuple)]);

    assert!(pool.structural_eq(source, target));
    let source_repr = narrowed_struct(&pool, source);
    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    let aliases = LayoutAliasIndex::new(&pool);

    propagate_layout_to_aliases(&mut plan, &pool, &aliases, source, &source_repr);

    assert_eq!(plan.get_repr(target), Some(&source_repr));
}
