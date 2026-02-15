use super::*;

#[test]
fn test_fusion_hints_constructors() {
    let two = FusionHints::two_pattern();
    assert_eq!(two.allocations_avoided, 1);
    assert!(two.eliminates_intermediate_lists);

    let three = FusionHints::three_pattern();
    assert_eq!(three.allocations_avoided, 2);
}

#[test]
fn test_fused_pattern_names() {
    let map_filter = FusedPattern::MapFilter {
        input: ExprId::new(0),
        map_fn: ExprId::new(1),
        filter_fn: ExprId::new(2),
    };
    assert_eq!(map_filter.name(), "map-filter");

    let map_fold = FusedPattern::MapFold {
        input: ExprId::new(0),
        map_fn: ExprId::new(1),
        init: ExprId::new(2),
        fold_fn: ExprId::new(3),
    };
    assert_eq!(map_fold.name(), "map-fold");
}
