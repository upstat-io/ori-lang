---
paths:
  - "compiler/ori_rt/**/*.rs"
---

# Runtime Library (ori_rt)

- C-ABI functions for LLVM-generated AOT code
- **rlib** for Rust consumers (JIT) | **staticlib** for AOT linking (`libori_rt.a`)
- Both built with `cargo build -p ori_rt`

## FFI Conventions

- **Two ABI classes**: most functions are `#[no_mangle] extern "C"` (non-unwinding); panic and assertion entry points are `#[no_mangle] extern "C-unwind"` (may unwind for cleanup-pad semantics). See below.
- `#[repr(C)]` for FFI types | Pointers from LLVM guaranteed valid
- **`c_char` not `i8`**: C string pointers MUST use `std::ffi::c_char`, never `i8`. `c_char` is `i8` on x86_64 but `u8` on aarch64/ARM — hardcoding `i8` breaks ARM builds. Applies to `ori_panic_cstr`, `ori_args_from_argv`, and any future C string FFI.

### Unwinding ABI (`extern "C-unwind"`)

The following functions use `extern "C-unwind"` because they may unwind via Ori's exception mechanism (Itanium `_Unwind_RaiseException` or MSVC SEH). Rust must NOT insert an abort guard before the exception reaches LLVM cleanup pads. These functions intentionally lack `Nounwind` in `ori_llvm`'s runtime declaration table; the audit test `all_non_unwinding_functions_have_nounwind` in `runtime_decl/tests.rs` enforces this split.

| Function | Reason |
|----------|--------|
| `ori_panic` | Raises Ori exception after storing panic state |
| `ori_panic_cstr` | Same as `ori_panic`, C-string variant |
| `ori_assert` | Calls `ori_panic_cstr` on failure |
| `ori_assert_eq_int` | Calls `ori_panic_cstr` on failure |
| `ori_assert_eq_bool` | Calls `ori_panic_cstr` on failure |
| `ori_assert_eq_float` | Calls `ori_panic_cstr` on failure |
| `ori_assert_eq_str` | Calls `ori_panic_cstr` on failure |
| `ori_list_get` | Panics on out-of-bounds access |

All other `ori_*` exports are plain `extern "C"` and carry `Nounwind` in the LLVM declaration. This distinction is load-bearing for `codegen-rules.md` RT-1 (see §RT-1 unwinding addendum).

## Type Representations

### `str` — `OriStr` (24-byte SSO/heap union)

The runtime `OriStr` is a 24-byte `#[repr(C)]` union of two layouts, discriminated by byte 23:
- **SSO** (`OriStrSSO`): `{ bytes: [u8; 23], flags: u8 }` — strings <= 23 bytes stored inline (no heap, no RC). High bit of `flags` = 1 (SSO flag); low 7 bits = length.
- **Heap** (`OriStrHeap`): `{ len: i64, cap: i64, data: *mut u8 }` — heap-allocated RC-managed buffer. High bit of byte 23 (MSB of `data` pointer) is always 0 on user-space 64-bit platforms.

Source: `compiler/ori_rt/src/string/mod.rs`.

### Collections — fat pointer `{ len: i64, cap: i64, data: ptr }`

- `[T]`, `{K: V}`, `Set<T>` all use the 24-byte fat pointer layout per `codegen-rules.md` TR-4.
- `str` uses the same fat pointer shape at the **codegen level** (LLVM emission via `CG:TR-4`), but the runtime's `OriStr` union adds SSO — the two views agree on heap layout but differ on the SSO fast path.

### `Option<T>` / `Result<T, E>` — layout owned by `TypeLayoutResolver`

The **language-level** layout of `Option<T>` and `Result<T, E>` is determined by `ori_llvm`'s `TypeLayoutResolver`, which may use niche encoding (eliding the tag when the payload has a niche) or tagged encoding per `codegen-rules.md` TR-1 and `repr.md` §7. Do NOT assume a fixed `{ tag, payload }` shape.

The runtime crate exposes **helper structs** `OriOption<T>` (`{ tag: i8, value: T }`, tag 0=Some, 1=None) and `OriResult<T>` (`{ tag: i8, value: T }`, tag 0=Ok, 1=Err) at `compiler/ori_rt/src/lib.rs` for runtime-internal use. These are NOT the canonical language ABI — they are convenience types for runtime code that needs to return simple tagged values to codegen.

## Functions

| Category | Functions |
|----------|-----------|
| Memory | `ori_alloc`, `ori_free`, `ori_realloc` |
| RefCount | `ori_rc_alloc`, `ori_rc_inc`, `ori_rc_dec`, `ori_rc_free` (32-byte V5 header: `data_size`, `elem_dec_fn`, `elem_count`, `strong_count`; `drop_fn` for non-buffer RC objects), `ori_buffer_store_elem_dec`, `ori_buffer_store_elem_count` |
| Strings | `ori_str_concat`, `ori_str_eq`, `ori_str_ne`, `ori_str_compare`, `ori_str_hash`, `ori_str_from_int/bool/float`, `ori_str_next_char`, `ori_str_rc_inc`, `ori_str_rc_dec` |
| I/O | `ori_print`, `ori_print_int`, `ori_print_float`, `ori_print_bool` |
| Lists | `ori_list_new`, `ori_list_free`, `ori_list_len`, `ori_list_alloc_data`, `ori_list_free_data` |
| Comparison | `ori_compare_int`, `ori_min_int`, `ori_max_int` |
| Assertions | `ori_assert`, `ori_assert_eq_int/bool/float/str` |
| Panic | `ori_panic`, `ori_panic_cstr`, `ori_register_panic_handler` |
| Entry | `ori_run_main`, `ori_args_from_argv` |

## Submodules

- `format/` — Template string interpolation (`ori_format_int/float/str/bool/char`)
- `iterator/` — Iterator runtime directory module (`mod.rs`, `sources.rs`, `state.rs`, `next.rs`, `adapters.rs`, `consumers.rs`, `tests.rs`). Entry points: `ori_iter_from_list` (4 params: data, len, cap, elem_size), `ori_iter_from_range`, `ori_iter_next`, `ori_iter_map/filter/take/skip/enumerate/collect/count/drop`. Element cleanup info is retrieved from the V5 RC header at cleanup time — `elem_dec_fn` is NOT a parameter to `ori_iter_from_list`.

## JIT Panic Recovery

- `JmpBuf` + `jit_setjmp`/`enter_jit_mode`/`leave_jit_mode` for `setjmp`/`longjmp`-based recovery
- `did_panic`, `get_panic_message`, `reset_panic_state` for test assertions

## Debugging

- For LLVM IR debugging workflow and verification, see @llvm.md
- **Runtime env vars**: `ORI_TRACE_RC=1` (RC event log) | `verbose` (adds backtraces) | `ORI_RT_DEBUG=1` (runtime assertions) | `ORI_CHECK_LEAKS=1` (leak report with attribution)
- **Diagnostic scripts**: see @diagnostic.md §Diagnostic Scripts for full list and flags
