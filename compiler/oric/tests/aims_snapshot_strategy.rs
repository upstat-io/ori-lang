//! AIMS pass-level snapshot test strategy.
//!
//! Implements `TestStrategy` from `ori_test_harness` to compile `.ori`
//! files, run the AIMS pipeline with a checkpoint observer, and compare
//! per-pass ARC IR snapshots against baselines.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::Path;

use ori_arc::ir::format::format_function;
use ori_arc::{ArcClassifier, ArcFunction, CheckpointObserver};
use ori_test_harness::artifact::{self, ArtifactPaths};
use ori_test_harness::bless;
use ori_test_harness::directive::{Directive, DirectiveLine};
use ori_test_harness::revision::RevisionConfig;
use ori_test_harness::runner::{TestOutput, TestStrategy};
use rustc_hash::FxHashMap;

/// Strategy for AIMS pass-level snapshot tests.
#[derive(Default)]
pub struct AimsSnapshotStrategy;

impl AimsSnapshotStrategy {
    pub fn new() -> Self {
        Self
    }
}

/// Error type for AIMS snapshot tests.
#[derive(Debug)]
pub struct SnapshotError(String);

impl fmt::Display for SnapshotError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<String> for SnapshotError {
    fn from(s: String) -> Self {
        Self(s)
    }
}

/// Extract `// @test-arc-pass: <pass_name>` directives from parsed directives.
fn extract_pass_names(directives: &[&DirectiveLine]) -> Vec<String> {
    directives
        .iter()
        .filter_map(|d| {
            if let Directive::Custom { key, value } = &d.directive {
                if key == "test-arc-pass" {
                    return Some(value.trim().to_owned());
                }
            }
            None
        })
        .collect()
}

impl TestStrategy for AimsSnapshotStrategy {
    type Error = SnapshotError;

    fn execute(
        &self,
        test_path: &Path,
        _revision: &RevisionConfig,
        directives: &[&DirectiveLine],
    ) -> Result<TestOutput, Self::Error> {
        // Parse directives for pass names.
        let pass_names = extract_pass_names(directives);
        if pass_names.is_empty() {
            return Err(SnapshotError(format!(
                "{}: no // @test-arc-pass directives found (orphan test)",
                test_path.display()
            )));
        }

        // Read source and compile to pre-AIMS ARC IR.
        let source = fs::read_to_string(test_path)
            .map_err(|e| SnapshotError(format!("read {}: {e}", test_path.display())))?;
        let source_path = test_path.to_string_lossy();
        let compile_result = oric::test_support::compile_to_arc(&source_path, &source)?;

        let pool = &compile_result.pool;
        let interner = compile_result.interner();
        let classifier = ArcClassifier::new(pool);

        // Compute AIMS interprocedural contracts and apply param ownership.
        // This matches the production pipeline (codegen_pipeline.rs:361-366):
        //   1. Flatten all functions from the cache
        //   2. Run analyze_program() → MemoryContracts
        //   3. Apply ownership annotations to parameters (Owned/Borrowed)
        // The mutated functions in `all_funcs` now carry production-accurate
        // param ownership — we iterate THESE, not fresh clones from arc_cache.
        let mut all_funcs = compile_result.all_arc_functions();
        let aims_contracts = ori_arc::compute_aims_contracts(
            &mut all_funcs,
            &classifier,
            interner,
            &ori_arc::BuiltinOwnershipSets::new(interner),
        );

        // Sort functions by name for deterministic output order.
        all_funcs.sort_by_key(|f| interner.lookup(f.name).to_owned());

        let mut all_artifacts = Vec::new();

        for func in &mut all_funcs {
            let func_name_str = interner.lookup(func.name).to_owned();

            // Capture lowered.arc baseline BEFORE AIMS pipeline.
            // This captures the function WITH contract-applied ownership,
            // matching the state production codegen sees before per-function
            // pipeline execution (define_phase.rs:315-328).
            let lowered_content = format_function(func, pool, interner);
            let lowered_suffix = format!("{func_name_str}.lowered.arc");
            write_artifact(
                test_path,
                &lowered_suffix,
                &lowered_content,
                &mut all_artifacts,
            )?;

            // Capture per-pass snapshots via observer.
            // Scoped block ensures the observer (which borrows `snapshots`)
            // is dropped before we read `snapshots.into_inner()`.
            let snapshots: RefCell<BTreeMap<String, String>> = RefCell::new(BTreeMap::new());
            {
                let pass_names_ref = &pass_names;
                let observer: Box<CheckpointObserver<'_>> =
                    Box::new(|f: &ArcFunction, phase: &str| {
                        if pass_names_ref.iter().any(|p| p == phase) {
                            let content = format_function(f, pool, interner);
                            snapshots.borrow_mut().insert(phase.to_owned(), content);
                        }
                    });

                // Run AIMS pipeline with observer and real contracts.
                let uniqueness = FxHashMap::default();
                let sigs = FxHashMap::default();
                let _ = ori_arc::run_arc_pipeline_with_observer(
                    func,
                    &classifier,
                    &sigs,
                    pool,
                    interner,
                    &uniqueness,
                    &aims_contracts,
                    false, // verify_arc
                    &*observer,
                );
            } // observer dropped here — snapshots no longer borrowed

            // Write per-pass snapshots and assert all requested passes were captured.
            let captured = snapshots.into_inner();
            for pass_name in &pass_names {
                let content = captured.get(pass_name).ok_or_else(|| {
                    SnapshotError(format!(
                        "{}: pass '{}' was requested but never captured for function '{}'. \
                         The AIMS pipeline did not invoke the observer for this phase — \
                         either the pass name is wrong or the pipeline skipped it.",
                        test_path.display(),
                        pass_name,
                        func_name_str,
                    ))
                })?;
                let suffix = format!("{func_name_str}.{pass_name}.after.arc");
                write_artifact(test_path, &suffix, content, &mut all_artifacts)?;
            }
        }

        Ok(TestOutput {
            content: String::new(), // Multi-artifact — content in artifacts
            artifacts: all_artifacts,
        })
    }

    fn verify(
        &self,
        _test_path: &Path,
        _revision: &RevisionConfig,
        _directives: &[&DirectiveLine],
        output: &TestOutput,
    ) -> Result<(), Self::Error> {
        let bless_mode = bless::is_bless_enabled();
        let mut failures = Vec::new();

        for artifact in &output.artifacts {
            let actual_content = fs::read_to_string(&artifact.actual).map_err(|e| {
                SnapshotError(format!("read actual {}: {e}", artifact.actual.display()))
            })?;

            match bless::compare_or_bless(&artifact.expected, &actual_content, bless_mode) {
                Ok(
                    bless::CompareOutcome::Match
                    | bless::CompareOutcome::Blessed
                    | bless::CompareOutcome::BlessedEmpty,
                ) => {}
                Ok(bless::CompareOutcome::Mismatch { diff }) => {
                    failures.push(format!(
                        "snapshot mismatch: {}\n{diff}",
                        artifact.expected.display()
                    ));
                }
                Err(e) => {
                    failures.push(format!(
                        "IO error comparing {}: {e}",
                        artifact.expected.display()
                    ));
                }
            }
        }

        if failures.is_empty() {
            Ok(())
        } else {
            Err(SnapshotError(failures.join("\n\n")))
        }
    }

    fn baseline_suffix(&self) -> Option<&str> {
        Some("arc")
    }

    fn clean_stale_revisions(
        &self,
        _test_path: &Path,
        _active_revisions: &[&str],
    ) -> Result<(), Self::Error> {
        // Revision-specific cleanup for the current test_path only.
        // Global orphan cleanup is a separate bless-sweep task.
        Ok(())
    }
}

/// Write actual snapshot content and register the artifact paths.
fn write_artifact(
    test_path: &Path,
    suffix: &str,
    content: &str,
    artifacts: &mut Vec<ArtifactPaths>,
) -> Result<(), SnapshotError> {
    let expected = artifact::resolve_expected(test_path, suffix, None);
    let actual = artifact::resolve_actual(test_path, suffix, None);

    // Ensure parent directory exists for actual file.
    if let Some(parent) = actual.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| SnapshotError(format!("mkdir {}: {e}", parent.display())))?;
    }

    fs::write(&actual, content)
        .map_err(|e| SnapshotError(format!("write {}: {e}", actual.display())))?;

    artifacts.push(ArtifactPaths { expected, actual });
    Ok(())
}
