//! COW (Copy-on-Write) sequencing verification.
//!
//! Three rules:
//!
//! 1. **No reuse after COW**: After `ori_list_*_cow(data_ptr, ...)`, any
//!    subsequent use of `data_ptr` (except via `ori_rc_dec`) is a bug — the
//!    COW function may have reallocated, invalidating the old pointer.
//!
//! 2. **COW output extraction**: Structurally impossible to violate (COW
//!    functions return `void`), so no check needed.
//!
//! 3. **No dec before COW**: If `ori_rc_dec(ptr)` fires before
//!    `ori_list_*_cow(ptr, ...)`, the COW function receives a potentially
//!    freed pointer.

use rustc_hash::FxHashSet;

use inkwell::module::Module;
use inkwell::values::InstructionOpcode;

use super::abi_check::callee_name;
use super::rc_balance::operand_name;
use super::report::{AuditFinding, AuditReport, FindingKind, Severity};
use super::AuditOptions;

/// Returns true if the function name matches a COW runtime function.
fn is_cow_function(name: &str) -> bool {
    name.starts_with("ori_") && name.ends_with("_cow")
}

/// Run COW sequencing checks on all functions in a module.
pub fn check_module(module: &Module<'_>, options: &AuditOptions, report: &mut AuditReport) {
    let mut func = module.get_first_function();
    while let Some(f) = func {
        if f.get_first_basic_block().is_some() {
            let fn_name = f.get_name().to_string_lossy();
            if super::rc_balance::should_audit_fn(&fn_name, options) {
                check_function(f, report);
            }
        }
        func = f.get_next_function();
    }
}

/// Check COW sequencing within a single function.
fn check_function(func: inkwell::values::FunctionValue<'_>, report: &mut AuditReport) {
    let fn_name = func.get_name().to_string_lossy().into_owned();

    // Track pointers that have been consumed by COW (Rule 1)
    let mut cow_consumed: FxHashSet<String> = FxHashSet::default();
    // Track which COW function consumed each pointer (for diagnostics)
    let mut cow_source: rustc_hash::FxHashMap<String, String> = rustc_hash::FxHashMap::default();
    // Track pointers that have been decremented (Rule 3)
    let mut decremented: FxHashSet<String> = FxHashSet::default();

    let mut block = func.get_first_basic_block();
    while let Some(bb) = block {
        let mut inst = bb.get_first_instruction();
        while let Some(i) = inst {
            if matches!(
                i.get_opcode(),
                InstructionOpcode::Call | InstructionOpcode::Invoke
            ) {
                process_call(
                    &fn_name,
                    i,
                    &mut cow_consumed,
                    &mut cow_source,
                    &mut decremented,
                    report,
                );
            } else {
                // Rule 1: Check non-call instructions for reuse of COW-consumed pointers
                check_reuse_in_non_call(&fn_name, i, &cow_consumed, &cow_source, report);
            }
            inst = i.get_next_instruction();
        }
        block = bb.get_next_basic_block();
    }
}

/// Process a call/invoke for COW sequencing.
fn process_call(
    fn_name: &str,
    inst: inkwell::values::InstructionValue<'_>,
    cow_consumed: &mut FxHashSet<String>,
    cow_source: &mut rustc_hash::FxHashMap<String, String>,
    decremented: &mut FxHashSet<String>,
    report: &mut AuditReport,
) {
    let Some(callee) = callee_name(inst) else {
        return;
    };

    if callee == "ori_rc_dec" {
        // Track dec'd pointers for Rule 3
        if let Some(ptr_name) = operand_name(inst, 0) {
            decremented.insert(ptr_name);
        }
    } else if is_cow_function(&callee) {
        // COW data_ptr is always argument index 0
        if let Some(ptr_name) = operand_name(inst, 0) {
            // Rule 3: Check if pointer was dec'd before COW
            if decremented.contains(&ptr_name) {
                report.add(AuditFinding {
                    function_name: fn_name.to_owned(),
                    severity: Severity::Error,
                    kind: FindingKind::CowInputDecBeforeCall {
                        ptr_name: ptr_name.clone(),
                        cow_fn: callee.clone(),
                    },
                    description: format!(
                        "`{ptr_name}` was decremented before COW call `{callee}` — \
                         COW may receive a freed pointer"
                    ),
                });
            }

            // Mark as consumed for Rule 1
            cow_consumed.insert(ptr_name.clone());
            cow_source.insert(ptr_name, callee);
        }
    } else {
        // Rule 1: Check call arguments for reuse of COW-consumed pointers
        // (except ori_rc_dec, which is the expected cleanup path)
        let n = inst.get_num_operands();
        // Last operand is callee, skip it
        for idx in 0..n.saturating_sub(1) {
            if let Some(ptr_name) = operand_name(inst, idx) {
                if cow_consumed.contains(&ptr_name) {
                    let cow_fn = cow_source.get(&ptr_name).cloned().unwrap_or_default();
                    report.add(AuditFinding {
                        function_name: fn_name.to_owned(),
                        severity: Severity::Error,
                        kind: FindingKind::CowInputReusedAfterCall {
                            ptr_name: ptr_name.clone(),
                            cow_fn: cow_fn.clone(),
                        },
                        description: format!(
                            "`{ptr_name}` reused after COW consumption by `{cow_fn}`"
                        ),
                    });
                }
            }
        }
    }
}

/// Rule 1: Check non-call instructions for operands that reference COW-consumed pointers.
fn check_reuse_in_non_call(
    fn_name: &str,
    inst: inkwell::values::InstructionValue<'_>,
    cow_consumed: &FxHashSet<String>,
    cow_source: &rustc_hash::FxHashMap<String, String>,
    report: &mut AuditReport,
) {
    let n = inst.get_num_operands();
    for idx in 0..n {
        if let Some(ptr_name) = operand_name(inst, idx) {
            if cow_consumed.contains(&ptr_name) {
                let cow_fn = cow_source.get(&ptr_name).cloned().unwrap_or_default();
                report.add(AuditFinding {
                    function_name: fn_name.to_owned(),
                    severity: Severity::Error,
                    kind: FindingKind::CowInputReusedAfterCall {
                        ptr_name: ptr_name.clone(),
                        cow_fn,
                    },
                    description: format!(
                        "`{ptr_name}` reused in non-call instruction after COW consumption"
                    ),
                });
            }
        }
    }
}
