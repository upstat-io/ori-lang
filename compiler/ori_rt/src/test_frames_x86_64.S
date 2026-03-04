# x86-64 System V ABI — test frames for ori_eh_personality
#
# Two assembly functions with hand-written LSDA entries that reference
# ori_eh_personality. Used by test_forced_unwind.c to verify:
# - catch-all landing pads are NOT entered during forced unwind
# - cleanup landing pads ARE entered during forced unwind
#
# Each function takes a void (*trigger)(void) in %rdi and calls it.
# The LSDA maps that call range to a landing pad.
#
# PIC notes: Uses 0x9b encoding for personality (DW_EH_PE_indirect |
# DW_EH_PE_pcrel | DW_EH_PE_sdata4) and 0x1b for LSDA (DW_EH_PE_pcrel |
# DW_EH_PE_sdata4) for compatibility with PIE executables. The personality
# is referenced indirectly through DW.ref.ori_eh_personality (a COMDAT
# section containing the function's absolute address).

    .text

# ---------------------------------------------------------------
# frame_with_catch_all(trigger: fn ptr in %rdi)
#
# LSDA has catch-all entry (ttype_index > 0, NULL type table entry).
# Forced unwind: personality must NOT install this handler.
# The landing pad does NOT call _Unwind_Resume, so installing it
# would swallow the forced unwind and hang the thread.
# ---------------------------------------------------------------
    .globl frame_with_catch_all
    .type frame_with_catch_all, @function
frame_with_catch_all:
    .cfi_startproc
    .cfi_personality 0x9b, DW.ref.ori_eh_personality
    .cfi_lsda 0x1b, .LLSDA_catch
    pushq   %rbp
    .cfi_def_cfa_offset 16
    .cfi_offset %rbp, -16
    movq    %rsp, %rbp
    .cfi_def_cfa_register %rbp
.Lcatch_call_start:
    callq   *%rdi                       # call trigger(void)
.Lcatch_call_end:
    xorl    %eax, %eax                  # return 0 (no exception)
    popq    %rbp
    .cfi_def_cfa %rsp, 8
    ret
.Lcatch_landing_pad:                     # landing pad — should NOT be reached
    .cfi_def_cfa %rbp, 16               # Restore prologue CFI: frame is still live
    .cfi_offset %rbp, -16
    movl    $1, catch_handler_entered(%rip)
    xorl    %eax, %eax
    popq    %rbp
    ret
    .cfi_endproc
    .size frame_with_catch_all, .-frame_with_catch_all

# LSDA for catch-all
#
# Uses the same encoding as LLVM-generated catch-all landing pads:
# - @TType encoding = absptr (0x00), with a type table containing a NULL entry
# - Action table: ttype_index = 1, pointing to the NULL type table entry
#
# In the Itanium ABI, ttype_index > 0 means "catch handler" and
# ttype_index == 0 means "cleanup only". A NULL type table entry = catch-all.
    .section .gcc_except_table,"a",@progbits
    .p2align 2
.LLSDA_catch:
    .byte   0xFF                        # @LPStart encoding: omit
    .byte   0x00                        # @TType encoding: absptr (native width)
    .uleb128 .Lcatch_ttype_base - .Lcatch_ttype_ref  # @TType base offset
.Lcatch_ttype_ref:
    .byte   0x01                        # call-site encoding: uleb128
    .uleb128 .Lcatch_cs_end - .Lcatch_cs_start
.Lcatch_cs_start:
    .uleb128 .Lcatch_call_start - frame_with_catch_all
    .uleb128 .Lcatch_call_end - .Lcatch_call_start
    .uleb128 .Lcatch_landing_pad - frame_with_catch_all
    .uleb128 1                          # action: index 1
.Lcatch_cs_end:
    # Action table
    .sleb128 1                          # ttype_index = 1 → catch handler (type table[1])
    .sleb128 0                          # next action = 0 → end
    # Type table (grows backwards from ttype_base)
    .p2align 3                          # align to 8 bytes for absptr
    .quad   0                           # type table entry 1: NULL → catch-all
.Lcatch_ttype_base:
    .text

# ---------------------------------------------------------------
# frame_with_cleanup(trigger: fn ptr in %rdi)
#
# LSDA has cleanup-only entry (action == 0).
# Forced unwind: personality MUST install this handler.
# The landing pad calls _Unwind_Resume to continue unwinding.
# ---------------------------------------------------------------
    .globl frame_with_cleanup
    .type frame_with_cleanup, @function
frame_with_cleanup:
    .cfi_startproc
    .cfi_personality 0x9b, DW.ref.ori_eh_personality
    .cfi_lsda 0x1b, .LLSDA_cleanup
    pushq   %rbp
    .cfi_def_cfa_offset 16
    .cfi_offset %rbp, -16
    movq    %rsp, %rbp
    .cfi_def_cfa_register %rbp
.Lclean_call_start:
    callq   *%rdi
.Lclean_call_end:
    xorl    %eax, %eax
    popq    %rbp
    .cfi_def_cfa %rsp, 8
    ret
.Lclean_landing_pad:                     # cleanup pad — SHOULD be reached
    .cfi_def_cfa %rbp, 16               # Restore prologue CFI: frame is still live
    .cfi_offset %rbp, -16
    movl    $1, cleanup_handler_entered(%rip)
    movq    %rax, %rdi                  # exception object (data reg 0 = %rax on x86-64)
    callq   _Unwind_Resume@PLT
    .cfi_endproc
    .size frame_with_cleanup, .-frame_with_cleanup

    .section .gcc_except_table,"a",@progbits
    .p2align 2
.LLSDA_cleanup:
    .byte   0xFF
    .byte   0xFF
    .byte   0x01
    .uleb128 .Lclean_cs_end - .Lclean_cs_start
.Lclean_cs_start:
    .uleb128 .Lclean_call_start - frame_with_cleanup
    .uleb128 .Lclean_call_end - .Lclean_call_start
    .uleb128 .Lclean_landing_pad - frame_with_cleanup
    .uleb128 0                          # action: 0 → cleanup only
.Lclean_cs_end:
    .text

# Shared state — written by landing pads, read by C harness
    .bss
    .globl catch_handler_entered
catch_handler_entered: .long 0
    .globl cleanup_handler_entered
cleanup_handler_entered: .long 0

# Indirect personality reference for PIC (required by 0x9b encoding).
# COMDAT section: linker deduplicates if multiple TUs reference it.
    .hidden DW.ref.ori_eh_personality
    .weak DW.ref.ori_eh_personality
    .section .data.DW.ref.ori_eh_personality,"awG",@progbits,DW.ref.ori_eh_personality,comdat
    .p2align 3
    .type DW.ref.ori_eh_personality, @object
    .size DW.ref.ori_eh_personality, 8
DW.ref.ori_eh_personality:
    .quad ori_eh_personality
