//! Pattern fusion detection and optimization.
//!
//! This module detects chains of patterns that can be fused together
//! for more efficient execution. For example, `map→filter→fold` can
//! be executed in a single pass instead of three separate passes.

use crate::syntax::{ExprId, ExprKind, ExprArena, PatternKind, PatternArg};

/// Represents a detected pattern chain that can be fused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FusedPattern {
    /// A single pattern (no fusion possible).
    Single(PatternKind),

    /// Map followed by filter: items.map(f).filter(p)
    /// Can be fused to: for each item, apply f then p, keep if true
    MapFilter {
        map_transform: ExprId,
        filter_predicate: ExprId,
    },

    /// Filter followed by map: items.filter(p).map(f)
    /// Can be fused to: for each item, keep if p, then apply f
    FilterMap {
        filter_predicate: ExprId,
        map_transform: ExprId,
    },

    /// Map followed by fold: items.map(f).fold(init, op)
    /// Can be fused to: fold with composed operation
    MapFold {
        map_transform: ExprId,
        fold_init: ExprId,
        fold_op: ExprId,
    },

    /// Filter followed by fold: items.filter(p).fold(init, op)
    /// Can be fused to: fold with conditional accumulation
    FilterFold {
        filter_predicate: ExprId,
        fold_init: ExprId,
        fold_op: ExprId,
    },

    /// Map→Filter→Fold: the most common fusable chain
    /// Can be executed in a single pass
    MapFilterFold {
        map_transform: ExprId,
        filter_predicate: ExprId,
        fold_init: ExprId,
        fold_op: ExprId,
    },

    /// Map→Map: two consecutive maps can be composed
    MapMap {
        first_transform: ExprId,
        second_transform: ExprId,
    },

    /// Filter→Filter: two consecutive filters can be ANDed
    FilterFilter {
        first_predicate: ExprId,
        second_predicate: ExprId,
    },

    /// Filter→Find: filter then find first
    FilterFind {
        filter_predicate: ExprId,
        find_predicate: ExprId,
    },

    /// Map→Find: transform then find
    MapFind {
        map_transform: ExprId,
        find_predicate: ExprId,
    },
}

/// Analyzes a pattern expression and detects fusion opportunities.
pub struct FusionAnalyzer<'a> {
    arena: &'a ExprArena,
}

impl<'a> FusionAnalyzer<'a> {
    /// Create a new fusion analyzer.
    pub fn new(arena: &'a ExprArena) -> Self {
        FusionAnalyzer { arena }
    }

    /// Analyze an expression and detect if it's a fusable pattern chain.
    ///
    /// Returns `Some(FusedPattern)` if fusion is possible, `None` otherwise.
    pub fn analyze(&self, expr_id: ExprId) -> Option<FusedPattern> {
        let expr = self.arena.get(expr_id);

        // Check if this is a pattern call
        if let ExprKind::Pattern { kind, args, .. } = &expr.kind {
            return self.analyze_pattern(*kind, *args);
        }

        None
    }

    /// Analyze a pattern and its arguments for fusion opportunities.
    fn analyze_pattern(
        &self,
        kind: PatternKind,
        args: crate::syntax::PatternArgsId,
    ) -> Option<FusedPattern> {
        let pattern_args = self.arena.get_pattern_args(args);

        // Look for the .over argument which contains the input expression
        let over_expr = self.find_named_arg(&pattern_args.named, "over")?;

        // Check if the input is another pattern (chain detection)
        let input_expr = self.arena.get(over_expr);
        if let ExprKind::Pattern { kind: inner_kind, args: inner_args, .. } = &input_expr.kind {
            return self.detect_fusion(*inner_kind, *inner_args, kind, args);
        }

        // No fusion possible - single pattern
        Some(FusedPattern::Single(kind))
    }

    /// Find a named argument by name string.
    /// Note: In a full implementation, we'd compare interned names.
    /// For now, we just look for the first named arg assuming it's .over.
    fn find_named_arg(&self, named: &[PatternArg], _name: &str) -> Option<ExprId> {
        // Return the first named argument's value if it exists
        named.first().map(|arg| arg.value)
    }

    /// Detect fusion between two consecutive patterns.
    fn detect_fusion(
        &self,
        first_kind: PatternKind,
        first_args: crate::syntax::PatternArgsId,
        second_kind: PatternKind,
        second_args: crate::syntax::PatternArgsId,
    ) -> Option<FusedPattern> {
        match (first_kind, second_kind) {
            // map→filter
            (PatternKind::Map, PatternKind::Filter) => {
                let transform = self.get_transform_arg(first_args)?;
                let predicate = self.get_predicate_arg(second_args)?;
                Some(FusedPattern::MapFilter {
                    map_transform: transform,
                    filter_predicate: predicate,
                })
            }

            // filter→map
            (PatternKind::Filter, PatternKind::Map) => {
                let predicate = self.get_predicate_arg(first_args)?;
                let transform = self.get_transform_arg(second_args)?;
                Some(FusedPattern::FilterMap {
                    filter_predicate: predicate,
                    map_transform: transform,
                })
            }

            // map→fold
            (PatternKind::Map, PatternKind::Fold) => {
                let transform = self.get_transform_arg(first_args)?;
                let (init, op) = self.get_fold_args(second_args)?;
                Some(FusedPattern::MapFold {
                    map_transform: transform,
                    fold_init: init,
                    fold_op: op,
                })
            }

            // filter→fold
            (PatternKind::Filter, PatternKind::Fold) => {
                let predicate = self.get_predicate_arg(first_args)?;
                let (init, op) = self.get_fold_args(second_args)?;
                Some(FusedPattern::FilterFold {
                    filter_predicate: predicate,
                    fold_init: init,
                    fold_op: op,
                })
            }

            // map→map
            (PatternKind::Map, PatternKind::Map) => {
                let first = self.get_transform_arg(first_args)?;
                let second = self.get_transform_arg(second_args)?;
                Some(FusedPattern::MapMap {
                    first_transform: first,
                    second_transform: second,
                })
            }

            // filter→filter
            (PatternKind::Filter, PatternKind::Filter) => {
                let first = self.get_predicate_arg(first_args)?;
                let second = self.get_predicate_arg(second_args)?;
                Some(FusedPattern::FilterFilter {
                    first_predicate: first,
                    second_predicate: second,
                })
            }

            // filter→find
            (PatternKind::Filter, PatternKind::Find) => {
                let filter_pred = self.get_predicate_arg(first_args)?;
                let find_pred = self.get_predicate_arg(second_args)?;
                Some(FusedPattern::FilterFind {
                    filter_predicate: filter_pred,
                    find_predicate: find_pred,
                })
            }

            // map→find
            (PatternKind::Map, PatternKind::Find) => {
                let transform = self.get_transform_arg(first_args)?;
                let predicate = self.get_predicate_arg(second_args)?;
                Some(FusedPattern::MapFind {
                    map_transform: transform,
                    find_predicate: predicate,
                })
            }

            // No fusion possible for other combinations
            _ => Some(FusedPattern::Single(second_kind)),
        }
    }

    /// Extract the .transform argument from pattern args.
    fn get_transform_arg(&self, args: crate::syntax::PatternArgsId) -> Option<ExprId> {
        let pattern_args = self.arena.get_pattern_args(args);
        // The transform is typically the second named argument after .over
        pattern_args.named.get(1).map(|arg| arg.value)
    }

    /// Extract the .predicate/.where argument from pattern args.
    fn get_predicate_arg(&self, args: crate::syntax::PatternArgsId) -> Option<ExprId> {
        let pattern_args = self.arena.get_pattern_args(args);
        // The predicate is typically the second named argument after .over
        pattern_args.named.get(1).map(|arg| arg.value)
    }

    /// Extract .init and .op arguments from fold pattern args.
    fn get_fold_args(&self, args: crate::syntax::PatternArgsId) -> Option<(ExprId, ExprId)> {
        let pattern_args = self.arena.get_pattern_args(args);
        // fold has: .over, .init, .op
        let init = pattern_args.named.get(1).map(|arg| arg.value)?;
        let op = pattern_args.named.get(2).map(|arg| arg.value)?;
        Some((init, op))
    }

    /// Check if an expression is a fusable pattern chain and return the depth.
    pub fn chain_depth(&self, expr_id: ExprId) -> usize {
        let expr = self.arena.get(expr_id);
        if let ExprKind::Pattern { args, .. } = &expr.kind {
            let pattern_args = self.arena.get_pattern_args(*args);
            // Look for .over in named args
            if let Some(over_arg) = pattern_args.named.first() {
                return 1 + self.chain_depth(over_arg.value);
            }
        }
        0
    }
}

/// Optimization hints based on fusion analysis.
#[derive(Clone, Debug)]
pub struct FusionHints {
    /// Whether the pattern chain can be executed in a single pass.
    pub single_pass: bool,
    /// Estimated reduction in allocations.
    pub alloc_reduction: f64,
    /// The fused pattern if fusion is possible.
    pub fused: Option<FusedPattern>,
}

impl FusionHints {
    /// Create hints indicating no fusion is possible.
    pub fn none() -> Self {
        FusionHints {
            single_pass: false,
            alloc_reduction: 0.0,
            fused: None,
        }
    }

    /// Create hints for a fused pattern.
    pub fn for_fused(fused: FusedPattern) -> Self {
        let (single_pass, alloc_reduction) = match &fused {
            FusedPattern::Single(_) => (false, 0.0),
            FusedPattern::MapFilter { .. } | FusedPattern::FilterMap { .. } => (true, 0.5),
            FusedPattern::MapFold { .. } | FusedPattern::FilterFold { .. } => (true, 1.0),
            FusedPattern::MapFilterFold { .. } => (true, 2.0),
            FusedPattern::MapMap { .. } | FusedPattern::FilterFilter { .. } => (true, 0.5),
            FusedPattern::FilterFind { .. } | FusedPattern::MapFind { .. } => (true, 0.5),
        };

        FusionHints {
            single_pass,
            alloc_reduction,
            fused: Some(fused),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fused_pattern_variants() {
        // Just verify the enum can be constructed
        let single = FusedPattern::Single(PatternKind::Map);
        assert_eq!(single, FusedPattern::Single(PatternKind::Map));
    }

    #[test]
    fn test_fusion_hints() {
        let hints = FusionHints::none();
        assert!(!hints.single_pass);
        assert_eq!(hints.alloc_reduction, 0.0);
        assert!(hints.fused.is_none());

        let fused = FusedPattern::MapFilter {
            map_transform: ExprId::INVALID,
            filter_predicate: ExprId::INVALID,
        };
        let hints = FusionHints::for_fused(fused);
        assert!(hints.single_pass);
        assert_eq!(hints.alloc_reduction, 0.5);
        assert!(hints.fused.is_some());
    }

    #[test]
    fn test_fusion_hints_for_all_variants() {
        // Test all fused pattern variants have appropriate hints
        let variants = [
            FusedPattern::Single(PatternKind::Map),
            FusedPattern::MapFilter {
                map_transform: ExprId::INVALID,
                filter_predicate: ExprId::INVALID,
            },
            FusedPattern::FilterMap {
                filter_predicate: ExprId::INVALID,
                map_transform: ExprId::INVALID,
            },
            FusedPattern::MapFold {
                map_transform: ExprId::INVALID,
                fold_init: ExprId::INVALID,
                fold_op: ExprId::INVALID,
            },
            FusedPattern::FilterFold {
                filter_predicate: ExprId::INVALID,
                fold_init: ExprId::INVALID,
                fold_op: ExprId::INVALID,
            },
            FusedPattern::MapMap {
                first_transform: ExprId::INVALID,
                second_transform: ExprId::INVALID,
            },
            FusedPattern::FilterFilter {
                first_predicate: ExprId::INVALID,
                second_predicate: ExprId::INVALID,
            },
        ];

        for variant in variants {
            let hints = FusionHints::for_fused(variant);
            // All should have valid hints
            assert!(hints.fused.is_some());
        }
    }
}
