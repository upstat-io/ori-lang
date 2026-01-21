// Type checker for Sigil
// Validates types and produces a typed AST or TIR
// STRICT: No unknown types allowed - all types must be determined at compile time

mod check;
mod check_pattern;
mod compat;
mod context;
pub mod lower;

pub use check::{check_block_expr, check_expr, check_expr_with_hint};
pub use check::lambdas::check_lambda;
pub use compat::{infer_type, is_numeric, is_type_parameter, types_compatible};
pub use context::{FunctionSig, TypeContext};
pub use lower::{type_expr_to_type, Lowerer};

use crate::ast::*;
use crate::ir::TModule;

/// Type-checked AST (same structure, but verified)
pub type TypedModule = Module;

/// Main entry point: type check an entire module
pub fn check(module: Module) -> Result<TypedModule, String> {
    let mut ctx = TypeContext::new();

    // First pass: collect all type and function definitions
    for item in &module.items {
        match item {
            Item::TypeDef(td) => {
                ctx.define_type(td.name.clone(), td.clone());
            }
            Item::Function(fd) => {
                let sig = FunctionSig {
                    type_params: fd.type_params.clone(),
                    params: fd
                        .params
                        .iter()
                        .map(|p| (p.name.clone(), p.ty.clone()))
                        .collect(),
                    return_type: fd.return_type.clone(),
                };
                ctx.define_function(fd.name.clone(), sig);
            }
            Item::Config(cd) => {
                let ty = if let Some(t) = cd.ty.clone() {
                    t
                } else {
                    infer_type(&cd.value).map_err(|e| format!("Config '{}': {}", cd.name, e))?
                };
                ctx.define_config(cd.name.clone(), ty);
            }
            Item::Use(_) => {
                // TODO: Handle imports
            }
            Item::Test(_) => {
                // Tests are checked separately
            }
        }
    }

    // Second pass: type check all expressions
    for item in &module.items {
        match item {
            Item::Function(fd) => {
                check_function(fd, &mut ctx)?;
            }
            Item::Config(cd) => {
                check_config(cd, &ctx)?;
            }
            Item::Test(td) => {
                check_test(td, &ctx)?;
            }
            _ => {}
        }
    }

    Ok(module)
}

/// Type check and lower a module to TIR
/// This combines type checking with IR generation
pub fn check_and_lower(module: Module) -> Result<TModule, String> {
    // First, do the regular type checking
    let mut ctx = TypeContext::new();

    // First pass: collect all type and function definitions
    for item in &module.items {
        match item {
            Item::TypeDef(td) => {
                ctx.define_type(td.name.clone(), td.clone());
            }
            Item::Function(fd) => {
                let sig = FunctionSig {
                    type_params: fd.type_params.clone(),
                    params: fd
                        .params
                        .iter()
                        .map(|p| (p.name.clone(), p.ty.clone()))
                        .collect(),
                    return_type: fd.return_type.clone(),
                };
                ctx.define_function(fd.name.clone(), sig);
            }
            Item::Config(cd) => {
                let ty = if let Some(t) = cd.ty.clone() {
                    t
                } else {
                    infer_type(&cd.value).map_err(|e| format!("Config '{}': {}", cd.name, e))?
                };
                ctx.define_config(cd.name.clone(), ty);
            }
            Item::Use(_) => {
                // TODO: Handle imports
            }
            Item::Test(_) => {
                // Tests are checked separately
            }
        }
    }

    // Second pass: type check all expressions
    for item in &module.items {
        match item {
            Item::Function(fd) => {
                check_function(fd, &mut ctx)?;
            }
            Item::Config(cd) => {
                check_config(cd, &ctx)?;
            }
            Item::Test(td) => {
                check_test(td, &ctx)?;
            }
            _ => {}
        }
    }

    // Third pass: lower to TIR
    Lowerer::lower_module(&module, &ctx)
}

fn check_function(fd: &FunctionDef, ctx: &mut TypeContext) -> Result<(), String> {
    // Save old state
    let old_locals = ctx.save_locals();
    let old_return_type = ctx.current_return_type();

    // Set current return type for self() calls
    ctx.set_current_return_type(fd.return_type.clone());

    // Add parameters to local scope
    for param in &fd.params {
        ctx.define_local(param.name.clone(), param.ty.clone());
    }

    // Check body expression with return type as hint (for lambdas that are directly returned)
    let body_type = check_expr_with_hint(&fd.body, ctx, Some(&fd.return_type))?;

    // Verify return type matches
    if !types_compatible(&body_type, &fd.return_type, ctx) {
        return Err(format!(
            "Function '{}' returns {:?} but body has type {:?}",
            fd.name, fd.return_type, body_type
        ));
    }

    // Restore state
    ctx.restore_locals(old_locals);
    if let Some(ty) = old_return_type {
        ctx.set_current_return_type(ty);
    } else {
        ctx.clear_current_return_type();
    }

    Ok(())
}

fn check_config(cd: &ConfigDef, ctx: &TypeContext) -> Result<(), String> {
    let value_type = check_expr(&cd.value, ctx)?;

    if let Some(ref declared) = cd.ty {
        if !types_compatible(&value_type, declared, ctx) {
            return Err(format!(
                "Config '{}' declared as {:?} but value has type {:?}",
                cd.name, declared, value_type
            ));
        }
    }

    Ok(())
}

fn check_test(td: &TestDef, ctx: &TypeContext) -> Result<(), String> {
    // Check that the target function exists
    if ctx.lookup_function(&td.target).is_none() {
        return Err(format!(
            "Test '{}' references unknown function '@{}'",
            td.name, td.target
        ));
    }

    // Type check the test body
    check_expr(&td.body, ctx)?;

    Ok(())
}

#[cfg(test)]
mod tests;
