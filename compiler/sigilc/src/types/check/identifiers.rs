// Identifier and config type checking

use crate::ast::TypeExpr;

use super::super::context::TypeContext;

pub fn check_ident(name: &str, ctx: &TypeContext) -> Result<TypeExpr, String> {
    if let Some(ty) = ctx.lookup_local(name) {
        Ok(ty.clone())
    } else if let Some(sig) = ctx.lookup_function(name) {
        // Return function type
        Ok(sig.return_type.clone())
    } else {
        Err(format!("Unknown identifier: {}", name))
    }
}

pub fn check_config(name: &str, ctx: &TypeContext) -> Result<TypeExpr, String> {
    ctx.lookup_config(name)
        .cloned()
        .ok_or_else(|| format!("Unknown config: ${}", name))
}
