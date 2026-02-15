use super::*;
use std::collections::HashSet;

use crate::db::CompilerDb;
use crate::ir::SharedInterner;

#[test]
fn generate_relative_candidates_file_module() {
    let interner = SharedInterner::default();
    let name = interner.intern("./math");
    let current = PathBuf::from("/project/src/main.ori");

    let candidates = generate_relative_candidates(name, &current, &interner);

    // Should try file first, then directory module
    assert_eq!(candidates.len(), 2);
    assert_eq!(candidates[0], PathBuf::from("/project/src/math.ori"));
    assert_eq!(candidates[1], PathBuf::from("/project/src/math/mod.ori"));
}

#[test]
fn generate_relative_candidates_parent_path() {
    let interner = SharedInterner::default();
    let name = interner.intern("../utils");
    let current = PathBuf::from("/project/src/main.ori");

    let candidates = generate_relative_candidates(name, &current, &interner);

    // Should try file first, then directory module
    assert_eq!(candidates.len(), 2);
    assert_eq!(candidates[0], PathBuf::from("/project/src/../utils.ori"));
    assert_eq!(
        candidates[1],
        PathBuf::from("/project/src/../utils/mod.ori")
    );
}

#[test]
fn generate_relative_candidates_with_extension() {
    let interner = SharedInterner::default();
    let name = interner.intern("./helper.ori");
    let current = PathBuf::from("/project/src/main.ori");

    let candidates = generate_relative_candidates(name, &current, &interner);

    // Should only try the exact path when extension is provided
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0], PathBuf::from("/project/src/helper.ori"));
}

#[test]
fn generate_relative_candidates_nested_directory() {
    let interner = SharedInterner::default();
    let name = interner.intern("./http/client");
    let current = PathBuf::from("/project/src/main.ori");

    let candidates = generate_relative_candidates(name, &current, &interner);

    // Should try file first, then directory module
    assert_eq!(candidates.len(), 2);
    assert_eq!(candidates[0], PathBuf::from("/project/src/http/client.ori"));
    assert_eq!(
        candidates[1],
        PathBuf::from("/project/src/http/client/mod.ori")
    );
}

#[test]
fn resolve_module_path_not_found() {
    let db = CompilerDb::new();
    let interner = db.interner();
    let std = interner.intern("std");
    let math = interner.intern("math");
    let path = ImportPath::Module(vec![std, math]);
    let current = PathBuf::from("/nonexistent/project/src/main.ori");

    let result = resolve_import(&db, &path, &current, None);
    assert!(result.is_err());
    assert!(result.unwrap_err().message.contains("not found"));
}

#[test]
fn import_error_display() {
    let err = ImportError::new(ImportErrorKind::ModuleNotFound, "test error");
    assert_eq!(format!("{err}"), "test error");
}

#[test]
fn is_test_module_valid() {
    // Valid test module: in _test/ with .test.ori extension
    let path = PathBuf::from("/project/src/_test/math.test.ori");
    assert!(is_test_module(&path));
}

#[test]
fn is_test_module_not_in_test_dir() {
    // Not in _test/ directory
    let path = PathBuf::from("/project/src/math.test.ori");
    assert!(!is_test_module(&path));
}

#[test]
fn is_test_module_wrong_extension() {
    // In _test/ but wrong extension
    let path = PathBuf::from("/project/src/_test/math.ori");
    assert!(!is_test_module(&path));
}

#[test]
fn is_test_module_nested() {
    // Nested _test/ directory
    let path = PathBuf::from("/project/src/utils/_test/helpers.test.ori");
    assert!(is_test_module(&path));
}

#[test]
fn is_parent_module_import_valid() {
    // Test module importing from parent directory
    let current = PathBuf::from("/project/src/_test/math.test.ori");
    let import = PathBuf::from("/project/src/math.ori");
    assert!(is_parent_module_import(&current, &import));
}

#[test]
fn is_parent_module_import_sibling() {
    // Importing from sibling, not parent
    let current = PathBuf::from("/project/src/_test/math.test.ori");
    let import = PathBuf::from("/project/src/_test/utils.ori");
    assert!(!is_parent_module_import(&current, &import));
}

#[test]
fn is_parent_module_import_not_test() {
    // Not in _test directory
    let current = PathBuf::from("/project/src/main.ori");
    let import = PathBuf::from("/project/src/math.ori");
    assert!(!is_parent_module_import(&current, &import));
}

/// Test-only context for loading modules with cycle detection.
///
/// Tracks which modules are currently being loaded to detect circular imports.
/// In production, Salsa's query dependency tracking handles cycle detection.
#[derive(Debug, Default)]
struct LoadingContext {
    loading_stack: Vec<PathBuf>,
    loading_set: HashSet<PathBuf>,
    loaded: HashSet<PathBuf>,
}

impl LoadingContext {
    fn new() -> Self {
        LoadingContext {
            loading_stack: Vec::new(),
            loading_set: HashSet::new(),
            loaded: HashSet::new(),
        }
    }

    fn would_cycle(&self, path: &Path) -> bool {
        self.loading_set.contains(path)
    }

    fn is_loaded(&self, path: &Path) -> bool {
        self.loaded.contains(path)
    }

    fn start_loading(&mut self, path: PathBuf) -> Result<(), ImportError> {
        if self.would_cycle(&path) {
            let cycle: Vec<String> = self
                .loading_stack
                .iter()
                .chain(std::iter::once(&path))
                .map(|p| p.display().to_string())
                .collect();
            return Err(ImportError::new(
                ImportErrorKind::CircularImport,
                format!("circular import detected: {}", cycle.join(" -> ")),
            ));
        }
        self.loading_set.insert(path.clone());
        self.loading_stack.push(path);
        Ok(())
    }

    fn finish_loading(&mut self, path: PathBuf) {
        if let Some(popped) = self.loading_stack.pop() {
            self.loading_set.remove(&popped);
        }
        self.loaded.insert(path);
    }
}

#[test]
fn loading_context_cycle_detection() {
    let mut ctx = LoadingContext::new();
    let path1 = PathBuf::from("/a.ori");
    let path2 = PathBuf::from("/b.ori");

    assert!(!ctx.would_cycle(&path1));
    ctx.start_loading(path1.clone()).unwrap();
    assert!(ctx.would_cycle(&path1));
    assert!(!ctx.would_cycle(&path2));

    ctx.start_loading(path2.clone()).unwrap();
    assert!(ctx.would_cycle(&path2));

    ctx.finish_loading(path2.clone());
    assert!(!ctx.would_cycle(&path2)); // Not in stack anymore
    assert!(ctx.is_loaded(&path2)); // But marked as loaded
}

#[test]
fn loading_context_cycle_error() {
    let mut ctx = LoadingContext::new();
    let path = PathBuf::from("/a.ori");

    ctx.start_loading(path.clone()).unwrap();
    let result = ctx.start_loading(path.clone());
    assert!(result.is_err());
    assert!(result.unwrap_err().message.contains("circular import"));
}
