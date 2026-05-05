#![no_main]

//! Single-phase fuzz target: lexer + parser + type checker + canonicalizer.
//!
//! Accepts arbitrary bytes; runs the in-process pipeline through
//! `ori_canon::lower_module`. Uses `ori_types::check_module_with_pool` (per
//! `compiler_repo/compiler/ori_types/src/check/api/mod.rs:108`) because
//! `lower_module` requires `&Pool` per
//! `compiler_repo/compiler/ori_canon/src/lower/mod.rs:95-101`; the basic
//! `check_module` discards the pool and is unsuitable here.
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
        let (checked, pool) =
            ori_types::check_module_with_pool(&parsed.module, &parsed.arena, &interner);
        let _canon =
            ori_canon::lower_module(&parsed.module, &parsed.arena, &checked, &pool, &interner);
    }));
});
