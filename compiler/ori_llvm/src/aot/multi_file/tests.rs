use super::*;

#[test]
fn test_derive_module_name_simple() {
    let path = Path::new("/project/src/helper.ori");
    assert_eq!(derive_module_name(path, None), "helper");
}

#[test]
fn test_derive_module_name_nested() {
    let base = Path::new("/project/src");
    let path = Path::new("/project/src/http/client.ori");
    assert_eq!(derive_module_name(path, Some(base)), "http$client");
}

#[test]
fn test_derive_module_name_deeply_nested() {
    let base = Path::new("/project/src");
    let path = Path::new("/project/src/net/http/json/parser.ori");
    assert_eq!(derive_module_name(path, Some(base)), "net$http$json$parser");
}

#[test]
fn test_extract_quoted_path() {
    assert_eq!(extract_quoted_path("\"./helper\""), Some("./helper"));
    assert_eq!(
        extract_quoted_path("\"../utils\" { foo }"),
        Some("../utils")
    );
    assert_eq!(extract_quoted_path("no quotes"), None);
    assert_eq!(extract_quoted_path("\"unclosed"), None);
}

#[test]
fn test_graph_build_context_cycle_detection() {
    let mut ctx = GraphBuildContext::new();
    let path_a = PathBuf::from("/a.ori");
    let path_b = PathBuf::from("/b.ori");

    // Start loading A
    ctx.start_loading(path_a.clone()).unwrap();
    assert!(ctx.would_cycle(&path_a));
    assert!(!ctx.would_cycle(&path_b));

    // Starting A again should error
    let result = ctx.start_loading(path_a.clone());
    assert!(matches!(
        result,
        Err(MultiFileError::CyclicDependency { .. })
    ));

    // Start loading B (should work)
    ctx.start_loading(path_b.clone()).unwrap();
    assert!(ctx.would_cycle(&path_b));

    // Finish B
    ctx.finish_loading(path_b.clone());
    assert!(!ctx.would_cycle(&path_b));
    assert!(ctx.is_visited(&path_b));
}

#[test]
fn test_multi_file_error_display() {
    let err = MultiFileError::ImportError {
        message: "not found".to_string(),
        path: PathBuf::from("/test.ori"),
    };
    assert!(err.to_string().contains("import error"));
    assert!(err.to_string().contains("/test.ori"));

    let err = MultiFileError::CyclicDependency {
        cycle: vec![
            PathBuf::from("a.ori"),
            PathBuf::from("b.ori"),
            PathBuf::from("a.ori"),
        ],
    };
    assert!(err.to_string().contains("circular dependency"));
    assert!(err.to_string().contains("a.ori"));
    assert!(err.to_string().contains("b.ori"));
}

#[test]
fn test_multi_file_config() {
    let config = MultiFileConfig::default()
        .with_obj_dir(PathBuf::from("/custom/obj"))
        .with_verbose(true);

    assert_eq!(config.obj_dir, PathBuf::from("/custom/obj"));
    assert!(config.verbose);
}

#[test]
fn test_extract_imports_basic() {
    let content = r#"
use "./helper" { add }
use "./utils" as util

@main () -> void = print(msg: "hello");
"#;

    // Mock resolver that just appends .ori
    let resolver = |current: &Path, import: &str| {
        let dir = current.parent().unwrap_or(Path::new("."));
        let path = dir.join(import);
        let with_ext = if path.extension().is_none() {
            path.with_extension("ori")
        } else {
            path
        };
        Ok(with_ext)
    };

    let current = Path::new("/project/main.ori");
    let imports = extract_imports(content, current, &resolver).unwrap();

    assert_eq!(imports.len(), 2);
}

#[test]
fn test_extract_imports_skips_module_imports() {
    let content = r#"
use std.math { sqrt }
use "./local" { foo }
"#;

    let resolver = |current: &Path, import: &str| {
        Ok(current.parent().unwrap().join(import).with_extension("ori"))
    };

    let current = Path::new("/project/main.ori");
    let imports = extract_imports(content, current, &resolver).unwrap();

    // Should only include the relative import, not std.math
    assert_eq!(imports.len(), 1);
}

#[test]
fn test_resolve_relative_import_file_module() {
    // Create a temp directory with a file module
    let temp_dir = std::env::temp_dir().join("ori_test_resolve_file");
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let helper_file = temp_dir.join("helper.ori");
    std::fs::write(&helper_file, "pub @foo () -> int = 42").unwrap();

    let current = temp_dir.join("main.ori");
    let result = resolve_relative_import(&current, "./helper");

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), helper_file);

    // Cleanup
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_resolve_relative_import_directory_module() {
    // Create a temp directory with a directory module
    let temp_dir = std::env::temp_dir().join("ori_test_resolve_dir");
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(temp_dir.join("http")).unwrap();

    let mod_file = temp_dir.join("http/mod.ori");
    std::fs::write(&mod_file, "pub @get () -> str = \"ok\"").unwrap();

    let current = temp_dir.join("main.ori");
    let result = resolve_relative_import(&current, "./http");

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), mod_file);

    // Cleanup
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_resolve_relative_import_prefers_file_over_directory() {
    // When both file and directory module exist, file takes precedence
    let temp_dir = std::env::temp_dir().join("ori_test_resolve_both");
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(temp_dir.join("utils")).unwrap();

    let file_module = temp_dir.join("utils.ori");
    let dir_module = temp_dir.join("utils/mod.ori");
    std::fs::write(&file_module, "// file module").unwrap();
    std::fs::write(&dir_module, "// dir module").unwrap();

    let current = temp_dir.join("main.ori");
    let result = resolve_relative_import(&current, "./utils");

    assert!(result.is_ok());
    // File module should be preferred
    assert_eq!(result.unwrap(), file_module);

    // Cleanup
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_resolve_relative_import_not_found() {
    let temp_dir = std::env::temp_dir().join("ori_test_resolve_notfound");
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let current = temp_dir.join("main.ori");
    let result = resolve_relative_import(&current, "./nonexistent");

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.contains("cannot find import"));
    assert!(err.contains("nonexistent.ori"));
    assert!(err.contains("nonexistent/mod.ori"));

    // Cleanup
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_resolve_relative_import_with_extension() {
    // When extension is provided, don't try directory module
    let temp_dir = std::env::temp_dir().join("ori_test_resolve_ext");
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let helper_file = temp_dir.join("helper.ori");
    std::fs::write(&helper_file, "pub @foo () -> int = 42").unwrap();

    let current = temp_dir.join("main.ori");
    let result = resolve_relative_import(&current, "./helper.ori");

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), helper_file);

    // Cleanup
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_resolve_relative_import_parent_path() {
    // Test ../path resolution
    let temp_dir = std::env::temp_dir().join("ori_test_resolve_parent");
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(temp_dir.join("src")).unwrap();

    let utils_file = temp_dir.join("utils.ori");
    std::fs::write(&utils_file, "pub @helper () -> int = 1").unwrap();

    let current = temp_dir.join("src/main.ori");
    let result = resolve_relative_import(&current, "../utils");

    assert!(result.is_ok());
    // Compare canonicalized paths since ../utils resolves to parent dir
    let resolved = result.unwrap().canonicalize().unwrap();
    let expected = utils_file.canonicalize().unwrap();
    assert_eq!(resolved, expected);

    // Cleanup
    let _ = std::fs::remove_dir_all(&temp_dir);
}
