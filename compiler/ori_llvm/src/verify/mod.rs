//! In-pipeline LLVM IR verification pass.
//!
//! Walks the in-memory LLVM IR (via inkwell) to detect RC lifecycle bugs,
//! COW sequencing violations, and ABI mismatches — all without parsing
//! textual IR.
//!
//! Gated behind `ORI_AUDIT_CODEGEN=1`. Zero cost when disabled.
//!
//! # Checks
//!
//! - **RC balance** (`rc_balance`): alloc→inc→dec→free lifecycle tracking
//! - **COW rules** (`cow_rules`): COW input sequencing (no reuse, no dec-before)
//! - **ABI check** (`abi_check`): arg counts, large aggregate loads, nounwind+invoke
//!
//! # Options
//!
//! - `ORI_AUDIT_STRICT=1`: Pessimistic mode — treats COW as always-freeing,
//!   elevates warnings to errors, tracks function pointer params as RC-managed.
//! - `ORI_AUDIT_FUNCTION=<name>`: Only audit functions whose LLVM name contains
//!   the given substring.

mod abi_check;
mod cow_rules;
mod rc_balance;
pub(crate) mod report;

pub use report::AuditReport;

use inkwell::module::Module;

/// Environment variable name for enabling the codegen audit.
///
/// Canonical source of truth — `oric::debug_flags` re-uses this value.
pub const ENV_AUDIT_CODEGEN: &str = "ORI_AUDIT_CODEGEN";

/// Environment variable name for strict (pessimistic) audit mode.
pub const ENV_AUDIT_STRICT: &str = "ORI_AUDIT_STRICT";

/// Environment variable name for filtering audit to a single function.
pub const ENV_AUDIT_FUNCTION: &str = "ORI_AUDIT_FUNCTION";

/// Runtime options for the codegen audit pass.
#[derive(Debug, Clone, Default)]
pub struct AuditOptions {
    /// Pessimistic mode: COW treated as always-freeing, warnings elevated.
    pub strict: bool,
    /// If set, only audit functions whose LLVM name contains this substring.
    pub function_filter: Option<String>,
}

impl AuditOptions {
    /// Read audit options from environment variables.
    fn from_env() -> Self {
        Self {
            strict: std::env::var(ENV_AUDIT_STRICT).is_ok_and(|v| v != "0"),
            function_filter: std::env::var(ENV_AUDIT_FUNCTION)
                .ok()
                .filter(|v| !v.is_empty()),
        }
    }
}

/// Check whether the codegen audit was requested via environment variable.
///
/// Cannot use `oric::dbg_do!` here because `ori_llvm` doesn't depend on `oric`.
/// Same pattern as `llvm_dump_requested()` in `evaluator/mod.rs`.
pub fn audit_requested() -> bool {
    std::env::var(ENV_AUDIT_CODEGEN).is_ok_and(|v| v != "0")
}

/// Returns true if the function name matches a COW runtime function.
///
/// COW functions follow the pattern `ori_list_*_cow`, `ori_set_*_cow`, etc.
/// Shared between `rc_balance` and `cow_rules` modules.
pub(super) fn is_cow_function(name: &str) -> bool {
    name.starts_with("ori_") && name.ends_with("_cow")
}

/// Extract the name of the callee from a call/invoke instruction.
///
/// LLVM convention: the last operand of a call instruction is the callee.
/// Returns `None` for indirect calls (function pointers with no name) or
/// instructions with zero operands.
///
/// Shared between `rc_balance`, `cow_rules`, and `abi_check`.
pub(super) fn callee_name(inst: inkwell::values::InstructionValue<'_>) -> Option<String> {
    let n = inst.get_num_operands();
    if n == 0 {
        return None;
    }
    let op = inst.get_operand(n - 1)?;
    match op {
        inkwell::values::Operand::Value(v) => {
            if !v.is_pointer_value() {
                return None;
            }
            let name = v
                .into_pointer_value()
                .get_name()
                .to_string_lossy()
                .into_owned();
            if name.is_empty() {
                None
            } else {
                Some(name)
            }
        }
        inkwell::values::Operand::Block(_) => None,
    }
}

/// Run all audit checks on an LLVM module and return findings.
///
/// Callers should gate this behind [`audit_requested()`] for zero overhead
/// in normal compilation. Reads `ORI_AUDIT_STRICT` and `ORI_AUDIT_FUNCTION`
/// from the environment.
pub fn audit_module(module: &Module<'_>) -> AuditReport {
    audit_module_with_options(module, &AuditOptions::from_env())
}

/// Run all audit checks with explicit options (used by tests).
pub fn audit_module_with_options(module: &Module<'_>, options: &AuditOptions) -> AuditReport {
    let mut report = AuditReport::new();
    rc_balance::check_module(module, options, &mut report);
    cow_rules::check_module(module, options, &mut report);
    abi_check::check_module(module, options, &mut report);
    report
}

#[cfg(test)]
mod tests;
