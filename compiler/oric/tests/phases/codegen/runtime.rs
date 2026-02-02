//! Tests for runtime library configuration (`ori_llvm::aot::runtime`).
//!
//! These tests verify:
//! - RuntimeConfig construction and defaults
//! - Static/dynamic linking configuration
//! - Link input configuration
//! - Platform-specific library naming
//! - RuntimeNotFound error display

use ori_llvm::aot::{LibraryKind, LinkInput, RuntimeConfig, RuntimeNotFound};
use std::path::PathBuf;

#[test]
fn test_runtime_config_new() {
    let config = RuntimeConfig::new(PathBuf::from("/usr/lib"));
    assert_eq!(config.library_path, PathBuf::from("/usr/lib"));
    assert!(config.static_link);
}

#[test]
fn test_runtime_config_static_linking() {
    let config = RuntimeConfig::new(PathBuf::from("/usr/lib")).static_linking(false);
    assert!(!config.static_link);
    assert_eq!(config.library_kind(), LibraryKind::Dynamic);
}

#[test]
fn test_configure_link() {
    let config = RuntimeConfig::new(PathBuf::from("/opt/ori/lib"));
    let mut input = LinkInput::default();

    config.configure_link(&mut input);

    assert!(input.library_paths.contains(&PathBuf::from("/opt/ori/lib")));
    assert!(input.libraries.iter().any(|l| l.name == "ori_rt"));
}

#[test]
fn test_lib_name_unix() {
    // On Unix systems, should be libori_rt.a
    #[cfg(unix)]
    assert_eq!(RuntimeConfig::lib_name(), "libori_rt.a");
}

#[test]
fn test_runtime_not_found_display() {
    let err = RuntimeNotFound {
        searched_paths: vec![PathBuf::from("/path/1"), PathBuf::from("/path/2")],
    };

    let msg = err.to_string();
    assert!(msg.contains(RuntimeConfig::lib_name()));
    assert!(msg.contains("/path/1"));
    assert!(msg.contains("/path/2"));
    assert!(msg.contains("cargo build -p ori_rt"));
    assert!(msg.contains("--runtime-path"));
    // Should NOT mention env vars anymore
    assert!(!msg.contains("ORI_RT_PATH"));
    assert!(!msg.contains("ORI_LIB_DIR"));
}
