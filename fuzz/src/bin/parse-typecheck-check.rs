//! Helper binary used by `populate-fuzz-corpus.sh` for the canon-stage corpus
//! filter. Reads an `.ori` source file from `argv[1]`, runs
//! `ori_lexer::lex` + `ori_parse::parse` + `ori_types::check_module`, and
//! exits 0 iff none of the three phases reports errors.
//!
//! Per §10.1: this wraps the SAME in-process pipeline the `ori_typecheck`
//! fuzz target uses. `cargo run -p oric -- check` would route through Salsa
//! with different prelude resolution and could admit programs the in-process
//! pipeline rejects (or vice versa).

use std::process::ExitCode;

fn main() -> ExitCode {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: parse-typecheck-check <path-to-source.ori>");
        return ExitCode::from(2);
    };

    let source = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("parse-typecheck-check: failed to read {path}: {e}");
            return ExitCode::from(2);
        }
    };

    let interner = ori_ir::StringInterner::new();
    let tokens = ori_lexer::lex(&source, &interner);
    let parsed = ori_parse::parse(&tokens, &interner);

    if parsed.has_errors() {
        return ExitCode::from(1);
    }

    let checked = ori_types::check_module(&parsed.module, &parsed.arena, &interner);

    if checked.has_errors() {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}
