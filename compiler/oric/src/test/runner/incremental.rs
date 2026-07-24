//! Parent-side incremental skip planning for subprocess-isolated LLVM runs.
//!
//! The parent runner owns the incremental cache (worker subprocesses are
//! fresh per spawn and must never consult a cache of their own), so under
//! `--incremental` the parent runs the crash-safe front end (parse ->
//! typecheck -> canonicalize) per file, diffs the canonical body hashes
//! against its cache, and decides what the worker may skip:
//!
//! - Every runnable test unchanged: the file's results are synthesized
//!   parent-side as `SkippedUnchanged` — no worker spawn at all.
//! - Otherwise: the skip decisions are forwarded to the worker via the
//!   hidden `--__skip-unchanged=` flag.

use std::path::Path;
use std::time::Duration;

use super::super::change_detection::{compute_skippable_and_update, TestRunCache};
use super::super::result::{FileSummary, TestOutcome, TestResult};
use super::file_run::non_compile_fail_type_errors;
use super::{TestRunner, TestRunnerConfig};
use crate::db::{CompilerDb, Db};
use crate::input::SourceFile;
use crate::query::{parsed, typed, typed_pool};

/// Parent-side per-file skip decision.
pub(super) enum SkipPlan {
    /// Spawn the worker, forwarding the parent-computed skip decisions
    /// (empty when nothing is skippable).
    Spawn { skip_names: Vec<String> },
    /// Every runnable test is unchanged: the synthesized summary stands in
    /// for the worker run.
    Synthesized(FileSummary),
}

impl SkipPlan {
    /// A spawn with no skip decisions (front-end failures defer everything
    /// to the worker, which surfaces the errors itself).
    fn run_all() -> Self {
        SkipPlan::Spawn {
            skip_names: Vec::new(),
        }
    }
}

/// Compute the skip plan for one test file against the parent-owned cache.
///
/// Mirrors the in-process incremental semantics exactly: the fresh canonical
/// snapshot replaces the cached one, and a test is skippable only when its
/// own body and every target are unchanged. Any front-end failure (read,
/// parse, missing pool) defers to the worker without touching the cache —
/// the in-process path also returns before its cache update on those.
pub(super) fn plan_file_skips(
    path: &Path,
    interner: &crate::ir::SharedInterner,
    config: &TestRunnerConfig,
    cache: &parking_lot::Mutex<TestRunCache>,
) -> SkipPlan {
    let Ok(content) = std::fs::read_to_string(path) else {
        return SkipPlan::run_all();
    };

    // Same front end the worker will run, with the shared interner so the
    // skip names rendered below match the worker's announced test names.
    let db = CompilerDb::with_interner(interner.clone());
    let file = SourceFile::new(&db, path.to_path_buf(), content);
    let parse_result = parsed(&db, file);
    if parse_result.has_errors() || parse_result.module.tests.is_empty() {
        return SkipPlan::run_all();
    }

    let type_result = typed(&db, file);
    let Some(pool) = typed_pool(&db, file) else {
        return SkipPlan::run_all();
    };
    let shared_canon =
        crate::query::canonicalize_cached(&db, file, &parse_result, &type_result, &pool);

    let skippable = compute_skippable_and_update(cache, path, &shared_canon, &parse_result.module);
    if skippable.is_empty() {
        return SkipPlan::run_all();
    }

    let interner = db.interner();
    let skip_names: Vec<String> = skippable
        .iter()
        .map(|&name| interner.lookup(name).to_string())
        .collect();

    // Full-skip synthesis: the worker spawn is skipped only when it would
    // produce exactly one `SkippedUnchanged` result per filter-passing
    // regular test and nothing else.
    let (compile_fail_tests, regular_tests): (Vec<_>, Vec<_>) = parse_result
        .module
        .tests
        .iter()
        .partition(|t| t.is_compile_fail());
    let any_compile_fail_runs = compile_fail_tests
        .iter()
        .copied()
        .any(|test| TestRunner::test_passes_filter(test, config, interner));
    let blocked_by_type_errors =
        !non_compile_fail_type_errors(&type_result, &compile_fail_tests).is_empty();
    let filtered_regular: Vec<_> = regular_tests
        .iter()
        .copied()
        .filter(|&test| TestRunner::test_passes_filter(test, config, interner))
        .collect();
    let all_regular_skippable = !filtered_regular.is_empty()
        && filtered_regular
            .iter()
            .all(|test| skippable.contains(&test.name));

    if any_compile_fail_runs || blocked_by_type_errors || !all_regular_skippable {
        return SkipPlan::Spawn { skip_names };
    }

    let mut summary = FileSummary::new(path.to_path_buf());
    for test in filtered_regular {
        summary.add_result(TestResult {
            name: test.name,
            targets: test.targets.clone(),
            outcome: TestOutcome::SkippedUnchanged,
            duration: Duration::ZERO,
        });
    }
    SkipPlan::Synthesized(summary)
}
