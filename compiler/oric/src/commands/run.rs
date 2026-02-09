//! The `run` command: parse, type-check, and evaluate an Ori source file.

use ori_diagnostic::emitter::{ColorMode, DiagnosticEmitter, TerminalEmitter};
use ori_diagnostic::queue::DiagnosticQueue;
use oric::problem::LexProblem;
use oric::query::{evaluated, lex_errors, parsed};
use oric::reporting::typeck::TypeErrorRenderer;
use oric::typeck;
use oric::{CompilerDb, Db, SourceFile};
use std::path::PathBuf;

#[cfg(feature = "llvm")]
use std::path::Path;

use super::read_file;

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

    let mut has_errors = false;

    // Create emitter once with source context for rich snippet rendering
    let is_tty = std::io::IsTerminal::is_terminal(&std::io::stderr());
    let mut emitter = TerminalEmitter::with_color_mode(std::io::stderr(), ColorMode::Auto, is_tty)
        .with_source(file.text(&db).as_str())
        .with_file_path(path);

    // Report lexer errors first (unterminated strings, semicolons, confusables, etc.)
    let lex_errs = lex_errors(&db, file);
    if !lex_errs.is_empty() {
        for err in &lex_errs {
            let diag = LexProblem::Error(err.clone()).into_diagnostic(db.interner());
            emitter.emit(&diag);
        }
        has_errors = true;
    }

    // Check for parse errors — route through DiagnosticQueue for
    // deduplication and soft-error suppression after hard errors
    let parse_result = parsed(&db, file);
    if parse_result.has_errors() {
        let source = file.text(&db);
        let mut queue = DiagnosticQueue::new();
        for error in &parse_result.errors {
            queue.add_with_source_and_severity(
                error.to_diagnostic(),
                source.as_str(),
                error.severity,
            );
        }
        for diag in queue.flush() {
            emitter.emit(&diag);
        }
        has_errors = true;
    }

    // Check for type errors using direct call to get Pool for rich rendering
    let (type_result, pool) =
        typeck::type_check_with_imports_and_pool(&db, &parse_result, file.path(&db));
    if type_result.has_errors() {
        let renderer = TypeErrorRenderer::new(&pool, db.interner());

        for error in type_result.errors() {
            emitter.emit(&renderer.render(error));
        }
        has_errors = true;
    }

    if has_errors {
        emitter.flush();
    }

    // Exit if any errors occurred
    if has_errors {
        std::process::exit(1);
    }

    // Evaluate only if no errors
    if profile {
        eval_with_profile(&db, &parse_result, file, path, &mut emitter);
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
            eprintln!("error: runtime error in '{path}': {error_msg}");
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

/// Evaluate with profiling enabled — bypasses Salsa query to access counters.
fn eval_with_profile(
    db: &oric::CompilerDb,
    parse_result: &oric::parser::ParseOutput,
    file: oric::SourceFile,
    path: &str,
    emitter: &mut TerminalEmitter<std::io::Stderr>,
) {
    use oric::eval::{EvalOutput, Evaluator, ModuleEvalResult};
    use oric::Db;

    let interner = db.interner();
    let file_path = file.path(db);

    // Type check (returns result + pool)
    let (type_result, pool) =
        oric::typeck::type_check_with_imports_and_pool(db, parse_result, file_path);

    if type_result.has_errors() {
        let error_count = type_result.errors().len();
        let result = ModuleEvalResult::failure(format!(
            "{error_count} type error{} found",
            if error_count == 1 { "" } else { "s" }
        ));
        report_eval_result(&result, db, file, path, emitter);
        return;
    }

    // Canonicalize: AST + types → self-contained canonical IR.
    let canon_result = ori_canon::lower_module(
        &parse_result.module,
        &parse_result.arena,
        &type_result,
        &pool,
        interner,
    );
    let shared_canon = ori_ir::canon::SharedCanonResult::new(canon_result);

    // Create evaluator with profiling enabled
    let mut evaluator = Evaluator::builder(interner, &parse_result.arena, db)
        .expr_types(&type_result.typed.expr_types)
        .pattern_resolutions(&type_result.typed.pattern_resolutions)
        .canon(shared_canon.clone())
        .build();
    evaluator.register_prelude();
    evaluator.enable_counters();

    if let Err(e) = evaluator.load_module(parse_result, file_path, Some(&shared_canon)) {
        let result = ModuleEvalResult::failure(format!("module error: {e}"));
        report_eval_result(&result, db, file, path, emitter);
        return;
    }

    // Evaluate main function or first zero-arg function
    let main_name = interner.intern("main");
    let eval_result = if let Some(main_func) = evaluator.env().lookup(main_name) {
        match evaluator.eval_call_value(&main_func, &[]) {
            Ok(value) => ModuleEvalResult::success(EvalOutput::from_value(&value, interner)),
            Err(e) => ModuleEvalResult::runtime_error(&e.into_eval_error()),
        }
    } else if let Some(func) = parse_result.module.functions.first() {
        let params = parse_result.arena.get_params(func.params);
        if params.is_empty() {
            let result = if let Some(can_id) = shared_canon.root_for(func.name) {
                evaluator.eval_can(can_id)
            } else {
                evaluator.eval(func.body)
            };
            match result {
                Ok(value) => ModuleEvalResult::success(EvalOutput::from_value(&value, interner)),
                Err(e) => ModuleEvalResult::runtime_error(&e.into_eval_error()),
            }
        } else {
            ModuleEvalResult::success(EvalOutput::Void)
        }
    } else {
        ModuleEvalResult::default()
    };

    // Print counters report to stderr before result
    if let Some(report) = evaluator.counters_report() {
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
    let file = SourceFile::new(&db, PathBuf::from(path), content.clone());

    let Some((parse_result, type_result, pool)) = check_source(&db, file, path) else {
        std::process::exit(1)
    };

    // Configure target (native)
    let target = ori_llvm::aot::TargetConfig::native()
        .unwrap_or_else(|e| crate::problem::codegen::report_codegen_error(e));

    // Generate LLVM IR (shared with build_file)
    let context = Context::create();
    let llvm_module = compile_to_llvm(&context, &db, &parse_result, &type_result, &pool, path);

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
