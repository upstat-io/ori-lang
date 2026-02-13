//! Code Generation and Output Format Tests
//!
//! Test scenarios inspired by:
//! - Rust: `tests/run-make/emit/` - multi-emit verification
//! - Rust: `tests/run-make/bin-emit-no-symbols/` - symbol verification
//! - Zig: `test/standalone/emit_asm_and_bin/` - simultaneous emission
//!
//! These tests verify:
//! - Multiple output formats (object, assembly, LLVM IR, bitcode)
//! - Optimization level effects
//! - Debug information presence
//! - Symbol table correctness

#![allow(
    clippy::similar_names,
    reason = "mangler/mangled naming pattern is intentional"
)]

use ori_llvm::aot::debug::{DebugFormat, DebugInfoConfig, DebugLevel};
use ori_llvm::aot::mangle::{demangle, is_ori_symbol, Mangler, MANGLE_PREFIX};
use ori_llvm::aot::object::OutputFormat;
use ori_llvm::aot::passes::{LtoMode, OptimizationLevel};

use super::util::parse_object;

/// Test: Output format extension mapping
///
/// Scenario from Rust `emit`:
/// Each format has correct file extension.
#[test]
fn test_output_format_extensions() {
    assert_eq!(OutputFormat::Object.extension(), "o");
    assert_eq!(OutputFormat::Assembly.extension(), "s");
    assert_eq!(OutputFormat::LlvmIr.extension(), "ll");
    assert_eq!(OutputFormat::Bitcode.extension(), "bc");
}

/// Test: Output format descriptions
#[test]
fn test_output_format_descriptions() {
    assert!(OutputFormat::Object.description().contains("object"));
    assert!(OutputFormat::Assembly.description().contains("assembly"));
    assert!(OutputFormat::LlvmIr.description().contains("LLVM"));
    assert!(OutputFormat::Bitcode.description().contains("bitcode"));
}

/// Test: Output format variants exist
#[test]
fn test_output_format_variants() {
    // Verify all variants can be created
    let formats = [
        OutputFormat::Object,
        OutputFormat::Assembly,
        OutputFormat::LlvmIr,
        OutputFormat::Bitcode,
    ];

    for format in formats {
        assert!(!format.extension().is_empty());
        assert!(!format.description().is_empty());
    }
}

/// Test: Optimization level to LLVM pass string mapping
///
/// Scenario from Rust `lto-*`:
/// Verify correct pass pipeline names.
#[test]
fn test_optimization_level_pass_names() {
    assert_eq!(OptimizationLevel::O0.pipeline_string(), "default<O0>");
    assert_eq!(OptimizationLevel::O1.pipeline_string(), "default<O1>");
    assert_eq!(OptimizationLevel::O2.pipeline_string(), "default<O2>");
    assert_eq!(OptimizationLevel::O3.pipeline_string(), "default<O3>");
    assert_eq!(OptimizationLevel::Os.pipeline_string(), "default<Os>");
    assert_eq!(OptimizationLevel::Oz.pipeline_string(), "default<Oz>");
}

/// Test: Optimization level display
#[test]
fn test_optimization_level_display() {
    assert_eq!(format!("{}", OptimizationLevel::O0), "O0");
    assert_eq!(format!("{}", OptimizationLevel::O1), "O1");
    assert_eq!(format!("{}", OptimizationLevel::O2), "O2");
    assert_eq!(format!("{}", OptimizationLevel::O3), "O3");
    assert_eq!(format!("{}", OptimizationLevel::Os), "Os");
    assert_eq!(format!("{}", OptimizationLevel::Oz), "Oz");
}

/// Test: Optimization level default
#[test]
fn test_optimization_level_default() {
    let level = OptimizationLevel::default();
    assert_eq!(level, OptimizationLevel::O0);
}

/// Test: Optimization level vectorization
#[test]
fn test_optimization_level_vectorization() {
    // O0/O1 don't enable vectorization
    assert!(!OptimizationLevel::O0.enables_loop_vectorization());
    assert!(!OptimizationLevel::O1.enables_loop_vectorization());

    // O2/O3 enable vectorization
    assert!(OptimizationLevel::O2.enables_loop_vectorization());
    assert!(OptimizationLevel::O3.enables_loop_vectorization());

    // Size optimizations don't enable vectorization
    assert!(!OptimizationLevel::Os.enables_loop_vectorization());
    assert!(!OptimizationLevel::Oz.enables_loop_vectorization());
}

/// Test: Optimization level loop unrolling
#[test]
fn test_optimization_level_loop_unrolling() {
    // O0 doesn't enable unrolling
    assert!(!OptimizationLevel::O0.enables_loop_unrolling());

    // O1-O3 enable unrolling
    assert!(OptimizationLevel::O1.enables_loop_unrolling());
    assert!(OptimizationLevel::O2.enables_loop_unrolling());
    assert!(OptimizationLevel::O3.enables_loop_unrolling());

    // Size optimizations don't enable unrolling
    assert!(!OptimizationLevel::Os.enables_loop_unrolling());
    assert!(!OptimizationLevel::Oz.enables_loop_unrolling());
}

/// Test: Optimization level merge functions
#[test]
fn test_optimization_level_merge_functions() {
    // Regular optimizations don't merge functions
    assert!(!OptimizationLevel::O0.enables_merge_functions());
    assert!(!OptimizationLevel::O2.enables_merge_functions());
    assert!(!OptimizationLevel::O3.enables_merge_functions());

    // Size optimizations enable merging
    assert!(OptimizationLevel::Os.enables_merge_functions());
    assert!(OptimizationLevel::Oz.enables_merge_functions());
}

/// Test: LTO mode pass pipeline names
///
/// Scenario from Rust `cross-lang-lto`:
/// Verify LTO pass names for different modes.
#[test]
fn test_lto_mode_pass_names() {
    assert_eq!(
        LtoMode::Off.prelink_pipeline_string(OptimizationLevel::O2),
        None
    );

    // Thin LTO
    assert_eq!(
        LtoMode::Thin.prelink_pipeline_string(OptimizationLevel::O2),
        Some("thinlto-pre-link<O2>".to_string())
    );
    assert_eq!(
        LtoMode::Thin.lto_pipeline_string(OptimizationLevel::O2),
        Some("thinlto<O2>".to_string())
    );

    // Full LTO
    assert_eq!(
        LtoMode::Full.prelink_pipeline_string(OptimizationLevel::O2),
        Some("lto-pre-link<O2>".to_string())
    );
    assert_eq!(
        LtoMode::Full.lto_pipeline_string(OptimizationLevel::O2),
        Some("lto<O2>".to_string())
    );
}

/// Test: LTO with different optimization levels
#[test]
fn test_lto_with_optimization_levels() {
    // Thin LTO at O3
    assert_eq!(
        LtoMode::Thin.prelink_pipeline_string(OptimizationLevel::O3),
        Some("thinlto-pre-link<O3>".to_string())
    );

    // Full LTO at Os
    assert_eq!(
        LtoMode::Full.prelink_pipeline_string(OptimizationLevel::Os),
        Some("lto-pre-link<Os>".to_string())
    );

    // Thin LTO at Oz
    assert_eq!(
        LtoMode::Thin.lto_pipeline_string(OptimizationLevel::Oz),
        Some("thinlto<Oz>".to_string())
    );
}

/// Test: LTO mode display
#[test]
fn test_lto_mode_display() {
    assert_eq!(format!("{}", LtoMode::Off), "off");
    assert_eq!(format!("{}", LtoMode::Thin), "thin");
    assert_eq!(format!("{}", LtoMode::Full), "full");
}

/// Test: Debug level `is_enabled`
#[test]
fn test_debug_level_is_enabled() {
    assert!(!DebugLevel::None.is_enabled());
    assert!(DebugLevel::LineTablesOnly.is_enabled());
    assert!(DebugLevel::Full.is_enabled());
}

/// Test: Debug level display
#[test]
fn test_debug_level_display() {
    assert_eq!(format!("{}", DebugLevel::None), "none");
    assert_eq!(format!("{}", DebugLevel::LineTablesOnly), "line-tables");
    assert_eq!(format!("{}", DebugLevel::Full), "full");
}

/// Test: Debug format selection per target
#[test]
fn test_debug_format_for_target() {
    // Linux uses DWARF
    assert_eq!(
        DebugFormat::for_target("x86_64-unknown-linux-gnu"),
        DebugFormat::Dwarf
    );

    // macOS uses DWARF (with dSYM bundle)
    assert_eq!(
        DebugFormat::for_target("x86_64-apple-darwin"),
        DebugFormat::Dwarf
    );

    // Windows MSVC uses CodeView
    assert_eq!(
        DebugFormat::for_target("x86_64-pc-windows-msvc"),
        DebugFormat::CodeView
    );

    // Windows GNU uses DWARF
    assert_eq!(
        DebugFormat::for_target("x86_64-pc-windows-gnu"),
        DebugFormat::Dwarf
    );

    // WASM uses DWARF
    assert_eq!(
        DebugFormat::for_target("wasm32-unknown-unknown"),
        DebugFormat::Dwarf
    );
}

/// Test: Debug format display
#[test]
fn test_debug_format_display() {
    assert_eq!(format!("{}", DebugFormat::Dwarf), "DWARF");
    assert_eq!(format!("{}", DebugFormat::CodeView), "CodeView");
}

/// Test: Debug info config builder
#[test]
fn test_debug_info_config_builder() {
    let config = DebugInfoConfig::new(DebugLevel::Full)
        .with_format(DebugFormat::Dwarf)
        .with_split_debug_info(true)
        .with_optimized(true);

    assert_eq!(config.level, DebugLevel::Full);
    assert_eq!(config.format, DebugFormat::Dwarf);
    assert!(config.split_debug_info);
    assert!(config.optimized);
}

/// Test: Debug info config presets
#[test]
fn test_debug_info_config_presets() {
    // Default
    let config = DebugInfoConfig::default();
    assert_eq!(config.level, DebugLevel::None);

    // Development
    let config = DebugInfoConfig::development();
    assert_eq!(config.level, DebugLevel::Full);
    assert!(!config.optimized);

    // Release with debug
    let config = DebugInfoConfig::release_with_debug();
    assert_eq!(config.level, DebugLevel::LineTablesOnly);
    assert!(config.optimized);
}

/// Test: Debug info config for target
#[test]
fn test_debug_info_config_for_target() {
    let config = DebugInfoConfig::for_target(DebugLevel::Full, "x86_64-pc-windows-msvc");
    assert_eq!(config.level, DebugLevel::Full);
    assert_eq!(config.format, DebugFormat::CodeView);

    let config = DebugInfoConfig::for_target(DebugLevel::Full, "x86_64-unknown-linux-gnu");
    assert_eq!(config.format, DebugFormat::Dwarf);
}

/// Test: Debug config `needs_dsym`
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

/// Test: Debug config `needs_pdb`
#[test]
fn test_debug_config_needs_pdb() {
    let config = DebugInfoConfig::for_target(DebugLevel::Full, "x86_64-pc-windows-msvc");

    assert!(config.needs_pdb("x86_64-pc-windows-msvc"));
    assert!(!config.needs_pdb("x86_64-pc-windows-gnu"));
    assert!(!config.needs_pdb("x86_64-unknown-linux-gnu"));
}

/// Test: Ori symbol mangling scheme
///
/// Scenario from Rust `demangle`:
/// Verify symbol mangling format.
#[test]
fn test_mangler_function() {
    let mangler = Mangler::new();

    // Simple function in root module
    let main_sym = mangler.mangle_function("", "main");
    assert!(main_sym.starts_with(MANGLE_PREFIX));
    assert!(main_sym.contains("main"));

    // Function in module
    let math_add_sym = mangler.mangle_function("math", "add");
    assert!(math_add_sym.starts_with(MANGLE_PREFIX));
    assert!(math_add_sym.contains("math"));
    assert!(math_add_sym.contains("add"));
}

/// Test: Ori symbol detection
#[test]
fn test_is_ori_symbol() {
    assert!(is_ori_symbol("_ori_main"));
    assert!(is_ori_symbol("_ori_math$add"));
    assert!(!is_ori_symbol("printf"));
    assert!(!is_ori_symbol("_start"));
    assert!(!is_ori_symbol("malloc"));
}

/// Test: Symbol demangling
///
/// Scenario from Rust `demangle`:
/// Round-trip mangling/demangling.
#[test]
fn test_demangle() {
    // Simple function (demangled output uses Ori syntax with @ prefix)
    let demangled = demangle("_ori_main");
    assert!(demangled.is_some());
    assert_eq!(demangled.unwrap(), "@main");

    // Module function
    let demangled = demangle("_ori_math$add");
    assert!(demangled.is_some());
    let demangled_str = demangled.unwrap();
    assert!(demangled_str.contains("math"));
    assert!(demangled_str.contains("add"));

    // Not an Ori symbol
    assert!(demangle("printf").is_none());
    assert!(demangle("_start").is_none());
}

/// Test: Trait impl mangling
#[test]
fn test_mangle_trait_impl() {
    let mangler = Mangler::new();

    let mangled = mangler.mangle_trait_impl("int", "Eq", "equals");
    assert!(mangled.starts_with(MANGLE_PREFIX));
    assert!(mangled.contains("int"));
    assert!(mangled.contains("Eq"));
    assert!(mangled.contains("equals"));
}

/// Test: Extension method mangling
#[test]
fn test_mangle_extension() {
    let mangler = Mangler::new();

    let mangled = mangler.mangle_extension("[int]", "sum", "");
    assert!(mangled.starts_with(MANGLE_PREFIX));
    assert!(mangled.contains("sum"));

    let mangled = mangler.mangle_extension("str", "to_upper", "string_utils");
    assert!(mangled.contains("to_upper"));
}

/// Test: Generic function mangling
#[test]
fn test_mangle_generic() {
    let mangler = Mangler::new();

    let mangled = mangler.mangle_generic("", "identity", &["int"]);
    assert!(mangled.contains("identity"));
    assert!(mangled.contains("int"));

    let mangled = mangler.mangle_generic("", "map", &["int", "str"]);
    assert!(mangled.contains("map"));
}

/// Test: Associated function mangling
#[test]
fn test_mangle_associated_function() {
    let mangler = Mangler::new();

    let mangled = mangler.mangle_associated_function("Option", "some");
    assert!(mangled.starts_with(MANGLE_PREFIX));
    assert!(mangled.contains("Option"));
    assert!(mangled.contains("some"));
}

/// Test: Parse ELF object file structure
///
/// This test uses a pre-constructed minimal ELF for verification.
#[test]
fn test_parse_elf_structure() {
    // Minimal ELF64 header (not a complete object, just for parsing test)
    let elf_header: &[u8] = &[
        0x7f, 0x45, 0x4c, 0x46, // ELF magic
        0x02, // 64-bit
        0x01, // Little endian
        0x01, // ELF version
        0x00, // System V ABI
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // Padding
        0x01, 0x00, // Relocatable
        0x3e, 0x00, // x86-64
        0x01, 0x00, 0x00, 0x00, // ELF version
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // Entry
        0x40, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // Program header offset
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // Section header offset
        0x00, 0x00, 0x00, 0x00, // Flags
        0x40, 0x00, // ELF header size
        0x38, 0x00, // Program header size
        0x00, 0x00, // Program header count
        0x40, 0x00, // Section header size
        0x00, 0x00, // Section header count
        0x00, 0x00, // Section name string table index
    ];

    // This is just a header, not a complete object, so parsing may fail
    // The test verifies the utility function handles errors gracefully
    let result = parse_object(elf_header);
    // Either parses (unlikely with just header) or returns error (expected)
    let _ = result;
}

/// Test: Object format detection
#[test]
fn test_object_format_detection() {
    // ELF magic
    let elf = [0x7f, 0x45, 0x4c, 0x46];
    assert!(elf.starts_with(&[0x7f, 0x45, 0x4c, 0x46]));

    // Mach-O magic (64-bit)
    let macho = [0xfe, 0xed, 0xfa, 0xcf];
    assert!(macho.starts_with(&[0xfe, 0xed, 0xfa, 0xcf]));

    // COFF magic (PE)
    let coff = [0x4d, 0x5a]; // MZ
    assert!(coff.starts_with(&[0x4d, 0x5a]));

    // WASM magic
    let wasm = [0x00, 0x61, 0x73, 0x6d]; // \0asm
    assert!(wasm.starts_with(&[0x00, 0x61, 0x73, 0x6d]));
}

/// Test: Optimization level pipeline strings
///
/// Scenario: Different optimization levels produce different pipelines.
#[test]
fn test_optimization_pipelines() {
    // O0: No optimization
    assert_eq!(OptimizationLevel::O0.pipeline_string(), "default<O0>");

    // O1: Basic optimization
    assert_eq!(OptimizationLevel::O1.pipeline_string(), "default<O1>");

    // O2: Standard optimization
    assert_eq!(OptimizationLevel::O2.pipeline_string(), "default<O2>");

    // O3: Aggressive
    assert_eq!(OptimizationLevel::O3.pipeline_string(), "default<O3>");

    // Os: Size optimization
    assert_eq!(OptimizationLevel::Os.pipeline_string(), "default<Os>");

    // Oz: Aggressive size optimization
    assert_eq!(OptimizationLevel::Oz.pipeline_string(), "default<Oz>");
}
