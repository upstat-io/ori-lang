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

use crate::ir::Span;
use ori_diagnostic::{Diagnostic, ErrorCode};

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
    /// Convert this problem into a [`Diagnostic`], consuming `self`.
    ///
    /// Consumes the problem to avoid cloning owned `String` fields (messages,
    /// paths, stderr) into the diagnostic. Callers that need to inspect the
    /// problem after conversion should do so before calling this method.
    #[cold]
    pub fn into_diagnostic(self) -> Diagnostic {
        match self {
            // ── ARC (E4xxx) ─────────────────────────────────────
            Self::ArcUnsupportedExpr { kind, span } => Diagnostic::warning(ErrorCode::E4001)
                .with_message(format!(
                    "expression kind '{kind}' is not yet supported in ARC IR lowering"
                ))
                .with_label(span, "this expression")
                .with_note(
                    "ARC analysis will skip this expression; RC operations may not be optimized",
                ),

            Self::ArcUnsupportedPattern { kind, span } => Diagnostic::warning(ErrorCode::E4002)
                .with_message(format!(
                    "pattern kind '{kind}' is not yet supported in ARC IR lowering"
                ))
                .with_label(span, "this pattern")
                .with_note(
                    "ARC analysis will skip this pattern; RC operations may not be optimized",
                ),

            Self::ArcInternalError { message, span } => Diagnostic::error(ErrorCode::E4003)
                .with_message(format!("ARC internal error: {message}"))
                .with_label(span, "while lowering this expression")
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
                .with_note(message),

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
                .with_note(message)
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
            LinkerError::UnsupportedTarget { triple } => {
                let message = format!("linker does not support target '{triple}'");
                Self::TargetNotSupported { triple, message }
            }
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

    /// Render all accumulated problems as diagnostics, consuming `self`.
    pub fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.problems
            .into_iter()
            .map(CodegenProblem::into_diagnostic)
            .collect()
    }

    /// Returns `true` if there are no accumulated problems.
    pub fn is_empty(&self) -> bool {
        self.problems.is_empty()
    }
}

// ── Emit helper ─────────────────────────────────────────────────────

/// Render a codegen error as a diagnostic and emit it to stderr.
///
/// Converts any error type that implements `Into<CodegenProblem>` into a
/// structured diagnostic and emits it via the terminal emitter.
///
/// This function does NOT call `process::exit` — that is the caller's
/// responsibility. Library code and tests can use this to emit diagnostics
/// without terminating the process.
#[cold]
pub fn emit_codegen_error(problem: impl Into<CodegenProblem>) {
    use ori_diagnostic::emitter::{ColorMode, DiagnosticEmitter, TerminalEmitter};

    let diag = problem.into().into_diagnostic();
    let is_tty = std::io::IsTerminal::is_terminal(&std::io::stderr());
    let mut emitter = TerminalEmitter::with_color_mode(std::io::stderr(), ColorMode::Auto, is_tty);
    emitter.emit(&diag);
    emitter.flush();
}

/// Emit a codegen error diagnostic and exit.
///
/// CLI convenience wrapper around [`emit_codegen_error`] that also calls
/// `process::exit(1)`. Use this only in CLI command handlers — library code
/// should use [`emit_codegen_error`] instead.
#[cold]
pub fn report_codegen_error(problem: impl Into<CodegenProblem>) -> ! {
    emit_codegen_error(problem);
    std::process::exit(1);
}

/// Emit accumulated codegen diagnostics (warnings and errors).
///
/// Returns `true` if any errors were emitted (callers should abort).
#[cold]
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
mod tests;
