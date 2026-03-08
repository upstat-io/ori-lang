//! Tests for consistency between the evaluator, `ori_registry` type definitions,
//! and the `ori_ir` builtin method registry.
//!
//! The registry (`ori_registry::BUILTIN_TYPES`) is the single source of truth
//! for what methods each type has. Both the type checker and evaluator read from
//! it. These tests validate cross-phase alignment that can't be enforced at
//! compile time.

use std::collections::BTreeSet;

use ori_eval::interpreter::resolvers::CollectionMethod;
use ori_ir::builtin_methods::BUILTIN_METHODS;
use ori_registry::legacy_type_name;

/// Build the set of `(type_name, method_name)` pairs from the registry.
///
/// DEI-only methods are listed under `"DoubleEndedIterator"` to match the
/// naming convention used by the IR registry. Type names are normalized
/// to the legacy lowercase convention via [`legacy_type_name()`].
fn registry_method_pairs() -> BTreeSet<(&'static str, &'static str)> {
    let mut set = BTreeSet::new();
    for td in ori_registry::BUILTIN_TYPES {
        let type_name = legacy_type_name(td.name);
        for m in td.methods {
            if m.dei_only {
                set.insert(("DoubleEndedIterator", m.name));
            } else {
                set.insert((type_name, m.name));
            }
        }
    }
    set
}

/// Build the set of `(type_name, method_name)` from the IR registry.
fn ir_method_set() -> BTreeSet<(&'static str, &'static str)> {
    BUILTIN_METHODS
        .iter()
        .map(|m| (m.receiver.name(), m.name))
        .collect()
}

/// Collection types that have registry methods but are not yet in the
/// `ori_ir` builtin method registry. Names use legacy lowercase convention
/// for mapped types (error, list, map, range, tuple) and `PascalCase` for
/// types not yet mapped (Channel, Iterator, etc.).
const COLLECTION_TYPES: &[&str] = &[
    "Channel",
    "DoubleEndedIterator",
    "Iterator",
    "Option",
    "Result",
    "Set",
    "error",
    "list",
    "map",
    "range",
    "tuple",
];

/// Registry/typeck methods for primitive types not yet in the IR registry.
/// These need to be added to `ori_ir/src/builtin_methods/mod.rs`.
///
/// **Cross-reference:** `METHODS_NOT_YET_IN_EVAL` in `dispatch_coverage.rs`
/// covers a similar but broader gap (registry methods not in eval). Both
/// allowlists will be eliminated by `plans/type_strategy_registry/` Section 13.
///
/// Kept until Section 13 consolidates `BUILTIN_METHODS` into the registry.
const TYPECK_METHODS_NOT_IN_IR: &[(&str, &str)] = &[
    // Duration — conversion aliases and factory methods
    ("Duration", "abs"),
    ("Duration", "as_micros"),
    ("Duration", "as_millis"),
    ("Duration", "as_nanos"),
    ("Duration", "as_seconds"),
    ("Duration", "format"),
    ("Duration", "from_hours"),
    ("Duration", "from_micros"),
    ("Duration", "from_microseconds"),
    ("Duration", "from_millis"),
    ("Duration", "from_milliseconds"),
    ("Duration", "from_minutes"),
    ("Duration", "from_nanos"),
    ("Duration", "from_nanoseconds"),
    ("Duration", "from_seconds"),
    ("Duration", "is_negative"),
    ("Duration", "is_positive"),
    ("Duration", "is_zero"),
    ("Duration", "to_micros"),
    ("Duration", "to_millis"),
    ("Duration", "to_nanos"),
    ("Duration", "to_seconds"),
    ("Duration", "zero"),
    // Size — conversion aliases and factory methods
    ("Size", "as_bytes"),
    ("Size", "format"),
    ("Size", "from_bytes"),
    ("Size", "from_gb"),
    ("Size", "from_gigabytes"),
    ("Size", "from_kb"),
    ("Size", "from_kilobytes"),
    ("Size", "from_mb"),
    ("Size", "from_megabytes"),
    ("Size", "from_tb"),
    ("Size", "from_terabytes"),
    ("Size", "is_zero"),
    ("Size", "to_bytes"),
    ("Size", "to_gb"),
    ("Size", "to_kb"),
    ("Size", "to_mb"),
    ("Size", "to_str"),
    ("Size", "to_tb"),
    ("Size", "zero"),
    // bool — typeck has conversions that IR doesn't list yet
    ("bool", "to_int"),
    // byte — arithmetic operators and predicates not in IR
    ("byte", "add"),
    ("byte", "bit_and"),
    ("byte", "bit_not"),
    ("byte", "bit_or"),
    ("byte", "bit_xor"),
    ("byte", "div"),
    ("byte", "is_ascii"),
    ("byte", "is_ascii_alpha"),
    ("byte", "is_ascii_digit"),
    ("byte", "is_ascii_whitespace"),
    ("byte", "mul"),
    ("byte", "rem"),
    ("byte", "shl"),
    ("byte", "shr"),
    ("byte", "sub"),
    ("byte", "to_char"),
    ("byte", "to_int"),
    // char — typeck has conversions and predicates not in IR
    ("char", "is_alpha"),
    ("char", "is_ascii"),
    ("char", "is_digit"),
    ("char", "is_lowercase"),
    ("char", "is_uppercase"),
    ("char", "is_whitespace"),
    ("char", "to_byte"),
    ("char", "to_int"),
    ("char", "to_lowercase"),
    ("char", "to_uppercase"),
    // Ordering — then_with takes closure, not expressible in IR ParamSpec
    ("Ordering", "then_with"),
    // error — Into trait and Traceable, not in IR registry
    ("error", "clone"),
    ("error", "debug"),
    ("error", "has_trace"),
    ("error", "message"),
    ("error", "to_str"),
    ("error", "trace"),
    ("error", "trace_entries"),
    ("error", "with_trace"),
    // float — typeck has many math methods not in IR
    ("float", "acos"),
    ("float", "asin"),
    ("float", "atan"),
    ("float", "atan2"),
    ("float", "cbrt"),
    ("float", "clamp"),
    ("float", "cos"),
    ("float", "exp"),
    // Float hash — compound type hash support, not yet in IR
    ("float", "hash"),
    ("float", "is_finite"),
    ("float", "is_infinite"),
    ("float", "is_nan"),
    ("float", "is_negative"),
    ("float", "is_normal"),
    ("float", "is_positive"),
    ("float", "is_zero"),
    ("float", "ln"),
    ("float", "log10"),
    ("float", "log2"),
    ("float", "pow"),
    ("float", "rem"),
    ("float", "signum"),
    ("float", "sin"),
    ("float", "tan"),
    ("float", "to_int"),
    ("float", "trunc"),
    // int — typeck has methods not in IR
    ("int", "byte"),
    ("int", "clamp"),
    ("int", "f"),
    ("int", "into"),
    ("int", "is_even"),
    ("int", "is_negative"),
    ("int", "is_odd"),
    ("int", "is_positive"),
    ("int", "is_zero"),
    ("int", "pow"),
    ("int", "signum"),
    ("int", "to_byte"),
    ("int", "to_float"),
    // str — typeck has many methods not in IR
    ("str", "as_bytes"),
    ("str", "byte_len"),
    ("str", "bytes"),
    ("str", "chars"),
    ("str", "concat"),
    ("str", "from_utf8"),
    ("str", "from_utf8_unchecked"),
    ("str", "index_of"),
    ("str", "into"),
    ("str", "iter"),
    ("str", "last_index_of"),
    ("str", "length"),
    ("str", "lines"),
    ("str", "pad_end"),
    ("str", "pad_start"),
    ("str", "parse_float"),
    ("str", "parse_int"),
    ("str", "repeat"),
    ("str", "replace"),
    ("str", "slice"),
    ("str", "split"),
    ("str", "substring"),
    ("str", "to_bytes"),
    ("str", "to_float"),
    ("str", "to_int"),
    ("str", "to_str"),
    ("str", "trim_end"),
    ("str", "trim_start"),
];

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

// Registry ↔ IR alignment (kept until Section 13)

/// Every registry method for primitive types should be in the IR registry.
#[test]
fn registry_primitive_methods_in_ir() {
    let ir_set = ir_method_set();
    let known_set: BTreeSet<_> = TYPECK_METHODS_NOT_IN_IR.iter().copied().collect();
    let registry_set = registry_method_pairs();

    let mut missing = Vec::new();
    for &(ty, method) in &registry_set {
        if COLLECTION_TYPES.contains(&ty) {
            continue;
        }
        if !ir_set.contains(&(ty, method)) && !known_set.contains(&(ty, method)) {
            missing.push((ty, method));
        }
    }

    assert!(
        missing.is_empty(),
        "Registry has primitive methods not in IR registry: {missing:?}\n\
         Add method definitions in ori_ir/src/builtin_methods/mod.rs or \
         add to TYPECK_METHODS_NOT_IN_IR"
    );
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
