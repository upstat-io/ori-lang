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
//! - **`ArtifactCache`** handles the back-end: object code caching (future).
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

/// Compile source to LLVM IR with imported-mono state for cross-module
/// generic dispatch.
///
/// Used by single-file builds whose source contains imports that resolve
/// to imported generic instantiations (e.g., `assert_eq<int>` from
/// `std.testing`). The caller is responsible for building the
/// `ImportedMonoState` ahead of this call — typically via
/// `crate::commands::build::build_imported_mono_state`. The
/// `imported_state.merged_pool` MUST outlive the returned LLVM module;
/// `'ctx` ties the LLVM context, the merged pool, and the returned module
/// together so the borrow checker enforces this.
///
/// Without this entry point, single-file programs with stdlib imports
/// (e.g., `use std.testing { assert_eq }`) yield `unresolved function in
/// apply/invoke — missing mono instance?` at LLVM verification because
/// `collect_mono_functions` silently skips imported generic instances.
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
    imported_mono_fns: &[super::ImportedMonoFn],
    re_interned_canons: &[CanonResult],
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
        imported_mono_fns,
        re_interned_canons,
        target_triple,
        narrowing_policy,
        &[], // Single-file: no imported type metadata
        &[], // Single-file: no imported collection surfaces
    )
}

/// Compile source to LLVM IR with explicit module name and import declarations.
///
/// This is used for multi-file compilation where:
/// - The module name is explicitly provided for proper symbol mangling
/// - Imported functions are declared as external symbols
///
/// The `arc_cache` and `module_hash` parameters are reserved for future ARC IR
/// disk caching integration with the Salsa watch-mode path.
/// Currently unused — they are accepted but discarded.
///
/// The Pool is required for proper compound type resolution during codegen.
/// The `CanonResult` provides canonical IR for both `ori_arc` and `ori_llvm`.
///
/// `imported_mono_fns` carries promoted `ImportedMonoFn` triples for
/// cross-module imported-generic body specialization via body-import
/// linkage. Pass `&[]` when the host module has no imported generic
/// instantiations to lower locally.
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
    imported_type_metadata: &[ori_types::ExportedTypeMetadata],
    imported_collection_surfaces: &[u64],
    imported_mono_fns: &[super::ImportedMonoFn],
    re_interned_canons: &[CanonResult],
    arc_cache: Option<&ori_llvm::aot::incremental::ArcIrCache>,
    module_hash: Option<ori_llvm::aot::incremental::ContentHash>,
    target_triple: Option<&str>,
    narrowing_policy: ori_repr::NarrowingPolicy,
) -> Result<ori_llvm::inkwell::module::Module<'ctx>, String> {
    // arc_cache and module_hash reserved for future ARC IR disk caching.
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
        imported_mono_fns,
        re_interned_canons,
        target_triple,
        narrowing_policy,
        imported_type_metadata,
        imported_collection_surfaces,
    )
}
