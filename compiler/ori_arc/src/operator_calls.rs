//! Resolve user-defined operator primitives into ordinary callable ARC IR.

mod validation;

use ori_ir::{builtin_constants::ordering, BinaryOp, Name, Span, StringInterner, UnaryOp};
use ori_types::{Idx, Pool};
use rustc_hash::FxHashSet;

use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, LitValue, PrimOp};
use validation::locate_invoke;

/// Failure to bind a non-builtin operator to one exact callable target.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperatorCallResolutionError {
    /// Function containing the operator expression.
    pub function: Name,
    /// Stable result variable of the unresolved expression.
    pub destination: ArcVarId,
    /// Fully resolved receiver type used for target lookup.
    pub receiver_type: Idx,
    /// Surface operator symbol.
    pub operation: &'static str,
}

impl std::fmt::Display for OperatorCallResolutionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "function {:?} operator {} at {:?} has no exact callable target for receiver type {:?}",
            self.function, self.operation, self.destination, self.receiver_type
        )
    }
}

impl std::error::Error for OperatorCallResolutionError {}

struct Rewrite {
    function: usize,
    block: usize,
    kind: RewriteKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RewriteKind {
    Builtin {
        operation: PrimOp,
        span: Option<Span>,
    },
    User {
        target: Name,
        projection: OperatorProjection,
        span: Option<Span>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OperatorProjection {
    Identity,
    BoolNot,
    Ordering { predicate: BinaryOp, bound: i64 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct OperatorCallPlan {
    pub(crate) method: &'static str,
    pub(crate) projection: OperatorProjection,
}

/// Close every lowered user-defined operator over one exact callable target.
///
/// Builtin receivers remain `PrimOp` sites and are resolved by the typed
/// primitive descriptor seam. User-defined operators are already ordinary
/// may-unwind calls at this seam so lexical catch routing and cleanup CFG are
/// preserved. This pass validates their lowering facts and replaces each
/// surface method name with the exact implementation identity supplied by the
/// realization owner. Any missing target or call site fails transactionally.
pub fn rewrite_operator_trait_calls(
    functions: &mut [ArcFunction],
    pool: &Pool,
    interner: &StringInterner,
    resolve_target: &impl Fn(Idx, Name) -> Option<Name>,
) -> Result<(), Vec<OperatorCallResolutionError>> {
    let mut rewrites = Vec::new();
    let mut errors = Vec::new();

    for (function_index, function) in functions.iter().enumerate() {
        let mut seen_destinations = FxHashSet::default();
        let predecessors = crate::graph::compute_predecessors(function);
        collect_unlowered_user_operators(function, pool, &mut errors);
        for fact in &function.operator_call_facts {
            let Some(&receiver_type) = function.var_types.get(fact.receiver.index()) else {
                errors.push(error(
                    function.name,
                    fact.destination,
                    fact.operation,
                    Idx::ERROR,
                ));
                continue;
            };
            if !seen_destinations.insert(fact.destination) {
                errors.push(error(
                    function.name,
                    fact.destination,
                    fact.operation,
                    receiver_type,
                ));
                continue;
            }
            let Some(plan) = operator_call_plan(fact.operation) else {
                errors.push(error(
                    function.name,
                    fact.destination,
                    fact.operation,
                    receiver_type,
                ));
                continue;
            };
            let method = interner.intern(plan.method);
            let Some(block) = locate_invoke(
                function,
                fact.destination,
                fact.receiver,
                fact.operation,
                method,
                &predecessors,
            ) else {
                errors.push(error(
                    function.name,
                    fact.destination,
                    fact.operation,
                    receiver_type,
                ));
                continue;
            };
            let kind = if pool.builtin_method_type_tag(receiver_type).is_some() {
                RewriteKind::Builtin {
                    operation: fact.operation,
                    span: fact.span,
                }
            } else {
                let Some(target) = resolve_target(receiver_type, method) else {
                    errors.push(error(
                        function.name,
                        fact.destination,
                        fact.operation,
                        receiver_type,
                    ));
                    continue;
                };
                RewriteKind::User {
                    target,
                    projection: plan.projection,
                    span: fact.span,
                }
            };
            rewrites.push(Rewrite {
                function: function_index,
                block,
                kind,
            });
        }
    }

    if !errors.is_empty() {
        return Err(errors);
    }

    for rewrite in rewrites {
        let function = &mut functions[rewrite.function];
        match rewrite.kind {
            RewriteKind::Builtin { operation, span } => {
                apply_builtin_rewrite(function, rewrite.block, operation, span, pool);
            }
            RewriteKind::User {
                target,
                projection,
                span,
            } => apply_user_rewrite(function, rewrite.block, target, projection, span),
        }
    }
    for function in functions {
        function.operator_call_facts.clear();
    }
    Ok(())
}

fn collect_unlowered_user_operators(
    function: &ArcFunction,
    pool: &Pool,
    errors: &mut Vec<OperatorCallResolutionError>,
) {
    for block in &function.blocks {
        for instruction in &block.body {
            let ArcInstr::Let {
                dst,
                value: crate::ir::ArcValue::PrimOp { op, args },
                ..
            } = instruction
            else {
                continue;
            };
            let Some(&receiver) = args.first() else {
                if operator_call_plan(*op).is_some() {
                    errors.push(error(function.name, *dst, *op, Idx::ERROR));
                }
                continue;
            };
            let Some(&receiver_type) = function.var_types.get(receiver.index()) else {
                if operator_call_plan(*op).is_some() {
                    errors.push(error(function.name, *dst, *op, Idx::ERROR));
                }
                continue;
            };
            if pool.builtin_method_type_tag(receiver_type).is_none()
                && operator_call_plan(*op).is_some()
            {
                errors.push(error(function.name, *dst, *op, receiver_type));
            }
        }
    }
}

fn apply_builtin_rewrite(
    function: &mut ArcFunction,
    block: usize,
    operation: PrimOp,
    span: Option<Span>,
    pool: &Pool,
) {
    let (destination, result_type, arguments, normal, unwind) =
        match &function.blocks[block].terminator {
            ArcTerminator::Invoke {
                dst,
                ty,
                args,
                normal,
                unwind,
                ..
            } => (*dst, *ty, args.clone(), *normal, *unwind),
            _ => unreachable!("validated builtin operator Invoke changed before commit"),
        };
    let catch_handler = match &function.blocks[unwind.index()].terminator {
        ArcTerminator::Jump { target, .. } => Some(*target),
        _ => None,
    };
    function.blocks[block].body.push(ArcInstr::Let {
        dst: destination,
        ty: result_type,
        value: ArcValue::PrimOp {
            op: operation,
            args: arguments,
        },
    });
    if let Some(spans) = function.spans.get_mut(block) {
        spans.push(span);
    }
    function.blocks[block].terminator = ArcTerminator::Jump {
        target: normal,
        args: Vec::new(),
    };
    function.blocks[unwind.index()].body.clear();
    function.blocks[unwind.index()].terminator = ArcTerminator::Unreachable;
    if let Some(spans) = function.spans.get_mut(unwind.index()) {
        spans.clear();
    }
    if operation_may_panic_on(result_type, operation, pool) {
        if let Some(handler) = catch_handler {
            function
                .catch_scoped_checked_ops
                .push((destination, handler));
        }
    }
}

fn apply_user_rewrite(
    function: &mut ArcFunction,
    block: usize,
    target: Name,
    projection: OperatorProjection,
    span: Option<Span>,
) {
    let (destination, result_type, normal) = match &function.blocks[block].terminator {
        ArcTerminator::Invoke {
            dst, ty, normal, ..
        } => (*dst, *ty, *normal),
        _ => unreachable!("validated user operator Invoke changed before commit"),
    };
    match projection {
        OperatorProjection::Identity => {
            // Invoke terminators have no instruction-span entry. Anchor the
            // operator span on the metadata-preserving result alias instead.
            let call_result = function.fresh_var_like_typed(destination, result_type);
            let ArcTerminator::Invoke { dst, func, .. } = &mut function.blocks[block].terminator
            else {
                unreachable!()
            };
            *dst = call_result;
            *func = target;
            prepend_instructions(
                function,
                normal.index(),
                vec![ArcInstr::Let {
                    dst: destination,
                    ty: result_type,
                    value: ArcValue::Var(call_result),
                }],
                vec![span],
            );
        }
        OperatorProjection::BoolNot => {
            let call_result = function.fresh_scalar_var(Idx::BOOL);
            let ArcTerminator::Invoke { dst, ty, func, .. } =
                &mut function.blocks[block].terminator
            else {
                unreachable!()
            };
            *dst = call_result;
            *ty = Idx::BOOL;
            *func = target;
            prepend_instructions(
                function,
                normal.index(),
                vec![ArcInstr::Let {
                    dst: destination,
                    ty: result_type,
                    value: ArcValue::PrimOp {
                        op: PrimOp::Unary(UnaryOp::Not),
                        args: vec![call_result],
                    },
                }],
                vec![span],
            );
        }
        OperatorProjection::Ordering { predicate, bound } => {
            let ordering_result = function.fresh_scalar_var(Idx::ORDERING);
            let tag = function.fresh_scalar_var(Idx::INT);
            let bound_var = function.fresh_scalar_var(Idx::INT);
            let ArcTerminator::Invoke { dst, ty, func, .. } =
                &mut function.blocks[block].terminator
            else {
                unreachable!()
            };
            *dst = ordering_result;
            *ty = Idx::ORDERING;
            *func = target;
            prepend_instructions(
                function,
                normal.index(),
                vec![
                    ArcInstr::Project {
                        dst: tag,
                        ty: Idx::INT,
                        value: ordering_result,
                        field: 0,
                    },
                    ArcInstr::Let {
                        dst: bound_var,
                        ty: Idx::INT,
                        value: ArcValue::Literal(LitValue::Int(bound)),
                    },
                    ArcInstr::Let {
                        dst: destination,
                        ty: result_type,
                        value: ArcValue::PrimOp {
                            op: PrimOp::Binary(predicate),
                            args: vec![tag, bound_var],
                        },
                    },
                ],
                vec![None, None, span],
            );
        }
    }
}

fn prepend_instructions(
    function: &mut ArcFunction,
    block: usize,
    instructions: Vec<ArcInstr>,
    new_spans: Vec<Option<Span>>,
) {
    function.blocks[block].body.splice(0..0, instructions);
    if let Some(spans) = function.spans.get_mut(block) {
        spans.splice(0..0, new_spans);
    }
}

fn operation_may_panic_on(result_type: Idx, operation: PrimOp, pool: &Pool) -> bool {
    let may_panic = match operation {
        PrimOp::Binary(operation) => operation.may_panic_on_int(),
        PrimOp::Unary(operation) => operation.may_panic_on_int(),
    };
    may_panic
        && pool
            .tag(pool.resolve_fully(result_type))
            .is_checked_int_arithmetic()
}

pub(crate) fn operator_call_plan(operation: PrimOp) -> Option<OperatorCallPlan> {
    let (method, projection) = match operation {
        PrimOp::Binary(BinaryOp::Eq) => ("eq", OperatorProjection::Identity),
        PrimOp::Binary(BinaryOp::NotEq) => ("eq", OperatorProjection::BoolNot),
        PrimOp::Binary(BinaryOp::Lt) => (
            "compare",
            OperatorProjection::Ordering {
                predicate: BinaryOp::Eq,
                bound: i64::from(ordering::LESS),
            },
        ),
        PrimOp::Binary(BinaryOp::LtEq) => (
            "compare",
            OperatorProjection::Ordering {
                predicate: BinaryOp::NotEq,
                bound: i64::from(ordering::GREATER),
            },
        ),
        PrimOp::Binary(BinaryOp::Gt) => (
            "compare",
            OperatorProjection::Ordering {
                predicate: BinaryOp::Eq,
                bound: i64::from(ordering::GREATER),
            },
        ),
        PrimOp::Binary(BinaryOp::GtEq) => (
            "compare",
            OperatorProjection::Ordering {
                predicate: BinaryOp::NotEq,
                bound: i64::from(ordering::LESS),
            },
        ),
        PrimOp::Binary(operation) => (operation.trait_method_name()?, OperatorProjection::Identity),
        PrimOp::Unary(operation) => (operation.trait_method_name()?, OperatorProjection::Identity),
    };
    Some(OperatorCallPlan { method, projection })
}

fn error(
    function: Name,
    destination: ArcVarId,
    operation: PrimOp,
    receiver_type: Idx,
) -> OperatorCallResolutionError {
    OperatorCallResolutionError {
        function,
        destination,
        receiver_type,
        operation: match operation {
            PrimOp::Binary(operation) => operation.as_symbol(),
            PrimOp::Unary(operation) => operation.as_symbol(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{ArcBlock, ArcBlockId, OperatorCallFact, ValueRepr, VariableMetadataState};

    fn placeholder_function(
        receiver_type: Idx,
        result_type: Idx,
        operation: PrimOp,
        method: Name,
        catch_handler: bool,
    ) -> ArcFunction {
        let source_span = Span::new(10, 20);
        ArcFunction {
            name: Name::from_raw(1),
            return_type: result_type,
            var_types: vec![receiver_type, receiver_type, result_type],
            spans: vec![vec![], vec![], vec![], vec![]],
            operator_call_facts: vec![OperatorCallFact {
                destination: ArcVarId::new(2),
                receiver: ArcVarId::new(0),
                operation,
                span: Some(source_span),
            }],
            blocks: vec![
                ArcBlock {
                    id: ArcBlockId::new(0),
                    params: vec![],
                    body: vec![],
                    terminator: ArcTerminator::Invoke {
                        dst: ArcVarId::new(2),
                        ty: result_type,
                        func: method,
                        args: vec![ArcVarId::new(0), ArcVarId::new(1)],
                        arg_ownership: vec![],
                        mono_instance_id: None,
                        normal: ArcBlockId::new(1),
                        unwind: ArcBlockId::new(2),
                    },
                },
                ArcBlock {
                    id: ArcBlockId::new(1),
                    params: vec![],
                    body: vec![],
                    terminator: ArcTerminator::Return {
                        value: ArcVarId::new(2),
                    },
                },
                ArcBlock {
                    id: ArcBlockId::new(2),
                    params: vec![],
                    body: vec![],
                    terminator: if catch_handler {
                        ArcTerminator::Jump {
                            target: ArcBlockId::new(3),
                            args: vec![],
                        }
                    } else {
                        ArcTerminator::Resume
                    },
                },
                ArcBlock {
                    id: ArcBlockId::new(3),
                    params: vec![],
                    body: vec![],
                    terminator: ArcTerminator::Unreachable,
                },
            ],
            ..ArcFunction::default()
        }
    }

    fn primitive_function(receiver_type: Idx, result_type: Idx, operation: PrimOp) -> ArcFunction {
        ArcFunction {
            name: Name::from_raw(1),
            return_type: result_type,
            var_types: vec![receiver_type, receiver_type, result_type],
            blocks: vec![ArcBlock {
                id: ArcBlockId::new(0),
                params: vec![],
                body: vec![ArcInstr::Let {
                    dst: ArcVarId::new(2),
                    ty: result_type,
                    value: ArcValue::PrimOp {
                        op: operation,
                        args: vec![ArcVarId::new(0), ArcVarId::new(1)],
                    },
                }],
                terminator: ArcTerminator::Return {
                    value: ArcVarId::new(2),
                },
            }],
            ..ArcFunction::default()
        }
    }

    #[test]
    fn resolved_user_operator_becomes_an_exact_invoke() {
        let interner = StringInterner::new();
        let point_name = interner.intern("Point");
        let field_name = interner.intern("x");
        let target = interner.intern("Point.add");
        let mut pool = Pool::new();
        let point = pool.struct_type(point_name, &[(field_name, Idx::INT)]);
        let method = interner.intern("add");
        let mut function =
            placeholder_function(point, point, PrimOp::Binary(BinaryOp::Add), method, false);
        function.var_reprs = vec![
            ValueRepr::Aggregate,
            ValueRepr::Aggregate,
            ValueRepr::Aggregate,
        ];
        function.var_metadata_state = VariableMetadataState::RepresentationsReady;
        let mut functions = vec![function];
        let Some(source_span) = functions[0].operator_call_facts[0].span else {
            panic!("operator fixture must carry its source span")
        };

        let result =
            rewrite_operator_trait_calls(&mut functions, &pool, &interner, &|receiver, method| {
                (receiver == point && interner.lookup(method) == "add").then_some(target)
            });
        if let Err(errors) = result {
            panic!("operator target should resolve: {errors:?}")
        }

        let function = &functions[0];
        assert!(matches!(
            &function.blocks[0].terminator,
            ArcTerminator::Invoke { dst, func, args, .. }
                if *dst == ArcVarId::new(3)
                    && *func == target
                    && args == &[ArcVarId::new(0), ArcVarId::new(1)]
        ));
        assert!(matches!(
            &function.blocks[1].body[0],
            ArcInstr::Let {
                dst,
                ty,
                value: ArcValue::Var(source),
            } if *dst == ArcVarId::new(2)
                && *ty == point
                && *source == ArcVarId::new(3)
        ));
        assert_eq!(function.spans[1], vec![Some(source_span)]);
        assert_eq!(function.var_types, vec![point, point, point, point]);
        assert_eq!(
            function.var_reprs,
            vec![
                ValueRepr::Aggregate,
                ValueRepr::Aggregate,
                ValueRepr::Aggregate,
                ValueRepr::Aggregate,
            ]
        );
        assert!(function.operator_call_facts.is_empty());
    }

    #[test]
    fn resolved_user_unary_operator_becomes_an_exact_invoke() {
        let interner = StringInterner::new();
        let mut pool = Pool::new();
        let vector = pool.struct_type(
            interner.intern("Vector"),
            &[(interner.intern("value"), Idx::INT)],
        );
        let method = interner.intern("negate");
        let target = interner.intern("Vector.neg");
        let mut function =
            placeholder_function(vector, vector, PrimOp::Unary(UnaryOp::Neg), method, false);
        let ArcTerminator::Invoke { args, .. } = &mut function.blocks[0].terminator else {
            unreachable!()
        };
        args.pop();
        let mut functions = vec![function];

        let result =
            rewrite_operator_trait_calls(&mut functions, &pool, &interner, &|receiver, name| {
                (receiver == vector && name == method).then_some(target)
            });
        if let Err(errors) = result {
            panic!("unary operator target should resolve: {errors:?}")
        }

        assert!(matches!(
            &functions[0].blocks[0].terminator,
            ArcTerminator::Invoke { func, args, .. }
                if *func == target && args == &[ArcVarId::new(0)]
        ));
        assert!(functions[0].operator_call_facts.is_empty());
    }

    #[test]
    fn missing_user_operator_target_fails_without_mutation() {
        let interner = StringInterner::new();
        let toggle_name = interner.intern("Toggle");
        let field_name = interner.intern("value");
        let mut pool = Pool::new();
        let toggle = pool.struct_type(toggle_name, &[(field_name, Idx::INT)]);
        let original = placeholder_function(
            toggle,
            toggle,
            PrimOp::Binary(BinaryOp::Add),
            interner.intern("add"),
            false,
        );
        let mut functions = vec![original.clone()];

        let Err(errors) =
            rewrite_operator_trait_calls(&mut functions, &pool, &interner, &|_, _| None)
        else {
            panic!("missing callable identity did not fail closed")
        };

        assert_eq!(errors.len(), 1);
        assert_eq!(functions[0], original);
    }

    #[test]
    fn builtin_list_add_remains_a_primitive_site() {
        let interner = StringInterner::new();
        let mut pool = Pool::new();
        let list = pool.list(Idx::INT);
        let original = primitive_function(list, list, PrimOp::Binary(BinaryOp::Add));
        let mut functions = vec![original.clone()];

        if let Err(errors) =
            rewrite_operator_trait_calls(&mut functions, &pool, &interner, &|_, _| None)
        {
            panic!("builtin operator should not require a callable target: {errors:?}")
        }

        assert_eq!(functions[0], original);
    }

    #[test]
    fn user_not_equal_calls_exact_eq_then_builtin_not() {
        let interner = StringInterner::new();
        let color_name = interner.intern("Color");
        let tag_name = interner.intern("tag");
        let target = interner.intern("Color.eq");
        let mut pool = Pool::new();
        let color = pool.struct_type(color_name, &[(tag_name, Idx::INT)]);
        let method = interner.intern("eq");
        let mut functions = vec![placeholder_function(
            color,
            Idx::BOOL,
            PrimOp::Binary(BinaryOp::NotEq),
            method,
            false,
        )];

        let result =
            rewrite_operator_trait_calls(&mut functions, &pool, &interner, &|receiver, method| {
                (receiver == color && interner.lookup(method) == "eq").then_some(target)
            });
        if let Err(errors) = result {
            panic!("user inequality must resolve through its exact Eq implementation: {errors:?}")
        }

        assert!(matches!(
            &functions[0].blocks[0].terminator,
            ArcTerminator::Invoke { dst, func, args, .. }
                if *dst == ArcVarId::new(3)
                    && *func == target
                    && args == &[ArcVarId::new(0), ArcVarId::new(1)]
        ));
        assert!(matches!(
            &functions[0].blocks[1].body[0],
            ArcInstr::Let {
                dst,
                ty,
                value: ArcValue::PrimOp {
                    op: PrimOp::Unary(UnaryOp::Not),
                    args,
                },
            } if *dst == ArcVarId::new(2)
                && *ty == Idx::BOOL
                && args == &[ArcVarId::new(3)]
        ));
    }

    /// Drive one `user_ordering_becomes_exact_compare_then_builtin_projection`
    /// case end to end: rewrite a `source_operation` comparison against a
    /// user `Comparable` impl, then assert the exact-compare + builtin
    /// `predicate`-projection shape.
    fn assert_user_ordering_case(
        source_operation: BinaryOp,
        predicate: BinaryOp,
        expected_bound: i64,
    ) {
        let interner = StringInterner::new();
        let pair_name = interner.intern("Pair");
        let field_name = interner.intern("value");
        let target = interner.intern("Pair.compare");
        let method = interner.intern("compare");
        let mut pool = Pool::new();
        let pair = pool.struct_type(pair_name, &[(field_name, Idx::INT)]);
        let mut function = placeholder_function(
            pair,
            Idx::BOOL,
            PrimOp::Binary(source_operation),
            method,
            false,
        );
        function.var_reprs = vec![
            ValueRepr::Aggregate,
            ValueRepr::Aggregate,
            ValueRepr::Scalar,
        ];
        function.var_metadata_state = VariableMetadataState::RepresentationsReady;
        let Some(source_span) = function.operator_call_facts[0].span else {
            panic!("operator fixture must carry its source span")
        };
        let mut functions = vec![function];

        let result =
            rewrite_operator_trait_calls(&mut functions, &pool, &interner, &|receiver, method| {
                (receiver == pair && interner.lookup(method) == "compare").then_some(target)
            });
        if let Err(errors) = result {
            panic!("user ordering must resolve through exact Comparable impl: {errors:?}")
        }

        let function = &functions[0];
        let body = &function.blocks[1].body;
        assert_eq!(body.len(), 3);
        assert_eq!(
            function.var_types,
            vec![pair, pair, Idx::BOOL, Idx::ORDERING, Idx::INT, Idx::INT,]
        );
        assert!(matches!(
            &function.blocks[0].terminator,
            ArcTerminator::Invoke {
                dst,
                ty,
                func,
                args,
                mono_instance_id: None,
                ..
            } if *dst == ArcVarId::new(3)
                && *ty == Idx::ORDERING
                && *func == target
                && args == &[ArcVarId::new(0), ArcVarId::new(1)]
        ));
        assert!(matches!(
            &body[0],
            ArcInstr::Project { dst, ty, value, field }
                if *dst == ArcVarId::new(4)
                    && *ty == Idx::INT
                    && *value == ArcVarId::new(3)
                    && *field == 0
        ));
        assert!(matches!(
            &body[1],
            ArcInstr::Let {
                dst,
                ty,
                value: ArcValue::Literal(LitValue::Int(bound)),
            } if *dst == ArcVarId::new(5)
                && *ty == Idx::INT
                && *bound == expected_bound
        ));
        assert!(matches!(
            &body[2],
            ArcInstr::Let {
                dst,
                ty,
                value: ArcValue::PrimOp { op, args },
            } if *dst == ArcVarId::new(2)
                && *ty == Idx::BOOL
                && *op == PrimOp::Binary(predicate)
                && args == &[ArcVarId::new(4), ArcVarId::new(5)]
        ));
        assert_eq!(function.spans[1], vec![None, None, Some(source_span)]);
        assert_eq!(function.var_reprs.len(), function.var_types.len());
        assert!(function.var_reprs[3..]
            .iter()
            .all(|representation| *representation == ValueRepr::Scalar));
    }

    #[test]
    fn user_ordering_becomes_exact_compare_then_builtin_projection() {
        let cases = [
            (BinaryOp::Lt, BinaryOp::Eq, i64::from(ordering::LESS)),
            (
                BinaryOp::LtEq,
                BinaryOp::NotEq,
                i64::from(ordering::GREATER),
            ),
            (BinaryOp::Gt, BinaryOp::Eq, i64::from(ordering::GREATER)),
            (BinaryOp::GtEq, BinaryOp::NotEq, i64::from(ordering::LESS)),
        ];

        for (source_operation, predicate, expected_bound) in cases {
            assert_user_ordering_case(source_operation, predicate, expected_bound);
        }
    }

    /// Build the `compare_twice` fixture for
    /// `sequential_user_ordering_sites_preserve_each_projection`: two
    /// sequential `Invoke`s of `method` on `pair`-typed operands (`Lt` then
    /// `GtEq`), each with its own normal/unwind block pair.
    fn build_sequential_ordering_fixture(
        interner: &StringInterner,
        pair: Idx,
        method: Name,
    ) -> ArcFunction {
        ArcFunction {
            name: interner.intern("compare_twice"),
            return_type: Idx::BOOL,
            var_types: vec![pair, pair, Idx::BOOL, pair, pair, Idx::BOOL],
            spans: vec![vec![], vec![], vec![], vec![], vec![]],
            operator_call_facts: vec![
                OperatorCallFact {
                    destination: ArcVarId::new(2),
                    receiver: ArcVarId::new(0),
                    operation: PrimOp::Binary(BinaryOp::Lt),
                    span: Some(Span::new(10, 20)),
                },
                OperatorCallFact {
                    destination: ArcVarId::new(5),
                    receiver: ArcVarId::new(3),
                    operation: PrimOp::Binary(BinaryOp::GtEq),
                    span: Some(Span::new(30, 40)),
                },
            ],
            blocks: vec![
                ArcBlock {
                    id: ArcBlockId::new(0),
                    params: vec![],
                    body: vec![],
                    terminator: ArcTerminator::Invoke {
                        dst: ArcVarId::new(2),
                        ty: Idx::BOOL,
                        func: method,
                        args: vec![ArcVarId::new(0), ArcVarId::new(1)],
                        arg_ownership: vec![],
                        mono_instance_id: None,
                        normal: ArcBlockId::new(1),
                        unwind: ArcBlockId::new(2),
                    },
                },
                ArcBlock {
                    id: ArcBlockId::new(1),
                    params: vec![],
                    body: vec![],
                    terminator: ArcTerminator::Invoke {
                        dst: ArcVarId::new(5),
                        ty: Idx::BOOL,
                        func: method,
                        args: vec![ArcVarId::new(3), ArcVarId::new(4)],
                        arg_ownership: vec![],
                        mono_instance_id: None,
                        normal: ArcBlockId::new(3),
                        unwind: ArcBlockId::new(4),
                    },
                },
                ArcBlock {
                    id: ArcBlockId::new(2),
                    params: vec![],
                    body: vec![],
                    terminator: ArcTerminator::Resume,
                },
                ArcBlock {
                    id: ArcBlockId::new(3),
                    params: vec![],
                    body: vec![],
                    terminator: ArcTerminator::Return {
                        value: ArcVarId::new(5),
                    },
                },
                ArcBlock {
                    id: ArcBlockId::new(4),
                    params: vec![],
                    body: vec![],
                    terminator: ArcTerminator::Resume,
                },
            ],
            ..ArcFunction::default()
        }
    }

    #[test]
    fn sequential_user_ordering_sites_preserve_each_projection() {
        let interner = StringInterner::new();
        let mut pool = Pool::new();
        let pair = pool.struct_type(
            interner.intern("Pair"),
            &[(interner.intern("value"), Idx::INT)],
        );
        let method = interner.intern("compare");
        let target = interner.intern("Pair.compare");
        let mut functions = vec![build_sequential_ordering_fixture(&interner, pair, method)];

        let result =
            rewrite_operator_trait_calls(&mut functions, &pool, &interner, &|receiver, name| {
                (receiver == pair && name == method).then_some(target)
            });
        if let Err(errors) = result {
            panic!("sequential ordering sites must both resolve: {errors:?}")
        }

        let function = &functions[0];
        assert_eq!(function.blocks[1].body.len(), 3);
        assert_eq!(function.blocks[3].body.len(), 3);
        assert!(matches!(
            &function.blocks[1].body[2],
            ArcInstr::Let { dst, .. } if *dst == ArcVarId::new(2)
        ));
        assert!(matches!(
            &function.blocks[1].terminator,
            ArcTerminator::Invoke { dst, func, .. }
                if *dst == ArcVarId::new(9) && *func == target
        ));
        assert!(matches!(
            &function.blocks[3].body[2],
            ArcInstr::Let { dst, .. } if *dst == ArcVarId::new(5)
        ));
        assert!(function.operator_call_facts.is_empty());
    }

    #[test]
    fn specialized_builtin_placeholder_returns_to_primitive_and_keeps_catch() {
        let interner = StringInterner::new();
        let pool = Pool::new();
        let method = interner.intern("add");
        let mut functions = vec![placeholder_function(
            Idx::INT,
            Idx::INT,
            PrimOp::Binary(BinaryOp::Add),
            method,
            true,
        )];
        let Some(source_span) = functions[0].operator_call_facts[0].span else {
            panic!("operator fixture must carry its source span")
        };

        let result = rewrite_operator_trait_calls(&mut functions, &pool, &interner, &|_, _| {
            panic!("builtin specialization must not query a user target")
        });
        if let Err(errors) = result {
            panic!("specialized builtin operator must return to PrimOp: {errors:?}")
        }

        let function = &functions[0];
        let body = &function.blocks[0].body;
        assert_eq!(body.len(), 1);
        assert!(matches!(
            &body[0],
            ArcInstr::Let {
                dst,
                ty,
                value: ArcValue::PrimOp {
                    op: PrimOp::Binary(BinaryOp::Add),
                    args,
                },
            } if *dst == ArcVarId::new(2)
                && *ty == Idx::INT
                && args == &[ArcVarId::new(0), ArcVarId::new(1)]
        ));
        assert!(matches!(
            &function.blocks[0].terminator,
            ArcTerminator::Jump { target, args }
                if *target == ArcBlockId::new(1) && args.is_empty()
        ));
        assert_eq!(function.spans[0], vec![Some(source_span)]);
        assert_eq!(
            function.catch_scoped_checked_ops,
            vec![(ArcVarId::new(2), ArcBlockId::new(3))]
        );
        assert!(function.blocks[2].body.is_empty());
        assert!(matches!(
            function.blocks[2].terminator,
            ArcTerminator::Unreachable
        ));
        assert!(function.operator_call_facts.is_empty());
    }

    #[test]
    fn specialized_builtin_unary_placeholder_keeps_checked_catch() {
        let interner = StringInterner::new();
        let pool = Pool::new();
        let mut function = placeholder_function(
            Idx::INT,
            Idx::INT,
            PrimOp::Unary(UnaryOp::Neg),
            interner.intern("negate"),
            true,
        );
        let ArcTerminator::Invoke { args, .. } = &mut function.blocks[0].terminator else {
            unreachable!()
        };
        args.pop();
        let mut functions = vec![function];

        let result = rewrite_operator_trait_calls(&mut functions, &pool, &interner, &|_, _| {
            panic!("builtin unary specialization must not query a user target")
        });
        if let Err(errors) = result {
            panic!("specialized builtin unary operator must return to PrimOp: {errors:?}")
        }

        let function = &functions[0];
        assert!(matches!(
            &function.blocks[0].body[0],
            ArcInstr::Let {
                dst,
                value: ArcValue::PrimOp {
                    op: PrimOp::Unary(UnaryOp::Neg),
                    args,
                },
                ..
            } if *dst == ArcVarId::new(2) && args == &[ArcVarId::new(0)]
        ));
        assert_eq!(
            function.catch_scoped_checked_ops,
            vec![(ArcVarId::new(2), ArcBlockId::new(3))]
        );
        assert!(matches!(
            function.blocks[2].terminator,
            ArcTerminator::Unreachable
        ));
    }

    #[test]
    fn duplicate_operator_facts_fail_without_mutation() {
        let interner = StringInterner::new();
        let mut pool = Pool::new();
        let point = pool.struct_type(
            interner.intern("Point"),
            &[(interner.intern("value"), Idx::INT)],
        );
        let mut original = placeholder_function(
            point,
            point,
            PrimOp::Binary(BinaryOp::Add),
            interner.intern("add"),
            false,
        );
        original
            .operator_call_facts
            .push(original.operator_call_facts[0].clone());
        let mut functions = vec![original.clone()];

        let result = rewrite_operator_trait_calls(&mut functions, &pool, &interner, &|_, _| {
            Some(interner.intern("Point.add"))
        });

        assert!(result.is_err());
        assert_eq!(functions[0], original);
    }

    #[test]
    fn orphaned_operator_fact_fails_without_mutation() {
        let interner = StringInterner::new();
        let mut pool = Pool::new();
        let point = pool.struct_type(
            interner.intern("Point"),
            &[(interner.intern("value"), Idx::INT)],
        );
        let mut original = placeholder_function(
            point,
            point,
            PrimOp::Binary(BinaryOp::Add),
            interner.intern("add"),
            false,
        );
        original.blocks[0].terminator = ArcTerminator::Jump {
            target: ArcBlockId::new(1),
            args: Vec::new(),
        };
        let mut functions = vec![original.clone()];

        let result = rewrite_operator_trait_calls(&mut functions, &pool, &interner, &|_, _| {
            Some(interner.intern("Point.add"))
        });

        assert!(result.is_err());
        assert_eq!(functions[0], original);
    }

    /// Build every malformed-operator-`Invoke`-metadata variant of `base`
    /// exercised by `malformed_operator_invoke_metadata_fails_without_mutation`
    /// — each variant introduces exactly one structural defect (wrong arity,
    /// wrong result type, wrong ownership-list arity, a shared/self-looping
    /// normal/unwind edge, a nonempty or argument-carrying unwind block, an
    /// invalid or parameterized unwind handler) that the rewrite must reject
    /// without mutating the function.
    fn build_malformed_operator_invoke_variants(base: ArcFunction, point: Idx) -> Vec<ArcFunction> {
        let mut malformed = Vec::new();

        let mut wrong_arity = base.clone();
        let ArcTerminator::Invoke { args, .. } = &mut wrong_arity.blocks[0].terminator else {
            unreachable!()
        };
        args.pop();
        malformed.push(wrong_arity);

        let mut wrong_result_type = base.clone();
        let ArcTerminator::Invoke { ty, .. } = &mut wrong_result_type.blocks[0].terminator else {
            unreachable!()
        };
        *ty = Idx::BOOL;
        malformed.push(wrong_result_type);

        let mut wrong_ownership_arity = base.clone();
        let ArcTerminator::Invoke { arg_ownership, .. } =
            &mut wrong_ownership_arity.blocks[0].terminator
        else {
            unreachable!()
        };
        arg_ownership.push(crate::ir::ArgOwnership::Owned);
        malformed.push(wrong_ownership_arity);

        let mut shared_edges = base.clone();
        let ArcTerminator::Invoke { normal, unwind, .. } = &mut shared_edges.blocks[0].terminator
        else {
            unreachable!()
        };
        *unwind = *normal;
        malformed.push(shared_edges);

        let mut self_normal = base.clone();
        let ArcTerminator::Invoke { normal, .. } = &mut self_normal.blocks[0].terminator else {
            unreachable!()
        };
        *normal = ArcBlockId::new(0);
        malformed.push(self_normal);

        let mut self_unwind = base.clone();
        let ArcTerminator::Invoke { unwind, .. } = &mut self_unwind.blocks[0].terminator else {
            unreachable!()
        };
        *unwind = ArcBlockId::new(0);
        malformed.push(self_unwind);

        let mut shared_normal = base.clone();
        shared_normal.blocks[3].terminator = ArcTerminator::Jump {
            target: ArcBlockId::new(1),
            args: Vec::new(),
        };
        malformed.push(shared_normal);

        let mut shared_unwind = base.clone();
        shared_unwind.blocks[3].terminator = ArcTerminator::Jump {
            target: ArcBlockId::new(2),
            args: Vec::new(),
        };
        malformed.push(shared_unwind);

        let mut nonempty_unwind = base.clone();
        nonempty_unwind.blocks[2].body.push(ArcInstr::Let {
            dst: ArcVarId::new(2),
            ty: point,
            value: ArcValue::Var(ArcVarId::new(0)),
        });
        malformed.push(nonempty_unwind);

        let mut argument_carrying_unwind = base.clone();
        argument_carrying_unwind.blocks[2].terminator = ArcTerminator::Jump {
            target: ArcBlockId::new(3),
            args: vec![ArcVarId::new(0)],
        };
        malformed.push(argument_carrying_unwind);

        for target in [0, 1, 2, 99] {
            let mut invalid_handler = base.clone();
            invalid_handler.blocks[2].terminator = ArcTerminator::Jump {
                target: ArcBlockId::new(target),
                args: Vec::new(),
            };
            malformed.push(invalid_handler);
        }

        let mut parameterized_handler = base.clone();
        parameterized_handler.blocks[2].terminator = ArcTerminator::Jump {
            target: ArcBlockId::new(3),
            args: Vec::new(),
        };
        parameterized_handler.blocks[3]
            .params
            .push((ArcVarId::new(0), point));
        malformed.push(parameterized_handler);

        let mut invalid_unwind = base;
        let ArcTerminator::Invoke { unwind, .. } = &mut invalid_unwind.blocks[0].terminator else {
            unreachable!()
        };
        *unwind = ArcBlockId::new(99);
        malformed.push(invalid_unwind);

        malformed
    }

    #[test]
    fn malformed_operator_invoke_metadata_fails_without_mutation() {
        let interner = StringInterner::new();
        let mut pool = Pool::new();
        let point = pool.struct_type(
            interner.intern("Point"),
            &[(interner.intern("value"), Idx::INT)],
        );
        let method = interner.intern("add");
        let base = placeholder_function(point, point, PrimOp::Binary(BinaryOp::Add), method, false);
        let malformed = build_malformed_operator_invoke_variants(base, point);

        for original in malformed {
            let mut functions = vec![original.clone()];
            let result = rewrite_operator_trait_calls(&mut functions, &pool, &interner, &|_, _| {
                Some(interner.intern("Point.add"))
            });

            assert!(result.is_err());
            assert_eq!(functions[0], original);
        }
    }
}
