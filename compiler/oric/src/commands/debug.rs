//! Debug commands: `parse` and `lex` for inspecting compiler internals.

use oric::query::{parsed, tokens};
use oric::{CompilerDb, Db, SourceFile};
use std::path::PathBuf;

use super::read_file;

/// Parse a file and display AST information.
pub fn parse_file(path: &str) {
    let content = read_file(path);
    let db = CompilerDb::new();
    let file = SourceFile::new(&db, PathBuf::from(path), content);

    let parse_result = parsed(&db, file);

    println!("Parse result for '{path}':");
    println!("  Functions: {}", parse_result.module.functions.len());
    println!("  Expressions: {}", parse_result.arena.expr_count());
    println!("  Errors: {}", parse_result.errors.len());

    if !parse_result.module.functions.is_empty() {
        println!();
        println!("Functions:");
        for func in &parse_result.module.functions {
            let name = db.interner().lookup(func.name);
            let params = parse_result.arena.get_params(func.params);
            let param_names: Vec<_> = params
                .iter()
                .map(|p| db.interner().lookup(p.name))
                .collect();
            println!("  @{} ({})", name, param_names.join(", "));
        }
    }

    if !parse_result.errors.is_empty() {
        println!();
        println!("Errors:");
        for error in &parse_result.errors {
            println!("  {}: {}", error.span(), error.message());
        }
    }
}

/// Lex a file and display the token stream.
pub fn lex_file(path: &str) {
    let content = read_file(path);
    let db = CompilerDb::new();
    let file = SourceFile::new(&db, PathBuf::from(path), content);

    let toks = tokens(&db, file);

    println!("Tokens for '{}' ({} tokens):", path, toks.len());
    for tok in toks.iter() {
        println!("  {:?} @ {}", tok.kind, tok.span);
    }
}
