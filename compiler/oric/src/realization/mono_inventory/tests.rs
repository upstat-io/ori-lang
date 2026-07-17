use ori_ir::canon::MonoInstanceId;
use ori_ir::StringInterner;
use ori_repr::monomorphize::{MonoFunction, MonoFunctionIdentity};
use ori_types::{FunctionSig, Idx, MonoInstance};
use rustc_hash::FxHashMap;

use super::{MonoFunctionInventory, MonoFunctionInventoryError};

#[track_caller]
fn require_inventory(
    result: Result<MonoFunctionInventory, MonoFunctionInventoryError>,
    context: &str,
) -> MonoFunctionInventory {
    match result {
        Ok(inventory) => inventory,
        Err(error) => panic!("{context}: {error}"),
    }
}

#[track_caller]
fn require_inventory_error(
    result: Result<MonoFunctionInventory, MonoFunctionInventoryError>,
    context: &str,
) -> MonoFunctionInventoryError {
    match result {
        Ok(inventory) => panic!("{context}: got {inventory:?}"),
        Err(error) => error,
    }
}

fn mono(
    interner: &StringInterner,
    callable: &str,
    return_type: Idx,
    instance_id: u32,
    is_imported: bool,
) -> MonoFunction {
    let name = interner.intern(callable);
    let instance = MonoInstance::new_top_level(
        interner.intern("identity"),
        Vec::new(),
        vec![Idx::INT],
        return_type,
        Vec::new(),
    );
    MonoFunction {
        mangled_name: name,
        origin: ori_repr::monomorphize::MonoFunctionOrigin::Source,
        identity: MonoFunctionIdentity::new(&instance, MonoInstanceId::new(instance_id)),
        sig: FunctionSig::simple(name, vec![Idx::INT], return_type),
        body_type_map: FxHashMap::default(),
        is_imported,
        receiver_type_name: None,
    }
}

#[test]
fn imported_body_owns_compatible_cross_inventory_identity() {
    let interner = StringInterner::new();
    let local = mono(&interner, "identity$m$int", Idx::INT, 0, true);
    let imported = mono(&interner, "identity$m$int", Idx::INT, 1, true);

    let inventory = require_inventory(
        MonoFunctionInventory::try_new(vec![local], vec![imported], &interner),
        "compatible imported identity must own the source body",
    );

    assert_eq!(inventory.all().len(), 1);
    assert!(inventory.all()[0].is_imported);
    assert_eq!(
        inventory.all()[0].identity.instance_ids(),
        vec![MonoInstanceId::new(1), MonoInstanceId::new(0)],
        "dispatch ids from both metadata paths remain bound to the survivor"
    );
}

#[test]
fn local_only_identity_remains_a_local_body() {
    let interner = StringInterner::new();
    let local = mono(&interner, "identity$m$int", Idx::INT, 0, false);

    let inventory = require_inventory(
        MonoFunctionInventory::try_new(vec![local], Vec::new(), &interner),
        "one local identity is valid",
    );

    assert_eq!(inventory.all().len(), 1);
    assert!(!inventory.all()[0].is_imported);
}

#[test]
fn conflicting_cross_inventory_identity_fails_with_actionable_name() {
    let interner = StringInterner::new();
    let local = mono(&interner, "identity$m$int", Idx::INT, 0, true);
    let imported = mono(&interner, "identity$m$int", Idx::STR, 1, true);

    let error = require_inventory_error(
        MonoFunctionInventory::try_new(vec![local], vec![imported], &interner),
        "different concrete signatures cannot share an executable identity",
    )
    .to_string();

    assert!(error.contains("identity$m$int"));
    assert!(error.contains("concrete signature"));
    assert!(error.contains("ORI_LOG=oric::realization::mono_inventory=debug"));
}

#[test]
fn duplicate_imported_producer_is_rejected_before_lowering() {
    let interner = StringInterner::new();
    let first = mono(&interner, "identity$m$int", Idx::INT, 0, true);
    let second = mono(&interner, "identity$m$int", Idx::INT, 1, true);

    let error = require_inventory_error(
        MonoFunctionInventory::try_new(Vec::new(), vec![first, second], &interner),
        "one imported source must own each final identity",
    )
    .to_string();

    assert!(error.contains("identity$m$int"));
    assert!(error.contains("imported producer emitted it more than once"));
}

#[test]
fn duplicate_local_producer_is_rejected_before_lowering() {
    let interner = StringInterner::new();
    let first = mono(&interner, "identity$m$int", Idx::INT, 0, false);
    let second = mono(&interner, "identity$m$int", Idx::INT, 1, false);

    let error = require_inventory_error(
        MonoFunctionInventory::try_new(vec![first, second], Vec::new(), &interner),
        "one local source must own each final identity",
    )
    .to_string();

    assert!(error.contains("identity$m$int"));
    assert!(error.contains("local producer emitted it more than once"));
}

#[test]
fn genuinely_local_body_cannot_collide_with_imported_source() {
    let interner = StringInterner::new();
    let local = mono(&interner, "identity$m$int", Idx::INT, 0, false);
    let imported = mono(&interner, "identity$m$int", Idx::INT, 1, true);

    let error = require_inventory_error(
        MonoFunctionInventory::try_new(vec![local], vec![imported], &interner),
        "two source namespaces cannot own one final identity",
    )
    .to_string();

    assert!(error.contains("identity$m$int"));
    assert!(error.contains("both local and imported source bodies"));
}

#[test]
fn imported_collector_metadata_requires_an_imported_source_body() {
    let interner = StringInterner::new();
    let metadata_only = mono(&interner, "identity$m$int", Idx::INT, 0, true);

    let error = require_inventory_error(
        MonoFunctionInventory::try_new(vec![metadata_only], Vec::new(), &interner),
        "import metadata cannot be lowered against the host canon",
    )
    .to_string();

    assert!(error.contains("identity$m$int"));
    assert!(error.contains("has no imported source body"));
}
