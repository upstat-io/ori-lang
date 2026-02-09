//! Codegen Problem Types
//!
//! Structured error types for the codegen pipeline (ARC analysis + LLVM backend).
//!
//! # Design
//!
//! `CodegenProblem` is deliberately **separate** from the main [`super::Problem`]
//! enum. The `Problem` enum requires Salsa-compatible derives (`Clone, Eq,
//! PartialEq, Hash, Debug`). Codegen errors are terminal (post-Salsa) and only
//! need `Clone + Debug`. They merge with front-end diagnostics at the rendering
//! stage only.
//!
//! Each variant carries all context needed to produce a rich [`Diagnostic`] via
//! [`CodegenProblem::into_diagnostic`].

use ori_diagnostic::{Diagnostic, ErrorCode};
use ori_ir::Span;

/// Problem encountered during codegen (ARC analysis or LLVM backend).
///
/// Variants map to error codes E4001–E4003 (ARC) and E5001–E5009 (LLVM).
#[derive(Clone, Debug)]
pub enum CodegenProblem {
    // ── ARC Analysis (E4xxx) ────────────────────────────────────────
    /// An expression kind not yet supported for ARC IR lowering.
    ArcUnsupportedExpr { kind: &'static str, span: Span },

    /// A pattern kind not yet supported for ARC IR lowering.
    ArcUnsupportedPattern { kind: &'static str, span: Span },

    /// An internal error (invariant violation) during ARC lowering.
    ArcInternalError { message: String, span: Span },

    // ── LLVM Verification (E5001) ───────────────────────────────────
    /// LLVM module verification failed — indicates a compiler bug.
    VerificationFailed { message: String },

    // ── Optimization (E5002) ────────────────────────────────────────
    /// Optimization pipeline failed.
    OptimizationFailed { pipeline: String, message: String },

    // ── Emission (E5003) ────────────────────────────────────────────
    /// Object/assembly/bitcode/IR emission failed.
    EmissionFailed {
        format: String,
        path: String,
        message: String,
    },

    // ── Target (E5004) ──────────────────────────────────────────────
    /// Target triple not supported or configuration failed.
    TargetNotSupported { triple: String, message: String },

    // ── Runtime (E5005) ─────────────────────────────────────────────
    /// Runtime library (libori_rt.a) not found.
    RuntimeNotFound { search_paths: Vec<String> },

    // ── Linker (E5006) ──────────────────────────────────────────────
    /// Linker executable not found.
    LinkerNotFound { linker: String, message: String },

    /// Linking failed with an error.
    LinkFailed {
        command: String,
        exit_code: Option<i32>,
        stderr: String,
    },

    // ── Debug Info (E5007) ──────────────────────────────────────────
    /// Debug info creation failed.
    DebugInfoFailed { message: String },

    // ── WASM (E5008) ────────────────────────────────────────────────
    /// WASM-specific error.
    WasmError { message: String },

    // ── Module Config (E5009) ───────────────────────────────────────
    /// Module target configuration failed.
    ModuleConfigFailed { message: String },
}

impl CodegenProblem {
    /// Convert this problem into a [`Diagnostic`].
    pub fn into_diagnostic(&self) -> Diagnostic {
        match self {
            // ── ARC (E4xxx) ─────────────────────────────────────
            Self::ArcUnsupportedExpr { kind, span } => Diagnostic::warning(ErrorCode::E4001)
                .with_message(format!(
                    "expression kind '{kind}' is not yet supported in ARC IR lowering"
                ))
                .with_label(*span, "this expression")
                .with_note(
                    "ARC analysis will skip this expression; RC operations may not be optimized",
                ),

            Self::ArcUnsupportedPattern { kind, span } => Diagnostic::warning(ErrorCode::E4002)
                .with_message(format!(
                    "pattern kind '{kind}' is not yet supported in ARC IR lowering"
                ))
                .with_label(*span, "this pattern")
                .with_note(
                    "ARC analysis will skip this pattern; RC operations may not be optimized",
                ),

            Self::ArcInternalError { message, span } => Diagnostic::error(ErrorCode::E4003)
                .with_message(format!("ARC internal error: {message}"))
                .with_label(*span, "while lowering this expression")
                .with_note(
                    "this is likely a compiler bug — please report it at \
                         https://github.com/oriproject/ori/issues",
                ),

            // ── Verification (E5001) ────────────────────────────
            Self::VerificationFailed { message } => Diagnostic::error(ErrorCode::E5001)
                .with_message("LLVM module verification failed")
                .with_note(format!("LLVM says: {message}"))
                .with_note("this is a compiler bug — the generated LLVM IR is malformed"),

            // ── Optimization (E5002) ────────────────────────────
            Self::OptimizationFailed { pipeline, message } => Diagnostic::error(ErrorCode::E5002)
                .with_message(format!("optimization pipeline '{pipeline}' failed"))
                .with_note(format!("LLVM says: {message}"))
                .with_suggestion("try compiling with --opt=0 to bypass optimization"),

            // ── Emission (E5003) ─────────────────────────────────
            Self::EmissionFailed {
                format,
                path,
                message,
            } => Diagnostic::error(ErrorCode::E5003)
                .with_message(format!("failed to emit {format} to '{path}'"))
                .with_note(format!("LLVM says: {message}"))
                .with_suggestion("check that the output directory exists and is writable"),

            // ── Target (E5004) ───────────────────────────────────
            Self::TargetNotSupported { triple, message } => Diagnostic::error(ErrorCode::E5004)
                .with_message(format!("target '{triple}' is not supported"))
                .with_note(message.clone()),

            // ── Runtime (E5005) ──────────────────────────────────
            Self::RuntimeNotFound { search_paths } => {
                let mut diag = Diagnostic::error(ErrorCode::E5005)
                    .with_message("runtime library (libori_rt.a) not found");

                if !search_paths.is_empty() {
                    let paths = search_paths.join(", ");
                    diag = diag.with_note(format!("searched: {paths}"));
                }

                diag.with_suggestion(
                    "build the runtime with `cargo bl` or `cargo build -p ori_rt --release`",
                )
            }

            // ── Linker (E5006) ───────────────────────────────────
            Self::LinkerNotFound { linker, message } => Diagnostic::error(ErrorCode::E5006)
                .with_message(format!("linker '{linker}' not found"))
                .with_note(message.clone())
                .with_suggestion("install a C toolchain (gcc/clang) or specify --linker=<path>"),

            Self::LinkFailed {
                command,
                exit_code,
                stderr,
            } => {
                let code_msg = match exit_code {
                    Some(code) => format!(" (exit code {code})"),
                    None => String::new(),
                };
                let mut diag = Diagnostic::error(ErrorCode::E5006)
                    .with_message(format!("linking failed{code_msg}"));
                if !stderr.is_empty() {
                    diag = diag.with_note(format!("linker output:\n{stderr}"));
                }
                diag = diag.with_note(format!("command: {command}"));
                diag
            }

            // ── Debug Info (E5007) ───────────────────────────────
            Self::DebugInfoFailed { message } => Diagnostic::error(ErrorCode::E5007)
                .with_message(format!("debug info creation failed: {message}"))
                .with_suggestion("try compiling with --debug=0 to disable debug info"),

            // ── WASM (E5008) ─────────────────────────────────────
            Self::WasmError { message } => Diagnostic::error(ErrorCode::E5008)
                .with_message(format!("WebAssembly error: {message}")),

            // ── Module Config (E5009) ────────────────────────────
            Self::ModuleConfigFailed { message } => Diagnostic::error(ErrorCode::E5009)
                .with_message(format!("module configuration failed: {message}"))
                .with_note("failed to apply target settings to LLVM module"),
        }
    }

    /// Returns `true` if this is an error (vs. a warning).
    ///
    /// ARC unsupported expr/pattern are warnings; everything else is an error.
    pub fn is_error(&self) -> bool {
        !matches!(
            self,
            Self::ArcUnsupportedExpr { .. } | Self::ArcUnsupportedPattern { .. }
        )
    }
}

// ── From impls ──────────────────────────────────────────────────────

impl From<ori_arc::ArcProblem> for CodegenProblem {
    fn from(problem: ori_arc::ArcProblem) -> Self {
        match problem {
            ori_arc::ArcProblem::UnsupportedExpr { kind, span } => {
                Self::ArcUnsupportedExpr { kind, span }
            }
            ori_arc::ArcProblem::UnsupportedPattern { kind, span } => {
                Self::ArcUnsupportedPattern { kind, span }
            }
            ori_arc::ArcProblem::InternalError { message, span } => {
                Self::ArcInternalError { message, span }
            }
        }
    }
}

impl From<ori_llvm::aot::TargetError> for CodegenProblem {
    fn from(err: ori_llvm::aot::TargetError) -> Self {
        use ori_llvm::aot::TargetError;
        match err {
            TargetError::UnsupportedTarget { triple, supported } => Self::TargetNotSupported {
                triple,
                message: format!("supported targets: {}", supported.join(", ")),
            },
            TargetError::InitializationFailed(msg) => Self::TargetNotSupported {
                triple: String::new(),
                message: format!("LLVM target initialization failed: {msg}"),
            },
            TargetError::TargetMachineCreationFailed(msg) => Self::TargetNotSupported {
                triple: String::new(),
                message: format!("failed to create target machine: {msg}"),
            },
            TargetError::InvalidTripleFormat { triple, reason } => Self::TargetNotSupported {
                triple,
                message: format!("invalid format: {reason}"),
            },
            TargetError::InvalidCpu { cpu, target } => Self::TargetNotSupported {
                triple: target,
                message: format!("invalid CPU '{cpu}'"),
            },
            TargetError::InvalidFeature { feature, reason } => Self::TargetNotSupported {
                triple: String::new(),
                message: format!("invalid feature '{feature}': {reason}"),
            },
        }
    }
}

impl From<ori_llvm::aot::EmitError> for CodegenProblem {
    fn from(err: ori_llvm::aot::EmitError) -> Self {
        use ori_llvm::aot::EmitError;
        match err {
            EmitError::TargetMachine(te) => te.into(),
            EmitError::ModuleConfiguration(te) => Self::ModuleConfigFailed {
                message: te.to_string(),
            },
            EmitError::ObjectEmission { path, message } => Self::EmissionFailed {
                format: "object".into(),
                path,
                message,
            },
            EmitError::AssemblyEmission { path, message } => Self::EmissionFailed {
                format: "assembly".into(),
                path,
                message,
            },
            EmitError::BitcodeEmission { path, message } => Self::EmissionFailed {
                format: "bitcode".into(),
                path,
                message,
            },
            EmitError::LlvmIrEmission { path, message } => Self::EmissionFailed {
                format: "LLVM IR".into(),
                path,
                message,
            },
            EmitError::InvalidPath { path, reason } => Self::EmissionFailed {
                format: "output".into(),
                path,
                message: reason,
            },
        }
    }
}

impl From<ori_llvm::aot::OptimizationError> for CodegenProblem {
    fn from(err: ori_llvm::aot::OptimizationError) -> Self {
        use ori_llvm::aot::OptimizationError;
        match err {
            OptimizationError::VerificationFailed { message } => {
                Self::VerificationFailed { message }
            }
            OptimizationError::PassBuilderOptionsCreationFailed => Self::OptimizationFailed {
                pipeline: String::new(),
                message: "failed to create pass builder options".into(),
            },
            OptimizationError::PassesFailed { message } => Self::OptimizationFailed {
                pipeline: String::new(),
                message,
            },
            OptimizationError::InvalidPipeline { pipeline, message } => {
                Self::OptimizationFailed { pipeline, message }
            }
            OptimizationError::BitcodeWriteFailed { path } => Self::EmissionFailed {
                format: "bitcode".into(),
                path,
                message: "write failed during LTO pre-link".into(),
            },
        }
    }
}

impl From<ori_llvm::aot::ModulePipelineError> for CodegenProblem {
    fn from(err: ori_llvm::aot::ModulePipelineError) -> Self {
        use ori_llvm::aot::ModulePipelineError;
        match err {
            ModulePipelineError::Verification(message) => Self::VerificationFailed { message },
            ModulePipelineError::Optimization(opt_err) => opt_err.into(),
            ModulePipelineError::Emission(emit_err) => emit_err.into(),
        }
    }
}

impl From<ori_llvm::aot::LinkerError> for CodegenProblem {
    fn from(err: ori_llvm::aot::LinkerError) -> Self {
        use ori_llvm::aot::LinkerError;
        match err {
            LinkerError::LinkerNotFound { linker, message } => {
                Self::LinkerNotFound { linker, message }
            }
            LinkerError::LinkFailed {
                linker: _,
                exit_code,
                stderr,
                command,
            } => Self::LinkFailed {
                command,
                exit_code,
                stderr,
            },
            LinkerError::ResponseFileError { path, message } => Self::LinkFailed {
                command: String::new(),
                exit_code: None,
                stderr: format!("failed to create response file '{path}': {message}"),
            },
            LinkerError::InvalidConfig { message } => Self::ModuleConfigFailed { message },
            LinkerError::IoError { message } => Self::LinkFailed {
                command: String::new(),
                exit_code: None,
                stderr: format!("I/O error: {message}"),
            },
            LinkerError::UnsupportedTarget { triple } => Self::TargetNotSupported {
                triple: triple.clone(),
                message: format!("linker does not support target '{triple}'"),
            },
        }
    }
}

impl From<ori_llvm::aot::DebugInfoError> for CodegenProblem {
    fn from(err: ori_llvm::aot::DebugInfoError) -> Self {
        Self::DebugInfoFailed {
            message: err.to_string(),
        }
    }
}

impl From<ori_llvm::aot::WasmError> for CodegenProblem {
    fn from(err: ori_llvm::aot::WasmError) -> Self {
        Self::WasmError {
            message: err.to_string(),
        }
    }
}

impl From<ori_llvm::aot::RuntimeNotFound> for CodegenProblem {
    fn from(err: ori_llvm::aot::RuntimeNotFound) -> Self {
        Self::RuntimeNotFound {
            search_paths: err
                .searched_paths
                .iter()
                .map(|p| p.display().to_string())
                .collect(),
        }
    }
}

// ── CodegenDiagnostics ──────────────────────────────────────────────

/// Accumulator for non-fatal codegen diagnostics.
///
/// ARC warnings are non-fatal and should be accumulated rather than
/// immediately terminating compilation. This struct collects them and
/// provides batch rendering.
#[derive(Clone, Debug, Default)]
pub struct CodegenDiagnostics {
    problems: Vec<CodegenProblem>,
}

impl CodegenDiagnostics {
    /// Create a new empty diagnostics accumulator.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a problem to the accumulator.
    pub fn push(&mut self, problem: CodegenProblem) {
        self.problems.push(problem);
    }

    /// Add all ARC problems from a lowering pass.
    pub fn add_arc_problems(&mut self, problems: &[ori_arc::ArcProblem]) {
        self.problems
            .extend(problems.iter().cloned().map(CodegenProblem::from));
    }

    /// Returns `true` if any problem is an error (not just warnings).
    pub fn has_errors(&self) -> bool {
        self.problems.iter().any(CodegenProblem::is_error)
    }

    /// Render all accumulated problems as diagnostics.
    pub fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.problems
            .iter()
            .map(CodegenProblem::into_diagnostic)
            .collect()
    }

    /// Returns `true` if there are no accumulated problems.
    pub fn is_empty(&self) -> bool {
        self.problems.is_empty()
    }
}

// ── Emit helper ─────────────────────────────────────────────────────

/// Emit a codegen error diagnostic and exit.
///
/// Converts any error type that implements `Into<CodegenProblem>` into a
/// structured diagnostic, emits it via the terminal emitter, and exits
/// with code 1.
pub fn report_codegen_error(problem: impl Into<CodegenProblem>) -> ! {
    use ori_diagnostic::emitter::{ColorMode, DiagnosticEmitter, TerminalEmitter};

    let diag = problem.into().into_diagnostic();
    let is_tty = std::io::IsTerminal::is_terminal(&std::io::stderr());
    let mut emitter = TerminalEmitter::with_color_mode(std::io::stderr(), ColorMode::Auto, is_tty);
    emitter.emit(&diag);
    emitter.flush();
    std::process::exit(1);
}

/// Emit accumulated codegen diagnostics (warnings and errors).
///
/// Returns `true` if any errors were emitted (callers should abort).
pub fn emit_codegen_diagnostics(diagnostics: CodegenDiagnostics) -> bool {
    if diagnostics.is_empty() {
        return false;
    }

    use ori_diagnostic::emitter::{ColorMode, DiagnosticEmitter, TerminalEmitter};

    let has_errors = diagnostics.has_errors();
    let diags = diagnostics.into_diagnostics();

    let is_tty = std::io::IsTerminal::is_terminal(&std::io::stderr());
    let mut emitter = TerminalEmitter::with_color_mode(std::io::stderr(), ColorMode::Auto, is_tty);
    for diag in &diags {
        emitter.emit(diag);
    }
    emitter.flush();

    has_errors
}

#[cfg(test)]
mod tests {
    use super::*;
    use ori_diagnostic::Severity;

    #[test]
    fn test_arc_problem_from() {
        let problem = ori_arc::ArcProblem::UnsupportedExpr {
            kind: "Await",
            span: Span::new(10, 20),
        };
        let codegen: CodegenProblem = problem.into();
        assert!(!codegen.is_error()); // warnings, not errors
        let diag = codegen.into_diagnostic();
        assert_eq!(diag.code, ErrorCode::E4001);
        assert_eq!(diag.severity, Severity::Warning);
        assert!(diag.message.contains("Await"));
    }

    #[test]
    fn test_arc_internal_error_is_error() {
        let problem = ori_arc::ArcProblem::InternalError {
            message: "bad invariant".into(),
            span: Span::new(0, 5),
        };
        let codegen: CodegenProblem = problem.into();
        assert!(codegen.is_error());
        let diag = codegen.into_diagnostic();
        assert_eq!(diag.code, ErrorCode::E4003);
        assert!(diag.message.contains("internal error"));
    }

    #[test]
    fn test_target_error_from() {
        let err = ori_llvm::aot::TargetError::UnsupportedTarget {
            triple: "mips-unknown-linux".into(),
            supported: vec!["x86_64", "aarch64"],
        };
        let codegen: CodegenProblem = err.into();
        let diag = codegen.into_diagnostic();
        assert_eq!(diag.code, ErrorCode::E5004);
        assert!(diag.message.contains("mips-unknown-linux"));
    }

    #[test]
    fn test_verification_failed_diagnostic() {
        let problem = CodegenProblem::VerificationFailed {
            message: "expected i64, got i32".into(),
        };
        let diag = problem.into_diagnostic();
        assert_eq!(diag.code, ErrorCode::E5001);
        assert!(diag.notes.iter().any(|n| n.contains("compiler bug")));
    }

    #[test]
    fn test_runtime_not_found_diagnostic() {
        let problem = CodegenProblem::RuntimeNotFound {
            search_paths: vec!["/usr/lib".into(), "/opt/ori/lib".into()],
        };
        let diag = problem.into_diagnostic();
        assert_eq!(diag.code, ErrorCode::E5005);
        assert!(diag.notes.iter().any(|n| n.contains("/usr/lib")));
        assert!(diag.suggestions.iter().any(|s| s.contains("cargo bl")));
    }

    #[test]
    fn test_linker_failed_diagnostic() {
        let problem = CodegenProblem::LinkFailed {
            command: "cc -o output main.o".into(),
            exit_code: Some(1),
            stderr: "undefined reference to `main`".into(),
        };
        let diag = problem.into_diagnostic();
        assert_eq!(diag.code, ErrorCode::E5006);
        assert!(diag.message.contains("exit code 1"));
        assert!(diag.notes.iter().any(|n| n.contains("undefined reference")));
    }

    #[test]
    fn test_codegen_diagnostics_accumulator() {
        let mut acc = CodegenDiagnostics::new();
        assert!(acc.is_empty());
        assert!(!acc.has_errors());

        acc.push(CodegenProblem::ArcUnsupportedExpr {
            kind: "Await",
            span: Span::new(0, 5),
        });
        assert!(!acc.has_errors()); // Only warnings so far

        acc.push(CodegenProblem::VerificationFailed {
            message: "bad".into(),
        });
        assert!(acc.has_errors()); // Now has an error

        let diags = acc.into_diagnostics();
        assert_eq!(diags.len(), 2);
        assert_eq!(diags[0].code, ErrorCode::E4001);
        assert_eq!(diags[1].code, ErrorCode::E5001);
    }

    #[test]
    fn test_module_pipeline_error_from() {
        let err = ori_llvm::aot::ModulePipelineError::Verification("use of undefined value".into());
        let codegen: CodegenProblem = err.into();
        let diag = codegen.into_diagnostic();
        assert_eq!(diag.code, ErrorCode::E5001);
    }

    #[test]
    fn test_optimization_error_from() {
        let err = ori_llvm::aot::OptimizationError::PassesFailed {
            message: "broken pass".into(),
        };
        let codegen: CodegenProblem = err.into();
        let diag = codegen.into_diagnostic();
        assert_eq!(diag.code, ErrorCode::E5002);
    }

    #[test]
    fn test_emit_error_from() {
        let err = ori_llvm::aot::EmitError::ObjectEmission {
            path: "/tmp/test.o".into(),
            message: "permission denied".into(),
        };
        let codegen: CodegenProblem = err.into();
        let diag = codegen.into_diagnostic();
        assert_eq!(diag.code, ErrorCode::E5003);
        assert!(diag.message.contains("/tmp/test.o"));
    }

    #[test]
    fn test_wasm_error_from() {
        let err = ori_llvm::aot::WasmError::InvalidConfig {
            message: "bad config".into(),
        };
        let codegen: CodegenProblem = err.into();
        let diag = codegen.into_diagnostic();
        assert_eq!(diag.code, ErrorCode::E5008);
    }

    #[test]
    fn test_debug_info_error_from() {
        let err = ori_llvm::aot::DebugInfoError::Disabled;
        let codegen: CodegenProblem = err.into();
        let diag = codegen.into_diagnostic();
        assert_eq!(diag.code, ErrorCode::E5007);
    }

    #[test]
    fn test_linker_error_from() {
        let err = ori_llvm::aot::LinkerError::LinkerNotFound {
            linker: "lld".into(),
            message: "not in PATH".into(),
        };
        let codegen: CodegenProblem = err.into();
        let diag = codegen.into_diagnostic();
        assert_eq!(diag.code, ErrorCode::E5006);
        assert!(diag.message.contains("lld"));
    }

    #[test]
    fn test_runtime_not_found_from() {
        let err = ori_llvm::aot::RuntimeNotFound {
            searched_paths: vec!["/usr/lib".into()],
        };
        let codegen: CodegenProblem = err.into();
        let diag = codegen.into_diagnostic();
        assert_eq!(diag.code, ErrorCode::E5005);
    }
}
