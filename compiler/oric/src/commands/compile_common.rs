//! Shared compilation utilities for AOT build and run commands.
//!
//! This module extracts common compilation logic to avoid duplication between
//! `build_file` and `run_file_compiled`. Both commands need to:
//! 1. Parse and type-check source files
//! 2. Print accumulated errors
//! 3. Generate LLVM IR
//!
//! By centralizing this logic, bug fixes and enhancements apply to both commands.
//!
//! # Salsa/ArtifactCache Boundary
//!
//! The compilation pipeline uses a **hybrid caching strategy**:
//!
//! - **Salsa** handles the front-end: `SourceFile → tokens() → parsed() → typed()`.
//!   Salsa's early cutoff skips downstream queries when results are unchanged
//!   (e.g., whitespace-only edits don't trigger re-parsing).
//!
//! - **ArtifactCache** handles the back-end: ARC IR caching (Layer 1) and
//!   object code caching (Layer 2, future). Codegen is **not** a Salsa query
//!   because LLVM types (`Module`, `FunctionValue`, `BasicBlock`) are lifetime-
//!   bound to an LLVM `Context` and do not satisfy Salsa's `Clone + Eq + Hash`
//!   requirements.
//!
//! The handoff occurs after `typed()`: function content hashes are computed from
//! the `TypeCheckResult`, and the `ArcIrCache` checks whether ARC analysis can
//! be skipped. See [`run_arc_pipeline_cached`] for the cache integration point.

#[cfg(feature = "llvm")]
use std::path::Path;

#[cfg(feature = "llvm")]
use ori_diagnostic::emitter::{ColorMode, DiagnosticEmitter, TerminalEmitter};
#[cfg(feature = "llvm")]
use ori_ir::canon::CanonResult;
#[cfg(feature = "llvm")]
use ori_llvm::inkwell::context::Context;
#[cfg(feature = "llvm")]
use ori_types::{FunctionSig, Idx, Pool, TypeCheckResult};
#[cfg(feature = "llvm")]
use oric::ir::{Name, StringInterner};
#[cfg(feature = "llvm")]
use oric::parser::ParseOutput;
#[cfg(feature = "llvm")]
use oric::{CompilerDb, Db, SourceFile};
#[cfg(feature = "llvm")]
use rustc_hash::FxHashMap;

/// Information about an imported function for codegen.
#[cfg(feature = "llvm")]
#[derive(Debug, Clone)]
pub struct ImportedFunctionInfo {
    /// The mangled name of the function (e.g., `_ori_helper$add`).
    pub mangled_name: String,
    /// Parameter types as `Idx`.
    pub param_types: Vec<Idx>,
    /// Return type.
    pub return_type: Idx,
}

/// Check a source file for parse and type errors, then canonicalize.
///
/// Prints all errors to stderr and returns `None` if any errors occurred.
/// This accumulates all errors before reporting, giving users a complete picture.
///
/// Returns the Pool (as `Arc<Pool>`) and `SharedCanonResult` alongside parse/type
/// results so callers can pass them to LLVM codegen. The `SharedCanonResult` contains
/// the canonical IR that both `ori_arc` and `ori_llvm` backends will consume.
/// It is stored in `CanonCache` for session-scoped reuse.
///
/// Uses `typed()` Salsa query so the type check result is cached — subsequent calls
/// (e.g., from `evaluated()`) reuse the same result.
#[cfg(feature = "llvm")]
pub fn check_source(
    db: &CompilerDb,
    file: SourceFile,
    path: &str,
) -> Option<(
    ParseOutput,
    TypeCheckResult,
    std::sync::Arc<Pool>,
    ori_ir::canon::SharedCanonResult,
)> {
    // Create emitter with source context for rich snippet rendering
    let is_tty = std::io::IsTerminal::is_terminal(&std::io::stderr());
    let mut emitter = TerminalEmitter::with_color_mode(std::io::stderr(), ColorMode::Auto, is_tty)
        .with_source(file.text(db).as_str())
        .with_file_path(path);

    // Run frontend pipeline: lex → parse → typecheck, reporting all errors.
    // This catches lex errors (unterminated strings, etc.) that were previously
    // silent, and uses TypeErrorRenderer for consistent error quality.
    let frontend = super::report_frontend_errors(db, file, &mut emitter)?;

    if frontend.has_errors() {
        emitter.flush();
        return None;
    }

    // Canonicalize: AST + types → self-contained canonical IR.
    // Uses session-scoped CanonCache for reuse across consumers.
    let shared_canon = oric::query::canonicalize_cached(
        db,
        file,
        &frontend.parse_result,
        &frontend.type_result,
        &frontend.pool,
    );
    Some((
        frontend.parse_result,
        frontend.type_result,
        frontend.pool,
        shared_canon,
    ))
}

/// Run ARC borrow inference on all non-generic module functions.
///
/// Lowers each function to ARC IR, runs the iterative borrow inference
/// algorithm, and returns both:
/// - A map from function `Name` → `AnnotatedSig` with ownership annotations
/// - The lowered `ArcFunction`s (for reuse by downstream passes, avoiding re-lowering)
///
/// Generic functions are skipped (they require monomorphization first).
/// Functions that fail ARC IR lowering are skipped with a diagnostic.
#[cfg(feature = "llvm")]
fn run_borrow_inference(
    parse_result: &ParseOutput,
    function_sigs: &[FunctionSig],
    canon: &CanonResult,
    interner: &StringInterner,
    pool: &Pool,
    classifier: &ori_arc::ArcClassifier<'_>,
) -> (
    FxHashMap<Name, ori_arc::AnnotatedSig>,
    Vec<ori_arc::ArcFunction>,
) {
    let mut arc_functions = Vec::new();
    let mut arc_problems = Vec::new();

    for (func, sig) in parse_result
        .module
        .functions
        .iter()
        .zip(function_sigs.iter())
    {
        // Skip generic functions — they need monomorphization first
        if sig.is_generic() {
            continue;
        }

        let params: Vec<(Name, Idx)> = sig
            .param_names
            .iter()
            .zip(sig.param_types.iter())
            .map(|(&n, &t)| (n, t))
            .collect();

        // Look up the canonical root for this function.
        let body_id = canon.root_for(func.name).unwrap_or(canon.root);
        let (arc_fn, lambdas) = ori_arc::lower_function_can(
            func.name,
            &params,
            sig.return_type,
            body_id,
            canon,
            interner,
            pool,
            &mut arc_problems,
        );
        arc_functions.push(arc_fn);
        arc_functions.extend(lambdas);
    }

    // Surface ARC lowering issues as structured diagnostics (non-fatal)
    if !arc_problems.is_empty() {
        use crate::problem::codegen::{emit_codegen_diagnostics, CodegenDiagnostics};
        let mut acc = CodegenDiagnostics::new();
        acc.add_arc_problems(&arc_problems);
        emit_codegen_diagnostics(acc);
    }

    let sigs = ori_arc::infer_borrows(&arc_functions, classifier);
    (sigs, arc_functions)
}

/// Run ARC pipeline with optional caching.
///
/// On cache hit (same module hash): deserializes cached ARC IR and extracts
/// annotated signatures, skipping the full ARC analysis pipeline.
///
/// On cache miss: runs the full pipeline (lower → borrow inference → RC
/// insertion → elimination → reuse), serializes the result to the cache.
///
/// Returns the annotated signatures (needed by codegen for RC operations).
#[cfg(feature = "llvm")]
pub fn run_arc_pipeline_cached(
    parse_result: &ParseOutput,
    function_sigs: &[ori_types::FunctionSig],
    canon: &CanonResult,
    interner: &StringInterner,
    pool: &Pool,
    classifier: &ori_arc::ArcClassifier<'_>,
    arc_cache: Option<&ori_llvm::aot::incremental::ArcIrCache>,
    module_hash: Option<ori_llvm::aot::incremental::ContentHash>,
) -> FxHashMap<Name, ori_arc::AnnotatedSig> {
    // Try cache hit
    if let (Some(cache), Some(hash)) = (arc_cache, module_hash) {
        let key = ori_llvm::aot::incremental::arc_cache::ArcIrCacheKey {
            function_hash: hash,
        };

        if let Some(cached) = cache.get(&key) {
            if let Ok(arc_functions) = cached.to_arc_functions() {
                tracing::debug!("ARC IR cache hit — skipping ARC analysis");
                return ori_arc::infer_borrows(&arc_functions, classifier);
            }
            tracing::debug!("ARC IR cache corrupt — re-analyzing");
        }
    }

    // Cache miss — run borrow inference (returns both sigs and lowered functions)
    let (annotated_sigs, arc_functions) = run_borrow_inference(
        parse_result,
        function_sigs,
        canon,
        interner,
        pool,
        classifier,
    );

    // Cache the lowered (pre-pipeline) functions for next time.
    // The cache hit path only needs these for `infer_borrows`, which operates
    // on the raw lowered IR — it doesn't need post-pipeline RC ops. Running
    // the full optimization pipeline here would be wasted work since Tier 2
    // ARC codegen (which would consume the transformed IR) is not yet active.
    if let (Some(cache), Some(hash)) = (arc_cache, module_hash) {
        let key = ori_llvm::aot::incremental::arc_cache::ArcIrCacheKey {
            function_hash: hash,
        };

        if let Ok(cached) =
            ori_llvm::aot::incremental::arc_cache::CachedArcIr::from_arc_functions(&arc_functions)
        {
            if let Err(e) = cache.put(&key, &cached) {
                tracing::debug!("failed to write ARC IR cache: {e}");
            }
        }
    }

    annotated_sigs
}

/// Compile source to LLVM IR using the V2 codegen pipeline.
///
/// Takes checked parse and type results and generates LLVM IR via:
/// 1. `TypeInfoStore` + `TypeLayoutResolver` for LLVM type computation
/// 2. `IrBuilder` for instruction emission
/// 3. `FunctionCompiler` for two-pass declare-then-define compilation
///
/// The Pool is required for proper compound type resolution during codegen
/// (e.g., determining which return types need the sret calling convention).
///
/// The `CanonResult` provides canonical IR for both `ori_arc` and `ori_llvm`.
#[cfg(feature = "llvm")]
#[allow(unsafe_code, reason = "LLVM C API requires unsafe FFI calls")]
pub fn compile_to_llvm<'ctx>(
    context: &'ctx Context,
    db: &CompilerDb,
    parse_result: &ParseOutput,
    type_result: &TypeCheckResult,
    pool: &'ctx Pool,
    canon: &CanonResult,
    source_path: &str,
) -> ori_llvm::inkwell::module::Module<'ctx> {
    use ori_llvm::codegen::function_compiler::FunctionCompiler;
    use ori_llvm::codegen::ir_builder::IrBuilder;
    use ori_llvm::codegen::runtime_decl;
    use ori_llvm::codegen::type_info::{TypeInfoStore, TypeLayoutResolver};
    use ori_llvm::codegen::type_registration;
    use ori_llvm::SimpleCx;

    use std::mem::ManuallyDrop;

    let interner = db.interner();
    let module_name = Path::new(source_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("module");

    // We use ManuallyDrop + raw-pointer reborrow to work around a borrow
    // checker limitation: FunctionCompiler's lifetime parameters tie the
    // compilation block's borrow of `scx` to the return lifetime, preventing
    // us from consuming `scx` afterward. The raw-pointer roundtrip creates
    // a detached reference whose borrow doesn't leak out of the block.
    // This is sound because `scx` lives for the entire function and the
    // compilation block's borrows genuinely end at the block boundary.
    let scx = ManuallyDrop::new(SimpleCx::new(context, module_name));

    // V2 pipeline
    {
        // SAFETY: Detached reference to scx — see comment above.
        let scx_ref: &SimpleCx<'_> = unsafe { &*std::ptr::from_ref(&*scx) };

        let store = TypeInfoStore::new(pool);
        let resolver = TypeLayoutResolver::new(&store, scx_ref);
        let mut builder = IrBuilder::new(scx_ref);

        // 1. Declare runtime functions
        runtime_decl::declare_runtime(&mut builder);

        // 2. Register user-defined types
        type_registration::register_user_types(&resolver, &type_result.typed.types);

        // 3. Run ARC borrow inference pipeline (uses same code path as multi-file)
        let function_sigs = oric::typeck::build_function_sigs(parse_result, type_result);
        let classifier = ori_arc::ArcClassifier::new(pool);
        let annotated_sigs = run_arc_pipeline_cached(
            parse_result,
            &function_sigs,
            canon,
            interner,
            pool,
            &classifier,
            None, // No cache for single-file path
            None, // No module hash
        );

        // 4. Two-pass function compilation with borrow annotations
        let mut fc = FunctionCompiler::new(
            &mut builder,
            &store,
            &resolver,
            interner,
            pool,
            "",
            Some(&annotated_sigs),
            Some(&classifier),
            None, // Debug info wiring deferred to AOT pipeline integration
        );
        fc.declare_all(&parse_result.module.functions, &function_sigs);

        // 5. Compile impl methods
        if !parse_result.module.impls.is_empty() {
            fc.compile_impls(
                &parse_result.module.impls,
                &type_result.typed.impl_sigs,
                canon,
                &parse_result.module.traits,
            );
        }

        // 5b. Compile derived trait methods
        if parse_result
            .module
            .types
            .iter()
            .any(|t| !t.derives.is_empty())
        {
            fc.compile_derives(&parse_result.module, &type_result.typed.types);
        }

        // 6. Define all function bodies
        fc.define_all(&parse_result.module.functions, &function_sigs, canon);

        // 7. Generate C main() entry point wrapper for @main (AOT only)
        // Also detect @panic handler for registration in main()
        let panic_name = parse_result
            .module
            .functions
            .iter()
            .find(|f| interner.lookup(f.name) == "panic")
            .map(|f| f.name);

        for (func, sig) in parse_result
            .module
            .functions
            .iter()
            .zip(function_sigs.iter())
        {
            if sig.is_main {
                fc.generate_main_wrapper(func.name, sig, panic_name);
                break;
            }
        }
    }

    // Debug IR output
    if std::env::var("ORI_DEBUG_LLVM").is_ok() {
        eprintln!("=== LLVM IR for {module_name} ===");
        eprintln!("{}", scx.llmod.print_to_string());
        eprintln!("=== END IR ===");
    }

    // SAFETY: ManuallyDrop is used only to suppress the borrow checker.
    // The compilation block's borrows have ended; we extract the module
    // by reading the field (Module implements Clone via LLVMCloneModule).
    // We can't call into_inner() because SimpleCx has other fields that
    // would be moved while the ManuallyDrop still exists, so we clone
    // the module instead.
    scx.llmod.clone()
}

/// Compile source to LLVM IR with explicit module name and import declarations.
///
/// Uses the V2 codegen pipeline. This is used for multi-file compilation where:
/// - The module name is explicitly provided for proper symbol mangling
/// - Imported functions are declared as external symbols
///
/// Optional `arc_cache` and `module_hash` parameters enable ARC IR caching.
/// When provided, unchanged modules skip ARC analysis entirely.
///
/// The Pool is required for proper compound type resolution during codegen.
/// The `CanonResult` provides canonical IR for both `ori_arc` and `ori_llvm`.
#[cfg(feature = "llvm")]
#[allow(
    unsafe_code,
    clippy::too_many_arguments,
    reason = "LLVM FFI requires unsafe; multi-module compilation needs all parameters"
)]
pub fn compile_to_llvm_with_imports<'ctx>(
    context: &'ctx Context,
    db: &CompilerDb,
    parse_result: &ParseOutput,
    type_result: &TypeCheckResult,
    pool: &'ctx Pool,
    canon: &CanonResult,
    source_path: &str,
    module_name: &str,
    imported_functions: &[ImportedFunctionInfo],
    arc_cache: Option<&ori_llvm::aot::incremental::ArcIrCache>,
    module_hash: Option<ori_llvm::aot::incremental::ContentHash>,
) -> ori_llvm::inkwell::module::Module<'ctx> {
    use ori_llvm::codegen::function_compiler::FunctionCompiler;
    use ori_llvm::codegen::ir_builder::IrBuilder;
    use ori_llvm::codegen::runtime_decl;
    use ori_llvm::codegen::type_info::{TypeInfoStore, TypeLayoutResolver};
    use ori_llvm::codegen::type_registration;
    use ori_llvm::SimpleCx;

    use std::mem::ManuallyDrop;

    let interner = db.interner();

    // ManuallyDrop + raw-pointer reborrow — see compile_to_llvm for rationale.
    let scx = ManuallyDrop::new(SimpleCx::new(context, module_name));

    // V2 pipeline
    {
        // SAFETY: Detached reference to scx — see compile_to_llvm comment.
        let scx_ref: &SimpleCx<'_> = unsafe { &*std::ptr::from_ref(&*scx) };

        let store = TypeInfoStore::new(pool);
        let resolver = TypeLayoutResolver::new(&store, scx_ref);
        let mut builder = IrBuilder::new(scx_ref);

        // 1. Declare runtime functions
        runtime_decl::declare_runtime(&mut builder);

        // 2. Register user-defined types
        type_registration::register_user_types(&resolver, &type_result.typed.types);

        // 3. Declare imported functions as external symbols
        let import_sigs: Vec<(Name, FunctionSig)> = imported_functions
            .iter()
            .map(|info| {
                let name = interner.intern(&info.mangled_name);
                let sig = FunctionSig {
                    name,
                    type_params: vec![],
                    param_names: vec![],
                    param_types: info.param_types.clone(),
                    return_type: info.return_type,
                    capabilities: vec![],
                    is_public: false,
                    is_test: false,
                    is_main: false,
                    type_param_bounds: vec![],
                    where_clauses: vec![],
                    generic_param_mapping: vec![],
                    required_params: info.param_types.len(),
                    param_defaults: vec![],
                };
                (name, sig)
            })
            .collect();

        // 4. Run ARC borrow inference pipeline (with optional caching)
        let function_sigs = oric::typeck::build_function_sigs(parse_result, type_result);
        let classifier = ori_arc::ArcClassifier::new(pool);
        let annotated_sigs = run_arc_pipeline_cached(
            parse_result,
            &function_sigs,
            canon,
            interner,
            pool,
            &classifier,
            arc_cache,
            module_hash,
        );

        // 5. Two-pass function compilation with borrow annotations
        let mut fc = FunctionCompiler::new(
            &mut builder,
            &store,
            &resolver,
            interner,
            pool,
            module_name,
            Some(&annotated_sigs),
            Some(&classifier),
            None, // Debug info wiring deferred to AOT pipeline integration
        );

        // Declare imports first so they're visible to function bodies
        fc.declare_imports(&import_sigs);
        fc.declare_all(&parse_result.module.functions, &function_sigs);

        // 6. Compile impl methods
        if !parse_result.module.impls.is_empty() {
            fc.compile_impls(
                &parse_result.module.impls,
                &type_result.typed.impl_sigs,
                canon,
                &parse_result.module.traits,
            );
        }

        // 6b. Compile derived trait methods
        if parse_result
            .module
            .types
            .iter()
            .any(|t| !t.derives.is_empty())
        {
            fc.compile_derives(&parse_result.module, &type_result.typed.types);
        }

        // 7. Define all function bodies
        fc.define_all(&parse_result.module.functions, &function_sigs, canon);

        // 8. Generate C main() entry point wrapper for @main (AOT only)
        // Also detect @panic handler for registration in main()
        let panic_name = parse_result
            .module
            .functions
            .iter()
            .find(|f| interner.lookup(f.name) == "panic")
            .map(|f| f.name);

        for (func, sig) in parse_result
            .module
            .functions
            .iter()
            .zip(function_sigs.iter())
        {
            if sig.is_main {
                fc.generate_main_wrapper(func.name, sig, panic_name);
                break;
            }
        }
    }

    // Debug output
    if std::env::var("ORI_DEBUG_LLVM").is_ok() {
        eprintln!(
            "Compiled module '{}' from '{}' with {} imported functions",
            module_name,
            source_path,
            imported_functions.len()
        );
        eprintln!("{}", scx.llmod.print_to_string());
    }

    scx.llmod.clone()
}
