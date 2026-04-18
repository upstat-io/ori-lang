# ori_rt

> **`ori_rt` is the AOT runtime** — a C-ABI static library emitted binaries link against for RC operations, panic/unwind, and any intrinsic Ori's code generator produces calls for.
>
> Full mission: [`.claude/rules/missions.md §ori_rt`](../../.claude/rules/missions.md)

## Role in the runtime

Every AOT-compiled Ori binary links against `ori_rt`. It provides:

- **RC operations**: `ori_rc_inc`, `ori_rc_dec`, allocation (`ori_alloc`), deallocation (`ori_free`).
- **Panic/unwind**: panic handler + stack unwinding support.
- **Runtime assertions** (optional, gated by `ORI_RT_DEBUG=1`): header validation, RC balance checks.
- **Leak detection** (optional, gated by `ORI_CHECK_LEAKS=1`): per-allocation attribution for debugging.

The ABI contract between `ori_rt` and `ori_llvm` is load-bearing — function signatures in this crate must match codegen call sites in `ori_llvm` exactly.

## Runtime env vars (consumed here)

- `ORI_TRACE_RC=1` — RC event trace (alloc/inc/dec/free)
- `ORI_RT_DEBUG=1` — runtime assertions (header validation, balance checks)
- `ORI_CHECK_LEAKS=1` — leak detection with attribution

## Features

- `single-threaded` (optional) — single-threaded runtime build for WASM and embedded targets.

## Dependencies

| Direction | Crates |
|---|---|
| Upstream | external FFI + runtime deps only |
| Downstream | every AOT-compiled Ori binary (via `ori_llvm` codegen) |

## Invariants

- **ABI agrees with `ori_llvm` codegen exactly**: any signature change in `ori_rt` requires a matched commit to `ori_llvm` call sites. Silent drift produces memory corruption.
- **C-ABI only**: every export uses `#[no_mangle] extern "C"` with `ori_` prefix.
- **No user-facing panics**: runtime panics represent internal invariant violations only; user-facing errors belong at compile time.
- **Sanitizer-clean on smoke corpus**: `scripts/sanitizer-smoke.sh` (17 programs, O0+O2, ASan/UBSan) must pass.

## Testing

```bash
cargo test -p ori_rt

# Runtime smoke suite (sanitizer)
scripts/sanitizer-smoke.sh

# Runtime with debug assertions
ORI_RT_DEBUG=1 ./target/debug/ori run file.ori
```

## References

- [`.claude/rules/runtime.md`](../../.claude/rules/runtime.md) — runtime rules + ABI contract
- [`.claude/rules/impl-hygiene.md §Unsafe & FFI`](../../.claude/rules/impl-hygiene.md) — FFI discipline
- [`.claude/rules/aot.md`](../../.claude/rules/aot.md) — AOT pipeline that links against this
