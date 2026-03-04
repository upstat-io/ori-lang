/*
 * Forced-unwind test harness for ori_eh_personality.
 *
 * Verifies that:
 * 1. Catch-all landing pads are NOT entered during forced unwind
 *    (they don't resume, so installing them would swallow the unwind)
 * 2. Cleanup landing pads ARE entered during forced unwind
 *    (they call _Unwind_Resume, so unwinding continues)
 *
 * Uses setjmp/longjmp to escape the forced unwind before it destroys
 * the test function's stack frame.
 */

#include <unwind.h>
#include <stdint.h>
#include <string.h>
#include <setjmp.h>

/* Defined in test_frames_{x86_64,aarch64}.S */
extern int catch_handler_entered;
extern int cleanup_handler_entered;
extern int frame_with_catch_all(void (*trigger)(void));
extern int frame_with_cleanup(void (*trigger)(void));

/* Non-local escape: the stop callback longjmps when the unwinder
 * reaches the target frame (identified by CFA >= saved frame pointer),
 * BEFORE that frame is unwound — so the setjmp context is still valid. */
static jmp_buf escape_buf;
static uintptr_t escape_target_fp;

static struct _Unwind_Exception test_exc;

/* Exception cleanup callback (required by _Unwind_Exception contract) */
static void exc_cleanup(_Unwind_Reason_Code reason,
                        struct _Unwind_Exception *exc)
{
    (void)reason;
    (void)exc;
}

/*
 * Stop function for _Unwind_ForcedUnwind.
 *
 * Allows unwinding through inner frames (trigger, assembly stubs).
 * When the unwinder reaches the frame that called setjmp, longjmp
 * escapes before that frame is destroyed.
 *
 * Frame detection: the stop function is called BEFORE each frame is
 * unwound, with _Unwind_GetCFA(ctx) giving that frame's CFA. The
 * stack grows downward, so CFA increases as we unwind outward.
 * Inner frames have CFA < test function's FP. The test function's
 * CFA >= FP, which fires exactly when the unwinder reaches the test
 * function's frame — while it is still live.
 */
static _Unwind_Reason_Code force_unwind_stop(
    int version, _Unwind_Action actions, uint64_t exception_class,
    struct _Unwind_Exception *exc, struct _Unwind_Context *ctx,
    void *stop_parameter)
{
    (void)version;
    (void)exception_class;
    (void)exc;
    (void)stop_parameter;

    uintptr_t cfa = _Unwind_GetCFA(ctx);

    /*
     * Escape when we reach the test function's frame (CFA >= saved FP),
     * or when we hit the end of the stack (safety net — should not happen
     * with correct CFI, but prevents abort if unwind info is incomplete).
     */
    if (cfa >= escape_target_fp || (actions & _UA_END_OF_STACK)) {
        longjmp(escape_buf, 1);
    }
    return _URC_NO_REASON;
}

static void trigger_forced_unwind(void) {
    memset(&test_exc, 0, sizeof(test_exc));
    test_exc.exception_cleanup = exc_cleanup;
    _Unwind_ForcedUnwind(&test_exc, force_unwind_stop, NULL);
    __builtin_unreachable(); /* _Unwind_ForcedUnwind does not return */
}

/* Returns 0 on success: catch-all handler was NOT entered */
int test_forced_unwind_skips_catch(void) {
    catch_handler_entered = 0;
    escape_target_fp = (uintptr_t)__builtin_frame_address(0);
    if (setjmp(escape_buf) == 0) {
        frame_with_catch_all(trigger_forced_unwind);
        __builtin_unreachable();
    }
    /* Reached via longjmp — unwind stopped at our frame (still live) */
    return catch_handler_entered; /* 0 = pass, 1 = fail */
}

/* Returns 0 on success: cleanup handler WAS entered */
int test_forced_unwind_runs_cleanup(void) {
    cleanup_handler_entered = 0;
    escape_target_fp = (uintptr_t)__builtin_frame_address(0);
    if (setjmp(escape_buf) == 0) {
        frame_with_cleanup(trigger_forced_unwind);
        __builtin_unreachable();
    }
    /* Reached via longjmp — unwind stopped at our frame (still live) */
    return cleanup_handler_entered ? 0 : 1; /* 0 = pass, 1 = fail */
}
