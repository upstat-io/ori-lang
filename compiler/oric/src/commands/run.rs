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
pub fn run_file(path: &str) {
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
    let eval_result = evaluated(&db, file);
    if eval_result.is_failure() {
        let error_msg = eval_result
            .error
            .unwrap_or_else(|| "unknown runtime error".to_string());
        eprintln!("error: runtime error in '{path}': {error_msg}");
        std::process::exit(1);
    }

    // Print the result if it's not void
    if let Some(result) = eval_result.result {
        use oric::EvalOutput;
        match result {
            EvalOutput::Void => {}
            _ => println!("{}", result.display(db.interner())),
        }
    }
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
    let target = match ori_llvm::aot::TargetConfig::native() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("error: failed to initialize native target: {e}");
            std::process::exit(1);
        }
    };

    // Generate LLVM IR (shared with build_file)
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
        eprintln!("error: pipeline failed: {e}");
        std::process::exit(1);
    }

    // Link into executable
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

    let mut link_input = LinkInput {
        objects: vec![obj_path.clone()],
        output: binary_path.clone(),
        output_kind: LinkOutput::Executable,
        gc_sections: true, // Remove unused sections
        ..Default::default()
    };

    runtime_config.configure_link(&mut link_input);

    if let Err(e) = driver.link(&link_input) {
        eprintln!("error: linking failed: {e}");
        // Clean up partial artifacts
        let _ = std::fs::remove_file(&obj_path);
        std::process::exit(1);
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
    eprintln!("error: the '--compile' flag requires the LLVM backend");
    eprintln!();
    eprintln!("The Ori compiler was built without LLVM support.");
    eprintln!("To enable AOT compilation, rebuild with the 'llvm' feature:");
    eprintln!();
    eprintln!("  cargo build --features llvm");
    eprintln!();
    eprintln!("Or use the LLVM-enabled Docker container:");
    eprintln!();
    eprintln!("  ./docker/llvm/run.sh ori run --compile <file.ori>");
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
