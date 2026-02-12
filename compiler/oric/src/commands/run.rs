//! The `run` command: parse, type-check, and evaluate an Ori source file.

use ori_diagnostic::emitter::{ColorMode, DiagnosticEmitter, TerminalEmitter};
use oric::query::evaluated;
use oric::{CompilerDb, Db, SourceFile};
use std::path::PathBuf;

#[cfg(feature = "llvm")]
use std::path::Path;

use super::{read_file, report_frontend_errors};

/// Run an Ori source file: parse, type-check, and evaluate it.
///
/// Accumulates all errors (parse and type) before exiting, giving the user
/// a complete picture of issues rather than stopping at the first error.
///
/// When `profile` is true, enables performance counters and prints a report
/// to stderr after evaluation completes.
pub fn run_file(path: &str, profile: bool) {
    let content = read_file(path);
    let db = CompilerDb::new();
    let file = SourceFile::new(&db, PathBuf::from(path), content);

    // Create emitter once with source context for rich snippet rendering
    let is_tty = std::io::IsTerminal::is_terminal(&std::io::stderr());
    let mut emitter = TerminalEmitter::with_color_mode(std::io::stderr(), ColorMode::Auto, is_tty)
        .with_source(file.text(&db).as_str())
        .with_file_path(path);

    // Run frontend pipeline: lex → parse → typecheck, reporting all errors
    let Some(frontend) = report_frontend_errors(&db, file, &mut emitter) else {
        std::process::exit(1);
    };

    if frontend.has_errors() {
        emitter.flush();
        std::process::exit(1);
    }

    // Evaluate only if no errors
    if profile {
        eval_with_profile(
            &db,
            file,
            path,
            &frontend.parse_result,
            &frontend.type_result,
            &frontend.pool,
            &mut emitter,
        );
    } else {
        let eval_result = evaluated(&db, file);
        report_eval_result(&eval_result, &db, file, path, &mut emitter);
    }
}

/// Report evaluation results, using enriched diagnostics for runtime errors.
fn report_eval_result(
    eval_result: &oric::eval::ModuleEvalResult,
    db: &oric::CompilerDb,
    file: oric::SourceFile,
    path: &str,
    emitter: &mut TerminalEmitter<std::io::Stderr>,
) {
    if eval_result.is_failure() {
        // Use enriched diagnostics when we have a structured error snapshot
        if let Some(ref snapshot) = eval_result.eval_error {
            let source = file.text(db);
            let diag = oric::problem::eval::snapshot_to_diagnostic(snapshot, source.as_str(), path);
            emitter.emit(&diag);
            emitter.flush();
        } else {
            let error_msg = eval_result
                .error
                .as_deref()
                .unwrap_or("unknown runtime error");
            let diag = ori_diagnostic::Diagnostic::error(ori_diagnostic::ErrorCode::E6099)
                .with_message(format!("runtime error in '{path}': {error_msg}"));
            emitter.emit(&diag);
            emitter.flush();
        }
        std::process::exit(1);
    }

    // Print the result if it's not void
    if let Some(ref result) = eval_result.result {
        use oric::EvalOutput;
        match result {
            EvalOutput::Void => {}
            _ => println!("{}", result.display(db.interner())),
        }
    }
}

/// Evaluate with profiling enabled — bypasses Salsa's `evaluated()` query
/// to access performance counters. Uses the already-computed frontend results
/// from `run_file` instead of re-querying.
fn eval_with_profile(
    db: &oric::CompilerDb,
    file: oric::SourceFile,
    path: &str,
    parse_result: &oric::parser::ParseOutput,
    type_result: &ori_types::TypeCheckResult,
    pool: &std::sync::Arc<ori_types::Pool>,
    emitter: &mut TerminalEmitter<std::io::Stderr>,
) {
    use oric::query::{run_evaluation, EvalRunMode};

    // Canonicalize + evaluate with counters via shared helper
    let (eval_result, counters) = run_evaluation(
        db,
        file,
        parse_result,
        type_result,
        pool,
        EvalRunMode::Profile,
    );

    // Print counters report to stderr before result
    if let Some(report) = counters {
        eprintln!("{report}");
    }

    report_eval_result(&eval_result, db, file, path, emitter);
}

/// Run an Ori source file using AOT compilation.
///
/// This mode compiles the source to a native executable and caches it.
/// Subsequent runs with unchanged source reuse the cached binary.
#[cfg(feature = "llvm")]
pub fn run_file_compiled(path: &str) {
    use ori_llvm::inkwell::context::Context;
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use std::process::Command;
    use std::time::Instant;

    use ori_llvm::aot::{
        LinkInput, LinkOutput, LinkerDriver, ObjectEmitter, OutputFormat, RuntimeConfig,
    };

    use super::compile_common::{check_source, compile_to_llvm};
    use oric::{CompilerDb, SourceFile};

    let start = Instant::now();

    // Read source file
    let content = read_file(path);

    // Compute content hash for caching
    let content_hash = {
        let mut hasher = DefaultHasher::new();
        content.hash(&mut hasher);
        // Include compiler version in hash for cache invalidation
        env!("CARGO_PKG_VERSION").hash(&mut hasher);
        hasher.finish()
    };

    // Determine cache directory and binary path
    let cache_dir = get_cache_dir();
    let source_name = Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("program");
    let binary_name = format!("{source_name}-{content_hash:016x}");
    let binary_path = cache_dir.join(&binary_name);

    // Check if cached binary exists and is valid
    if binary_path.exists() {
        // Cache hit - execute directly
        let exec_start = Instant::now();
        let status = match Command::new(&binary_path).status() {
            Ok(s) => s,
            Err(e) => {
                eprintln!(
                    "error: failed to execute cached binary '{}': {}",
                    binary_path.display(),
                    e
                );
                eprintln!(
                    "hint: try removing the cache with: rm -rf {}",
                    cache_dir.display()
                );
                std::process::exit(1);
            }
        };

        tracing::debug!(
            elapsed_ms = exec_start.elapsed().as_secs_f64() * 1000.0,
            "cache hit: executed compiled binary"
        );

        std::process::exit(status.code().unwrap_or(1));
    }

    // Cache miss - need to compile
    eprintln!("  Compiling {path} (first run)...");

    // Parse and type-check (shared with build_file)
    let db = CompilerDb::new();
    let file = SourceFile::new(&db, PathBuf::from(path), content);

    let Some((parse_result, type_result, pool, canon_result)) = check_source(&db, file, path)
    else {
        std::process::exit(1)
    };

    // Configure target (native)
    let target = ori_llvm::aot::TargetConfig::native()
        .unwrap_or_else(|e| crate::problem::codegen::report_codegen_error(e));

    // Generate LLVM IR (shared with build_file)
    let context = Context::create();
    let llvm_module = compile_to_llvm(
        &context,
        &db,
        &parse_result,
        &type_result,
        &pool,
        &canon_result,
        path,
    );

    // Configure module for target
    let emitter = ObjectEmitter::new(&target)
        .unwrap_or_else(|e| crate::problem::codegen::report_codegen_error(e));

    if let Err(e) = emitter.configure_module(&llvm_module) {
        crate::problem::codegen::report_codegen_error(
            crate::problem::codegen::CodegenProblem::ModuleConfigFailed {
                message: e.to_string(),
            },
        );
    }

    // Ensure cache directory exists
    if let Err(e) = std::fs::create_dir_all(&cache_dir) {
        eprintln!("warning: could not create cache directory: {e}");
    }

    // Verify → optimize → emit object file via unified pipeline (O2 for good performance)
    let opt_config = ori_llvm::aot::OptimizationConfig::new(ori_llvm::aot::OptimizationLevel::O2);
    let obj_path = cache_dir.join(format!("{binary_name}.o"));

    if let Err(e) =
        emitter.verify_optimize_emit(&llvm_module, &opt_config, &obj_path, OutputFormat::Object)
    {
        crate::problem::codegen::report_codegen_error(e);
    }

    // Link into executable
    let driver = LinkerDriver::new(&target);

    // Find runtime library
    let runtime_config = match RuntimeConfig::detect() {
        Ok(config) => config,
        Err(e) => {
            crate::problem::codegen::report_codegen_error(e);
        }
    };

    let mut link_input = LinkInput {
        objects: vec![obj_path.clone()],
        output: binary_path.clone(),
        output_kind: LinkOutput::Executable,
        gc_sections: true, // Remove unused sections
        ..Default::default()
    };

    runtime_config.configure_link(&mut link_input);

    if let Err(e) = driver.link(&link_input) {
        // Clean up partial artifacts
        let _ = std::fs::remove_file(&obj_path);
        crate::problem::codegen::report_codegen_error(e);
    }

    // Clean up object file
    let _ = std::fs::remove_file(&obj_path);

    let compile_time = start.elapsed();
    eprintln!("  Compiled in {:.2}s", compile_time.as_secs_f64());

    // Execute the compiled binary
    let status = match Command::new(&binary_path).status() {
        Ok(s) => s,
        Err(e) => {
            eprintln!(
                "error: failed to execute compiled binary '{}': {}",
                binary_path.display(),
                e
            );
            std::process::exit(1);
        }
    };

    std::process::exit(status.code().unwrap_or(1));
}

/// Get the cache directory for compiled binaries.
#[cfg(feature = "llvm")]
fn get_cache_dir() -> PathBuf {
    // Try XDG cache directory first, fall back to home directory
    if let Ok(xdg_cache) = std::env::var("XDG_CACHE_HOME") {
        return PathBuf::from(xdg_cache).join("ori").join("compiled");
    }

    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home)
            .join(".cache")
            .join("ori")
            .join("compiled");
    }

    // Fall back to temp directory
    std::env::temp_dir().join("ori-cache").join("compiled")
}

/// Run with compile mode when LLVM feature is not enabled.
#[cfg(not(feature = "llvm"))]
pub fn run_file_compiled(_path: &str) {
    use ori_diagnostic::emitter::{ColorMode, DiagnosticEmitter, TerminalEmitter};
    use ori_diagnostic::{Diagnostic, ErrorCode};

    let diag = Diagnostic::error(ErrorCode::E5004)
        .with_message("the '--compile' flag requires the LLVM backend")
        .with_note("the Ori compiler was built without LLVM support")
        .with_suggestion("rebuild with `cargo build --features llvm`");

    let is_tty = std::io::IsTerminal::is_terminal(&std::io::stderr());
    let mut emitter = TerminalEmitter::with_color_mode(std::io::stderr(), ColorMode::Auto, is_tty);
    emitter.emit(&diag);
    emitter.flush();
    std::process::exit(1);
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "llvm")]
    mod llvm_tests {
        use super::super::get_cache_dir;
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        #[test]
        fn test_cache_dir_exists_or_creatable() {
            let cache_dir = get_cache_dir();
            // Should be a valid path
            assert!(!cache_dir.as_os_str().is_empty());
            // Should contain "ori" somewhere in the path
            let path_str = cache_dir.to_string_lossy();
            assert!(path_str.contains("ori"), "cache dir should contain 'ori'");
        }

        #[test]
        fn test_cache_dir_is_absolute_or_temp() {
            let cache_dir = get_cache_dir();
            // Should be either absolute or in temp
            let is_absolute = cache_dir.is_absolute();
            let is_in_temp = cache_dir.starts_with(std::env::temp_dir());
            assert!(
                is_absolute || is_in_temp,
                "cache dir should be absolute or in temp: {cache_dir:?}"
            );
        }

        #[test]
        fn test_content_hash_deterministic() {
            let content = "let x = 42";
            let version = env!("CARGO_PKG_VERSION");

            let hash1 = {
                let mut hasher = DefaultHasher::new();
                content.hash(&mut hasher);
                version.hash(&mut hasher);
                hasher.finish()
            };

            let hash2 = {
                let mut hasher = DefaultHasher::new();
                content.hash(&mut hasher);
                version.hash(&mut hasher);
                hasher.finish()
            };

            assert_eq!(hash1, hash2, "same content should produce same hash");
        }

        #[test]
        fn test_content_hash_differs_for_different_content() {
            let version = env!("CARGO_PKG_VERSION");

            let hash1 = {
                let mut hasher = DefaultHasher::new();
                "let x = 42".hash(&mut hasher);
                version.hash(&mut hasher);
                hasher.finish()
            };

            let hash2 = {
                let mut hasher = DefaultHasher::new();
                "let x = 43".hash(&mut hasher);
                version.hash(&mut hasher);
                hasher.finish()
            };

            assert_ne!(
                hash1, hash2,
                "different content should produce different hash"
            );
        }

        #[test]
        fn test_binary_name_format() {
            let source_name = "hello";
            let content_hash: u64 = 0x1234_5678_90AB_CDEF;
            let binary_name = format!("{source_name}-{content_hash:016x}");

            assert_eq!(binary_name, "hello-1234567890abcdef");
            assert!(binary_name.contains(source_name));
            // Hash should be exactly 16 hex characters
            let parts: Vec<&str> = binary_name.split('-').collect();
            assert_eq!(parts.len(), 2);
            assert_eq!(parts[1].len(), 16);
        }
    }
}
