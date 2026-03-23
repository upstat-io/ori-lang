/*
 * Ori Exception Handling — Windows MSVC Implementation
 *
 * Uses C++ exceptions (throw/catch) instead of Win32 RaiseException so that
 * LLVM-generated cleanup blocks (cleanuppad/cleanupret with __CxxFrameHandler3
 * personality) execute during unwind. Win32 RaiseException bypasses C++ frame
 * handlers, causing cleanup blocks to be skipped and leaking RC-managed memory.
 *
 * Architecture:
 *   - ori_raise_exception: throws OriPanicException (C++ exception)
 *   - ori_try_call: try/catch trampoline for catch(expr:) codegen
 *
 * The panic message is stored in thread-local storage by ori_panic/ori_panic_cstr
 * (Rust side) before ori_raise_exception is called.
 */

#include <cstdint>
#include <cstdio>
#include <cstdlib>

#define ORI_FATAL_EXIT_CODE (128 + 6)

/*
 * Custom C++ exception type for Ori panics.
 *
 * Intentionally minimal — the panic message is stored in thread-local
 * storage (Rust side), not in the exception object. This avoids
 * cross-language allocation issues and keeps the throw/catch lightweight.
 *
 * Using a dedicated type (not int, not std::exception) ensures that
 * ori_try_call catches ONLY Ori panics — other C++ exceptions, Rust
 * panics, and access violations propagate normally.
 */
struct OriPanicException {};

extern "C" {

/*
 * Raise an Ori exception via C++ throw.
 *
 * Called by ori_panic/ori_panic_cstr (Rust) after storing the panic
 * message in thread-local storage. The C++ exception propagates through
 * LLVM-generated functions with __CxxFrameHandler3 personality, which
 * executes cleanuppad blocks (RC decrements) during unwind.
 */
__declspec(noreturn)
void ori_raise_exception(void) {
    throw OriPanicException{};
}

/*
 * Try calling a thunk, catching Ori panics via C++ catch.
 *
 * Used by catch(expr:) codegen on MSVC. The LLVM backend generates a
 * thunk function `void (ptr %ctx)` that packs args into ctx, calls
 * the real function, and stores the result back.
 *
 * The catch clause only matches OriPanicException — all other exceptions
 * (access violations, other C++ exceptions, etc.) propagate normally.
 *
 * Returns 1 on success, 0 if an Ori panic was caught.
 * On catch, the panic message is available via ori_catch_recover (Rust).
 */
int64_t ori_try_call(void (*thunk)(void *), void *ctx) {
    try {
        thunk(ctx);
        return 1;
    } catch (const OriPanicException&) {
        return 0;
    }
}

} /* extern "C" */
