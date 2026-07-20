use ori_arc::ir::PrimitiveFacts;
use ori_arc::uniqueness::{CowAnnotations, DropHints};
use ori_arc::{
    prove_param_disjointness, ArcBlock, ArcBlockId, ArcClassifier, ArcFunction, ArcInstr, ArcParam,
    ArcTerminator, ArcValue, ArcVarId, ArgOwnership, EffectSummary, LitValue, MemoryAccessClass,
    MemoryContract, MethodCallFact, MethodCallForm, OperatorCallFact, Ownership, PrimOp,
    RcStrategy, RetainPlanTable, ValueRepr, VariableMetadataState,
};
use ori_ir::{BinaryOp, SharedInterner, Span};
use ori_types::burden::UserBurdenSpec;
use ori_types::{FieldDef, Idx, Pool, TypeRegistry, Visibility};
use rustc_hash::FxHashMap;

use super::{
    validate_external_callables, BlockIndex, CallPosition, CallSite, CallableTarget,
    ExecutableProgram, ExecutableProgramParts, ExternalCallable, ExternalFactIdentities,
    ExternalUnwind, FunctionFamilyTopology, IteratorSource, RealizationError, RuntimeCall,
    UserDropBinding,
};
use crate::{NarrowingPolicy, ReprPlan};

fn empty_function(name: ori_ir::Name) -> ArcFunction {
    ArcFunction {
        name,
        params: Vec::new(),
        return_type: Idx::UNIT,
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: Vec::new(),
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(0),
            },
        }],
        entry: ArcBlockId::new(0),
        var_types: vec![Idx::UNIT],
        var_reprs: vec![ValueRepr::Scalar],
        var_rc_strategies: vec![None],
        var_metadata_state: VariableMetadataState::Realized,
        spans: Vec::new(),
        is_fbip: false,
        num_captures: 0,
        cow_annotations: CowAnnotations::default(),
        primitive_facts: PrimitiveFacts::default(),
        drop_hints: DropHints::default(),
        tail_calls: Vec::new(),
        burden_emitted: Vec::new(),
        reassign_deaths: Vec::new(),
        catch_scoped_checked_ops: Vec::new(),
        method_call_facts: Vec::new(),
        operator_call_facts: Vec::new(),
        direct_call_facts: Vec::new(),
        yield_allocations: Vec::new(),
        class_ledger_emission: false,
    }
}

fn parts(symbols: &SharedInterner) -> ExecutableProgramParts {
    let main = symbols.intern("main");
    let function = empty_function(main);
    let contract = MemoryContract::conservative(function.params.len());
    let function_effects = [(main, contract.function_effect_facts(&function))]
        .into_iter()
        .collect();
    let fresh_return_facts = [(main, contract.fresh_self_allocation_facts())]
        .into_iter()
        .collect();
    let pool = Pool::new();
    let param_disjointness = [(main, prove_param_disjointness(&[], &pool))]
        .into_iter()
        .collect();
    let contracts = [(main, contract)].into_iter().collect();
    let functions = vec![function];
    let callable_facts = ori_arc::freeze_function_callable_facts(&functions, &pool);
    ExecutableProgramParts {
        version: super::EXECUTABLE_PROGRAM_VERSION,
        symbols: symbols.clone(),
        pool,
        functions,
        contracts,
        function_effects,
        fresh_return_facts,
        param_disjointness,
        callable_facts,
        closure_adapters: FxHashMap::default(),
        retain_plans: RetainPlanTable::default(),
        function_families: vec![FunctionFamilyTopology::new(main, Vec::new())],
        roots: vec![main],
        cli_entry: Some(main),
        externals: Vec::new(),
        method_targets: FxHashMap::default(),
        user_drop_bindings: Vec::new(),
        repr_plan: ReprPlan::new(NarrowingPolicy::Disabled),
        type_registry: TypeRegistry::new(),
    }
}

#[test]
fn freezes_receiver_qualified_method_targets_to_stable_callable_identity() {
    let symbols = SharedInterner::new();
    let main = symbols.intern("main");
    let make = symbols.intern("make");
    let mut input = parts(&symbols);
    input.method_targets.insert((Idx::UNIT, make), main);

    let program = ExecutableProgram::validate(input)
        .unwrap_or_else(|error| panic!("method target should validate: {error}"));
    let main_id = program
        .function_id(main)
        .unwrap_or_else(|| panic!("main must have a stable identity"));

    assert_eq!(
        program.method_target(Idx::UNIT, make),
        Some(CallableTarget::Function(main_id))
    );
}

#[test]
fn rejects_invalid_or_unavailable_method_targets() {
    let symbols = SharedInterner::new();
    let debug = symbols.intern("debug");
    let main = symbols.intern("main");
    let mut invalid_receiver = parts(&symbols);
    let invalid = Idx::from_raw(u32::MAX);
    invalid_receiver
        .method_targets
        .insert((invalid, debug), main);
    assert!(matches!(
        ExecutableProgram::validate(invalid_receiver),
        Err(RealizationError::InvalidMethodTargetReceiver { receiver, method })
            if receiver == invalid && method == debug
    ));

    let missing = symbols.intern("missing.debug");
    let mut unavailable = parts(&symbols);
    unavailable
        .method_targets
        .insert((Idx::INT, debug), missing);
    assert!(matches!(
        ExecutableProgram::validate(unavailable),
        Err(RealizationError::MissingMethodTarget { receiver, method, target })
            if receiver == Idx::INT && method == debug && target == missing
    ));
}

fn user_drop_parts(
    symbols: &SharedInterner,
) -> (
    ExecutableProgramParts,
    Idx,
    ori_registry::FnSym,
    ori_ir::Name,
) {
    let mut input = parts(symbols);
    let type_name = symbols.intern("DropFixture");
    let field_name = symbols.intern("value");
    let target_name = symbols.intern("DropFixture.drop");
    let mut pool = Pool::new();
    let ty = pool.struct_type(type_name, &[(field_name, Idx::INT)]);
    let logical = ori_registry::FnSym::new(core::num::NonZeroU32::MIN);
    let burden = UserBurdenSpec {
        self_owned_identity: true,
        drop_operation: Some(logical),
        user_drop: Some(logical),
        ..UserBurdenSpec::default()
    };
    let mut registry = TypeRegistry::new();
    registry.register_struct(
        type_name,
        ty,
        Vec::new(),
        vec![FieldDef {
            name: field_name,
            ty: Idx::INT,
            span: Span::DUMMY,
            visibility: Visibility::Private,
        }],
        Span::DUMMY,
        Visibility::Private,
        0,
        None,
        Some(burden),
    );

    let mut target = empty_function(target_name);
    target.params.push(ArcParam {
        var: ArcVarId::new(0),
        ty,
        ownership: Ownership::Borrowed,
    });
    target.var_types = vec![ty, Idx::UNIT];
    target.blocks[0].body.push(ArcInstr::Let {
        dst: ArcVarId::new(1),
        ty: Idx::UNIT,
        value: ArcValue::Literal(LitValue::Unit),
    });
    target.blocks[0].terminator = ArcTerminator::Return {
        value: ArcVarId::new(1),
    };
    populate_metadata(&mut target, &pool);

    let contract = MemoryContract::conservative(target.params.len());
    input
        .function_effects
        .insert(target_name, contract.function_effect_facts(&target));
    input
        .fresh_return_facts
        .insert(target_name, contract.fresh_self_allocation_facts());
    input
        .param_disjointness
        .insert(target_name, prove_param_disjointness(&[ty], &pool));
    input.contracts.insert(target_name, contract);
    input.functions.push(target);
    input
        .function_families
        .push(FunctionFamilyTopology::new(target_name, Vec::new()));
    input.pool = pool;
    input.type_registry = registry;
    input.callable_facts = ori_arc::freeze_function_callable_facts(&input.functions, &input.pool);
    input
        .user_drop_bindings
        .push(UserDropBinding::new(ty, logical, target_name));
    (input, ty, logical, target_name)
}

#[test]
fn freezes_exact_user_drop_operation_identity_and_target() {
    let symbols = SharedInterner::new();
    let (input, ty, logical, target_name) = user_drop_parts(&symbols);
    let program = ExecutableProgram::validate(input)
        .unwrap_or_else(|error| panic!("exact user-drop plan should validate: {error}"));
    let operation = program
        .user_drop_for_type(ty)
        .unwrap_or_else(|| panic!("user-drop type must resolve to its frozen operation"));

    assert_eq!(operation.logical(), logical);
    assert_eq!(
        program.functions()[operation.target().index()].name,
        target_name
    );
}

#[test]
fn user_drop_plan_rejects_missing_mismatched_and_invalid_bindings() {
    let symbols = SharedInterner::new();

    let (mut missing, ty, _, _) = user_drop_parts(&symbols);
    missing.user_drop_bindings.clear();
    assert!(matches!(
        ExecutableProgram::validate(missing),
        Err(RealizationError::MissingUserDropBinding { ty: found }) if found == ty
    ));

    let (mut mismatched, ty, _, target) = user_drop_parts(&symbols);
    let wrong = ori_registry::FnSym::new(
        core::num::NonZeroU32::new(2).unwrap_or_else(|| unreachable!("two is nonzero")),
    );
    mismatched.user_drop_bindings = vec![UserDropBinding::new(ty, wrong, target)];
    assert!(matches!(
        ExecutableProgram::validate(mismatched),
        Err(RealizationError::UserDropLogicalIdentityMismatch { ty: found, .. }) if found == ty
    ));

    let (mut invalid_signature, ty, logical, _) = user_drop_parts(&symbols);
    let main = symbols.intern("main");
    invalid_signature.user_drop_bindings = vec![UserDropBinding::new(ty, logical, main)];
    assert!(matches!(
        ExecutableProgram::validate(invalid_signature),
        Err(RealizationError::InvalidUserDropSignature { ty: found, target, .. })
            if found == ty && target == main
    ));
}

#[test]
fn closes_valid_main_function() {
    let symbols = SharedInterner::new();
    let program = ExecutableProgram::validate(parts(&symbols));
    program.unwrap_or_else(|error| panic!("valid main function was rejected: {error:?}"));
}

#[test]
fn freezes_function_family_topology_after_stable_function_sort() {
    let symbols = SharedInterner::new();
    let mut input = parts(&symbols);
    let main = symbols.intern("main");
    let first_lambda = symbols.intern("__lambda_main_z");
    let second_lambda = symbols.intern("__lambda_main_a");
    for lambda_name in [first_lambda, second_lambda] {
        let lambda = empty_function(lambda_name);
        let contract = MemoryContract::conservative(0);
        input
            .function_effects
            .insert(lambda_name, contract.function_effect_facts(&lambda));
        input
            .fresh_return_facts
            .insert(lambda_name, contract.fresh_self_allocation_facts());
        input
            .param_disjointness
            .insert(lambda_name, prove_param_disjointness(&[], &input.pool));
        input.contracts.insert(lambda_name, contract);
        input.functions.push(lambda);
    }
    input.function_families = vec![FunctionFamilyTopology::new(
        main,
        vec![first_lambda, second_lambda],
    )];
    input.callable_facts = ori_arc::freeze_function_callable_facts(&input.functions, &input.pool);

    let program = ExecutableProgram::validate(input)
        .unwrap_or_else(|error| panic!("function family should validate: {error}"));
    let main_id = program
        .function_id(main)
        .unwrap_or_else(|| panic!("main must have a stable identity"));
    let first_lambda_id = program
        .function_id(first_lambda)
        .unwrap_or_else(|| panic!("first lambda must have a stable identity"));
    let second_lambda_id = program
        .function_id(second_lambda)
        .unwrap_or_else(|| panic!("second lambda must have a stable identity"));

    assert_eq!(program.function(main_id).name, main);
    assert_eq!(program.function(first_lambda_id).name, first_lambda);
    assert_eq!(program.function(second_lambda_id).name, second_lambda);
    assert_eq!(
        program.function_family_lambdas(main_id),
        Some(&[first_lambda_id, second_lambda_id][..])
    );
    assert_eq!(program.function_family_lambdas(first_lambda_id), None);
    assert!(second_lambda_id.index() < first_lambda_id.index());
}

#[test]
fn rejects_unknown_duplicate_missing_and_root_lambda_family_members() {
    let symbols = SharedInterner::new();
    let main = symbols.intern("main");
    let missing = symbols.intern("missing");

    let mut unknown_parent = parts(&symbols);
    unknown_parent.function_families = vec![FunctionFamilyTopology::new(missing, Vec::new())];
    assert!(matches!(
        ExecutableProgram::validate(unknown_parent),
        Err(RealizationError::UnknownFunctionFamilyParent { parent }) if parent == missing
    ));

    let mut unknown_lambda = parts(&symbols);
    unknown_lambda.function_families = vec![FunctionFamilyTopology::new(main, vec![missing])];
    assert!(matches!(
        ExecutableProgram::validate(unknown_lambda),
        Err(RealizationError::UnknownFunctionFamilyLambda { parent, lambda })
            if parent == main && lambda == missing
    ));

    let mut duplicate = parts(&symbols);
    duplicate.function_families = vec![
        FunctionFamilyTopology::new(main, Vec::new()),
        FunctionFamilyTopology::new(main, Vec::new()),
    ];
    assert!(matches!(
        ExecutableProgram::validate(duplicate),
        Err(RealizationError::DuplicateFunctionFamilyMember { member }) if member == main
    ));

    let mut missing_family = parts(&symbols);
    missing_family.function_families.clear();
    assert!(matches!(
        ExecutableProgram::validate(missing_family),
        Err(RealizationError::MissingFunctionFamily { function }) if function == main
    ));

    let lambda_name = symbols.intern("__lambda_main_0");
    let lambda = empty_function(lambda_name);
    let contract = MemoryContract::conservative(0);
    let mut root_lambda = parts(&symbols);
    root_lambda
        .function_effects
        .insert(lambda_name, contract.function_effect_facts(&lambda));
    root_lambda
        .fresh_return_facts
        .insert(lambda_name, contract.fresh_self_allocation_facts());
    root_lambda.param_disjointness.insert(
        lambda_name,
        prove_param_disjointness(&[], &root_lambda.pool),
    );
    root_lambda.contracts.insert(lambda_name, contract);
    root_lambda.functions.push(lambda);
    root_lambda.function_families = vec![FunctionFamilyTopology::new(main, vec![lambda_name])];
    root_lambda.callable_facts =
        ori_arc::freeze_function_callable_facts(&root_lambda.functions, &root_lambda.pool);
    root_lambda.roots = vec![lambda_name];
    root_lambda.cli_entry = None;
    assert!(matches!(
        ExecutableProgram::validate(root_lambda),
        Err(RealizationError::ProgramRootIsLambda { name }) if name == lambda_name
    ));
}

#[test]
fn freezes_explicit_roots_without_fabricating_a_cli_entry() {
    let symbols = SharedInterner::new();
    let mut input = parts(&symbols);
    input.cli_entry = None;

    let program = ExecutableProgram::validate(input)
        .unwrap_or_else(|error| panic!("library artifact should validate: {error}"));
    assert!(program.cli_entry().is_none());
    assert_eq!(program.roots().len(), 1);
}

#[test]
fn accepts_a_type_only_library_with_no_functions_or_roots() {
    let symbols = SharedInterner::new();
    let mut input = parts(&symbols);
    input.functions.clear();
    input.contracts.clear();
    input.function_effects.clear();
    input.fresh_return_facts.clear();
    input.param_disjointness.clear();
    input.callable_facts.clear();
    input.function_families.clear();
    input.roots.clear();
    input.cli_entry = None;

    let program = ExecutableProgram::validate(input)
        .unwrap_or_else(|error| panic!("type-only library should validate: {error}"));
    assert!(program.functions().is_empty());
    assert!(program.roots().is_empty());
    assert!(program.cli_entry().is_none());
}

#[test]
fn rejects_empty_duplicate_unknown_and_nonroot_cli_entries() {
    let symbols = SharedInterner::new();
    let main = symbols.intern("main");

    let mut empty = parts(&symbols);
    empty.roots.clear();
    assert!(matches!(
        ExecutableProgram::validate(empty),
        Err(RealizationError::MissingProgramRoots)
    ));

    let mut duplicate = parts(&symbols);
    duplicate.roots.push(main);
    assert!(matches!(
        ExecutableProgram::validate(duplicate),
        Err(RealizationError::DuplicateProgramRoot { name }) if name == main
    ));

    let missing = symbols.intern("missing_root");
    let mut unknown = parts(&symbols);
    unknown.roots = vec![missing];
    unknown.cli_entry = None;
    assert!(matches!(
        ExecutableProgram::validate(unknown),
        Err(RealizationError::MissingProgramRoot { name }) if name == missing
    ));

    let mut nonroot = parts(&symbols);
    nonroot.cli_entry = Some(missing);
    assert!(matches!(
        ExecutableProgram::validate(nonroot),
        Err(RealizationError::CliEntryNotRoot { name }) if name == missing
    ));
}

#[test]
fn resolves_external_call_from_exact_producer_facts() {
    let symbols = SharedInterner::new();
    let mut input = parts(&symbols);
    let external_name = symbols.intern("dependency");
    let mut contract = MemoryContract::conservative(0);
    contract.effects.may_throw = false;
    input.externals.push(ExternalCallable::freeze(
        external_name,
        "_ori_dependency",
        Vec::new(),
        Idx::UNIT,
        contract,
        ExternalUnwind::NoUnwind,
        &input.pool,
    ));
    input.functions[0].blocks[0].body.push(ArcInstr::Apply {
        dst: ArcVarId::new(0),
        ty: Idx::UNIT,
        func: external_name,
        args: Vec::new(),
        arg_ownership: Vec::new(),
        mono_instance_id: None,
    });
    let program = ExecutableProgram::validate(input)
        .unwrap_or_else(|error| panic!("typed external call should validate: {error}"));
    let entry = program
        .cli_entry()
        .unwrap_or_else(|| panic!("fixture must have a CLI entry"));
    let block = BlockIndex::new(0, program.functions()[entry.index()].name)
        .unwrap_or_else(|error| panic!("block should be representable: {error}"));
    let position = CallPosition::instruction(0, program.functions()[entry.index()].name)
        .unwrap_or_else(|error| panic!("instruction should be representable: {error}"));
    let external = program
        .external_function_id(external_name)
        .unwrap_or_else(|| panic!("external should have a stable identity"));
    assert_eq!(
        program.call_target(CallSite::new(entry, block, position)),
        Some(CallableTarget::External(external))
    );
    assert_eq!(
        program.external_function(external).link_symbol(),
        "_ori_dependency"
    );
}

#[test]
fn rejects_stale_external_facts_and_local_body_collisions() {
    let symbols = SharedInterner::new();
    let main = symbols.intern("main");
    let external_name = symbols.intern("dependency");
    let mut contract = MemoryContract::conservative(0);
    contract.effects.may_throw = false;

    let mut stale = parts(&symbols);
    let frozen = ExternalCallable::freeze(
        external_name,
        "_ori_dependency",
        Vec::new(),
        Idx::UNIT,
        contract.clone(),
        ExternalUnwind::NoUnwind,
        &stale.pool,
    );
    let ids = frozen.identities();
    stale.externals.push(ExternalCallable::from_imported_parts(
        external_name,
        "_ori_dependency",
        Vec::new(),
        Idx::UNIT,
        contract.clone(),
        ExternalUnwind::NoUnwind,
        ExternalFactIdentities::from_raw(
            ids.signature() ^ 1,
            ids.ownership(),
            ids.effects(),
            ids.unwind(),
        ),
    ));
    assert!(matches!(
        ExecutableProgram::validate(stale),
        Err(RealizationError::StaleExternalFacts { name }) if name == external_name
    ));

    let mut collision = parts(&symbols);
    collision.externals.push(ExternalCallable::freeze(
        main,
        "_ori_other_main",
        Vec::new(),
        Idx::UNIT,
        contract,
        ExternalUnwind::NoUnwind,
        &collision.pool,
    ));
    assert!(matches!(
        ExecutableProgram::validate(collision),
        Err(RealizationError::ExternalFunctionBodyCollision { name }) if name == main
    ));
}

#[test]
fn exports_callable_metadata_only_from_the_closed_program_and_revalidates_import_signature() {
    let symbols = SharedInterner::new();
    let imported_name = symbols.intern("imported_main");
    let program = ExecutableProgram::validate(parts(&symbols))
        .unwrap_or_else(|error| panic!("producer artifact should validate: {error}"));
    let producer = program
        .cli_entry()
        .unwrap_or_else(|| panic!("fixture must have a CLI entry"));
    let metadata = program.export_callable_metadata(producer, "_ori_producer_main");

    let exact = ExternalCallable::from_imported_metadata(
        imported_name,
        "_ori_producer_main",
        Vec::new(),
        Idx::UNIT,
        metadata.clone(),
    );
    validate_external_callables(std::slice::from_ref(&exact), program.pool())
        .unwrap_or_else(|error| panic!("exact producer metadata should import: {error}"));
    assert_eq!(exact.contract(), program.function_contract(producer));
    assert_eq!(
        exact.unwind(),
        ExternalUnwind::from_effects(program.effect_summary(producer))
    );

    let signature_mismatch = ExternalCallable::from_imported_metadata(
        imported_name,
        "_ori_producer_main",
        Vec::new(),
        Idx::INT,
        metadata,
    );
    assert!(matches!(
        validate_external_callables(std::slice::from_ref(&signature_mismatch), program.pool()),
        Err(RealizationError::StaleExternalFacts { name }) if name == imported_name
    ));
}

#[test]
fn rejects_external_aliases_with_different_logical_facts() {
    let symbols = SharedInterner::new();
    let first = symbols.intern("first_alias");
    let second = symbols.intern("second_alias");
    let mut input = parts(&symbols);
    let mut first_contract = MemoryContract::conservative(0);
    first_contract.effects.may_throw = false;
    let mut second_contract = first_contract.clone();
    second_contract.effects.may_allocate = false;
    input.externals.extend([
        ExternalCallable::freeze(
            first,
            "_ori_shared_symbol",
            Vec::new(),
            Idx::UNIT,
            first_contract,
            ExternalUnwind::NoUnwind,
            &input.pool,
        ),
        ExternalCallable::freeze(
            second,
            "_ori_shared_symbol",
            Vec::new(),
            Idx::UNIT,
            second_contract,
            ExternalUnwind::NoUnwind,
            &input.pool,
        ),
    ]);

    assert!(matches!(
        ExecutableProgram::validate(input),
        Err(RealizationError::ExternalAliasFactsMismatch { .. })
    ));
}

#[test]
fn rejects_nested_bound_var_before_backend_selection() {
    let symbols = SharedInterner::new();
    let main = symbols.intern("main");
    let mut input = parts(&symbols);
    let mut pool = Pool::new();
    let bound = pool.bound_var(7);
    let unresolved_list = pool.list(bound);
    input.functions[0].return_type = unresolved_list;
    input.pool = pool;

    let error = ExecutableProgram::validate(input)
        .err()
        .unwrap_or_else(|| panic!("artifact closure must reject nested BoundVar leaves"));
    assert!(matches!(
        error,
        RealizationError::UnresolvedBoundVar {
            function,
            var_id: ArcVarId::INVALID,
            idx,
            ..
        } if function == main && idx == bound
    ));
}

#[test]
fn rejects_realized_primitive_without_its_frozen_fact() {
    let symbols = SharedInterner::new();
    let main = symbols.intern("main");
    let mut input = parts(&symbols);
    let function = &mut input.functions[0];
    function.return_type = Idx::INT;
    function.var_types[0] = Idx::INT;
    function.blocks[0].body.push(ArcInstr::Let {
        dst: ArcVarId::new(0),
        ty: Idx::INT,
        value: ArcValue::PrimOp {
            op: PrimOp::Binary(BinaryOp::Add),
            args: vec![ArcVarId::new(0), ArcVarId::new(0)],
        },
    });

    let error = ExecutableProgram::validate(input)
        .err()
        .unwrap_or_else(|| panic!("primitive facts must be part of artifact closure"));
    assert!(matches!(
        error,
        RealizationError::InvalidPrimitiveFacts { function, .. } if function == main
    ));
}

#[test]
fn freezes_param_disjointness_by_stable_function_id() {
    let symbols = SharedInterner::new();
    let mut input = parts(&symbols);
    let helper_name = symbols.intern("a_helper");
    let mut helper = empty_function(helper_name);
    helper.params.push(ArcParam {
        var: ArcVarId::new(0),
        ty: Idx::STR,
        ownership: Ownership::Borrowed,
    });
    helper.var_types[0] = Idx::STR;
    populate_metadata(&mut helper, &input.pool);
    let contract = MemoryContract::conservative(helper.params.len());
    input
        .function_effects
        .insert(helper_name, contract.function_effect_facts(&helper));
    input
        .fresh_return_facts
        .insert(helper_name, contract.fresh_self_allocation_facts());
    let param_types: Vec<_> = helper.params.iter().map(|param| param.ty).collect();
    input.param_disjointness.insert(
        helper_name,
        prove_param_disjointness(&param_types, &input.pool),
    );
    input.contracts.insert(helper_name, contract);
    input.functions.push(helper);
    input
        .function_families
        .push(FunctionFamilyTopology::new(helper_name, Vec::new()));
    input.callable_facts = ori_arc::freeze_function_callable_facts(&input.functions, &input.pool);

    let program = ExecutableProgram::validate(input)
        .unwrap_or_else(|error| panic!("shared realization must freeze RL-31 facts: {error}"));
    let helper_id = program
        .function_id(helper_name)
        .unwrap_or_else(|| panic!("helper must have a stable identity"));
    assert!(
        helper_id.index()
            < program
                .cli_entry()
                .unwrap_or_else(|| panic!("fixture must have a CLI entry"))
                .index()
    );

    let facts = program.param_disjointness(helper_id);
    assert_eq!(facts.type_disjointness(), [true]);
    assert!(!facts.call_site_provenance_disjoint());
    assert!(
        !facts.proves_disjoint(0),
        "the frozen combined fact must fail closed while provenance is unproven"
    );
}

#[test]
fn freezes_final_effect_summary_by_stable_function_id() {
    let symbols = SharedInterner::new();
    let mut input = parts(&symbols);
    let helper_name = symbols.intern("a_helper");
    let helper = empty_function(helper_name);
    let effects = EffectSummary {
        may_allocate: false,
        alloc_only_on_slow_path: false,
        may_deallocate: false,
        may_share: false,
        may_throw: true,
        has_unbounded_stack: false,
    };
    let mut contract = MemoryContract::conservative(0);
    contract.effects = effects;
    input
        .function_effects
        .insert(helper_name, contract.function_effect_facts(&helper));
    input
        .fresh_return_facts
        .insert(helper_name, contract.fresh_self_allocation_facts());
    input
        .param_disjointness
        .insert(helper_name, prove_param_disjointness(&[], &input.pool));
    input.contracts.insert(helper_name, contract);
    input.functions.push(helper);
    input
        .function_families
        .push(FunctionFamilyTopology::new(helper_name, Vec::new()));
    input.callable_facts = ori_arc::freeze_function_callable_facts(&input.functions, &input.pool);

    let program = ExecutableProgram::validate(input)
        .unwrap_or_else(|error| panic!("final AIMS facts must freeze: {error}"));
    let helper_id = program
        .function_id(helper_name)
        .unwrap_or_else(|| panic!("helper must have a stable identity"));

    assert_eq!(program.effect_summary(helper_id), effects);
    assert_eq!(program.function_contract(helper_id).effects, effects);
    assert_eq!(
        program.function_effects(helper_id).memory_access(),
        MemoryAccessClass::ReadWrite
    );
}

#[test]
fn rejects_missing_unexpected_and_wrong_arity_contracts() {
    let symbols = SharedInterner::new();
    let main = symbols.intern("main");

    let mut missing = parts(&symbols);
    missing.contracts.clear();
    let error = ExecutableProgram::validate(missing)
        .err()
        .unwrap_or_else(|| panic!("missing final contract must be rejected"));
    assert!(matches!(
        error,
        RealizationError::MissingFunctionContract { function, .. } if function == main
    ));

    let mut unexpected = parts(&symbols);
    let ghost = symbols.intern("ghost");
    unexpected
        .contracts
        .insert(ghost, MemoryContract::conservative(0));
    let error = ExecutableProgram::validate(unexpected)
        .err()
        .unwrap_or_else(|| panic!("contract without a body must be rejected"));
    assert!(matches!(
        error,
        RealizationError::UnexpectedFunctionContract { function } if function == ghost
    ));

    let mut wrong_arity = parts(&symbols);
    wrong_arity
        .contracts
        .insert(main, MemoryContract::conservative(1));
    let error = ExecutableProgram::validate(wrong_arity)
        .err()
        .unwrap_or_else(|| panic!("contract arity mismatch must be rejected"));
    assert!(matches!(
        error,
        RealizationError::FunctionContractArity {
            function,
            parameters: 0,
            contract_parameters: 1,
            ..
        } if function == main
    ));
}

#[test]
fn rejects_missing_unexpected_and_mismatched_function_effect_facts() {
    let symbols = SharedInterner::new();
    let main = symbols.intern("main");

    let mut missing = parts(&symbols);
    missing.function_effects.clear();
    let error = ExecutableProgram::validate(missing)
        .err()
        .unwrap_or_else(|| panic!("missing final RL-30 facts must be rejected"));
    assert!(matches!(
        error,
        RealizationError::MissingFunctionEffectFacts { function, .. } if function == main
    ));

    let mut unexpected = parts(&symbols);
    let ghost = symbols.intern("ghost");
    let ghost_function = empty_function(ghost);
    let ghost_contract = MemoryContract::conservative(0);
    unexpected
        .function_effects
        .insert(ghost, ghost_contract.function_effect_facts(&ghost_function));
    let error = ExecutableProgram::validate(unexpected)
        .err()
        .unwrap_or_else(|| panic!("RL-30 facts without a body must be rejected"));
    assert!(matches!(
        error,
        RealizationError::UnexpectedFunctionEffectFacts { function } if function == ghost
    ));

    let mut mismatch = parts(&symbols);
    let mut other_contract = MemoryContract::conservative(0);
    other_contract.effects = EffectSummary::OPTIMISTIC;
    let other_facts = other_contract.function_effect_facts(&mismatch.functions[0]);
    mismatch.function_effects.insert(main, other_facts);
    let error = ExecutableProgram::validate(mismatch)
        .err()
        .unwrap_or_else(|| panic!("RL-30 facts from another contract must be rejected"));
    assert!(matches!(
        error,
        RealizationError::FunctionEffectFactsMismatch { function, .. } if function == main
    ));
}

#[test]
fn freezes_and_validates_exact_fresh_return_facts() {
    let symbols = SharedInterner::new();
    let main = symbols.intern("main");

    let mut positive = parts(&symbols);
    positive
        .contracts
        .get_mut(&main)
        .unwrap_or_else(|| panic!("fixture contract must exist"))
        .return_info
        .returns_fresh_self_alloc = true;
    let positive_contract = &positive.contracts[&main];
    positive
        .fresh_return_facts
        .insert(main, positive_contract.fresh_self_allocation_facts());
    let program = ExecutableProgram::validate(positive)
        .unwrap_or_else(|error| panic!("fresh-return fact must freeze: {error}"));
    let entry = program
        .cli_entry()
        .unwrap_or_else(|| panic!("fixture must have a CLI entry"));
    assert!(program.fresh_return_facts(entry).is_proven());

    let mut forwarded_or_consumed = parts(&symbols);
    forwarded_or_consumed
        .contracts
        .get_mut(&main)
        .unwrap_or_else(|| panic!("fixture contract must exist"))
        .return_info
        .preserves_freshness = true;
    assert!(
        !forwarded_or_consumed.fresh_return_facts[&main].is_proven(),
        "preserved freshness cannot upgrade caller-owned or consumed storage"
    );
    let program = ExecutableProgram::validate(forwarded_or_consumed)
        .unwrap_or_else(|error| panic!("negative fresh-return fact must freeze: {error}"));
    let entry = program
        .cli_entry()
        .unwrap_or_else(|| panic!("fixture must have a CLI entry"));
    assert!(!program.fresh_return_facts(entry).is_proven());

    let mut missing = parts(&symbols);
    missing.fresh_return_facts.clear();
    let error = ExecutableProgram::validate(missing)
        .err()
        .unwrap_or_else(|| panic!("missing final RL-29 facts must be rejected"));
    assert!(matches!(
        error,
        RealizationError::MissingFreshReturnFacts { function, .. } if function == main
    ));

    let mut unexpected = parts(&symbols);
    let ghost = symbols.intern("ghost");
    unexpected.fresh_return_facts.insert(
        ghost,
        MemoryContract::conservative(0).fresh_self_allocation_facts(),
    );
    let error = ExecutableProgram::validate(unexpected)
        .err()
        .unwrap_or_else(|| panic!("RL-29 facts without a body must be rejected"));
    assert!(matches!(
        error,
        RealizationError::UnexpectedFreshReturnFacts { function } if function == ghost
    ));

    let mut mismatch = parts(&symbols);
    let mut other = MemoryContract::conservative(0);
    other.return_info.returns_fresh_self_alloc = true;
    mismatch
        .fresh_return_facts
        .insert(main, other.fresh_self_allocation_facts());
    let error = ExecutableProgram::validate(mismatch)
        .err()
        .unwrap_or_else(|| panic!("RL-29 facts from another contract must be rejected"));
    assert!(matches!(
        error,
        RealizationError::FreshReturnFactsMismatch { function, .. } if function == main
    ));
}

#[test]
fn rejects_missing_unexpected_and_wrong_arity_param_disjointness_facts() {
    let symbols = SharedInterner::new();
    let main = symbols.intern("main");

    let mut missing = parts(&symbols);
    missing.param_disjointness.clear();
    let error = ExecutableProgram::validate(missing)
        .err()
        .unwrap_or_else(|| panic!("missing final RL-31 facts must be rejected"));
    assert!(matches!(
        error,
        RealizationError::MissingParamDisjointnessFacts { function, .. } if function == main
    ));

    let mut unexpected = parts(&symbols);
    let ghost = symbols.intern("ghost");
    let ghost_facts = prove_param_disjointness(&[], &unexpected.pool);
    unexpected.param_disjointness.insert(ghost, ghost_facts);
    let error = ExecutableProgram::validate(unexpected)
        .err()
        .unwrap_or_else(|| panic!("RL-31 facts without a body must be rejected"));
    assert!(matches!(
        error,
        RealizationError::UnexpectedParamDisjointnessFacts { function } if function == ghost
    ));

    let mut wrong_arity = parts(&symbols);
    let wrong_facts = prove_param_disjointness(&[Idx::STR], &wrong_arity.pool);
    wrong_arity.param_disjointness.insert(main, wrong_facts);
    let error = ExecutableProgram::validate(wrong_arity)
        .err()
        .unwrap_or_else(|| panic!("RL-31 arity mismatch must be rejected"));
    assert!(matches!(
        error,
        RealizationError::ParamDisjointnessArity {
            function,
            parameters: 0,
            fact_parameters: 1,
            ..
        } if function == main
    ));
}

#[test]
fn rejects_unrealized_zero_variable_function() {
    let symbols = SharedInterner::new();
    let main = symbols.intern("main");
    let mut input = parts(&symbols);
    let function = &mut input.functions[0];
    function.blocks[0].terminator = ArcTerminator::Unreachable;
    function.var_types.clear();
    function.var_reprs.clear();
    function.var_rc_strategies.clear();
    function.var_metadata_state = VariableMetadataState::Unrealized;

    let error = ExecutableProgram::validate(input)
        .err()
        .unwrap_or_else(|| panic!("unrealized empty metadata must not cross the backend seam"));

    assert!(matches!(
        error,
        RealizationError::VariableMetadataUnrealized { function, .. } if function == main
    ));
    let message = error.to_string();
    assert!(message.contains("ARC pipeline did not finish variable metadata for function 'main'"));
    assert!(message.contains("rerun the same command with ORI_VERIFY_ARC=1"));
}

#[test]
fn accepts_explicitly_realized_zero_variable_function() {
    let symbols = SharedInterner::new();
    let mut input = parts(&symbols);
    let function = &mut input.functions[0];
    function.blocks[0].terminator = ArcTerminator::Unreachable;
    function.var_types.clear();
    function.var_reprs.clear();
    function.var_rc_strategies.clear();
    function.var_metadata_state = VariableMetadataState::Realized;
    input.callable_facts = ori_arc::freeze_function_callable_facts(&input.functions, &input.pool);

    ExecutableProgram::validate(input)
        .unwrap_or_else(|error| panic!("explicitly realized empty metadata is valid: {error}"));
}

#[test]
fn rejects_duplicate_function_identity() {
    let symbols = SharedInterner::new();
    let mut input = parts(&symbols);
    input.functions.push(input.functions[0].clone());
    let result = ExecutableProgram::validate(input);
    assert!(matches!(
        result,
        Err(RealizationError::DuplicateFunction { .. })
    ));
}

#[test]
fn rejects_incomplete_representation_and_rc_strategy_tables() {
    let symbols = SharedInterner::new();
    let main = symbols.intern("main");
    let cases = [
        MetadataLengthMutation::MissingRepresentation,
        MetadataLengthMutation::MissingStrategy,
        MetadataLengthMutation::ExtraStrategy,
    ];
    let mut visited = 0;
    for mutation in cases {
        let mut input = parts(&symbols);
        match mutation {
            MetadataLengthMutation::MissingRepresentation => input.functions[0].var_reprs.clear(),
            MetadataLengthMutation::MissingStrategy => {
                input.functions[0].var_rc_strategies.clear();
            }
            MetadataLengthMutation::ExtraStrategy => {
                input.functions[0].var_rc_strategies.push(None);
            }
        }

        let error = ExecutableProgram::validate(input)
            .err()
            .unwrap_or_else(|| panic!("{mutation:?} should be rejected"));
        assert_metadata_message(&error);
        match mutation {
            MetadataLengthMutation::MissingRepresentation => assert!(matches!(
                error,
                RealizationError::RepresentationMetadata {
                    function,
                    variables: 1,
                    representations: 0,
                    ..
                } if function == main
            )),
            MetadataLengthMutation::MissingStrategy => assert!(matches!(
                error,
                RealizationError::RcStrategyMetadata {
                    function,
                    variables: 1,
                    strategies: 0,
                    ..
                } if function == main
            )),
            MetadataLengthMutation::ExtraStrategy => assert!(matches!(
                error,
                RealizationError::RcStrategyMetadata {
                    function,
                    variables: 1,
                    strategies: 2,
                    ..
                } if function == main
            )),
        }
        visited += 1;
    }
    assert_eq!(visited, cases.len());
}

#[test]
fn rejects_scalar_and_non_scalar_rc_strategy_shape_mismatches() {
    let symbols = SharedInterner::new();
    let main = symbols.intern("main");
    let cases = [
        (ValueRepr::Scalar, Some(RcStrategy::HeapPointer)),
        (ValueRepr::RcPointer, None),
    ];
    let mut visited = 0;
    for (representation, strategy) in cases {
        let mut input = parts(&symbols);
        input.functions[0].var_reprs[0] = representation;
        input.functions[0].var_rc_strategies[0] = strategy;

        let error = ExecutableProgram::validate(input)
            .err()
            .unwrap_or_else(|| panic!("shape mismatch should be rejected"));
        assert_metadata_message(&error);
        assert!(matches!(
            error,
            RealizationError::RcStrategyShape {
                function,
                register: 0,
                representation: found_representation,
                strategy: found_strategy,
                ..
            } if function == main
                && found_representation == representation
                && found_strategy == strategy
        ));
        visited += 1;
    }
    assert_eq!(visited, cases.len());
}

#[test]
fn rejects_cached_strategy_that_disagrees_with_realized_representation() {
    let symbols = SharedInterner::new();
    let main = symbols.intern("main");
    let mut input = parts(&symbols);
    let mut pool = Pool::new();
    let list = pool.list(Idx::INT);
    input.functions[0].var_types[0] = list;
    input.functions[0].var_reprs[0] = ValueRepr::RcPointer;
    input.functions[0].var_rc_strategies[0] = Some(RcStrategy::AggregateFields);
    input.pool = pool;

    let error = ExecutableProgram::validate(input)
        .err()
        .unwrap_or_else(|| panic!("strategy mismatch should be rejected"));
    assert_metadata_message(&error);
    assert!(matches!(
        error,
        RealizationError::RcStrategyCoherence {
            function,
            register: 0,
            expected: Some(RcStrategy::HeapPointer),
            found: Some(RcStrategy::AggregateFields),
            ..
        } if function == main
    ));
}

fn assert_metadata_message(error: &RealizationError) {
    let message = error.to_string();
    assert!(message.contains("ARC pipeline produced"));
    assert!(message.contains("function 'main'"));
    assert!(message
        .contains("rerun the same command with ORI_VERIFY_ARC=1 and report this compiler bug"));
    assert!(!message.contains("Name("));
}

#[derive(Clone, Copy, Debug)]
enum MetadataLengthMutation {
    MissingRepresentation,
    MissingStrategy,
    ExtraStrategy,
}

#[test]
fn unresolved_callable_message_names_cause_and_fix() {
    let symbols = SharedInterner::new();
    let mut input = parts(&symbols);
    let missing = symbols.intern("missing_operation");
    input.functions[0].blocks[0].body.push(ArcInstr::Apply {
        dst: ArcVarId::new(0),
        ty: Idx::UNIT,
        func: missing,
        args: Vec::new(),
        arg_ownership: Vec::new(),
        mono_instance_id: None,
    });

    let error = ExecutableProgram::validate(input)
        .err()
        .map(|error| error.to_string())
        .unwrap_or_default();
    assert!(error.contains("cannot resolve call to 'missing_operation' from 'main'"));
    assert!(error.contains("add a realized body or RuntimeCall mapping"));
}

#[test]
fn rejects_unresolved_operator_fact_before_backend_selection() {
    let symbols = SharedInterner::new();
    let mut input = parts(&symbols);
    input.functions[0].operator_call_facts = vec![OperatorCallFact {
        destination: ArcVarId::new(0),
        receiver: ArcVarId::new(0),
        operation: PrimOp::Binary(BinaryOp::Eq),
        span: Some(Span::DUMMY),
    }];

    let error = ExecutableProgram::validate(input)
        .err()
        .unwrap_or_else(|| panic!("closed executable accepted unresolved operator provenance"));

    assert!(matches!(
        &error,
        RealizationError::InvalidGeneratedCallProvenance { function, details, .. }
            if *function == symbols.intern("main")
                && details.contains("unresolved operator")
    ));
}

#[test]
fn rejects_call_ownership_that_does_not_cover_every_argument() {
    let symbols = SharedInterner::new();
    let mut input = parts(&symbols);
    let callee = symbols.intern("missing_operation");
    input.functions[0].blocks[0].body.push(ArcInstr::Apply {
        dst: ArcVarId::new(0),
        ty: Idx::UNIT,
        func: callee,
        args: vec![ArcVarId::new(0)],
        arg_ownership: Vec::new(),
        mono_instance_id: None,
    });

    let error = ExecutableProgram::validate(input)
        .err()
        .unwrap_or_else(|| panic!("missing post-AIMS call ownership must be rejected"));

    assert!(matches!(
        error,
        RealizationError::CallOwnershipMetadata {
            block: 0,
            position: CallPosition::Instruction(0),
            arguments: 1,
            ownership_entries: 0,
            ..
        }
    ));
}

#[test]
fn resolves_list_set_before_backend_selection() {
    let symbols = SharedInterner::new();
    let mut input = parts(&symbols);
    let set = symbols.intern("set");
    let mut pool = Pool::new();
    let list = pool.list(Idx::BOOL);
    input.functions[0].var_types = vec![list, Idx::INT, Idx::BOOL];
    input.functions[0].var_reprs = vec![ValueRepr::RcPointer, ValueRepr::Scalar, ValueRepr::Scalar];
    input.functions[0].var_rc_strategies = vec![Some(RcStrategy::HeapPointer), None, None];
    input.functions[0].blocks[0].body.push(ArcInstr::Apply {
        dst: ArcVarId::new(0),
        ty: list,
        func: set,
        args: vec![ArcVarId::new(0), ArcVarId::new(1), ArcVarId::new(2)],
        arg_ownership: vec![
            ArgOwnership::Owned,
            ArgOwnership::Borrowed,
            ArgOwnership::Owned,
        ],
        mono_instance_id: None,
    });
    input.functions[0].method_call_facts = vec![MethodCallFact {
        destination: ArcVarId::new(0),
        receiver_type: list,
        form: MethodCallForm::Instance,
        producer: None,
        selected_producer: None,
        derived_position: None,
    }];
    input.pool = pool;
    input.callable_facts = ori_arc::freeze_function_callable_facts(&input.functions, &input.pool);

    let program = ExecutableProgram::validate(input).unwrap_or_else(|error| {
        panic!("list.set should resolve before backend selection: {error}")
    });
    let entry = program
        .cli_entry()
        .unwrap_or_else(|| panic!("fixture must have a CLI entry"));
    let function = program.functions()[entry.index()].name;
    let block = BlockIndex::new(0, function)
        .unwrap_or_else(|error| panic!("test block should be representable: {error}"));
    let position = CallPosition::instruction(0, function)
        .unwrap_or_else(|error| panic!("test instruction should be representable: {error}"));
    assert_eq!(
        program.call_target(CallSite::new(entry, block, position)),
        Some(CallableTarget::Runtime(RuntimeCall::ListSet))
    );
}

#[test]
fn resolves_registered_error_method_before_backend_selection() {
    let symbols = SharedInterner::new();
    let mut input = parts(&symbols);
    let trace_entries = symbols.intern("trace_entries");
    let error_name = symbols.intern("Error");
    let message_name = symbols.intern("message");
    let trace_name = symbols.intern("trace");
    let mut pool = Pool::new();
    let error = pool.named(error_name);
    let trace_list = pool.list(Idx::UNIT);
    let error_struct = pool.struct_type(
        error_name,
        &[(message_name, Idx::STR), (trace_name, trace_list)],
    );
    pool.set_resolution(error, error_struct);
    pool.set_error_struct_idx(error);

    input.functions[0].return_type = trace_list;
    input.functions[0].var_types = vec![error, trace_list];
    input.functions[0].blocks[0].body.push(ArcInstr::Apply {
        dst: ArcVarId::new(1),
        ty: trace_list,
        func: trace_entries,
        args: vec![ArcVarId::new(0)],
        arg_ownership: vec![ArgOwnership::Borrowed],
        mono_instance_id: None,
    });
    input.functions[0].blocks[0].terminator = ArcTerminator::Return {
        value: ArcVarId::new(1),
    };
    input.functions[0].method_call_facts = vec![MethodCallFact {
        destination: ArcVarId::new(1),
        receiver_type: error,
        form: MethodCallForm::Instance,
        producer: None,
        selected_producer: None,
        derived_position: None,
    }];
    populate_metadata(&mut input.functions[0], &pool);
    input.pool = pool;
    input.callable_facts = ori_arc::freeze_function_callable_facts(&input.functions, &input.pool);

    let program = ExecutableProgram::validate(input).unwrap_or_else(|error| {
        panic!("Error.trace_entries should resolve before backend selection: {error}")
    });
    let entry = program
        .cli_entry()
        .unwrap_or_else(|| panic!("fixture must have a CLI entry"));
    let function = program.functions()[entry.index()].name;
    let block = BlockIndex::new(0, function)
        .unwrap_or_else(|error| panic!("test block should be representable: {error}"));
    let position = CallPosition::instruction(0, function)
        .unwrap_or_else(|error| panic!("test instruction should be representable: {error}"));
    let target = program
        .call_target(CallSite::new(entry, block, position))
        .unwrap_or_else(|| panic!("Error.trace_entries must have a callable target"));
    let CallableTarget::Runtime(RuntimeCall::RegistryMethod(method)) = target else {
        panic!("Error.trace_entries must retain its registry method identity, got {target:?}");
    };
    assert_eq!(method.receiver(), ori_registry::TypeTag::Error);
}

#[test]
fn receiver_qualified_method_wins_over_same_spelled_free_function() {
    let symbols = SharedInterner::new();
    let mut input = parts(&symbols);
    let trace = symbols.intern("trace");
    let error_name = symbols.intern("Error");
    let message_name = symbols.intern("message");
    let trace_name = symbols.intern("trace_entries");
    let mut pool = Pool::new();
    let error = pool.named(error_name);
    let trace_list = pool.list(Idx::UNIT);
    let error_struct = pool.struct_type(
        error_name,
        &[(message_name, Idx::STR), (trace_name, trace_list)],
    );
    pool.set_resolution(error, error_struct);
    pool.set_error_struct_idx(error);

    input.functions[0].return_type = Idx::BOOL;
    input.functions[0].var_types = vec![error, Idx::BOOL];
    input.functions[0].blocks[0].body.push(ArcInstr::Apply {
        dst: ArcVarId::new(1),
        ty: Idx::BOOL,
        func: trace,
        args: vec![ArcVarId::new(0)],
        arg_ownership: vec![ArgOwnership::Borrowed],
        mono_instance_id: None,
    });
    input.functions[0].blocks[0].terminator = ArcTerminator::Return {
        value: ArcVarId::new(1),
    };
    input.functions[0].method_call_facts = vec![MethodCallFact {
        destination: ArcVarId::new(1),
        receiver_type: error,
        form: MethodCallForm::Instance,
        producer: None,
        selected_producer: None,
        derived_position: None,
    }];
    populate_metadata(&mut input.functions[0], &pool);

    let mut free_trace = empty_function(trace);
    populate_metadata(&mut free_trace, &pool);
    let contract = MemoryContract::conservative(free_trace.params.len());
    input
        .function_effects
        .insert(trace, contract.function_effect_facts(&free_trace));
    input
        .fresh_return_facts
        .insert(trace, contract.fresh_self_allocation_facts());
    input
        .param_disjointness
        .insert(trace, prove_param_disjointness(&[], &pool));
    input.contracts.insert(trace, contract);
    input.functions.push(free_trace);
    input
        .function_families
        .push(FunctionFamilyTopology::new(trace, Vec::new()));
    input.pool = pool;
    input.callable_facts = ori_arc::freeze_function_callable_facts(&input.functions, &input.pool);

    let program = ExecutableProgram::validate(input).unwrap_or_else(|error| {
        panic!("Error.trace should resolve by receiver before free-function spelling: {error}")
    });
    let entry = program
        .cli_entry()
        .unwrap_or_else(|| panic!("fixture must have a CLI entry"));
    let function = program.functions()[entry.index()].name;
    let block = BlockIndex::new(0, function)
        .unwrap_or_else(|error| panic!("test block should be representable: {error}"));
    let position = CallPosition::instruction(0, function)
        .unwrap_or_else(|error| panic!("test instruction should be representable: {error}"));
    let target = program
        .call_target(CallSite::new(entry, block, position))
        .unwrap_or_else(|| panic!("Error.trace must have a callable target"));
    let CallableTarget::Runtime(RuntimeCall::RegistryMethod(method)) = target else {
        panic!("Error.trace must not bind the same-spelled free function, got {target:?}");
    };
    assert_eq!(method.receiver(), ori_registry::TypeTag::Error);
}

#[test]
fn producerless_method_fact_accepts_an_exact_closed_function_target() {
    let symbols = SharedInterner::new();
    let mut input = parts(&symbols);
    let exact_target = symbols.intern("debug$m$Pair_int_str$im$");
    input.functions[0].blocks[0].body.push(ArcInstr::Apply {
        dst: ArcVarId::new(0),
        ty: Idx::UNIT,
        func: exact_target,
        args: vec![ArcVarId::new(0)],
        arg_ownership: vec![ArgOwnership::Borrowed],
        mono_instance_id: None,
    });
    input.functions[0].method_call_facts = vec![MethodCallFact {
        destination: ArcVarId::new(0),
        receiver_type: Idx::UNIT,
        form: MethodCallForm::Instance,
        producer: None,
        selected_producer: None,
        derived_position: None,
    }];

    let mut exact_function = empty_function(exact_target);
    exact_function.params.push(ArcParam {
        var: ArcVarId::new(0),
        ty: Idx::UNIT,
        ownership: Ownership::Borrowed,
    });
    let contract = MemoryContract::conservative(exact_function.params.len());
    input.function_effects.insert(
        exact_target,
        contract.function_effect_facts(&exact_function),
    );
    input
        .fresh_return_facts
        .insert(exact_target, contract.fresh_self_allocation_facts());
    input.param_disjointness.insert(
        exact_target,
        prove_param_disjointness(&[Idx::UNIT], &input.pool),
    );
    input.contracts.insert(exact_target, contract);
    input.functions.push(exact_function);
    input
        .function_families
        .push(FunctionFamilyTopology::new(exact_target, Vec::new()));
    input.callable_facts = ori_arc::freeze_function_callable_facts(&input.functions, &input.pool);

    let program = ExecutableProgram::validate(input)
        .unwrap_or_else(|error| panic!("exact rewritten method target should validate: {error}"));
    let entry = program
        .cli_entry()
        .unwrap_or_else(|| panic!("fixture must have a CLI entry"));
    let function = program.functions()[entry.index()].name;
    let block = BlockIndex::new(0, function)
        .unwrap_or_else(|error| panic!("test block should be representable: {error}"));
    let position = CallPosition::instruction(0, function)
        .unwrap_or_else(|error| panic!("test instruction should be representable: {error}"));
    let target = program
        .call_target(CallSite::new(entry, block, position))
        .unwrap_or_else(|| panic!("exact rewritten method must have a callable target"));
    let exact_id = program
        .function_id(exact_target)
        .unwrap_or_else(|| panic!("exact target must have a frozen function identity"));
    assert_eq!(target, CallableTarget::Function(exact_id));
}

#[test]
fn resolves_persistent_list_mutations_from_registry_identity() {
    let cases = [
        ("insert", RuntimeCall::ListInsert),
        ("prepend", RuntimeCall::ListPrepend),
        ("push", RuntimeCall::ListPush),
        ("remove", RuntimeCall::ListRemove),
        ("set", RuntimeCall::ListSet),
        ("updated", RuntimeCall::ListSet),
    ];

    for (symbol, expected) in cases {
        assert_eq!(
            RuntimeCall::resolve(symbol, Some(ori_registry::TypeTag::List)),
            Some(expected),
            "List.{symbol}",
        );
        assert_ne!(
            RuntimeCall::resolve(symbol, Some(ori_registry::TypeTag::Set)),
            Some(expected),
            "Set.{symbol} must not inherit List runtime identity",
        );
    }
    let map_updated = RuntimeCall::resolve("updated", Some(ori_registry::TypeTag::Map))
        .unwrap_or_else(|| panic!("Map.updated must retain its registry callable identity"));
    let RuntimeCall::RegistryMethod(method) = map_updated else {
        panic!("Map.updated must not inherit List.set runtime semantics");
    };
    assert_eq!(method.receiver(), ori_registry::TypeTag::Map);
}

#[test]
fn collect_set_remains_fail_closed_without_a_set_representation() {
    let expected = Some(RuntimeCall::Protocol(
        ori_ir::builtin_constants::protocol::ProtocolBuiltin::CollectSet,
    ));
    assert_eq!(
        RuntimeCall::resolve("__collect_set", Some(ori_registry::TypeTag::Set)),
        expected,
    );
    assert_eq!(RuntimeCall::resolve("__collect_set", None), expected);
}

#[test]
fn resolves_string_runtime_surface_before_backend_selection() {
    let cases = [
        ("contains", RuntimeCall::StringContains),
        ("starts_with", RuntimeCall::StringStartsWith),
        ("ends_with", RuntimeCall::StringEndsWith),
        ("concat", RuntimeCall::Concat),
        ("is_empty", RuntimeCall::StringIsEmpty),
        ("trim", RuntimeCall::StringTrim),
        ("to_uppercase", RuntimeCall::StringUppercase),
        ("to_lowercase", RuntimeCall::StringLowercase),
        ("split", RuntimeCall::StringSplit),
    ];

    for (symbol, expected) in cases {
        assert_eq!(
            RuntimeCall::resolve(symbol, Some(ori_registry::TypeTag::Str)),
            Some(expected),
            "string runtime resolution drifted for {symbol}",
        );
        assert_eq!(
            RuntimeCall::resolve(symbol, Some(ori_registry::TypeTag::Int)),
            None,
            "{symbol} must not acquire string semantics from spelling alone",
        );
        assert_eq!(
            RuntimeCall::resolve(symbol, None),
            None,
            "unqualified {symbol} must not acquire string semantics",
        );
    }
}

#[test]
fn iterator_resolution_preserves_the_receiver_source_kind() {
    let cases = [
        (ori_registry::TypeTag::Range, IteratorSource::Range),
        (ori_registry::TypeTag::List, IteratorSource::List),
        (ori_registry::TypeTag::Str, IteratorSource::Str),
        (ori_registry::TypeTag::Map, IteratorSource::Map),
        (ori_registry::TypeTag::Set, IteratorSource::Set),
        (ori_registry::TypeTag::Option, IteratorSource::Option),
    ];

    for (receiver, source) in cases {
        assert_eq!(
            RuntimeCall::resolve("iter", Some(receiver)),
            Some(RuntimeCall::Iter(source)),
        );
    }
    assert_eq!(RuntimeCall::resolve("iter", None), None);
    assert_eq!(
        RuntimeCall::resolve("iter", Some(ori_registry::TypeTag::Int)),
        None,
    );
}

#[test]
fn resolves_len_protocol_aliases_from_receiver_registry_evidence() {
    let receivers = [LengthReceiver::List, LengthReceiver::Str];
    let aliases = ["len", "length"];
    let mut visited = 0;
    for receiver in receivers {
        for alias in aliases {
            let symbols = SharedInterner::new();
            let mut input = parts(&symbols);
            let callee = symbols.intern(alias);
            let mut pool = Pool::new();
            let receiver_type = receiver.type_index(&mut pool);
            input.functions[0].var_types[0] = receiver_type;
            input.functions[0].blocks[0].body.push(ArcInstr::Apply {
                dst: ArcVarId::new(0),
                ty: Idx::INT,
                func: callee,
                args: vec![ArcVarId::new(0)],
                arg_ownership: vec![ArgOwnership::Borrowed],
                mono_instance_id: None,
            });
            input.functions[0].method_call_facts = vec![MethodCallFact {
                destination: ArcVarId::new(0),
                receiver_type,
                form: MethodCallForm::Instance,
                producer: None,
                selected_producer: None,
                derived_position: None,
            }];
            populate_metadata(&mut input.functions[0], &pool);
            input.pool = pool;

            let program = ExecutableProgram::validate(input).unwrap_or_else(|error| {
                panic!("{receiver:?}.{alias} should realize through Len: {error}")
            });
            let entry = program
                .cli_entry()
                .unwrap_or_else(|| panic!("fixture must have a CLI entry"));
            let function = program.functions()[entry.index()].name;
            let block = BlockIndex::new(0, function)
                .unwrap_or_else(|error| panic!("test block should be representable: {error}"));
            let position = CallPosition::instruction(0, function).unwrap_or_else(|error| {
                panic!("test instruction should be representable: {error}")
            });
            assert_eq!(
                program.call_target(CallSite::new(entry, block, position)),
                Some(CallableTarget::Runtime(RuntimeCall::Length))
            );
            visited += 1;
        }
    }
    assert_eq!(visited, receivers.len() * aliases.len());
}

#[derive(Clone, Copy, Debug)]
enum LengthReceiver {
    List,
    Str,
}

impl LengthReceiver {
    fn type_index(self, pool: &mut Pool) -> Idx {
        match self {
            Self::List => pool.list(Idx::INT),
            Self::Str => Idx::STR,
        }
    }
}

fn populate_metadata(function: &mut ArcFunction, pool: &Pool) {
    let classifier = ArcClassifier::new(pool);
    function.var_reprs = ori_arc::compute_var_reprs(function, &classifier, pool);
    function.var_rc_strategies = ori_arc::ir::compute_var_rc_strategies(function, pool);
}

#[test]
fn length_resolution_requires_a_registered_len_receiver() {
    for alias in ["len", "length"] {
        assert_eq!(RuntimeCall::resolve(alias, None), None);
        assert_eq!(
            RuntimeCall::resolve(alias, Some(ori_registry::TypeTag::Int)),
            None
        );
    }
}

#[test]
fn range_len_keeps_receiver_specific_runtime_identity() {
    assert_eq!(
        RuntimeCall::resolve("len", Some(ori_registry::TypeTag::Range)),
        Some(RuntimeCall::RangeLength)
    );
}

#[test]
fn map_and_set_len_aliases_realize_without_claiming_runtime_value_support() {
    let receivers = [ori_registry::TypeTag::Map, ori_registry::TypeTag::Set];
    let aliases = ["len", "length"];
    let mut visited = 0;
    for receiver in receivers {
        for alias in aliases {
            assert_eq!(
                RuntimeCall::resolve(alias, Some(receiver)),
                Some(RuntimeCall::Length)
            );
            visited += 1;
        }
    }
    assert_eq!(visited, receivers.len() * aliases.len());
}

#[test]
fn keeps_list_builder_append_distinct_from_persistent_list_push() {
    assert_eq!(
        RuntimeCall::resolve("ori_list_push", None),
        Some(RuntimeCall::ListBuilderPush),
    );
    assert_eq!(
        RuntimeCall::resolve("push", Some(ori_registry::TypeTag::List)),
        Some(RuntimeCall::ListPush),
    );
}

#[test]
fn resolves_list_builder_unwind_cleanup() {
    assert_eq!(
        RuntimeCall::resolve("ori_list_free", None),
        Some(RuntimeCall::ListFree),
    );
    assert_eq!(RuntimeCall::ListFree.arity(), 2);
}
