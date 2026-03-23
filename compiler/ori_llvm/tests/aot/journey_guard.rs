//! Journey Score Regression Guard
//!
//! Each code journey compiles and runs through the AOT pipeline with
//! `ORI_CHECK_LEAKS=1`. Tests verify both the computed exit code (semantic
//! correctness) and zero leaks (RC correctness).
//!
//! These are NOT error codes — each journey's `@main` returns a computed
//! value that serves as a regression guard for that feature area.

use std::path::PathBuf;

use crate::util::{ori_binary, stdlib_path};

// Expected exit codes for each journey (computed values, not error codes)
const J01_ARITHMETIC: i32 = 33;
const J02_BRANCHING: i32 = 17;
const J03_RECURSION: i32 = 61;
const J04_STRUCTS: i32 = 57;
const J05_CLOSURES: i32 = 27;
const J06_PATTERN_MATCHING: i32 = 41;
const J07_LOOPS: i32 = 30;
const J08_GENERICS: i32 = 57;
const J09_STRINGS: i32 = 13;
const J10_LISTS: i32 = 33;
const J11_DERIVED_TRAITS: i32 = 33;
const J12_OPTIONS: i32 = 33;
const J13_ITERATORS: i32 = 55;
const J14_FAT_STRING_SHARING: i32 = 65;
const J15_FAT_NESTED_COLLECTIONS: i32 = 18;
const J16_FAT_OWNERSHIP_TRANSFER: i32 = 42;
const J17_FAT_CLOSURE_CAPTURE: i32 = 10;
const J18_STRING_BUILDER: i32 = 67;
const J19_RC_LIFECYCLE: i32 = 51;
const J20_COW_PATTERNS: i32 = 105;

/// Get the path to a journey `.ori` file. Panics if the file doesn't exist.
fn journey_path(name: &str) -> PathBuf {
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .find(|p| p.join("Cargo.toml").exists() && p.join("compiler").exists())
        .map_or_else(|| PathBuf::from("/workspace"), std::path::Path::to_path_buf);

    let path = workspace_root
        .join("plans/code-journeys")
        .join(format!("{name}.ori"));

    assert!(
        path.exists(),
        "Journey file missing: {} — journey guard tests require all journey \
         .ori files to be present. This is a hard failure, not a skip.",
        path.display()
    );
    path
}

/// Compile and run a journey file through AOT with leak detection.
///
/// Returns `(exit_code, stdout, stderr)`.
fn run_journey(name: &str) -> (i32, String, String) {
    let source_path = journey_path(name);
    let temp_dir = tempfile::TempDir::new().expect("Failed to create temp dir");
    let binary_path = temp_dir
        .path()
        .join(format!("journey{}", std::env::consts::EXE_SUFFIX));

    let compile_result = std::process::Command::new(ori_binary())
        .args([
            "build",
            source_path.to_str().unwrap(),
            "-o",
            binary_path.to_str().unwrap(),
        ])
        .env("ORI_STDLIB", stdlib_path())
        .output()
        .expect("Failed to execute ori build");

    if !compile_result.status.success() {
        let stderr = String::from_utf8_lossy(&compile_result.stderr).to_string();
        return (-1, String::new(), stderr);
    }

    let run_result = std::process::Command::new(&binary_path)
        .env("ORI_CHECK_LEAKS", "1")
        .output()
        .expect("Failed to execute journey binary");

    let exit_code = run_result.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&run_result.stdout).to_string();
    let stderr = String::from_utf8_lossy(&run_result.stderr).to_string();
    (exit_code, stdout, stderr)
}

/// Assert a journey compiles, runs without leaks, and returns the expected value.
fn assert_journey(name: &str, expected_exit: i32) {
    let (exit_code, _stdout, stderr) = run_journey(name);

    match exit_code {
        -1 => panic!("Journey {name} compilation failed:\n{stderr}"),
        2 => panic!("Journey {name} leaked memory:\n{stderr}"),
        code if code == expected_exit => {} // success
        code => panic!("Journey {name} returned {code}, expected {expected_exit}:\n{stderr}"),
    }
}

#[test]
fn test_journey_01_arithmetic() {
    assert_journey("01-arithmetic", J01_ARITHMETIC);
}

#[test]
fn test_journey_02_branching() {
    assert_journey("02-branching", J02_BRANCHING);
}

#[test]
fn test_journey_03_recursion() {
    assert_journey("03-recursion", J03_RECURSION);
}

#[test]
fn test_journey_04_structs() {
    assert_journey("04-structs", J04_STRUCTS);
}

#[test]
fn test_journey_05_closures() {
    assert_journey("05-closures", J05_CLOSURES);
}

#[test]
fn test_journey_06_pattern_matching() {
    assert_journey("06-pattern-matching", J06_PATTERN_MATCHING);
}

#[test]
fn test_journey_07_loops() {
    assert_journey("07-loops", J07_LOOPS);
}

#[test]
fn test_journey_08_generics() {
    assert_journey("08-generics", J08_GENERICS);
}

#[test]
fn test_journey_09_strings() {
    assert_journey("09-strings", J09_STRINGS);
}

#[test]
fn test_journey_10_lists() {
    assert_journey("10-lists", J10_LISTS);
}

#[test]
fn test_journey_11_derived_traits() {
    assert_journey("11-derived-traits", J11_DERIVED_TRAITS);
}

#[test]
fn test_journey_12_options() {
    assert_journey("12-options", J12_OPTIONS);
}

#[test]
fn test_journey_13_iterators() {
    assert_journey("13-iterators", J13_ITERATORS);
}

#[test]
fn test_journey_14_fat_string_sharing() {
    assert_journey("14-fat-string-sharing", J14_FAT_STRING_SHARING);
}

#[test]
fn test_journey_15_fat_nested_collections() {
    assert_journey("15-fat-nested-collections", J15_FAT_NESTED_COLLECTIONS);
}

#[test]
fn test_journey_16_fat_ownership_transfer() {
    assert_journey("16-fat-ownership-transfer", J16_FAT_OWNERSHIP_TRANSFER);
}

#[test]
fn test_journey_17_fat_closure_capture() {
    assert_journey("17-fat-closure-capture", J17_FAT_CLOSURE_CAPTURE);
}

#[test]
fn test_journey_18_string_builder() {
    assert_journey("18-string-builder", J18_STRING_BUILDER);
}

#[test]
fn test_journey_19_rc_lifecycle() {
    assert_journey("19-rc-lifecycle", J19_RC_LIFECYCLE);
}

#[test]
fn test_journey_20_cow_patterns() {
    assert_journey("20-cow-patterns", J20_COW_PATTERNS);
}
