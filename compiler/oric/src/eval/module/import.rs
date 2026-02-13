//! Import registration for the evaluator.
//!
//! Handles registering resolved imports into the evaluator's `Environment`.
//! Path resolution lives in [`crate::imports`]; this module only handles
//! the eval-specific concern of building `FunctionValue`s and binding them.
//!
//! ## Visibility
//!
//! - Public items (`pub @func`) can be imported normally
//! - Private items require `::` prefix: `use './mod' { ::private_func }`
//! - Test modules in `_test/` can access private items from parent module
//!
//! ## Module Aliases
//!
//! `use path as alias` imports the entire module as a namespace.
//! Access via qualified syntax: `alias.function()`.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use rustc_hash::FxHashMap;

use ori_ir::canon::SharedCanonResult;

use crate::eval::{Environment, FunctionValue, Mutability, Value};
use crate::imports::{is_parent_module_import, is_test_module, ImportError, ImportErrorKind};
use crate::ir::{Name, SharedArena, StringInterner};
use crate::parser::ParseOutput;

/// Extract params and capabilities from a function definition.
///
/// This is a common pattern when building `FunctionValue` from AST.
fn extract_function_metadata(
    func: &crate::ir::Function,
    arena: &SharedArena,
) -> (Vec<Name>, Vec<Name>) {
    let params = arena.get_param_names(func.params);
    let capabilities = func.capabilities.iter().map(|c| c.name).collect();
    (params, capabilities)
}

/// Represents a parsed and loaded module ready for import registration.
///
/// Groups together the parse result, arena, and pre-built function map
/// to reduce parameter count in `register_imports`.
///
/// Uses `BTreeMap` for deterministic iteration order, which is important
/// for reproducible builds and Salsa query compatibility.
pub struct ImportedModule<'a> {
    /// The parse result containing the module's AST.
    pub result: &'a ParseOutput,
    /// The expression arena for the imported module.
    pub arena: &'a SharedArena,
    /// Pre-built map of all functions in the module.
    /// Uses `BTreeMap` for deterministic iteration order.
    pub functions: BTreeMap<Name, Value>,
}

impl<'a> ImportedModule<'a> {
    /// Create a new imported module from parse result and arena.
    ///
    /// Builds the function map automatically. When `canon` is provided,
    /// each function's `FunctionValue` is enriched with canonical IR data,
    /// enabling the evaluator to dispatch on `CanExpr` instead of `ExprKind`.
    pub fn new(
        result: &'a ParseOutput,
        arena: &'a SharedArena,
        canon: Option<&SharedCanonResult>,
    ) -> Self {
        let functions = Self::build_functions(result, arena, canon);
        ImportedModule {
            result,
            arena,
            functions,
        }
    }

    /// Build a map of all functions in a module.
    ///
    /// This allows imported functions to call other functions from their module.
    /// Uses `BTreeMap` for deterministic iteration order.
    ///
    /// When `canon` is provided, attaches canonical IR to each function via
    /// `set_canon()`, mirroring `register_module_functions` for local functions.
    fn build_functions(
        parse_result: &ParseOutput,
        imported_arena: &SharedArena,
        canon: Option<&SharedCanonResult>,
    ) -> BTreeMap<Name, Value> {
        let mut module_functions: BTreeMap<Name, Value> = BTreeMap::new();

        for func in &parse_result.module.functions {
            let (params, capabilities) = extract_function_metadata(func, imported_arena);
            let mut func_value = FunctionValue::with_capabilities(
                params,
                FxHashMap::default(),
                imported_arena.clone(),
                capabilities,
            );

            // Attach canonical IR when available
            if let Some(cr) = canon {
                if let Some(root) = cr.root_for(func.name) {
                    func_value.set_canon(root, cr.clone());
                }
            }

            module_functions.insert(func.name, Value::Function(func_value));
        }

        module_functions
    }
}

/// Build a map of all functions in a module.
///
/// This allows imported functions to call other functions from their module.
/// Uses `BTreeMap` for deterministic iteration order.
///
/// When `canon` is provided, attaches canonical IR to each function.
pub(crate) fn build_module_functions(
    parse_result: &ParseOutput,
    imported_arena: &SharedArena,
    canon: Option<&SharedCanonResult>,
) -> BTreeMap<Name, Value> {
    ImportedModule::build_functions(parse_result, imported_arena, canon)
}

/// Register imported items into the environment.
///
/// Looks up the requested items in the imported module and registers them
/// in the current environment with proper captures.
///
/// Visibility rules:
/// - Public items (`pub @func`) can be imported normally
/// - Private items (no `pub`) require `::` prefix: `use './mod' { ::private_func }`
/// - Test modules in `_test/` can access private items from parent module
///
/// Module alias imports:
/// - `use path as alias` imports the entire module as a namespace
/// - Only public items are included in the namespace
/// - Access via qualified syntax: `alias.function()`
pub(crate) fn register_imports(
    import: &crate::ir::UseDef,
    imported: &ImportedModule<'_>,
    env: &mut Environment,
    interner: &StringInterner,
    import_path: &Path,
    current_file: &Path,
    canon: Option<&SharedCanonResult>,
) -> Result<(), Vec<ImportError>> {
    // Handle module alias: `use path as alias`
    if let Some(alias) = import.module_alias {
        return register_module_alias(import, imported, env, alias, import_path, canon)
            .map_err(|e| vec![e]);
    }

    // Check if this is a test module importing from its parent module
    let allow_private_access =
        is_test_module(current_file) && is_parent_module_import(current_file, import_path);

    // Build FxHashMap for O(1) function lookup instead of O(n) linear scan.
    // Keyed by Name (u32) rather than &str — avoids interner lookups on both
    // the build side and the lookup side (line 189). String lookup is only
    // needed on the cold error path for diagnostic messages.
    let func_by_name: FxHashMap<Name, &crate::ir::Function> = imported
        .result
        .module
        .functions
        .iter()
        .map(|f| (f.name, f))
        .collect();

    // Build enriched captures once: current environment + all module functions.
    // Previously this was done per-item inside the loop, cloning the entire
    // environment N times for N imports. Now we build it once and share via Arc.
    let shared_captures: Arc<FxHashMap<Name, Value>> = {
        let mut captures = env.capture();
        for (name, value) in &imported.functions {
            captures.insert(*name, value.clone());
        }
        Arc::new(captures)
    };

    let mut errors = Vec::new();

    for item in &import.items {
        // Find the function in the imported module (O(1) Name-based lookup)
        if let Some(&func) = func_by_name.get(&item.name) {
            // Check visibility: private items require :: prefix unless test module
            if !func.visibility.is_public() && !item.is_private && !allow_private_access {
                let name_str = interner.lookup(item.name);
                errors.push(ImportError::with_span(
                    ImportErrorKind::PrivateAccess,
                    format!(
                        "'{name_str}' is private in '{}'. Use '::{name_str}' to import private items.",
                        import_path.display(),
                    ),
                    import.span,
                ));
                continue;
            }

            let (params, capabilities) = extract_function_metadata(func, imported.arena);

            let mut func_value = FunctionValue::with_shared_captures(
                params,
                Arc::clone(&shared_captures),
                imported.arena.clone(),
                capabilities,
            );

            // Attach canonical IR when available
            if let Some(cr) = canon {
                if let Some(can_id) = cr.root_for(func.name) {
                    func_value.set_canon(can_id, cr.clone());
                }
            }

            // Use alias if provided, otherwise use original name
            let bind_name = item.alias.unwrap_or(item.name);
            env.define(
                bind_name,
                Value::Function(func_value),
                Mutability::Immutable,
            );
        } else {
            errors.push(ImportError::with_span(
                ImportErrorKind::ItemNotFound,
                format!(
                    "'{}' not found in '{}'",
                    interner.lookup(item.name),
                    import_path.display()
                ),
                import.span,
            ));
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Register a module alias import.
///
/// Creates a `ModuleNamespace` containing all public functions from the module
/// and binds it to the alias name.
fn register_module_alias(
    import: &crate::ir::UseDef,
    imported: &ImportedModule<'_>,
    env: &mut Environment,
    alias: Name,
    import_path: &Path,
    canon: Option<&SharedCanonResult>,
) -> Result<(), ImportError> {
    // Module alias imports should not have individual items
    if !import.items.is_empty() {
        return Err(ImportError::with_span(
            ImportErrorKind::ModuleAliasWithItems,
            format!(
                "module alias import cannot have individual items: '{}'",
                import_path.display()
            ),
            import.span,
        ));
    }

    // Collect all public functions into the namespace
    // Uses BTreeMap for deterministic iteration order
    let mut namespace: BTreeMap<Name, Value> = BTreeMap::new();

    // Clone captures once and wrap in Arc for sharing across all functions
    // Convert BTreeMap to FxHashMap (FunctionValue expects FxHashMap for captures)
    let shared_captures: Arc<FxHashMap<Name, Value>> = Arc::new(
        imported
            .functions
            .iter()
            .map(|(&k, v)| (k, v.clone()))
            .collect(),
    );

    for func in &imported.result.module.functions {
        if func.visibility.is_public() {
            let (params, capabilities) = extract_function_metadata(func, imported.arena);
            let mut func_value = FunctionValue::with_shared_captures(
                params,
                Arc::clone(&shared_captures),
                imported.arena.clone(),
                capabilities,
            );

            // Attach canonical IR when available
            if let Some(cr) = canon {
                if let Some(can_id) = cr.root_for(func.name) {
                    func_value.set_canon(can_id, cr.clone());
                }
            }

            namespace.insert(func.name, Value::Function(func_value));
        }
    }

    // Bind the namespace to the alias
    // (BTreeMap used for deterministic iteration order in Salsa queries)
    env.define(
        alias,
        Value::module_namespace(namespace),
        Mutability::Immutable,
    );

    Ok(())
}
