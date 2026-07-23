//! Entry-point ownership-seam report: the semantic AIMS param facts and the
//! physical wrapper cleanup decision, rendered side by side.
//!
//! The C `main()` wrapper decides argv cleanup from the callee's owner demand.
//! This module is the single place that projects the governing semantic facts
//! (`ParamContract`, realized `ArcParam` ownership) next to that wrapper
//! decision and the per-cleanup-site verdict, so a reader sees whether the
//! wrapper decision agrees with the semantic fact on every exit. A regression
//! that derived the decision from the ABI passing mode would read DIVERGENT.
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

    /// Whether the site's cleanup call is guarded by `wrapper_owns`.
    ///
    /// Every site — the three normal-return sites AND the two caught sites — is
    /// guarded by the one `wrapper_owns` decision: the wrapper releases the
    /// buffer on an exit iff it owns the cleanup obligation there.
    #[must_use]
    pub(super) fn is_guarded(self) -> bool {
        match self {
            Self::ItaniumInvokeNormal
            | Self::ItaniumDirectNormal
            | Self::SehSuccess
            | Self::ItaniumCatch
            | Self::SehCaught => true,
        }
    }

    /// The guard's rendered name.
    #[must_use]
    pub(super) fn guard(self) -> &'static str {
        if self.is_guarded() {
            "wrapper_owns"
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
    pub(super) fn emits_cleanup(self, wrapper_owns: bool) -> bool {
        !self.is_guarded() || wrapper_owns
    }
}

/// Per-active-site agreement between the physical emission and the demand.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SiteCorrectness {
    /// The site emits or skips cleanup exactly as the owner demand requires.
    Ok,
    /// The site releases a credit the callee already consumed (double free).
    DoubleFree,
    /// The site never releases a credit the wrapper retained (leak).
    Leak,
}

impl SiteCorrectness {
    #[must_use]
    fn name(self) -> &'static str {
        match self {
            Self::Ok => "OK",
            Self::DoubleFree => "DOUBLE-FREE",
            Self::Leak => "LEAK",
        }
    }

    /// Whether this per-site outcome is a defect.
    #[must_use]
    fn is_defect(self) -> bool {
        !matches!(self, Self::Ok)
    }
}

/// Whether the physical cleanup decision agrees with the semantic demand across
/// EVERY active exit, not only the normal-return decision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SeamVerdict {
    /// Every active cleanup site's emission matches the callee's owner demand
    /// on every exit.
    Consistent,
    /// At least one active cleanup site disagrees with the owner demand: the
    /// wrapper releases a consumed credit (double free) or never releases a
    /// retained one (leak), on some exit.
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
    /// The wrapper's computed cleanup-ownership decision, applied on every
    /// exit (normal AND unwind), derived from the callee's owner demand.
    pub(super) wrapper_owns: bool,
}

impl EntryParamSeam {
    /// The RL-2 boundary transfer verdict, read from the contract's oracle.
    #[must_use]
    pub(super) fn owner_demand(&self) -> Option<CalleeOwnerDemand> {
        self.contract
            .as_ref()
            .map(ParamContract::callee_owner_demand)
    }

    /// Whether the wrapper must emit cleanup at all: `Borrow` retains the credit
    /// (wrapper releases it), `WholeValue` transfers it inward (wrapper must
    /// not). Unknown demand yields no obligation.
    #[must_use]
    fn wrapper_must_release(&self) -> Option<bool> {
        Some(match self.owner_demand()? {
            CalleeOwnerDemand::Borrow => true,
            CalleeOwnerDemand::WholeValue => false,
        })
    }

    /// Per-site agreement for one cleanup site under a given wrapper shape.
    ///
    /// `None` when the site is inactive for this shape or the owner demand is
    /// unknown. An ACTIVE guarded normal site and an ACTIVE unconditional caught
    /// site are judged independently, so a normal-path-only cure that leaves a
    /// caught site emitting for a consumed value is still a defect here.
    #[must_use]
    pub(super) fn site_correctness(
        &self,
        site: CleanupSite,
        eh_model: EhModel,
        can_unwind: bool,
    ) -> Option<SiteCorrectness> {
        if !site.is_active(eh_model, can_unwind) {
            return None;
        }
        let must_release = self.wrapper_must_release()?;
        let emits = site.emits_cleanup(self.wrapper_owns);
        Some(match (emits, must_release) {
            (true, false) => SiteCorrectness::DoubleFree,
            (false, true) => SiteCorrectness::Leak,
            _ => SiteCorrectness::Ok,
        })
    }

    /// The both-exits seam verdict: `Divergent` iff ANY active cleanup site is a
    /// defect under this wrapper shape, `Consistent` when every active site
    /// agrees with the demand. Unknown demand yields no verdict.
    ///
    /// Depends on `eh_model` + `can_unwind` because the active-site set differs
    /// per exception-handling shape; a verdict blind to the caught sites would
    /// pass a cure that still double-frees on the unwind exit.
    #[must_use]
    pub(super) fn seam_verdict(&self, eh_model: EhModel, can_unwind: bool) -> Option<SeamVerdict> {
        self.wrapper_must_release()?;
        let any_defect = CleanupSite::ALL.iter().any(|&site| {
            self.site_correctness(site, eh_model, can_unwind)
                .is_some_and(SiteCorrectness::is_defect)
        });
        Some(if any_defect {
            SeamVerdict::Divergent
        } else {
            SeamVerdict::Consistent
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
        out.push_str("    physical (wrapper decision):\n");
        let _ = writeln!(
            out,
            "      param_passing             = {:?}",
            param.param_passing
        );
        let _ = writeln!(
            out,
            "      wrapper_owns              = {}  <- one cleanup decision, applied on every exit",
            param.wrapper_owns
        );
    }

    fn render_cleanup_verdicts(&self, out: &mut String, param: &EntryParamSeam) {
        out.push_str("    cleanup-site verdicts:\n");
        for site in CleanupSite::ALL {
            let active = site.is_active(self.eh_model, self.can_unwind);
            let correctness = param
                .site_correctness(site, self.eh_model, self.can_unwind)
                .map_or("", SiteCorrectness::name);
            let _ = writeln!(
                out,
                "      {:<24} {:<5} (guard: {:<22}) [{:<8}] {}",
                site.name(),
                if site.emits_cleanup(param.wrapper_owns) {
                    "EMIT"
                } else {
                    "SKIP"
                },
                site.guard(),
                if active { "active" } else { "inactive" },
                correctness,
            );
        }

        match param.seam_verdict(self.eh_model, self.can_unwind) {
            Some(verdict) => {
                let _ = writeln!(out, "    seam: {}", verdict.name());
            }
            None => out.push_str("    seam: <unknown — no contract>\n"),
        }
    }
}

#[cfg(test)]
mod tests;
