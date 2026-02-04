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
    // Utility
    add_pattern_bindings,
    ensure_sufficient_stack,
    primitive_implements_trait,
    // Convenience functions
    type_check,
    type_check_with_config,
    type_check_with_source,
    // Components
    CheckContext,
    DiagnosticState,
    FunctionType,
    GenericBound,
    // Import support
    ImportedFunction,
    ImportedGeneric,
    ImportedModuleAlias,
    InferenceState,
    Registries,
    ScopeContext,
    SharedRegistry,
    TypeCheckError,
    // Main type checker
    TypeChecker,
    TypeCheckerBuilder,
    // Output types
    TypedModule,
    WhereConstraint,
};

// Registry re-exports (also available as ori_typeck::registry::*)
pub mod type_registry {
    pub use ori_typeck::registry::*;
}

pub use ori_typeck::registry::{
    CoherenceError, ImplAssocTypeDef, ImplEntry, ImplMethodDef, MethodLookup, TraitAssocTypeDef,
    TraitEntry, TraitMethodDef, TraitRegistry, TypeEntry, TypeKind, TypeRegistry, VariantDef,
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

use std::path::{Path, PathBuf};

use rustc_hash::{FxHashMap, FxHashSet};

use crate::context::CompilerContext;
use crate::db::Db;
use crate::eval::module::import::{resolve_import, ImportError};
use crate::ir::{Name, StringInterner};
use crate::parser::ParseOutput;
use crate::query::parsed;

// =============================================================================
// Prelude Auto-Loading for Type Checking
// =============================================================================

/// Generate candidate paths for the prelude by walking up from the current file.
fn prelude_candidates(current_file: &Path) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    let mut dir = current_file.parent();
    while let Some(d) = dir {
        candidates.push(d.join("library").join("std").join("prelude.ori"));
        dir = d.parent();
    }
    candidates
}

/// Check if a file is the prelude itself (to avoid recursive loading).
fn is_prelude_file(file_path: &Path) -> bool {
    file_path.ends_with("library/std/prelude.ori")
        || (file_path.file_name().is_some_and(|n| n == "prelude.ori")
            && file_path.parent().is_some_and(|p| p.ends_with("std")))
}

/// Load prelude functions for type checking.
///
/// This function finds and loads the prelude file, extracting all public functions
/// as `ImportedFunction` entries. This makes prelude functions available to the type
/// checker without requiring explicit imports, matching the evaluator's behavior.
fn load_prelude_for_type_checking(db: &dyn Db, current_file: &Path) -> Vec<ImportedFunction> {
    // Don't load prelude if we're type checking the prelude itself
    if is_prelude_file(current_file) {
        return Vec::new();
    }

    // Find the prelude file
    let prelude_file = prelude_candidates(current_file)
        .iter()
        .find_map(|candidate| db.load_file(candidate));

    let Some(prelude_file) = prelude_file else {
        // Prelude not found - this is okay for tests outside the project
        return Vec::new();
    };

    let interner = db.interner();
    let prelude_result = parsed(db, prelude_file);

    // Extract all public functions from the prelude
    let mut functions = Vec::new();
    for func in &prelude_result.module.functions {
        if func.visibility.is_public() {
            functions.push(create_imported_function(
                func,
                &prelude_result.arena,
                interner,
            ));
        }
    }

    functions
}

/// Type check a parsed module with a custom compiler context.
///
/// This allows dependency injection of custom registries for testing.
/// This function is specific to oric since it uses `CompilerContext`.
pub fn type_check_with_context(
    parse_result: &ParseOutput,
    interner: &StringInterner,
    context: &CompilerContext,
) -> TypedModule {
    TypeCheckerBuilder::new(&parse_result.arena, interner)
        .with_pattern_registry(context.pattern_registry.clone())
        .build()
        .check_module(&parse_result.module)
}

/// Result of resolving imports for type checking.
///
/// Contains both individual function imports and module alias imports.
#[derive(Debug, Default)]
pub struct ResolvedImports {
    /// Individual function imports.
    pub functions: Vec<ImportedFunction>,
    /// Module alias imports (e.g., `use std.http as http`).
    pub module_aliases: Vec<ImportedModuleAlias>,
}

/// Resolve imports and extract function signatures for type checking.
///
/// This function:
/// 1. Resolves each import path to a file
/// 2. Type checks each imported module (via the `typed` Salsa query)
/// 3. Extracts `ImportedFunction` for each function that's imported
/// 4. Extracts `ImportedModuleAlias` for module alias imports
///
/// # Returns
///
/// A `ResolvedImports` containing all imported function signatures and module aliases,
/// or an error if any import fails to resolve.
pub fn resolve_imports_for_type_checking(
    db: &dyn Db,
    parse_result: &ParseOutput,
    current_file: &Path,
) -> Result<ResolvedImports, ImportError> {
    let mut result = ResolvedImports::default();
    let interner = db.interner();

    for imp in &parse_result.module.imports {
        // Resolve the import path to a file
        let resolved = resolve_import(db, &imp.path, current_file)
            .map_err(|e| ImportError::with_span(e.message, imp.span))?;

        // Get the parsed module to access function definitions
        let imported_parsed = parsed(db, resolved.file);

        // Handle module alias imports (use std.http as http)
        if let Some(alias) = imp.module_alias {
            let mut functions = Vec::new();

            // Collect all public functions from the module
            for func in &imported_parsed.module.functions {
                if func.visibility.is_public() {
                    let imported = create_imported_function(func, &imported_parsed.arena, interner);
                    functions.push(imported);
                }
            }

            result
                .module_aliases
                .push(ImportedModuleAlias { alias, functions });
            continue;
        }

        // Handle individual item imports
        // Build a map of imported function names to their aliases
        let import_map: FxHashMap<Name, Option<Name>> = imp
            .items
            .iter()
            .map(|item| (item.name, item.alias))
            .collect();

        // Build a set of names that request private access
        let private_access: FxHashSet<Name> = imp
            .items
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
            if !func.visibility.is_public() && !private_access.contains(&func.name) {
                continue;
            }

            // Create ImportedFunction directly from the Function AST
            // This avoids the TypeId interner issue by converting ParsedTypes directly
            let imported = create_imported_function(func, &imported_parsed.arena, interner);

            // Apply alias if present
            let final_name = alias.unwrap_or(func.name);

            result.functions.push(ImportedFunction {
                name: final_name,
                ..imported
            });
        }
    }

    Ok(result)
}

/// Convert a `ParsedType` to a Type.
///
/// This function handles the conversion using the arena for type ID lookups,
/// making it suitable for cross-module type conversion.
fn parsed_type_to_type(
    parsed: &ori_ir::ParsedType,
    arena: &ori_ir::ExprArena,
    interner: &StringInterner,
) -> ori_types::Type {
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
            let arg_ids = arena.get_parsed_type_list(*type_args);
            match name_str {
                "Option" if arg_ids.len() == 1 => {
                    let arg_ty = arena.get_parsed_type(arg_ids[0]);
                    Type::Option(Box::new(parsed_type_to_type(arg_ty, arena, interner)))
                }
                "Result" if arg_ids.len() == 2 => {
                    let ok_ty = arena.get_parsed_type(arg_ids[0]);
                    let err_ty = arena.get_parsed_type(arg_ids[1]);
                    Type::Result {
                        ok: Box::new(parsed_type_to_type(ok_ty, arena, interner)),
                        err: Box::new(parsed_type_to_type(err_ty, arena, interner)),
                    }
                }
                "Set" if arg_ids.len() == 1 => {
                    let arg_ty = arena.get_parsed_type(arg_ids[0]);
                    Type::Set(Box::new(parsed_type_to_type(arg_ty, arena, interner)))
                }
                "Range" if arg_ids.len() == 1 => {
                    let arg_ty = arena.get_parsed_type(arg_ids[0]);
                    Type::Range(Box::new(parsed_type_to_type(arg_ty, arena, interner)))
                }
                "Channel" if arg_ids.len() == 1 => {
                    let arg_ty = arena.get_parsed_type(arg_ids[0]);
                    Type::Channel(Box::new(parsed_type_to_type(arg_ty, arena, interner)))
                }
                "Duration" => Type::Duration,
                "Size" => Type::Size,
                _ if arg_ids.is_empty() => Type::Named(*name),
                _ => Type::Applied {
                    name: *name,
                    args: arg_ids
                        .iter()
                        .map(|id| {
                            let ty = arena.get_parsed_type(*id);
                            parsed_type_to_type(ty, arena, interner)
                        })
                        .collect(),
                },
            }
        }
        ParsedType::List(elem_id) => {
            let elem_ty = arena.get_parsed_type(*elem_id);
            Type::List(Box::new(parsed_type_to_type(elem_ty, arena, interner)))
        }
        ParsedType::FixedList { elem, .. } => {
            // Fixed-capacity lists resolve to List for now
            // (full type system support in later phase)
            let elem_ty = arena.get_parsed_type(*elem);
            Type::List(Box::new(parsed_type_to_type(elem_ty, arena, interner)))
        }
        ParsedType::Tuple(elems) => {
            let elem_ids = arena.get_parsed_type_list(*elems);
            Type::Tuple(
                elem_ids
                    .iter()
                    .map(|id| {
                        let ty = arena.get_parsed_type(*id);
                        parsed_type_to_type(ty, arena, interner)
                    })
                    .collect(),
            )
        }
        ParsedType::Function { params, ret } => {
            let param_ids = arena.get_parsed_type_list(*params);
            let ret_ty = arena.get_parsed_type(*ret);
            Type::Function {
                params: param_ids
                    .iter()
                    .map(|id| {
                        let ty = arena.get_parsed_type(*id);
                        parsed_type_to_type(ty, arena, interner)
                    })
                    .collect(),
                ret: Box::new(parsed_type_to_type(ret_ty, arena, interner)),
            }
        }
        ParsedType::Map { key, value } => {
            let key_ty = arena.get_parsed_type(*key);
            let value_ty = arena.get_parsed_type(*value);
            Type::Map {
                key: Box::new(parsed_type_to_type(key_ty, arena, interner)),
                value: Box::new(parsed_type_to_type(value_ty, arena, interner)),
            }
        }
        ParsedType::Infer => Type::Var(ori_types::TypeVar::new(0)), // Fresh var placeholder
        ParsedType::SelfType => Type::Named(interner.intern("Self")),
        ParsedType::AssociatedType {
            base: _,
            assoc_name,
        } => {
            // For associated types, we create a projection type
            // This is a simplified representation
            Type::Named(*assoc_name)
        }
    }
}

/// Create an `ImportedFunction` directly from a Function AST.
///
/// For generic functions, this creates unique `TypeVar`s for each generic parameter
/// and substitutes `Type::Named("T")` references with `Type::Var` in the function type.
fn create_imported_function(
    func: &ori_ir::Function,
    arena: &ori_ir::ExprArena,
    interner: &StringInterner,
) -> ImportedFunction {
    use ori_types::{Type, TypeFolder, TypeVar};
    use rustc_hash::FxHashMap;

    // First, collect generic parameters and create TypeVars for them.
    // We use local IDs (0, 1, 2...) since each function is independent.
    let generic_params = arena.get_generic_params(func.generics);
    let mut name_to_typevar: FxHashMap<Name, TypeVar> = FxHashMap::default();
    let mut generics: Vec<ImportedGeneric> = Vec::with_capacity(generic_params.len());

    for (idx, gp) in generic_params.iter().enumerate() {
        // Generic parameter count is always small (typically < 10), safe to truncate
        #[expect(
            clippy::cast_possible_truncation,
            reason = "generic param count << u32::MAX"
        )]
        let type_var = TypeVar::new(idx as u32);
        name_to_typevar.insert(gp.name, type_var);
        generics.push(ImportedGeneric {
            param: gp.name,
            bounds: gp.bounds.iter().map(ori_ir::TraitBound::path).collect(),
            type_var,
        });
    }

    // Convert parameter types, initially as Type::Named for generic params
    let raw_params: Vec<Type> = arena
        .get_params(func.params)
        .iter()
        .map(|p| match &p.ty {
            Some(parsed_ty) => parsed_type_to_type(parsed_ty, arena, interner),
            None => Type::Var(TypeVar::new(0)), // Inference placeholder
        })
        .collect();

    // Convert return type
    let raw_return_type = match &func.return_ty {
        Some(parsed_ty) => parsed_type_to_type(parsed_ty, arena, interner),
        None => Type::Unit,
    };

    // Substitute Type::Named(generic_name) with Type::Var for generic parameters.
    // This ensures the function type uses TypeVars that match ImportedGeneric.type_var.
    let (params, return_type) = if name_to_typevar.is_empty() {
        (raw_params, raw_return_type)
    } else {
        struct GenericSubstituter<'a> {
            name_to_typevar: &'a FxHashMap<Name, TypeVar>,
        }
        impl TypeFolder for GenericSubstituter<'_> {
            fn fold_named(&mut self, name: Name) -> Type {
                if let Some(&tv) = self.name_to_typevar.get(&name) {
                    Type::Var(tv)
                } else {
                    Type::Named(name)
                }
            }
        }

        let mut folder = GenericSubstituter {
            name_to_typevar: &name_to_typevar,
        };
        let params = raw_params.iter().map(|t| folder.fold(t)).collect();
        let return_type = folder.fold(&raw_return_type);
        (params, return_type)
    };

    // Extract capabilities
    let capabilities: Vec<Name> = func
        .capabilities
        .iter()
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
    parse_result: &ParseOutput,
    current_file: &Path,
) -> TypedModule {
    type_check_with_imports_and_interner(db, parse_result, current_file, None)
}

/// Type check a parsed module with resolved imports and a shared type interner.
///
/// Like `type_check_with_imports`, but allows passing a `SharedTypeInterner` so the
/// same interner can be shared between the type checker and evaluator. This enables
/// type-aware evaluation for operators like `??` that need access to type information.
///
/// # Arguments
///
/// * `db` - The compiler database for Salsa queries
/// * `parse_result` - The parsed module to type check
/// * `current_file` - Path to the current file (for resolving relative imports)
/// * `type_interner` - Optional shared type interner to use (creates a new one if None)
///
/// # Returns
///
/// A `TypedModule` with type information, or errors if type checking fails.
pub fn type_check_with_imports_and_interner(
    db: &dyn Db,
    parse_result: &ParseOutput,
    current_file: &Path,
    type_interner: Option<ori_types::SharedTypeInterner>,
) -> TypedModule {
    let interner = db.interner();

    // Resolve imports and extract function signatures and module aliases
    let resolved = match resolve_imports_for_type_checking(db, parse_result, current_file) {
        Ok(imports) => imports,
        Err(e) => {
            // Return a TypedModule with the import error
            return TypedModule {
                expr_types: Vec::new(),
                function_types: Vec::new(),
                errors: vec![TypeCheckError::Generic {
                    message: format!("import error: {}", e.message),
                    span: e.span.unwrap_or_default(),
                    code: ori_diagnostic::ErrorCode::E2003, // Unknown identifier
                    suggestion: None,
                }],
                error_guarantee: ori_diagnostic::ErrorGuaranteed::from_error_count(1),
            };
        }
    };

    // Create type checker, optionally with a shared type interner
    let mut builder = TypeCheckerBuilder::new(&parse_result.arena, interner);
    if let Some(ti) = type_interner {
        builder = builder.with_type_interner(ti);
    }
    let mut checker = builder.build();

    // Auto-load prelude functions (like the evaluator does)
    let prelude_functions = load_prelude_for_type_checking(db, current_file);
    checker.register_imported_functions(&prelude_functions);

    // Register explicitly imported functions and module aliases
    checker.register_imported_functions(&resolved.functions);
    for alias in &resolved.module_aliases {
        checker.register_module_alias(alias);
    }

    // Type check the module
    checker.check_module(&parse_result.module)
}

/// Type check a parsed module with resolved imports and source for diagnostics.
///
/// Like `type_check_with_imports`, but also accepts source code for better
/// error messages with deduplication and limits.
pub fn type_check_with_imports_and_source(
    db: &dyn Db,
    parse_result: &ParseOutput,
    current_file: &Path,
    source: String,
) -> TypedModule {
    type_check_with_imports_source_and_interner(db, parse_result, current_file, source, None)
}

/// Type check a parsed module with resolved imports, source, and shared type interner.
///
/// Like `type_check_with_imports_and_source`, but allows passing a `SharedTypeInterner`
/// so the same interner can be shared between the type checker and evaluator. This enables
/// type-aware evaluation for operators like `??` that need access to type information.
///
/// # Arguments
///
/// * `db` - The compiler database for Salsa queries
/// * `parse_result` - The parsed module to type check
/// * `current_file` - Path to the current file (for resolving relative imports)
/// * `source` - Source code for better error messages
/// * `type_interner` - Optional shared type interner to use (creates a new one if None)
pub fn type_check_with_imports_source_and_interner(
    db: &dyn Db,
    parse_result: &ParseOutput,
    current_file: &Path,
    source: String,
    type_interner: Option<ori_types::SharedTypeInterner>,
) -> TypedModule {
    let interner = db.interner();

    // Resolve imports and extract function signatures and module aliases
    let resolved = match resolve_imports_for_type_checking(db, parse_result, current_file) {
        Ok(imports) => imports,
        Err(e) => {
            return TypedModule {
                expr_types: Vec::new(),
                function_types: Vec::new(),
                errors: vec![TypeCheckError::Generic {
                    message: format!("import error: {}", e.message),
                    span: e.span.unwrap_or_default(),
                    code: ori_diagnostic::ErrorCode::E2003,
                    suggestion: None,
                }],
                error_guarantee: ori_diagnostic::ErrorGuaranteed::from_error_count(1),
            };
        }
    };

    // Create type checker with source, optionally with a shared type interner
    let mut builder = TypeCheckerBuilder::new(&parse_result.arena, interner).with_source(source);
    if let Some(ti) = type_interner {
        builder = builder.with_type_interner(ti);
    }
    let mut checker = builder.build();

    // Auto-load prelude functions (like the evaluator does)
    let prelude_functions = load_prelude_for_type_checking(db, current_file);
    checker.register_imported_functions(&prelude_functions);

    // Register explicitly imported functions and module aliases
    checker.register_imported_functions(&resolved.functions);
    for alias in &resolved.module_aliases {
        checker.register_module_alias(alias);
    }

    // Type check the module
    checker.check_module(&parse_result.module)
}
