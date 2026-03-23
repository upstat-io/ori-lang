use crate::*;

// Registry-level integrity tests (Section 14.1)

#[test]
fn no_duplicate_methods() {
    use std::collections::BTreeSet;

    for type_def in BUILTIN_TYPES {
        let mut seen = BTreeSet::new();
        for method in type_def.methods {
            assert!(
                seen.insert(method.name),
                "Duplicate method `{}` on type `{}`",
                method.name,
                type_def.name,
            );
        }
    }
}

#[test]
fn no_empty_types() {
    for type_def in BUILTIN_TYPES {
        assert!(
            !type_def.methods.is_empty(),
            "TypeDef `{}` has zero methods -- every registered type must \
             have at least one method (minimally: clone, equals, to_str)",
            type_def.name,
        );
    }
}

#[test]
fn all_type_tags_present() {
    use std::collections::HashSet;

    let registered_tags: HashSet<TypeTag> = BUILTIN_TYPES.iter().map(|td| td.tag).collect();

    // TypeTag variants that intentionally have no TypeDef:
    // - Unit, Never: no methods, no operators
    // - Function: no methods (memory classification only)
    // - DoubleEndedIterator: aliased to Iterator via base_type()
    let excluded = [
        TypeTag::Unit,
        TypeTag::Never,
        TypeTag::Function,
        TypeTag::DoubleEndedIterator,
    ];

    for tag in TypeTag::all() {
        if excluded.contains(tag) {
            continue;
        }
        assert!(
            registered_tags.contains(tag),
            "TypeTag::{tag:?} has no TypeDef in BUILTIN_TYPES. \
             Add a const TypeDef in ori_registry/src/defs/ and include \
             it in BUILTIN_TYPES.",
        );
    }
}

#[test]
fn methods_sorted_by_name() {
    for type_def in BUILTIN_TYPES {
        for window in type_def.methods.windows(2) {
            assert!(
                window[0].name <= window[1].name,
                "Methods not sorted in `{}`: `{}` > `{}`\n\
                 Methods must be alphabetically sorted within each TypeDef.",
                type_def.name,
                window[0].name,
                window[1].name,
            );
        }
    }
}

#[test]
fn all_receivers_documented() {
    for type_def in BUILTIN_TYPES {
        for method in type_def.methods {
            // Copy types should always borrow (borrow == copy for Copy types,
            // but the annotation documents the intent).
            if type_def.memory == MemoryStrategy::Copy {
                assert_eq!(
                    method.receiver,
                    Ownership::Borrow,
                    "Method `{}.{}` on a Copy type should use Ownership::Borrow \
                     (Copy types are trivially borrowed)",
                    type_def.name,
                    method.name,
                );
            }
            // Arc types: most methods borrow, but consuming methods (into)
            // may use Owned. The field access below proves the field exists.
            let _ = method.receiver;
        }
    }
}

#[test]
fn no_unsupported_eq() {
    // Equality is provided via EITHER OpStrategy::eq (primitives) OR
    // an `equals` method (Eq trait). Types with neither are intentionally
    // excluded:
    // - Error: compared via message/trace, not structural equality
    // - Channel: runtime handle, not a value type
    // - Iterator: stateful, equality is meaningless
    // - Range: generic, equality dispatched through type checker
    let excluded = [
        TypeTag::Error,
        TypeTag::Channel,
        TypeTag::Iterator,
        TypeTag::Range,
    ];

    for type_def in BUILTIN_TYPES {
        if excluded.contains(&type_def.tag) {
            continue;
        }
        let has_op_eq = type_def.operators.eq != OpStrategy::Unsupported;
        let has_equals_method = type_def.methods.iter().any(|m| m.name == "equals");
        assert!(
            has_op_eq || has_equals_method,
            "Type `{}` has neither OpStrategy::eq nor an `equals` method. \
             All Ori types must support equality via one mechanism.",
            type_def.name,
        );
    }
}

#[test]
fn operator_consistency() {
    for type_def in BUILTIN_TYPES {
        let ops = &type_def.operators;

        // If any comparison operator is supported, eq must be too
        let has_any_cmp = ops.lt != OpStrategy::Unsupported
            || ops.gt != OpStrategy::Unsupported
            || ops.lt_eq != OpStrategy::Unsupported
            || ops.gt_eq != OpStrategy::Unsupported;

        if has_any_cmp {
            assert!(
                ops.eq != OpStrategy::Unsupported,
                "Type `{}` supports comparison but not equality. \
                 If lt/gt/le/ge are supported, eq must be too.",
                type_def.name,
            );
            assert!(
                ops.neq != OpStrategy::Unsupported,
                "Type `{}` supports comparison but not not-equal. \
                 If lt/gt/le/ge are supported, neq must be too.",
                type_def.name,
            );
        }

        // If multiple comparison operators are supported, they should
        // use the same strategy (no mixing signed/unsigned/float)
        let cmp_ops = [ops.lt, ops.gt, ops.lt_eq, ops.gt_eq];
        let supported_cmp: Vec<_> = cmp_ops
            .iter()
            .filter(|s| **s != OpStrategy::Unsupported)
            .collect();
        if supported_cmp.len() > 1 {
            let first = supported_cmp[0];
            for s in &supported_cmp[1..] {
                assert_eq!(
                    *s, first,
                    "Type `{}` uses mixed comparison strategies: {:?} vs {:?}. \
                     All comparison operators should use the same strategy.",
                    type_def.name, first, s,
                );
            }
        }
    }
}

#[test]
fn type_tag_all_contains_every_variant() {
    // Hard-coded count serves as regression guard.
    // Update both the assertion AND ALL_TYPE_TAGS when adding variants.
    let expected_count = 23;
    assert_eq!(
        TypeTag::all().len(),
        expected_count,
        "TypeTag::all() has {} entries but expected {expected_count}. \
         A TypeTag variant was added/removed without updating ALL_TYPE_TAGS.",
        TypeTag::all().len(),
    );

    // Verify no duplicates
    let mut seen = std::collections::HashSet::new();
    for tag in TypeTag::all() {
        assert!(
            seen.insert(tag),
            "Duplicate TypeTag::{tag:?} in TypeTag::all()",
        );
    }
}

#[test]
fn self_type_returns_valid() {
    for type_def in BUILTIN_TYPES {
        for method in type_def.methods {
            if method.returns == ReturnTag::SelfType {
                // SelfType is NOT valid for to_* conversion methods
                // (except identity conversions like to_str on str).
                if method.name.starts_with("to_")
                    && method.name != "to_uppercase"
                    && method.name != "to_lowercase"
                    && method.name != "to_ascii_uppercase"
                    && method.name != "to_ascii_lowercase"
                {
                    let is_identity = type_def.tag == TypeTag::Str && method.name == "to_str";
                    assert!(
                        is_identity,
                        "Method `{}.{}` returns SelfType but is a conversion \
                         method (`to_*`). Conversion methods should return \
                         a concrete TypeTag, not SelfType.",
                        type_def.name, method.name,
                    );
                }
            }
        }
    }
}

// Purity tests (Section 02)

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
