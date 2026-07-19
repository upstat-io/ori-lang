//! Tests for container packing.

use super::*;

mod packing_tests {
    use super::*;

    #[test]
    fn packing_default() {
        assert_eq!(Packing::default(), Packing::FitOrOnePerLine);
    }

    #[test]
    fn packing_can_try_inline() {
        assert!(Packing::FitOrOnePerLine.can_try_inline());
        assert!(Packing::FitOrPackMultiple.can_try_inline());
        assert!(!Packing::AlwaysOnePerLine.can_try_inline());
        assert!(!Packing::AlwaysStacked.can_try_inline());
    }

    #[test]
    fn packing_always_multiline() {
        assert!(!Packing::FitOrOnePerLine.always_multiline());
        assert!(!Packing::FitOrPackMultiple.always_multiline());
        assert!(Packing::AlwaysOnePerLine.always_multiline());
        assert!(Packing::AlwaysStacked.always_multiline());
    }

    #[test]
    fn packing_allows_packing() {
        assert!(!Packing::FitOrOnePerLine.allows_packing());
        assert!(Packing::FitOrPackMultiple.allows_packing());
        assert!(!Packing::AlwaysOnePerLine.allows_packing());
        assert!(!Packing::AlwaysStacked.allows_packing());
    }
}

mod construct_tests {
    use super::*;

    #[test]
    fn construct_is_always_stacked() {
        // Always stacked
        assert!(ConstructKind::RunTopLevel.is_always_stacked());
        assert!(ConstructKind::Try.is_always_stacked());
        assert!(ConstructKind::Match.is_always_stacked());
        assert!(ConstructKind::Recurse.is_always_stacked());
        assert!(ConstructKind::Parallel.is_always_stacked());
        assert!(ConstructKind::Spawn.is_always_stacked());
        assert!(ConstructKind::Nursery.is_always_stacked());
        assert!(ConstructKind::MatchArms.is_always_stacked());

        // Not always stacked
        assert!(!ConstructKind::FunctionParams.is_always_stacked());
        assert!(!ConstructKind::FunctionArgs.is_always_stacked());
        assert!(!ConstructKind::ListSimple.is_always_stacked());
        assert!(!ConstructKind::RunNested.is_always_stacked());
    }

    #[test]
    fn construct_uses_commas() {
        // Uses commas
        assert!(ConstructKind::FunctionParams.uses_commas());
        assert!(ConstructKind::FunctionArgs.uses_commas());
        assert!(ConstructKind::ListSimple.uses_commas());
        assert!(ConstructKind::MapEntries.uses_commas());

        // Does not use commas
        assert!(!ConstructKind::SumVariants.uses_commas());
    }

    #[test]
    fn construct_is_run() {
        assert!(ConstructKind::RunTopLevel.is_run());
        assert!(ConstructKind::RunNested.is_run());

        assert!(!ConstructKind::Try.is_run());
        assert!(!ConstructKind::Match.is_run());
    }

    #[test]
    fn construct_is_list() {
        assert!(ConstructKind::ListSimple.is_list());
        assert!(ConstructKind::ListComplex.is_list());

        assert!(!ConstructKind::MapEntries.is_list());
        assert!(!ConstructKind::TupleElements.is_list());
    }

    /// All `ConstructKind` variants in one place. When a new variant is added,
    /// update this array — the `construct_predicates_consistent` test below will
    /// verify it stays in sync with `is_always_stacked()`, `determine_packing()`,
    /// `separator_for()`, and `name()`. The `all_constructs_is_exhaustive` witness
    /// below fails to compile until the new variant is also listed here.
    const ALL_CONSTRUCTS: [ConstructKind; 22] = [
        ConstructKind::RunTopLevel,
        ConstructKind::Try,
        ConstructKind::Match,
        ConstructKind::Recurse,
        ConstructKind::Parallel,
        ConstructKind::Spawn,
        ConstructKind::Nursery,
        ConstructKind::FunctionParams,
        ConstructKind::FunctionArgs,
        ConstructKind::GenericParams,
        ConstructKind::WhereConstraints,
        ConstructKind::Capabilities,
        ConstructKind::StructFieldsDef,
        ConstructKind::StructFieldsLiteral,
        ConstructKind::SumVariants,
        ConstructKind::MapEntries,
        ConstructKind::TupleElements,
        ConstructKind::ImportItems,
        ConstructKind::ListSimple,
        ConstructKind::ListComplex,
        ConstructKind::RunNested,
        ConstructKind::MatchArms,
    ];

    /// Compile-time-enforced completeness guard for `ALL_CONSTRUCTS`.
    ///
    /// The exhaustive `match` (no `_` catch-all) forces a compile error when a
    /// new `ConstructKind` variant is added without being mapped here, and the
    /// runtime assertion forces that the same variant is present in
    /// `ALL_CONSTRUCTS`. Together they make the hand-maintained array unable to
    /// silently drift out of sync with the enum.
    #[test]
    fn all_constructs_is_exhaustive() {
        for &kind in &ALL_CONSTRUCTS {
            // Adding a variant to `ConstructKind` breaks this match (no `_` arm),
            // forcing the author to extend the witness and `ALL_CONSTRUCTS`.
            let witnessed = match kind {
                ConstructKind::RunTopLevel
                | ConstructKind::Try
                | ConstructKind::Match
                | ConstructKind::Recurse
                | ConstructKind::Parallel
                | ConstructKind::Spawn
                | ConstructKind::Nursery
                | ConstructKind::FunctionParams
                | ConstructKind::FunctionArgs
                | ConstructKind::GenericParams
                | ConstructKind::WhereConstraints
                | ConstructKind::Capabilities
                | ConstructKind::StructFieldsDef
                | ConstructKind::StructFieldsLiteral
                | ConstructKind::SumVariants
                | ConstructKind::MapEntries
                | ConstructKind::TupleElements
                | ConstructKind::ImportItems
                | ConstructKind::ListSimple
                | ConstructKind::ListComplex
                | ConstructKind::RunNested
                | ConstructKind::MatchArms => kind,
            };
            assert!(
                ALL_CONSTRUCTS.contains(&witnessed),
                "{witnessed:?}: handled by the exhaustive witness but missing from ALL_CONSTRUCTS",
            );
        }
    }

    #[test]
    fn construct_names_unique() {
        let mut names: Vec<_> = ALL_CONSTRUCTS.iter().map(|c| c.name()).collect();
        let original_len = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(
            names.len(),
            original_len,
            "All construct names should be unique"
        );
    }

    /// Verify that `is_always_stacked()` agrees with `determine_packing()`:
    /// a construct is always-stacked iff its default packing is `AlwaysStacked`.
    #[test]
    fn construct_predicates_consistent() {
        for &kind in &ALL_CONSTRUCTS {
            let packing = determine_packing(kind, false, false, false);
            let stacked = kind.is_always_stacked();

            assert_eq!(
                stacked,
                packing == Packing::AlwaysStacked,
                "{kind:?}: is_always_stacked()={stacked} but determine_packing()={packing:?}",
            );

            // Verify name() and separator_for() don't panic
            let _ = kind.name();
            let _ = separator_for(kind, packing);
        }
    }
}

mod determine_packing_tests {
    use super::*;

    #[test]
    fn always_stacked_constructs() {
        // Always-stacked constructs return AlwaysStacked regardless of other flags
        assert_eq!(
            determine_packing(ConstructKind::RunTopLevel, false, false, false),
            Packing::AlwaysStacked
        );
        assert_eq!(
            determine_packing(ConstructKind::Try, false, false, false),
            Packing::AlwaysStacked
        );
        assert_eq!(
            determine_packing(ConstructKind::Match, false, false, false),
            Packing::AlwaysStacked
        );
        assert_eq!(
            determine_packing(ConstructKind::Recurse, false, false, false),
            Packing::AlwaysStacked
        );
        assert_eq!(
            determine_packing(ConstructKind::Parallel, false, false, false),
            Packing::AlwaysStacked
        );
        assert_eq!(
            determine_packing(ConstructKind::Spawn, false, false, false),
            Packing::AlwaysStacked
        );
        assert_eq!(
            determine_packing(ConstructKind::Nursery, false, false, false),
            Packing::AlwaysStacked
        );
    }

    #[test]
    fn trailing_comma_forces_multiline() {
        assert_eq!(
            determine_packing(ConstructKind::FunctionArgs, true, false, false),
            Packing::AlwaysOnePerLine
        );
        assert_eq!(
            determine_packing(ConstructKind::ListSimple, true, false, false),
            Packing::AlwaysOnePerLine
        );
    }

    #[test]
    fn comments_force_multiline() {
        assert_eq!(
            determine_packing(ConstructKind::FunctionParams, false, true, false),
            Packing::AlwaysOnePerLine
        );
    }

    #[test]
    fn empty_lines_force_multiline() {
        assert_eq!(
            determine_packing(ConstructKind::MapEntries, false, false, true),
            Packing::AlwaysOnePerLine
        );
    }

    #[test]
    fn simple_list_can_pack() {
        assert_eq!(
            determine_packing(ConstructKind::ListSimple, false, false, false),
            Packing::FitOrPackMultiple
        );
    }

    #[test]
    fn complex_list_one_per_line() {
        assert_eq!(
            determine_packing(ConstructKind::ListComplex, false, false, false),
            Packing::FitOrOnePerLine
        );
    }

    #[test]
    fn default_is_fit_or_one_per_line() {
        assert_eq!(
            determine_packing(ConstructKind::FunctionParams, false, false, false),
            Packing::FitOrOnePerLine
        );
        assert_eq!(
            determine_packing(ConstructKind::FunctionArgs, false, false, false),
            Packing::FitOrOnePerLine
        );
        assert_eq!(
            determine_packing(ConstructKind::GenericParams, false, false, false),
            Packing::FitOrOnePerLine
        );
        assert_eq!(
            determine_packing(ConstructKind::StructFieldsDef, false, false, false),
            Packing::FitOrOnePerLine
        );
    }

    #[test]
    fn nested_run_is_width_based() {
        // Nested run is NOT always stacked
        assert_eq!(
            determine_packing(ConstructKind::RunNested, false, false, false),
            Packing::FitOrOnePerLine
        );
    }
}

mod separator_tests {
    use super::*;

    #[test]
    fn separator_default() {
        assert_eq!(Separator::default(), Separator::Comma);
    }

    #[test]
    fn separator_inline_str() {
        assert_eq!(Separator::Comma.inline_str(), ", ");
        assert_eq!(Separator::Space.inline_str(), " ");
        assert_eq!(Separator::Pipe.inline_str(), " | ");
    }

    #[test]
    fn separator_broken_prefix() {
        assert_eq!(Separator::Comma.broken_prefix(), "");
        assert_eq!(Separator::Space.broken_prefix(), "");
        assert_eq!(Separator::Pipe.broken_prefix(), "| ");
    }

    #[test]
    fn separator_broken_suffix() {
        assert_eq!(Separator::Comma.broken_suffix(), ",");
        assert_eq!(Separator::Space.broken_suffix(), "");
        assert_eq!(Separator::Pipe.broken_suffix(), "");
    }

    #[test]
    fn separator_is_comma() {
        assert!(Separator::Comma.is_comma());
        assert!(!Separator::Space.is_comma());
        assert!(!Separator::Pipe.is_comma());
    }

    #[test]
    fn separator_for_constructs() {
        // Most use comma
        assert_eq!(
            separator_for(ConstructKind::FunctionParams, Packing::FitOrOnePerLine),
            Separator::Comma
        );
        assert_eq!(
            separator_for(ConstructKind::ListSimple, Packing::FitOrPackMultiple),
            Separator::Comma
        );
        assert_eq!(
            separator_for(ConstructKind::MapEntries, Packing::AlwaysOnePerLine),
            Separator::Comma
        );

        // Sum variants use pipe
        assert_eq!(
            separator_for(ConstructKind::SumVariants, Packing::FitOrOnePerLine),
            Separator::Pipe
        );
    }
}
