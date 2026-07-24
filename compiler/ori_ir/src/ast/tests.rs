use super::*;
use crate::Span;

#[test]
fn test_expr_kind_hash() {
    use std::collections::HashSet;
    let mut set = HashSet::new();

    set.insert(ExprKind::Int(42));
    set.insert(ExprKind::Int(42));
    set.insert(ExprKind::Int(43));
    set.insert(ExprKind::Bool(true));

    assert_eq!(set.len(), 3);
}

#[test]
fn test_access_step_and_expr_kind_are_copy() {
    // AssignTarget must keep ExprKind `Copy`: a `Vec<AccessStep>` payload would
    // break Copy and blow the 24-byte budget; the flattened AccessStepRange is
    // the only sound carrier.
    fn assert_copy<T: Copy>() {}
    assert_copy::<AccessStep>();
    assert_copy::<crate::AccessStepRange>();
    assert_copy::<ExprKind>();
    assert_copy::<Expr>();
}

#[test]
fn test_expr_kind_size_unchanged() {
    // The static_assert_size! block in expr.rs enforces this at compile time;
    // this test pins it as an explicit, greppable regression guard.
    assert_eq!(std::mem::size_of::<ExprKind>(), 24);
    assert_eq!(std::mem::size_of::<Expr>(), 32);
}

#[test]
fn test_assign_target_debug_renders_root_and_steps() {
    use crate::AccessStepRange;
    use crate::ExprId;
    let kind = ExprKind::AssignTarget {
        root: ExprId::new(3),
        steps: AccessStepRange::new(1, 2),
    };
    let rendered = format!("{kind:?}");
    assert!(
        rendered.starts_with("AssignTarget("),
        "unexpected debug: {rendered}"
    );
}

#[test]
fn test_binary_op() {
    let op = BinaryOp::Add;
    assert_eq!(op, BinaryOp::Add);
    assert_ne!(op, BinaryOp::Sub);
}

#[test]
fn test_expr_spanned() {
    use crate::Spanned;
    let expr = Expr::new(ExprKind::Int(42), Span::new(0, 2));
    assert_eq!(expr.span().start, 0);
    assert_eq!(expr.span().end, 2);
}

#[test]
fn test_module_hash() {
    use std::collections::HashSet;
    let mut set = HashSet::new();

    let m1 = Module::new();
    let m2 = Module::new();

    set.insert(m1);
    set.insert(m2);

    assert_eq!(set.len(), 1);
}

#[test]
fn test_function_exp_kind() {
    assert_eq!(FunctionExpKind::Parallel, FunctionExpKind::Parallel);
    assert_ne!(FunctionExpKind::Parallel, FunctionExpKind::Spawn);
    assert_eq!(FunctionExpKind::Parallel.name(), "parallel");
}

#[test]
fn test_impl_def_type_name_single_segment_returns_it() {
    use crate::{Name, ParsedType, ParsedTypeRange};
    let name = Name::new(0, 7);
    let imp = ImplDef {
        generics: GenericParamRange::EMPTY,
        trait_path: None,
        trait_type_args: ParsedTypeRange::EMPTY,
        self_path: vec![name],
        self_ty: ParsedType::Named {
            name,
            type_args: ParsedTypeRange::EMPTY,
        },
        where_clauses: Vec::new(),
        methods: Vec::new(),
        assoc_types: Vec::new(),
        span: Span::DUMMY,
        target_attr: None,
        cfg_attr: None,
    };
    assert_eq!(imp.type_name(), Some(name));
}

#[test]
fn test_impl_def_type_name_qualified_path_returns_last_segment() {
    use crate::{Name, ParsedType, ParsedTypeRange};
    // Semantic pin: the impl subject's type name is the LAST path segment
    // (the parser builds ParsedType::Named from path.last()); resolving the
    // FIRST segment of a qualified path yields the qualifier, not the type.
    let qualifier = Name::new(0, 11);
    let type_name = Name::new(0, 12);
    let imp = ImplDef {
        generics: GenericParamRange::EMPTY,
        trait_path: None,
        trait_type_args: ParsedTypeRange::EMPTY,
        self_path: vec![qualifier, type_name],
        self_ty: ParsedType::Named {
            name: type_name,
            type_args: ParsedTypeRange::EMPTY,
        },
        where_clauses: Vec::new(),
        methods: Vec::new(),
        assoc_types: Vec::new(),
        span: Span::DUMMY,
        target_attr: None,
        cfg_attr: None,
    };
    assert_eq!(imp.type_name(), Some(type_name));
    assert_ne!(imp.type_name(), imp.self_path.first().copied());
}

#[test]
fn test_impl_def_type_name_empty_path_returns_none() {
    use crate::{Name, ParsedType, ParsedTypeRange};
    let imp = ImplDef {
        generics: GenericParamRange::EMPTY,
        trait_path: None,
        trait_type_args: ParsedTypeRange::EMPTY,
        self_path: Vec::new(),
        self_ty: ParsedType::Named {
            name: Name::EMPTY,
            type_args: ParsedTypeRange::EMPTY,
        },
        where_clauses: Vec::new(),
        methods: Vec::new(),
        assoc_types: Vec::new(),
        span: Span::DUMMY,
        target_attr: None,
        cfg_attr: None,
    };
    assert_eq!(imp.type_name(), None);
}
