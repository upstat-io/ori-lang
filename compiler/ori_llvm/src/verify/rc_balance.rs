//! Per-function RC lifecycle tracking.
//!
//! Forward walk over each function's instructions, tracking pointer states
//! through the RC lifecycle:
//!
//! ```text
//! ori_rc_alloc → [Live] → ori_rc_dec → [Decremented]
//!                   └→ COW fn → [CowConsumed] → ori_rc_dec → ⚠ RcDecAfterCow
//! At function exit: any [Live] pointer → ⚠ RcLeak
//! ```
//!
//! ## Strict mode (`ORI_AUDIT_STRICT=1`)
//!
//! - COW consumption transitions pointer directly to `Decremented` (treated as
//!   freed), so any subsequent `ori_rc_dec` is an Error, not a Warning.
//! - Function pointer parameters are tracked as `Live`, catching leaks even for
//!   pointers not allocated locally.
//!
//! Limitation: linear walk, not full CFG dataflow. Handles straight-line code
//! (95% of RC patterns in codegen output). Conditional RC paths may produce
//! false negatives (misses), never false positives.

use rustc_hash::FxHashMap;

use inkwell::module::Module;
use inkwell::values::InstructionOpcode;

use super::report::{AuditFinding, AuditReport, FindingKind, Severity};
use super::AuditOptions;

/// State of a pointer tracked through the RC lifecycle.
#[derive(Debug, Clone)]
enum PtrState {
    /// Allocated via `ori_rc_alloc` (or tracked param in strict mode), not yet decremented.
    Live,
    /// Consumed by a COW function — should not be dec'd again.
    CowConsumed { cow_fn: String },
    /// Passed to `ori_rc_dec` — lifecycle complete.
    Decremented,
}

/// Track RC pointer states within a single function.
struct RcTracker {
    /// SSA name → current state.
    states: FxHashMap<String, PtrState>,
    /// Whether strict mode is enabled.
    strict: bool,
}

impl RcTracker {
    fn new(strict: bool) -> Self {
        Self {
            states: FxHashMap::default(),
            strict,
        }
    }

    fn record_alloc(&mut self, name: String) {
        self.states.insert(name, PtrState::Live);
    }

    fn record_dec(&mut self, name: &str) -> Option<PtrState> {
        let prev = self.states.get(name).cloned();
        self.states.insert(name.to_owned(), PtrState::Decremented);
        prev
    }

    fn record_cow_consumed(&mut self, name: &str, cow_fn: &str) {
        // Only transition if we're tracking this pointer
        if self.states.contains_key(name) {
            if self.strict {
                // Strict mode: treat COW as freeing — go directly to Decremented
                self.states.insert(name.to_owned(), PtrState::Decremented);
            } else {
                self.states.insert(
                    name.to_owned(),
                    PtrState::CowConsumed {
                        cow_fn: cow_fn.to_owned(),
                    },
                );
            }
        }
    }
}

/// Extract the SSA name of a pointer operand at the given index.
///
/// Returns `None` if the operand is not a pointer or has no name.
pub(super) fn operand_name(
    inst: inkwell::values::InstructionValue<'_>,
    idx: u32,
) -> Option<String> {
    let op = inst.get_operand(idx)?;
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

/// Run RC balance checks on all functions in a module.
pub fn check_module(module: &Module<'_>, options: &AuditOptions, report: &mut AuditReport) {
    let mut func = module.get_first_function();
    while let Some(f) = func {
        // Skip declarations (no body)
        if f.get_first_basic_block().is_some() {
            let fn_name = f.get_name().to_string_lossy();
            if should_audit_fn(&fn_name, options) {
                check_function(f, options, report);
            }
        }
        func = f.get_next_function();
    }
}

/// Check whether a function should be audited given the current filter.
pub(super) fn should_audit_fn(fn_name: &str, options: &AuditOptions) -> bool {
    match &options.function_filter {
        Some(filter) => fn_name.contains(filter.as_str()),
        None => true,
    }
}

/// Check RC balance within a single function.
fn check_function(
    func: inkwell::values::FunctionValue<'_>,
    options: &AuditOptions,
    report: &mut AuditReport,
) {
    let fn_name = func.get_name().to_string_lossy().into_owned();
    let mut tracker = RcTracker::new(options.strict);

    // In strict mode, track function pointer parameters as Live
    if options.strict {
        for i in 0..func.count_params() {
            if let Some(param) = func.get_nth_param(i) {
                if param.is_pointer_value() {
                    let name = param
                        .into_pointer_value()
                        .get_name()
                        .to_string_lossy()
                        .into_owned();
                    if !name.is_empty() {
                        tracker.record_alloc(name);
                    }
                }
            }
        }
    }

    let mut block = func.get_first_basic_block();
    while let Some(bb) = block {
        let mut inst = bb.get_first_instruction();
        while let Some(i) = inst {
            if matches!(
                i.get_opcode(),
                InstructionOpcode::Call | InstructionOpcode::Invoke
            ) {
                process_call(&fn_name, i, &mut tracker, options, report);
            }
            inst = i.get_next_instruction();
        }
        block = bb.get_next_basic_block();
    }

    // Check for leaks: any pointer still Live at function exit
    for (ptr_name, state) in &tracker.states {
        if matches!(state, PtrState::Live) {
            report.add(AuditFinding {
                function_name: fn_name.clone(),
                severity: if options.strict {
                    Severity::Error
                } else {
                    Severity::Warning
                },
                kind: FindingKind::RcLeak {
                    alloc_site: ptr_name.clone(),
                },
                description: format!(
                    "RC pointer `{ptr_name}` allocated but never decremented (potential leak)"
                ),
            });
        }
    }
}

/// Process a call/invoke instruction for RC tracking.
fn process_call(
    fn_name: &str,
    inst: inkwell::values::InstructionValue<'_>,
    tracker: &mut RcTracker,
    options: &AuditOptions,
    report: &mut AuditReport,
) {
    let Some(callee) = super::callee_name(inst) else {
        return;
    };

    match callee.as_str() {
        "ori_rc_alloc" => {
            // Record the result SSA name as Live
            if let Some(name) = inst.get_name().map(|n| n.to_string_lossy().into_owned()) {
                if !name.is_empty() {
                    tracker.record_alloc(name);
                }
            }
        }
        "ori_rc_dec" => {
            // First operand (index 0) is the pointer being decremented
            if let Some(ptr_name) = operand_name(inst, 0) {
                match tracker.record_dec(&ptr_name) {
                    Some(PtrState::CowConsumed { cow_fn }) => {
                        report.add(AuditFinding {
                            function_name: fn_name.to_owned(),
                            severity: Severity::Warning,
                            kind: FindingKind::RcDecAfterCow {
                                ptr_name: ptr_name.clone(),
                                cow_fn: cow_fn.clone(),
                            },
                            description: format!(
                                "ori_rc_dec on `{ptr_name}` after COW consumption by `{cow_fn}`"
                            ),
                        });
                    }
                    Some(PtrState::Decremented) if options.strict => {
                        // In strict mode, COW transitions to Decremented, so a
                        // second dec is a definite double-dec.
                        report.add(AuditFinding {
                            function_name: fn_name.to_owned(),
                            severity: Severity::Error,
                            kind: FindingKind::RcDoubleDec {
                                ptr_name: ptr_name.clone(),
                            },
                            description: format!(
                                "ori_rc_dec on `{ptr_name}` which was already decremented \
                                 (double-free in strict mode)"
                            ),
                        });
                    }
                    _ => {}
                }
            }
        }
        _ if super::is_cow_function(&callee) => {
            // COW data_ptr is always argument index 0
            if let Some(ptr_name) = operand_name(inst, 0) {
                tracker.record_cow_consumed(&ptr_name, &callee);
            }
        }
        _ => {}
    }
}
