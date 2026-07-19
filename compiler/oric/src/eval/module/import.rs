//! Evaluator registration for resolved imports.
//!
//! [`crate::imports`] resolves paths; this module creates and binds
//! `FunctionValue`s. Public items import normally, private items require `::`
//! except from parent test modules, and module aliases bind qualified names.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rustc_hash::FxHashMap;

use ori_ir::{canon::SharedCanonResult, ImportCycleGuard};

use crate::eval::{Environment, FunctionValue, Mutability, Value};
use crate::imports::{self, is_parent_module_import, is_test_module, ImportError, ImportErrorKind};
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
#[derive(Debug)]
pub struct ImportedModule<'a> {
    /// The parse result containing the module's AST.
    pub result: &'a ParseOutput,
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
        db: &dyn crate::db::Db,
        result: &'a ParseOutput,
        arena: &'a SharedArena,
        module_path: &Path,
        canon: Option<&SharedCanonResult>,
    ) -> Result<Self, Vec<ImportError>> {
        let mut cycle_guard = ImportCycleGuard::new();
        Self::build_with_guard(db, result, arena, module_path, canon, &mut cycle_guard)
    }

    /// Build one module after recursively completing its lexical imports.
    ///
    /// A fresh environment is used for each module so imported functions do
    /// not accidentally capture bindings from their eventual consumer. Direct
    /// imports are completed first, then local constructors and functions are
    /// registered into that same environment before its bindings are frozen.
    fn build_with_guard<'module>(
        db: &dyn crate::db::Db,
        result: &'module ParseOutput,
        arena: &'module SharedArena,
        module_path: &Path,
        canon: Option<&SharedCanonResult>,
        cycle_guard: &mut ImportCycleGuard,
    ) -> Result<ImportedModule<'module>, Vec<ImportError>> {
        if let Err(cycle) = cycle_guard.start_loading(module_path.to_path_buf()) {
            return Err(vec![circular_import_error(&cycle)]);
        }

        let built =
            Self::build_lexical_environment(db, result, arena, module_path, canon, cycle_guard);

        // INVARIANT: every successful guard entry exits on both Ok and Err.
        // `is_visited` is intentionally not consulted: the same provider may
        // need a fresh lexical environment along separate diamond branches.
        cycle_guard.finish_loading(module_path);
        built
    }

    fn build_lexical_environment<'module>(
        db: &dyn crate::db::Db,
        result: &'module ParseOutput,
        arena: &'module SharedArena,
        module_path: &Path,
        canon: Option<&SharedCanonResult>,
        cycle_guard: &mut ImportCycleGuard,
    ) -> Result<ImportedModule<'module>, Vec<ImportError>> {
        let resolved = imports::resolve_imports(db, result, module_path);
        let mut errors = resolved.errors.clone();
        let mut env = Environment::new();

        // Prelude bindings are part of every module's lexical environment, not
        // ambient bindings inherited from whichever module eventually imports it.
        if let Some(prelude) = &resolved.prelude {
            let prelude_arena = prelude.parse_output.arena.clone();
            let prelude_canon = crate::query::canonicalize_module(
                db,
                &prelude.parse_output,
                &prelude.module_path,
                prelude.source_file,
            );
            let prelude_path = prelude
                .source_file
                .map_or(prelude.module_path.as_path(), |source| source.path(db));

            match Self::build_with_guard(
                db,
                &prelude.parse_output,
                &prelude_arena,
                prelude_path,
                prelude_canon.as_ref(),
                cycle_guard,
            ) {
                Ok(imported_prelude) => imported_prelude.register_public_functions(&mut env),
                Err(prelude_errors) => errors.extend(prelude_errors),
            }
        }

        // INVARIANT: visit every direct sibling even after one fails so import
        // diagnostics accumulate in source order.
        for imported in &resolved.modules {
            let import = &result.module.imports[imported.import_index];
            let imported_arena = imported.parse_output.arena.clone();
            let imported_canon = crate::query::canonicalize_module(
                db,
                &imported.parse_output,
                &imported.module_path,
                imported.source_file,
            );

            match Self::build_with_guard(
                db,
                &imported.parse_output,
                &imported_arena,
                &imported.module_path,
                imported_canon.as_ref(),
                cycle_guard,
            ) {
                Ok(imported_module) => {
                    if let Err(import_errors) = register_imports(
                        import,
                        &imported_module,
                        &mut env,
                        db.interner(),
                        &imported.module_path,
                        module_path,
                        imported_canon.as_ref(),
                    ) {
                        errors.extend(import_errors);
                    }
                }
                Err(mut import_errors) => {
                    for error in &mut import_errors {
                        if error.span.is_none() {
                            error.span = Some(import.span);
                        }
                    }
                    errors.extend(import_errors);
                }
            }
        }

        ori_eval::register_module_bindings(&result.module, arena, &mut env, canon);
        let functions = env.capture().into_iter().collect();
        let imported = ImportedModule { result, functions };

        if errors.is_empty() {
            Ok(imported)
        } else {
            Err(errors)
        }
    }

    fn register_public_functions(&self, env: &mut Environment) {
        for function in &self.result.module.functions {
            if function.visibility.is_public() {
                if let Some(value) = self.functions.get(&function.name) {
                    env.define_global(function.name, value.clone());
                }
            }
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
        let mut function_values = FxHashMap::default();

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

            function_values.insert(func.name, func_value);
        }
        FunctionValue::attach_module_scope(&mut function_values);
        function_values
            .into_iter()
            .map(|(name, function)| (name, Value::Function(function)))
            .collect()
    }

    /// Capture the importing environment together with every function owned by
    /// this module.
    ///
    /// Imported methods need the same lexical view as imported functions: they
    /// may call bindings already visible to the consumer as well as private
    /// helpers from their defining module.
    pub(crate) fn shared_captures(&self, env: &Environment) -> Arc<FxHashMap<Name, Value>> {
        let mut captures = env.capture();
        for (name, value) in &self.functions {
            captures.insert(*name, value.clone());
        }
        Arc::new(captures)
    }
}

fn circular_import_error(cycle: &[PathBuf]) -> ImportError {
    let path = cycle
        .iter()
        .map(|entry| entry.display().to_string())
        .collect::<Vec<_>>()
        .join(" -> ");
    ImportError::new(
        ImportErrorKind::CircularImport,
        format!("circular import detected: {path}"),
    )
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
    if let Some(alias) = import.module_alias {
        return register_module_alias(import, imported, env, alias, interner, import_path, canon)
            .map_err(|e| vec![e]);
    }

    let allow_private_access =
        is_test_module(current_file) && is_parent_module_import(current_file, import_path);

    // Why: `Name` keys avoid interner lookups outside the cold diagnostic path.
    let func_by_name: FxHashMap<Name, &crate::ir::Function> = imported
        .result
        .module
        .functions
        .iter()
        .map(|f| (f.name, f))
        .collect();

    let mut errors = Vec::new();

    for item in &import.items {
        if item.is_constant {
            let Some(constant) = imported
                .result
                .module
                .consts
                .iter()
                .find(|constant| constant.name == item.name)
            else {
                errors.push(ImportError::with_span(
                    ImportErrorKind::ItemNotFound,
                    format!(
                        "constant '${}' not found in '{}'",
                        interner.lookup(item.name),
                        import_path.display()
                    ),
                    import.span,
                ));
                continue;
            };

            if !constant.visibility.is_public() && !allow_private_access {
                let name = interner.lookup(item.name);
                errors.push(ImportError::with_span(
                    ImportErrorKind::PrivateAccess,
                    format!(
                        "constant '${name}' is private in '{}'; add `pub` to the constant definition before importing it",
                        import_path.display()
                    ),
                    import.span,
                ));
            }

            // Canon replaced every successful module-constant reference with
            // `CanExpr::Constant`; there is intentionally no runtime binding
            // and no second evaluator at this import boundary.
            continue;
        }

        if let Some(&func) = func_by_name.get(&item.name) {
            // INVARIANT: private imports require `::` outside parent test modules.
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

            // Use alias if provided, otherwise use original name
            let bind_name = item.alias.unwrap_or(item.name);
            let value = imported
                .functions
                .get(&func.name)
                .cloned()
                .unwrap_or_else(|| {
                    unreachable!("imported function inventory must cover every parsed function")
                });
            env.define(bind_name, value, Mutability::Immutable);
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
    interner: &StringInterner,
    import_path: &Path,
    _canon: Option<&SharedCanonResult>,
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

    // Why: namespace iteration must remain deterministic.
    let mut namespace: BTreeMap<Name, Value> = BTreeMap::new();

    for func in &imported.result.module.functions {
        if func.visibility.is_public() {
            let value = imported
                .functions
                .get(&func.name)
                .cloned()
                .unwrap_or_else(|| {
                    unreachable!("imported function inventory must cover every parsed function")
                });

            // INVARIANT: canonical alias calls and namespace dispatch share this binding.
            let qualified = interner.intern(&ori_ir::qualified_alias_name(
                interner.lookup(alias),
                interner.lookup(func.name),
            ));
            env.define(qualified, value.clone(), Mutability::Immutable);

            namespace.insert(func.name, value);
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
