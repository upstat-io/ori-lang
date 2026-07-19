//! Test support utilities for integration tests.
//!
//! Always-compiled (NOT `#[cfg(test)]`) so integration test binaries
//! in `compiler/oric/tests/` can use these functions. Integration tests
//! compile the normal library build — `#[cfg(test)]` items are invisible.
//!
//! Provides the pre-AIMS ARC pipeline used by snapshot tests and a thin
//! frontend harness for the backend-neutral executable-program seam.

use ori_arc::ArcFunction;
use ori_ir::{Name, StringInterner};
use ori_repr::executable::ExecutableProgram;
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
    /// Delegates to [`ori_arc::collect_all_arc_functions`] — the single
    /// canonical implementation of arc cache flattening.
    pub fn all_arc_functions(&self) -> Vec<ArcFunction> {
        ori_arc::collect_all_arc_functions(&self.arc_cache)
    }
}

/// A typed failure in the source-to-executable test harness.
#[derive(Debug, thiserror::Error)]
pub enum ExecutableCompileError {
    /// Parsing failed before realization.
    #[error(
        "could not parse `{path}` ({count} syntax error(s)): {details}; fix the syntax at the reported location, then rerun the same command"
    )]
    Parse {
        /// Source path used for diagnostics.
        path: String,
        /// Number of parse errors.
        count: usize,
        /// Location-aware parser messages.
        details: String,
    },
    /// Type checking failed before realization.
    #[error(
        "could not type-check `{path}` ({count} type error(s)): {details}; apply each diagnostic's named correction at its reported source location, then rerun the same command"
    )]
    Type {
        /// Source path used for diagnostics.
        path: String,
        /// Number of type errors.
        count: usize,
        /// Location-aware type-checker messages.
        details: String,
    },
    /// Type checking did not publish the pool required by realization.
    #[error("type checking produced no type pool for {path}")]
    MissingTypePool {
        /// Source path used for diagnostics.
        path: String,
    },
    /// Backend-neutral realization rejected the checked module.
    #[error(transparent)]
    Realization(#[from] crate::realization::ProgramRealizationError),
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
        let mut context = crate::arc_lowering::ArcLoweringContext {
            canon,
            interner,
            pool: &pool,
            problems: &mut arc_problems,
        };
        let (arc_fn, lambdas) =
            crate::arc_lowering::lower_to_arc(func.name, sig, func.name, &mut context, None);
        arc_cache.insert(arc_fn.name, (arc_fn, lambdas));
    }

    if !arc_problems.is_empty() {
        return Err(format!(
            "ARC lowering problems in {source_path}: {} problem(s)",
            arc_problems.len()
        ));
    }

    Ok(ArcCompileResult {
        arc_cache,
        pool,
        db,
    })
}

/// Compile a checked local module into the backend-neutral executable artifact.
///
/// This harness owns only source/database orchestration. Realization, AIMS, and
/// validation are delegated to the same explicit seam executable backends use.
pub fn compile_to_executable(
    source_path: &str,
    source_text: &str,
    narrowing_policy: ori_repr::NarrowingPolicy,
) -> Result<ExecutableProgram, ExecutableCompileError> {
    let db = CompilerDb::new();
    let file = SourceFile::new(&db, source_path.into(), source_text.into());
    let parse_result = crate::query::parsed(&db, file);
    if parse_result.has_errors() {
        let details = parse_result
            .errors
            .iter()
            .map(|error| format!("{}: {}", error.span(), error.message()))
            .collect::<Vec<_>>()
            .join("; ");
        return Err(ExecutableCompileError::Parse {
            path: source_path.to_owned(),
            count: parse_result.errors.len(),
            details,
        });
    }

    let type_result = crate::query::typed(&db, file);
    let typed_pool = crate::query::typed_pool(&db, file);
    if type_result.has_errors() {
        let details = type_result
            .errors()
            .iter()
            .map(|error| {
                let message = typed_pool.as_deref().map_or_else(
                    || error.message(),
                    |pool| error.format_with(pool, db.interner()),
                );
                format!("{}: {message}", error.span())
            })
            .collect::<Vec<_>>()
            .join("; ");
        return Err(ExecutableCompileError::Type {
            path: source_path.to_owned(),
            count: type_result.errors().len(),
            details,
        });
    }

    let pool = typed_pool.map(|pool| (*pool).clone()).ok_or_else(|| {
        ExecutableCompileError::MissingTypePool {
            path: source_path.to_owned(),
        }
    })?;
    let shared_canon =
        crate::query::canonicalize_cached(&db, file, &parse_result, &type_result, &pool);
    crate::realization::realize_local_program(crate::realization::ProgramRealizationInput {
        parse: &parse_result,
        types: &type_result,
        canon: &shared_canon,
        pool,
        symbols: db.shared_interner(),
        narrowing_policy,
        verify_arc: std::env::var(crate::debug_flags::ORI_VERIFY_ARC)
            .is_ok_and(|value| value != "0"),
    })
    .map_err(ExecutableCompileError::from)
}

#[cfg(test)]
mod tests;
