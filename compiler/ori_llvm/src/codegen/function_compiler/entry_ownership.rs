//! Entry-point ownership-seam report: the semantic AIMS param facts and the
//! physical wrapper cleanup decision, rendered side by side.
//!
//! The C `main()` wrapper decides argv cleanup from `ParamPassing` alone. This
//! module is the single place that projects the governing semantic facts
//! (`ParamContract`, realized `ArcParam` ownership) next to that physical
//! decision and the per-cleanup-site verdict, so a reader sees whether the
//! physical decision followed from the semantic fact or from the ABI bit.
//!
//! Read-only: every fact is READ from its owner. The RL-2 boundary verdict is
//! `ParamContract::callee_owner_demand()`, the same oracle
//! `arc_emitter::emit_function_setup::compute_borrowed_rooted_vars` consumes at
//! this grain; no ownership fact is re-derived here.
//!
//! Env: `ORI_DUMP_ENTRY_OWNERSHIP` — dump the entry-point ownership seam, debug-only.

use std::fmt::Write as _;

use ori_arc::aims::contract::{CalleeOwnerDemand, ParamContract};
use ori_arc::ownership::Ownership;

use crate::codegen::abi::ParamPassing;
use crate::codegen::eh_model::EhModel;

/// Canonical env-var name for the entry-point ownership-seam dump.
///
/// Mirrored by `oric::debug_flags::ORI_DUMP_ENTRY_OWNERSHIP`; the two are
/// const-asserted equal at compile time in that module.
pub const ENV_DUMP_ENTRY_OWNERSHIP: &str = "ORI_DUMP_ENTRY_OWNERSHIP";

/// Whether the dump is enabled for this process.
#[must_use]
pub(super) fn dump_enabled() -> bool {
    std::env::var(ENV_DUMP_ENTRY_OWNERSHIP).is_ok_and(|value| value != "0")
}

/// One physical cleanup site in the C `main()` wrapper.
///
/// The five members are the complete set of `ori_args_cleanup` call sites
/// across both exception-handling legs. Adding a leg adds a member here; a
/// leg never renders its own row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CleanupSite {
    /// `entry_point::emit_main_call_with_invoke` normal-return path.
    ItaniumInvokeNormal,
    /// `entry_point::emit_main_call_with_invoke` landingpad catch-all path.
    ItaniumCatch,
    /// `entry_point::emit_main_call_direct` normal-return path.
    ItaniumDirectNormal,
    /// `seh_main_thunk::emit_main_call_with_seh_try` success path.
    SehSuccess,
    /// `seh_main_thunk::emit_main_call_with_seh_try` caught path.
    SehCaught,
}

impl CleanupSite {
    /// All five sites, in emission order.
    pub(super) const ALL: [Self; 5] = [
        Self::ItaniumInvokeNormal,
        Self::ItaniumCatch,
        Self::ItaniumDirectNormal,
        Self::SehSuccess,
        Self::SehCaught,
    ];

    /// The site's stable identifier.
    #[must_use]
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::ItaniumInvokeNormal => "itanium_invoke_normal",
            Self::ItaniumCatch => "itanium_catch",
            Self::ItaniumDirectNormal => "itanium_direct_normal",
            Self::SehSuccess => "seh_success",
            Self::SehCaught => "seh_caught",
        }
    }

    /// Whether the site's cleanup call is guarded by `wrapper_owns_on_normal`.
    ///
    /// The unwind-path sites clean up unconditionally: the callee's release
    /// does not execute when it unwinds.
    #[must_use]
    pub(super) fn is_guarded(self) -> bool {
        match self {
            Self::ItaniumInvokeNormal | Self::ItaniumDirectNormal | Self::SehSuccess => true,
            Self::ItaniumCatch | Self::SehCaught => false,
        }
    }

    /// The guard's rendered name.
    #[must_use]
    pub(super) fn guard(self) -> &'static str {
        if self.is_guarded() {
            "wrapper_owns_on_normal"
        } else {
            "unconditional"
        }
    }

    /// Whether this site is emitted for the given wrapper shape.
    #[must_use]
    pub(super) fn is_active(self, eh_model: EhModel, can_unwind: bool) -> bool {
        match self {
            Self::ItaniumInvokeNormal | Self::ItaniumCatch => {
                can_unwind && matches!(eh_model, EhModel::Itanium)
            }
            Self::SehSuccess | Self::SehCaught => can_unwind && matches!(eh_model, EhModel::Seh),
            Self::ItaniumDirectNormal => !can_unwind,
        }
    }

    /// Whether this site emits an `ori_args_cleanup` call.
    #[must_use]
    pub(super) fn emits_cleanup(self, wrapper_owns_on_normal: bool) -> bool {
        !self.is_guarded() || wrapper_owns_on_normal
    }
}

/// Whether the physical cleanup decision agrees with the semantic demand.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SeamVerdict {
    /// The wrapper's normal-path ownership matches the callee's owner demand.
    Consistent,
    /// The callee consumes an owner credit the wrapper also releases, or the
    /// callee borrows a credit the wrapper never releases.
    Divergent,
}

impl SeamVerdict {
    #[must_use]
    fn name(self) -> &'static str {
        match self {
            Self::Consistent => "CONSISTENT",
            Self::Divergent => "DIVERGENT",
        }
    }
}

/// Semantic + physical facts for one entry-point parameter.
pub(super) struct EntryParamSeam {
    /// Zero-based parameter position.
    pub(super) index: usize,
    /// Source-level parameter name.
    pub(super) name: String,
    /// The AIMS interprocedural contract for this parameter, when the frozen
    /// executable artifact carries one.
    pub(super) contract: Option<ParamContract>,
    /// Realized `ArcParam` ownership from the closed executable artifact.
    pub(super) realized_ownership: Option<Ownership>,
    /// Whether the realized parameter roots a borrowed lineage.
    ///
    /// Read with the same rule `compute_borrowed_rooted_vars` applies to a
    /// parameter: a `Borrowed` realized ownership roots the borrowed set.
    pub(super) borrowed_rooted: Option<bool>,
    /// How the parameter is physically passed to `_ori_main`.
    pub(super) param_passing: ParamPassing,
    /// The wrapper's computed normal-path ownership flag.
    pub(super) wrapper_owns_on_normal: bool,
}

impl EntryParamSeam {
    /// The RL-2 boundary transfer verdict, read from the contract's oracle.
    #[must_use]
    pub(super) fn owner_demand(&self) -> Option<CalleeOwnerDemand> {
        self.contract
            .as_ref()
            .map(ParamContract::callee_owner_demand)
    }

    /// Compare the physical decision against the semantic demand.
    ///
    /// `WholeValue` means the callee consumes one owner credit for the whole
    /// value, so the wrapper must not also release it; `Borrow` means the
    /// wrapper retains the credit and must release it. Unknown demand yields
    /// no verdict.
    #[must_use]
    pub(super) fn seam_verdict(&self) -> Option<SeamVerdict> {
        let demand = self.owner_demand()?;
        let wrapper_should_own = match demand {
            CalleeOwnerDemand::Borrow => true,
            CalleeOwnerDemand::WholeValue => false,
        };
        Some(if wrapper_should_own == self.wrapper_owns_on_normal {
            SeamVerdict::Consistent
        } else {
            SeamVerdict::Divergent
        })
    }
}

/// The whole entry-point ownership seam for one compiled `@main`.
pub(super) struct EntryOwnershipReport {
    /// Mangled or source name of the Ori entry point.
    pub(super) main_name: String,
    /// Exception-handling model selected for this target.
    pub(super) eh_model: EhModel,
    /// Whether `_ori_main` can unwind.
    pub(super) can_unwind: bool,
    /// Per-parameter seams, in parameter order.
    pub(super) params: Vec<EntryParamSeam>,
}

impl EntryOwnershipReport {
    /// Render the report as the stderr dump.
    #[must_use]
    pub(super) fn render(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(
            out,
            "=== entry-point ownership seam: @{} (eh_model={:?}, can_unwind={}) ===",
            self.main_name, self.eh_model, self.can_unwind
        );
        if self.params.is_empty() {
            out.push_str("  (no entry-point parameters)\n");
            return out;
        }
        for param in &self.params {
            self.render_param(&mut out, param);
        }
        out
    }

    fn render_param(&self, out: &mut String, param: &EntryParamSeam) {
        let _ = writeln!(out, "  param#{} name={}", param.index, param.name);
        Self::render_semantic(out, param);
        Self::render_physical(out, param);
        self.render_cleanup_verdicts(out, param);
    }

    fn render_semantic(out: &mut String, param: &EntryParamSeam) {
        out.push_str("    semantic (AIMS):\n");
        match &param.contract {
            Some(contract) => {
                let _ = writeln!(
                    out,
                    "      access                    = {:?}",
                    contract.access
                );
                let _ = writeln!(
                    out,
                    "      consumption               = {:?}",
                    contract.consumption
                );
                let _ = writeln!(
                    out,
                    "      cardinality               = {:?}",
                    contract.cardinality
                );
                let _ = writeln!(
                    out,
                    "      iter_consumes             = {}",
                    contract.iter_consumes
                );
                let _ = writeln!(
                    out,
                    "      transfers_through_return  = {}",
                    contract.transfers_through_return
                );
                let _ = writeln!(
                    out,
                    "      borrowed_read_only        = {}",
                    contract.borrowed_read_only
                );
                let _ = writeln!(
                    out,
                    "      borrowed_cow_consumed     = {}",
                    contract.borrowed_cow_consumed
                );
                let _ = writeln!(
                    out,
                    "      may_escape / may_share    = {} / {}",
                    contract.may_escape, contract.may_share
                );
                let _ = writeln!(
                    out,
                    "      uniqueness                = {:?}",
                    contract.uniqueness
                );
                let _ = writeln!(
                    out,
                    "      exact_transfer            = {:?}",
                    contract.exact_transfer
                );
            }
            None => out.push_str("      <no frozen AIMS contract for this entry point>\n"),
        }
        match param.owner_demand() {
            Some(demand) => {
                let _ = writeln!(
                    out,
                    "      callee_owner_demand       = {demand:?}  <- RL-2 boundary verdict: {}",
                    match demand {
                        CalleeOwnerDemand::WholeValue => "TRANSFER inward",
                        CalleeOwnerDemand::Borrow => "NON-TRANSFER",
                    }
                );
            }
            None => out.push_str("      callee_owner_demand       = <unknown>\n"),
        }
        let _ = writeln!(
            out,
            "      realized ArcParam.ownership = {}",
            param
                .realized_ownership
                .map_or_else(|| "<unknown>".to_owned(), |own| format!("{own:?}"))
        );
        let _ = writeln!(
            out,
            "      borrowed_rooted           = {}",
            param
                .borrowed_rooted
                .map_or_else(|| "<unknown>".to_owned(), |flag| flag.to_string())
        );
    }

    fn render_physical(out: &mut String, param: &EntryParamSeam) {
        out.push_str("    physical (ABI):\n");
        let _ = writeln!(
            out,
            "      param_passing             = {:?}",
            param.param_passing
        );
        let _ = writeln!(
            out,
            "      wrapper_owns_on_normal    = {}  <- derived from param_passing ONLY",
            param.wrapper_owns_on_normal
        );
    }

    fn render_cleanup_verdicts(&self, out: &mut String, param: &EntryParamSeam) {
        out.push_str("    cleanup-site verdicts:\n");
        for site in CleanupSite::ALL {
            let _ = writeln!(
                out,
                "      {:<24} {:<5} (guard: {:<22}) [{}]",
                site.name(),
                if site.emits_cleanup(param.wrapper_owns_on_normal) {
                    "EMIT"
                } else {
                    "SKIP"
                },
                site.guard(),
                if site.is_active(self.eh_model, self.can_unwind) {
                    "active"
                } else {
                    "inactive"
                }
            );
        }

        match param.seam_verdict() {
            Some(verdict) => {
                let _ = writeln!(out, "    seam: {}", verdict.name());
            }
            None => out.push_str("    seam: <unknown — no contract>\n"),
        }
    }
}

#[cfg(test)]
mod tests;
