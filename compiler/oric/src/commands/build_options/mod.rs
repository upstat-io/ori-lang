//! Build command options and CLI argument parsing.
//!
//! Contains the `BuildOptions` struct, associated enums (`OptLevel`, `DebugLevel`,
//! `EmitType`, `LinkMode`, `LtoMode`), and the `parse_build_options()` CLI parser.

use std::path::PathBuf;

/// Build options parsed from command line arguments.
///
/// This struct naturally has many boolean flags representing independent
/// build configuration options (release mode, library type flags, verbose
/// output, etc.). These are not state machine candidates as they are
/// independent orthogonal settings.
#[derive(Debug, Clone)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "independent orthogonal CLI flags, not a state machine"
)]
pub struct BuildOptions {
    /// Build with optimizations (--release)
    pub release: bool,
    /// Target triple (--target=<triple>)
    pub target: Option<String>,
    /// Optimization level: 0, 1, 2, 3, s, z (--opt=<level>)
    pub opt_level: OptLevel,
    /// Whether `opt_level` was explicitly set via `--opt=`.
    pub opt_level_explicit: bool,
    /// Debug info level: 0, 1, 2 (--debug=<level>)
    pub debug_level: DebugLevel,
    /// Whether `debug_level` was explicitly set via `--debug=`.
    pub debug_level_explicit: bool,
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
    /// Whether `lto` was explicitly set via `--lto=`.
    pub lto_explicit: bool,
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
    /// Disable representation optimization (`--no-repr-opt` / `ORI_NO_REPR_OPT=1`).
    ///
    /// When true, `compute_repr_plan()` returns canonical-only representations
    /// (zero behavioral change vs pre-repr-opt pipeline). Used by §12 for
    /// dual-execution comparison and as a debugging kill switch.
    pub no_repr_opt: bool,
}

impl Default for BuildOptions {
    fn default() -> Self {
        Self {
            release: false,
            target: None,
            opt_level: OptLevel::O0,
            opt_level_explicit: false,
            debug_level: DebugLevel::Full,
            debug_level_explicit: false,
            output: None,
            out_dir: None,
            emit: None,
            lib: false,
            dylib: false,
            wasm: false,
            linker: None,
            link_mode: LinkMode::Static,
            lto: LtoMode::Off,
            lto_explicit: false,
            jobs: None,
            cpu: None,
            features: None,
            js_bindings: false,
            wasm_opt: false,
            verbose: false,
            no_repr_opt: false,
        }
    }
}

impl BuildOptions {
    /// Merge another `BuildOptions` into this one.
    ///
    /// For boolean flags, uses OR (true wins).
    /// For Option fields, takes the new value if present.
    /// For `--release`, applies its implied `opt_level` and `debug_level`
    /// as defaults; explicit `--opt=`/`--debug=`/`--lto=` always win.
    pub fn merge(&mut self, other: &Self) {
        // --release sets defaults for opt_level and debug_level
        if other.release {
            self.release = true;
            // Only apply --release defaults if not already explicitly overridden
            if !self.opt_level_explicit {
                self.opt_level = other.opt_level;
            }
            if !self.debug_level_explicit {
                self.debug_level = other.debug_level;
            }
        }

        // Explicit --opt=, --debug=, --lto= always override
        if other.opt_level_explicit {
            self.opt_level = other.opt_level;
            self.opt_level_explicit = true;
        }
        if other.debug_level_explicit {
            self.debug_level = other.debug_level;
            self.debug_level_explicit = true;
        }
        if other.lto_explicit {
            self.lto = other.lto;
            self.lto_explicit = true;
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
        self.no_repr_opt |= other.no_repr_opt;
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
#[expect(
    clippy::cognitive_complexity,
    reason = "linear if/else CLI flag parser"
)]
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
                options.opt_level_explicit = true;
            } else {
                eprintln!("warning: unknown optimization level '{level}', using O0");
            }
        } else if let Some(level) = arg.strip_prefix("--debug=") {
            if let Some(dbg) = DebugLevel::parse(level) {
                options.debug_level = dbg;
                options.debug_level_explicit = true;
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
                options.lto_explicit = true;
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
        } else if arg == "--no-repr-opt" {
            options.no_repr_opt = true;
        }
    }

    // Also check ORI_NO_REPR_OPT environment variable.
    // Uses strict value parsing: only "1"/"true"/"yes". (TPR-01-030)
    if ori_repr::NarrowingPolicy::env_disabled() {
        options.no_repr_opt = true;
    }

    // Handle -o without = (next arg is the path)
    // This is handled in the caller since it requires peeking ahead

    options
}

#[cfg(test)]
mod tests;
