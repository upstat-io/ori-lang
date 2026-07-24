#![no_main]

//! Single-phase fuzz target: lexer + parser robustness.
//!
//! Accepts arbitrary bytes; converts to `&str` (returning early on invalid
//! UTF-8 — this is NOT a divergence, just an invalid input shape libFuzzer
//! supplies); runs `ori_lexer::lex` + `ori_parse::parse`. Wraps the call in
//! `std::panic::catch_unwind` per constraint #2 of §10 — panics MUST be
//! caught so libFuzzer reports them as crashes rather than losing them when
//! the worker thread aborts.
//!
//! Smoke target at §10.1; the in-process pipeline is wired here. Full
//! per-phase signal (parse-error stats, recovery quality) lands in §10.3.

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(source) = std::str::from_utf8(data) else {
        return;
    };

    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let interner = ori_ir::StringInterner::new();
        let tokens = ori_lexer::lex(source, &interner);
        let _parsed = ori_parse::parse(&tokens, &interner);
    }));
});
