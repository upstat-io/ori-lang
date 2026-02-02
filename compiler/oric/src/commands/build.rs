//! The `build` command: AOT compilation to native executable.
//!
//! Compiles Ori source files to native executables, shared/static libraries,
//! or WebAssembly modules through the full LLVM pipeline.

#[cfg(feature = "llvm")]
use std::path::Path;
use std::path::PathBuf;

#[cfg(feature = "llvm")]
use super::read_file;

/// Build options parsed from command line arguments.
///
/// This struct naturally has many boolean flags representing independent
/// build configuration options (release mode, library type flags, verbose
/// output, etc.). These are not state machine candidates as they are
/// independent orthogonal settings.
#[derive(Debug, Clone)]
#[allow(clippy::struct_excessive_bools)]
pub struct BuildOptions {
    /// Build with optimizations (--release)
    pub release: bool,
    /// Target triple (--target=<triple>)
    pub target: Option<String>,
    /// Optimization level: 0, 1, 2, 3, s, z (--opt=<level>)
    pub opt_level: OptLevel,
    /// Debug info level: 0, 1, 2 (--debug=<level>)
    pub debug_level: DebugLevel,
    /// Output file path (-o, --output)
    pub output: Option<PathBuf>,
    /// Output directory (--out-dir)
    pub out_dir: Option<PathBuf>,
    /// Emit type: obj, llvm-ir, llvm-bc, asm (--emit)
    pub emit: Option<EmitType>,
    /// Build as static library (--lib)
    pub lib: bool,
    /// Build as shared library (--dylib)
    pub dylib: bool,
    /// Build for WebAssembly (--wasm)
    pub wasm: bool,
    /// Linker to use: system, lld (--linker)
    pub linker: Option<String>,
    /// Link mode: static, dynamic (--link)
    pub link_mode: LinkMode,
    /// LTO mode: off, thin, full (--lto)
    pub lto: LtoMode,
    /// Parallel compilation jobs (--jobs)
    pub jobs: Option<usize>,
    /// Target CPU (--cpu)
    pub cpu: Option<String>,
    /// CPU features (--features)
    pub features: Option<String>,
    /// Generate JavaScript bindings for WASM (--js-bindings)
    pub js_bindings: bool,
    /// Run wasm-opt post-processor (--wasm-opt)
    pub wasm_opt: bool,
    /// Verbose output (-v, --verbose)
    pub verbose: bool,
}

impl Default for BuildOptions {
    fn default() -> Self {
        Self {
            release: false,
            target: None,
            opt_level: OptLevel::O0,
            debug_level: DebugLevel::Full,
            output: None,
            out_dir: None,
            emit: None,
            lib: false,
            dylib: false,
            wasm: false,
            linker: None,
            link_mode: LinkMode::Static,
            lto: LtoMode::Off,
            jobs: None,
            cpu: None,
            features: None,
            js_bindings: false,
            wasm_opt: false,
            verbose: false,
        }
    }
}

impl BuildOptions {
    /// Merge another `BuildOptions` into this one.
    ///
    /// For boolean flags, uses OR (true wins).
    /// For Option fields, takes the new value if present.
    /// For --release, also applies its implied `opt_level` and `debug_level`.
    pub fn merge(&mut self, other: &Self) {
        // Handle --release specially: it implies opt_level and debug_level
        if other.release {
            self.release = true;
            self.opt_level = other.opt_level;
            self.debug_level = other.debug_level;
        }

        // Option fields: take new value if present
        if other.target.is_some() {
            self.target.clone_from(&other.target);
        }
        if other.output.is_some() {
            self.output.clone_from(&other.output);
        }
        if other.out_dir.is_some() {
            self.out_dir.clone_from(&other.out_dir);
        }
        if other.emit.is_some() {
            self.emit = other.emit;
        }
        if other.linker.is_some() {
            self.linker.clone_from(&other.linker);
        }
        if other.cpu.is_some() {
            self.cpu.clone_from(&other.cpu);
        }
        if other.features.is_some() {
            self.features.clone_from(&other.features);
        }

        // Boolean flags: OR (true wins)
        self.lib |= other.lib;
        self.dylib |= other.dylib;
        self.wasm |= other.wasm;
        self.js_bindings |= other.js_bindings;
        self.wasm_opt |= other.wasm_opt;
        self.verbose |= other.verbose;
    }
}

/// Optimization level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OptLevel {
    /// No optimization (fastest compile, debugging)
    #[default]
    O0,
    /// Basic optimization
    O1,
    /// Standard optimization (production default)
    O2,
    /// Aggressive optimization
    O3,
    /// Optimize for size
    Os,
    /// Minimize size aggressively
    Oz,
}

impl OptLevel {
    /// Parse from command line string.
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "0" => Some(Self::O0),
            "1" => Some(Self::O1),
            "2" => Some(Self::O2),
            "3" => Some(Self::O3),
            "s" => Some(Self::Os),
            "z" => Some(Self::Oz),
            _ => None,
        }
    }
}

/// Debug information level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DebugLevel {
    /// No debug info
    None,
    /// Line tables only
    LineTablesOnly,
    /// Full debug info (variables, types, source)
    #[default]
    Full,
}

impl DebugLevel {
    /// Parse from command line string.
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "0" => Some(Self::None),
            "1" => Some(Self::LineTablesOnly),
            "2" => Some(Self::Full),
            _ => None,
        }
    }
}

/// What to emit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmitType {
    /// Native object file (.o)
    Object,
    /// LLVM IR text (.ll)
    LlvmIr,
    /// LLVM bitcode (.bc)
    LlvmBc,
    /// Assembly (.s)
    Assembly,
}

impl EmitType {
    /// Parse from command line string.
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "obj" | "object" => Some(Self::Object),
            "llvm-ir" | "ir" => Some(Self::LlvmIr),
            "llvm-bc" | "bc" | "bitcode" => Some(Self::LlvmBc),
            "asm" | "assembly" => Some(Self::Assembly),
            _ => None,
        }
    }

    /// Get the file extension for this emit type.
    #[cfg(feature = "llvm")]
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Object => "o",
            Self::LlvmIr => "ll",
            Self::LlvmBc => "bc",
            Self::Assembly => "s",
        }
    }
}

/// Link mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LinkMode {
    /// Static linking (embed runtime)
    #[default]
    Static,
    /// Dynamic linking (link to `libori_rt.so`)
    Dynamic,
}

impl LinkMode {
    /// Parse from command line string.
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "static" => Some(Self::Static),
            "dynamic" => Some(Self::Dynamic),
            _ => None,
        }
    }
}

/// LTO mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LtoMode {
    /// No LTO
    #[default]
    Off,
    /// Thin LTO (parallel, fast)
    Thin,
    /// Full LTO (maximum optimization)
    Full,
}

impl LtoMode {
    /// Parse from command line string.
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "off" | "false" | "no" => Some(Self::Off),
            "thin" => Some(Self::Thin),
            "full" | "true" | "yes" => Some(Self::Full),
            _ => None,
        }
    }
}

/// Parse build options from command line arguments.
pub fn parse_build_options(args: &[String]) -> BuildOptions {
    let mut options = BuildOptions::default();

    for arg in args {
        if arg == "--release" {
            options.release = true;
            // --release implies O2 and no debug info
            options.opt_level = OptLevel::O2;
            options.debug_level = DebugLevel::None;
        } else if let Some(target) = arg.strip_prefix("--target=") {
            options.target = Some(target.to_string());
        } else if let Some(level) = arg.strip_prefix("--opt=") {
            if let Some(opt) = OptLevel::from_str(level) {
                options.opt_level = opt;
            } else {
                eprintln!("warning: unknown optimization level '{level}', using O0");
            }
        } else if let Some(level) = arg.strip_prefix("--debug=") {
            if let Some(dbg) = DebugLevel::from_str(level) {
                options.debug_level = dbg;
            } else {
                eprintln!("warning: unknown debug level '{level}', using full");
            }
        } else if let Some(output) = arg.strip_prefix("-o=") {
            options.output = Some(PathBuf::from(output));
        } else if let Some(output) = arg.strip_prefix("--output=") {
            options.output = Some(PathBuf::from(output));
        } else if let Some(dir) = arg.strip_prefix("--out-dir=") {
            options.out_dir = Some(PathBuf::from(dir));
        } else if let Some(emit) = arg.strip_prefix("--emit=") {
            if let Some(e) = EmitType::from_str(emit) {
                options.emit = Some(e);
            } else {
                eprintln!(
                    "warning: unknown emit type '{emit}', options: obj, llvm-ir, llvm-bc, asm"
                );
            }
        } else if arg == "--lib" {
            options.lib = true;
        } else if arg == "--dylib" {
            options.dylib = true;
        } else if arg == "--wasm" {
            options.wasm = true;
        } else if let Some(linker) = arg.strip_prefix("--linker=") {
            options.linker = Some(linker.to_string());
        } else if let Some(link) = arg.strip_prefix("--link=") {
            if let Some(mode) = LinkMode::from_str(link) {
                options.link_mode = mode;
            } else {
                eprintln!("warning: unknown link mode '{link}', using static");
            }
        } else if let Some(lto) = arg.strip_prefix("--lto=") {
            if let Some(mode) = LtoMode::from_str(lto) {
                options.lto = mode;
            } else {
                eprintln!("warning: unknown LTO mode '{lto}', using off");
            }
        } else if let Some(jobs) = arg.strip_prefix("--jobs=") {
            if jobs == "auto" {
                options.jobs = None; // Will use available cores
            } else if let Ok(n) = jobs.parse() {
                options.jobs = Some(n);
            } else {
                eprintln!("warning: invalid jobs count '{jobs}', using auto");
            }
        } else if arg == "-j" {
            // Shorthand for --jobs=auto
            options.jobs = None;
        } else if let Some(cpu) = arg.strip_prefix("--cpu=") {
            options.cpu = Some(cpu.to_string());
        } else if let Some(features) = arg.strip_prefix("--features=") {
            options.features = Some(features.to_string());
        } else if arg == "--js-bindings" {
            options.js_bindings = true;
        } else if arg == "--wasm-opt" {
            options.wasm_opt = true;
        } else if arg == "-v" || arg == "--verbose" {
            options.verbose = true;
        }
    }

    // Handle -o without = (next arg is the path)
    // This is handled in the caller since it requires peeking ahead

    options
}

/// Build an Ori source file to a native executable.
///
/// This performs the full AOT compilation pipeline:
/// 1. Parse and type-check the source
/// 2. Generate LLVM IR
/// 3. Run optimization passes
/// 4. Emit object file
/// 5. Link into executable
#[cfg(feature = "llvm")]
pub(crate) fn build_file(path: &str, options: &BuildOptions) {
    use ori_llvm::inkwell::context::Context;
    use std::time::Instant;

    use oric::{CompilerDb, SourceFile};

    use ori_llvm::aot::{
        LinkInput, LinkOutput, LinkerDriver, LinkerFlavor, ObjectEmitter, OutputFormat,
        RuntimeConfig,
    };

    use super::compile_common::{check_source, compile_to_llvm};

    let start = Instant::now();

    // Step 1: Read and parse the source file
    if options.verbose {
        eprintln!("  Compiling {path}...");
    }

    let content = read_file(path);
    let db = CompilerDb::new();
    let file = SourceFile::new(&db, PathBuf::from(path), content);

    // Check for parse and type errors (shared with run_file_compiled)
    let (parse_result, type_result) = match check_source(&db, file, path) {
        Some(results) => results,
        None => std::process::exit(1),
    };

    // Step 2: Configure target
    let target = match configure_target(options) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("error: failed to configure target: {e}");
            std::process::exit(1);
        }
    };

    if options.verbose {
        eprintln!("  Target: {}", target.triple());
        eprintln!("  Optimization: {:?}", options.opt_level);
    }

    // Step 3: Generate LLVM IR (shared with run_file_compiled)
    let context = Context::create();
    let llvm_module = compile_to_llvm(&context, &db, &parse_result, &type_result, path);

    // TODO: Add main entry point wrapper that calls @main and handles exit code
    // For now, the linker will use the compiled @main directly if it exists

    // Configure module for target
    let emitter = match ObjectEmitter::new(&target) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("error: failed to create object emitter: {e}");
            std::process::exit(1);
        }
    };

    if let Err(e) = emitter.configure_module(&llvm_module) {
        eprintln!("error: failed to configure module: {e}");
        std::process::exit(1);
    }

    // Step 4: Run optimization passes
    let opt_config = build_optimization_config(options);
    if let Err(e) =
        ori_llvm::aot::run_optimization_passes(&llvm_module, emitter.machine(), &opt_config)
    {
        eprintln!("error: optimization failed: {e}");
        std::process::exit(1);
    }

    // Step 5: Determine output path
    let output_path = determine_output_path(path, options);

    // Step 6: Emit based on emit type
    if let Some(emit_type) = options.emit {
        // Just emit the requested format, don't link
        let emit_path = output_path.with_extension(emit_type.extension());

        if options.verbose {
            eprintln!("  Emitting {:?} to {}", emit_type, emit_path.display());
        }

        let format = match emit_type {
            EmitType::Object => OutputFormat::Object,
            EmitType::LlvmIr => OutputFormat::LlvmIr,
            EmitType::LlvmBc => OutputFormat::Bitcode,
            EmitType::Assembly => OutputFormat::Assembly,
        };

        if let Err(e) = emitter.emit(&llvm_module, &emit_path, format) {
            eprintln!("error: failed to emit: {e}");
            std::process::exit(1);
        }

        let elapsed = start.elapsed();
        if options.verbose {
            eprintln!("  Finished in {:.2}s", elapsed.as_secs_f64());
        }
        return;
    }

    // Step 7: Emit object file to temp location
    let temp_dir = match std::env::temp_dir().canonicalize() {
        Ok(d) => d,
        Err(_) => std::env::temp_dir(),
    };
    let module_name = Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("module");
    let obj_path = temp_dir.join(format!("{module_name}.o"));

    if options.verbose {
        eprintln!("  Emitting object to {}", obj_path.display());
    }

    if let Err(e) = emitter.emit_object(&llvm_module, &obj_path) {
        eprintln!("error: failed to emit object file: {e}");
        std::process::exit(1);
    }

    // Step 8: Link into executable
    if options.verbose {
        eprintln!("  Linking to {}", output_path.display());
    }

    let driver = LinkerDriver::new(&target);

    // Find runtime library
    let runtime_config = match RuntimeConfig::detect() {
        Ok(config) => config,
        Err(e) => {
            eprintln!("error: {e}");
            eprintln!("hint: ensure libori_rt is built and available");
            std::process::exit(1);
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
        objects: vec![obj_path.clone()],
        output: output_path.clone(),
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
        eprintln!("error: linking failed: {e}");
        std::process::exit(1);
    }

    // Clean up temp object file
    let _ = std::fs::remove_file(&obj_path);

    let elapsed = start.elapsed();
    eprintln!(
        "  Finished {} in {:.2}s",
        output_path.display(),
        elapsed.as_secs_f64()
    );
}

/// Build command when LLVM feature is not enabled.
#[cfg(not(feature = "llvm"))]
pub(crate) fn build_file(_path: &str, _options: &BuildOptions) {
    eprintln!("error: the 'build' command requires the LLVM backend");
    eprintln!();
    eprintln!("The Ori compiler was built without LLVM support.");
    eprintln!("To enable AOT compilation, rebuild with the 'llvm' feature:");
    eprintln!();
    eprintln!("  cargo build --features llvm");
    eprintln!();
    eprintln!("Or use the LLVM-enabled Docker container:");
    eprintln!();
    eprintln!("  ./docker/llvm/run.sh ori build <file.ori>");
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

#[cfg(test)]
mod tests {
    use super::*;

    // -- OptLevel tests --

    #[test]
    fn test_opt_level_from_str_valid() {
        assert_eq!(OptLevel::from_str("0"), Some(OptLevel::O0));
        assert_eq!(OptLevel::from_str("1"), Some(OptLevel::O1));
        assert_eq!(OptLevel::from_str("2"), Some(OptLevel::O2));
        assert_eq!(OptLevel::from_str("3"), Some(OptLevel::O3));
        assert_eq!(OptLevel::from_str("s"), Some(OptLevel::Os));
        assert_eq!(OptLevel::from_str("z"), Some(OptLevel::Oz));
    }

    #[test]
    fn test_opt_level_from_str_invalid() {
        assert_eq!(OptLevel::from_str("4"), None);
        assert_eq!(OptLevel::from_str("x"), None);
        assert_eq!(OptLevel::from_str(""), None);
        assert_eq!(OptLevel::from_str("O2"), None); // Must be just "2", not "O2"
    }

    #[test]
    fn test_opt_level_default() {
        assert_eq!(OptLevel::default(), OptLevel::O0);
    }

    // -- DebugLevel tests --

    #[test]
    fn test_debug_level_from_str_valid() {
        assert_eq!(DebugLevel::from_str("0"), Some(DebugLevel::None));
        assert_eq!(DebugLevel::from_str("1"), Some(DebugLevel::LineTablesOnly));
        assert_eq!(DebugLevel::from_str("2"), Some(DebugLevel::Full));
    }

    #[test]
    fn test_debug_level_from_str_invalid() {
        assert_eq!(DebugLevel::from_str("3"), None);
        assert_eq!(DebugLevel::from_str("full"), None);
        assert_eq!(DebugLevel::from_str(""), None);
    }

    #[test]
    fn test_debug_level_default() {
        assert_eq!(DebugLevel::default(), DebugLevel::Full);
    }

    // -- EmitType tests --

    #[test]
    fn test_emit_type_from_str_valid() {
        assert_eq!(EmitType::from_str("obj"), Some(EmitType::Object));
        assert_eq!(EmitType::from_str("object"), Some(EmitType::Object));
        assert_eq!(EmitType::from_str("llvm-ir"), Some(EmitType::LlvmIr));
        assert_eq!(EmitType::from_str("ir"), Some(EmitType::LlvmIr));
        assert_eq!(EmitType::from_str("llvm-bc"), Some(EmitType::LlvmBc));
        assert_eq!(EmitType::from_str("bc"), Some(EmitType::LlvmBc));
        assert_eq!(EmitType::from_str("bitcode"), Some(EmitType::LlvmBc));
        assert_eq!(EmitType::from_str("asm"), Some(EmitType::Assembly));
        assert_eq!(EmitType::from_str("assembly"), Some(EmitType::Assembly));
    }

    #[test]
    fn test_emit_type_from_str_invalid() {
        assert_eq!(EmitType::from_str("exe"), None);
        assert_eq!(EmitType::from_str("wasm"), None);
        assert_eq!(EmitType::from_str(""), None);
    }

    // -- LinkMode tests --

    #[test]
    fn test_link_mode_from_str_valid() {
        assert_eq!(LinkMode::from_str("static"), Some(LinkMode::Static));
        assert_eq!(LinkMode::from_str("dynamic"), Some(LinkMode::Dynamic));
    }

    #[test]
    fn test_link_mode_from_str_invalid() {
        assert_eq!(LinkMode::from_str("shared"), None);
        assert_eq!(LinkMode::from_str(""), None);
    }

    #[test]
    fn test_link_mode_default() {
        assert_eq!(LinkMode::default(), LinkMode::Static);
    }

    // -- LtoMode tests --

    #[test]
    fn test_lto_mode_from_str_valid() {
        assert_eq!(LtoMode::from_str("off"), Some(LtoMode::Off));
        assert_eq!(LtoMode::from_str("false"), Some(LtoMode::Off));
        assert_eq!(LtoMode::from_str("no"), Some(LtoMode::Off));
        assert_eq!(LtoMode::from_str("thin"), Some(LtoMode::Thin));
        assert_eq!(LtoMode::from_str("full"), Some(LtoMode::Full));
        assert_eq!(LtoMode::from_str("true"), Some(LtoMode::Full));
        assert_eq!(LtoMode::from_str("yes"), Some(LtoMode::Full));
    }

    #[test]
    fn test_lto_mode_from_str_invalid() {
        assert_eq!(LtoMode::from_str("none"), None);
        assert_eq!(LtoMode::from_str(""), None);
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
}
