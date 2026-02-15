use super::*;
use std::fs::File;
use tempfile::tempdir;

#[test]
fn test_discover_empty_dir() {
    let dir = tempdir().unwrap();
    let files = discover_tests(dir.path());
    assert!(files.is_empty());
}

#[test]
fn test_discover_si_files() {
    let dir = tempdir().unwrap();

    // Create some test files
    File::create(dir.path().join("test1.ori")).unwrap();
    File::create(dir.path().join("test2.ori")).unwrap();
    File::create(dir.path().join("not_a_test.txt")).unwrap();

    let files = discover_tests(dir.path());
    assert_eq!(files.len(), 2);
}

#[test]
fn test_discover_recursive() {
    let dir = tempdir().unwrap();

    // Create nested structure
    let sub = dir.path().join("subdir");
    fs::create_dir(&sub).unwrap();

    File::create(dir.path().join("root.ori")).unwrap();
    File::create(sub.join("nested.ori")).unwrap();

    let files = discover_tests(dir.path());
    assert_eq!(files.len(), 2);
}

#[test]
fn test_skip_hidden_and_target() {
    let dir = tempdir().unwrap();

    // Create directories that should be skipped
    let hidden = dir.path().join(".hidden");
    let target = dir.path().join("target");
    fs::create_dir(&hidden).unwrap();
    fs::create_dir(&target).unwrap();

    File::create(hidden.join("test.ori")).unwrap();
    File::create(target.join("test.ori")).unwrap();
    File::create(dir.path().join("real.ori")).unwrap();

    let files = discover_tests(dir.path());
    assert_eq!(files.len(), 1);
    assert!(files[0].path.ends_with("real.ori"));
}

#[test]
fn test_discover_single_file() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("test.ori");
    File::create(&path).unwrap();

    let files = discover_tests_in(&path);
    assert_eq!(files.len(), 1);
}
