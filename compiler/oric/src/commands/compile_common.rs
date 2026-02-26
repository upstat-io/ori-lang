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
//! - **ArtifactCache** handles the back-end: object code caching (future).
//!   Codegen is **not** a Salsa query because LLVM types (`Module`,
//!   `FunctionValue`, `BasicBlock`) are lifetime-bound to an LLVM `Context`
//!   and do not satisfy Salsa's `Clone + Eq + Hash` requirements.
//!
//! - **ARC borrow inference** is now fully Salsa-tracked via per-SCC queries
//!   (Section 12). See [`run_borrow_inference`] for the Salsa integration point.

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

/// Result of borrow inference: annotated signatures + pre-lowered ARC cache.
///
/// The `arc_cache` contains pre-lowered `ArcFunction`s grouped by parent
/// function. These are consumed by `define_all_cached` during codegen,
/// eliminating the redundant second lowering pass.
#[cfg(feature = "llvm")]
struct BorrowInferenceResult {
    /// Borrow-annotated function signatures from SCC analysis.
    sigs: FxHashMap<Name, ori_arc::AnnotatedSig>,
    /// Pre-lowered ARC functions: parent → (ArcFunction, lambdas).
    arc_cache: FxHashMap<Name, (ori_arc::ArcFunction, Vec<ori_arc::ArcFunction>)>,
}

/// Run ARC borrow inference on all non-generic module functions.
///
/// Lowers each function to ARC IR and runs per-SCC Salsa-tracked borrow
/// inference queries. Returns both the annotated signatures and a cache of
/// pre-lowered ARC functions for zero-copy consumption by codegen.
///
/// # Flow
///
/// 1. Lower each function to ARC IR
/// 2. Clone into flat map for Salsa, keep grouped cache for codegen
/// 3. Create [`ArcModuleInput`] Salsa input from lowered functions
/// 4. Query [`arc_scc_decomposition`] for SCC structure
/// 5. Query [`infer_borrow_scc`] per SCC (creates Salsa dependency edges)
/// 6. Return `(annotated_sigs, arc_cache)`
///
/// Salsa memoizes per-SCC results. On recompilation, only SCCs with changed
/// function bodies are re-analyzed. Early cutoff skips dependent SCCs when
/// borrow signatures are unchanged.
#[cfg(feature = "llvm")]
fn run_borrow_inference(
    db: &CompilerDb,
    parse_result: &ParseOutput,
    function_sigs: &[FunctionSig],
    canon: &CanonResult,
    interner: &StringInterner,
    pool: &Pool,
    source_path: &str,
    mono_instances: &[ori_types::MonoInstance],
) -> BorrowInferenceResult {
    use crate::query::arc_queries::{arc_scc_decomposition, infer_borrow_scc, ArcModuleInput};

    // 1. Lower functions to ARC IR.
    // We build both a grouped cache (parent → lambdas) for codegen and a flat
    // map for Salsa. The flat map is cloned from the grouped data before being
    // consumed by ArcModuleInput::sorted_functions.
    let mut arc_cache: FxHashMap<Name, (ori_arc::ArcFunction, Vec<ori_arc::ArcFunction>)> =
        FxHashMap::default();
    let mut arc_problems = Vec::new();

    for (func, sig) in parse_result
        .module
        .functions
        .iter()
        .zip(function_sigs.iter())
    {
        if sig.is_generic() {
            continue;
        }
        let (arc_fn, lambdas) = crate::arc_lowering::lower_to_arc(
            func.name,
            sig,
            func.name,
            canon,
            interner,
            pool,
            &mut arc_problems,
            None,
        );
        arc_cache.insert(arc_fn.name, (arc_fn, lambdas));
    }

    // Lower monomorphized generic functions.
    // The JIT runner already does this (runner/mod.rs), but the AOT path was
    // missing it — mono function names would be absent from annotated_sigs,
    // causing the warn! fallback to all-Owned in define_function_body_arc_with_subst.
    let mono_functions = ori_llvm::monomorphize::collect_mono_functions(
        mono_instances,
        function_sigs,
        interner,
        pool,
    );
    for mono_fn in &mono_functions {
        let (arc_fn, lambdas) = crate::arc_lowering::lower_to_arc(
            mono_fn.mangled_name,
            &mono_fn.sig,
            mono_fn.original_name,
            canon,
            interner,
            pool,
            &mut arc_problems,
            Some(&mono_fn.body_type_map),
        );
        arc_cache.insert(arc_fn.name, (arc_fn, lambdas));
    }

    if !arc_problems.is_empty() {
        use crate::problem::codegen::{emit_codegen_diagnostics, CodegenDiagnostics};
        let mut acc = CodegenDiagnostics::new();
        acc.add_arc_problems(&arc_problems);
        if emit_codegen_diagnostics(acc) {
            return BorrowInferenceResult {
                sigs: FxHashMap::default(),
                arc_cache: FxHashMap::default(),
            };
        }
    }

    // 2. Build flat map for Salsa (clone from grouped cache).
    // The clone cost is negligible vs borrow inference + LLVM codegen,
    // and it replaces a full second lowering pass.
    let mut arc_functions_map: FxHashMap<Name, ori_arc::ArcFunction> = FxHashMap::default();
    for (parent, lambdas) in arc_cache.values() {
        arc_functions_map.insert(parent.name, parent.clone());
        for lambda in lambdas {
            arc_functions_map.insert(lambda.name, lambda.clone());
        }
    }

    // 3. Create ArcModuleInput Salsa input.
    debug_assert!(
        db.pool_cache()
            .get(&std::path::PathBuf::from(source_path))
            .is_some(),
        "Pool not cached for source_path '{source_path}' — path may diverge from file.path(db)"
    );

    let sorted_functions = ArcModuleInput::sorted_functions(arc_functions_map);
    let module = ArcModuleInput::new(db, std::path::PathBuf::from(source_path), sorted_functions);

    tracing::debug!(
        function_count = module.functions(db).len(),
        "created ArcModuleInput for Salsa borrow inference"
    );

    // 4. Query SCC decomposition (Salsa-tracked).
    let decomp = arc_scc_decomposition(db, module);

    // 5. Query per-SCC borrow inference and collect results.
    let mut annotated_sigs = FxHashMap::default();
    #[expect(
        clippy::cast_possible_truncation,
        reason = "SCC count bounded by function count, fits in u32"
    )]
    for i in 0..decomp.len() {
        let result = infer_borrow_scc(db, module, i as u32);
        for (name, sig) in result.iter() {
            annotated_sigs.insert(*name, sig.clone());
        }
    }

    tracing::debug!(
        sig_count = annotated_sigs.len(),
        scc_count = decomp.len(),
        "Salsa borrow inference complete"
    );

    BorrowInferenceResult {
        sigs: annotated_sigs,
        arc_cache,
    }
}

/// Run the codegen pipeline on a pre-checked module.
///
/// Shared implementation for [`compile_to_llvm`] and [`compile_to_llvm_with_imports`].
/// The pipeline:
/// 1. Declares runtime functions
/// 2. Registers user-defined types
/// 3. Runs ARC borrow inference (per-SCC Salsa queries)
/// 4. Two-pass function compilation (declare, then define)
/// 5. Monomorphization of generic functions
/// 6. Impl method and derived trait compilation
/// 7. Main wrapper generation
///
/// `symbol_prefix` controls symbol mangling: `""` for single-file (no module
/// prefix on symbols), or the module name for multi-file compilation.
/// `import_sigs` declares external symbols from other modules; pass `&[]` for
/// single-file compilation.
#[cfg(feature = "llvm")]
#[expect(
    clippy::too_many_arguments,
    reason = "private pipeline helper — params match the data flow from both public compile functions"
)]
#[allow(unsafe_code, reason = "LLVM C API requires unsafe FFI calls")]
fn run_codegen_pipeline<'ctx>(
    context: &'ctx Context,
    db: &CompilerDb,
    parse_result: &ParseOutput,
    type_result: &TypeCheckResult,
    pool: &'ctx Pool,
    canon: &CanonResult,
    source_path: &str,
    module_name: &str,
    symbol_prefix: &str,
    import_sigs: &[(Name, FunctionSig)],
) -> ori_llvm::inkwell::module::Module<'ctx> {
    use ori_llvm::codegen::function_compiler::FunctionCompiler;
    use ori_llvm::codegen::ir_builder::IrBuilder;
    use ori_llvm::codegen::runtime_decl;
    use ori_llvm::codegen::type_info::{TypeInfoStore, TypeLayoutResolver};
    use ori_llvm::codegen::type_registration;
    use ori_llvm::SimpleCx;

    use std::mem::ManuallyDrop;

    let interner = db.interner();

    // ManuallyDrop + raw-pointer reborrow to work around a borrow checker
    // limitation: FunctionCompiler's lifetime parameters tie the compilation
    // block's borrow of `scx` to the return lifetime, preventing us from
    // consuming `scx` afterward. The raw-pointer roundtrip creates a detached
    // reference whose borrow doesn't leak out of the block. Sound because `scx`
    // lives for the entire function and compilation borrows end at the block
    // boundary.
    let scx = ManuallyDrop::new(SimpleCx::new(context, module_name));

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

        // 3. Run ARC borrow inference pipeline (per-SCC Salsa queries)
        // Returns both annotated sigs and pre-lowered ARC functions to
        // eliminate the redundant second lowering pass during codegen.
        let function_sigs = oric::typeck::build_function_sigs(parse_result, type_result);
        let classifier = ori_arc::ArcClassifier::new(pool);
        let BorrowInferenceResult {
            sigs: annotated_sigs,
            mut arc_cache,
        } = run_borrow_inference(
            db,
            parse_result,
            &function_sigs,
            canon,
            interner,
            pool,
            source_path,
            &type_result.typed.mono_instances,
        );

        // 4. Two-pass function compilation with borrow annotations
        let mut fc = FunctionCompiler::new(
            &mut builder,
            &store,
            &resolver,
            interner,
            pool,
            symbol_prefix,
            &annotated_sigs,
            &classifier,
            None, // Debug info wiring deferred to AOT pipeline integration
        );

        // Declare imports (no-op when import_sigs is empty for single-file compilation)
        if !import_sigs.is_empty() {
            fc.declare_imports(import_sigs);
        }
        fc.declare_all(&parse_result.module.functions, &function_sigs);

        // 4b. Collect and declare monomorphized generic functions
        let mono_functions = ori_llvm::monomorphize::collect_mono_functions(
            &type_result.typed.mono_instances,
            &function_sigs,
            interner,
            pool,
        );
        fc.declare_mono_functions(&mono_functions);

        // 5. Compile impl methods (still inline — they use type-qualified
        // canon lookup paths and are not pre-lowered for borrow inference)
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

        // 6. Two-pass function compilation for sound nounwind analysis:
        //    a) Lower all functions to ARC IR (no LLVM emission)
        //    b) Build complete nounwind set via fixed-point analysis
        //    c) Emit LLVM IR using the complete nounwind set
        let mut prepared = fc.prepare_all_cached(
            &parse_result.module.functions,
            &function_sigs,
            canon,
            &mut arc_cache,
        );

        // 6b. Prepare monomorphized function bodies from pre-lowered cache.
        prepared.extend(fc.prepare_mono_cached(&mono_functions, canon, &mut arc_cache));

        // 6c. Build complete nounwind set and emit LLVM IR
        fc.compute_nounwind_set(&prepared);
        fc.emit_prepared_functions(prepared);

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

    // SAFETY: ManuallyDrop is used only to suppress the borrow checker.
    // The compilation block's borrows have ended; we extract the module
    // by reading the field (Module implements Clone via LLVMCloneModule).
    // We can't call into_inner() because SimpleCx has other fields that
    // would be moved while the ManuallyDrop still exists, so we clone
    // the module instead.
    scx.llmod.clone()
}

/// Compile source to LLVM IR.
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
pub fn compile_to_llvm<'ctx>(
    context: &'ctx Context,
    db: &CompilerDb,
    parse_result: &ParseOutput,
    type_result: &TypeCheckResult,
    pool: &'ctx Pool,
    canon: &CanonResult,
    source_path: &str,
) -> ori_llvm::inkwell::module::Module<'ctx> {
    let module_name = Path::new(source_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("module");

    let llvm_module = run_codegen_pipeline(
        context,
        db,
        parse_result,
        type_result,
        pool,
        canon,
        source_path,
        module_name,
        "", // No symbol prefix for single-file compilation
        &[],
    );

    if std::env::var("ORI_DEBUG_LLVM").is_ok() {
        eprintln!("=== LLVM IR for {module_name} ===");
        eprintln!("{}", llvm_module.print_to_string());
        eprintln!("=== END IR ===");
    }

    llvm_module
}

/// Compile source to LLVM IR with explicit module name and import declarations.
///
/// This is used for multi-file compilation where:
/// - The module name is explicitly provided for proper symbol mangling
/// - Imported functions are declared as external symbols
///
/// The `arc_cache` and `module_hash` parameters are reserved for future ARC IR
/// disk caching integration with the Salsa path (Section 12.14 watch-mode).
/// Currently unused — they are accepted but discarded.
///
/// The Pool is required for proper compound type resolution during codegen.
/// The `CanonResult` provides canonical IR for both `ori_arc` and `ori_llvm`.
#[cfg(feature = "llvm")]
#[allow(
    clippy::too_many_arguments,
    reason = "multi-module compilation needs all parameters"
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
    // arc_cache and module_hash reserved for future ARC IR disk caching
    // integration with the Salsa path (Section 12.14 watch-mode).
    let _ = (arc_cache, module_hash);

    let interner = db.interner();
    let import_sigs: Vec<(Name, FunctionSig)> = imported_functions
        .iter()
        .map(|info| {
            let name = interner.intern(&info.mangled_name);
            // Generate synthetic param names — compute_function_abi() zips
            // param_names with param_types, so they must be parallel.
            let param_names: Vec<Name> = (0..info.param_types.len())
                .map(|i| interner.intern(&format!("_p{i}")))
                .collect();
            let sig = FunctionSig::synthetic(
                name,
                param_names,
                info.param_types.clone(),
                info.return_type,
            );
            (name, sig)
        })
        .collect();

    let llvm_module = run_codegen_pipeline(
        context,
        db,
        parse_result,
        type_result,
        pool,
        canon,
        source_path,
        module_name,
        module_name, // Multi-file: symbol prefix matches module name
        &import_sigs,
    );

    if std::env::var("ORI_DEBUG_LLVM").is_ok() {
        eprintln!(
            "Compiled module '{}' from '{}' with {} imported functions",
            module_name,
            source_path,
            imported_functions.len()
        );
        eprintln!("{}", llvm_module.print_to_string());
    }

    llvm_module
}
