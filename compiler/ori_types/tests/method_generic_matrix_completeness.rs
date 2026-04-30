//! Self-verifying matrix completeness counter for BUG-02-023's TDD matrix.
//!
//! Per `tests.md` §Matrix Testing Rule "Self-verifying matrix completeness"
//! (Zig-derived rule) and `bug-tracker/plans/BUG-02-023/section-03-tdd-matrix.md`
//! lines 105-122 (Round 4 TPR-04-R4-10 codex F4 + R3-9 carry-forward).
//!
//! The counter iterates the cartesian product of all eight active dimensions
//! exercised by §03's in-scope cells, filters by per-cell `prereq_available()`
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

/// Per-cell prerequisite-availability gate. Returns `false` for cells that are
/// blocked by an unshipped feature; those cells are documented as `#skip` in
/// the matching `.ori` fixture or as `#[ignore]` in the matching Rust test.
///
/// Each `false` return MUST point at a concrete blocker (BUG-XX-NNN) per
/// CLAUDE.md §ALL Deferrals Must Have Implementation Anchors.
fn prereq_available(
    call: CallShape,
    pattern: ConstPattern,
    element: ElementType,
    feature: Feature,
    parser: ParserSyntax,
    module: ModuleBoundary,
) -> bool {
    // BUG-01-004: parser does not yet ship turbofish on method calls.
    // Every cell whose parser axis is ExplicitTurbofish is gated until
    // BUG-01-004 lands.
    if parser == ParserSyntax::ExplicitTurbofish {
        return false;
    }

    // MultiConstConditional inherently requires turbofish (no ascription
    // path drives multiple const args from one let binding); co-blocked
    // by BUG-01-004 even on the AscriptionDriven axis.
    if call == CallShape::MultiConstConditional {
        return false;
    }

    // ConstPattern::DollarLetBinding requires Phase E0.2's
    // ParsedType::ConstExpr resolution for $-let-bound names; covered by
    // BUG-02-023 itself.
    let _ = pattern; // all patterns gated by BUG-02-023 alone

    // Cross-module testing requires multi-file .ori test infrastructure
    // beyond the current single-file fixture model. Tracked as a
    // follow-up testing-infrastructure gap (file via /add-bug at Phase 5
    // close-out — see §07 Completion Checklist follow-ups).
    if module == ModuleBoundary::CrossModule {
        return false;
    }

    // Feature::Closure — prerequisite: closure turbofish syntax must
    // parse. Blocked by BUG-01-004 (same parser dispatch issue).
    if feature == Feature::Closure {
        return false;
    }

    // Feature::QuestionMark on Option<[T, max N]> — blocked by TY-2
    // target-only fixed-capacity erasure (`[T, max N]` collapses to `[T]`
    // today, so capacity is lost across `?`). Per §03 line 65.
    if feature == Feature::QuestionMark {
        return false;
    }

    // ElementType::TupleIntInt — deferred (covered by other element types
    // in the immediate fix; tuple element coverage is acceptable as a
    // follow-up since the tuple type follows the same canon side-table
    // path as struct/sum element types). Per §03 cross-type list.
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

    // Cardinality assertion: 5 × 6 × 5 × 4 × 2 × 4 × 2 × 2 = 19200 total cells.
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
        "matrix axis cardinality drifted from §03 design (expected {cardinality})",
    );
    assert_eq!(
        total, 19200,
        "axis cardinality changed — re-derive expected count"
    );

    // Filter math (each `false` arm in `prereq_available` is mutually
    // independent; the actual enabled-count is computed by traversal, not by
    // hand-multiplication, to keep the test resilient to gate refinements).
    //
    // Pre-fix expected enabled count: traversal yields 0 IF every gate fires
    // (turbofish ALL gated AND multi-const ALL gated AND closure ALL gated AND
    // ? ALL gated AND tuple element ALL gated AND cross-module ALL gated).
    // The remaining cells (4 call-shapes × 6 patterns × 4 element types ×
    // 2 features × 2 backends × 4 phases × 1 parser × 1 module) =
    // 4 * 6 * 4 * 2 * 2 * 4 * 1 * 1 = 1536 cells survive all gates.
    //
    // Post-fix (BUG-01-004 + BUG-02-023 land): turbofish + closure gates lift,
    // count grows; the §07 completion checklist updates this constant.
    assert_eq!(
        enabled, 1536,
        "enabled-cell count drifted; recompute against §03 prereq gates AND \
         the §03 in-scope cell list. If a new cell shipped, update the gate \
         AND this constant in the same commit per impl-hygiene §Two Hats.",
    );
}

/// Documents the named `#skip` enumeration per §03 line 120's requirement:
/// "Skipped cells MUST be enumerated by name in the test body so a future
/// feature landing (turbofish parser) trivially flips the skip flag."
#[test]
fn skipped_cells_enumerated_by_name() {
    // Each entry: (description, blocker_anchor, fixture_path).
    let documented_skips: &[(&str, &str, &str)] = &[
        (
            "explicit turbofish call shape",
            "BUG-01-004",
            "method_generics_const_explicit_turbofish.ori",
        ),
        (
            "multi-const method (requires turbofish)",
            "BUG-01-004 + BUG-02-023",
            "method_generics_const_multi.ori",
        ),
        (
            "closure feature interaction (requires closure turbofish)",
            "BUG-01-004",
            "(deferred — no fixture file yet)",
        ),
        (
            "? operator on Option<[T, max N]> (TY-2 fixed-capacity erasure)",
            "TY-2 target-only",
            "(deferred — no fixture file yet)",
        ),
        (
            "tuple element type (cross-type follow-up)",
            "BUG-02-023 follow-up",
            "(deferred — covered by other element types)",
        ),
        (
            "cross-module test (multi-file infrastructure)",
            "testing-infrastructure follow-up",
            "(deferred — file via /add-bug at Phase 5 closeout)",
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
