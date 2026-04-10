use super::*;
use crate::verify::rc_histogram::{BlockHistogram, FunctionHistogram};

#[test]
fn test_empty_histogram_produces_version_one_empty_functions() {
    let report = RcStatsReport::from_histograms(&[], false);
    assert_eq!(report.schema_version, 1);
    assert!(!report.optimized);
    assert!(report.functions.is_empty());
}

#[test]
fn test_histogram_to_report_computes_function_totals() {
    let histograms = vec![FunctionHistogram {
        name: "test_fn".to_string(),
        blocks: vec![
            BlockHistogram {
                label: "entry".to_string(),
                // [alloc, inc, dec, free, cow]
                counts: [2, 0, 0, 0, 0],
            },
            BlockHistogram {
                label: "exit".to_string(),
                counts: [0, 1, 3, 0, 1],
            },
        ],
    }];

    let report = RcStatsReport::from_histograms(&histograms, false);

    assert_eq!(report.functions.len(), 1);
    let func = &report.functions[0];
    assert_eq!(func.name, "test_fn");
    assert_eq!(func.blocks.len(), 2);

    // Totals = sum across blocks.
    assert_eq!(
        func.totals,
        OpCounts {
            alloc: 2,
            inc: 1,
            dec: 3,
            free: 0,
            cow: 1,
        }
    );
}

#[test]
fn test_report_serializes_to_valid_json() {
    let report = RcStatsReport::from_histograms(&[], true);
    let json = serde_json::to_string(&report).unwrap();
    assert!(json.contains("\"schema_version\":1"));
    assert!(json.contains("\"optimized\":true"));
}

#[test]
fn test_function_name_with_special_chars_serializes_correctly() {
    let histograms = vec![FunctionHistogram {
        name: "fn_with_\"quotes\"_and_\\backslash".to_string(),
        blocks: vec![],
    }];

    let report = RcStatsReport::from_histograms(&histograms, false);
    let json = serde_json::to_string(&report).unwrap();

    // serde_json properly escapes special characters.
    assert!(json.contains(r#"fn_with_\"quotes\"_and_\\backslash"#));

    // Round-trip: parse back to verify structural validity.
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(
        parsed["functions"][0]["name"],
        "fn_with_\"quotes\"_and_\\backslash"
    );
}

#[test]
fn test_optimized_flag_propagates_to_report() {
    let report = RcStatsReport::from_histograms(&[], true);
    assert!(report.optimized);

    let report = RcStatsReport::from_histograms(&[], false);
    assert!(!report.optimized);
}
