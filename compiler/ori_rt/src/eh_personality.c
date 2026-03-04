/*
 * Ori Exception Handling — Platform-Specific Implementations
 *
 * Two EH backends, zero Rust runtime dependency:
 *
 *   1. Itanium (Linux, macOS, MinGW):
 *      - ori_eh_personality: LSDA-based personality for invoke/landingpad
 *      - ori_raise_exception: _Unwind_RaiseException with OriException object
 *
 *   2. Windows SEH (MSVC):
 *      - ori_raise_exception: Win32 RaiseException with custom exception code
 *      - ori_try_call: __try/__except trampoline for catch(expr:)
 *
 * The panic message is stored in thread-local storage by ori_panic/ori_panic_cstr
 * (Rust side) before either raise path is invoked.
 */

#ifdef _MSC_VER
/* ═══════════════════════════════════════════════════════════════════
 * Windows SEH Implementation
 * ═══════════════════════════════════════════════════════════════════ */

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

#define WIN32_LEAN_AND_MEAN
#include <windows.h>

/*
 * Custom exception code for Ori panics.
 *
 * Bits 31-30: 11 (severity = error)
 * Bit  29:     1 (customer-defined, not Microsoft)
 * Bit  28:     0 (reserved)
 * Bits 27-0:   0x524900 ("RI\0" — from "ORI")
 *
 * This distinguishes Ori exceptions from C++, SEH, and other
 * language exceptions. The EXCEPTION_NONCONTINUABLE flag prevents
 * the handler from continuing execution at the faulting instruction.
 */
#define ORI_SEH_EXCEPTION_CODE 0xE0524900UL

/*
 * Raise an Ori exception via Win32 SEH.
 *
 * Called by ori_panic/ori_panic_cstr (Rust) after storing the panic
 * message in thread-local storage. Uses RaiseException with a custom
 * code that ori_try_call's __except filter recognizes.
 *
 * EXCEPTION_NONCONTINUABLE: the exception cannot be continued —
 * attempting to continue after this exception terminates the process.
 */
__declspec(noreturn)
void ori_raise_exception(void) {
    RaiseException(ORI_SEH_EXCEPTION_CODE, EXCEPTION_NONCONTINUABLE, 0, NULL);
    /* RaiseException does not return when EXCEPTION_NONCONTINUABLE is set
     * and a handler is found. If no handler exists, Windows terminates
     * the process. This abort() is a safety net. */
    abort();
}

/*
 * Try calling a thunk, catching Ori panics via SEH.
 *
 * Used by catch(expr:) codegen on MSVC. The LLVM backend generates a
 * thunk function `void (ptr %ctx)` that packs args into ctx, calls
 * the real function, and stores the result back.
 *
 * The __except filter only catches ORI_SEH_EXCEPTION_CODE — all other
 * exceptions (access violations, C++ exceptions, etc.) propagate normally.
 *
 * Returns 1 on success, 0 if an Ori panic was caught.
 * On catch, the panic message is available via ori_catch_recover (Rust).
 */
int64_t ori_try_call(void (*thunk)(void *), void *ctx) {
    __try {
        thunk(ctx);
        return 1;
    }
    __except (GetExceptionCode() == ORI_SEH_EXCEPTION_CODE
                  ? EXCEPTION_EXECUTE_HANDLER
                  : EXCEPTION_CONTINUE_SEARCH) {
        return 0;
    }
}

#else
/* ═══════════════════════════════════════════════════════════════════
 * Itanium EH Implementation (Linux, macOS, MinGW)
 * ═══════════════════════════════════════════════════════════════════ */

/*
 * Implements the Itanium C++ ABI personality routine for Ori's LLVM-compiled
 * code. Every function emitted with `invoke`/`landingpad` references this
 * symbol via `personality ptr @ori_eh_personality`.
 *
 * Ori uses two landing pad types:
 *   1. Cleanup  (action == 0): ARC reference count decrements on unwind.
 *      Ends with `resume` — continues unwinding after cleanup.
 *   2. Catch-all (ttype_index == 0 in action table): `catch()` expressions.
 *      Absorbs the exception — does NOT resume.
 *
 * During forced unwind (_UA_FORCE_UNWIND), catch-all pads are skipped because
 * they don't resume — installing them would swallow the forced unwind.
 * Cleanup pads always run (they resume, so unwinding continues).
 *
 * References:
 *   - Itanium C++ ABI: Exception Handling
 *     https://itanium-cxx-abi.github.io/cxx-abi/abi-eh.html
 *   - DWARF Exception Header Encoding
 *     https://refspecs.linuxfoundation.org/LSB_5.0.0/LSB-Core-generic/LSB-Core-generic/dwarfext.html
 *   - GCC gcc_personality_v0.c (via libunwind)
 *   - Rust library/std/src/sys/personality/gcc.rs
 */

#include <unwind.h>
#include <stdint.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>

/* ── DWARF pointer encoding constants ────────────────────────────── */

#define DW_EH_PE_absptr  0x00
#define DW_EH_PE_uleb128 0x01
#define DW_EH_PE_udata2  0x02
#define DW_EH_PE_udata4  0x03
#define DW_EH_PE_udata8  0x04
#define DW_EH_PE_sleb128 0x09
#define DW_EH_PE_sdata2  0x0A
#define DW_EH_PE_sdata4  0x0B
#define DW_EH_PE_sdata8  0x0C
#define DW_EH_PE_pcrel   0x10
#define DW_EH_PE_omit    0xFF

/* ── LEB128 decoders ─────────────────────────────────────────────── */

static uintptr_t read_uleb128(const uint8_t **p) {
    uintptr_t result = 0;
    int shift = 0;
    uint8_t byte;
    do {
        byte = **p;
        (*p)++;
        result |= (uintptr_t)(byte & 0x7F) << shift;
        shift += 7;
    } while (byte & 0x80);
    return result;
}

static intptr_t read_sleb128(const uint8_t **p) {
    intptr_t result = 0;
    int shift = 0;
    uint8_t byte;
    do {
        byte = **p;
        (*p)++;
        result |= (intptr_t)(byte & 0x7F) << shift;
        shift += 7;
    } while (byte & 0x80);
    if ((shift < (int)(sizeof(intptr_t) * 8)) && (byte & 0x40))
        result |= -(((intptr_t)1) << shift);
    return result;
}

/* ── Encoded pointer reader ──────────────────────────────────────── */

/*
 * Read a DWARF-encoded value from the LSDA byte stream.
 *
 * Only the lower 4 bits (value format) are used here — call-site table
 * entries are offsets, not relocated pointers, so the upper 4 bits
 * (application/relocation) are not applicable.
 *
 * Returns the decoded value and advances *p past the encoded bytes.
 */
static uintptr_t read_encoded_value(const uint8_t **p, uint8_t encoding) {
    if (encoding == DW_EH_PE_omit) return 0;

    uintptr_t result;
    uint8_t format = encoding & 0x0F;

    switch (format) {
    case DW_EH_PE_absptr:
        /* Natural-width pointer */
        result = *(const uintptr_t *)*p;
        *p += sizeof(uintptr_t);
        break;
    case DW_EH_PE_uleb128:
        result = read_uleb128(p);
        break;
    case DW_EH_PE_udata2:
        result = (uintptr_t)(*(const uint16_t *)*p);
        *p += 2;
        break;
    case DW_EH_PE_udata4:
        result = (uintptr_t)(*(const uint32_t *)*p);
        *p += 4;
        break;
    case DW_EH_PE_udata8:
        result = (uintptr_t)(*(const uint64_t *)*p);
        *p += 8;
        break;
    case DW_EH_PE_sleb128:
        result = (uintptr_t)read_sleb128(p);
        break;
    case DW_EH_PE_sdata2:
        result = (uintptr_t)(intptr_t)(*(const int16_t *)*p);
        *p += 2;
        break;
    case DW_EH_PE_sdata4:
        result = (uintptr_t)(intptr_t)(*(const int32_t *)*p);
        *p += 4;
        break;
    case DW_EH_PE_sdata8:
        result = (uintptr_t)(intptr_t)(*(const int64_t *)*p);
        *p += 8;
        break;
    default:
        /* Unknown encoding — should not happen with LLVM-emitted LSDA */
        __builtin_unreachable();
    }

    return result;
}

/* ── Action classification ───────────────────────────────────────── */

/* What the personality should do for a matched call-site entry */
enum eh_action {
    EH_ACTION_NONE,     /* No landing pad — continue unwinding */
    EH_ACTION_CLEANUP,  /* Cleanup pad — always install (runs ARC cleanup + resume) */
    EH_ACTION_CATCH_ALL /* Catch-all pad — skip during forced unwind */
};

/* ── Personality function ────────────────────────────────────────── */

/*
 * Ori's Itanium EH ABI personality function.
 *
 * Called by the platform unwinder (libunwind / libgcc_s) during stack
 * unwinding. Reads the LSDA to classify the current frame's landing pad
 * and tells the unwinder what to do.
 *
 * __attribute__((used)) prevents the C compiler from dead-code-eliminating
 * this function when it appears unreferenced within the translation unit.
 */
__attribute__((used))
_Unwind_Reason_Code ori_eh_personality(
    int version,
    _Unwind_Action actions,
    uint64_t exception_class,
    struct _Unwind_Exception *exception_object,
    struct _Unwind_Context *context)
{
    (void)exception_class; /* Ori catches all exception types uniformly */

    /* Version gate: Itanium ABI specifies version 1 */
    if (version != 1)
        return _URC_FATAL_PHASE1_ERROR;

    /* Get the LSDA for this frame */
    const uint8_t *lsda = (const uint8_t *)_Unwind_GetLanguageSpecificData(context);
    if (!lsda)
        return _URC_CONTINUE_UNWIND;

    /* Function start address (region start in Itanium terms) */
    uintptr_t func_start = _Unwind_GetRegionStart(context);

    /* ── Parse LSDA header ───────────────────────────────────────── */

    const uint8_t *p = lsda;

    /* Landing pad base: defaults to func_start if encoding is omit */
    uintptr_t lpad_base = func_start;
    uint8_t lp_start_encoding = *p++;
    if (lp_start_encoding != DW_EH_PE_omit) {
        lpad_base = read_encoded_value(&p, lp_start_encoding);
        /* Apply PC-relative relocation if needed */
        if (lp_start_encoding & DW_EH_PE_pcrel)
            lpad_base += (uintptr_t)(p - sizeof(uintptr_t));
    }

    /* Type table encoding — we skip the type table (Ori has no typed catches) */
    uint8_t ttype_encoding = *p++;
    if (ttype_encoding != DW_EH_PE_omit) {
        read_uleb128(&p); /* skip ttype_base_offset */
    }

    /* Call-site table */
    uint8_t cs_encoding = *p++;
    uintptr_t cs_table_length = read_uleb128(&p);
    const uint8_t *cs_table_start = p;
    const uint8_t *cs_table_end = p + cs_table_length;
    const uint8_t *action_table = cs_table_end;

    /* ── Get adjusted IP ─────────────────────────────────────────── */

    /*
     * The unwinder provides the return address (instruction AFTER the call).
     * Subtract 1 to map back into the calling instruction's range.
     * This is what every production personality does (GCC, Rust, libcxxabi).
     *
     * _Unwind_GetIPInfo provides an ip_before_instr flag for signal frames
     * where the IP is already at the faulting instruction, but it's not
     * universally available. Since Ori doesn't generate signal-frame
     * landing pads, the unconditional -1 is correct and simpler.
     */
    uintptr_t ip = _Unwind_GetIP(context) - 1;

    /* ── Walk call-site table ────────────────────────────────────── */

    uintptr_t landing_pad = 0;
    enum eh_action action = EH_ACTION_NONE;

    p = cs_table_start;
    while (p < cs_table_end) {
        uintptr_t cs_start  = read_encoded_value(&p, cs_encoding);
        uintptr_t cs_len    = read_encoded_value(&p, cs_encoding);
        uintptr_t cs_lpad   = read_encoded_value(&p, cs_encoding);
        uintptr_t cs_action = read_uleb128(&p);

        /* Does this call-site entry cover our IP? */
        if (ip < func_start + cs_start)
            break; /* Table is sorted — no point continuing */

        if (ip < func_start + cs_start + cs_len) {
            /* Found our entry */
            if (cs_lpad == 0) {
                /* No landing pad — continue unwinding */
                action = EH_ACTION_NONE;
            } else {
                landing_pad = lpad_base + cs_lpad;

                if (cs_action == 0) {
                    /* action == 0: cleanup only (no action record) */
                    action = EH_ACTION_CLEANUP;
                } else {
                    /*
                     * action > 0: index into action table.
                     * Read first ttype_index (SLEB128) at action_table + (cs_action - 1).
                     *
                     * ttype_index >  0 → catch handler (type table entry; NULL = catch-all)
                     * ttype_index == 0 → cleanup only (no catch handler)
                     * ttype_index <  0 → exception spec filter (not used by Ori)
                     */
                    const uint8_t *action_pos = action_table + cs_action - 1;
                    intptr_t ttype_index = read_sleb128(&action_pos);

                    if (ttype_index > 0) {
                        /*
                         * Positive ttype_index = catch handler.
                         *
                         * In the Itanium ABI, `catch ptr null` (LLVM's catch-all)
                         * generates ttype_index > 0 pointing to a NULL entry in the
                         * type table. Ori only generates `catch ptr null`, so all
                         * positive ttype_index values are catch-all handlers.
                         *
                         * A proper implementation would look up the type table entry
                         * and check for NULL, but since Ori has no typed catches,
                         * this simplification is correct and matches Rust's approach.
                         */
                        action = EH_ACTION_CATCH_ALL;
                    } else if (ttype_index == 0) {
                        /* ttype_index == 0: cleanup only (no catch handler) */
                        action = EH_ACTION_CLEANUP;
                    } else {
                        /* ttype_index < 0: exception spec filter (unused by Ori) */
                        action = EH_ACTION_CLEANUP;
                    }
                }
            }
            break;
        }
    }

    /* ── Phase 1: Search ─────────────────────────────────────────── */

    if (actions & _UA_SEARCH_PHASE) {
        switch (action) {
        case EH_ACTION_CATCH_ALL:
            /*
             * During forced unwind, never claim to be a handler.
             * Catch-all pads don't resume, so installing them would
             * swallow the forced unwind and hang the thread.
             */
            if (actions & _UA_FORCE_UNWIND)
                return _URC_CONTINUE_UNWIND;
            return _URC_HANDLER_FOUND;
        case EH_ACTION_CLEANUP:
        case EH_ACTION_NONE:
            return _URC_CONTINUE_UNWIND;
        }
    }

    /* ── Phase 2: Cleanup / Install ──────────────────────────────── */

    switch (action) {
    case EH_ACTION_NONE:
        return _URC_CONTINUE_UNWIND;

    case EH_ACTION_CATCH_ALL:
        /*
         * During forced unwind, skip catch-all pads entirely.
         * They absorb the exception (no resume), which would stop
         * the forced unwind from completing.
         */
        if (actions & _UA_FORCE_UNWIND)
            return _URC_CONTINUE_UNWIND;
        /* Fall through to install landing pad */
        /* fallthrough */

    case EH_ACTION_CLEANUP:
        /*
         * Install the landing pad: set exception object in data reg 0,
         * selector (always 0 for Ori) in data reg 1, and jump to the pad.
         *
         * __builtin_eh_return_data_regno gives the architecture-specific
         * DWARF register numbers:
         *   x86_64:  reg 0 = RAX, reg 1 = RDX
         *   aarch64: reg 0 = X0,  reg 1 = X1
         */
        _Unwind_SetGR(context, __builtin_eh_return_data_regno(0),
                       (uintptr_t)exception_object);
        _Unwind_SetGR(context, __builtin_eh_return_data_regno(1), 0);
        _Unwind_SetIP(context, landing_pad);
        return _URC_INSTALL_CONTEXT;
    }

    /* Should be unreachable — all enum cases handled above */
    return _URC_FATAL_PHASE2_ERROR;
}

/* ── Ori Exception Object ───────────────────────────────────────── */

/*
 * Ori exception class identifier (8 bytes).
 *
 * Itanium ABI: exception_class is a vendor/language identifier.
 * Convention: 4-char vendor + 4-char language, null-terminated.
 * "ORI\0PNC\0" → Ori vendor, PNC (panic) language.
 *
 * This distinguishes Ori exceptions from Rust, C++, or other
 * language exceptions during mixed-language unwinding.
 */
#define ORI_EXCEPTION_CLASS 0x4f524900504e4300ULL

/*
 * Ori exception object.
 *
 * Wraps the Itanium _Unwind_Exception header. The header contains:
 * - exception_class: ORI_EXCEPTION_CLASS
 * - exception_cleanup: ori_exception_cleanup (called by _Unwind_DeleteException)
 *
 * No payload beyond the header — the panic message is stored in
 * thread-local storage by ori_panic/ori_panic_cstr before raising.
 */
typedef struct {
    struct _Unwind_Exception header;
} OriException;

/*
 * Cleanup callback for caught Ori exceptions.
 *
 * Called by _Unwind_DeleteException when a catch handler consumes
 * the exception via ori_catch_cleanup → _Unwind_DeleteException.
 * Frees the malloc'd OriException object.
 */
static void ori_exception_cleanup(
    _Unwind_Reason_Code reason,
    struct _Unwind_Exception *exc)
{
    (void)reason;
    free(exc);
}

/*
 * Raise an Ori exception via the Itanium unwind ABI.
 *
 * Called by ori_panic/ori_panic_cstr after:
 * 1. Storing the panic message in thread-local storage
 * 2. Calling the user's @panic handler (if registered)
 * 3. Attempting JIT longjmp recovery (if in JIT mode)
 *
 * Allocates an OriException on the heap, sets up the Itanium
 * _Unwind_Exception header, and calls _Unwind_RaiseException.
 *
 * If _Unwind_RaiseException returns (no handler found, or
 * internal error), prints a fatal message and aborts.
 */
__attribute__((noreturn))
void ori_raise_exception(void) {
    OriException *exc = (OriException *)malloc(sizeof(OriException));
    if (!exc) {
        fprintf(stderr, "ori: fatal — out of memory during panic\n");
        abort();
    }
    memset(exc, 0, sizeof(*exc));
    exc->header.exception_class = ORI_EXCEPTION_CLASS;
    exc->header.exception_cleanup = ori_exception_cleanup;

    _Unwind_Reason_Code result = _Unwind_RaiseException(&exc->header);

    /* _Unwind_RaiseException should not return on success.
     * If it does, no handler was found or an internal error occurred. */
    free(exc);
    fprintf(stderr,
        "ori: fatal — _Unwind_RaiseException returned (code %d)\n",
        (int)result);
    abort();
}

#endif /* _MSC_VER */
