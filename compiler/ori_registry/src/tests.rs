use crate::*;

#[test]
fn purity_cargo_toml_has_no_dependencies() {
    // Normalize CRLF → LF so this test works on Windows (git autocrlf).
    let cargo_toml = include_str!("../Cargo.toml").replace("\r\n", "\n");

    // Find the [dependencies] section header on its own line
    // (not embedded in a comment like "zero [dependencies].")
    let Some(deps_pos) = cargo_toml.find("\n[dependencies]\n") else {
        panic!("Cargo.toml must have a [dependencies] section (even if empty)");
    };
    let deps_start = deps_pos + 1; // skip the leading newline to point at '['

    // Find the next section header after [dependencies]
    let after_deps = &cargo_toml[deps_start + "[dependencies]".len()..];
    let next_section = after_deps.find("\n[").map_or(after_deps.len(), |i| i);
    let deps_body = after_deps[..next_section].trim();

    // The body between [dependencies] and the next section must be empty
    // (comments are allowed — they're not dependencies)
    let non_comment_lines: Vec<&str> = deps_body
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            !trimmed.is_empty() && !trimmed.starts_with('#')
        })
        .collect();

    assert!(
        non_comment_lines.is_empty(),
        "ori_registry MUST have zero [dependencies]. Found:\n{}",
        non_comment_lines.join("\n")
    );
}

#[test]
fn purity_core_enums_are_copy() {
    // Compile-time checks — if they compile, they pass.
    fn assert_copy<T: Copy>() {}
    fn assert_clone<T: Clone>() {}

    assert_copy::<TypeTag>();
    assert_copy::<MemoryStrategy>();
    assert_copy::<Ownership>();
    assert_copy::<OpStrategy>();
    assert_copy::<MethodDef>();
    assert_copy::<ParamDef>();
    assert_copy::<OpDefs>();

    // TypeDef is intentionally Clone-only (too large for implicit Copy).
    assert_clone::<TypeDef>();
}

#[test]
fn purity_type_defs_are_const() {
    // The `const _:` lines prove const-constructibility at compile time.
    use crate::defs::*;

    const _: TypeTag = INT.tag;
    const _: TypeTag = FLOAT.tag;
    const _: TypeTag = STR.tag;
    const _: TypeTag = BOOL.tag;
    const _: TypeTag = BYTE.tag;
    const _: TypeTag = CHAR.tag;

    assert_eq!(INT.tag, TypeTag::Int);
    assert_eq!(FLOAT.tag, TypeTag::Float);
    assert_eq!(STR.tag, TypeTag::Str);
    assert_eq!(BOOL.tag, TypeTag::Bool);
    assert_eq!(BYTE.tag, TypeTag::Byte);
    assert_eq!(CHAR.tag, TypeTag::Char);
}

#[test]
fn purity_no_unsafe_code() {
    fn scan_dir(dir: &std::path::Path, results: &mut Vec<String>) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    scan_dir(&path, results);
                } else if path.extension().is_some_and(|e| e == "rs") {
                    // Skip test files — they may reference "unsafe" in assertions
                    if path.file_name().is_some_and(|n| n == "tests.rs") {
                        continue;
                    }
                    if let Ok(contents) = std::fs::read_to_string(&path) {
                        for (i, line) in contents.lines().enumerate() {
                            let trimmed = line.trim();
                            if trimmed.contains("unsafe ")
                                && !trimmed.starts_with("//")
                                && !trimmed.starts_with("///")
                                && !trimmed.starts_with("#[deny(unsafe")
                                && !trimmed.starts_with("#[forbid(unsafe")
                                && !trimmed.contains("unsafe_code")
                            {
                                results.push(format!("{}:{}: {}", path.display(), i + 1, trimmed));
                            }
                        }
                    }
                }
            }
        }
    }

    // Enforced by workspace lints, but verified explicitly by scanning source.
    let src_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/src");
    let mut found_unsafe = Vec::new();
    scan_dir(std::path::Path::new(src_dir), &mut found_unsafe);
    assert!(
        found_unsafe.is_empty(),
        "ori_registry MUST NOT contain unsafe code. Found:\n{}",
        found_unsafe.join("\n")
    );
}

#[test]
fn purity_no_mutable_api() {
    fn scan_dir(dir: &std::path::Path, results: &mut Vec<String>) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    scan_dir(&path, results);
                } else if path.extension().is_some_and(|e| e == "rs") {
                    // Skip test files
                    if path.file_name().is_some_and(|n| n == "tests.rs") {
                        continue;
                    }
                    if let Ok(contents) = std::fs::read_to_string(&path) {
                        for (i, line) in contents.lines().enumerate() {
                            let trimmed = line.trim();
                            if trimmed.starts_with("pub ")
                                && trimmed.contains("fn ")
                                && trimmed.contains("&mut")
                            {
                                results.push(format!("{}:{}: {}", path.display(), i + 1, trimmed));
                            }
                        }
                    }
                }
            }
        }
    }

    // A pure-data crate should never need &mut in its public API.
    let src_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/src");
    let mut violations = Vec::new();
    scan_dir(std::path::Path::new(src_dir), &mut violations);
    assert!(
        violations.is_empty(),
        "ori_registry public API MUST NOT have &mut parameters (pure data). Found:\n{}",
        violations.join("\n")
    );
}

#[test]
fn purity_no_heap_allocation_types() {
    fn scan_dir(dir: &std::path::Path, heap_types: &[&str], results: &mut Vec<String>) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    scan_dir(&path, heap_types, results);
                } else if path.extension().is_some_and(|e| e == "rs") {
                    if path.file_name().is_some_and(|n| n == "tests.rs") {
                        continue;
                    }
                    if let Ok(contents) = std::fs::read_to_string(&path) {
                        for (i, line) in contents.lines().enumerate() {
                            let trimmed = line.trim();
                            // Skip comments and doc comments
                            if trimmed.starts_with("//") {
                                continue;
                            }
                            for heap_type in heap_types {
                                if trimmed.contains(heap_type) {
                                    results.push(format!(
                                        "{}:{}: {} (contains `{}`)",
                                        path.display(),
                                        i + 1,
                                        trimmed,
                                        heap_type
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // TypeDef uses &'static [MethodDef] (slice references to const data),
    // not Vec<MethodDef> (heap allocation).
    let src_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/src");
    let heap_types = [
        "String", "Vec<", "Box<", "Arc<", "Rc<", "HashMap", "BTreeMap",
    ];
    let mut violations = Vec::new();
    scan_dir(std::path::Path::new(src_dir), &heap_types, &mut violations);
    assert!(
        violations.is_empty(),
        "ori_registry MUST NOT use heap-allocating types (use &'static slices). Found:\n{}",
        violations.join("\n")
    );
}
