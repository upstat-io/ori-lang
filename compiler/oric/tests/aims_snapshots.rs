//! AIMS pass-level snapshot tests.
//!
//! Lives in `oric` (not `ori_arc`) because compiling `.ori` files
//! requires the full compiler driver (`CompilerDb`, `SourceFile`, etc.).
//! The checkpoint observer in `ori_arc` captures ARC IR at pipeline
//! boundaries; this test binary compiles, captures, and compares.

use ori_test_harness::bless;
use ori_test_harness::runner::run_test_directory;
use std::path::Path;

mod aims_snapshot_strategy;

/// Verifies that per-pass ARC IR snapshots match baselines across all
/// 5 priority AIMS passes (`realize_rc_reuse`, `merge_blocks`,
/// `realize_annotations`, `normalize_function`, `tail_calls`).
#[test]
fn aims_snapshots_across_all_passes_match_baselines() {
    let test_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/aims-snapshots");
    if !test_dir.exists() {
        return;
    }
    let strategy = aims_snapshot_strategy::AimsSnapshotStrategy::new();
    let bless_mode = bless::is_bless_enabled();
    let summary = run_test_directory(&test_dir, &strategy, bless_mode);
    assert!(
        summary.is_success(),
        "AIMS snapshot failures ({} failed):\n{}",
        summary.failed,
        summary.failures.join("\n")
    );
}
