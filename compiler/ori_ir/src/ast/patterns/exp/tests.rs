use super::*;

#[test]
fn test_function_exp_kind_name_all_variants() {
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
