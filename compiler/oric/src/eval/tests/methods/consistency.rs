//! Tests for consistency between `ori_registry` type definitions and consuming
//! compiler phases (type checker, evaluator).
//!
//! The registry (`ori_registry::BUILTIN_TYPES`) is the single source of truth
//! for what methods each type has. Both the type checker and evaluator read from
//! it. These tests validate cross-phase alignment that can't be enforced at
//! compile time.

use std::collections::BTreeSet;

use ori_eval::interpreter::resolvers::CollectionMethod;

// Registry method ordering

/// Registry methods must be sorted alphabetically within each `TypeDef`.
#[test]
fn registry_methods_sorted_per_type() {
    for td in ori_registry::BUILTIN_TYPES {
        for window in td.methods.windows(2) {
            assert!(
                window[0].name <= window[1].name,
                "Registry methods not sorted for {}: {:?} > {:?}",
                td.name,
                window[0].name,
                window[1].name
            );
        }
    }
}

// Iterator method consistency

/// Verify that every Iterator/DoubleEndedIterator method in the registry has
/// a corresponding `CollectionMethod` variant in the evaluator, and vice versa.
///
/// Replaces the old `iterator_registry_methods_match_eval_resolver` test that
/// compared against `ITERATOR_METHOD_NAMES` (now eliminated).
#[test]
fn iterator_methods_match_registry() {
    // Registry iterator methods (DEI methods are on the Iterator TypeDef
    // with dei_only flag; BUILTIN_TYPES has no separate DoubleEndedIterator entry)
    let registry_iter_methods: BTreeSet<&str> = ori_registry::BUILTIN_TYPES
        .iter()
        .filter(|td| td.tag == ori_registry::TypeTag::Iterator)
        .flat_map(|td| td.methods.iter().map(|m| m.name))
        .collect();

    // Eval iterator methods from CollectionMethod, excluding __-prefixed
    // protocol methods (__collect_set, __iter_next) that the registry
    // intentionally omits
    let eval_iter_methods: BTreeSet<&str> = CollectionMethod::all_iterator_variants()
        .iter()
        .map(|&(name, _)| name)
        .filter(|name| !name.starts_with("__"))
        .collect();

    let in_registry_not_eval: Vec<_> = registry_iter_methods
        .difference(&eval_iter_methods)
        .collect();
    let in_eval_not_registry: Vec<_> = eval_iter_methods
        .difference(&registry_iter_methods)
        .collect();

    assert!(
        in_registry_not_eval.is_empty(),
        "Registry has iterator methods not in eval CollectionMethod: {in_registry_not_eval:?}"
    );
    assert!(
        in_eval_not_registry.is_empty(),
        "Eval CollectionMethod has iterator methods not in registry: {in_eval_not_registry:?}"
    );
}

// Format spec variant registration consistency
//
// The `FormatType`, `Alignment`, and `Sign` enums appear as string arrays in
// 4 independent locations:
//   1. `ori_ir/src/format_spec.rs` — enum definition (source of truth)
//   2. `ori_types/src/check/registration/mod.rs` — type registration
//   3. `ori_eval/src/interpreter/mod.rs` — `register_format_variants()` globals
//   4. `ori_rt/src/format/mod.rs` — runtime enum + parse (guarded by ori_rt tests)
//
// ori_rt <-> ori_ir sync is guarded by `format_type_variant_count()` in ori_rt.
// These tests guard ori_types <-> ori_ir and ori_eval <-> ori_ir sync.

/// Source-of-truth variant names for `ori_ir::FormatType`.
///
/// Exhaustive match ensures compile failure if a variant is added to `ori_ir`.
fn ir_format_type_names() -> Vec<&'static str> {
    use ori_ir::format_spec::FormatType;
    [
        FormatType::Binary,
        FormatType::Octal,
        FormatType::Hex,
        FormatType::HexUpper,
        FormatType::Exp,
        FormatType::ExpUpper,
        FormatType::Fixed,
        FormatType::Percent,
    ]
    .iter()
    .map(|ft| match ft {
        FormatType::Binary => "Binary",
        FormatType::Octal => "Octal",
        FormatType::Hex => "Hex",
        FormatType::HexUpper => "HexUpper",
        FormatType::Exp => "Exp",
        FormatType::ExpUpper => "ExpUpper",
        FormatType::Fixed => "Fixed",
        FormatType::Percent => "Percent",
    })
    .collect()
}

/// Source-of-truth variant names for `ori_ir::Align`.
fn ir_align_names() -> Vec<&'static str> {
    use ori_ir::format_spec::Align;
    [Align::Left, Align::Center, Align::Right]
        .iter()
        .map(|a| match a {
            Align::Left => "Left",
            Align::Center => "Center",
            Align::Right => "Right",
        })
        .collect()
}

/// Source-of-truth variant names for `ori_ir::Sign`.
fn ir_sign_names() -> Vec<&'static str> {
    use ori_ir::format_spec::Sign;
    [Sign::Plus, Sign::Minus, Sign::Space]
        .iter()
        .map(|s| match s {
            Sign::Plus => "Plus",
            Sign::Minus => "Minus",
            Sign::Space => "Space",
        })
        .collect()
}

/// Read a source file relative to the compiler workspace root.
fn read_workspace_file(rel_path: &str) -> String {
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let Some(workspace) = manifest_dir.parent() else {
        panic!("oric crate should be inside compiler/");
    };
    std::fs::read_to_string(workspace.join(rel_path))
        .unwrap_or_else(|e| panic!("failed to read {rel_path}: {e}"))
}

#[test]
fn format_type_variants_synced_with_types_registration() {
    let src = read_workspace_file("ori_types/src/check/registration/builtin_types.rs");
    for name in ir_format_type_names() {
        let pattern = format!("\"{name}\"");
        assert!(
            src.contains(&pattern),
            "FormatType variant `{name}` missing from ori_types registration \
             (register_format_type_type in check/registration/builtin_types.rs)"
        );
    }
}

#[test]
fn format_type_variants_synced_with_eval_registration() {
    let src = read_workspace_file("ori_eval/src/interpreter/prelude.rs");
    for name in ir_format_type_names() {
        let pattern = format!("\"{name}\"");
        assert!(
            src.contains(&pattern),
            "FormatType variant `{name}` missing from ori_eval registration \
             (register_format_variants in interpreter/prelude.rs)"
        );
    }
}

#[test]
fn alignment_variants_synced_with_types_registration() {
    let src = read_workspace_file("ori_types/src/check/registration/builtin_types.rs");
    for name in ir_align_names() {
        let pattern = format!("\"{name}\"");
        assert!(
            src.contains(&pattern),
            "Alignment variant `{name}` missing from ori_types registration \
             (register_alignment_type in check/registration/builtin_types.rs)"
        );
    }
}

#[test]
fn alignment_variants_synced_with_eval_registration() {
    let src = read_workspace_file("ori_eval/src/interpreter/prelude.rs");
    for name in ir_align_names() {
        let pattern = format!("\"{name}\"");
        assert!(
            src.contains(&pattern),
            "Alignment variant `{name}` missing from ori_eval registration \
             (register_format_variants in interpreter/prelude.rs)"
        );
    }
}

#[test]
fn sign_variants_synced_with_types_registration() {
    let src = read_workspace_file("ori_types/src/check/registration/builtin_types.rs");
    for name in ir_sign_names() {
        let pattern = format!("\"{name}\"");
        assert!(
            src.contains(&pattern),
            "Sign variant `{name}` missing from ori_types registration \
             (register_sign_type in check/registration/builtin_types.rs)"
        );
    }
}

#[test]
fn sign_variants_synced_with_eval_registration() {
    let src = read_workspace_file("ori_eval/src/interpreter/prelude.rs");
    for name in ir_sign_names() {
        let pattern = format!("\"{name}\"");
        assert!(
            src.contains(&pattern),
            "Sign variant `{name}` missing from ori_eval registration \
             (register_format_variants in interpreter/prelude.rs)"
        );
    }
}

// Well-known generic type resolution consistency

/// Well-known generic types that must be handled in the centralized
/// `resolve_well_known_generic()` function to ensure `Pool` tags match
/// between annotations and inference.
const WELL_KNOWN_GENERIC_TYPES: &[&str] = &[
    "Channel",
    "DoubleEndedIterator",
    "Iterator",
    "Option",
    "Range",
    "Result",
    "Set",
];

/// The centralized `resolve_well_known_generic()` in `check/well_known.rs`
/// must contain all well-known generic types, and all three resolution
/// functions must delegate to it.
#[test]
fn well_known_generic_types_consistent() {
    use std::path::PathBuf;

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let Some(workspace) = manifest_dir.parent() else {
        panic!("oric crate should be inside compiler/");
    };

    // 1. Verify the single source of truth contains all types.
    let well_known_path = workspace.join("ori_types/src/check/well_known/mod.rs");
    let well_known_src = std::fs::read_to_string(&well_known_path)
        .unwrap_or_else(|e| panic!("failed to read well_known/mod.rs: {e}"));

    for &ty in WELL_KNOWN_GENERIC_TYPES {
        let pattern = format!("\"{ty}\"");
        assert!(
            well_known_src.contains(&pattern),
            "Well-known generic type `{ty}` missing from check/well_known/mod.rs\n\
             Add a match arm for `(\"{ty}\", N)` in resolve_well_known_generic()."
        );
    }

    // 2. Verify all three consumers delegate to the shared helper.
    let consumers: &[(&str, &str)] = &[
        (
            "registration",
            "ori_types/src/check/registration/type_resolution.rs",
        ),
        ("signatures", "ori_types/src/check/signatures/mod.rs"),
        (
            "type_resolution",
            "ori_types/src/infer/expr/type_resolution.rs",
        ),
    ];

    for &(label, rel_path) in consumers {
        let path = workspace.join(rel_path);
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("failed to read {rel_path}: {e}"));

        assert!(
            source.contains("resolve_well_known_generic"),
            "{label} ({rel_path}) does not call resolve_well_known_generic().\n\
             All three resolution functions must delegate to the shared helper \
             in check/well_known.rs to prevent drift."
        );
    }
}
