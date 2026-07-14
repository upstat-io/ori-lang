use ori_arc::uniqueness::{CowAnnotations, DropHints};
use ori_arc::{ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArcVarId};
use ori_ir::SharedInterner;
use ori_types::{Idx, Pool, TypeRegistry};

use super::{
    BlockIndex, CallPosition, CallSite, CallableTarget, ExecutableProgram, ExecutableProgramParts,
    RealizationError, RuntimeCall,
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
        var_reprs: Vec::new(),
        var_rc_strategies: Vec::new(),
        spans: Vec::new(),
        is_fbip: false,
        num_captures: 0,
        cow_annotations: CowAnnotations::default(),
        drop_hints: DropHints::default(),
        tail_calls: Vec::new(),
        burden_emitted: Vec::new(),
        reassign_deaths: Vec::new(),
        catch_scoped_checked_ops: Vec::new(),
        class_ledger_emission: false,
    }
}

fn parts(symbols: &SharedInterner) -> ExecutableProgramParts {
    let main = symbols.intern("main");
    ExecutableProgramParts {
        version: super::EXECUTABLE_PROGRAM_VERSION,
        symbols: symbols.clone(),
        pool: Pool::new(),
        functions: vec![empty_function(main)],
        main,
        repr_plan: ReprPlan::new(NarrowingPolicy::Disabled),
        type_registry: TypeRegistry::new(),
    }
}

#[test]
fn closes_valid_main_function() {
    let symbols = SharedInterner::new();
    let program = ExecutableProgram::validate(parts(&symbols));
    assert!(program.is_ok());
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
fn resolves_list_set_before_backend_selection() {
    let symbols = SharedInterner::new();
    let mut input = parts(&symbols);
    let set = symbols.intern("set");
    let mut pool = Pool::new();
    let list = pool.list(Idx::BOOL);
    input.functions[0].var_types = vec![list, Idx::INT, Idx::BOOL];
    input.functions[0].blocks[0].body.push(ArcInstr::Apply {
        dst: ArcVarId::new(0),
        ty: list,
        func: set,
        args: vec![ArcVarId::new(0), ArcVarId::new(1), ArcVarId::new(2)],
        arg_ownership: Vec::new(),
        mono_instance_id: None,
    });
    input.pool = pool;

    let program = ExecutableProgram::validate(input).unwrap_or_else(|error| {
        panic!("list.set should resolve before backend selection: {error}")
    });
    let function = program.functions()[program.main().index()].name;
    let block = BlockIndex::new(0, function)
        .unwrap_or_else(|error| panic!("test block should be representable: {error}"));
    let position = CallPosition::instruction(0, function)
        .unwrap_or_else(|error| panic!("test instruction should be representable: {error}"));
    assert_eq!(
        program.call_target(CallSite::new(program.main(), block, position)),
        Some(CallableTarget::Runtime(RuntimeCall::ListSet))
    );
}

#[test]
fn resolves_string_runtime_surface_before_backend_selection() {
    let cases = [
        ("contains", RuntimeCall::StringContains),
        ("starts_with", RuntimeCall::StringStartsWith),
        ("ends_with", RuntimeCall::StringEndsWith),
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
    }
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
