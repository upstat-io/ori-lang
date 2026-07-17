use ori_arc::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArcVarId, ArgOwnership,
    MethodCallFact, MethodCallForm,
};
use ori_ir::{Name, StringInterner};
use ori_types::{Idx, Pool};
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

    rewrite_impl_targets(&mut functions, &targets, &pool);

    let rewritten_targets: Vec<Name> = functions
        .iter()
        .map(|function| match function.blocks[0].body[0] {
            ArcInstr::Apply { func, .. } => func,
            ref instruction => panic!("expected rewritten hash call, found {instruction:?}"),
        })
        .collect();
    assert_eq!(rewritten_targets, vec![left_target, right_target]);
}
