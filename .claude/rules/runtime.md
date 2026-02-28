---
paths:
  - "**/ori_rt/**"
---

**NO WORKAROUNDS/HACKS/SHORTCUTS.** Proper fixes only. When unsure, STOP and ask. Fact-check against spec. Consult `~/projects/reference_repos/lang_repos/` (includes Swift for ARC, Koka for effects, Lean 4 for RC).

**Ori tooling is under construction** — bugs are usually in compiler, not user code. This is one system: every piece must fit for any piece to work. Fix every issue you encounter — no "unrelated", no "out of scope", no "pre-existing." If it's broken, research why and fix it.

# Runtime Library (ori_rt)

C-ABI functions for LLVM-generated AOT code.

## Build Outputs
- **rlib**: Rust consumers (JIT)
- **staticlib**: AOT linking (`libori_rt.a`)

Both built with `cargo build -p ori_rt`.

## FFI Conventions
- All functions: `#[no_mangle] extern "C"`
- `#[repr(C)]` for FFI types
- Pointers from LLVM guaranteed valid

## Type Representations
- `str` → `{ len: i64, cap: i64, data: *mut u8 }` (24-byte SSO layout)
- `[T]` → `{ len: i64, cap: i64, data: *mut u8 }`
- `Option<T>` → `{ tag: i64, value: T }`

## Functions

| Category | Functions |
|----------|-----------|
| Memory | `ori_alloc`, `ori_free`, `ori_realloc` |
| RefCount | `ori_rc_alloc`, `ori_rc_inc`, `ori_rc_dec`, `ori_rc_free` (8-byte header, `drop_fn` for children) |
| Strings | `ori_str_concat`, `ori_str_eq`, `ori_str_ne`, `ori_str_compare`, `ori_str_hash`, `ori_str_from_int/bool/float`, `ori_str_next_char` |
| I/O | `ori_print`, `ori_print_int`, `ori_print_float`, `ori_print_bool` |
| Lists | `ori_list_new`, `ori_list_free`, `ori_list_len`, `ori_list_alloc_data`, `ori_list_free_data` |
| Comparison | `ori_compare_int`, `ori_min_int`, `ori_max_int` |
| Assertions | `ori_assert`, `ori_assert_eq_int/bool/float/str` |
| Panic | `ori_panic`, `ori_panic_cstr`, `ori_register_panic_handler` |
| Entry | `ori_run_main`, `ori_args_from_argv` |

## Submodules

- `format/` — Template string interpolation (`ori_format_int/float/str/bool/char`)
- `iterator.rs` — Iterator runtime (`ori_iter_from_list/range`, `ori_iter_next`, `ori_iter_map/filter/take/skip/enumerate/collect/count/drop`)

## JIT Panic Recovery
- `JmpBuf` + `jit_setjmp`/`enter_jit_mode`/`leave_jit_mode` for `setjmp`/`longjmp`-based recovery
- `did_panic`, `get_panic_message`, `reset_panic_state` for test assertions

## Debugging

For LLVM IR debugging workflow, tools, common bug categories, and verification strategy, see @llvm.md

**Runtime env vars:**
- `ORI_TRACE_RC=1` — Logs every RC alloc/inc/dec/free event to stderr
- `ORI_TRACE_RC=verbose` — Adds backtraces to RC events
- `ORI_RT_DEBUG=1` — Enables runtime assertions (RC header validation, bounds checks)
- `ORI_CHECK_LEAKS=1` — Reports unfreed allocations with allocation-site attribution

**RC debugging scripts** — use these for memory/RC issues:
- `diagnostics/rc-stats.sh <file.ori>` — static RC balance analysis per function in LLVM IR
- `diagnostics/codegen-audit.sh <file.ori>` — in-pipeline RC balance, COW correctness, ABI checks (`--strict`)
- `diagnostics/diagnose-aot.sh <file.ori>` — all-in-one: build + run + `ORI_CHECK_LEAKS` + RC stats + IR (add `--valgrind`)
- `diagnostics/dual-exec-debug.sh <file.ori>` — interpreter vs AOT comparison, auto-dumps on mismatch
