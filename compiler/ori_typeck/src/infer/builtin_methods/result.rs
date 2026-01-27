//! Result method handler.

use ori_diagnostic::ErrorCode;
use ori_ir::{Span, StringInterner};
use ori_types::{InferenceContext, Type};

use super::{BuiltinMethodHandler, MethodTypeError, MethodTypeResult};

/// Type checking for Result methods.
pub struct ResultMethodHandler;

impl BuiltinMethodHandler for ResultMethodHandler {
    fn handles(&self, receiver_ty: &Type) -> bool {
        matches!(receiver_ty, Type::Result { .. })
    }

    fn check(
        &self,
        ctx: &mut InferenceContext,
        _interner: &StringInterner,
        receiver_ty: &Type,
        method: &str,
        _args: &[Type],
        _span: Span,
    ) -> MethodTypeResult {
        let Type::Result {
            ok: ok_ty,
            err: err_ty,
        } = receiver_ty
        else {
            unreachable!("handles() verified type is Result");
        };

        match method {
            "is_ok" | "is_err" => MethodTypeResult::Ok(Type::Bool),
            "unwrap" | "unwrap_or" => MethodTypeResult::Ok((**ok_ty).clone()),
            "unwrap_err" => MethodTypeResult::Ok((**err_ty).clone()),
            "ok" => MethodTypeResult::Ok(Type::Option(ok_ty.clone())),
            "err" => MethodTypeResult::Ok(Type::Option(err_ty.clone())),
            "map" | "and_then" => {
                let result_ok = ctx.fresh_var();
                MethodTypeResult::Ok(Type::Result {
                    ok: Box::new(result_ok),
                    err: err_ty.clone(),
                })
            }
            "map_err" => {
                let result_err = ctx.fresh_var();
                MethodTypeResult::Ok(Type::Result {
                    ok: ok_ty.clone(),
                    err: Box::new(result_err),
                })
            }
            _ => MethodTypeResult::Err(MethodTypeError::new(
                format!("unknown method `{method}` for type `Result<T, E>`"),
                ErrorCode::E2002,
            )),
        }
    }
}
