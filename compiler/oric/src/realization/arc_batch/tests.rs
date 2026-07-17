use ori_arc::ArcFunction;
use ori_ir::StringInterner;

use super::{ArcFunctionGroup, LoweredArcBatch};

#[test]
fn duplicate_parent_diagnostic_names_callable_and_trace_action() {
    let interner = StringInterner::new();
    let name = interner.intern("duplicate_callable");
    let group = || {
        ArcFunctionGroup::new(
            ArcFunction {
                name,
                ..ArcFunction::default()
            },
            Vec::new(),
        )
    };

    let Err(error) = LoweredArcBatch::try_from_groups([group(), group()], &interner) else {
        panic!("duplicate parents must fail before batch mutation completes");
    };
    let message = error.to_string();

    assert!(message.contains("duplicate_callable"));
    assert!(message.contains("multiple lowering sources"));
    assert!(message.contains("ORI_LOG=oric::realization::arc_batch=debug"));
    assert!(!message.contains("Name(shard="));
}
