use std::collections::HashSet;

use super::{entry_for, CANON_CONSUMERS};

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
