use super::*;
use ori_arc::aims::contract::ExactTransferState;
use ori_arc::aims::lattice::prelude::{AccessClass, Cardinality, Consumption};

fn borrowed_read_only_contract() -> ParamContract {
    let mut contract = ParamContract::CONSERVATIVE;
    contract.access = AccessClass::Borrowed;
    contract.consumption = Consumption::Linear;
    contract.cardinality = Cardinality::Once;
    contract.may_escape = false;
    contract.may_share = false;
    contract.borrowed_read_only = true;
    contract.iter_consumes = false;
    contract.exact_transfer = ExactTransferState::Unproven;
    contract
}

fn iter_consuming_contract() -> ParamContract {
    let mut contract = borrowed_read_only_contract();
    contract.borrowed_read_only = false;
    contract.iter_consumes = true;
    contract
}

fn seam(contract: Option<ParamContract>, wrapper_owns: bool) -> EntryParamSeam {
    EntryParamSeam {
        index: 0,
        name: "args".to_owned(),
        contract,
        realized_ownership: Some(Ownership::Borrowed),
        borrowed_rooted: Some(true),
        param_passing: ParamPassing::Reference,
        wrapper_owns,
    }
}

fn report(
    params: Vec<EntryParamSeam>,
    eh_model: EhModel,
    can_unwind: bool,
) -> EntryOwnershipReport {
    EntryOwnershipReport {
        main_name: "main".to_owned(),
        eh_model,
        can_unwind,
        params,
    }
}

// This pins the diagnostic's OWN cleanup-site enum against expected values; it
// does NOT read the real emitters in `entry_point.rs` / `seh_main_thunk.rs`, so
// it cannot catch a future emitter that changes a guard. Source-binding the
// table to the emitter is the shared ownership-decision SSOT.
#[test]
fn cleanup_site_table_is_internally_consistent() {
    assert_eq!(CleanupSite::ALL.len(), 5);
    let names: Vec<&str> = CleanupSite::ALL.iter().map(|site| site.name()).collect();
    assert_eq!(
        names,
        vec![
            "itanium_invoke_normal",
            "itanium_catch",
            "itanium_direct_normal",
            "seh_success",
            "seh_caught",
        ]
    );
    // Every site — normal AND caught — is guarded by the one `wrapper_owns`
    // decision. No site cleans up unconditionally.
    for site in CleanupSite::ALL {
        assert!(site.is_guarded(), "{} must be guarded", site.name());
    }
}

#[test]
fn owning_wrapper_emits_every_site_skipping_wrapper_emits_none() {
    for site in CleanupSite::ALL {
        assert!(
            site.emits_cleanup(true),
            "{} must emit when owned",
            site.name()
        );
        assert!(
            !site.emits_cleanup(false),
            "{} must skip when not owned",
            site.name()
        );
    }
}

#[test]
fn site_activity_covers_every_eh_model_and_unwind_combination() {
    let itanium_unwinding: Vec<&str> = CleanupSite::ALL
        .iter()
        .filter(|site| site.is_active(EhModel::Itanium, true))
        .map(|site| site.name())
        .collect();
    assert_eq!(
        itanium_unwinding,
        vec!["itanium_invoke_normal", "itanium_catch"]
    );

    let seh_unwinding: Vec<&str> = CleanupSite::ALL
        .iter()
        .filter(|site| site.is_active(EhModel::Seh, true))
        .map(|site| site.name())
        .collect();
    assert_eq!(seh_unwinding, vec!["seh_success", "seh_caught"]);

    for model in [EhModel::Itanium, EhModel::Seh] {
        let nounwind: Vec<&str> = CleanupSite::ALL
            .iter()
            .filter(|site| site.is_active(model, false))
            .map(|site| site.name())
            .collect();
        assert_eq!(nounwind, vec!["itanium_direct_normal"]);
    }
}

// The cured emitter derives `wrapper_owns` from the owner demand, so the seam
// reads CONSISTENT: a borrowed value the wrapper owns is released on every exit.
#[test]
fn borrow_demand_owned_by_wrapper_is_consistent() {
    let seam = seam(Some(borrowed_read_only_contract()), true);
    assert_eq!(seam.owner_demand(), Some(CalleeOwnerDemand::Borrow));
    assert_eq!(
        seam.seam_verdict(EhModel::Itanium, true),
        Some(SeamVerdict::Consistent)
    );
}

// The cure: a consumed value the wrapper does NOT own reads CONSISTENT on every
// active exit, INCLUDING the unwind exit — no site double-frees.
#[test]
fn consumed_demand_not_owned_by_wrapper_is_consistent_on_both_exits() {
    let seam = seam(Some(iter_consuming_contract()), false);
    assert_eq!(seam.owner_demand(), Some(CalleeOwnerDemand::WholeValue));
    // Unwinding entry: normal AND caught sites both skip and are both OK.
    assert_eq!(
        seam.site_correctness(CleanupSite::ItaniumInvokeNormal, EhModel::Itanium, true),
        Some(SiteCorrectness::Ok)
    );
    assert_eq!(
        seam.site_correctness(CleanupSite::ItaniumCatch, EhModel::Itanium, true),
        Some(SiteCorrectness::Ok)
    );
    assert_eq!(
        seam.seam_verdict(EhModel::Itanium, true),
        Some(SeamVerdict::Consistent)
    );
    // Non-unwinding entry: the sole active site also correctly skips.
    assert_eq!(
        seam.seam_verdict(EhModel::Itanium, false),
        Some(SeamVerdict::Consistent)
    );
}

// Regression guard, unwind exit. If the emitter regressed to OWNING a consumed
// buffer (e.g. reverting to the ABI derivation), the CAUGHT site double-frees on
// the unwind exit and the seam must go DIVERGENT. A verdict blind to the caught
// site would miss this — the both-exits check catches it.
#[test]
fn owning_wrapper_over_a_consumed_value_double_frees_on_the_caught_exit() {
    let seam = seam(Some(iter_consuming_contract()), true);
    assert_eq!(seam.owner_demand(), Some(CalleeOwnerDemand::WholeValue));
    assert_eq!(
        seam.site_correctness(CleanupSite::ItaniumCatch, EhModel::Itanium, true),
        Some(SiteCorrectness::DoubleFree)
    );
    assert_eq!(
        seam.site_correctness(CleanupSite::ItaniumInvokeNormal, EhModel::Itanium, true),
        Some(SiteCorrectness::DoubleFree)
    );
    assert_eq!(
        seam.seam_verdict(EhModel::Itanium, true),
        Some(SeamVerdict::Divergent)
    );
}

// Regression guard, leak direction. A borrowed value the wrapper wrongly skips
// leaks rather than double-frees; the per-site verdict names the direction.
#[test]
fn skipping_wrapper_over_a_borrowed_value_leaks() {
    let seam = seam(Some(borrowed_read_only_contract()), false);
    assert_eq!(
        seam.site_correctness(CleanupSite::ItaniumDirectNormal, EhModel::Itanium, false),
        Some(SiteCorrectness::Leak)
    );
    assert_eq!(
        seam.seam_verdict(EhModel::Itanium, false),
        Some(SeamVerdict::Divergent)
    );
}

#[test]
fn missing_contract_yields_no_verdict() {
    let seam = seam(None, true);
    assert_eq!(seam.owner_demand(), None);
    assert_eq!(seam.seam_verdict(EhModel::Itanium, true), None);
}

#[test]
fn render_carries_every_semantic_field_and_all_five_sites() {
    let rendered = report(
        vec![seam(Some(iter_consuming_contract()), false)],
        EhModel::Itanium,
        true,
    )
    .render();
    for field in [
        "access",
        "consumption",
        "cardinality",
        "iter_consumes",
        "transfers_through_return",
        "borrowed_read_only",
        "borrowed_cow_consumed",
        "may_escape / may_share",
        "uniqueness",
        "exact_transfer",
        "callee_owner_demand",
        "realized ArcParam.ownership",
        "borrowed_rooted",
        "param_passing",
        "wrapper_owns",
    ] {
        assert!(rendered.contains(field), "render dropped `{field}`");
    }
    for site in CleanupSite::ALL {
        assert!(
            rendered.contains(site.name()),
            "render dropped site `{}`",
            site.name()
        );
    }
    // The cured consuming program reads consistent, with the active sites OK.
    assert!(rendered.contains("seam: CONSISTENT"));
    assert!(rendered.contains("TRANSFER inward"));
}

// The seam verdict tracks the semantic column: with the physical `wrapper_owns`
// held identical, a borrowed vs consumed contract must not both read the same.
#[test]
fn verdict_tracks_the_semantic_column() {
    // A wrapper that OWNS the buffer: correct for the borrowed value (release),
    // a double-free for the consumed one.
    let borrowed = report(
        vec![seam(Some(borrowed_read_only_contract()), true)],
        EhModel::Itanium,
        true,
    )
    .render();
    let consumed = report(
        vec![seam(Some(iter_consuming_contract()), true)],
        EhModel::Itanium,
        true,
    )
    .render();
    assert!(borrowed.contains("wrapper_owns              = true"));
    assert!(consumed.contains("wrapper_owns              = true"));
    assert!(borrowed.contains("iter_consumes             = false"));
    assert!(consumed.contains("iter_consumes             = true"));
    assert!(borrowed.contains("seam: CONSISTENT"));
    assert!(consumed.contains("seam: DIVERGENT"));
}

#[test]
fn report_without_params_renders_the_empty_note() {
    let rendered = report(vec![], EhModel::Itanium, false).render();
    assert!(rendered.contains("(no entry-point parameters)"));
}
