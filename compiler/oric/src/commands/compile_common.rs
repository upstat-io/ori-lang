//! Shared compilation utilities for AOT build and run commands.
//!
//! # Salsa/ArtifactCache Boundary
//!
//! The compilation pipeline uses a **hybrid caching strategy**:
//!
//! - **Salsa** handles the front-end: `SourceFile → tokens() → parsed() → typed()`.
//!   Salsa's early cutoff skips dependent queries when results are unchanged
//!   (e.g., whitespace-only edits don't trigger re-parsing).
//!
//! - **`ArtifactCache`** handles the back-end: object code caching (future).
//!   Codegen is **not** a Salsa query because LLVM types (`Module`,
//!   `FunctionValue`, `BasicBlock`) are lifetime-bound to an LLVM `Context`
//!   and do not satisfy Salsa's `Clone + Eq + Hash` requirements.
//!
//! - **ARC/AIMS realization** closes one complete body batch before any
//!   physical backend projects it.

#[cfg(feature = "llvm")]
use std::path::Path;

#[cfg(feature = "llvm")]
use ori_diagnostic::emitter::{ColorMode, DiagnosticEmitter, TerminalEmitter};
#[cfg(feature = "llvm")]
use ori_ir::canon::CanonResult;
#[cfg(feature = "llvm")]
use ori_llvm::inkwell::context::Context;
#[cfg(feature = "llvm")]
use ori_repr::RealizedProgram;
#[cfg(feature = "llvm")]
use ori_types::{FunctionSig, Idx, Pool, TypeCheckResult};
#[cfg(feature = "llvm")]
use oric::ir::Name;
#[cfg(feature = "llvm")]
use oric::parser::ParseOutput;
#[cfg(feature = "llvm")]
use oric::{CompilerDb, Db, SourceFile};

#[cfg(feature = "llvm")]
use super::backend::{BackendChoice, LlvmBackend};

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
    /// Required producer-authored ownership/effect compatibility facts.
    pub metadata: ori_repr::executable::ExternalCallableMetadata,
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
#[derive(Clone, Copy)]
pub struct ImportedMonoCompilation<'ctx, 'a> {
    pub context: &'ctx Context,
    pub db: &'a CompilerDb,
    pub parse: &'a ParseOutput,
    pub typed: &'a TypeCheckResult,
    pub pool: &'ctx Pool,
    pub imported: super::ImportedSurfaces<'a>,
    pub canon: &'a CanonResult,
    pub source_path: &'a str,
    pub target_triple: Option<&'a str>,
    pub narrowing_policy: ori_repr::NarrowingPolicy,
}

#[cfg(feature = "llvm")]
pub fn compile_to_llvm_with_imported_monos<'ctx>(
    input: ImportedMonoCompilation<'ctx, '_>,
) -> Result<ori_llvm::inkwell::module::Module<'ctx>, String> {
    let ImportedMonoCompilation {
        context,
        db,
        parse: parse_result,
        typed: type_result,
        pool: merged_pool,
        imported,
        canon,
        source_path,
        target_triple,
        narrowing_policy,
    } = input;
    let module_name = Path::new(source_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("module");

    let program = RealizedProgram {
        type_result,
        pool: merged_pool,
        canon,
        source_path,
        module_name,
        symbol_prefix: "", // No symbol prefix for single-file compilation
        target_triple,
        narrowing_policy,
        imported_type_metadata: &[], // Single-file: no imported type metadata
        imported_collection_surfaces: &[], // Single-file: no imported collection surfaces
    };
    let backend = BackendChoice::Llvm(LlvmBackend {
        context,
        db,
        parse_result,
        import_sigs: &[],
        imported,
    });

    backend
        .compile(&program)
        .map(|output| output.module)
        .map_err(|e| e.0)
}

/// Compile source to LLVM IR with explicit module name and import
/// declarations, for multi-file compilation.
///
/// Imported functions are declared as external symbols; the module name
/// drives symbol mangling. `imported` carries the `ImportedMonoFn` entries
/// plus re-interned canons for cross-module generic body specialization
/// (both empty when the host has no imported generic instantiations).
#[cfg(feature = "llvm")]
#[derive(Clone, Copy)]
pub struct ImportedModuleCompilation<'ctx, 'a> {
    pub context: &'ctx Context,
    pub db: &'a CompilerDb,
    pub parse: &'a ParseOutput,
    pub typed: &'a TypeCheckResult,
    pub pool: &'ctx Pool,
    pub canon: &'a CanonResult,
    pub source_path: &'a str,
    pub module_name: &'a str,
    pub imported_functions: &'a [ImportedFunctionInfo],
    pub imported_type_metadata: &'a [ori_types::ExportedTypeMetadata],
    pub imported_collection_surfaces: &'a [u64],
    pub imported: super::ImportedSurfaces<'a>,
    pub target_triple: Option<&'a str>,
    pub narrowing_policy: ori_repr::NarrowingPolicy,
}

#[cfg(feature = "llvm")]
pub fn compile_to_llvm_with_imports<'ctx>(
    input: ImportedModuleCompilation<'ctx, '_>,
) -> Result<super::codegen_pipeline::LlvmCodegenOutput<'ctx>, String> {
    let ImportedModuleCompilation {
        context,
        db,
        parse: parse_result,
        typed: type_result,
        pool,
        canon,
        source_path,
        module_name,
        imported_functions,
        imported_type_metadata,
        imported_collection_surfaces,
        imported,
        target_triple,
        narrowing_policy,
    } = input;
    let interner = db.interner();
    // Registration key = the call-site local/aliased name (matching the ARC
    // IR callee Name resolve_callee probes); the LLVM extern symbol stays the
    // exporting module's exact mangled name. A function the host never
    // imports by name keeps its mangled-name key (unreachable from ARC IR).
    let import_sigs: Vec<ori_repr::monomorphize::ImportSig> = imported_functions
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
            ori_repr::monomorphize::ImportSig {
                name,
                symbol: info.mangled_name.clone(),
                sig,
                metadata: info.metadata.clone(),
            }
        })
        .collect();

    let program = RealizedProgram {
        pool,
        type_result,
        canon,
        source_path,
        module_name,
        symbol_prefix: module_name, // Multi-file: symbol prefix matches module name
        target_triple,
        narrowing_policy,
        imported_type_metadata,
        imported_collection_surfaces,
    };
    let backend = BackendChoice::Llvm(LlvmBackend {
        context,
        db,
        parse_result,
        import_sigs: &import_sigs,
        imported,
    });

    backend.compile(&program).map_err(|e| e.0)
}
