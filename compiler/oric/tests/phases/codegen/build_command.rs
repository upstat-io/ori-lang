//! Tests for the build command options parsing (`oric::commands::build`).
//!
//! These tests verify:
//! - `OptLevel`, `DebugLevel`, `EmitType`, `LinkMode`, `LtoMode` parsing
//! - `BuildOptions` parsing from command line arguments
//! - Flag combinations and defaults

use oric::commands::build::{
    parse_build_options, BuildOptions, DebugLevel, EmitType, LinkMode, LtoMode, OptLevel,
};
use std::path::PathBuf;

// -- OptLevel tests --

#[test]
fn test_opt_level_parse_valid() {
    assert_eq!(OptLevel::parse("0"), Some(OptLevel::O0));
    assert_eq!(OptLevel::parse("1"), Some(OptLevel::O1));
    assert_eq!(OptLevel::parse("2"), Some(OptLevel::O2));
    assert_eq!(OptLevel::parse("3"), Some(OptLevel::O3));
    assert_eq!(OptLevel::parse("s"), Some(OptLevel::Os));
    assert_eq!(OptLevel::parse("z"), Some(OptLevel::Oz));
}

#[test]
fn test_opt_level_parse_invalid() {
    assert_eq!(OptLevel::parse("4"), None);
    assert_eq!(OptLevel::parse("x"), None);
    assert_eq!(OptLevel::parse(""), None);
    assert_eq!(OptLevel::parse("O2"), None); // Must be just "2", not "O2"
}

#[test]
fn test_opt_level_default() {
    assert_eq!(OptLevel::default(), OptLevel::O0);
}

// -- DebugLevel tests --

#[test]
fn test_debug_level_parse_valid() {
    assert_eq!(DebugLevel::parse("0"), Some(DebugLevel::None));
    assert_eq!(DebugLevel::parse("1"), Some(DebugLevel::LineTablesOnly));
    assert_eq!(DebugLevel::parse("2"), Some(DebugLevel::Full));
}

#[test]
fn test_debug_level_parse_invalid() {
    assert_eq!(DebugLevel::parse("3"), None);
    assert_eq!(DebugLevel::parse("full"), None);
    assert_eq!(DebugLevel::parse(""), None);
}

#[test]
fn test_debug_level_default() {
    assert_eq!(DebugLevel::default(), DebugLevel::Full);
}

// -- EmitType tests --

#[test]
fn test_emit_type_parse_valid() {
    assert_eq!(EmitType::parse("obj"), Some(EmitType::Object));
    assert_eq!(EmitType::parse("object"), Some(EmitType::Object));
    assert_eq!(EmitType::parse("llvm-ir"), Some(EmitType::LlvmIr));
    assert_eq!(EmitType::parse("ir"), Some(EmitType::LlvmIr));
    assert_eq!(EmitType::parse("llvm-bc"), Some(EmitType::LlvmBc));
    assert_eq!(EmitType::parse("bc"), Some(EmitType::LlvmBc));
    assert_eq!(EmitType::parse("bitcode"), Some(EmitType::LlvmBc));
    assert_eq!(EmitType::parse("asm"), Some(EmitType::Assembly));
    assert_eq!(EmitType::parse("assembly"), Some(EmitType::Assembly));
}

#[test]
fn test_emit_type_parse_invalid() {
    assert_eq!(EmitType::parse("exe"), None);
    assert_eq!(EmitType::parse("wasm"), None);
    assert_eq!(EmitType::parse(""), None);
}

// -- LinkMode tests --

#[test]
fn test_link_mode_parse_valid() {
    assert_eq!(LinkMode::parse("static"), Some(LinkMode::Static));
    assert_eq!(LinkMode::parse("dynamic"), Some(LinkMode::Dynamic));
}

#[test]
fn test_link_mode_parse_invalid() {
    assert_eq!(LinkMode::parse("shared"), None);
    assert_eq!(LinkMode::parse(""), None);
}

#[test]
fn test_link_mode_default() {
    assert_eq!(LinkMode::default(), LinkMode::Static);
}

// -- LtoMode tests --

#[test]
fn test_lto_mode_parse_valid() {
    assert_eq!(LtoMode::parse("off"), Some(LtoMode::Off));
    assert_eq!(LtoMode::parse("false"), Some(LtoMode::Off));
    assert_eq!(LtoMode::parse("no"), Some(LtoMode::Off));
    assert_eq!(LtoMode::parse("thin"), Some(LtoMode::Thin));
    assert_eq!(LtoMode::parse("full"), Some(LtoMode::Full));
    assert_eq!(LtoMode::parse("true"), Some(LtoMode::Full));
    assert_eq!(LtoMode::parse("yes"), Some(LtoMode::Full));
}

#[test]
fn test_lto_mode_parse_invalid() {
    assert_eq!(LtoMode::parse("none"), None);
    assert_eq!(LtoMode::parse(""), None);
}

#[test]
fn test_lto_mode_default() {
    assert_eq!(LtoMode::default(), LtoMode::Off);
}

// -- parse_build_options tests --

#[test]
fn test_parse_build_options_defaults() {
    let options = parse_build_options(&[]);
    assert!(!options.release);
    assert!(options.target.is_none());
    assert_eq!(options.opt_level, OptLevel::O0);
    assert_eq!(options.debug_level, DebugLevel::Full);
    assert!(options.output.is_none());
    assert!(options.out_dir.is_none());
    assert!(options.emit.is_none());
    assert!(!options.lib);
    assert!(!options.dylib);
    assert!(!options.wasm);
    assert!(options.linker.is_none());
    assert_eq!(options.link_mode, LinkMode::Static);
    assert_eq!(options.lto, LtoMode::Off);
    assert!(options.jobs.is_none());
    assert!(options.cpu.is_none());
    assert!(options.features.is_none());
    assert!(!options.js_bindings);
    assert!(!options.wasm_opt);
    assert!(!options.verbose);
}

#[test]
fn test_parse_build_options_release() {
    let args = vec!["--release".to_string()];
    let options = parse_build_options(&args);
    assert!(options.release);
    assert_eq!(options.opt_level, OptLevel::O2); // --release implies O2
    assert_eq!(options.debug_level, DebugLevel::None); // --release implies no debug
}

#[test]
fn test_parse_build_options_target() {
    let args = vec!["--target=x86_64-unknown-linux-gnu".to_string()];
    let options = parse_build_options(&args);
    assert_eq!(options.target, Some("x86_64-unknown-linux-gnu".to_string()));
}

#[test]
fn test_parse_build_options_opt_level() {
    let args = vec!["--opt=3".to_string()];
    let options = parse_build_options(&args);
    assert_eq!(options.opt_level, OptLevel::O3);

    let args = vec!["--opt=s".to_string()];
    let options = parse_build_options(&args);
    assert_eq!(options.opt_level, OptLevel::Os);

    let args = vec!["--opt=z".to_string()];
    let options = parse_build_options(&args);
    assert_eq!(options.opt_level, OptLevel::Oz);
}

#[test]
fn test_parse_build_options_debug_level() {
    let args = vec!["--debug=0".to_string()];
    let options = parse_build_options(&args);
    assert_eq!(options.debug_level, DebugLevel::None);

    let args = vec!["--debug=1".to_string()];
    let options = parse_build_options(&args);
    assert_eq!(options.debug_level, DebugLevel::LineTablesOnly);

    let args = vec!["--debug=2".to_string()];
    let options = parse_build_options(&args);
    assert_eq!(options.debug_level, DebugLevel::Full);
}

#[test]
fn test_parse_build_options_output_path() {
    let args = vec!["-o=myapp".to_string()];
    let options = parse_build_options(&args);
    assert_eq!(options.output, Some(PathBuf::from("myapp")));

    let args = vec!["--output=/path/to/output".to_string()];
    let options = parse_build_options(&args);
    assert_eq!(options.output, Some(PathBuf::from("/path/to/output")));
}

#[test]
fn test_parse_build_options_out_dir() {
    let args = vec!["--out-dir=build/custom".to_string()];
    let options = parse_build_options(&args);
    assert_eq!(options.out_dir, Some(PathBuf::from("build/custom")));
}

#[test]
fn test_parse_build_options_emit() {
    let args = vec!["--emit=obj".to_string()];
    let options = parse_build_options(&args);
    assert_eq!(options.emit, Some(EmitType::Object));

    let args = vec!["--emit=llvm-ir".to_string()];
    let options = parse_build_options(&args);
    assert_eq!(options.emit, Some(EmitType::LlvmIr));

    let args = vec!["--emit=asm".to_string()];
    let options = parse_build_options(&args);
    assert_eq!(options.emit, Some(EmitType::Assembly));
}

#[test]
fn test_parse_build_options_library_modes() {
    let args = vec!["--lib".to_string()];
    let options = parse_build_options(&args);
    assert!(options.lib);

    let args = vec!["--dylib".to_string()];
    let options = parse_build_options(&args);
    assert!(options.dylib);
}

#[test]
fn test_parse_build_options_wasm() {
    let args = vec!["--wasm".to_string()];
    let options = parse_build_options(&args);
    assert!(options.wasm);
}

#[test]
fn test_parse_build_options_linker() {
    let args = vec!["--linker=lld".to_string()];
    let options = parse_build_options(&args);
    assert_eq!(options.linker, Some("lld".to_string()));

    let args = vec!["--linker=system".to_string()];
    let options = parse_build_options(&args);
    assert_eq!(options.linker, Some("system".to_string()));
}

#[test]
fn test_parse_build_options_link_mode() {
    let args = vec!["--link=static".to_string()];
    let options = parse_build_options(&args);
    assert_eq!(options.link_mode, LinkMode::Static);

    let args = vec!["--link=dynamic".to_string()];
    let options = parse_build_options(&args);
    assert_eq!(options.link_mode, LinkMode::Dynamic);
}

#[test]
fn test_parse_build_options_lto() {
    let args = vec!["--lto=thin".to_string()];
    let options = parse_build_options(&args);
    assert_eq!(options.lto, LtoMode::Thin);

    let args = vec!["--lto=full".to_string()];
    let options = parse_build_options(&args);
    assert_eq!(options.lto, LtoMode::Full);
}

#[test]
fn test_parse_build_options_jobs() {
    let args = vec!["--jobs=4".to_string()];
    let options = parse_build_options(&args);
    assert_eq!(options.jobs, Some(4));

    let args = vec!["--jobs=auto".to_string()];
    let options = parse_build_options(&args);
    assert_eq!(options.jobs, None); // auto = use available cores

    let args = vec!["-j".to_string()];
    let options = parse_build_options(&args);
    assert_eq!(options.jobs, None); // -j = auto
}

#[test]
fn test_parse_build_options_cpu_features() {
    let args = vec!["--cpu=native".to_string()];
    let options = parse_build_options(&args);
    assert_eq!(options.cpu, Some("native".to_string()));

    let args = vec!["--cpu=haswell".to_string()];
    let options = parse_build_options(&args);
    assert_eq!(options.cpu, Some("haswell".to_string()));

    let args = vec!["--features=+avx2,-sse4".to_string()];
    let options = parse_build_options(&args);
    assert_eq!(options.features, Some("+avx2,-sse4".to_string()));
}

#[test]
fn test_parse_build_options_wasm_flags() {
    let args = vec!["--js-bindings".to_string()];
    let options = parse_build_options(&args);
    assert!(options.js_bindings);

    let args = vec!["--wasm-opt".to_string()];
    let options = parse_build_options(&args);
    assert!(options.wasm_opt);
}

#[test]
fn test_parse_build_options_verbose() {
    let args = vec!["-v".to_string()];
    let options = parse_build_options(&args);
    assert!(options.verbose);

    let args = vec!["--verbose".to_string()];
    let options = parse_build_options(&args);
    assert!(options.verbose);
}

#[test]
fn test_parse_build_options_multiple_flags() {
    let args = vec![
        "--release".to_string(),
        "--target=wasm32-unknown-unknown".to_string(),
        "--opt=z".to_string(),
        "-v".to_string(),
        "--js-bindings".to_string(),
    ];
    let options = parse_build_options(&args);
    assert!(options.release);
    assert_eq!(options.target, Some("wasm32-unknown-unknown".to_string()));
    assert_eq!(options.opt_level, OptLevel::Oz); // --opt overrides --release default
    assert!(options.verbose);
    assert!(options.js_bindings);
}

#[test]
fn test_parse_build_options_flag_order_independent() {
    // Order shouldn't matter for independent flags
    let args1 = vec!["--wasm".to_string(), "--verbose".to_string()];
    let args2 = vec!["--verbose".to_string(), "--wasm".to_string()];

    let opt1 = parse_build_options(&args1);
    let opt2 = parse_build_options(&args2);

    assert_eq!(opt1.wasm, opt2.wasm);
    assert_eq!(opt1.verbose, opt2.verbose);
}

// -- BuildOptions Default tests --

#[test]
fn test_build_options_default() {
    let default = BuildOptions::default();
    assert!(!default.release);
    assert!(default.target.is_none());
    assert_eq!(default.opt_level, OptLevel::O0);
    assert_eq!(default.debug_level, DebugLevel::Full);
    assert!(default.output.is_none());
    assert!(default.emit.is_none());
    assert!(!default.lib);
    assert!(!default.dylib);
    assert!(!default.wasm);
    assert!(default.linker.is_none());
    assert_eq!(default.link_mode, LinkMode::Static);
    assert_eq!(default.lto, LtoMode::Off);
    assert!(default.jobs.is_none());
    assert!(!default.verbose);
}

#[test]
fn test_build_options_clone() {
    let options = BuildOptions {
        release: true,
        target: Some("x86_64-apple-darwin".to_string()),
        opt_level: OptLevel::O3,
        ..Default::default()
    };

    let cloned = options.clone();
    assert_eq!(cloned.release, options.release);
    assert_eq!(cloned.target, options.target);
    assert_eq!(cloned.opt_level, options.opt_level);
}

// -- EmitType extension tests (only when LLVM feature enabled) --

#[cfg(feature = "llvm")]
#[test]
fn test_emit_type_extension() {
    assert_eq!(EmitType::Object.extension(), "o");
    assert_eq!(EmitType::LlvmIr.extension(), "ll");
    assert_eq!(EmitType::LlvmBc.extension(), "bc");
    assert_eq!(EmitType::Assembly.extension(), "s");
}
