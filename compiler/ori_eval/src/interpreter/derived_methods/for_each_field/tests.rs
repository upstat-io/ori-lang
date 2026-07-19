use ori_ir::{DerivedMethodInfo, DerivedTrait, ExprArena, Name, StringInterner};
use rustc_hash::FxHashMap;

use crate::{
    EvalResult, Interpreter, InterpreterBuilder, SharedMutableRegistry, StructValue,
    UserMethodRegistry, Value,
};

fn hash_info(fields: &[Name]) -> DerivedMethodInfo {
    DerivedMethodInfo::new(DerivedTrait::Hashable, fields.to_vec())
}

fn sum_hash_info(variants: &[Name]) -> DerivedMethodInfo {
    DerivedMethodInfo::new_sum(DerivedTrait::Hashable, variants.to_vec())
}

fn registry_with_hashes(
    interner: &StringInterner,
    registrations: &[(Name, DerivedMethodInfo)],
) -> SharedMutableRegistry<UserMethodRegistry> {
    let registry = SharedMutableRegistry::new(UserMethodRegistry::new());
    let hash = interner.intern("hash");
    {
        let mut writer = registry.write();
        for (type_name, info) in registrations {
            writer.register_derived(*type_name, hash, info.clone());
        }
    }
    registry
}

fn build_interpreter<'a>(
    interner: &'a StringInterner,
    arena: &'a ExprArena,
    registry: SharedMutableRegistry<UserMethodRegistry>,
) -> Interpreter<'a> {
    InterpreterBuilder::new(interner, arena)
        .user_method_registry(registry)
        .build()
}

fn struct_value(type_name: Name, fields: &[(Name, Value)]) -> Value {
    let mut values = FxHashMap::default();
    for (field, value) in fields {
        values.insert(*field, value.clone());
    }
    Value::Struct(StructValue::new(type_name, values))
}

fn eval_hash_result(
    interpreter: &mut Interpreter<'_>,
    interner: &StringInterner,
    value: Value,
) -> EvalResult {
    interpreter.eval_method_call(value, interner.intern("hash"), vec![])
}

fn eval_hash(interpreter: &mut Interpreter<'_>, interner: &StringInterner, value: Value) -> i64 {
    match eval_hash_result(interpreter, interner, value) {
        Ok(Value::Int(value)) => value.raw(),
        Ok(other) => panic!("derived hash returned {}, expected int", other.type_name()),
        Err(action) => panic!("derived hash failed: {action:?}"),
    }
}

#[test]
fn derived_product_hash_uses_declaration_order_and_ignores_unit_fields() {
    let interner = StringInterner::new();
    let arena = ExprArena::new();
    let forward = interner.intern("Forward");
    let reverse = interner.intern("Reverse");
    let first = interner.intern("first");
    let unit = interner.intern("unit");
    let second = interner.intern("second");
    let registry = registry_with_hashes(
        &interner,
        &[
            (forward, hash_info(&[first, unit, second])),
            (reverse, hash_info(&[second, unit, first])),
        ],
    );
    let mut interpreter = build_interpreter(&interner, &arena, registry);
    let forward_value = struct_value(
        forward,
        &[
            (first, Value::int(1)),
            (unit, Value::tuple(Vec::new())),
            (second, Value::int(2)),
        ],
    );
    let reverse_value = struct_value(
        reverse,
        &[
            (first, Value::int(1)),
            (unit, Value::tuple(Vec::new())),
            (second, Value::int(2)),
        ],
    );

    assert_eq!(
        (
            eval_hash(&mut interpreter, &interner, forward_value),
            eval_hash(&mut interpreter, &interner, reverse_value),
        ),
        (175_247_769_363, 175_247_769_427),
    );
}

#[test]
fn derived_product_hash_of_empty_or_all_unit_product_is_zero() {
    let interner = StringInterner::new();
    let arena = ExprArena::new();
    let empty = interner.intern("Empty");
    let only_unit = interner.intern("OnlyUnit");
    let unit = interner.intern("unit");
    let registry = registry_with_hashes(
        &interner,
        &[(empty, hash_info(&[])), (only_unit, hash_info(&[unit]))],
    );
    let mut interpreter = build_interpreter(&interner, &arena, registry);

    assert_eq!(
        (
            eval_hash(&mut interpreter, &interner, struct_value(empty, &[])),
            eval_hash(
                &mut interpreter,
                &interner,
                struct_value(only_unit, &[(unit, Value::tuple(Vec::new()))]),
            ),
        ),
        (0, 0),
    );
}

#[test]
fn derived_sum_hash_combines_declaration_ordinal_before_payload() {
    let interner = StringInterner::new();
    let arena = ExprArena::new();
    let state = interner.intern("State");
    let idle = interner.intern("Idle");
    let active = interner.intern("Active");
    let done = interner.intern("Done");
    let registry =
        registry_with_hashes(&interner, &[(state, sum_hash_info(&[idle, active, done]))]);
    let mut interpreter = build_interpreter(&interner, &arena, registry);

    assert_eq!(
        (
            eval_hash(
                &mut interpreter,
                &interner,
                Value::variant(state, active, vec![Value::int(2)]),
            ),
            eval_hash(
                &mut interpreter,
                &interner,
                Value::variant(state, done, vec![]),
            ),
        ),
        (175_247_769_363, 2_654_435_771),
    );
}

#[test]
fn derived_sum_hash_rejects_variant_absent_from_declaration_order() {
    let interner = StringInterner::new();
    let arena = ExprArena::new();
    let state = interner.intern("State");
    let idle = interner.intern("Idle");
    let unknown = interner.intern("Unknown");
    let registry = registry_with_hashes(&interner, &[(state, sum_hash_info(&[idle]))]);
    let mut interpreter = build_interpreter(&interner, &arena, registry);

    assert!(eval_hash_result(
        &mut interpreter,
        &interner,
        Value::variant(state, unknown, vec![]),
    )
    .is_err());
}

#[test]
fn derived_newtype_hash_delegates_to_underlying_value_exactly() {
    let interner = StringInterner::new();
    let arena = ExprArena::new();
    let user_id = interner.intern("UserId");
    let registry = registry_with_hashes(&interner, &[(user_id, hash_info(&[]))]);
    let mut interpreter = build_interpreter(&interner, &arena, registry);

    assert_eq!(
        eval_hash(
            &mut interpreter,
            &interner,
            Value::newtype(user_id, Value::int(42)),
        ),
        42,
    );
}

#[test]
fn derived_product_hash_dispatches_nested_user_hash() {
    let interner = StringInterner::new();
    let arena = ExprArena::new();
    let inner = interner.intern("Inner");
    let outer = interner.intern("Outer");
    let x = interner.intern("x");
    let y = interner.intern("y");
    let nested = interner.intern("nested");
    let registry = registry_with_hashes(
        &interner,
        &[(inner, hash_info(&[x, y])), (outer, hash_info(&[nested]))],
    );
    let mut interpreter = build_interpreter(&interner, &arena, registry);
    let inner_value = struct_value(inner, &[(x, Value::int(1)), (y, Value::int(2))]);
    let outer_value = struct_value(outer, &[(nested, inner_value)]);

    assert_eq!(
        eval_hash(&mut interpreter, &interner, outer_value),
        177_902_205_132,
    );
}

#[test]
fn builtin_collection_hash_dispatches_nested_user_hash() {
    let interner = StringInterner::new();
    let arena = ExprArena::new();
    let inner = interner.intern("Inner");
    let outer = interner.intern("Outer");
    let x = interner.intern("x");
    let y = interner.intern("y");
    let nested = interner.intern("nested");
    let registry = registry_with_hashes(
        &interner,
        &[(inner, hash_info(&[x, y])), (outer, hash_info(&[nested]))],
    );
    let mut interpreter = build_interpreter(&interner, &arena, registry);
    let inner_value = struct_value(inner, &[(x, Value::int(1)), (y, Value::int(2))]);
    let outer_value = struct_value(outer, &[(nested, Value::list(vec![inner_value]))]);

    assert_eq!(
        eval_hash(&mut interpreter, &interner, outer_value),
        180_556_640_901,
    );
}
