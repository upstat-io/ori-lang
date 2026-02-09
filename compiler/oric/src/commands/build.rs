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
// Many independent orthogonal flags (see doc comment above) - not a state machine
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
    pub fn parse(s: &str) -> Option<Self> {
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
    pub fn parse(s: &str) -> Option<Self> {
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
    pub fn parse(s: &str) -> Option<Self> {
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
    pub fn parse(s: &str) -> Option<Self> {
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
    pub fn parse(s: &str) -> Option<Self> {
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
            if let Some(opt) = OptLevel::parse(level) {
                options.opt_level = opt;
            } else {
                eprintln!("warning: unknown optimization level '{level}', using O0");
            }
        } else if let Some(level) = arg.strip_prefix("--debug=") {
            if let Some(dbg) = DebugLevel::parse(level) {
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
            if let Some(e) = EmitType::parse(emit) {
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
            if let Some(mode) = LinkMode::parse(link) {
                options.link_mode = mode;
            } else {
                eprintln!("warning: unknown link mode '{link}', using static");
            }
        } else if let Some(lto) = arg.strip_prefix("--lto=") {
            if let Some(mode) = LtoMode::parse(lto) {
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
        build_file_multi(path, &content, options, start);
    } else {
        build_file_single(path, &content, options, start);
    }
}

/// Build a single Ori source file (no imports).
#[cfg(feature = "llvm")]
fn build_file_single(path: &str, content: &str, options: &BuildOptions, start: std::time::Instant) {
    use ori_llvm::aot::ObjectEmitter;
    use ori_llvm::inkwell::context::Context;
    use oric::{CompilerDb, SourceFile};
    use tempfile::TempDir;

    use super::compile_common::{check_source, compile_to_llvm};

    // Step 1: Parse and type-check the source file
    if options.verbose {
        eprintln!("  Compiling {path}...");
    }

    let db = CompilerDb::new();
    let file = SourceFile::new(&db, PathBuf::from(path), content.to_string());

    // Check for parse and type errors
    let Some((parse_result, type_result, pool)) = check_source(&db, file, path) else {
        std::process::exit(1)
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

    // Step 3: Generate LLVM IR
    let context = Context::create();
    let llvm_module = compile_to_llvm(&context, &db, &parse_result, &type_result, &pool, path);

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

    // Step 4: Build optimization config
    let opt_config = build_optimization_config(options);

    // Step 5: Determine output path
    let output_path = determine_output_path(path, options);

    // Step 6: Emit based on emit type (--emit flag)
    // For --emit, we still verify+optimize first, then emit in the requested format.
    if let Some(emit_type) = options.emit {
        if let Err(e) = ori_llvm::aot::optimize_module(&llvm_module, emitter.machine(), &opt_config)
        {
            eprintln!("error: optimization failed: {e}");
            std::process::exit(1);
        }
        emit_and_finish(
            &llvm_module,
            &emitter,
            &output_path,
            emit_type,
            options,
            start,
        );
        return;
    }

    // Step 7: Verify → optimize → emit object file via unified pipeline
    // Use tempfile for unique directory to avoid race conditions in parallel builds
    let temp_dir = match TempDir::new() {
        Ok(dir) => dir,
        Err(e) => {
            eprintln!("error: failed to create temp directory: {e}");
            std::process::exit(1);
        }
    };
    let module_name = Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("module");
    let obj_path = temp_dir.path().join(format!("{module_name}.o"));

    if options.verbose {
        eprintln!("  Emitting object to {}", obj_path.display());
    }

    if let Err(e) = emitter.verify_optimize_emit(
        &llvm_module,
        &opt_config,
        &obj_path,
        ori_llvm::aot::OutputFormat::Object,
    ) {
        eprintln!("error: pipeline failed: {e}");
        std::process::exit(1);
    }

    // Step 8: Link into executable
    // Note: temp_dir must stay alive until linking completes (auto-cleaned on drop)
    link_and_finish(&[obj_path], &output_path, &target, options, start);
}

/// Build a multi-file Ori program (with imports).
///
/// This builds all dependent modules in topological order and links them together.
#[cfg(feature = "llvm")]
fn build_file_multi(path: &str, _content: &str, options: &BuildOptions, start: std::time::Instant) {
    use ori_llvm::aot::{build_dependency_graph, Mangler};
    use oric::CompilerDb;
    use tempfile::TempDir;

    // Step 1: Build dependency graph
    if options.verbose {
        eprintln!("  Building dependency graph...");
    }

    let entry_path = Path::new(path);
    let entry_canonical = entry_path
        .canonicalize()
        .unwrap_or_else(|_| entry_path.to_path_buf());

    // Import resolver that converts relative paths to absolute paths
    let resolve_import = |current: &Path, import: &str| -> Result<PathBuf, String> {
        let dir = current.parent().unwrap_or(Path::new("."));
        let resolved = dir.join(import);
        let with_ext = if resolved.extension().is_none() {
            resolved.with_extension("ori")
        } else {
            resolved
        };

        if with_ext.exists() {
            Ok(with_ext)
        } else {
            Err(format!(
                "cannot find '{}' at '{}'",
                import,
                with_ext.display()
            ))
        }
    };

    let dep_result = match build_dependency_graph(&entry_canonical, resolve_import) {
        Ok(result) => result,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    };

    if options.verbose {
        eprintln!(
            "  Found {} files to compile",
            dep_result.compilation_order.len()
        );
        for (i, p) in dep_result.compilation_order.iter().enumerate() {
            eprintln!("    {}: {}", i + 1, p.display());
        }
    }

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

    // Step 3: Compile each module in topological order
    // Use tempfile for unique directory to avoid race conditions in parallel builds
    let temp_dir = match TempDir::new() {
        Ok(dir) => dir,
        Err(e) => {
            eprintln!("error: failed to create temp directory: {e}");
            std::process::exit(1);
        }
    };
    let obj_dir = temp_dir.path().to_path_buf();

    let db = CompilerDb::new();
    let mangler = Mangler::new();
    let opt_config = build_optimization_config(options);

    // Set up ARC IR cache for incremental compilation
    let arc_cache = {
        let cache_dir = obj_dir.join("arc_cache");
        match ori_llvm::aot::incremental::ArcIrCache::new(&cache_dir) {
            Ok(cache) => {
                if options.verbose {
                    eprintln!("  ARC IR cache enabled at {}", cache_dir.display());
                }
                Some(cache)
            }
            Err(e) => {
                if options.verbose {
                    eprintln!("  ARC IR cache disabled: {e}");
                }
                None
            }
        }
    };

    // Create compilation context (avoids passing many parameters to helper)
    let compile_ctx = ModuleCompileContext {
        db: &db,
        target: &target,
        opt_config: &opt_config,
        mangler: &mangler,
        graph: &dep_result.graph,
        base_dir: &dep_result.base_dir,
        obj_dir: &obj_dir,
        verbose: options.verbose,
        arc_cache,
        module_hash: None, // Per-module hashes computed below if needed
    };

    // Pre-allocate vectors with known capacity to avoid reallocation
    let module_count = dep_result.compilation_order.len();
    let mut compiled_modules: Vec<CompiledModuleInfo> = Vec::with_capacity(module_count);
    let mut object_files: Vec<PathBuf> = Vec::with_capacity(module_count);

    // Compile each module in topological order
    for source_path in &dep_result.compilation_order {
        match compile_single_module(&compile_ctx, source_path, &compiled_modules) {
            Some((obj_path, module_info)) => {
                compiled_modules.push(module_info);
                object_files.push(obj_path);
            }
            None => std::process::exit(1),
        }
    }

    // Step 4: LTO merge (if enabled) or direct linking
    let is_lto = !matches!(options.lto, LtoMode::Off);
    let final_object_files = if is_lto && object_files.len() > 1 {
        // LTO: merge bitcode files → run LTO pipeline → emit single object
        use ori_llvm::aot::ObjectEmitter;
        use ori_llvm::inkwell::context::Context;
        use ori_llvm::inkwell::module::Module;

        if options.verbose {
            eprintln!("  Running LTO merge ({} modules)...", object_files.len());
        }

        let lto_context = Context::create();
        // Load first bitcode as the base module
        let merged_module = Module::parse_bitcode_from_path(&object_files[0], &lto_context)
            .unwrap_or_else(|e| {
                eprintln!(
                    "error: failed to load bitcode '{}': {e}",
                    object_files[0].display()
                );
                std::process::exit(1);
            });

        // Link remaining bitcode modules into the base
        for bc_path in &object_files[1..] {
            let other =
                Module::parse_bitcode_from_path(bc_path, &lto_context).unwrap_or_else(|e| {
                    eprintln!("error: failed to load bitcode '{}': {e}", bc_path.display());
                    std::process::exit(1);
                });
            if let Err(e) = merged_module.link_in_module(other) {
                eprintln!("error: LTO module linking failed: {e}");
                std::process::exit(1);
            }
        }

        // Configure merged module for target
        let emitter = match ObjectEmitter::new(&target) {
            Ok(e) => e,
            Err(e) => {
                eprintln!("error: failed to create object emitter: {e}");
                std::process::exit(1);
            }
        };
        if let Err(e) = emitter.configure_module(&merged_module) {
            eprintln!("error: failed to configure merged module: {e}");
            std::process::exit(1);
        }

        // Run LTO pipeline on merged module
        let opt_config = build_optimization_config(options);
        if let Err(e) =
            ori_llvm::aot::run_lto_pipeline(&merged_module, emitter.machine(), &opt_config)
        {
            eprintln!("error: LTO pipeline failed: {e}");
            std::process::exit(1);
        }

        // Emit final object
        let final_obj = obj_dir.join("merged_lto.o");
        if let Err(e) = emitter.emit_object(&merged_module, &final_obj) {
            eprintln!("error: failed to emit LTO object: {e}");
            std::process::exit(1);
        }

        if options.verbose {
            eprintln!("  LTO merge complete → {}", final_obj.display());
        }

        vec![final_obj]
    } else {
        object_files
    };

    // Note: temp_dir must stay alive until linking completes (auto-cleaned on drop)
    let output_path = determine_output_path(path, options);
    link_and_finish(&final_object_files, &output_path, &target, options, start);

    // temp_dir automatically cleans up when it goes out of scope
    drop(temp_dir);
}

/// Context for compiling a single module in multi-file compilation.
#[cfg(feature = "llvm")]
struct ModuleCompileContext<'a> {
    db: &'a oric::CompilerDb,
    target: &'a ori_llvm::aot::TargetConfig,
    opt_config: &'a ori_llvm::aot::OptimizationConfig,
    mangler: &'a ori_llvm::aot::Mangler,
    graph: &'a ori_llvm::aot::incremental::deps::DependencyGraph,
    base_dir: &'a Path,
    obj_dir: &'a Path,
    verbose: bool,
    /// Optional ARC IR cache for incremental compilation.
    arc_cache: Option<ori_llvm::aot::incremental::ArcIrCache>,
    /// Per-module content hashes for ARC cache keying.
    module_hash: Option<rustc_hash::FxHashMap<PathBuf, ori_llvm::aot::incremental::ContentHash>>,
}

/// Information about a compiled module, including its function signatures.
#[cfg(feature = "llvm")]
struct CompiledModuleInfo {
    /// Path to the source file.
    path: PathBuf,
    /// Module name for mangling.
    #[allow(dead_code)] // Kept for debugging and potential future use
    module_name: String,
    /// Public function signatures (`mangled_name`, `param_types`, `return_type`).
    /// These are the actual types from type checking, not defaults.
    /// The mangled name is pre-computed to avoid needing the interner later.
    public_functions: Vec<(String, Vec<ori_types::Idx>, ori_types::Idx)>,
}

/// Compile a single module to an object file.
///
/// Returns (`object_path`, `CompiledModuleInfo`) on success.
#[cfg(feature = "llvm")]
fn compile_single_module(
    ctx: &ModuleCompileContext<'_>,
    source_path: &Path,
    compiled_modules: &[CompiledModuleInfo],
) -> Option<(PathBuf, CompiledModuleInfo)> {
    use ori_llvm::aot::{derive_module_name, ObjectEmitter};
    use ori_llvm::inkwell::context::Context;
    use oric::SourceFile;

    use super::compile_common::{check_source, compile_to_llvm_with_imports};

    let source_path_str = source_path.to_string_lossy();

    if ctx.verbose {
        eprintln!("  Compiling {}...", source_path.display());
    }

    // Read source content
    let content = match std::fs::read_to_string(source_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: failed to read '{}': {}", source_path.display(), e);
            return None;
        }
    };

    // Derive module name
    let module_name = derive_module_name(source_path, Some(ctx.base_dir));

    // Load and check the source
    let file = SourceFile::new(ctx.db, source_path.to_path_buf(), content);
    let (parse_result, type_result, pool) = check_source(ctx.db, file, &source_path_str)?;

    // Extract public function signatures with actual types from type checking
    let public_functions = extract_public_function_types(
        &parse_result,
        &type_result,
        &module_name,
        ctx.mangler,
        ctx.db,
    );

    // Build list of imported functions for this module
    let imported_functions = build_import_infos(
        source_path,
        ctx.graph,
        compiled_modules,
        ctx.base_dir,
        ctx.mangler,
    );

    // Compile to LLVM IR (with ARC cache if available)
    let context = Context::create();
    let llvm_module = compile_to_llvm_with_imports(
        &context,
        ctx.db,
        &parse_result,
        &type_result,
        &pool,
        &source_path_str,
        &module_name,
        &imported_functions,
        ctx.arc_cache.as_ref(),
        ctx.module_hash
            .as_ref()
            .and_then(|hashes| hashes.get(source_path).copied()),
    );

    // Configure module for target
    let emitter = match ObjectEmitter::new(ctx.target) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("error: failed to create object emitter: {e}");
            return None;
        }
    };

    if let Err(e) = emitter.configure_module(&llvm_module) {
        eprintln!("error: failed to configure module: {e}");
        return None;
    }

    // Verify and optimize module
    // When LTO is enabled, the config's pipeline_string() automatically
    // returns the pre-link variant (e.g., thinlto-pre-link<O2>)
    let is_lto = !matches!(ctx.opt_config.lto, ori_llvm::aot::LtoMode::Off);

    if is_lto {
        // LTO: run pre-link pipeline and emit bitcode
        let bc_path = ctx
            .obj_dir
            .join(format!("{}.bc", module_name.replace('$', "_")));
        if ctx.verbose {
            eprintln!(
                "    Emitting bitcode to {} (LTO pre-link)",
                bc_path.display()
            );
        }
        if let Err(e) = ori_llvm::aot::prelink_and_emit_bitcode(
            &llvm_module,
            emitter.machine(),
            ctx.opt_config,
            &bc_path,
        ) {
            eprintln!("error: LTO pre-link failed: {e}");
            return None;
        }
        let obj_path = bc_path; // Return bitcode path in place of object path
        return Some((
            obj_path,
            CompiledModuleInfo {
                path: source_path.to_path_buf(),
                module_name,
                public_functions,
            },
        ));
    }

    // Non-LTO: verify → optimize → emit object file via unified pipeline
    let obj_path = ctx
        .obj_dir
        .join(format!("{}.o", module_name.replace('$', "_")));
    if ctx.verbose {
        eprintln!("    Emitting object to {}", obj_path.display());
    }

    if let Err(e) = emitter.verify_optimize_emit(
        &llvm_module,
        ctx.opt_config,
        &obj_path,
        ori_llvm::aot::OutputFormat::Object,
    ) {
        eprintln!("error: pipeline failed: {e}");
        return None;
    }

    let module_info = CompiledModuleInfo {
        path: source_path.to_path_buf(),
        module_name,
        public_functions,
    };

    Some((obj_path, module_info))
}

/// Extract public function signatures with actual types from a type-checked module.
///
/// Returns tuples of (`mangled_name`, `param_types`, `return_type`).
/// The mangled name is pre-computed to avoid needing the interner later.
#[cfg(feature = "llvm")]
fn extract_public_function_types(
    parse_result: &ori_parse::ParseOutput,
    type_result: &ori_types::TypeCheckResult,
    module_name: &str,
    mangler: &ori_llvm::aot::Mangler,
    db: &oric::CompilerDb,
) -> Vec<(String, Vec<ori_types::Idx>, ori_types::Idx)> {
    use oric::Db; // For interner() method

    let interner = db.interner();
    let mut public_functions = Vec::new();

    // Build a name-based lookup map because typed.functions is sorted by name
    // (for Salsa determinism) while module.functions is in source order.
    let sig_map: std::collections::HashMap<ori_ir::Name, &ori_types::FunctionSig> = type_result
        .typed
        .functions
        .iter()
        .map(|ft| (ft.name, ft))
        .collect();

    // Match parsed functions with their type-checked signatures by name
    for func in &parse_result.module.functions {
        if !func.visibility.is_public() {
            continue;
        }

        if let Some(func_sig) = sig_map.get(&func.name) {
            let func_name_str = interner.lookup(func.name);
            let mangled_name = mangler.mangle_function(module_name, func_name_str);

            public_functions.push((
                mangled_name,
                func_sig.param_types.clone(),
                func_sig.return_type,
            ));
        }
    }

    public_functions
}

/// Build import information for a module based on its dependencies.
///
/// Uses actual type information from already-compiled modules rather than
/// defaulting to INT. This ensures correct calling conventions for cross-module calls.
#[cfg(feature = "llvm")]
fn build_import_infos(
    source_path: &Path,
    graph: &ori_llvm::aot::incremental::deps::DependencyGraph,
    compiled_modules: &[CompiledModuleInfo],
    _base_dir: &Path,
    _mangler: &ori_llvm::aot::Mangler,
) -> Vec<super::compile_common::ImportedFunctionInfo> {
    let mut imported_functions = Vec::new();

    // Get the direct imports of this module
    let Some(imports) = graph.get_imports(source_path) else {
        return imported_functions;
    };

    // Build index once for O(1) lookups instead of O(n) linear scan per import
    let module_index: rustc_hash::FxHashMap<&Path, &CompiledModuleInfo> = compiled_modules
        .iter()
        .map(|m| (m.path.as_path(), m))
        .collect();

    for import_path in imports {
        // O(1) lookup using the index
        let Some(module_info) = module_index.get(import_path.as_path()) else {
            // Module not yet compiled - shouldn't happen in topological order
            eprintln!(
                "warning: import '{}' not found in compiled modules",
                import_path.display()
            );
            continue;
        };
        let module_info = *module_info;

        // Add each public function using the actual types from type checking
        // The mangled names are pre-computed when the module was compiled
        // Pre-allocate to avoid reallocations in the inner loop
        imported_functions.reserve(module_info.public_functions.len());
        for (mangled_name, param_types, return_type) in &module_info.public_functions {
            imported_functions.push(super::compile_common::ImportedFunctionInfo {
                mangled_name: mangled_name.clone(),
                param_types: param_types.clone(),
                return_type: *return_type,
            });
        }
    }

    imported_functions
}

/// Emit a module and finish (used for --emit flag).
#[cfg(feature = "llvm")]
fn emit_and_finish(
    llvm_module: &ori_llvm::inkwell::module::Module<'_>,
    emitter: &ori_llvm::aot::ObjectEmitter,
    output_path: &Path,
    emit_type: EmitType,
    options: &BuildOptions,
    start: std::time::Instant,
) {
    use ori_llvm::aot::OutputFormat;

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

    if let Err(e) = emitter.emit(llvm_module, &emit_path, format) {
        eprintln!("error: failed to emit: {e}");
        std::process::exit(1);
    }

    let elapsed = start.elapsed();
    if options.verbose {
        eprintln!("  Finished in {:.2}s", elapsed.as_secs_f64());
    }
}

/// Link object files and finish.
#[cfg(feature = "llvm")]
fn link_and_finish(
    object_files: &[PathBuf],
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
        objects: object_files.to_vec(), // Clone required: link_input takes ownership, caller may need objects for cleanup
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
        eprintln!("error: linking failed: {e}");
        std::process::exit(1);
    }

    let elapsed = start.elapsed();
    eprintln!(
        "  Finished {} in {:.2}s",
        output_path.display(),
        elapsed.as_secs_f64()
    );
}

/// Build command when LLVM feature is not enabled.
#[cfg(not(feature = "llvm"))]
pub fn build_file(_path: &str, _options: &BuildOptions) {
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
