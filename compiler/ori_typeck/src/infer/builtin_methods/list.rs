//! List method handler.

use ori_diagnostic::ErrorCode;
use ori_ir::{Span, StringInterner};
use ori_types::{InferenceContext, Type};

use super::{BuiltinMethodHandler, MethodTypeError, MethodTypeResult};

/// Type checking for list methods.
pub struct ListMethodHandler;

impl BuiltinMethodHandler for ListMethodHandler {
    fn handles(&self, receiver_ty: &Type) -> bool {
        matches!(receiver_ty, Type::List(_))
    }

    fn check(
        &self,
        ctx: &mut InferenceContext,
        interner: &StringInterner,
        receiver_ty: &Type,
        method: &str,
        args: &[Type],
        _span: Span,
    ) -> MethodTypeResult {
        let Type::List(elem_ty) = receiver_ty else {
            unreachable!("handles() verified type is List");
        };

        match method {
            "len" => MethodTypeResult::Ok(Type::Int),
            "is_empty" | "contains" => MethodTypeResult::Ok(Type::Bool),
            "first" | "last" | "pop" | "find" => {
                MethodTypeResult::Ok(Type::Option(elem_ty.clone()))
            }
            "push" => MethodTypeResult::Ok(Type::Unit),
            "map" => {
                let result_elem = ctx.fresh_var();
                MethodTypeResult::Ok(Type::List(Box::new(result_elem)))
            }
            "filter" | "reverse" | "sort" => MethodTypeResult::Ok(Type::List(elem_ty.clone())),
            "fold" => {
                if let Some(acc_ty) = args.first() {
                    MethodTypeResult::Ok(acc_ty.clone())
                } else {
                    MethodTypeResult::Ok(ctx.fresh_var())
                }
            }
            "compare" => MethodTypeResult::Ok(Type::Named(interner.intern("Ordering"))),
            _ => MethodTypeResult::Err(MethodTypeError::new(
                format!("unknown method `{method}` for type `[T]`"),
                ErrorCode::E2002,
            )),
        }
    }
}
