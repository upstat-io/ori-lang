use std::collections::HashSet;

use super::{entry_for, CanonPool, PoolAccess, CANON_CONSUMERS};

#[test]
fn canon_consumers_registry_is_nonempty_with_unique_crate_names() {
    assert!(!CANON_CONSUMERS.is_empty());
    let unique: HashSet<&str> = CANON_CONSUMERS.iter().map(|e| e.crate_name).collect();
    assert_eq!(
        unique.len(),
        CANON_CONSUMERS.len(),
        "crate_name entries must be unique"
    );
}

#[test]
fn every_entry_resolves_via_entry_for() {
    for entry in CANON_CONSUMERS {
        let found = entry_for(entry.crate_name);
        assert_eq!(found, Some(entry));
    }
}

#[test]
fn unregistered_crate_resolves_to_none() {
    assert_eq!(entry_for("ori_ir"), None);
}

#[test]
fn every_meaning_consumer_carries_a_nonempty_read_set() {
    for entry in CANON_CONSUMERS {
        if let PoolAccess::MeaningConsumer(pools) = entry.pool_access {
            assert!(
                !pools.is_empty(),
                "{} is a MeaningConsumer with an empty pool read-set",
                entry.crate_name
            );
        }
    }
}

#[test]
fn no_meaning_consumer_read_set_has_duplicate_pools() {
    for entry in CANON_CONSUMERS {
        if let PoolAccess::MeaningConsumer(pools) = entry.pool_access {
            let unique: HashSet<CanonPool> = pools.iter().copied().collect();
            assert_eq!(
                unique.len(),
                pools.len(),
                "{} MeaningConsumer read-set repeats a pool",
                entry.crate_name
            );
        }
    }
}

#[test]
fn interpreter_reads_decision_trees_but_not_the_resolved_type_pool() {
    let eval = entry_for("ori_eval").unwrap_or_else(|| panic!("ori_eval must be registered"));
    match eval.pool_access {
        PoolAccess::MeaningConsumer(pools) => {
            assert!(
                pools.contains(&CanonPool::DecisionTreePool),
                "ori_eval must read the decision-tree pool"
            );
            assert!(
                !pools.contains(&CanonPool::ResolvedTypePool),
                "ori_eval carries no type-checker dependency and must NOT read the resolved type pool"
            );
        }
        other => panic!("ori_eval must be a MeaningConsumer, got {other:?}"),
    }
}

#[test]
fn arc_lowering_reads_all_three_pools() {
    let arc = entry_for("ori_arc").unwrap_or_else(|| panic!("ori_arc must be registered"));
    match arc.pool_access {
        PoolAccess::MeaningConsumer(pools) => {
            for pool in [
                CanonPool::CanExpr,
                CanonPool::DecisionTreePool,
                CanonPool::ResolvedTypePool,
            ] {
                assert!(pools.contains(&pool), "ori_arc must read {pool:?}");
            }
        }
        other => panic!("ori_arc must be a MeaningConsumer, got {other:?}"),
    }
}

#[test]
fn llvm_backend_reads_only_the_resolved_type_pool() {
    let llvm = entry_for("ori_llvm").unwrap_or_else(|| panic!("ori_llvm must be registered"));
    match llvm.pool_access {
        PoolAccess::MeaningConsumer(pools) => {
            assert!(
                pools.contains(&CanonPool::ResolvedTypePool),
                "ori_llvm must read the resolved type pool for layout/codegen"
            );
            assert!(
                !pools.contains(&CanonPool::CanExpr),
                "ori_llvm delegates the CanExpr walk to ARC and must NOT read CanExpr directly"
            );
            assert!(
                !pools.contains(&CanonPool::DecisionTreePool),
                "ori_llvm delegates the decision-tree walk to ARC and must NOT read it directly"
            );
        }
        other => panic!("ori_llvm must be a MeaningConsumer, got {other:?}"),
    }
}

#[test]
fn id_only_importers_are_not_meaning_consumers() {
    for crate_name in ["ori_types", "ori_patterns"] {
        let entry =
            entry_for(crate_name).unwrap_or_else(|| panic!("{crate_name} must be registered"));
        assert_eq!(
            entry.pool_access,
            PoolAccess::IdOnlyImporter,
            "{crate_name} imports a canon handle as a bare id and must be IdOnlyImporter"
        );
    }
}

#[test]
fn orchestration_facades_pass_handles_without_walking_pools() {
    for crate_name in ["ori_compiler", "oric"] {
        let entry =
            entry_for(crate_name).unwrap_or_else(|| panic!("{crate_name} must be registered"));
        assert_eq!(
            entry.pool_access,
            PoolAccess::OrchestrationIdOnly,
            "{crate_name} wires the pipeline without tree-walking a pool and must be OrchestrationIdOnly"
        );
    }
}
