//! Resolve user-defined operator primitives into ordinary callable ARC IR.

use ori_ir::{Name, StringInterner};
use ori_types::{Idx, Pool};

use crate::ir::{ArcFunction, ArcInstr, ArcValue, ArcVarId, ArgOwnership, PrimOp};

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
    instruction: usize,
    destination: ArcVarId,
    result_type: Idx,
    target: Name,
    arguments: Vec<ArcVarId>,
}

/// Rewrite every resolved user-defined operator to an ordinary direct call.
///
/// Builtin receivers remain `PrimOp` sites and are resolved by the typed
/// primitive descriptor seam. Overloaded arithmetic and user-defined equality
/// must resolve to one exact callable identity supplied by the realization
/// owner; otherwise this pass fails without mutating any function. The
/// resulting `Apply` sites flow through normal interprocedural
/// `MemoryContract` analysis and backend call projection.
pub fn rewrite_operator_trait_calls(
    functions: &mut [ArcFunction],
    pool: &Pool,
    interner: &StringInterner,
    resolve_target: &impl Fn(Idx, Name) -> Option<Name>,
) -> Result<(), Vec<OperatorCallResolutionError>> {
    let mut rewrites = Vec::new();
    let mut errors = Vec::new();

    for (function_index, function) in functions.iter().enumerate() {
        for (block_index, block) in function.blocks.iter().enumerate() {
            for (instruction_index, instruction) in block.body.iter().enumerate() {
                let ArcInstr::Let {
                    dst,
                    ty,
                    value: ArcValue::PrimOp { op, args },
                } = instruction
                else {
                    continue;
                };
                let Some(&receiver) = args.first() else {
                    errors.push(error(function.name, *dst, *op, Idx::ERROR));
                    continue;
                };
                let Some(&receiver_type) = function.var_types.get(receiver.index()) else {
                    errors.push(error(function.name, *dst, *op, Idx::ERROR));
                    continue;
                };
                let receiver_type = pool.resolve_fully(receiver_type);
                if pool.builtin_type_tag(receiver_type).is_some() {
                    continue;
                }
                let method = match op {
                    PrimOp::Binary(ori_ir::BinaryOp::Eq) => Some("eq"),
                    PrimOp::Binary(operation) => operation.trait_method_name(),
                    PrimOp::Unary(operation) => operation.trait_method_name(),
                };
                let Some(method) = method else {
                    // Ordered comparisons, logical operations, ranges,
                    // coalescing, and the temporary NotEq lowering remain
                    // typed structural primitives. Eq itself is an exact
                    // user-callable operation per Clause 14.5.4.
                    continue;
                };
                let method = interner.intern(method);
                let Some(target) = resolve_target(receiver_type, method) else {
                    errors.push(error(function.name, *dst, *op, receiver_type));
                    continue;
                };
                let expected_arity = match op {
                    PrimOp::Binary(_) => 2,
                    PrimOp::Unary(_) => 1,
                };
                if args.len() != expected_arity {
                    errors.push(error(function.name, *dst, *op, receiver_type));
                    continue;
                }
                rewrites.push(Rewrite {
                    function: function_index,
                    block: block_index,
                    instruction: instruction_index,
                    destination: *dst,
                    result_type: *ty,
                    target,
                    arguments: args.clone(),
                });
            }
        }
    }

    if !errors.is_empty() {
        return Err(errors);
    }

    for rewrite in rewrites {
        let argument_count = rewrite.arguments.len();
        functions[rewrite.function].blocks[rewrite.block].body[rewrite.instruction] =
            ArcInstr::Apply {
                dst: rewrite.destination,
                ty: rewrite.result_type,
                func: rewrite.target,
                args: rewrite.arguments,
                arg_ownership: vec![ArgOwnership::Owned; argument_count],
                mono_instance_id: None,
            };
    }
    Ok(())
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
    use crate::ir::{ArcBlock, ArcBlockId, ArcTerminator};
    use ori_ir::{BinaryOp, UnaryOp};

    fn function(receiver_type: Idx, operation: PrimOp, arguments: Vec<ArcVarId>) -> ArcFunction {
        ArcFunction {
            name: Name::from_raw(1),
            var_types: vec![receiver_type, receiver_type, receiver_type],
            blocks: vec![ArcBlock {
                id: ArcBlockId::new(0),
                params: vec![],
                body: vec![ArcInstr::Let {
                    dst: ArcVarId::new(2),
                    ty: receiver_type,
                    value: ArcValue::PrimOp {
                        op: operation,
                        args: arguments,
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
    fn resolved_user_operator_becomes_an_exact_apply() {
        let interner = StringInterner::new();
        let point_name = interner.intern("Point");
        let field_name = interner.intern("x");
        let target = interner.intern("Point.add");
        let mut pool = Pool::new();
        let point = pool.struct_type(point_name, &[(field_name, Idx::INT)]);
        let mut functions = vec![function(
            point,
            PrimOp::Binary(BinaryOp::Add),
            vec![ArcVarId::new(0), ArcVarId::new(1)],
        )];

        let result =
            rewrite_operator_trait_calls(&mut functions, &pool, &interner, &|receiver, method| {
                (receiver == point && interner.lookup(method) == "add").then_some(target)
            });
        if let Err(errors) = result {
            panic!("operator target should resolve: {errors:?}")
        }

        assert!(matches!(
            &functions[0].blocks[0].body[0],
            ArcInstr::Apply { func, args, .. }
                if *func == target && args == &[ArcVarId::new(0), ArcVarId::new(1)]
        ));
    }

    #[test]
    fn missing_user_operator_target_fails_without_mutation() {
        let interner = StringInterner::new();
        let toggle_name = interner.intern("Toggle");
        let field_name = interner.intern("on");
        let mut pool = Pool::new();
        let toggle = pool.struct_type(toggle_name, &[(field_name, Idx::BOOL)]);
        let original = function(toggle, PrimOp::Unary(UnaryOp::Not), vec![ArcVarId::new(0)]);
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
        let original = function(
            list,
            PrimOp::Binary(BinaryOp::Add),
            vec![ArcVarId::new(0), ArcVarId::new(1)],
        );
        let mut functions = vec![original.clone()];

        if let Err(errors) =
            rewrite_operator_trait_calls(&mut functions, &pool, &interner, &|_, _| None)
        {
            panic!("builtin operator should not require a callable target: {errors:?}")
        }

        assert_eq!(functions[0], original);
    }

    #[test]
    fn user_equality_becomes_an_exact_apply() {
        let interner = StringInterner::new();
        let color_name = interner.intern("Color");
        let tag_name = interner.intern("tag");
        let target = interner.intern("Color.eq");
        let mut pool = Pool::new();
        let color = pool.struct_type(color_name, &[(tag_name, Idx::INT)]);
        let mut functions = vec![function(
            color,
            PrimOp::Binary(BinaryOp::Eq),
            vec![ArcVarId::new(0), ArcVarId::new(1)],
        )];

        let result =
            rewrite_operator_trait_calls(&mut functions, &pool, &interner, &|receiver, method| {
                (receiver == color && interner.lookup(method) == "eq").then_some(target)
            });
        if let Err(errors) = result {
            panic!("user equality must resolve through its exact Eq implementation: {errors:?}")
        }

        assert!(matches!(
            &functions[0].blocks[0].body[0],
            ArcInstr::Apply { func, args, .. }
                if *func == target && args == &[ArcVarId::new(0), ArcVarId::new(1)]
        ));
    }
}
