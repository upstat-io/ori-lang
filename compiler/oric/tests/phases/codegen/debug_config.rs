//! Debug configuration tests.
//!
//! Tests for `DebugLevel`, `DebugInfoConfig`, `DebugFormat`, and `DebugInfoError`.
//! These validate the configuration layer of debug info generation.
//!
//! Note: Tests for private methods like `to_emission_kind` remain inline in the source.

#[cfg(feature = "llvm")]
mod tests {
    use ori_llvm::aot::debug::{DebugFormat, DebugInfoConfig, DebugInfoError, DebugLevel};

    // -- DebugLevel tests --

    #[test]
    fn test_debug_level_is_enabled() {
        assert!(!DebugLevel::None.is_enabled());
        assert!(DebugLevel::LineTablesOnly.is_enabled());
        assert!(DebugLevel::Full.is_enabled());
    }

    #[test]
    fn test_debug_level_display() {
        assert_eq!(format!("{}", DebugLevel::None), "none");
        assert_eq!(format!("{}", DebugLevel::LineTablesOnly), "line-tables");
        assert_eq!(format!("{}", DebugLevel::Full), "full");
    }

    // -- DebugInfoConfig tests --

    #[test]
    fn test_debug_info_config_default() {
        let config = DebugInfoConfig::default();
        assert_eq!(config.level, DebugLevel::None);
        assert!(!config.optimized);
        assert_eq!(config.dwarf_version, 4);
    }

    #[test]
    fn test_debug_info_config_development() {
        let config = DebugInfoConfig::development();
        assert_eq!(config.level, DebugLevel::Full);
        assert!(!config.optimized);
    }

    #[test]
    fn test_debug_info_config_release() {
        let config = DebugInfoConfig::release_with_debug();
        assert_eq!(config.level, DebugLevel::LineTablesOnly);
        assert!(config.optimized);
    }

    #[test]
    fn test_debug_info_config_builder() {
        let config = DebugInfoConfig::new(DebugLevel::Full)
            .with_optimized(true)
            .with_dwarf_version(5);

        assert_eq!(config.level, DebugLevel::Full);
        assert!(config.optimized);
        assert_eq!(config.dwarf_version, 5);
    }

    #[test]
    fn test_debug_config_for_target() {
        let config = DebugInfoConfig::for_target(DebugLevel::Full, "x86_64-pc-windows-msvc");
        assert_eq!(config.level, DebugLevel::Full);
        assert!(config.format.is_codeview());

        let config = DebugInfoConfig::for_target(DebugLevel::Full, "x86_64-unknown-linux-gnu");
        assert!(config.format.is_dwarf());
    }

    #[test]
    fn test_debug_config_needs_dsym() {
        let config = DebugInfoConfig::new(DebugLevel::Full).with_split_debug_info(true);

        assert!(config.needs_dsym("aarch64-apple-darwin"));
        assert!(config.needs_dsym("x86_64-apple-darwin"));
        assert!(!config.needs_dsym("x86_64-unknown-linux-gnu"));
        assert!(!config.needs_dsym("x86_64-pc-windows-msvc"));

        // Without split debug, no dSYM needed
        let config = DebugInfoConfig::new(DebugLevel::Full);
        assert!(!config.needs_dsym("aarch64-apple-darwin"));
    }

    #[test]
    fn test_debug_config_needs_pdb() {
        let config = DebugInfoConfig::for_target(DebugLevel::Full, "x86_64-pc-windows-msvc");

        assert!(config.needs_pdb("x86_64-pc-windows-msvc"));
        assert!(!config.needs_pdb("x86_64-pc-windows-gnu"));
        assert!(!config.needs_pdb("x86_64-unknown-linux-gnu"));
    }

    #[test]
    fn test_debug_config_with_profiling() {
        let config = DebugInfoConfig::new(DebugLevel::Full).with_profiling(true);
        assert!(config.debug_info_for_profiling);
    }

    #[test]
    fn test_debug_config_release_with_debug() {
        let config = DebugInfoConfig::release_with_debug();

        assert_eq!(config.level, DebugLevel::LineTablesOnly);
        assert!(config.optimized);
        assert!(config.split_debug_info);
    }

    #[test]
    fn test_debug_config_with_format() {
        let config = DebugInfoConfig::new(DebugLevel::Full).with_format(DebugFormat::CodeView);
        assert_eq!(config.format, DebugFormat::CodeView);

        let config = DebugInfoConfig::new(DebugLevel::Full).with_format(DebugFormat::Dwarf);
        assert_eq!(config.format, DebugFormat::Dwarf);
    }

    #[test]
    fn test_debug_config_with_split_debug_info() {
        let config = DebugInfoConfig::new(DebugLevel::Full).with_split_debug_info(true);
        assert!(config.split_debug_info);

        let config = DebugInfoConfig::new(DebugLevel::Full).with_split_debug_info(false);
        assert!(!config.split_debug_info);
    }

    #[test]
    fn test_debug_config_with_profiling_enabled() {
        let config = DebugInfoConfig::new(DebugLevel::Full)
            .with_profiling(true)
            .with_optimized(true)
            .with_split_debug_info(true);

        assert!(config.debug_info_for_profiling);
        assert!(config.optimized);
        assert!(config.split_debug_info);
    }

    // -- DebugFormat tests --

    #[test]
    fn test_debug_format_for_target_linux() {
        let format = DebugFormat::for_target("x86_64-unknown-linux-gnu");
        assert!(format.is_dwarf());
        assert!(!format.is_codeview());
    }

    #[test]
    fn test_debug_format_for_target_macos() {
        let format = DebugFormat::for_target("aarch64-apple-darwin");
        assert!(format.is_dwarf());
    }

    #[test]
    fn test_debug_format_for_target_windows_msvc() {
        let format = DebugFormat::for_target("x86_64-pc-windows-msvc");
        assert!(format.is_codeview());
        assert!(!format.is_dwarf());
    }

    #[test]
    fn test_debug_format_for_target_windows_gnu() {
        // MinGW uses DWARF, not CodeView
        let format = DebugFormat::for_target("x86_64-pc-windows-gnu");
        assert!(format.is_dwarf());
    }

    #[test]
    fn test_debug_format_for_target_wasm() {
        let format = DebugFormat::for_target("wasm32-unknown-unknown");
        assert!(format.is_dwarf());
    }

    #[test]
    fn test_debug_format_display() {
        assert_eq!(format!("{}", DebugFormat::Dwarf), "DWARF");
        assert_eq!(format!("{}", DebugFormat::CodeView), "CodeView");
    }

    // -- DebugInfoError tests --

    #[test]
    fn test_debug_info_error_display() {
        let err = DebugInfoError::BasicType {
            name: "int".to_string(),
            message: "encoding error".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "failed to create debug type 'int': encoding error"
        );

        let err = DebugInfoError::Disabled;
        assert_eq!(err.to_string(), "debug info is disabled");
    }

    #[test]
    fn test_debug_info_error_all_variants() {
        let err = DebugInfoError::BasicType {
            name: "int".to_string(),
            message: "invalid size".to_string(),
        };
        assert!(err.to_string().contains("int"));
        assert!(err.to_string().contains("invalid size"));
        assert!(err.to_string().contains("failed to create"));

        let err = DebugInfoError::Disabled;
        assert!(err.to_string().contains("disabled"));
    }

    #[test]
    fn test_debug_info_error_std_error_impl() {
        use std::error::Error;

        let err = DebugInfoError::BasicType {
            name: "test".to_string(),
            message: "failed".to_string(),
        };

        // Test that it implements std::error::Error
        let _: &dyn Error = &err;
        assert!(err.source().is_none());
    }
}
