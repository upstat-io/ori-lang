use crate::check::ModuleChecker;
use crate::{GenericArg, Idx, MonoInstance, MonoInstanceId};
use ori_ir::{ExprArena, ExprId, Name, StringInterner};

fn mono_instance(name: u32) -> MonoInstance {
    MonoInstance::new_top_level(
        Name::from_raw(name),
        vec![GenericArg::Type(Idx::INT)],
        Vec::new(),
        Idx::INT,
        Vec::new(),
    )
}

#[test]
fn reanchors_each_body_session_into_module_coordinates() {
    let arena = ExprArena::new();
    let interner = StringInterner::new();
    let mut checker = ModuleChecker::new(&arena, &interner);

    checker.accumulate_mono_session(
        vec![mono_instance(1), mono_instance(2)],
        vec![(ExprId::new(10), MonoInstanceId::new(1))],
    );
    checker.accumulate_mono_session(
        vec![mono_instance(3)],
        vec![(ExprId::new(20), MonoInstanceId::new(0))],
    );

    assert_eq!(checker.mono_instances.len(), 3);
    assert_eq!(
        checker.mono_dispatch_pre_dedup,
        vec![
            (ExprId::new(10), MonoInstanceId::new(1)),
            (ExprId::new(20), MonoInstanceId::new(2)),
        ]
    );
}
