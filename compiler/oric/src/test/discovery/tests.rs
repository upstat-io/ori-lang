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

#[test]
fn test_discover_in_all_multiple_files_preserves_order() {
    let dir = tempdir().unwrap();
    File::create(dir.path().join("b.ori")).unwrap();
    File::create(dir.path().join("a.ori")).unwrap();

    let paths = vec![dir.path().join("b.ori"), dir.path().join("a.ori")];
    let files = discover_tests_in_all(&paths);
    assert_eq!(files.len(), 2);
    assert!(files[0].path.ends_with("b.ori"));
    assert!(files[1].path.ends_with("a.ori"));
}

#[test]
fn test_discover_in_all_dedups_repeated_file() {
    let dir = tempdir().unwrap();
    File::create(dir.path().join("a.ori")).unwrap();

    let p = dir.path().join("a.ori");
    let files = discover_tests_in_all(&[p.clone(), p]);
    assert_eq!(files.len(), 1);
}

#[test]
fn test_discover_in_all_dedups_file_nested_under_dir_arg() {
    let dir = tempdir().unwrap();
    File::create(dir.path().join("a.ori")).unwrap();

    let paths = vec![dir.path().to_path_buf(), dir.path().join("a.ori")];
    let files = discover_tests_in_all(&paths);
    assert_eq!(files.len(), 1);
}

#[test]
fn test_discover_in_all_mixes_file_and_dir() {
    let dir = tempdir().unwrap();
    let sub = dir.path().join("sub");
    fs::create_dir(&sub).unwrap();
    File::create(dir.path().join("solo.ori")).unwrap();
    File::create(sub.join("nested.ori")).unwrap();

    let paths = vec![dir.path().join("solo.ori"), sub];
    let files = discover_tests_in_all(&paths);
    assert_eq!(files.len(), 2);
}

#[test]
fn test_discover_in_all_empty_input_yields_empty() {
    let files = discover_tests_in_all(&[]);
    assert!(files.is_empty());
}
