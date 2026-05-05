//! Helper binary used by `populate-fuzz-corpus.sh` for the parse-stage corpus
//! filter. Reads an `.ori` source file from `argv[1]`, runs `ori_lexer::lex` +
//! `ori_parse::parse`, and exits 0 iff parsing reported no errors.
//!
//! This wraps the SAME in-process pipeline the `ori_parse` fuzz target uses,
//! so the corpus filter accepts exactly the inputs the fuzz target does.

use std::process::ExitCode;

fn main() -> ExitCode {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: parse-only-check <path-to-source.ori>");
        return ExitCode::from(2);
    };

    let source = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("parse-only-check: failed to read {path}: {e}");
            return ExitCode::from(2);
        }
    };

    let interner = ori_ir::StringInterner::new();
    let tokens = ori_lexer::lex(&source, &interner);
    let parsed = ori_parse::parse(&tokens, &interner);

    if parsed.has_errors() {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}
