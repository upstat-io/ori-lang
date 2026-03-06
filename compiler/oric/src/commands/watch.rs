//! The `watch` command: watch a file and re-check on changes.
//!
//! Reuses the Salsa `CompilerDb` across file changes, demonstrating incremental
//! recompilation. When a watched file changes, only the invalidated queries
//! re-execute — unchanged parts of the pipeline return cached results.

use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use notify::{EventKind, RecursiveMode, Watcher};
use ori_diagnostic::emitter::{ColorMode, TerminalEmitter};
use oric::{CompilerDb, SourceFile};
use salsa::Setter;

use super::print_check_success;
use super::read_file;
use super::report_frontend_errors;
use super::run_post_frontend_checks;
use super::TestEnforcement;

/// Debounce window — drain events for this long after the first event.
const DEBOUNCE_MS: u64 = 300;

/// Watch a file and re-check on every modification.
///
/// Creates a Salsa database once and reuses it across edits. On each change,
/// calls `file.set_text(&mut db).to(new_content)` which invalidates only the
/// affected Salsa queries — downstream queries that depend on unchanged results
/// are not re-executed (early cutoff).
pub fn watch_file(path: &str, enforcement: TestEnforcement) {
    let canonical = match std::fs::canonicalize(path) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: cannot resolve path '{path}': {e}");
            std::process::exit(1);
        }
    };

    // Initial check
    let content = read_file(path);
    let mut db = CompilerDb::new();
    let file = SourceFile::new(&db, PathBuf::from(path), content);

    run_check(&db, file, path, enforcement);

    println!();
    println!("Watching {path}...");
    println!("(Press Ctrl+C to stop)");
    println!();

    // Set up file watcher on the parent directory. We watch the parent because
    // editors like vim/emacs use rename-on-save (write to temp, rename over
    // original), which doesn't trigger Modify events on the original inode.
    let (tx, rx) = mpsc::channel();
    let mut watcher = match notify::recommended_watcher(move |res| {
        if let Ok(event) = res {
            let _ = tx.send(event);
        }
    }) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("error: cannot create file watcher: {e}");
            std::process::exit(1);
        }
    };

    let watch_dir = canonical.parent().unwrap_or(&canonical);
    if let Err(e) = watcher.watch(watch_dir, RecursiveMode::NonRecursive) {
        eprintln!(
            "error: cannot watch directory '{}': {e}",
            watch_dir.display()
        );
        std::process::exit(1);
    }

    // Event loop — blocks on channel recv, breaks when watcher disconnects
    while let Ok(event) = rx.recv() {
        // Only react to events that affect our file
        if !is_relevant_event(&event, &canonical) {
            continue;
        }

        // Debounce: drain events for DEBOUNCE_MS after the first trigger.
        // Editors often emit multiple events for a single save (write, chmod,
        // rename, etc.). Without debouncing we'd re-check multiple times.
        let deadline = Instant::now() + Duration::from_millis(DEBOUNCE_MS);
        while rx
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .is_ok()
        {}

        // Re-read file
        let Ok(new_content) = std::fs::read_to_string(path) else {
            eprintln!("error reading '{path}'");
            continue;
        };

        // Clear screen
        eprint!("\x1b[2J\x1b[H");

        let now = chrono_free_timestamp();
        println!("[{now}] Checking {path}...");
        println!();

        // Update Salsa input — this is the key incremental step.
        // Only queries that depend on the changed text will re-execute.
        file.set_text(&mut db).to(new_content);

        run_check(&db, file, path, enforcement);

        println!();
        println!("Watching {path}...");
    }
}

/// Run the full check pipeline and print results.
///
/// Does NOT exit on errors — in watch mode we keep running so the user can fix
/// issues and see the results immediately.
fn run_check(db: &CompilerDb, file: SourceFile, path: &str, enforcement: TestEnforcement) {
    let is_tty = std::io::IsTerminal::is_terminal(&std::io::stderr());
    let mut emitter = TerminalEmitter::with_color_mode(std::io::stderr(), ColorMode::Auto, is_tty)
        .with_source(file.text(db).as_str())
        .with_file_path(path);

    let Some(frontend) = report_frontend_errors(db, file, &mut emitter) else {
        eprintln!("internal error: Pool not cached after type checking");
        return;
    };

    let result = run_post_frontend_checks(db, file, &frontend, enforcement, &mut emitter);
    if result.has_errors {
        return;
    }

    print_check_success(path, enforcement, &result);
}

/// Check if a notify event is relevant to our watched file.
fn is_relevant_event(event: &notify::Event, canonical_path: &std::path::Path) -> bool {
    // Only trigger on content modifications or file creation (rename-on-save).
    // Ignore metadata-only changes, directory events, and access events.
    match event.kind {
        EventKind::Modify(notify::event::ModifyKind::Data(_) | notify::event::ModifyKind::Any)
        | EventKind::Create(_) => {}
        _ => return false,
    }

    // Check if any event path matches our file
    event.paths.iter().any(|p| {
        std::fs::canonicalize(p).map_or_else(|_| p == canonical_path, |cp| cp == canonical_path)
    })
}

/// Simple timestamp without pulling in chrono.
fn chrono_free_timestamp() -> String {
    use std::time::SystemTime;
    let elapsed = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = elapsed.as_secs();
    let hours = (secs / 3600) % 24;
    let mins = (secs / 60) % 60;
    let s = secs % 60;
    format!("{hours:02}:{mins:02}:{s:02}")
}
