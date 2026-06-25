//! Self-verifying matrix completeness counter for method-generic const dispatch.
//!
//! The counter iterates the cartesian product of all eight active dimensions
//! exercised by the in-scope cells, filters by per-cell `prereq_available()`
//! (returns `false` for `#skip`'d cells like turbofish-gated rows), accumulates
//! the count, and asserts equality against an explicit constant computed from
//! the axis cardinalities minus the documented `#skip` count. Skipped cells are
//! enumerated by name in the test body so a future feature landing (turbofish
//! parser) trivially flips the skip flag.

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
enum CallShape {
    Inherent,
    TraitImpl,
    TypeAndConst,
    ConstOnly,
    MultiConstConditional,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
enum ConstPattern {
    NEqualsZero,
    NEqualsOne,
    NEqualsIntMax,
    Negative,
    LiteralSite,
    DollarLetBinding,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
enum ElementType {
    Int,
    Str,
    OptionInt,
    BoxInt,
    TupleIntInt,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
enum Feature {
    Closure,
    QuestionMark,
    MatchArm,
    Composed,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
enum Backend {
    Interpreter,
    LlvmAot,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
enum Phase {
    Typeck,
    Canon,
    Eval,
    Llvm,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
enum ParserSyntax {
    AscriptionDriven,
    ExplicitTurbofish,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
enum ModuleBoundary {
    SameModule,
    CrossModule,
}

const CALL_SHAPES: &[CallShape] = &[
    CallShape::Inherent,
    CallShape::TraitImpl,
    CallShape::TypeAndConst,
    CallShape::ConstOnly,
    CallShape::MultiConstConditional,
];

const CONST_PATTERNS: &[ConstPattern] = &[
    ConstPattern::NEqualsZero,
    ConstPattern::NEqualsOne,
    ConstPattern::NEqualsIntMax,
    ConstPattern::Negative,
    ConstPattern::LiteralSite,
    ConstPattern::DollarLetBinding,
];

const ELEMENT_TYPES: &[ElementType] = &[
    ElementType::Int,
    ElementType::Str,
    ElementType::OptionInt,
    ElementType::BoxInt,
    ElementType::TupleIntInt,
];

const FEATURES: &[Feature] = &[
    Feature::Closure,
    Feature::QuestionMark,
    Feature::MatchArm,
    Feature::Composed,
];

const BACKENDS: &[Backend] = &[Backend::Interpreter, Backend::LlvmAot];

const PHASES: &[Phase] = &[Phase::Typeck, Phase::Canon, Phase::Eval, Phase::Llvm];

const PARSER_SYNTAXES: &[ParserSyntax] = &[
    ParserSyntax::AscriptionDriven,
    ParserSyntax::ExplicitTurbofish,
];

const MODULE_BOUNDARIES: &[ModuleBoundary] =
    &[ModuleBoundary::SameModule, ModuleBoundary::CrossModule];

/// Per-cell prerequisite-availability gate. Returns `false` for cells blocked
/// by an unshipped feature; those cells are documented as `#skip` in the
/// matching `.ori` fixture or as `#[ignore]` in the matching Rust test.
fn prereq_available(
    call: CallShape,
    pattern: ConstPattern,
    element: ElementType,
    feature: Feature,
    parser: ParserSyntax,
    module: ModuleBoundary,
) -> bool {
    // Turbofish on method calls is not yet parsed; every ExplicitTurbofish
    // cell is gated until the parser ships it.
    if parser == ParserSyntax::ExplicitTurbofish {
        return false;
    }

    // MultiConstConditional requires turbofish (no ascription path drives
    // multiple const args from one let binding); co-blocked on the
    // AscriptionDriven axis until turbofish ships.
    if call == CallShape::MultiConstConditional {
        return false;
    }

    let _ = pattern; // all patterns gated by the const-dispatch feature surface

    // Cross-module testing requires multi-file .ori test infrastructure beyond
    // the current single-file fixture model.
    if module == ModuleBoundary::CrossModule {
        return false;
    }

    // Closure turbofish syntax must parse first; same parser dispatch gap as
    // ExplicitTurbofish.
    if feature == Feature::Closure {
        return false;
    }

    // ? operator on Option<[T, max N]>: fixed-capacity erasure collapses
    // `[T, max N]` to `[T]` today, so capacity is lost across `?`.
    if feature == Feature::QuestionMark {
        return false;
    }

    // Tuple element coverage is a follow-up; the tuple type follows the same
    // canon side-table path as struct/sum element types, covered by the other
    // element types in the immediate matrix.
    if element == ElementType::TupleIntInt {
        return false;
    }

    true
}

#[test]
fn matrix_cell_count_matches_expected_constant() {
    let mut enabled = 0usize;
    let mut total = 0usize;
    for &call in CALL_SHAPES {
        for &pattern in CONST_PATTERNS {
            for &element in ELEMENT_TYPES {
                for &feature in FEATURES {
                    for &backend in BACKENDS {
                        for &phase in PHASES {
                            for &parser in PARSER_SYNTAXES {
                                for &module in MODULE_BOUNDARIES {
                                    total += 1;
                                    let _ = backend;
                                    let _ = phase;
                                    if prereq_available(
                                        call, pattern, element, feature, parser, module,
                                    ) {
                                        enabled += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Cardinality assertion: 5 x 6 x 5 x 4 x 2 x 4 x 2 x 2 = 19200 total cells.
    let cardinality = CALL_SHAPES.len()
        * CONST_PATTERNS.len()
        * ELEMENT_TYPES.len()
        * FEATURES.len()
        * BACKENDS.len()
        * PHASES.len()
        * PARSER_SYNTAXES.len()
        * MODULE_BOUNDARIES.len();
    assert_eq!(
        total, cardinality,
        "matrix axis cardinality drifted from design (expected {cardinality})",
    );
    assert_eq!(
        total, 19200,
        "axis cardinality changed — re-derive expected count"
    );

    // Filter math (each `false` arm in `prereq_available` is mutually
    // independent; the actual enabled-count is computed by traversal, not by
    // hand-multiplication, to keep the test resilient to gate refinements).
    //
    // Cells surviving all gates: 4 call-shapes x 6 patterns x 4 element types
    // x 2 features x 2 backends x 4 phases x 1 parser x 1 module
    // = 4 * 6 * 4 * 2 * 2 * 4 * 1 * 1 = 1536.
    //
    // When turbofish + closure gates lift, this constant grows and is updated
    // in the same commit as the gate that lifted it.
    assert_eq!(
        enabled, 1536,
        "enabled-cell count drifted; recompute against the prereq gates AND \
         the in-scope cell list. If a new cell shipped, update the gate AND \
         this constant in the same commit per impl-hygiene Two Hats.",
    );
}

/// Documents the named `#skip` enumeration so a future feature landing
/// (turbofish parser) trivially flips the skip flag.
#[test]
fn skipped_cells_enumerated_by_name() {
    // Each entry: (description, blocker, fixture_path).
    let documented_skips: &[(&str, &str, &str)] = &[
        (
            "explicit turbofish call shape",
            "turbofish parser",
            "method_generics_const_explicit_turbofish.ori",
        ),
        (
            "multi-const method (requires turbofish)",
            "turbofish parser + const dispatch",
            "method_generics_const_multi.ori",
        ),
        (
            "closure feature interaction (requires closure turbofish)",
            "turbofish parser",
            "(deferred — no fixture file yet)",
        ),
        (
            "? operator on Option<[T, max N]> (fixed-capacity erasure)",
            "fixed-capacity erasure (target-only)",
            "(deferred — no fixture file yet)",
        ),
        (
            "tuple element type (cross-type follow-up)",
            "const dispatch follow-up",
            "(deferred — covered by other element types)",
        ),
        (
            "cross-module test (multi-file infrastructure)",
            "multi-file test infrastructure",
            "(deferred — multi-file fixture model)",
        ),
    ];

    // Sanity: skip enumeration is non-empty and every entry has a blocker.
    assert!(!documented_skips.is_empty());
    for (desc, blocker, fixture) in documented_skips {
        assert!(
            !desc.is_empty() && !blocker.is_empty() && !fixture.is_empty(),
            "skip enumeration entry must be fully populated: ({desc}, {blocker}, {fixture})",
        );
    }
}
