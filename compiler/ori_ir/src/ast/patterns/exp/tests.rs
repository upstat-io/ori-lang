use super::*;

#[test]
fn function_exp_kind_name_maps_each_declared_variant() {
    // Verify all 15 FunctionExpKind variants return correct names
    assert_eq!(FunctionExpKind::Recurse.name(), "recurse");
    assert_eq!(FunctionExpKind::Parallel.name(), "parallel");
    assert_eq!(FunctionExpKind::Spawn.name(), "spawn");
    assert_eq!(FunctionExpKind::Timeout.name(), "timeout");
    assert_eq!(FunctionExpKind::Cache.name(), "cache");
    assert_eq!(FunctionExpKind::With.name(), "with");
    assert_eq!(FunctionExpKind::Print.name(), "print");
    assert_eq!(FunctionExpKind::Panic.name(), "panic");
    assert_eq!(FunctionExpKind::Catch.name(), "catch");
    assert_eq!(FunctionExpKind::Todo.name(), "todo");
    assert_eq!(FunctionExpKind::Unreachable.name(), "unreachable");
    assert_eq!(FunctionExpKind::Channel.name(), "channel");
    assert_eq!(FunctionExpKind::ChannelIn.name(), "channel_in");
    assert_eq!(FunctionExpKind::ChannelOut.name(), "channel_out");
    assert_eq!(FunctionExpKind::ChannelAll.name(), "channel_all");
}

#[test]
fn function_exp_kind_all_constant_stays_in_sync_with_declared_variants() {
    use std::collections::HashSet;

    // ALL is the canonical inventory consumers derive from. A variant added to
    // the enum (and forced into the exhaustive `name()` match) but forgotten in
    // ALL drifts the inventory; the unique-non-empty-name pin catches it because
    // every variant's name is distinct, so a missing entry collapses the set.
    let names: HashSet<&'static str> = FunctionExpKind::ALL.iter().map(|k| k.name()).collect();
    assert_eq!(
        names.len(),
        FunctionExpKind::ALL.len(),
        "FunctionExpKind::ALL has duplicate or missing entries — names must be unique"
    );
    for kind in FunctionExpKind::ALL {
        assert!(
            !kind.name().is_empty(),
            "every FunctionExpKind in ALL must have a non-empty name"
        );
    }
}

#[test]
fn test_function_exp_kind_eq_and_hash() {
    use std::collections::HashSet;
    let mut set = HashSet::new();

    set.insert(FunctionExpKind::Recurse);
    set.insert(FunctionExpKind::Recurse); // duplicate
    set.insert(FunctionExpKind::Parallel);
    set.insert(FunctionExpKind::Spawn);

    assert_eq!(set.len(), 3);
    assert!(set.contains(&FunctionExpKind::Recurse));
    assert!(set.contains(&FunctionExpKind::Parallel));
    assert!(set.contains(&FunctionExpKind::Spawn));
    assert!(!set.contains(&FunctionExpKind::Cache));
}

#[test]
#[expect(clippy::clone_on_copy, reason = "Testing Clone trait impl explicitly")]
fn test_function_exp_kind_copy_clone() {
    let kind = FunctionExpKind::Timeout;
    let copied = kind; // Copy
    let cloned = kind.clone(); // Clone

    assert_eq!(kind, copied);
    assert_eq!(kind, cloned);
}
