//! ABI verification checks for LLVM IR.
//!
//! Three independent sub-checks:
//! - **Large aggregate loads**: `load %BigStruct, ptr` for structs >16 bytes
//!   (these trigger `FastISel` bugs in JIT — see CLAUDE.md "`FastISel` Aggregate Bug")
//! - **Runtime arg count mismatch**: calls to `ori_*` functions with wrong arity
//! - **Nounwind + invoke**: `invoke` on functions marked `nounwind` (should use `call`)

use inkwell::module::Module;
use inkwell::types::{AnyTypeEnum, BasicTypeEnum};
use inkwell::values::InstructionOpcode;

use crate::codegen::runtime_decl::runtime_functions::{Attr, Ty, RT_FUNCTIONS};

use super::report::{AuditFinding, AuditReport, FindingKind, Severity};
use super::AuditOptions;

/// Estimated size in bytes of a basic LLVM type.
///
/// Conservative: ignores alignment padding (real structs are only larger).
fn estimated_basic_type_size(ty: &BasicTypeEnum<'_>) -> u64 {
    match ty {
        BasicTypeEnum::IntType(t) => u64::from(t.get_bit_width() + 7) / 8,
        BasicTypeEnum::FloatType(_) | BasicTypeEnum::PointerType(_) => 8,
        BasicTypeEnum::StructType(st) => {
            let mut total = 0u64;
            for i in 0..st.count_fields() {
                if let Some(field) = st.get_field_type_at_index(i) {
                    total += estimated_basic_type_size(&field);
                }
            }
            total
        }
        BasicTypeEnum::ArrayType(at) => {
            let elem = at.get_element_type();
            estimated_basic_type_size(&elem) * u64::from(at.len())
        }
        BasicTypeEnum::VectorType(_) | BasicTypeEnum::ScalableVectorType(_) => 0,
    }
}

/// Estimated size for instruction result types (`AnyTypeEnum`).
fn estimated_type_size(ty: &AnyTypeEnum<'_>) -> u64 {
    match ty {
        AnyTypeEnum::IntType(t) => u64::from(t.get_bit_width() + 7) / 8,
        AnyTypeEnum::FloatType(_) | AnyTypeEnum::PointerType(_) => 8,
        AnyTypeEnum::StructType(st) => {
            let mut total = 0u64;
            for i in 0..st.count_fields() {
                if let Some(field) = st.get_field_type_at_index(i) {
                    total += estimated_basic_type_size(&field);
                }
            }
            total
        }
        AnyTypeEnum::ArrayType(at) => {
            let elem = at.get_element_type();
            estimated_basic_type_size(&elem) * u64::from(at.len())
        }
        _ => 0,
    }
}

/// Run all ABI checks on a module.
pub fn check_module(module: &Module<'_>, options: &AuditOptions, report: &mut AuditReport) {
    let mut func = module.get_first_function();
    while let Some(f) = func {
        let fn_name = f.get_name().to_string_lossy().into_owned();
        if super::rc_balance::should_audit_fn(&fn_name, options) {
            let mut block = f.get_first_basic_block();
            while let Some(bb) = block {
                let mut inst = bb.get_first_instruction();
                while let Some(i) = inst {
                    check_instruction(&fn_name, i, report);
                    inst = i.get_next_instruction();
                }
                block = bb.get_next_basic_block();
            }
        }
        func = f.get_next_function();
    }
}

/// Check a single instruction for ABI violations.
fn check_instruction(
    fn_name: &str,
    inst: inkwell::values::InstructionValue<'_>,
    report: &mut AuditReport,
) {
    match inst.get_opcode() {
        InstructionOpcode::Load => check_large_aggregate_load(fn_name, inst, report),
        InstructionOpcode::Call => check_call(fn_name, inst, report),
        InstructionOpcode::Invoke => check_invoke(fn_name, inst, report),
        _ => {}
    }
}

/// A: Large aggregate loads (>16 bytes).
fn check_large_aggregate_load(
    fn_name: &str,
    inst: inkwell::values::InstructionValue<'_>,
    report: &mut AuditReport,
) {
    let ty = inst.get_type();
    if let AnyTypeEnum::StructType(_) = &ty {
        let size = estimated_type_size(&ty);
        if size > 16 {
            report.add(AuditFinding {
                function_name: fn_name.to_owned(),
                severity: Severity::Warning,
                kind: FindingKind::LargeAggregateLoad {
                    type_size_bytes: size,
                },
                description: format!(
                    "load of {size}-byte aggregate (>16B threshold) — may trigger FastISel bug in JIT"
                ),
            });
        }
    }
}

/// B: Runtime arg count mismatch on `call` instructions.
fn check_call(
    fn_name: &str,
    inst: inkwell::values::InstructionValue<'_>,
    report: &mut AuditReport,
) {
    if let Some(callee) = super::callee_name(inst) {
        check_runtime_arg_count(fn_name, &callee, inst, report);
    }
}

/// C: `invoke` on nounwind functions + arg count check.
fn check_invoke(
    fn_name: &str,
    inst: inkwell::values::InstructionValue<'_>,
    report: &mut AuditReport,
) {
    if let Some(callee) = super::callee_name(inst) {
        // Check arg count for invoke too
        check_runtime_arg_count(fn_name, &callee, inst, report);

        // Check nounwind + invoke
        if is_nounwind_runtime_fn(&callee) {
            report.add(AuditFinding {
                function_name: fn_name.to_owned(),
                severity: Severity::Warning,
                kind: FindingKind::NounwindCalledWithInvoke {
                    fn_name: callee.clone(),
                },
                description: format!(
                    "`{callee}` is nounwind but called via invoke (should use call)"
                ),
            });
        }
    }
}

/// Check that a call/invoke to a known runtime function has the expected arg count.
fn check_runtime_arg_count(
    fn_name: &str,
    callee: &str,
    inst: inkwell::values::InstructionValue<'_>,
    report: &mut AuditReport,
) {
    if let Some(spec) = RT_FUNCTIONS.iter().find(|f| f.name == callee) {
        let mut expected = spec.params.len();
        // Sret return types add an extra pointer parameter at position 0
        if spec.ret.is_some_and(Ty::needs_sret) {
            expected += 1;
        }
        // Operand count includes the callee itself as the last operand
        let actual = inst.get_num_operands().saturating_sub(1) as usize;
        if actual != expected {
            report.add(AuditFinding {
                function_name: fn_name.to_owned(),
                severity: Severity::Error,
                kind: FindingKind::RuntimeArgCountMismatch {
                    fn_name: callee.to_owned(),
                    expected,
                    actual,
                },
                description: format!(
                    "`{callee}` expects {expected} arg(s) but called with {actual}"
                ),
            });
        }
    }
}

/// Check if a runtime function is marked `nounwind`.
fn is_nounwind_runtime_fn(name: &str) -> bool {
    RT_FUNCTIONS
        .iter()
        .find(|f| f.name == name)
        .is_some_and(|f| f.attrs.iter().any(|a| matches!(a, Attr::Nounwind)))
}
