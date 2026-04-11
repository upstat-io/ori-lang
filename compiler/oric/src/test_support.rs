//! Test support utilities for integration tests.
//!
//! Always-compiled (NOT `#[cfg(test)]`) so integration test binaries
//! in `compiler/oric/tests/` can use these functions. Integration tests
//! compile the normal library build — `#[cfg(test)]` items are invisible.
//!
//! Provides the compilation pipeline up to ARC IR without AIMS processing,
//! suitable for snapshot tests that capture per-pass ARC IR via the
//! checkpoint observer.

use ori_arc::ArcFunction;
use ori_ir::{Name, StringInterner};
use ori_types::Pool;
use rustc_hash::FxHashMap;

use crate::db::{CompilerDb, Db};
use crate::input::SourceFile;

/// Result of compiling a source file to pre-AIMS ARC IR.
pub struct ArcCompileResult {
    /// ARC functions grouped by parent -> lambdas.
    pub arc_cache: FxHashMap<Name, (ArcFunction, Vec<ArcFunction>)>,
    /// Type pool for type formatting.
    pub pool: Pool,
    /// The compiler database (owns the interner).
    pub db: CompilerDb,
}

impl ArcCompileResult {
    /// Get the string interner from the database.
    pub fn interner(&self) -> &StringInterner {
        self.db.interner()
    }

    /// Flatten the arc cache into a single Vec of all functions (parents + lambdas).
    ///
    /// This is the canonical flattening helper — also used by
    /// `commands/repr_setup::collect_all_arc_functions` in the production pipeline.
    pub fn all_arc_functions(&self) -> Vec<ArcFunction> {
        self.arc_cache
            .values()
            .flat_map(|(parent, lambdas)| {
                std::iter::once(parent.clone()).chain(lambdas.iter().cloned())
            })
            .collect()
    }
}

/// Compile a source file to ARC IR (pre-AIMS pipeline).
///
/// Runs the full frontend pipeline (parse -> typecheck -> canonicalize ->
/// ARC lowering) and returns the ARC function cache before any AIMS
/// processing. Used by snapshot tests to capture `lowered.arc` baselines
/// and then run the AIMS pipeline with an observer.
///
/// Returns `Err` if the source has parse or type errors.
pub fn compile_to_arc(source_path: &str, source_text: &str) -> Result<ArcCompileResult, String> {
    let db = CompilerDb::new();

    let file = SourceFile::new(&db, source_path.into(), source_text.into());

    // Parse.
    let parse_result = crate::query::parsed(&db, file);
    if parse_result.has_errors() {
        return Err(format!(
            "parse errors in {source_path}: {} error(s)",
            parse_result.errors.len()
        ));
    }

    // Typecheck.
    let type_result = crate::query::typed(&db, file);
    if type_result.has_errors() {
        return Err(format!(
            "type errors in {source_path}: {} error(s)",
            type_result.errors().len()
        ));
    }

    let pool_opt = crate::query::typed_pool(&db, file);
    let pool = match pool_opt {
        Some(p) => (*p).clone(),
        None => return Err("no type pool available".into()),
    };

    // Canonicalize.
    let shared_canon =
        crate::query::canonicalize_cached(&db, file, &parse_result, &type_result, &pool);
    let canon = &*shared_canon;

    // Build function signatures.
    let function_sigs = crate::typeck::build_function_sigs(&parse_result, &type_result);
    let interner = db.interner();

    // Lower each non-generic function to ARC IR.
    let mut arc_cache: FxHashMap<Name, (ArcFunction, Vec<ArcFunction>)> = FxHashMap::default();
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
            &pool,
            &mut arc_problems,
            None,
        );
        arc_cache.insert(arc_fn.name, (arc_fn, lambdas));
    }

    Ok(ArcCompileResult {
        arc_cache,
        pool,
        db,
    })
}
