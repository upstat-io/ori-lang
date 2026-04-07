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

// Cross-phase enforcement tests (Section 14.2)
//
// These tests verify that every compiler phase faithfully implements all
// methods declared in `ori_registry`. No manual lists. No allowlists.
// The registry IS the specification.

/// For each type in `BUILTIN_TYPES`, for each method, verify that the
/// registry resolves it. Since the type checker uses
/// `ori_registry::find_method()` directly (Section 09 wiring), method
/// existence in the registry IS type checker recognition.
///
/// Replaces: `typeck_method_list_is_sorted`, `typeck_primitive_methods_in_ir`,
/// `eval_methods_recognized_by_typeck`.
#[test]
fn every_registry_method_has_typeck_handler() {
    use ori_registry::{find_method, BUILTIN_TYPES};

    let mut missing = Vec::new();

    for type_def in BUILTIN_TYPES {
        for method in type_def.methods {
            // DEI-only methods are filtered out by find_method(Iterator, ...)
            // by design — they're only visible on DoubleEndedIterator.
            let lookup_tag = if method.dei_only {
                ori_registry::TypeTag::DoubleEndedIterator
            } else {
                type_def.tag
            };
            if find_method(lookup_tag, method.name).is_none() {
                missing.push((type_def.name, method.name));
            }
        }
    }

    assert!(
        missing.is_empty(),
        "Registry methods not found by find_method ({} missing):\n{}",
        missing.len(),
        missing
            .iter()
            .map(|(ty, m)| format!("  {ty}.{m}"))
            .collect::<Vec<_>>()
            .join("\n"),
    );
}

/// For each method in `ori_registry::borrowing_method_names()`, verify
/// that `ori_arc::borrowing_builtin_names()` includes it.
///
/// After Section 11, `ori_arc` reads ownership directly from the registry
/// via `ori_registry::borrowing_method_names()`. This test verifies the
/// interned ARC set matches the registry-derived source set.
#[cfg(feature = "llvm")]
#[test]
fn every_registry_borrowing_method_in_arc_set() {
    let interner = super::test_interner();

    // Registry-derived borrowing set (the source of truth)
    let registry_borrowing: BTreeSet<&str> = ori_registry::borrowing_method_names()
        .iter()
        .copied()
        .collect();

    // ARC pipeline's borrowing set (interned)
    let arc_borrowing: BTreeSet<String> = ori_arc::borrowing_builtin_names(&interner)
        .iter()
        .map(|name| interner.lookup(*name).to_string())
        .collect();

    // Every registry borrowing name should be in the ARC set
    let mut missing = Vec::new();
    for name in &registry_borrowing {
        if !arc_borrowing.contains(*name) {
            missing.push(*name);
        }
    }

    assert!(
        missing.is_empty(),
        "Registry borrowing methods not in ARC borrow set ({} missing):\n{}",
        missing.len(),
        missing.join(", "),
    );
}

/// For each method with `backend_required: true`, verify that the
/// evaluator has a handler (does not produce `UndefinedMethod`).
///
/// Methods with `backend_required: false` are intentionally exempt
/// (e.g., associated functions not yet in eval, Channel methods).
///
/// Types without a `Value` representation (Channel, Iterator, DEI) are
/// skipped — their methods are dispatched by specialized resolvers.
#[test]
fn backend_required_methods_in_eval() {
    use ori_patterns::EvalErrorKind;
    use ori_registry::BUILTIN_TYPES;

    use super::dispatch_coverage::minimal_value_for;

    let interner = super::test_interner();

    let mut missing = Vec::new();

    for type_def in BUILTIN_TYPES {
        let Some(receiver) = minimal_value_for(type_def.tag) else {
            continue;
        };

        for method in type_def.methods {
            if !method.backend_required {
                continue;
            }

            let result = ori_eval::dispatch_builtin_method_str(
                receiver.clone(),
                method.name,
                vec![],
                &interner,
            );

            let is_undefined = match &result {
                Err(action) => {
                    if let ori_patterns::ControlAction::Error(e) = action {
                        matches!(e.kind, EvalErrorKind::UndefinedMethod { .. })
                    } else {
                        false
                    }
                }
                Ok(_) => false,
            };

            if is_undefined {
                missing.push((type_def.name, method.name));
            }
        }
    }

    assert!(
        missing.is_empty(),
        "backend_required methods missing from evaluator ({} missing):\n{}",
        missing.len(),
        missing
            .iter()
            .map(|(ty, m)| format!("  {ty}.{m}"))
            .collect::<Vec<_>>()
            .join("\n"),
    );
}

/// Methods marked `pure: true` should not consume their receiver
/// (`Ownership::Owned` implies mutation/consumption, contradicting purity).
///
/// **Exception: iterator methods.** Every method on `Iterator` and
/// `DoubleEndedIterator` is pure (same inputs → same outputs, no
/// observable side effects) but also **consumes** the receiver via
/// `Box::from_raw(iter.cast::<IterState>())`. Purity (referential
/// transparency) and linear consumption (move-only ownership) are
/// independent concepts — iterators are both. The iterator method
/// receiver was previously marked `Borrow` as a workaround, which
/// caused memory leaks. The SSOT is now honest: these
/// methods are `Owned + pure`.
///
/// Also verifies that at least 30% of methods are marked pure (catches
/// the failure mode of someone defaulting everything to `pure: false`).
#[test]
fn pure_method_sanity() {
    use ori_registry::{Ownership, TypeTag, BUILTIN_TYPES};

    let mut total_methods = 0;
    let mut pure_count = 0;

    for type_def in BUILTIN_TYPES {
        // Iterator methods are pure + Owned (see doc comment above).
        // previously `Borrow` as a workaround for the ARC
        // pipeline treating iterators as trivial; that workaround
        // leaked memory and is now fixed at the SSOT.
        let is_iterator = matches!(
            type_def.tag,
            TypeTag::Iterator | TypeTag::DoubleEndedIterator
        );
        for method in type_def.methods {
            total_methods += 1;
            if method.pure {
                pure_count += 1;
                if !is_iterator {
                    assert_ne!(
                        method.receiver,
                        Ownership::Owned,
                        "Method `{}.{}` is marked pure but has Ownership::Owned receiver. \
                         Pure methods should borrow, not consume (except iterator methods — see TPR-07-008).",
                        type_def.name,
                        method.name,
                    );
                }
            }
        }
    }

    let pure_pct = (pure_count * 100) / total_methods;
    assert!(
        pure_pct >= 30,
        "Only {pure_pct}% ({pure_count}/{total_methods}) methods marked pure. \
         Expected at least 30%. Check that pure is being set correctly.",
    );
}

// Testing matrix (type x method x phase)

/// Generate the testing matrix coverage counts.
///
/// Since the type checker reads directly from `ori_registry` (Section 09),
/// typeck coverage is 100% by construction. Eval coverage uses
/// `dispatch_builtin_method_str`. LLVM coverage must be checked in `ori_llvm`.
/// ARC borrow coverage uses `ori_registry::borrowing_method_names()`.
#[test]
fn testing_matrix_coverage() {
    use ori_patterns::EvalErrorKind;
    use ori_registry::{Ownership, BUILTIN_TYPES};

    use super::dispatch_coverage::minimal_value_for;

    let interner = super::test_interner();

    let mut total = 0;
    let mut eval_count = 0;
    let mut eval_skipped = 0;
    let mut arc_borrow_count = 0;

    let borrowing_names: BTreeSet<&str> = ori_registry::borrowing_method_names()
        .iter()
        .copied()
        .collect();

    let mut arc_borrow_expected = 0;

    for type_def in BUILTIN_TYPES {
        let receiver = minimal_value_for(type_def.tag);

        for method in type_def.methods {
            total += 1;

            // Eval: check dispatch (skip types without Value representation)
            if let Some(ref recv) = receiver {
                let result = ori_eval::dispatch_builtin_method_str(
                    recv.clone(),
                    method.name,
                    vec![],
                    &interner,
                );

                let is_undefined = match &result {
                    Err(action) => {
                        if let ori_patterns::ControlAction::Error(e) = action {
                            matches!(e.kind, EvalErrorKind::UndefinedMethod { .. })
                        } else {
                            false
                        }
                    }
                    Ok(_) => false,
                };

                if !is_undefined {
                    eval_count += 1;
                }
            } else {
                eval_skipped += 1;
            }

            // ARC borrow: check if in borrowing set.
            // `borrowing_method_names()` excludes Iterator TypeDef entirely
            // (iterator methods are adapter-based, not borrowed from a
            // concrete receiver) and excludes `iter` (creates derived value).
            let skip_arc = type_def.tag == ori_registry::TypeTag::Iterator || method.name == "iter";
            if method.receiver == Ownership::Borrow && !skip_arc {
                arc_borrow_expected += 1;
                if borrowing_names.contains(method.name) {
                    arc_borrow_count += 1;
                }
            }
        }
    }

    // Typeck is 100% by construction (reads from registry).
    // LLVM coverage checked in ori_llvm tests.
    let eval_pct = eval_count * 100 / total;
    eprintln!(
        "Testing matrix: {total} methods total, \
         typeck={total} (by construction), \
         eval={eval_count}/{total} ({eval_pct}%, {eval_skipped} skipped — no Value repr), \
         arc_borrow={arc_borrow_count}/{arc_borrow_expected}",
    );

    // Eval coverage: all methods with a Value representation should be dispatched.
    // Types without Value repr (Channel, Iterator, DEI) are skipped.
    assert!(
        eval_pct >= 80,
        "Eval coverage dropped to {eval_pct}% ({eval_count}/{total}). \
         Expected at least 80%.",
    );

    // ARC borrow: every Ownership::Borrow method's name should be in the
    // borrowing set (borrowing_method_names() deduplicates across types).
    // This verifies the ARC column matches registry Ownership annotations.
    let mut arc_missing = Vec::new();
    for type_def in BUILTIN_TYPES {
        if type_def.tag == ori_registry::TypeTag::Iterator {
            continue;
        }
        for method in type_def.methods {
            if method.receiver == Ownership::Borrow
                && method.name != "iter"
                && !borrowing_names.contains(method.name)
            {
                arc_missing.push(format!("{}.{}", type_def.name, method.name));
            }
        }
    }
    assert!(
        arc_missing.is_empty(),
        "ARC borrow set missing {} methods with Ownership::Borrow:\n{}",
        arc_missing.len(),
        arc_missing.join("\n"),
    );
}

// Well-known generic type resolution consistency

/// Well-known generic types derived from the registry: types with generic
/// type parameters (`TypeParamArity` != `Fixed(0)`).
///
/// The centralized `resolve_well_known_generic()` in `check/well_known.rs`
/// must contain all these types, and all three resolution functions must
/// delegate to it.
#[test]
fn well_known_generic_types_consistent() {
    use ori_registry::TypeParamArity;

    // Derive expected list from registry: generic types resolved by name
    // (not by syntax). List ([T]), Map ({K:V}), Tuple ((T,U)) use
    // built-in syntax, not resolve_well_known_generic().
    let syntax_builtin_tags = [
        ori_registry::TypeTag::List,
        ori_registry::TypeTag::Map,
        ori_registry::TypeTag::Tuple,
    ];
    let well_known: Vec<&str> = ori_registry::BUILTIN_TYPES
        .iter()
        .filter(|td| {
            !matches!(td.type_params, TypeParamArity::Fixed(0))
                && !syntax_builtin_tags.contains(&td.tag)
        })
        .map(|td| td.name)
        .collect();

    assert!(
        !well_known.is_empty(),
        "No generic types found in registry — TypeParamArity filtering is broken"
    );

    // 1. Verify the single source of truth contains all types.
    let well_known_src = read_workspace_file("ori_types/src/check/well_known/mod.rs");

    for ty in &well_known {
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
        let source = read_workspace_file(rel_path);
        assert!(
            source.contains("resolve_well_known_generic"),
            "{label} ({rel_path}) does not call resolve_well_known_generic().\n\
             All three resolution functions must delegate to the shared helper \
             in check/well_known.rs to prevent drift."
        );
    }
}
