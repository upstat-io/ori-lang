//! Shared compilation utilities for AOT build and run commands.
//!
//! # Salsa/ArtifactCache Boundary
//!
//! The compilation pipeline uses a **hybrid caching strategy**:
//!
//! - **Salsa** handles the front-end: `SourceFile → tokens() → parsed() → typed()`.
//!   Salsa's early cutoff skips downstream queries when results are unchanged
//!   (e.g., whitespace-only edits don't trigger re-parsing).
//!
//! - **`ArtifactCache`** handles the back-end: object code caching (future).
//!   Codegen is **not** a Salsa query because LLVM types (`Module`,
//!   `FunctionValue`, `BasicBlock`) are lifetime-bound to an LLVM `Context`
//!   and do not satisfy Salsa's `Clone + Eq + Hash` requirements.
//!
//! - **ARC borrow inference** is fully Salsa-tracked via per-SCC queries.
//!   See [`run_borrow_inference`] for the Salsa integration point.

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
use oric::ir::Name;
#[cfg(feature = "llvm")]
use oric::parser::ParseOutput;
#[cfg(feature = "llvm")]
use oric::{CompilerDb, Db, SourceFile};

/// Information about an imported function for codegen.
#[cfg(feature = "llvm")]
#[derive(Debug, Clone)]
pub struct ImportedFunctionInfo {
    /// The mangled name of the function (e.g., `_ori_helper$add`).
    pub mangled_name: String,
    /// The call-site local/aliased name from the importing module's `use`
    /// statement (`None` when the host does not import this function by
    /// name). ARC IR call sites carry this name; `codegen_ctx.functions`
    /// is keyed by it so `resolve_callee` finds the import.
    pub local_name: Option<String>,
    /// Parameter types as `Idx`.
    pub param_types: Vec<Idx>,
    /// Return type.
    pub return_type: Idx,
}

/// Check a source file for parse and type errors, then canonicalize.
///
/// Accumulates all errors, prints them to stderr, and returns `None` if any
/// occurred. Returns the `Arc<Pool>` and `SharedCanonResult` alongside the
/// parse/type results for LLVM codegen; the canonical IR is consumed by both
/// the `ori_arc` and `ori_llvm` backends and cached session-scoped in
/// `CanonCache`. Uses the `typed()` Salsa query so the type-check result is
/// reused by later consumers (e.g. `evaluated()`).
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

/// Compile source to LLVM IR with imported-mono state for cross-module
/// generic dispatch.
///
/// Used by single-file builds whose imports resolve to imported generic
/// instantiations (e.g. `assert_eq<int>` from `std.testing`). The caller
/// builds the `ImportedMonoState` ahead of this call via
/// `build_imported_mono_state`; `imported_state.merged_pool` MUST outlive the
/// returned LLVM module — `'ctx` ties context, pool, and module together.
#[cfg(feature = "llvm")]
#[expect(
    clippy::too_many_arguments,
    reason = "pipeline boundary mirrors compile_to_llvm + adds imported-mono state"
)]
pub fn compile_to_llvm_with_imported_monos<'ctx>(
    context: &'ctx Context,
    db: &CompilerDb,
    parse_result: &ParseOutput,
    type_result: &TypeCheckResult,
    merged_pool: &'ctx Pool,
    imported: super::ImportedSurfaces<'_>,
    canon: &CanonResult,
    source_path: &str,
    target_triple: Option<&str>,
    narrowing_policy: ori_repr::NarrowingPolicy,
) -> Result<ori_llvm::inkwell::module::Module<'ctx>, String> {
    let module_name = Path::new(source_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("module");

    super::codegen_pipeline::run_codegen_pipeline(
        context,
        db,
        parse_result,
        type_result,
        merged_pool,
        canon,
        source_path,
        module_name,
        "", // No symbol prefix for single-file compilation
        &[],
        imported,
        target_triple,
        narrowing_policy,
        &[], // Single-file: no imported type metadata
        &[], // Single-file: no imported collection surfaces
    )
}

/// Compile source to LLVM IR with explicit module name and import
/// declarations, for multi-file compilation.
///
/// Imported functions are declared as external symbols; the module name
/// drives symbol mangling. `imported` carries the `ImportedMonoFn` triples
/// plus re-interned canons for cross-module generic body specialization
/// (both empty when the host has no imported generic instantiations).
/// `arc_cache` / `module_hash` are reserved for ARC IR disk caching.
#[cfg(feature = "llvm")]
#[expect(
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
    imported_type_metadata: &[ori_types::ExportedTypeMetadata],
    imported_collection_surfaces: &[u64],
    imported: super::ImportedSurfaces<'_>,
    arc_cache: Option<&ori_llvm::aot::incremental::ArcIrCache>,
    module_hash: Option<ori_llvm::aot::incremental::ContentHash>,
    target_triple: Option<&str>,
    narrowing_policy: ori_repr::NarrowingPolicy,
) -> Result<ori_llvm::inkwell::module::Module<'ctx>, String> {
    // arc_cache and module_hash reserved for future ARC IR disk caching.
    let _ = (arc_cache, module_hash);

    let interner = db.interner();
    // Registration key = the call-site local/aliased name (matching the ARC
    // IR callee Name resolve_callee probes); the LLVM extern symbol stays the
    // exporting module's exact mangled name. A function the host never
    // imports by name keeps its mangled-name key (unreachable from ARC IR).
    let import_sigs: Vec<(Name, String, FunctionSig)> = imported_functions
        .iter()
        .map(|info| {
            let key = info.local_name.as_deref().unwrap_or(&info.mangled_name);
            let name = interner.intern(key);
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
            (name, info.mangled_name.clone(), sig)
        })
        .collect();

    super::codegen_pipeline::run_codegen_pipeline(
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
        imported,
        target_triple,
        narrowing_policy,
        imported_type_metadata,
        imported_collection_surfaces,
    )
}
