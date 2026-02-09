//! Diagnostic integration tests for codegen errors.
//!
//! Verifies that all `From` conversions produce correct `ErrorCode`s and that
//! diagnostic messages contain relevant context (target triples, linker commands,
//! search paths, etc.).

use ori_diagnostic::{ErrorCode, Severity};
use ori_ir::Span;

// ── Import the types under test ─────────────────────────────────────

use oric::problem::codegen::{CodegenDiagnostics, CodegenProblem};

// ── ARC Problem conversions (E4xxx) ─────────────────────────────────

#[test]
fn from_arc_unsupported_expr() {
    let arc = ori_arc::ArcProblem::UnsupportedExpr {
        kind: "Await",
        span: Span::new(10, 20),
    };
    let problem: CodegenProblem = arc.into();
    let diag = problem.into_diagnostic();

    assert_eq!(diag.code, ErrorCode::E4001);
    assert_eq!(diag.severity, Severity::Warning);
    assert!(diag.message.contains("Await"));
    assert!(!diag.labels.is_empty());
}

#[test]
fn from_arc_unsupported_pattern() {
    let arc = ori_arc::ArcProblem::UnsupportedPattern {
        kind: "Guard",
        span: Span::new(5, 15),
    };
    let problem: CodegenProblem = arc.into();
    let diag = problem.into_diagnostic();

    assert_eq!(diag.code, ErrorCode::E4002);
    assert_eq!(diag.severity, Severity::Warning);
    assert!(diag.message.contains("Guard"));
}

#[test]
fn from_arc_internal_error() {
    let arc = ori_arc::ArcProblem::InternalError {
        message: "block predecessor mismatch".into(),
        span: Span::new(0, 5),
    };
    let problem: CodegenProblem = arc.into();
    let diag = problem.into_diagnostic();

    assert_eq!(diag.code, ErrorCode::E4003);
    assert_eq!(diag.severity, Severity::Error);
    assert!(diag.message.contains("internal error"));
    assert!(diag.notes.iter().any(|n| n.contains("compiler bug")));
}

#[test]
fn arc_unsupported_is_warning_not_error() {
    let expr = CodegenProblem::ArcUnsupportedExpr {
        kind: "Match",
        span: Span::new(0, 5),
    };
    let pattern = CodegenProblem::ArcUnsupportedPattern {
        kind: "Rest",
        span: Span::new(0, 5),
    };
    let internal = CodegenProblem::ArcInternalError {
        message: "bad".into(),
        span: Span::new(0, 5),
    };

    assert!(!expr.is_error());
    assert!(!pattern.is_error());
    assert!(internal.is_error());
}

// ── Target error conversions (E5004) ────────────────────────────────

#[test]
fn from_target_unsupported() {
    let err = ori_llvm::aot::TargetError::UnsupportedTarget {
        triple: "sparc-sun-solaris".into(),
        supported: vec!["x86_64", "aarch64", "wasm32"],
    };
    let problem: CodegenProblem = err.into();
    let diag = problem.into_diagnostic();

    assert_eq!(diag.code, ErrorCode::E5004);
    assert!(diag.message.contains("sparc-sun-solaris"));
    assert!(diag.notes.iter().any(|n| n.contains("x86_64")));
}

#[test]
fn from_target_initialization_failed() {
    let err = ori_llvm::aot::TargetError::InitializationFailed("no backend".into());
    let problem: CodegenProblem = err.into();
    let diag = problem.into_diagnostic();

    assert_eq!(diag.code, ErrorCode::E5004);
    assert!(diag.notes.iter().any(|n| n.contains("initialization")));
}

#[test]
fn from_target_invalid_triple() {
    let err = ori_llvm::aot::TargetError::InvalidTripleFormat {
        triple: "not-a-triple".into(),
        reason: "missing OS component".into(),
    };
    let problem: CodegenProblem = err.into();
    let diag = problem.into_diagnostic();

    assert_eq!(diag.code, ErrorCode::E5004);
    assert!(diag.message.contains("not-a-triple"));
}

#[test]
fn from_target_invalid_cpu() {
    let err = ori_llvm::aot::TargetError::InvalidCpu {
        cpu: "pentium-mmx".into(),
        target: "x86_64-unknown-linux-gnu".into(),
    };
    let problem: CodegenProblem = err.into();
    let diag = problem.into_diagnostic();

    assert_eq!(diag.code, ErrorCode::E5004);
    assert!(diag.notes.iter().any(|n| n.contains("pentium-mmx")));
}

// ── Optimization error conversions (E5001/E5002) ────────────────────

#[test]
fn from_optimization_verification_failed() {
    let err = ori_llvm::aot::OptimizationError::VerificationFailed {
        message: "expected i64, got i32".into(),
    };
    let problem: CodegenProblem = err.into();
    let diag = problem.into_diagnostic();

    assert_eq!(diag.code, ErrorCode::E5001);
    assert!(diag.notes.iter().any(|n| n.contains("compiler bug")));
}

#[test]
fn from_optimization_passes_failed() {
    let err = ori_llvm::aot::OptimizationError::PassesFailed {
        message: "segfault in LICM".into(),
    };
    let problem: CodegenProblem = err.into();
    let diag = problem.into_diagnostic();

    assert_eq!(diag.code, ErrorCode::E5002);
    assert!(diag.suggestions.iter().any(|s| s.contains("--opt=0")));
}

#[test]
fn from_optimization_invalid_pipeline() {
    let err = ori_llvm::aot::OptimizationError::InvalidPipeline {
        pipeline: "default<O99>".into(),
        message: "unknown opt level".into(),
    };
    let problem: CodegenProblem = err.into();
    let diag = problem.into_diagnostic();

    assert_eq!(diag.code, ErrorCode::E5002);
    assert!(diag.message.contains("O99"));
}

// ── Emit error conversions (E5003/E5004/E5009) ─────────────────────

#[test]
fn from_emit_object_emission() {
    let err = ori_llvm::aot::EmitError::ObjectEmission {
        path: "/tmp/out.o".into(),
        message: "permission denied".into(),
    };
    let problem: CodegenProblem = err.into();
    let diag = problem.into_diagnostic();

    assert_eq!(diag.code, ErrorCode::E5003);
    assert!(diag.message.contains("/tmp/out.o"));
}

#[test]
fn from_emit_module_configuration() {
    let err = ori_llvm::aot::EmitError::ModuleConfiguration(
        ori_llvm::aot::TargetError::TargetMachineCreationFailed("out of memory".into()),
    );
    let problem: CodegenProblem = err.into();
    let diag = problem.into_diagnostic();

    assert_eq!(diag.code, ErrorCode::E5009);
}

#[test]
fn from_emit_target_machine() {
    let err = ori_llvm::aot::EmitError::TargetMachine(
        ori_llvm::aot::TargetError::InitializationFailed("no X86 backend".into()),
    );
    let problem: CodegenProblem = err.into();
    let diag = problem.into_diagnostic();

    // TargetMachine errors become TargetNotSupported → E5004
    assert_eq!(diag.code, ErrorCode::E5004);
}

// ── Module pipeline error conversions ───────────────────────────────

#[test]
fn from_pipeline_verification() {
    let err = ori_llvm::aot::ModulePipelineError::Verification("bad IR".into());
    let problem: CodegenProblem = err.into();
    let diag = problem.into_diagnostic();
    assert_eq!(diag.code, ErrorCode::E5001);
}

#[test]
fn from_pipeline_optimization() {
    let err = ori_llvm::aot::ModulePipelineError::Optimization(
        ori_llvm::aot::OptimizationError::PassesFailed {
            message: "crash".into(),
        },
    );
    let problem: CodegenProblem = err.into();
    let diag = problem.into_diagnostic();
    assert_eq!(diag.code, ErrorCode::E5002);
}

#[test]
fn from_pipeline_emission() {
    let err =
        ori_llvm::aot::ModulePipelineError::Emission(ori_llvm::aot::EmitError::ObjectEmission {
            path: "out.o".into(),
            message: "disk full".into(),
        });
    let problem: CodegenProblem = err.into();
    let diag = problem.into_diagnostic();
    assert_eq!(diag.code, ErrorCode::E5003);
}

// ── Linker error conversions (E5006) ────────────────────────────────

#[test]
fn from_linker_not_found() {
    let err = ori_llvm::aot::LinkerError::LinkerNotFound {
        linker: "ld.lld".into(),
        message: "not in PATH".into(),
    };
    let problem: CodegenProblem = err.into();
    let diag = problem.into_diagnostic();

    assert_eq!(diag.code, ErrorCode::E5006);
    assert!(diag.message.contains("ld.lld"));
    assert!(diag.suggestions.iter().any(|s| s.contains("C toolchain")));
}

#[test]
fn from_linker_link_failed() {
    let err = ori_llvm::aot::LinkerError::LinkFailed {
        linker: "cc".into(),
        exit_code: Some(1),
        stderr: "undefined reference to `main`".into(),
        command: "cc -o output main.o -lori_rt".into(),
    };
    let problem: CodegenProblem = err.into();
    let diag = problem.into_diagnostic();

    assert_eq!(diag.code, ErrorCode::E5006);
    assert!(diag.message.contains("exit code 1"));
    assert!(diag.notes.iter().any(|n| n.contains("undefined reference")));
    assert!(diag.notes.iter().any(|n| n.contains("cc -o")));
}

#[test]
fn from_linker_unsupported_target() {
    let err = ori_llvm::aot::LinkerError::UnsupportedTarget {
        triple: "riscv64-unknown-elf".into(),
    };
    let problem: CodegenProblem = err.into();
    let diag = problem.into_diagnostic();

    // Becomes TargetNotSupported → E5004
    assert_eq!(diag.code, ErrorCode::E5004);
    assert!(diag.message.contains("riscv64"));
}

// ── Debug info error conversions (E5007) ────────────────────────────

#[test]
fn from_debug_info_disabled() {
    let err = ori_llvm::aot::DebugInfoError::Disabled;
    let problem: CodegenProblem = err.into();
    let diag = problem.into_diagnostic();

    assert_eq!(diag.code, ErrorCode::E5007);
    assert!(diag.suggestions.iter().any(|s| s.contains("--debug=0")));
}

#[test]
fn from_debug_info_basic_type() {
    let err = ori_llvm::aot::DebugInfoError::BasicType {
        name: "i128".into(),
        message: "unsupported width".into(),
    };
    let problem: CodegenProblem = err.into();
    let diag = problem.into_diagnostic();

    assert_eq!(diag.code, ErrorCode::E5007);
}

// ── WASM error conversions (E5008) ──────────────────────────────────

#[test]
fn from_wasm_invalid_config() {
    let err = ori_llvm::aot::WasmError::InvalidConfig {
        message: "missing export".into(),
    };
    let problem: CodegenProblem = err.into();
    let diag = problem.into_diagnostic();

    assert_eq!(diag.code, ErrorCode::E5008);
    assert!(diag.message.contains("WebAssembly"));
}

// ── Runtime not found (E5005) ───────────────────────────────────────

#[test]
fn from_runtime_not_found() {
    let err = ori_llvm::aot::RuntimeNotFound {
        searched_paths: vec!["/usr/lib".into(), "/opt/ori/lib".into()],
    };
    let problem: CodegenProblem = err.into();
    let diag = problem.into_diagnostic();

    assert_eq!(diag.code, ErrorCode::E5005);
    assert!(diag.message.contains("libori_rt.a"));
    assert!(diag.notes.iter().any(|n| n.contains("/usr/lib")));
    assert!(diag.suggestions.iter().any(|s| s.contains("cargo bl")));
}

// ── CodegenDiagnostics accumulator ──────────────────────────────────

#[test]
fn diagnostics_accumulator_empty() {
    let acc = CodegenDiagnostics::new();
    assert!(acc.is_empty());
    assert!(!acc.has_errors());
    assert!(acc.into_diagnostics().is_empty());
}

#[test]
fn diagnostics_accumulator_warnings_only() {
    let mut acc = CodegenDiagnostics::new();
    acc.push(CodegenProblem::ArcUnsupportedExpr {
        kind: "Await",
        span: Span::new(0, 5),
    });
    acc.push(CodegenProblem::ArcUnsupportedPattern {
        kind: "Guard",
        span: Span::new(10, 15),
    });

    assert!(!acc.is_empty());
    assert!(!acc.has_errors()); // Only warnings
    let diags = acc.into_diagnostics();
    assert_eq!(diags.len(), 2);
    assert!(diags.iter().all(|d| d.severity == Severity::Warning));
}

#[test]
fn diagnostics_accumulator_with_errors() {
    let mut acc = CodegenDiagnostics::new();
    acc.push(CodegenProblem::ArcUnsupportedExpr {
        kind: "Await",
        span: Span::new(0, 5),
    });
    acc.push(CodegenProblem::VerificationFailed {
        message: "bad".into(),
    });

    assert!(acc.has_errors());
    let diags = acc.into_diagnostics();
    assert_eq!(diags.len(), 2);
    assert_eq!(diags[0].severity, Severity::Warning);
    assert_eq!(diags[1].severity, Severity::Error);
}

#[test]
fn diagnostics_accumulator_add_arc_problems() {
    let mut acc = CodegenDiagnostics::new();
    let arc_problems = vec![
        ori_arc::ArcProblem::UnsupportedExpr {
            kind: "Await",
            span: Span::new(0, 5),
        },
        ori_arc::ArcProblem::InternalError {
            message: "bad invariant".into(),
            span: Span::new(10, 15),
        },
    ];
    acc.add_arc_problems(&arc_problems);

    assert!(acc.has_errors()); // InternalError is an error
    let diags = acc.into_diagnostics();
    assert_eq!(diags.len(), 2);
    assert_eq!(diags[0].code, ErrorCode::E4001);
    assert_eq!(diags[1].code, ErrorCode::E4003);
}

// ── Direct construction diagnostics ─────────────────────────────────

#[test]
fn verification_failed_has_ice_note() {
    let problem = CodegenProblem::VerificationFailed {
        message: "instruction does not dominate all uses".into(),
    };
    let diag = problem.into_diagnostic();

    assert_eq!(diag.code, ErrorCode::E5001);
    assert!(diag.notes.iter().any(|n| n.contains("compiler bug")));
    assert!(diag.notes.iter().any(|n| n.contains("dominate")));
}

#[test]
fn emission_failed_has_suggestion() {
    let problem = CodegenProblem::EmissionFailed {
        format: "object".into(),
        path: "/read-only/out.o".into(),
        message: "permission denied".into(),
    };
    let diag = problem.into_diagnostic();

    assert_eq!(diag.code, ErrorCode::E5003);
    assert!(diag.message.contains("/read-only/out.o"));
    assert!(diag.suggestions.iter().any(|s| s.contains("directory")));
}

#[test]
fn module_config_failed() {
    let problem = CodegenProblem::ModuleConfigFailed {
        message: "data layout mismatch".into(),
    };
    let diag = problem.into_diagnostic();

    assert_eq!(diag.code, ErrorCode::E5009);
    assert!(diag.message.contains("module configuration failed"));
}
