use ori_ir::canon::{
    CanArena, CanExpr, CanNode, CanonResult, IndexDispatch, MethodProducerId, SharedCanonResult,
};
use ori_ir::{ExprArena, SharedInterner, Span, TypeId};
use ori_patterns::{ControlAction, EvalErrorKind};

use crate::interpreter::InterpreterBuilder;

fn push(arena: &mut CanArena, kind: CanExpr, ty: TypeId) -> ori_ir::canon::CanId {
    arena.push(CanNode::new(kind, Span::DUMMY, ty))
}

#[test]
fn index_selected_route_builtin_receiver_reports_method_error() {
    let mut arena = CanArena::new();
    let element = push(&mut arena, CanExpr::Int(7), TypeId::INT);
    let elements = arena.push_expr_list(&[element]);
    let receiver = push(&mut arena, CanExpr::List(elements), TypeId::INFER);
    let index = push(&mut arena, CanExpr::Int(0), TypeId::INT);
    let root = push(
        &mut arena,
        CanExpr::Index {
            receiver,
            index,
            dispatch: IndexDispatch::Selected(MethodProducerId::new(0)),
        },
        TypeId::INT,
    );
    let canon = SharedCanonResult::new(CanonResult::new(arena, root));
    let interner = SharedInterner::default();
    let source = ExprArena::new();
    let mut interpreter = InterpreterBuilder::new(&interner, &source)
        .canon(canon)
        .build();

    let Err(ControlAction::Error(error)) = interpreter.eval_can(root) else {
        panic!("a selected user route must not fall back to builtin indexing");
    };
    assert!(matches!(error.kind, EvalErrorKind::UndefinedMethod { .. }));
}
