use super::*;

#[test]
fn test_target_subcommand_from_str() {
    assert_eq!(
        TargetSubcommand::parse("list"),
        Some(TargetSubcommand::List)
    );
    assert_eq!(TargetSubcommand::parse("add"), Some(TargetSubcommand::Add));
    assert_eq!(
        TargetSubcommand::parse("remove"),
        Some(TargetSubcommand::Remove)
    );
    assert_eq!(TargetSubcommand::parse("invalid"), None);
    assert_eq!(TargetSubcommand::parse(""), None);
}

#[test]
fn test_sysroots_dir() {
    let dir = sysroots_dir();
    assert!(dir.to_string_lossy().contains(".ori"));
    assert!(dir.to_string_lossy().contains("sysroots"));
}

#[test]
fn test_sysroot_path() {
    let path = sysroot_path("x86_64-unknown-linux-gnu");
    assert!(path.to_string_lossy().contains("x86_64-unknown-linux-gnu"));
}

#[test]
fn test_is_target_installed_nonexistent() {
    // A random target name that definitely doesn't exist
    // Test the underlying logic since is_target_installed is feature-gated
    let path = sysroot_path("nonexistent-fake-target-12345");
    assert!(!path.exists());
}

#[test]
fn test_target_subcommand_variants() {
    // Verify all variants can be compared
    assert_ne!(TargetSubcommand::List, TargetSubcommand::Add);
    assert_ne!(TargetSubcommand::Add, TargetSubcommand::Remove);
    assert_ne!(TargetSubcommand::Remove, TargetSubcommand::List);
}

#[test]
fn test_target_subcommand_debug() {
    // Verify Debug trait works
    let list = TargetSubcommand::List;
    let debug_str = format!("{list:?}");
    assert_eq!(debug_str, "List");
}

#[test]
fn test_target_subcommand_clone() {
    let original = TargetSubcommand::Add;
    let cloned = original;
    assert_eq!(original, cloned);
}
