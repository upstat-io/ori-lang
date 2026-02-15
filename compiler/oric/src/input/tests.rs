use super::*;
use crate::db::CompilerDb;
use salsa::Setter;

#[test]
fn test_source_file_creation() {
    let db = CompilerDb::new();

    let file = SourceFile::new(
        &db,
        PathBuf::from("/test/file.ori"),
        "let x = 42".to_string(),
    );

    assert_eq!(file.path(&db), &PathBuf::from("/test/file.ori"));
    assert_eq!(file.text(&db), "let x = 42");
}

#[test]
fn test_source_file_mutation() {
    let mut db = CompilerDb::new();

    let file = SourceFile::new(
        &db,
        PathBuf::from("/test/file.ori"),
        "let x = 42".to_string(),
    );

    assert_eq!(file.text(&db), "let x = 42");

    // Mutate the source using Salsa's Setter trait
    file.set_text(&mut db).to("let x = 100".to_string());

    assert_eq!(file.text(&db), "let x = 100");
}
