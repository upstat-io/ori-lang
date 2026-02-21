//! Shared interpreter setup for module content registration.
//!
//! Extracts the common boilerplate for registering module functions, variant
//! constructors, newtype constructors, `impl`/`extend`/`def_impl` methods, and
//! derived traits into a single function. Used by both the playground WASM
//! and (optionally) the `oric` CLI.

use ori_eval::{
    collect_def_impl_methods_with_config, collect_extend_methods_with_config,
    collect_impl_methods_with_config, process_derives, register_module_functions,
    register_newtype_constructors, register_variant_constructors, DefaultFieldTypeRegistry,
    Interpreter, MethodCollectionConfig, UserMethodRegistry,
};
use ori_ir::canon::SharedCanonResult;
use ori_ir::StringInterner;
use ori_parse::ParseOutput;

/// Register all local module content into an interpreter.
///
/// Handles: module functions, variant constructors, newtype constructors,
/// `impl`/`extend`/`def_impl` methods, derived traits. Does NOT handle imports
/// (Salsa-dependent) or prelude loading from disk.
pub fn setup_module(
    interpreter: &mut Interpreter<'_>,
    parse_result: &ParseOutput,
    interner: &StringInterner,
    canon: Option<&SharedCanonResult>,
) {
    let shared_arena = parse_result.arena.clone();

    // Register all functions from the module into the environment
    register_module_functions(
        &parse_result.module,
        &shared_arena,
        interpreter.env_mut(),
        canon,
    );

    // Register variant constructors from sum type declarations
    register_variant_constructors(&parse_result.module, interpreter.env_mut());

    // Register newtype constructors from type declarations
    register_newtype_constructors(&parse_result.module, interpreter.env_mut());

    // Build user method registry from impl and extend blocks
    let mut user_methods = UserMethodRegistry::new();
    #[expect(
        clippy::disallowed_types,
        reason = "MethodCollectionConfig.captures requires Arc"
    )]
    let captures = std::sync::Arc::new(interpreter.env().capture());
    let config = MethodCollectionConfig {
        module: &parse_result.module,
        arena: &shared_arena,
        captures,
        canon,
        interner,
    };
    collect_impl_methods_with_config(&config, &mut user_methods);
    collect_extend_methods_with_config(&config, &mut user_methods);
    collect_def_impl_methods_with_config(&config, &mut user_methods);

    // Process derived traits (Eq, Clone, Hashable, Printable, Default)
    let mut default_ft = DefaultFieldTypeRegistry::new();
    process_derives(
        &parse_result.module,
        &mut user_methods,
        &mut default_ft,
        interner,
    );

    // Merge the collected methods into the interpreter's registry
    interpreter
        .user_method_registry()
        .write()
        .merge(user_methods);
    interpreter.default_field_types().write().merge(default_ft);
}
