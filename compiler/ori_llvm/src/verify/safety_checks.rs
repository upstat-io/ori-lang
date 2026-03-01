//! Safety check density analysis for LLVM IR.
//!
//! Scans for calls to panic/assert runtime functions and conditional branches
//! targeting panic blocks. Reports per-function **check density** (checks per
//! 100 instructions). High density in hot functions indicates LLVM's optimizer
//! failed to prove safety — the checks are correct but potentially suboptimal.
//!
//! All findings are Note-level: safety checks are correct, just visible.
//!
//! **Two-pass per function:**
//! 1. Identify "panic blocks" — blocks containing calls to safety functions
//! 2. Count safety calls + conditional branches to panic blocks + total
//!    instructions → compute density

use inkwell::module::Module;
use inkwell::values::InstructionOpcode;
use rustc_hash::FxHashSet;

use crate::codegen::runtime_decl::runtime_functions::{Attr, RT_FUNCTIONS};

use super::report::{AuditFinding, AuditReport, FindingKind, Severity};
use super::AuditOptions;

/// Returns true if `name` is a safety-related runtime function.
///
/// Matches two categories:
/// - Functions in `RT_FUNCTIONS` with `Attr::Cold` (panic handlers)
/// - Functions whose name starts with `ori_assert` (assertion variants)
fn is_safety_function(name: &str) -> bool {
    if name.starts_with("ori_assert") {
        return true;
    }
    RT_FUNCTIONS
        .iter()
        .find(|f| f.name == name)
        .is_some_and(|f| f.attrs.iter().any(|a| matches!(a, Attr::Cold)))
}

/// Run safety check density analysis on all functions in a module.
pub fn check_module(module: &Module<'_>, options: &AuditOptions, report: &mut AuditReport) {
    let mut func = module.get_first_function();
    while let Some(f) = func {
        let fn_name = f.get_name().to_string_lossy().into_owned();
        if super::rc_balance::should_audit_fn(&fn_name, options) {
            check_function(f, &fn_name, report);
        }
        func = f.get_next_function();
    }
}

/// Analyze a single function for safety check density.
///
/// Pass 1: identify panic blocks (blocks containing safety function calls).
/// Pass 2: count safety calls, conditional branches to panic blocks, and
/// total instructions.
fn check_function(
    func: inkwell::values::FunctionValue<'_>,
    fn_name: &str,
    report: &mut AuditReport,
) {
    // Pass 1: find panic blocks
    let panic_blocks = find_panic_blocks(func);
    if panic_blocks.is_empty() && !has_safety_calls(func) {
        return; // fast exit — no safety infrastructure in this function
    }

    // Pass 2: count checks and instructions
    let mut check_count = 0usize;
    let mut instruction_count = 0usize;

    let mut block = func.get_first_basic_block();
    while let Some(bb) = block {
        let mut inst = bb.get_first_instruction();
        while let Some(i) = inst {
            instruction_count += 1;

            match i.get_opcode() {
                InstructionOpcode::Call | InstructionOpcode::Invoke => {
                    if let Some(callee) = super::callee_name(i) {
                        if is_safety_function(&callee) {
                            report.add(AuditFinding {
                                function_name: fn_name.to_owned(),
                                severity: Severity::Note,
                                kind: FindingKind::SafetyCheckCall {
                                    callee: callee.clone(),
                                },
                                description: format!("safety check: call to `{callee}`"),
                            });
                            check_count += 1;
                        }
                    }
                }
                InstructionOpcode::Br => {
                    // Conditional branch: 3 operands (cond, true_bb, false_bb)
                    // Unconditional branch: 1 operand (target_bb)
                    if i.get_num_operands() == 3 {
                        for idx in 1..3 {
                            if let Some(target_name) = branch_target_name(i, idx) {
                                if panic_blocks.contains(target_name.as_str()) {
                                    report.add(AuditFinding {
                                        function_name: fn_name.to_owned(),
                                        severity: Severity::Note,
                                        kind: FindingKind::SafetyCheckBranch {
                                            panic_block: target_name.clone(),
                                        },
                                        description: format!(
                                            "safety check: conditional branch to panic block `{target_name}`"
                                        ),
                                    });
                                    check_count += 1;
                                    break; // count once per branch instruction
                                }
                            }
                        }
                    }
                }
                _ => {}
            }

            inst = i.get_next_instruction();
        }
        block = bb.get_next_basic_block();
    }

    if check_count > 0 {
        #[expect(
            clippy::cast_precision_loss,
            reason = "per-function counts are far below f64 mantissa precision"
        )]
        let density = if instruction_count > 0 {
            (check_count as f64 / instruction_count as f64) * 100.0
        } else {
            0.0
        };
        report.add(AuditFinding {
            function_name: fn_name.to_owned(),
            severity: Severity::Note,
            kind: FindingKind::SafetyCheckSummary {
                check_count,
                instruction_count,
            },
            description: format!(
                "safety check density: {check_count} check(s) in {instruction_count} instruction(s) \
                 ({density:.1}/100)"
            ),
        });
    }
}

/// Pass 1: collect names of basic blocks that contain calls to safety functions.
fn find_panic_blocks(func: inkwell::values::FunctionValue<'_>) -> FxHashSet<String> {
    let mut panic_blocks = FxHashSet::default();
    let mut block = func.get_first_basic_block();
    while let Some(bb) = block {
        let bb_name = bb.get_name().to_string_lossy().into_owned();
        let mut inst = bb.get_first_instruction();
        while let Some(i) = inst {
            if matches!(
                i.get_opcode(),
                InstructionOpcode::Call | InstructionOpcode::Invoke
            ) {
                if let Some(callee) = super::callee_name(i) {
                    if is_safety_function(&callee) {
                        panic_blocks.insert(bb_name);
                        break; // one hit is enough to mark the block
                    }
                }
            }
            inst = i.get_next_instruction();
        }
        block = bb.get_next_basic_block();
    }
    panic_blocks
}

/// Quick check: does the function contain any safety calls at all?
fn has_safety_calls(func: inkwell::values::FunctionValue<'_>) -> bool {
    let mut block = func.get_first_basic_block();
    while let Some(bb) = block {
        let mut inst = bb.get_first_instruction();
        while let Some(i) = inst {
            if matches!(
                i.get_opcode(),
                InstructionOpcode::Call | InstructionOpcode::Invoke
            ) {
                if let Some(callee) = super::callee_name(i) {
                    if is_safety_function(&callee) {
                        return true;
                    }
                }
            }
            inst = i.get_next_instruction();
        }
        block = bb.get_next_basic_block();
    }
    false
}

/// Extract the name of a branch target block from a branch instruction operand.
fn branch_target_name(
    inst: inkwell::values::InstructionValue<'_>,
    operand_idx: u32,
) -> Option<String> {
    let operand = inst.get_operand(operand_idx)?;
    match operand {
        inkwell::values::Operand::Block(bb) => {
            let name = bb.get_name().to_string_lossy().into_owned();
            if name.is_empty() {
                None
            } else {
                Some(name)
            }
        }
        inkwell::values::Operand::Value(_) => None,
    }
}
