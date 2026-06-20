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
    // Use a module name that doesn't exist anywhere (not in project library/,
    // user-local ~/.local/share/ori/library/, or system locations).
    let nonexistent_root = interner.intern("zzz_nonexistent_pkg");
    let nonexistent_mod = interner.intern("does_not_exist");
    let path = ImportPath::Module(vec![nonexistent_root, nonexistent_mod]);
    let current = PathBuf::from("/nonexistent/project/src/main.ori");

    let result = resolve_import(&db, &path, &current, None);
    assert!(result.is_err());
    assert!(result.unwrap_err().message.contains("not found"));
}

#[test]
fn resolve_stdlib_module_not_found_is_friendly_and_actionable() {
    let db = CompilerDb::new();
    let interner = db.interner();
    // `std.testing` with no stdlib reachable (nonexistent current path, no
    // ORI_STDLIB override) must produce an actionable message naming the fix,
    // not the terse generic "Searched: ..." line.
    let std_root = interner.intern("std");
    let testing_mod = interner.intern("testing");
    let path = ImportPath::Module(vec![std_root, testing_mod]);
    let current = PathBuf::from("/nonexistent/project/src/main.ori");

    let result = resolve_import(&db, &path, &current, None);
    assert!(result.is_err());
    let msg = result.unwrap_err().message;
    // Names the missing module.
    assert!(msg.contains("std.testing"), "message: {msg}");
    // Names the actionable fix (ORI_STDLIB) and the standard-library framing.
    assert!(msg.contains("ORI_STDLIB"), "message: {msg}");
    assert!(msg.contains("standard library"), "message: {msg}");
    assert!(msg.contains("./library/std/"), "message: {msg}");
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

#[test]
fn ori_stdlib_library_roots_correct_layout_single_root() {
    // ORI_STDLIB pointing at the `library/` root yields just that root.
    let roots = ori_stdlib_library_roots("/w/library");
    assert_eq!(roots, vec![PathBuf::from("/w/library")]);
}

#[test]
fn ori_stdlib_library_roots_over_deep_adds_parent() {
    // ORI_STDLIB pointing directly at `library/std/` (one level too deep)
    // yields the as-is path AND its parent, so `<parent>/std/...` resolves.
    let roots = ori_stdlib_library_roots("/w/library/std");
    assert_eq!(
        roots,
        vec![PathBuf::from("/w/library/std"), PathBuf::from("/w/library")]
    );
}

#[test]
fn module_candidates_resolve_when_ori_stdlib_points_at_std_dir() {
    // Regression: ORI_STDLIB set one level too deep (`library/std`) must still
    // surface the correct `library/std/testing.ori` candidate for `std.testing`.
    let current = PathBuf::from("/proj/main.ori");
    let candidates =
        generate_module_candidates(&["std", "testing"], &current, Some("/w/library/std"));
    assert!(
        candidates.contains(&PathBuf::from("/w/library/std/testing.ori")),
        "over-deep ORI_STDLIB must still resolve the module; got {candidates:?}"
    );
}

#[test]
fn module_candidates_resolve_for_correct_ori_stdlib_layout() {
    let current = PathBuf::from("/proj/main.ori");
    let candidates = generate_module_candidates(&["std", "testing"], &current, Some("/w/library"));
    assert!(
        candidates.contains(&PathBuf::from("/w/library/std/testing.ori")),
        "correct ORI_STDLIB layout must resolve the module; got {candidates:?}"
    );
}
