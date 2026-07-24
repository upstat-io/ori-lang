use ori_arc::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArcVarId, ArgOwnership,
    MethodCallFact, MethodCallForm,
};
use ori_ir::{Name, StringInterner};
use ori_types::{Idx, ImplMethodId, MethodProducer, Pool};
use rustc_hash::FxHashMap;

use super::rewrite_impl_targets;

fn hash_call(function_name: Name, hash: Name, receiver_type: Idx) -> ArcFunction {
    ArcFunction {
        name: function_name,
        var_types: vec![receiver_type, Idx::INT],
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![ArcInstr::Apply {
                dst: ArcVarId::new(1),
                ty: Idx::INT,
                func: hash,
                args: vec![ArcVarId::new(0)],
                arg_ownership: vec![ArgOwnership::Owned],
                mono_instance_id: None,
            }],
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(1),
            },
        }],
        method_call_facts: vec![MethodCallFact {
            destination: ArcVarId::new(1),
            receiver_type,
            form: MethodCallForm::Instance,
            producer: None,
            selected_producer: None,
            derived_position: None,
        }],
        ..ArcFunction::default()
    }
}

#[test]
fn newtype_hash_calls_rewrite_by_nominal_receiver() {
    let interner = StringInterner::new();
    let left_name = interner.intern("LeftKey");
    let right_name = interner.intern("RightKey");
    let hash = interner.intern("hash");
    let left_target = interner.intern("hash$derived$LeftKey");
    let right_target = interner.intern("hash$derived$RightKey");
    let mut pool = Pool::new();
    let left = pool.named(left_name);
    let right = pool.named(right_name);
    pool.register_newtype_ctor(left_name, Idx::INT);
    pool.register_newtype_ctor(right_name, Idx::INT);
    pool.set_resolution(left, Idx::INT);
    pool.set_resolution(right, Idx::INT);
    let targets =
        FxHashMap::from_iter([((left, hash), left_target), ((right, hash), right_target)]);
    let mut functions = vec![
        hash_call(interner.intern("hash_left"), hash, left),
        hash_call(interner.intern("hash_right"), hash, right),
    ];

    rewrite_impl_targets(&mut functions, &targets, &FxHashMap::default(), &pool);

    let rewritten_targets: Vec<Name> = functions
        .iter()
        .map(|function| match function.blocks[0].body[0] {
            ArcInstr::Apply { func, .. } => func,
            ref instruction => panic!("expected rewritten hash call, found {instruction:?}"),
        })
        .collect();
    assert_eq!(rewritten_targets, vec![left_target, right_target]);
}

#[test]
fn same_receiver_same_name_impls_rewrite_by_exact_producer() {
    let interner = StringInterner::new();
    let mut pool = Pool::new();
    let receiver_name = interner.intern("JsonValue");
    let field_name = interner.intern("data");
    let receiver = pool.struct_type(receiver_name, &[(field_name, Idx::STR)]);
    let index = interner.intern("index");
    let int_target = interner.intern("__impl_json_index_int");
    let str_target = interner.intern("__impl_json_index_str");
    let int_producer = MethodProducer::Impl(ImplMethodId::new(0, ori_ir::ExprId::new(10)));
    let str_producer = MethodProducer::Impl(ImplMethodId::new(1, ori_ir::ExprId::new(20)));
    let mut functions = vec![
        hash_call(interner.intern("read_int"), index, receiver),
        hash_call(interner.intern("read_str"), index, receiver),
    ];
    functions[0].method_call_facts[0].producer = Some(int_producer.clone());
    functions[1].method_call_facts[0].producer = Some(str_producer.clone());
    let producer_targets =
        FxHashMap::from_iter([(int_producer, int_target), (str_producer, str_target)]);

    rewrite_impl_targets(
        &mut functions,
        &FxHashMap::default(),
        &producer_targets,
        &pool,
    );

    let rewritten: Vec<_> = functions
        .iter()
        .map(|function| match function.blocks[0].body[0] {
            ArcInstr::Apply { func, .. } => func,
            ref instruction => panic!("expected rewritten index call, found {instruction:?}"),
        })
        .collect();
    assert_eq!(rewritten, [int_target, str_target]);
}
