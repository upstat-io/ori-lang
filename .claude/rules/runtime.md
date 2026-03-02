---
paths:
  - "**ori_rt**"
---

# Runtime Library (ori_rt)

- C-ABI functions for LLVM-generated AOT code
- **rlib** for Rust consumers (JIT) | **staticlib** for AOT linking (`libori_rt.a`)
- Both built with `cargo build -p ori_rt`

## FFI Conventions

- All functions: `#[no_mangle] extern "C"` | `#[repr(C)]` for FFI types
- Pointers from LLVM guaranteed valid
- **`c_char` not `i8`**: C string pointers MUST use `std::ffi::c_char`, never `i8`. `c_char` is `i8` on x86_64 but `u8` on aarch64/ARM — hardcoding `i8` breaks ARM builds. Applies to `ori_panic_cstr`, `ori_args_from_argv`, and any future C string FFI.

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

- For LLVM IR debugging workflow and verification, see @llvm.md
- **Runtime env vars**: `ORI_TRACE_RC=1` (RC event log) | `verbose` (adds backtraces) | `ORI_RT_DEBUG=1` (runtime assertions) | `ORI_CHECK_LEAKS=1` (leak report with attribution)
- **Diagnostic scripts**: `diagnostics/rc-stats.sh` | `codegen-audit.sh` | `diagnose-aot.sh` | `dual-exec-debug.sh` (see compiler.md for full list)
