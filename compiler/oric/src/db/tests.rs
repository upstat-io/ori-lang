use super::*;

#[test]
fn test_db_creation() {
    let _db = CompilerDb::new();
    // If this compiles and runs, Salsa is working
}

#[test]
fn test_db_clone() {
    let db1 = CompilerDb::new();
    let _db2 = db1.clone();
    // Clone must work for Salsa
}

#[test]
fn test_db_default() {
    let _db = CompilerDb::default();
}

#[test]
fn test_is_stdlib_path() {
    // Native-separator paths (these work on the current platform)
    assert!(is_stdlib_path(Path::new(
        "/home/user/ori/library/std/prelude.ori"
    )));
    assert!(is_stdlib_path(Path::new(
        "/home/user/ori/library/std/io.ori"
    )));

    // User files are NOT stdlib
    assert!(!is_stdlib_path(Path::new("/home/user/project/main.ori")));
    assert!(!is_stdlib_path(Path::new("/home/user/project/src/lib.ori")));
}

#[test]
#[cfg(target_os = "windows")]
fn test_is_stdlib_path_windows() {
    assert!(is_stdlib_path(Path::new(
        "C:\\Users\\user\\ori\\library\\std\\prelude.ori"
    )));
}
