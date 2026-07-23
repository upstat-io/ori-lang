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

fn seam(contract: Option<ParamContract>, wrapper_owns_on_normal: bool) -> EntryParamSeam {
    EntryParamSeam {
        index: 0,
        name: "args".to_owned(),
        contract,
        realized_ownership: Some(Ownership::Borrowed),
        borrowed_rooted: Some(true),
        param_passing: ParamPassing::Reference,
        wrapper_owns_on_normal,
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

#[test]
fn cleanup_site_table_is_exhaustive_and_guards_match_source() {
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
    // The two unwind-path sites clean up unconditionally; the three
    // normal-return sites are guarded by `wrapper_owns_on_normal`.
    assert!(!CleanupSite::ItaniumCatch.is_guarded());
    assert!(!CleanupSite::SehCaught.is_guarded());
    assert!(CleanupSite::ItaniumInvokeNormal.is_guarded());
    assert!(CleanupSite::ItaniumDirectNormal.is_guarded());
    assert!(CleanupSite::SehSuccess.is_guarded());
}

#[test]
fn guarded_sites_skip_when_wrapper_does_not_own() {
    for site in CleanupSite::ALL {
        assert!(
            site.emits_cleanup(true),
            "{} must emit when owned",
            site.name()
        );
        assert_eq!(
            site.emits_cleanup(false),
            !site.is_guarded(),
            "{} unguarded sites always emit",
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

#[test]
fn borrow_demand_with_owning_wrapper_is_consistent() {
    let seam = seam(Some(borrowed_read_only_contract()), true);
    assert_eq!(seam.owner_demand(), Some(CalleeOwnerDemand::Borrow));
    assert_eq!(seam.seam_verdict(), Some(SeamVerdict::Consistent));
}

#[test]
fn iter_consuming_demand_with_owning_wrapper_is_divergent() {
    let seam = seam(Some(iter_consuming_contract()), true);
    assert_eq!(seam.owner_demand(), Some(CalleeOwnerDemand::WholeValue));
    assert_eq!(seam.seam_verdict(), Some(SeamVerdict::Divergent));
}

#[test]
fn verdict_flips_with_the_semantic_column_alone() {
    // Negative pin: the physical column is IDENTICAL across the pair; only
    // the semantic contract differs. A renderer or verdict that ignores the
    // semantic column cannot distinguish these.
    let read_only = seam(Some(borrowed_read_only_contract()), true);
    let consuming = seam(Some(iter_consuming_contract()), true);
    assert_eq!(
        read_only.wrapper_owns_on_normal,
        consuming.wrapper_owns_on_normal
    );
    assert_eq!(read_only.param_passing, consuming.param_passing);
    assert_ne!(read_only.seam_verdict(), consuming.seam_verdict());
}

#[test]
fn missing_contract_yields_no_verdict() {
    let seam = seam(None, true);
    assert_eq!(seam.owner_demand(), None);
    assert_eq!(seam.seam_verdict(), None);
}

#[test]
fn render_carries_every_semantic_field_and_all_five_sites() {
    let rendered = report(
        vec![seam(Some(iter_consuming_contract()), true)],
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
        "wrapper_owns_on_normal",
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
    assert!(rendered.contains("DIVERGENT"));
    assert!(rendered.contains("TRANSFER inward"));
}

#[test]
fn render_of_the_discriminating_pair_differs_only_in_semantic_columns() {
    let read_only = report(
        vec![seam(Some(borrowed_read_only_contract()), true)],
        EhModel::Itanium,
        true,
    )
    .render();
    let consuming = report(
        vec![seam(Some(iter_consuming_contract()), true)],
        EhModel::Itanium,
        true,
    )
    .render();
    assert_ne!(read_only, consuming);
    // Physical column is identical across the pair.
    assert!(read_only.contains("wrapper_owns_on_normal    = true"));
    assert!(consuming.contains("wrapper_owns_on_normal    = true"));
    // Semantic column separates them.
    assert!(read_only.contains("iter_consumes             = false"));
    assert!(consuming.contains("iter_consumes             = true"));
    assert!(read_only.contains("seam: CONSISTENT"));
    assert!(consuming.contains("seam: DIVERGENT"));
}

#[test]
fn report_without_params_renders_the_empty_note() {
    let rendered = report(vec![], EhModel::Itanium, false).render();
    assert!(rendered.contains("(no entry-point parameters)"));
}
