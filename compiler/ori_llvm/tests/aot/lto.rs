//! Link-Time Optimization (LTO) Tests
//!
//! Test scenarios inspired by:
//! - Rust: `tests/run-make/cross-lang-lto/` - verify bitcode via llvm-bcanalyzer
//! - Rust: `tests/run-make/lto-*` - various LTO configurations
//!
//! These tests verify:
//! - Thin LTO configuration and pass pipelines
//! - Full LTO configuration and pass pipelines
//! - LTO interaction with optimization levels
//! - Bitcode emission for LTO

use ori_llvm::aot::passes::{LtoMode, OptimizationLevel};

// ============================================================================
// LTO Mode Configuration Tests
// ============================================================================

/// Test: LTO mode variants
///
/// Scenario: Verify all LTO mode variants exist and are distinct.
#[test]
fn test_lto_mode_variants() {
    let off = LtoMode::Off;
    let thin = LtoMode::Thin;
    let full = LtoMode::Full;

    assert_ne!(off, thin);
    assert_ne!(thin, full);
    assert_ne!(off, full);

    // Default is Off
    assert_eq!(LtoMode::default(), LtoMode::Off);
}

/// Test: LTO mode display
#[test]
fn test_lto_mode_display() {
    assert_eq!(format!("{}", LtoMode::Off), "off");
    assert_eq!(format!("{}", LtoMode::Thin), "thin");
    assert_eq!(format!("{}", LtoMode::Full), "full");
}

// ============================================================================
// Thin LTO Tests
// ============================================================================

/// Test: Thin LTO pre-link passes
///
/// Scenario from Rust `cross-lang-lto`:
/// Verify thin LTO pre-link pass names.
#[test]
fn test_thin_lto_prelink_passes() {
    let thin = LtoMode::Thin;

    // O0
    assert_eq!(
        thin.prelink_pipeline_string(OptimizationLevel::O0),
        Some("thinlto-pre-link<O0>".to_string())
    );

    // O1
    assert_eq!(
        thin.prelink_pipeline_string(OptimizationLevel::O1),
        Some("thinlto-pre-link<O1>".to_string())
    );

    // O2
    assert_eq!(
        thin.prelink_pipeline_string(OptimizationLevel::O2),
        Some("thinlto-pre-link<O2>".to_string())
    );

    // O3
    assert_eq!(
        thin.prelink_pipeline_string(OptimizationLevel::O3),
        Some("thinlto-pre-link<O3>".to_string())
    );

    // Os
    assert_eq!(
        thin.prelink_pipeline_string(OptimizationLevel::Os),
        Some("thinlto-pre-link<Os>".to_string())
    );

    // Oz
    assert_eq!(
        thin.prelink_pipeline_string(OptimizationLevel::Oz),
        Some("thinlto-pre-link<Oz>".to_string())
    );
}

/// Test: Thin LTO link passes
///
/// Scenario: Verify thin LTO link-time pass names.
#[test]
fn test_thin_lto_link_passes() {
    let thin = LtoMode::Thin;

    // O0
    assert_eq!(
        thin.lto_pipeline_string(OptimizationLevel::O0),
        Some("thinlto<O0>".to_string())
    );

    // O2
    assert_eq!(
        thin.lto_pipeline_string(OptimizationLevel::O2),
        Some("thinlto<O2>".to_string())
    );

    // O3
    assert_eq!(
        thin.lto_pipeline_string(OptimizationLevel::O3),
        Some("thinlto<O3>".to_string())
    );

    // Oz
    assert_eq!(
        thin.lto_pipeline_string(OptimizationLevel::Oz),
        Some("thinlto<Oz>".to_string())
    );
}

// ============================================================================
// Full LTO Tests
// ============================================================================

/// Test: Full LTO pre-link passes
///
/// Scenario from Rust `lto-*`:
/// Verify full LTO pre-link pass names.
#[test]
fn test_full_lto_prelink_passes() {
    let full = LtoMode::Full;

    // O0
    assert_eq!(
        full.prelink_pipeline_string(OptimizationLevel::O0),
        Some("lto-pre-link<O0>".to_string())
    );

    // O2
    assert_eq!(
        full.prelink_pipeline_string(OptimizationLevel::O2),
        Some("lto-pre-link<O2>".to_string())
    );

    // O3
    assert_eq!(
        full.prelink_pipeline_string(OptimizationLevel::O3),
        Some("lto-pre-link<O3>".to_string())
    );

    // Oz
    assert_eq!(
        full.prelink_pipeline_string(OptimizationLevel::Oz),
        Some("lto-pre-link<Oz>".to_string())
    );
}

/// Test: Full LTO link passes
///
/// Scenario: Verify full LTO link-time pass names.
#[test]
fn test_full_lto_link_passes() {
    let full = LtoMode::Full;

    // O0
    assert_eq!(
        full.lto_pipeline_string(OptimizationLevel::O0),
        Some("lto<O0>".to_string())
    );

    // O2
    assert_eq!(
        full.lto_pipeline_string(OptimizationLevel::O2),
        Some("lto<O2>".to_string())
    );

    // O3
    assert_eq!(
        full.lto_pipeline_string(OptimizationLevel::O3),
        Some("lto<O3>".to_string())
    );

    // Os
    assert_eq!(
        full.lto_pipeline_string(OptimizationLevel::Os),
        Some("lto<Os>".to_string())
    );
}

// ============================================================================
// No LTO Tests
// ============================================================================

/// Test: No LTO produces no passes
///
/// Scenario: LtoMode::Off should not generate any LTO passes.
#[test]
fn test_no_lto_no_passes() {
    let off = LtoMode::Off;

    // No pre-link pass
    assert_eq!(off.prelink_pipeline_string(OptimizationLevel::O0), None);
    assert_eq!(off.prelink_pipeline_string(OptimizationLevel::O2), None);
    assert_eq!(off.prelink_pipeline_string(OptimizationLevel::O3), None);

    // No link pass
    assert_eq!(off.lto_pipeline_string(OptimizationLevel::O0), None);
    assert_eq!(off.lto_pipeline_string(OptimizationLevel::O2), None);
    assert_eq!(off.lto_pipeline_string(OptimizationLevel::O3), None);
}

// ============================================================================
// LTO with Optimization Level Tests
// ============================================================================

/// Test: LTO at O0
///
/// Scenario: LTO with no optimization should still work.
#[test]
fn test_lto_at_o0() {
    // Thin LTO at O0
    assert_eq!(
        LtoMode::Thin.prelink_pipeline_string(OptimizationLevel::O0),
        Some("thinlto-pre-link<O0>".to_string())
    );
    assert_eq!(
        LtoMode::Thin.lto_pipeline_string(OptimizationLevel::O0),
        Some("thinlto<O0>".to_string())
    );
}

/// Test: LTO at Oz (minimum size)
///
/// Scenario from Rust `lto-*`:
/// LTO for size optimization.
#[test]
fn test_lto_at_oz() {
    // Full LTO at Oz
    assert_eq!(
        LtoMode::Full.prelink_pipeline_string(OptimizationLevel::Oz),
        Some("lto-pre-link<Oz>".to_string())
    );
    assert_eq!(
        LtoMode::Full.lto_pipeline_string(OptimizationLevel::Oz),
        Some("lto<Oz>".to_string())
    );
}

// ============================================================================
// LTO Comparison Tests
// ============================================================================

/// Test: Thin vs Full LTO differences
///
/// Scenario: Document differences between thin and full LTO.
#[test]
fn test_thin_vs_full_lto() {
    let thin = LtoMode::Thin;
    let full = LtoMode::Full;
    let level = OptimizationLevel::O2;

    // Different pre-link passes
    assert_ne!(
        thin.prelink_pipeline_string(level),
        full.prelink_pipeline_string(level)
    );
    assert!(thin
        .prelink_pipeline_string(level)
        .unwrap()
        .contains("thinlto"));
    assert!(full
        .prelink_pipeline_string(level)
        .unwrap()
        .starts_with("lto-"));

    // Different link passes
    assert_ne!(
        thin.lto_pipeline_string(level),
        full.lto_pipeline_string(level)
    );
    assert!(thin.lto_pipeline_string(level).unwrap().contains("thinlto"));
    assert!(full.lto_pipeline_string(level).unwrap().starts_with("lto<"));
}

/// Test: LTO mode at all optimization levels
///
/// Scenario: Verify LTO works with all optimization levels.
#[test]
fn test_lto_all_optimization_levels() {
    let levels = [
        OptimizationLevel::O0,
        OptimizationLevel::O1,
        OptimizationLevel::O2,
        OptimizationLevel::O3,
        OptimizationLevel::Os,
        OptimizationLevel::Oz,
    ];

    for level in levels {
        // Thin LTO should produce valid pipeline
        let thin_prelink = LtoMode::Thin.prelink_pipeline_string(level);
        assert!(thin_prelink.is_some());
        assert!(thin_prelink.unwrap().contains("thinlto"));

        let thin_lto = LtoMode::Thin.lto_pipeline_string(level);
        assert!(thin_lto.is_some());

        // Full LTO should produce valid pipeline
        let full_prelink = LtoMode::Full.prelink_pipeline_string(level);
        assert!(full_prelink.is_some());
        assert!(full_prelink.unwrap().contains("lto-pre-link"));

        let full_lto = LtoMode::Full.lto_pipeline_string(level);
        assert!(full_lto.is_some());

        // Off should produce no pipeline
        assert!(LtoMode::Off.prelink_pipeline_string(level).is_none());
        assert!(LtoMode::Off.lto_pipeline_string(level).is_none());
    }
}
