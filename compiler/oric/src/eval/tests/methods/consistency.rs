//! Cross-phase enforcement tests for `ori_registry` type definitions.
//!
//! The registry (`ori_registry::BUILTIN_TYPES`) is the single source of truth
//! for what methods each type has. All compiler phases (type checker, evaluator,
//! ARC pass, LLVM backend) consume it. These tests verify that every phase
//! faithfully implements every registry-declared method.
//!
//! # Organization
//!
//! - **Registry integrity**: sorted methods, iterator consistency
//! - **Cross-phase enforcement**: typeck, eval, ARC borrow set, `backend_required`, `pure`
//! - **Format spec sync**: FormatType/Alignment/Sign enum consistency
//! - **Well-known generics**: registry-derived generic type resolution
//!
//! # LLVM enforcement
//!
//! LLVM-specific enforcement tests live in `ori_llvm/src/codegen/arc_emitter/
//! builtins/tests.rs` (where `pub(crate)` `BuiltinTable` is accessible):
//! - `no_phantom_builtin_entries` — every `BuiltinTable` entry has registry backing
//! - `builtin_coverage_above_threshold` — codegen coverage tracking
//! - `registry_op_strategies_cover_all_operators` — all `OpStrategy` variants handled

use std::collections::BTreeSet;

use ori_eval::interpreter::resolvers::CollectionMethod;
use ori_registry::{Ownership, TypeParamArity, TypeTag, BUILTIN_TYPES};

// Registry integrity

/// Registry methods must be sorted alphabetically within each `TypeDef`.
#[test]
fn registry_methods_sorted_per_type() {
    for td in BUILTIN_TYPES {
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

/// Verify that every Iterator/DoubleEndedIterator method in the registry has
/// a corresponding `CollectionMethod` variant in the evaluator, and vice versa.
#[test]
fn iterator_methods_match_registry() {
    let registry_iter_methods: BTreeSet<&str> = BUILTIN_TYPES
        .iter()
        .filter(|td| td.tag == TypeTag::Iterator)
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

// Cross-phase enforcement tests

/// Every registry method must be findable via `ori_registry::has_method()`,
/// which is the type checker's resolution path (post-Section 09).
///
/// Replaces: `typeck_method_list_is_sorted`, `typeck_primitive_methods_in_ir`,
/// `eval_methods_recognized_by_typeck` (all eliminated in Sections 09-10).
///
/// For plain `Iterator`, DEI-only methods are excluded by `has_method()`.
/// They must be findable via `DoubleEndedIterator` instead.
#[test]
fn every_registry_method_has_typeck_handler() {
    let mut missing = Vec::new();

    for type_def in BUILTIN_TYPES {
        for method in type_def.methods {
            let tag = type_def.tag;

            // DEI-only methods on the Iterator TypeDef are not visible via
            // has_method(Iterator, name) — verify they're visible via DEI tag
            if tag == TypeTag::Iterator && method.dei_only {
                assert!(
                    ori_registry::has_method(TypeTag::DoubleEndedIterator, method.name),
                    "DEI-only method `Iterator.{}` not findable via DoubleEndedIterator tag",
                    method.name,
                );
                continue;
            }

            if !ori_registry::has_method(tag, method.name) {
                missing.push(format!("{}.{}", type_def.name, method.name));
            }
        }
    }

    assert!(
        missing.is_empty(),
        "Registry methods not resolvable by type checker ({} missing):\n  {}",
        missing.len(),
        missing.join("\n  "),
    );
}

/// Every registry method must be dispatchable by the evaluator.
///
/// Uses `ori_eval::can_dispatch_builtin()` which checks both resolver paths
/// (`CollectionMethodResolver` + `BuiltinMethodResolver`). Methods in the
/// `METHODS_NOT_YET_IN_EVAL` allowlist are temporarily exempt.
///
/// Replaces: `ir_methods_implemented_in_eval`, `eval_method_list_is_sorted`,
/// `eval_primitive_methods_in_ir`, `typeck_methods_implemented_in_eval`,
/// `iterator_typeck_methods_match_eval_resolver`, `eval_iterator_method_names_sorted`
/// (all eliminated in Sections 09-10).
#[test]
fn every_registry_method_has_eval_handler() {
    let interner = super::test_interner();
    let allowlist: BTreeSet<(&str, &str)> = super::dispatch_coverage::METHODS_NOT_YET_IN_EVAL
        .iter()
        .copied()
        .collect();

    let mut missing = Vec::new();
    let mut implemented = Vec::new();

    for type_def in BUILTIN_TYPES {
        for method in type_def.methods {
            let pair = (type_def.name, method.name);
            let dispatched = ori_eval::can_dispatch_builtin(type_def.tag, method.name, &interner);

            if dispatched && allowlist.contains(&pair) {
                implemented.push(format!("{}.{}", type_def.name, method.name));
            } else if !dispatched && !allowlist.contains(&pair) {
                missing.push(format!("{}.{}", type_def.name, method.name));
            }
        }
    }

    assert!(
        missing.is_empty(),
        "Registry methods not handled by evaluator ({} missing):\n  {}\n\
         Add a dispatch handler or add to METHODS_NOT_YET_IN_EVAL.",
        missing.len(),
        missing.join("\n  "),
    );

    assert!(
        implemented.is_empty(),
        "Methods in METHODS_NOT_YET_IN_EVAL now have dispatch handlers — \
         remove from allowlist:\n  {}",
        implemented.join("\n  "),
    );
}

/// Every method with `Ownership::Borrow` in the registry must appear in
/// the ARC borrow set (via `ori_registry::borrowing_method_names()`).
///
/// Excluded: Iterator methods (ARC can't model iterator dependencies)
/// and `.iter()` (creates iterator referencing receiver data).
#[test]
fn every_registry_borrowing_method_in_arc_set() {
    let arc_borrow_set: BTreeSet<&str> = ori_registry::borrowing_method_names()
        .iter()
        .copied()
        .collect();

    let mut missing = Vec::new();

    for type_def in BUILTIN_TYPES {
        // Iterator methods are excluded from borrowing_method_names()
        if type_def.tag == TypeTag::Iterator {
            continue;
        }

        for method in type_def.methods {
            if method.receiver == Ownership::Borrow
                && method.name != "iter"
                && !arc_borrow_set.contains(method.name)
            {
                missing.push(format!("{}.{}", type_def.name, method.name));
            }
        }
    }

    assert!(
        missing.is_empty(),
        "Borrowing methods not in ARC borrow set ({} missing):\n  {}",
        missing.len(),
        missing.join("\n  "),
    );
}

/// Every method with `backend_required: true` must be dispatchable by the
/// evaluator (LLVM enforcement is in `ori_llvm`'s own test suite).
///
/// Methods in `METHODS_NOT_YET_IN_EVAL` are temporarily exempt — but the
/// intent is that `backend_required` methods get implemented in both backends.
#[test]
fn backend_required_methods_fully_implemented() {
    let interner = super::test_interner();
    let allowlist: BTreeSet<(&str, &str)> = super::dispatch_coverage::METHODS_NOT_YET_IN_EVAL
        .iter()
        .copied()
        .collect();

    let mut eval_missing = Vec::new();

    for type_def in BUILTIN_TYPES {
        for method in type_def.methods {
            if !method.backend_required {
                continue;
            }

            let pair = (type_def.name, method.name);
            if allowlist.contains(&pair) {
                continue; // Known eval gap
            }

            if !ori_eval::can_dispatch_builtin(type_def.tag, method.name, &interner) {
                eval_missing.push(format!("{}.{}", type_def.name, method.name));
            }
        }
    }

    assert!(
        eval_missing.is_empty(),
        "backend_required methods not handled by evaluator ({} missing):\n  {}\n\
         LLVM enforcement: see ori_llvm/src/codegen/arc_emitter/builtins/tests.rs",
        eval_missing.len(),
        eval_missing.join("\n  "),
    );
}

/// Methods marked `pure: true` must not consume their receiver (`Ownership::Owned`
/// implies mutation/consumption, contradicting purity). At least 30% of methods
/// should be pure (catches accidentally defaulting everything to false).
#[test]
fn pure_method_sanity() {
    let mut total_methods = 0usize;
    let mut pure_count = 0usize;

    for type_def in BUILTIN_TYPES {
        for method in type_def.methods {
            total_methods += 1;
            if method.pure {
                pure_count += 1;
                assert_ne!(
                    method.receiver,
                    Ownership::Owned,
                    "Method `{}.{}` is marked pure but has Ownership::Owned receiver. \
                     Pure methods should borrow, not consume.",
                    type_def.name,
                    method.name,
                );
            }
        }
    }

    let pure_pct = pure_count
        .checked_mul(100)
        .and_then(|n| n.checked_div(total_methods))
        .unwrap_or(0);
    assert!(
        pure_pct >= 30,
        "Only {pure_pct}% ({pure_count}/{total_methods}) methods marked pure. \
         Expected at least 30%. Check that pure is being set correctly.",
    );
}

// Testing matrix (type x method x phase)

/// Unified coverage report: for every registry method, count how many phases
/// handle it. Asserts typeck == total (tautological — registry IS typeck source).
/// Reports eval coverage vs allowlist. ARC borrow count verified structurally.
///
/// LLVM coverage is enforced by `ori_llvm`'s own tests (`builtin_coverage_above_threshold`,
/// `no_phantom_builtin_entries`) — not accessible from oric due to `pub(crate)`.
#[test]
fn testing_matrix_coverage() {
    let interner = super::test_interner();
    let eval_allowlist: BTreeSet<(&str, &str)> = super::dispatch_coverage::METHODS_NOT_YET_IN_EVAL
        .iter()
        .copied()
        .collect();

    let mut total = 0usize;
    let mut typeck_count = 0usize;
    let mut eval_count = 0usize;
    let mut arc_borrow_count = 0usize;

    for type_def in BUILTIN_TYPES {
        for method in type_def.methods {
            total += 1;

            // Type checker: has_method with DEI-aware filtering
            let has_typeck = if type_def.tag == TypeTag::Iterator && method.dei_only {
                ori_registry::has_method(TypeTag::DoubleEndedIterator, method.name)
            } else {
                ori_registry::has_method(type_def.tag, method.name)
            };
            if has_typeck {
                typeck_count += 1;
            }

            // Evaluator: can_dispatch_builtin checks both resolvers
            if ori_eval::can_dispatch_builtin(type_def.tag, method.name, &interner) {
                eval_count += 1;
            }

            // ARC: borrowing methods minus exclusions
            if method.receiver == Ownership::Borrow
                && type_def.tag != TypeTag::Iterator
                && method.name != "iter"
            {
                arc_borrow_count += 1;
            }
        }
    }

    // Typeck MUST handle all methods (registry IS the typeck source)
    assert_eq!(
        typeck_count,
        total,
        "Type checker missing {}/{total} registry methods",
        total.saturating_sub(typeck_count)
    );

    // Eval: report coverage. The gap is tracked by METHODS_NOT_YET_IN_EVAL.
    let eval_gap = total.saturating_sub(eval_count);
    assert_eq!(
        eval_gap,
        eval_allowlist.len(),
        "Eval coverage mismatch: {eval_gap} missing methods vs {} allowlisted. \
         Update METHODS_NOT_YET_IN_EVAL if methods were implemented or added.",
        eval_allowlist.len()
    );

    // ARC: verify count matches borrowing_method_names() after dedup
    // (borrowing_method_names returns deduplicated names across all types,
    // so count may differ from our per-type count)
    let arc_set_size = ori_registry::borrowing_method_names().len();
    assert!(
        arc_set_size > 0,
        "ARC borrow set is empty — something is wrong"
    );

    // Report the matrix (visible in test output with --nocapture)
    eprintln!(
        "Testing matrix: {total} methods, \
         typeck={typeck_count}, eval={eval_count} (gap={eval_gap}), \
         arc_borrow={arc_borrow_count} (deduped set={arc_set_size})",
    );
}

// Format spec variant registration consistency

/// Source-of-truth variant names for `ori_ir::FormatType`.
///
/// Const exhaustive match ensures compile failure if a variant is added.
fn ir_format_type_names() -> &'static [&'static str] {
    use ori_ir::format_spec::FormatType;
    const _: () = {
        match FormatType::Binary {
            FormatType::Binary
            | FormatType::Octal
            | FormatType::Hex
            | FormatType::HexUpper
            | FormatType::Exp
            | FormatType::ExpUpper
            | FormatType::Fixed
            | FormatType::Percent => {}
        }
    };
    &[
        "Binary", "Octal", "Hex", "HexUpper", "Exp", "ExpUpper", "Fixed", "Percent",
    ]
}

/// Source-of-truth variant names for `ori_ir::Align`.
fn ir_align_names() -> &'static [&'static str] {
    use ori_ir::format_spec::Align;
    const _: () = {
        match Align::Left {
            Align::Left | Align::Center | Align::Right => {}
        }
    };
    &["Left", "Center", "Right"]
}

/// Source-of-truth variant names for `ori_ir::Sign`.
fn ir_sign_names() -> &'static [&'static str] {
    use ori_ir::format_spec::Sign;
    const _: () = {
        match Sign::Plus {
            Sign::Plus | Sign::Minus | Sign::Space => {}
        }
    };
    &["Plus", "Minus", "Space"]
}

/// Read a source file relative to the compiler workspace root.
fn read_workspace_file(rel_path: &str) -> String {
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let Some(workspace) = manifest_dir.parent() else {
        panic!("oric crate should be inside compiler/ (reading {rel_path})");
    };
    std::fs::read_to_string(workspace.join(rel_path))
        .unwrap_or_else(|e| panic!("failed to read {rel_path}: {e}"))
}

/// `FormatType`, `Alignment`, and `Sign` enums must be consistent between `ori_ir`
/// (source of truth), `ori_types` (registration), and `ori_eval` (runtime globals).
///
/// Migrated from the 6 individual format variant tests in the previous
/// consistency.rs. Logic is identical; just unified into a single test.
#[test]
fn format_spec_variants_synced() {
    let types_src = read_workspace_file("ori_types/src/check/registration/builtin_types.rs");
    let eval_src = read_workspace_file("ori_eval/src/interpreter/prelude.rs");

    // FormatType variants
    for name in ir_format_type_names() {
        let pattern = format!("\"{name}\"");
        assert!(
            types_src.contains(&pattern),
            "FormatType variant `{name}` missing from ori_types registration \
             (register_format_type_type in check/registration/builtin_types.rs)"
        );
        assert!(
            eval_src.contains(&pattern),
            "FormatType variant `{name}` missing from ori_eval registration \
             (register_format_variants in interpreter/prelude.rs)"
        );
    }

    // Alignment variants
    for name in ir_align_names() {
        let pattern = format!("\"{name}\"");
        assert!(
            types_src.contains(&pattern),
            "Alignment variant `{name}` missing from ori_types registration \
             (register_alignment_type in check/registration/builtin_types.rs)"
        );
        assert!(
            eval_src.contains(&pattern),
            "Alignment variant `{name}` missing from ori_eval registration \
             (register_format_variants in interpreter/prelude.rs)"
        );
    }

    // Sign variants
    for name in ir_sign_names() {
        let pattern = format!("\"{name}\"");
        assert!(
            types_src.contains(&pattern),
            "Sign variant `{name}` missing from ori_types registration \
             (register_sign_type in check/registration/builtin_types.rs)"
        );
        assert!(
            eval_src.contains(&pattern),
            "Sign variant `{name}` missing from ori_eval registration \
             (register_format_variants in interpreter/prelude.rs)"
        );
    }
}

// Well-known generic type resolution consistency

/// Well-known generic types (derived from registry) must be handled in the
/// centralized `resolve_well_known_generic()` function, and all three
/// resolution consumers must delegate to the shared helper.
///
/// Replaces: `well_known_generic_types_consistent` and
/// `well_known_generic_types_matches_registry` (merged + registry-derived).
#[test]
fn well_known_generic_types_consistent() {
    // Types resolved structurally via pool.list(), pool.map()
    // rather than name-based resolve_well_known_generic().
    const STRUCTURALLY_RESOLVED: &[&str] = &["List", "Map"];

    // Derive expected list from BUILTIN_TYPES (generic types with arity > 0)
    let well_known: Vec<&str> = BUILTIN_TYPES
        .iter()
        .filter(|td| matches!(td.type_params, TypeParamArity::Fixed(n) if n > 0))
        .filter(|td| !STRUCTURALLY_RESOLVED.contains(&td.name))
        .map(|td| td.name)
        .collect();

    assert!(
        !well_known.is_empty(),
        "No generic types found in registry — something is wrong"
    );

    // 1. Verify the single source of truth contains all types
    let well_known_src = read_workspace_file("ori_types/src/check/well_known/mod.rs");
    for ty in &well_known {
        let pattern = format!("\"{ty}\"");
        assert!(
            well_known_src.contains(&pattern),
            "Well-known generic type `{ty}` missing from check/well_known/mod.rs\n\
             Add a match arm for `(\"{ty}\", N)` in resolve_well_known_generic()."
        );
    }

    // 2. Verify all three consumers delegate to the shared helper
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
        let source = read_workspace_file(rel_path);
        assert!(
            source.contains("resolve_well_known_generic"),
            "{label} ({rel_path}) does not call resolve_well_known_generic().\n\
             All three resolution functions must delegate to the shared helper \
             in check/well_known.rs to prevent drift."
        );
    }
}
