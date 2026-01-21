// Type checker for Sigil
// Validates types and produces a typed AST or TIR
// STRICT: No unknown types allowed - all types must be determined at compile time

mod builtins;
mod check;
pub mod check_pattern;
mod compat;
pub mod context;
pub mod diagnostics;
pub mod lower;
mod registries;
mod scope;
pub mod traits;

pub use check::{check_block_expr, check_expr, check_expr_with_hint};
pub use check::lambdas::check_lambda;
pub use compat::{
    get_function_return_type, get_iterable_element_type, get_list_element_type,
    infer_type, is_numeric, is_type_parameter, types_compatible,
};
pub use context::{FunctionSig, TypeContext};
pub use registries::{FunctionSig as FunctionSignature, TypeParamBound}; // Aliases for direct access
pub use diagnostics::{format_type, TypeResultExt};
pub use scope::LocalBinding;
pub use lower::{type_expr_to_type, Lowerer};

use crate::ast::*;
use crate::errors::{Diagnostic, DiagnosticCollector, DiagnosticResult};
use crate::errors::codes::ErrorCode;
use crate::ir::TModule;
use crate::modules::{ModuleGraph, get_imported_items};
use std::path::Path;

/// Type-checked AST (same structure, but verified)
pub type TypedModule = Module;

/// Internal: Collect definitions and type check a module
fn collect_and_check_module(module: &Module) -> DiagnosticResult<TypeContext> {
    let mut ctx = TypeContext::with_filename(&module.name);
    let mut collector = DiagnosticCollector::new();

    // First pass: handle imports to register imported symbols
    let source_path = Path::new(&module.name);
    let root_dir = source_path.parent().unwrap_or(Path::new("."));
    let mut module_graph = ModuleGraph::new(root_dir);

    for item in &module.items {
        if let Item::Use(use_def) = item {
            // Resolve the import path
            match module_graph.resolve_import(source_path, &use_def.path) {
                Ok(import_path) => {
                    // Load the imported module
                    match module_graph.load_module(&import_path) {
                        Ok(loaded) => {
                            // Get the items to import
                            match get_imported_items(use_def, &loaded.module) {
                                Ok(imported_items) => {
                                    // Register imported items
                                    for imported_item in imported_items {
                                        register_item_in_context(imported_item, &mut ctx, use_def, &mut collector);
                                    }
                                }
                                Err(msg) => {
                                    collector.push(Diagnostic::error(ErrorCode::E3006, msg)
                                        .with_label(ctx.make_span(use_def.span.clone()), "import error"));
                                }
                            }
                        }
                        Err(diag) => {
                            collector.push(diag.with_label(ctx.make_span(use_def.span.clone()), "failed to load module"));
                        }
                    }
                }
                Err(msg) => {
                    collector.push(Diagnostic::error(ErrorCode::E3006, msg)
                        .with_label(ctx.make_span(use_def.span.clone()), "cannot resolve import"));
                }
            }
        }
    }

    // Second pass: collect all type and function definitions from this module
    for item in &module.items {
        match item {
            Item::TypeDef(td) => {
                ctx.define_type(td.name.clone(), td.clone());
            }
            Item::Function(fd) => {
                let sig = FunctionSig {
                    type_params: fd.type_params.clone(),
                    type_param_bounds: collect_type_param_bounds(fd),
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
                match cd.ty.clone().map_or_else(
                    || infer_type(&cd.value.expr).map_err(|e| format!("Config '{}': {}", cd.name, e)),
                    Ok,
                ) {
                    Ok(ty) => ctx.define_config(cd.name.clone(), ty),
                    Err(msg) => {
                        collector.push(Diagnostic::error(ErrorCode::E3005, msg)
                            .with_label(ctx.make_span(cd.value.span.clone()), "cannot infer type"));
                    }
                }
            }
            Item::Use(_) => {
                // Already handled in first pass
            }
            Item::Test(_) => {
                // Tests are checked separately
            }
            Item::Trait(trait_def) => {
                // Register trait in type context (trait system integration)
                // For now, just register the trait name as a type
                ctx.define_type(
                    trait_def.name.clone(),
                    TypeDef {
                        public: trait_def.public,
                        name: trait_def.name.clone(),
                        params: trait_def.type_params.clone(),
                        kind: TypeDefKind::Alias(TypeExpr::Named(trait_def.name.clone())),
                        span: trait_def.span.clone(),
                    },
                );
            }
            Item::Impl(impl_block) => {
                // Register impl block methods
                for method in &impl_block.methods {
                    let sig = FunctionSig {
                        type_params: method.type_params.clone(),
                        type_param_bounds: collect_type_param_bounds(method),
                        params: method
                            .params
                            .iter()
                            .map(|p| (p.name.clone(), p.ty.clone()))
                            .collect(),
                        return_type: method.return_type.clone(),
                    };
                    // Register with qualified name: TypeName::method_name
                    let type_name = match &impl_block.for_type {
                        TypeExpr::Named(n) => n.clone(),
                        _ => "Unknown".to_string(),
                    };
                    ctx.define_function(format!("{}::{}", type_name, method.name), sig);
                }
            }
        }
    }

    // Second pass: type check all expressions (continue on errors)
    for item in &module.items {
        match item {
            Item::Function(fd) => {
                if let Err(msg) = check_function(fd, &mut ctx) {
                    collector.push(Diagnostic::error(ErrorCode::E3001, msg)
                        .with_label(ctx.make_span(fd.body.span.clone()), "type error in function body"));
                }
            }
            Item::Config(cd) => {
                if let Err(msg) = check_config(cd, &ctx) {
                    collector.push(Diagnostic::error(ErrorCode::E3001, msg)
                        .with_label(ctx.make_span(cd.value.span.clone()), "type error in config value"));
                }
            }
            Item::Test(td) => {
                if let Err(msg) = check_test(td, &ctx) {
                    collector.push(Diagnostic::error(ErrorCode::E3001, msg)
                        .with_label(ctx.make_span(td.body.span.clone()), "type error in test body"));
                }
            }
            _ => {}
        }
    }

    // If we accumulated errors, return all of them
    if collector.has_errors() {
        let errors: Vec<_> = collector.into_diagnostics();
        // Return first error for backwards compatibility with DiagnosticResult
        // The full error list is available through check_with_all_errors()
        Err(errors.into_iter().next().unwrap())
    } else {
        Ok(ctx)
    }
}

/// Type check a module and return ALL errors (not just the first)
/// This is the recommended entry point for tools that want to show all errors
pub fn check_with_all_errors(module: Module) -> crate::errors::MultiDiagnosticResult<TypedModule> {
    let mut ctx = TypeContext::with_filename(&module.name);
    let mut collector = DiagnosticCollector::new();

    // First pass: handle imports
    let source_path = Path::new(&module.name);
    let root_dir = source_path.parent().unwrap_or(Path::new("."));
    let mut module_graph = ModuleGraph::new(root_dir);

    for item in &module.items {
        if let Item::Use(use_def) = item {
            match module_graph.resolve_import(source_path, &use_def.path) {
                Ok(import_path) => {
                    match module_graph.load_module(&import_path) {
                        Ok(loaded) => {
                            match get_imported_items(use_def, &loaded.module) {
                                Ok(imported_items) => {
                                    for imported_item in imported_items {
                                        register_item_in_context(imported_item, &mut ctx, use_def, &mut collector);
                                    }
                                }
                                Err(msg) => {
                                    collector.push(Diagnostic::error(ErrorCode::E3006, msg)
                                        .with_label(ctx.make_span(use_def.span.clone()), "import error"));
                                }
                            }
                        }
                        Err(diag) => {
                            collector.push(diag.with_label(ctx.make_span(use_def.span.clone()), "failed to load module"));
                        }
                    }
                }
                Err(msg) => {
                    collector.push(Diagnostic::error(ErrorCode::E3006, msg)
                        .with_label(ctx.make_span(use_def.span.clone()), "cannot resolve import"));
                }
            }
        }
    }

    // Second pass: collect definitions
    for item in &module.items {
        match item {
            Item::TypeDef(td) => {
                ctx.define_type(td.name.clone(), td.clone());
            }
            Item::Function(fd) => {
                let sig = FunctionSig {
                    type_params: fd.type_params.clone(),
                    type_param_bounds: collect_type_param_bounds(fd),
                    params: fd.params.iter().map(|p| (p.name.clone(), p.ty.clone())).collect(),
                    return_type: fd.return_type.clone(),
                };
                ctx.define_function(fd.name.clone(), sig);
            }
            Item::Config(cd) => {
                match cd.ty.clone().map_or_else(
                    || infer_type(&cd.value.expr).map_err(|e| format!("Config '{}': {}", cd.name, e)),
                    Ok,
                ) {
                    Ok(ty) => ctx.define_config(cd.name.clone(), ty),
                    Err(msg) => {
                        collector.push(Diagnostic::error(ErrorCode::E3005, msg)
                            .with_label(ctx.make_span(cd.value.span.clone()), "cannot infer type"));
                    }
                }
            }
            _ => {}
        }
    }

    // Third pass: type check all expressions
    for item in &module.items {
        match item {
            Item::Function(fd) => {
                if let Err(msg) = check_function(fd, &mut ctx) {
                    collector.push(Diagnostic::error(ErrorCode::E3001, msg)
                        .with_label(ctx.make_span(fd.body.span.clone()), "type error in function body"));
                }
            }
            Item::Config(cd) => {
                if let Err(msg) = check_config(cd, &ctx) {
                    collector.push(Diagnostic::error(ErrorCode::E3001, msg)
                        .with_label(ctx.make_span(cd.value.span.clone()), "type error in config value"));
                }
            }
            Item::Test(td) => {
                if let Err(msg) = check_test(td, &ctx) {
                    collector.push(Diagnostic::error(ErrorCode::E3001, msg)
                        .with_label(ctx.make_span(td.body.span.clone()), "type error in test body"));
                }
            }
            _ => {}
        }
    }

    // Return all errors or the module
    if collector.has_errors() {
        Err(collector.into_diagnostics())
    } else {
        Ok(module)
    }
}

/// Main entry point: type check an entire module
pub fn check(module: Module) -> DiagnosticResult<TypedModule> {
    collect_and_check_module(&module)?;
    Ok(module)
}

/// Type check and lower a module to TIR
/// This combines type checking with IR generation
pub fn check_and_lower(module: Module) -> DiagnosticResult<TModule> {
    let ctx = collect_and_check_module(&module)?;
    Lowerer::lower_module(&module, &ctx)
        .map_err(|msg| Diagnostic::error(ErrorCode::E0000, msg))
}

fn check_function(fd: &FunctionDef, ctx: &mut TypeContext) -> Result<(), String> {
    // Save old state
    let old_locals = ctx.save_locals();
    let old_return_type = ctx.current_return_type();

    // Set current return type for self() calls
    ctx.set_current_return_type(fd.return_type.clone());

    // Add parameters to local scope (function parameters are immutable by default)
    for param in &fd.params {
        ctx.define_local(param.name.clone(), param.ty.clone(), false);
    }

    // Check body expression with return type as hint (for lambdas that are directly returned)
    let body_type = check_expr_with_hint(&fd.body.expr, ctx, Some(&fd.return_type))?;

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
    let value_type = check_expr(&cd.value.expr, ctx)?;

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
    check_expr(&td.body.expr, ctx)?;

    Ok(())
}

/// Register an imported item in the type context
fn register_item_in_context(
    item: &Item,
    ctx: &mut TypeContext,
    use_def: &UseDef,
    collector: &mut DiagnosticCollector,
) {
    match item {
        Item::TypeDef(td) => {
            // Check for alias
            let name = find_alias(&use_def.items, &td.name).unwrap_or(&td.name);
            ctx.define_type(name.clone(), td.clone());
        }
        Item::Function(fd) => {
            // Check for alias
            let name = find_alias(&use_def.items, &fd.name).unwrap_or(&fd.name);
            let sig = FunctionSig {
                type_params: fd.type_params.clone(),
                type_param_bounds: collect_type_param_bounds(fd),
                params: fd
                    .params
                    .iter()
                    .map(|p| (p.name.clone(), p.ty.clone()))
                    .collect(),
                return_type: fd.return_type.clone(),
            };
            ctx.define_function(name.clone(), sig);
        }
        Item::Config(cd) => {
            // Check for alias
            let name = find_alias(&use_def.items, &cd.name).unwrap_or(&cd.name);
            match cd.ty.clone().map_or_else(
                || infer_type(&cd.value.expr).map_err(|e| format!("Config '{}': {}", cd.name, e)),
                Ok,
            ) {
                Ok(ty) => ctx.define_config(name.clone(), ty),
                Err(msg) => {
                    collector.push(Diagnostic::error(ErrorCode::E3005, msg)
                        .with_label(ctx.make_span(use_def.span.clone()), "cannot infer imported config type"));
                }
            }
        }
        _ => {} // Tests and uses are not imported
    }
}

/// Find the alias for an imported item, if any
fn find_alias<'a>(items: &'a [UseItem], name: &str) -> Option<&'a String> {
    items.iter()
        .find(|i| i.name == name)
        .and_then(|i| i.alias.as_ref())
}

/// Collect type parameter bounds from a function definition
/// Merges inline bounds (from TypeParam) and where clause bounds
fn collect_type_param_bounds(fd: &FunctionDef) -> Vec<TypeParamBound> {
    use std::collections::HashMap;

    // Create a map from type param name to bounds
    let mut bounds_map: HashMap<String, Vec<String>> = HashMap::new();

    // First, collect bounds from inline syntax (type_param_bounds)
    for tp in &fd.type_param_bounds {
        if !tp.bounds.is_empty() {
            bounds_map
                .entry(tp.name.clone())
                .or_default()
                .extend(tp.bounds.clone());
        }
    }

    // Then, add bounds from where clause
    for wb in &fd.where_clause {
        bounds_map
            .entry(wb.type_param.clone())
            .or_default()
            .extend(wb.bounds.clone());
    }

    // Convert to Vec<TypeParamBound>, preserving original type param order
    let mut result = Vec::new();
    for param in &fd.type_params {
        if let Some(bounds) = bounds_map.remove(param) {
            // Deduplicate bounds
            let unique_bounds: Vec<String> = bounds
                .into_iter()
                .collect::<std::collections::HashSet<_>>()
                .into_iter()
                .collect();
            if !unique_bounds.is_empty() {
                result.push(TypeParamBound::new(param.clone(), unique_bounds));
            }
        }
    }

    // Add any remaining bounds from where clause for type params not in type_params
    // (This can happen with where clauses that reference associated types, etc.)
    for (type_param, bounds) in bounds_map {
        let unique_bounds: Vec<String> = bounds
            .into_iter()
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        if !unique_bounds.is_empty() {
            result.push(TypeParamBound::new(type_param, unique_bounds));
        }
    }

    result
}

#[cfg(test)]
mod tests;
