---
plan: "aot-perf"
title: "AOT Codegen Performance"
status: active
keywords:
  overflow:
    - checked_add, checked_sub, checked_mul, checked_neg
    - sadd.with.overflow, ssub.with.overflow, smul.with.overflow
    - add nsw, sub nsw, nuw, overflow elision
    - emit_int_binary_op, emit_checked_binop, CseOperand
    - checked_ops.rs, strategy.rs, arithmetic.rs
    - ori_panic_cstr, ovf.msg, panic block
    - range metadata, llvm.assume, post-guard narrowing
  string_indexing:
    - __index, ProtocolBuiltin::Index, apply_protocols.rs
    - TypeInfo::Str, ori_str_index, ori_str_get
    - emit_list_index, emit_map_get, try_emit_protocol
    - string indexing, s[i], codepoint, UTF-8
    - ori_rt, string/ops.rs
  benchmarks:
    - bench_compute, bench_recursion, bench_alloc, bench_string
    - hyperfine, perf-baseline.sh
    - overflow check count, 19 vs 8
---

# AOT Codegen Performance

Two benchmark-discovered issues: excess overflow checks (19 where Rust needs 8) and missing string indexing codegen.

## Sections

| # | Title | Goal |
|---|-------|------|
| 01 | Overflow Check Elision | Reduce unnecessary overflow checks to match Rust with `-C overflow-checks=yes` |
| 02 | String Indexing Codegen | Implement `s[i]` in AOT — add `TypeInfo::Str` handler + runtime function |
