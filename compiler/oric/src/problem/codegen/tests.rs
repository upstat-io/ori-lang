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
