//! Type checking re-exports from `ori_typeck`.
//!
//! This module provides the type checking infrastructure for oric.
//! The core implementation lives in the `ori_typeck` crate.
//!
//! # Import-Aware Type Checking
//!
//! The main entry points for type checking with import resolution are:
//! - [`type_check_with_imports`]: Type checks a module with resolved imports
//! - [`resolve_imports_for_type_checking`]: Extracts imported function signatures

// Re-export all public types from ori_typeck
pub use ori_typeck::{
    // Main type checker
    TypeChecker, TypeCheckerBuilder,
    // Components
    CheckContext, InferenceState, Registries, DiagnosticState, ScopeContext,
    SavedCapabilityContext, SavedImplContext,
    // Output types
    TypedModule, FunctionType, GenericBound, WhereConstraint, TypeCheckError,
    // Convenience functions
    type_check, type_check_with_source, type_check_with_config,
    // Utility
    add_pattern_bindings, ensure_sufficient_stack,
    primitive_implements_trait,
    SharedRegistry,
    // Import support
    ImportedFunction, ImportedGeneric,
};

// Registry re-exports (also available as ori_typeck::registry::*)
pub mod type_registry {
    pub use ori_typeck::registry::*;
}

pub use ori_typeck::registry::{
    TypeRegistry, TypeEntry, TypeKind, VariantDef,
    TraitRegistry, TraitEntry, TraitMethodDef, TraitAssocTypeDef,
    ImplEntry, ImplMethodDef, ImplAssocTypeDef, MethodLookup, CoherenceError,
};

// Operator re-exports
pub mod operators {
    pub use ori_typeck::operators::*;
}

// Derives re-exports
pub mod derives {
    pub use ori_typeck::derives::*;
}

// Inference re-exports
pub mod infer {
    pub use ori_typeck::infer::*;
}

// Re-export DiagnosticConfig from ori_diagnostic (for type_check_with_config)
pub use ori_diagnostic::queue::DiagnosticConfig;

use std::path::Path;

use crate::context::CompilerContext;
use crate::db::Db;
use crate::eval::module::import::{resolve_import, ImportError};
use crate::ir::{StringInterner, Name};
use crate::parser::ParseResult;
use crate::query::parsed;

/// Type check a parsed module with a custom compiler context.
///
/// This allows dependency injection of custom registries for testing.
/// This function is specific to oric since it uses `CompilerContext`.
pub fn type_check_with_context(
    parse_result: &ParseResult,
    interner: &StringInterner,
    context: &CompilerContext,
) -> TypedModule {
    TypeCheckerBuilder::new(&parse_result.arena, interner)
        .with_pattern_registry(context.pattern_registry.clone())
        .build()
        .check_module(&parse_result.module)
}

/// Resolve imports and extract function signatures for type checking.
///
/// This function:
/// 1. Resolves each import path to a file
/// 2. Type checks each imported module (via the `typed` Salsa query)
/// 3. Extracts `ImportedFunction` for each function that's imported
///
/// # Returns
///
/// A list of `ImportedFunction` representing all imported function signatures,
/// or an error if any import fails to resolve.
pub fn resolve_imports_for_type_checking(
    db: &dyn Db,
    parse_result: &ParseResult,
    current_file: &Path,
) -> Result<Vec<ImportedFunction>, ImportError> {
    let mut imported_functions = Vec::new();
    let interner = db.interner();

    for imp in &parse_result.module.imports {
        // Resolve the import path to a file
        let resolved = resolve_import(db, &imp.path, current_file)?;

        // Get the parsed module to access function definitions
        let imported_parsed = parsed(db, resolved.file);

        // Build a map of imported function names to their aliases
        let import_map: std::collections::HashMap<Name, Option<Name>> = imp.items
            .iter()
            .map(|item| (item.name, item.alias))
            .collect();

        // Build a set of names that request private access
        let private_access: std::collections::HashSet<Name> = imp.items
            .iter()
            .filter(|item| item.is_private)
            .map(|item| item.name)
            .collect();

        // Extract ImportedFunction for each imported function
        for func in &imported_parsed.module.functions {
            // Only include functions that are actually imported
            let Some(&alias) = import_map.get(&func.name) else {
                continue;
            };

            // Check visibility (public or private with :: prefix)
            if !func.is_public && !private_access.contains(&func.name) {
                continue;
            }

            // Create ImportedFunction directly from the Function AST
            // This avoids the TypeId interner issue by converting ParsedTypes directly
            let imported = create_imported_function(func, &imported_parsed.arena, interner);

            // Apply alias if present
            let final_name = alias.unwrap_or(func.name);

            imported_functions.push(ImportedFunction {
                name: final_name,
                ..imported
            });
        }
    }

    Ok(imported_functions)
}

/// Convert a `ParsedType` to a Type.
///
/// This function handles the conversion without needing access to a `TypeInterner`,
/// making it suitable for cross-module type conversion.
fn parsed_type_to_type(parsed: &ori_ir::ParsedType, interner: &StringInterner) -> ori_types::Type {
    use ori_ir::{ParsedType, TypeId};
    use ori_types::Type;

    match parsed {
        ParsedType::Primitive(type_id) => match *type_id {
            TypeId::INT => Type::Int,
            TypeId::FLOAT => Type::Float,
            TypeId::BOOL => Type::Bool,
            TypeId::STR => Type::Str,
            TypeId::CHAR => Type::Char,
            TypeId::BYTE => Type::Byte,
            TypeId::NEVER => Type::Never,
            // VOID and any unknown primitives fall back to Unit
            _ => Type::Unit,
        },
        ParsedType::Named { name, type_args } => {
            // Check for well-known generic types
            let name_str = interner.lookup(*name);
            match name_str {
                "Option" if type_args.len() == 1 => {
                    Type::Option(Box::new(parsed_type_to_type(&type_args[0], interner)))
                }
                "Result" if type_args.len() == 2 => {
                    Type::Result {
                        ok: Box::new(parsed_type_to_type(&type_args[0], interner)),
                        err: Box::new(parsed_type_to_type(&type_args[1], interner)),
                    }
                }
                "Set" if type_args.len() == 1 => {
                    Type::Set(Box::new(parsed_type_to_type(&type_args[0], interner)))
                }
                "Range" if type_args.len() == 1 => {
                    Type::Range(Box::new(parsed_type_to_type(&type_args[0], interner)))
                }
                "Channel" if type_args.len() == 1 => {
                    Type::Channel(Box::new(parsed_type_to_type(&type_args[0], interner)))
                }
                "Duration" => Type::Duration,
                "Size" => Type::Size,
                _ if type_args.is_empty() => Type::Named(*name),
                _ => Type::Applied {
                    name: *name,
                    args: type_args.iter().map(|t| parsed_type_to_type(t, interner)).collect(),
                },
            }
        }
        ParsedType::List(elem) => {
            Type::List(Box::new(parsed_type_to_type(elem, interner)))
        }
        ParsedType::Tuple(elems) => {
            Type::Tuple(elems.iter().map(|e| parsed_type_to_type(e, interner)).collect())
        }
        ParsedType::Function { params, ret } => {
            Type::Function {
                params: params.iter().map(|p| parsed_type_to_type(p, interner)).collect(),
                ret: Box::new(parsed_type_to_type(ret, interner)),
            }
        }
        ParsedType::Map { key, value } => {
            Type::Map {
                key: Box::new(parsed_type_to_type(key, interner)),
                value: Box::new(parsed_type_to_type(value, interner)),
            }
        }
        ParsedType::Infer => Type::Var(ori_types::TypeVar::new(0)), // Fresh var placeholder
        ParsedType::SelfType => Type::Named(interner.intern("Self")),
        ParsedType::AssociatedType { base: _, assoc_name } => {
            // For associated types, we create a projection type
            // This is a simplified representation
            Type::Named(*assoc_name)
        }
    }
}

/// Create an `ImportedFunction` directly from a Function AST.
fn create_imported_function(
    func: &ori_ir::Function,
    arena: &ori_ir::ExprArena,
    interner: &StringInterner,
) -> ImportedFunction {
    // Convert parameter types
    let params: Vec<ori_types::Type> = arena.get_params(func.params)
        .iter()
        .map(|p| {
            match &p.ty {
                Some(parsed_ty) => parsed_type_to_type(parsed_ty, interner),
                None => ori_types::Type::Var(ori_types::TypeVar::new(0)), // Inference placeholder
            }
        })
        .collect();

    // Convert return type
    let return_type = match &func.return_ty {
        Some(parsed_ty) => parsed_type_to_type(parsed_ty, interner),
        None => ori_types::Type::Unit, // Default to void
    };

    // Convert generics
    let generics: Vec<ImportedGeneric> = arena.get_generic_params(func.generics)
        .iter()
        .map(|gp| ImportedGeneric {
            param: gp.name,
            bounds: gp.bounds.iter().map(ori_ir::TraitBound::path).collect(),
        })
        .collect();

    // Extract capabilities
    let capabilities: Vec<Name> = func.capabilities.iter()
        .map(|cap_ref| cap_ref.name)
        .collect();

    ImportedFunction {
        name: func.name,
        params,
        return_type,
        generics,
        capabilities,
    }
}

/// Type check a parsed module with resolved imports.
///
/// This is the main entry point for import-aware type checking. It:
/// 1. Resolves imports and extracts function signatures
/// 2. Creates a type checker with imported functions registered
/// 3. Type checks the module
///
/// # Arguments
///
/// * `db` - The compiler database for Salsa queries
/// * `parse_result` - The parsed module to type check
/// * `current_file` - Path to the current file (for resolving relative imports)
///
/// # Returns
///
/// A `TypedModule` with type information, or errors if type checking fails.
pub fn type_check_with_imports(
    db: &dyn Db,
    parse_result: &ParseResult,
    current_file: &Path,
) -> TypedModule {
    let interner = db.interner();

    // Resolve imports and extract function signatures
    let imported_functions = match resolve_imports_for_type_checking(db, parse_result, current_file) {
        Ok(imports) => imports,
        Err(e) => {
            // Return a TypedModule with the import error
            return TypedModule {
                expr_types: Vec::new(),
                function_types: Vec::new(),
                errors: vec![TypeCheckError {
                    message: format!("import error: {}", e.message),
                    span: ori_ir::Span::default(),
                    code: ori_diagnostic::ErrorCode::E2003, // Unknown identifier
                }],
                error_guarantee: ori_diagnostic::ErrorGuaranteed::from_error_count(1),
            };
        }
    };

    // Create type checker and register imported functions
    let mut checker = TypeCheckerBuilder::new(&parse_result.arena, interner).build();
    checker.register_imported_functions(&imported_functions);

    // Type check the module
    checker.check_module(&parse_result.module)
}

/// Type check a parsed module with resolved imports and source for diagnostics.
///
/// Like `type_check_with_imports`, but also accepts source code for better
/// error messages with deduplication and limits.
pub fn type_check_with_imports_and_source(
    db: &dyn Db,
    parse_result: &ParseResult,
    current_file: &Path,
    source: String,
) -> TypedModule {
    let interner = db.interner();

    // Resolve imports and extract function signatures
    let imported_functions = match resolve_imports_for_type_checking(db, parse_result, current_file) {
        Ok(imports) => imports,
        Err(e) => {
            return TypedModule {
                expr_types: Vec::new(),
                function_types: Vec::new(),
                errors: vec![TypeCheckError {
                    message: format!("import error: {}", e.message),
                    span: ori_ir::Span::default(),
                    code: ori_diagnostic::ErrorCode::E2003,
                }],
                error_guarantee: ori_diagnostic::ErrorGuaranteed::from_error_count(1),
            };
        }
    };

    // Create type checker with source and register imported functions
    let mut checker = TypeCheckerBuilder::new(&parse_result.arena, interner)
        .with_source(source)
        .build();
    checker.register_imported_functions(&imported_functions);

    // Type check the module
    checker.check_module(&parse_result.module)
}
