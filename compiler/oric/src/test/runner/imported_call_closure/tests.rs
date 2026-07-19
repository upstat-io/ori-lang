use ori_arc::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArcVarId, ArgOwnership, CtorKind,
    MethodCallFact, MethodCallForm,
};
use ori_ir::Name;
use ori_types::Idx;
use rustc_hash::FxHashMap;

use super::{close_reachable_imports, rewrite_function};
use crate::realization::ArcFunctionGroup;

fn function(name: Name, targets: &[Name]) -> ArcFunction {
    ArcFunction {
        name,
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: targets
                .iter()
                .enumerate()
                .map(|(index, target)| ArcInstr::Apply {
                    dst: ArcVarId::new(
                        u32::try_from(index)
                            .unwrap_or_else(|_| panic!("test fixture index {index} exceeds u32")),
                    ),
                    ty: Idx::UNIT,
                    func: *target,
                    args: Vec::new(),
                    arg_ownership: Vec::new(),
                    mono_instance_id: None,
                })
                .collect(),
            terminator: ArcTerminator::Unreachable,
        }],
        var_types: vec![Idx::UNIT; targets.len()],
        ..ArcFunction::default()
    }
}

#[test]
fn lexical_rewrite_covers_calls_function_values_closures_and_invokes() {
    let source = Name::from_raw(10);
    let target = Name::from_raw(20);
    let mut function = function(Name::from_raw(1), &[]);
    function.blocks[0].body = vec![
        ArcInstr::Apply {
            dst: ArcVarId::new(0),
            ty: Idx::UNIT,
            func: source,
            args: Vec::new(),
            arg_ownership: Vec::new(),
            mono_instance_id: Some(ori_ir::canon::MonoInstanceId::new(0)),
        },
        ArcInstr::PartialApply {
            dst: ArcVarId::new(1),
            ty: Idx::UNIT,
            func: source,
            args: Vec::new(),
        },
        ArcInstr::Construct {
            dst: ArcVarId::new(2),
            ty: Idx::UNIT,
            ctor: CtorKind::Closure { func: source },
            args: Vec::new(),
        },
    ];
    function.blocks[0].terminator = ArcTerminator::Invoke {
        dst: ArcVarId::new(3),
        ty: Idx::UNIT,
        func: source,
        args: Vec::new(),
        arg_ownership: Vec::<ArgOwnership>::new(),
        mono_instance_id: Some(ori_ir::canon::MonoInstanceId::new(0)),
        normal: ArcBlockId::new(0),
        unwind: ArcBlockId::new(0),
    };
    let targets = FxHashMap::from_iter([(source, target)]);

    rewrite_function(&mut function, &targets, true);

    assert!(matches!(
        function.blocks[0].body[0],
        ArcInstr::Apply {
            func,
            mono_instance_id: None,
            ..
        } if func == target
    ));
    assert!(matches!(
        function.blocks[0].body[1],
        ArcInstr::PartialApply { func, .. } if func == target
    ));
    assert!(matches!(
        function.blocks[0].body[2],
        ArcInstr::Construct {
            ctor: CtorKind::Closure { func },
            ..
        } if func == target
    ));
    assert!(matches!(
        function.blocks[0].terminator,
        ArcTerminator::Invoke {
            func,
            mono_instance_id: None,
            ..
        } if func == target
    ));
}

#[test]
fn lexical_rewrite_preserves_typed_method_dispatch() {
    let source = Name::from_raw(10);
    let target = Name::from_raw(20);
    let destination = ArcVarId::new(1);
    let mut function = function(Name::from_raw(1), &[]);
    function.var_types = vec![Idx::INT, Idx::INT];
    function.blocks[0].body = vec![ArcInstr::Apply {
        dst: destination,
        ty: Idx::INT,
        func: source,
        args: vec![ArcVarId::new(0)],
        arg_ownership: vec![ArgOwnership::Borrowed],
        mono_instance_id: Some(ori_ir::canon::MonoInstanceId::new(0)),
    }];
    function.method_call_facts = vec![MethodCallFact {
        destination,
        receiver_type: Idx::INT,
        form: MethodCallForm::Instance,
        producer: None,
        selected_producer: None,
        derived_position: None,
    }];
    let targets = FxHashMap::from_iter([(source, target)]);

    rewrite_function(&mut function, &targets, true);

    assert!(matches!(
        function.blocks[0].body[0],
        ArcInstr::Apply {
            func,
            mono_instance_id: None,
            ..
        } if func == source
    ));
}

#[test]
fn lexical_rewrite_preserves_typed_method_invoke_dispatch() {
    let source = Name::from_raw(10);
    let target = Name::from_raw(20);
    let destination = ArcVarId::new(1);
    let mut function = function(Name::from_raw(1), &[]);
    function.var_types = vec![Idx::INT, Idx::INT];
    function.blocks[0].terminator = ArcTerminator::Invoke {
        dst: destination,
        ty: Idx::INT,
        func: source,
        args: vec![ArcVarId::new(0)],
        arg_ownership: vec![ArgOwnership::Borrowed],
        mono_instance_id: Some(ori_ir::canon::MonoInstanceId::new(0)),
        normal: ArcBlockId::new(0),
        unwind: ArcBlockId::new(0),
    };
    function.method_call_facts = vec![MethodCallFact {
        destination,
        receiver_type: Idx::INT,
        form: MethodCallForm::Instance,
        producer: None,
        selected_producer: None,
        derived_position: None,
    }];
    let targets = FxHashMap::from_iter([(source, target)]);

    rewrite_function(&mut function, &targets, true);

    assert!(matches!(
        function.blocks[0].terminator,
        ArcTerminator::Invoke {
            func,
            mono_instance_id: None,
            ..
        } if func == source
    ));
}

#[test]
fn reachability_walks_transitive_import_edges_and_drops_unrelated_candidates() {
    let root = Name::from_raw(1);
    let first = Name::from_raw(10);
    let second = Name::from_raw(20);
    let unrelated = Name::from_raw(30);
    let roots = vec![ArcFunctionGroup::new(function(root, &[first]), Vec::new())];
    let candidates = vec![
        ArcFunctionGroup::new(function(first, &[second]), Vec::new()),
        ArcFunctionGroup::new(function(second, &[]), Vec::new()),
        ArcFunctionGroup::new(function(unrelated, &[]), Vec::new()),
    ];

    let (retained, reachable) = close_reachable_imports(&roots, candidates);

    assert_eq!(retained.len(), 2);
    assert!(reachable.contains(&first));
    assert!(reachable.contains(&second));
    assert!(!reachable.contains(&unrelated));
}
