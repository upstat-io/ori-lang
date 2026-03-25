//! The `build` command: AOT compilation to native executable.
//!
//! Compiles Ori source files to native executables, shared/static libraries,
//! or WebAssembly modules through the full LLVM pipeline.
//!
//! The build pipeline has two modes:
//! - **Single-file** (`single.rs`): direct compilation for files without imports
//! - **Multi-file** (`multi.rs`): dependency-graph-based compilation with LTO support

#[cfg(feature = "llvm")]
mod multi;
#[cfg(feature = "llvm")]
mod multi_emission;
#[cfg(feature = "llvm")]
mod single;

#[cfg(feature = "llvm")]
use std::path::Path;
#[cfg(feature = "llvm")]
use std::path::PathBuf;

#[cfg(feature = "llvm")]
use super::read_file;

// Re-export options types so `commands::build::BuildOptions` etc. still resolves.
pub use super::build_options::{
    parse_build_options, BuildOptions, DebugLevel, EmitType, LinkMode, LtoMode, OptLevel,
};

/// Check if source code has any imports.
///
/// Uses a simple line-based check for `use "./` or `use "../` patterns.
/// This is faster than parsing when we just need to detect presence of imports.
#[cfg(feature = "llvm")]
fn has_imports(content: &str) -> bool {
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("use \"./") || trimmed.starts_with("use \"../") {
            return true;
        }
    }
    false
}

/// Build an Ori source file to a native executable.
///
/// This performs the full AOT compilation pipeline:
/// 1. Parse and type-check the source
/// 2. Generate LLVM IR
/// 3. Run optimization passes
/// 4. Emit object file
/// 5. Link into executable
///
/// If the source file has imports, this delegates to multi-file compilation.
#[cfg(feature = "llvm")]
pub fn build_file(path: &str, options: &BuildOptions) {
    use std::time::Instant;

    let start = Instant::now();

    // Read the source file
    let content = read_file(path);

    // Check if file has imports - if so, use multi-file compilation
    if has_imports(&content) {
        if options.verbose {
            eprintln!("  Detected imports, using multi-file compilation...");
        }
        multi::build_file_multi(path, options, start);
    } else {
        single::build_file_single(path, content, options, start);
    }
}

/// Build command when LLVM feature is not enabled.
#[cfg(not(feature = "llvm"))]
pub fn build_file(_path: &str, _options: &BuildOptions) {
    use ori_diagnostic::emitter::{ColorMode, DiagnosticEmitter, TerminalEmitter};
    use ori_diagnostic::{Diagnostic, ErrorCode};

    let diag = Diagnostic::error(ErrorCode::E5004)
        .with_message("the 'build' command requires the LLVM backend")
        .with_note("the Ori compiler was built without LLVM support")
        .with_suggestion("rebuild with `cargo build --features llvm`");

    let is_tty = std::io::IsTerminal::is_terminal(&std::io::stderr());
    let mut emitter = TerminalEmitter::with_color_mode(std::io::stderr(), ColorMode::Auto, is_tty);
    emitter.emit(&diag);
    emitter.flush();
    std::process::exit(1);
}

/// Configure target from build options.
#[cfg(feature = "llvm")]
fn configure_target(
    options: &BuildOptions,
) -> Result<ori_llvm::aot::TargetConfig, ori_llvm::aot::TargetError> {
    use ori_llvm::aot::TargetConfig;
    use ori_llvm::inkwell::OptimizationLevel as InkwellOptLevel;

    let mut target = if let Some(ref triple) = options.target {
        TargetConfig::from_triple(triple)?
    } else if options.wasm {
        TargetConfig::from_triple("wasm32-unknown-unknown")?
    } else {
        TargetConfig::native()?
    };

    // Apply CPU setting
    if let Some(ref cpu) = options.cpu {
        if cpu == "native" {
            target = target.with_cpu_native();
        } else {
            target = target.with_cpu(cpu);
        }
    }

    // Apply features
    if let Some(ref features) = options.features {
        target = target.with_features(features);
    }

    // Apply optimization level for codegen
    let opt_level = match options.opt_level {
        OptLevel::O0 => InkwellOptLevel::None,
        OptLevel::O1 => InkwellOptLevel::Less,
        OptLevel::O2 | OptLevel::Os => InkwellOptLevel::Default,
        OptLevel::O3 | OptLevel::Oz => InkwellOptLevel::Aggressive,
    };
    target = target.with_opt_level(opt_level);

    Ok(target)
}

/// Build optimization configuration from options.
#[cfg(feature = "llvm")]
fn build_optimization_config(options: &BuildOptions) -> ori_llvm::aot::OptimizationConfig {
    use ori_llvm::aot::{LtoMode as LlvmLtoMode, OptimizationConfig, OptimizationLevel};

    let level = match options.opt_level {
        OptLevel::O0 => OptimizationLevel::O0,
        OptLevel::O1 => OptimizationLevel::O1,
        OptLevel::O2 => OptimizationLevel::O2,
        OptLevel::O3 => OptimizationLevel::O3,
        OptLevel::Os => OptimizationLevel::Os,
        OptLevel::Oz => OptimizationLevel::Oz,
    };

    let lto = match options.lto {
        LtoMode::Off => LlvmLtoMode::Off,
        LtoMode::Thin => LlvmLtoMode::Thin,
        LtoMode::Full => LlvmLtoMode::Full,
    };

    OptimizationConfig::new(level).with_lto(lto)
}

/// Determine the output path for the build.
#[cfg(feature = "llvm")]
fn determine_output_path(source_path: &str, options: &BuildOptions) -> PathBuf {
    // If explicit output path given, use it
    if let Some(ref output) = options.output {
        return output.clone();
    }

    // Get the base name from source file
    let source = Path::new(source_path);
    let stem = source.file_stem().and_then(|s| s.to_str()).unwrap_or("a");

    // Determine output directory
    let out_dir = if let Some(ref dir) = options.out_dir {
        dir.clone()
    } else if options.release {
        PathBuf::from("build/release")
    } else {
        PathBuf::from("build/debug")
    };

    // Create output directory if it doesn't exist
    if let Err(e) = std::fs::create_dir_all(&out_dir) {
        eprintln!("warning: could not create output directory: {e}");
    }

    // Determine extension based on output type and target
    let extension = if options.lib {
        "a"
    } else if options.dylib {
        if cfg!(target_os = "windows") {
            "dll"
        } else if cfg!(target_os = "macos") {
            "dylib"
        } else {
            "so"
        }
    } else if options.wasm {
        "wasm"
    } else if cfg!(target_os = "windows") {
        "exe"
    } else {
        ""
    };

    let mut output = out_dir.join(stem);
    if !extension.is_empty() {
        output.set_extension(extension);
    }

    output
}

/// Link object files and finish.
#[cfg(feature = "llvm")]
fn link_and_finish(
    object_files: Vec<PathBuf>,
    output_path: &Path,
    target: &ori_llvm::aot::TargetConfig,
    options: &BuildOptions,
    start: std::time::Instant,
) {
    use ori_llvm::aot::{LinkInput, LinkOutput, LinkerDriver, LinkerFlavor, RuntimeConfig};

    if options.verbose {
        eprintln!("  Linking to {}", output_path.display());
    }

    let driver = LinkerDriver::new(target);

    // Find runtime library
    let runtime_config = match RuntimeConfig::detect() {
        Ok(config) => config,
        Err(e) => {
            crate::problem::codegen::report_codegen_error(e);
        }
    };

    let output_kind = if options.lib {
        LinkOutput::StaticLibrary
    } else if options.dylib {
        LinkOutput::SharedLibrary
    } else {
        LinkOutput::Executable
    };

    let mut link_input = LinkInput {
        objects: object_files,
        output: output_path.to_path_buf(),
        output_kind,
        lto: matches!(options.lto, LtoMode::Thin | LtoMode::Full),
        gc_sections: options.release,
        strip: options.release && matches!(options.debug_level, DebugLevel::None),
        ..Default::default()
    };

    // Configure runtime library linking
    runtime_config.configure_link(&mut link_input);

    // Override linker flavor if specified
    if let Some(ref linker_name) = options.linker {
        link_input.linker = match linker_name.as_str() {
            "lld" => Some(LinkerFlavor::Lld),
            "system" | "gcc" | "cc" => Some(LinkerFlavor::Gcc),
            "msvc" => Some(LinkerFlavor::Msvc),
            _ => None,
        };
    }

    if let Err(e) = driver.link(&link_input) {
        crate::problem::codegen::report_codegen_error(e);
    }

    let elapsed = start.elapsed();
    eprintln!(
        "  Finished {} in {:.2}s",
        output_path.display(),
        elapsed.as_secs_f64()
    );
}
