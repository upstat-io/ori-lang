use std::path::PathBuf;

use oric::{CompilerDb, SourceFile};

#[test]
fn check_source_rejects_const_errors_before_codegen() {
    let db = CompilerDb::new();
    let path = PathBuf::from("/const-error-build.ori");
    let file = SourceFile::new(
        &db,
        path.clone(),
        "$unsupported = [1]\n@main () -> int = 0\n".to_string(),
    );

    let result = super::check_source(&db, file, &path.to_string_lossy());

    assert!(
        result.is_none(),
        "E2058 must stop compile_common before ARC/AIMS/LLVM lowering"
    );
}
