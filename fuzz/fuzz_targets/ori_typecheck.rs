#![no_main]

//! Single-phase fuzz target: lexer + parser + type checker robustness.
//!
//! Accepts arbitrary bytes; runs the in-process pipeline through
//! `ori_types::check_module`. Per `compiler_repo/compiler/ori_types/src/check/api/mod.rs:61`,
//! `check_module(module, arena, interner)` is the lowest-arity entry — fits
//! the smoke-only goal of §10.1. Richer entry points (`check_module_with_pool`,
//! `check_module_with_imports`) are wired in §10.3 / §10.4 as needed.
//!
//! Wraps the call in `std::panic::catch_unwind` per constraint #2 of §10.

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(source) = std::str::from_utf8(data) else {
        return;
    };

    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let interner = ori_ir::StringInterner::new();
        let tokens = ori_lexer::lex(source, &interner);
        let parsed = ori_parse::parse(&tokens, &interner);
        let _checked = ori_types::check_module(&parsed.module, &parsed.arena, &interner);
    }));
});
