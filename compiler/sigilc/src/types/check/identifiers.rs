// Identifier and config type checking

use crate::ast::TypeExpr;
use crate::errors::{codes::ErrorCode, Diagnostic, DiagnosticResult};

use super::super::context::TypeContext;

pub fn check_ident(name: &str, ctx: &TypeContext) -> DiagnosticResult<TypeExpr> {
    if let Some(ty) = ctx.lookup_local(name) {
        Ok(ty.clone())
    } else if let Some(sig) = ctx.lookup_function(name) {
        // Return function type
        Ok(sig.return_type.clone())
    } else {
        Err(
            Diagnostic::error(ErrorCode::E3002, format!("unknown identifier '{}'", name))
                .with_label(ctx.make_span(0..0), "not found in this scope"),
        )
    }
}

pub fn check_config(name: &str, ctx: &TypeContext) -> DiagnosticResult<TypeExpr> {
    ctx.lookup_config(name).cloned().ok_or_else(|| {
        Diagnostic::error(ErrorCode::E3002, format!("unknown config '${}'", name))
            .with_label(ctx.make_span(0..0), "not defined")
    })
}
