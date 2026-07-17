#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "corpus setup and formatter failures are assertion failures for this test harness"
)]
//! Idempotence verification tests.
//!
//! These tests verify, for every .ori file in the repository:
//! 1. format(format(code)) == format(code) — formatting is idempotent
//! 2. The formatted output re-parses (the format->parse round-trip holds)
//!
//! This covers all test files in the repository, not just golden tests.

use std::fs;
use std::path::{Path, PathBuf};

use ori_fmt::{format_module, format_module_with_comments};
use ori_ir::StringInterner;
use ori_lexer::lex_with_comments;

fn fixture_without_trailing_newline(source: &'static str) -> &'static str {
    source
        .strip_suffix('\n')
        .expect("committed Ori fixtures end with a newline")
}

/// Repo-relative paths whose formatted output fails to re-parse — a
/// `format -> parse` round-trip defect (Spec: Annex D — formatting must preserve
/// observable semantics, so the output must re-parse). The harness counts these
/// as KNOWN-tracked breaks, FAILS on any NEW re-parse break, and FAILS when a
/// listed file re-parses clean again (stale-entry guard, mirroring
/// DISPOSITION_DRIFT:stale-tracking) so the list shrinks as each break is fixed.
/// The sole entry is a nested tuple-index `.0.0` round-trip break (BUG-07-339).
const KNOWN_REPARSE_BREAKS: &[&str] = &["tests/spec/types/tuple_types.ori"];

/// Classification of a single file's idempotence + round-trip check.
enum IdemOutcome {
    /// `format(format(x)) == format(x)` AND output re-parsed.
    Pass,
    /// Source itself does not parse — a legitimate skip (the input never parsed).
    SourceParseSkip,
    /// Formatter output failed to re-parse AND the file is in
    /// `KNOWN_REPARSE_BREAKS` — a BUG-07-339-tracked round-trip break.
    KnownReparseBreak,
    /// Formatter output failed to re-parse and the file is NOT tracked — a NEW
    /// round-trip defect. Hard failure.
    NewReparseBreak(String),
    /// `format(format(x)) != format(x)` — a non-idempotent format. Hard failure.
    IdempotenceFailure(String),
}

/// Whether `repo_rel` is a tracked BUG-07-339 re-parse break.
fn is_known_reparse_break(repo_rel: &str) -> bool {
    KNOWN_REPARSE_BREAKS.contains(&repo_rel)
}

/// Repo-relative path string for a file under the repo root, for allowlist matching.
fn repo_relative(path: &Path) -> String {
    let root = repo_root();
    path.strip_prefix(&root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// Find all .ori files in a directory recursively, excluding certain patterns.
fn find_all_ori_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();

    if !dir.exists() || !dir.is_dir() {
        return files;
    }

    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = path.file_name().map(|n| n.to_string_lossy().to_string());

            // Skip hidden directories, target, node_modules
            if let Some(ref n) = name {
                if n.starts_with('.') || n == "target" || n == "node_modules" {
                    continue;
                }
            }

            if path.is_dir() {
                files.extend(find_all_ori_files(&path));
            } else if path.extension().is_some_and(|e| e == "ori") {
                // Skip .expected files (they have .ori.expected extension)
                if !path.to_string_lossy().contains(".expected") {
                    files.push(path);
                }
            }
        }
    }

    files.sort();
    files
}

/// Get repository root.
fn repo_root() -> PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    PathBuf::from(manifest_dir)
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

/// Parse and format code.
fn parse_and_format(source: &str) -> Result<String, String> {
    let interner = StringInterner::new();
    let tokens = ori_lexer::lex(source, &interner);
    let output = ori_parse::parse(&tokens, &interner);

    if output.has_errors() {
        let errors: Vec<String> = output.errors.iter().map(|e| format!("{e:?}")).collect();
        return Err(format!("Parse errors:\n{}", errors.join("\n")));
    }

    Ok(format_module(&output.module, &output.arena, &interner))
}

/// Parse and format code with comment preservation.
fn parse_and_format_with_comments(source: &str) -> Result<String, String> {
    let interner = StringInterner::new();
    let lex_output = lex_with_comments(source, &interner);
    let parse_output = ori_parse::parse(&lex_output.tokens, &interner);

    if parse_output.has_errors() {
        let errors: Vec<String> = parse_output
            .errors
            .iter()
            .map(|e| format!("{e:?}"))
            .collect();
        return Err(format!("Parse errors:\n{}", errors.join("\n")));
    }

    Ok(format_module_with_comments(
        &parse_output.module,
        &lex_output.comments,
        &parse_output.arena,
        &interner,
    ))
}

#[test]
fn duplicate_test_names_round_trip_with_source_names() {
    let source = fixture_without_trailing_newline(include_str!(
        "fixtures/idempotence/duplicate_test_names_round_trip_with_source_names.ori"
    ));

    let first = parse_and_format_with_comments(source).expect("source should parse and format");
    assert!(!first.contains("$test$"));
    assert!(first.contains("@test tests @left"));
    assert!(first.contains("@test tests @right"));

    let second = parse_and_format_with_comments(&first).expect("formatted output should re-parse");
    assert_eq!(normalize_whitespace(&first), normalize_whitespace(&second));
}

#[test]
fn constants_round_trip_with_complete_declaration_syntax() {
    let source = fixture_without_trailing_newline(include_str!(
        "fixtures/idempotence/constants_round_trip_with_complete_declaration_syntax.ori"
    ));

    let first = parse_and_format_with_comments(source).expect("source should parse and format");
    assert!(first.contains("pub let $LIMIT: int = 4;"));
    assert!(first.contains("let $ENABLED = true;"));

    let second = parse_and_format_with_comments(&first).expect("formatted output should re-parse");
    assert_eq!(normalize_whitespace(&first), normalize_whitespace(&second));
}

#[test]
fn trait_associated_type_defaults_round_trip() {
    let source = fixture_without_trailing_newline(include_str!(
        "fixtures/idempotence/trait_associated_type_defaults_round_trip.ori"
    ));

    let first = parse_and_format_with_comments(source).expect("source should parse and format");
    assert!(first.contains("trait Addable<Rhs = Self>"));
    assert!(first.contains("type Output = Self"));

    let second = parse_and_format_with_comments(&first).expect("formatted output should re-parse");
    assert_eq!(normalize_whitespace(&first), normalize_whitespace(&second));
}

/// Normalize whitespace for comparison.
fn normalize_whitespace(source: &str) -> String {
    let lines: Vec<&str> = source.lines().map(str::trim_end).collect();
    let start = lines.iter().position(|l| !l.is_empty()).unwrap_or(0);
    let mut result = Vec::new();
    let mut prev_blank = false;

    for line in &lines[start..] {
        let is_blank = line.is_empty();
        if is_blank && prev_blank {
            continue;
        }
        result.push(*line);
        prev_blank = is_blank;
    }

    while result.last().is_some_and(|l| l.is_empty()) {
        result.pop();
    }

    let mut output = result.join("\n");
    if !output.is_empty() {
        output.push('\n');
    }
    output
}

/// Classify a single file: idempotence AND format->parse round-trip.
///
/// A source that does not parse is a legitimate skip. A formatter output that
/// does not re-parse is a round-trip defect — tracked when listed in
/// `KNOWN_REPARSE_BREAKS` (BUG-07-339), a hard failure when not.
fn test_idempotence_for_file(path: &Path) -> IdemOutcome {
    let source = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            return IdemOutcome::NewReparseBreak(format!(
                "Failed to read {}: {}",
                path.display(),
                e
            ))
        }
    };

    // Try formatting with comments first; fall back to no-comment formatting.
    let first = match parse_and_format_with_comments(&source) {
        Ok(s) => s,
        Err(_) => match parse_and_format(&source) {
            Ok(s) => s,
            // Source itself does not parse — legitimate skip.
            Err(_) => return IdemOutcome::SourceParseSkip,
        },
    };

    // The formatted output MUST re-parse (format->parse round-trip).
    let second = match parse_and_format_with_comments(&first) {
        Ok(s) => s,
        Err(e) => {
            let repo_rel = repo_relative(path);
            let detail = format!("Re-parse error: {e}");
            return if is_known_reparse_break(&repo_rel) {
                IdemOutcome::KnownReparseBreak
            } else {
                IdemOutcome::NewReparseBreak(detail)
            };
        }
    };

    let first_normalized = normalize_whitespace(&first);
    let second_normalized = normalize_whitespace(&second);

    if first_normalized != second_normalized {
        return IdemOutcome::IdempotenceFailure(format!(
            "Idempotence failure:\n\n--- First format ---\n{first_normalized}\n--- Second format ---\n{second_normalized}\n"
        ));
    }

    IdemOutcome::Pass
}

/// Aggregate of one directory's idempotence + round-trip run.
#[derive(Default)]
struct DirIdemResult {
    passed: usize,
    /// Source did not parse (legitimate skip).
    skipped: usize,
    /// Re-parse breaks listed in `KNOWN_REPARSE_BREAKS` (BUG-07-339), observed
    /// to still break in this run.
    known_breaks_hit: Vec<String>,
    /// Repo-relative paths whose SOURCE did not parse this run (`SourceParseSkip`).
    /// A `KNOWN_REPARSE_BREAKS` entry that dropped to `SourceParseSkip` is NOT a
    /// genuine fix — its source regressed to unparseable, the opposite of its
    /// output re-parsing clean — so the stale-check MUST NOT flag it (BUG-07-339).
    source_skipped: Vec<String>,
    /// New round-trip breaks + idempotence mismatches — hard failures.
    failures: Vec<String>,
}

/// Run idempotence + round-trip checks on all files in a directory.
fn run_idempotence_tests_for_dir(dir: &Path) -> DirIdemResult {
    let files = find_all_ori_files(dir);
    let mut result = DirIdemResult::default();

    for file in &files {
        match test_idempotence_for_file(file) {
            IdemOutcome::Pass => result.passed += 1,
            IdemOutcome::SourceParseSkip => {
                result.skipped += 1;
                result.source_skipped.push(repo_relative(file));
            }
            IdemOutcome::KnownReparseBreak => {
                result.known_breaks_hit.push(repo_relative(file));
            }
            IdemOutcome::NewReparseBreak(detail) => {
                result.failures.push(format!(
                    "{}: NEW re-parse break (not in BUG-07-339 allowlist): {}",
                    file.display(),
                    detail
                ));
            }
            IdemOutcome::IdempotenceFailure(detail) => {
                result
                    .failures
                    .push(format!("{}: {}", file.display(), detail));
            }
        }
    }

    result
}

/// Fail if any `KNOWN_REPARSE_BREAKS` entry was NOT observed to break across the
/// union of files this run scanned — a stale tracked entry (the round-trip
/// defect is fixed; the allowlist must shrink), mirroring
/// DISPOSITION_DRIFT:stale-tracking.
fn assert_no_stale_known_breaks(
    hit_repo_rels: &[String],
    scanned_repo_rels: &[String],
    source_skipped_rels: &[String],
) {
    let mut stale = Vec::new();
    for known in KNOWN_REPARSE_BREAKS {
        let known = *known;
        // Only adjudicate entries whose dir was actually scanned this run.
        if !scanned_repo_rels.iter().any(|p| p == known) {
            continue;
        }
        // A KNOWN entry that dropped to SourceParseSkip is NOT a genuine fix —
        // its SOURCE stopped parsing, not its formatted OUTPUT re-parsing clean.
        // Only a genuine fix (Pass: output re-parses) makes an entry stale.
        if source_skipped_rels.iter().any(|p| p == known) {
            continue;
        }
        if !hit_repo_rels.iter().any(|p| p == known) {
            stale.push(known);
        }
    }
    assert!(
        stale.is_empty(),
        "{} stale KNOWN_REPARSE_BREAKS entries now re-parse clean — remove them from the allowlist (BUG-07-339 progress):\n{}",
        stale.len(),
        stale.join("\n")
    );
}

/// Repo-relative paths of every `.ori` file under `dir`.
fn scanned_repo_relatives(dir: &Path) -> Vec<String> {
    find_all_ori_files(dir)
        .iter()
        .map(|p| repo_relative(p))
        .collect()
}

/// Assert a single directory's run carries no NEW round-trip break / idempotence
/// failure, and no stale tracked break. Known BUG-07-339 breaks are reported but
/// do not fail the run.
fn assert_dir_idem_clean(label: &str, dir: &Path, result: &DirIdemResult) {
    println!(
        "{}: {} passed, {} skipped, {} known-reparse-break (BUG-07-339), {} failed",
        label,
        result.passed,
        result.skipped,
        result.known_breaks_hit.len(),
        result.failures.len()
    );

    assert_no_stale_known_breaks(
        &result.known_breaks_hit,
        &scanned_repo_relatives(dir),
        &result.source_skipped,
    );

    assert!(
        result.failures.is_empty(),
        "{} idempotence / round-trip failures in {}:\n\n{}",
        result.failures.len(),
        label,
        result.failures.join("\n---\n")
    );
}

#[test]
fn idempotence_tests_spec() {
    let dir = repo_root().join("tests").join("spec");
    let result = run_idempotence_tests_for_dir(&dir);
    assert_dir_idem_clean("tests/spec", &dir, &result);
}

#[test]
fn idempotence_tests_run_pass() {
    let dir = repo_root().join("tests").join("run-pass");
    let result = run_idempotence_tests_for_dir(&dir);
    assert_dir_idem_clean("tests/run-pass", &dir, &result);
}

#[test]
fn idempotence_tests_fmt() {
    let dir = repo_root().join("tests").join("fmt");
    let result = run_idempotence_tests_for_dir(&dir);
    assert_dir_idem_clean("tests/fmt", &dir, &result);
}

#[test]
fn idempotence_tests_library() {
    let dir = repo_root().join("library");
    let result = run_idempotence_tests_for_dir(&dir);
    assert_dir_idem_clean("library/", &dir, &result);
}

/// Runs idempotence on all .ori files in the repository.
#[test]
fn idempotence_comprehensive() {
    let root = repo_root();
    let dirs = [
        root.join("tests").join("spec"),
        root.join("tests").join("run-pass"),
        root.join("tests").join("fmt"),
        root.join("library"),
    ];

    let mut total_passed = 0;
    let mut total_skipped = 0;
    let mut total_known_breaks = 0;
    let mut all_failures = Vec::new();
    let mut all_hit = Vec::new();
    let mut all_scanned = Vec::new();
    let mut all_source_skipped = Vec::new();

    for dir in &dirs {
        let result = run_idempotence_tests_for_dir(dir);
        total_passed += result.passed;
        total_skipped += result.skipped;
        total_known_breaks += result.known_breaks_hit.len();
        all_failures.extend(result.failures);
        all_hit.extend(result.known_breaks_hit);
        all_scanned.extend(scanned_repo_relatives(dir));
        all_source_skipped.extend(result.source_skipped);
    }

    println!(
        "\nComprehensive idempotence: {} passed, {} skipped, {} known-reparse-break (BUG-07-339), {} failed",
        total_passed, total_skipped, total_known_breaks, all_failures.len()
    );

    assert_no_stale_known_breaks(&all_hit, &all_scanned, &all_source_skipped);

    assert!(
        all_failures.is_empty(),
        "{} idempotence / round-trip failures:\n\n{}",
        all_failures.len(),
        all_failures.join("\n---\n")
    );
}
