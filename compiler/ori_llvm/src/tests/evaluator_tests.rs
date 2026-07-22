//! Tests for `OwnedLLVMEvaluator` and evaluator types.

#![expect(
    clippy::default_trait_access,
    reason = "evaluator fixtures spell concrete default types to identify each LLVM value shape"
)]

use ori_arc::uniqueness::{CowAnnotations, DropHints};
use ori_arc::{
    prove_param_disjointness, ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArcValue,
    ArcVarId, LitValue, MemoryContract, ValueRepr, VariableMetadataState,
};
use ori_ir::{SharedInterner, StringInterner};
use ori_repr::executable::{ExecutableProgram, ExecutableProgramParts, EXECUTABLE_PROGRAM_VERSION};
use ori_repr::{NarrowingPolicy, ReprPlan};
use ori_types::{Idx, Pool, TypeRegistry};

use crate::evaluator::{LLVMEvalError, LLVMValue, OwnedLLVMEvaluator};

#[test]
fn test_llvm_value_debug() {
    let void = LLVMValue::Void;
    let int = LLVMValue::Int(42);
    let float = LLVMValue::Float(3.5);
    let bool_val = LLVMValue::Bool(true);

    assert_eq!(format!("{void:?}"), "Void");
    assert_eq!(format!("{int:?}"), "Int(42)");
    assert_eq!(format!("{float:?}"), "Float(3.5)");
    assert_eq!(format!("{bool_val:?}"), "Bool(true)");
}

#[test]
fn test_llvm_value_equality() {
    assert_eq!(LLVMValue::Void, LLVMValue::Void);
    assert_eq!(LLVMValue::Int(42), LLVMValue::Int(42));
    assert_ne!(LLVMValue::Int(42), LLVMValue::Int(43));
    assert_eq!(LLVMValue::Float(3.5), LLVMValue::Float(3.5));
    assert_eq!(LLVMValue::Bool(true), LLVMValue::Bool(true));
    assert_ne!(LLVMValue::Bool(true), LLVMValue::Bool(false));
}

#[test]
fn test_llvm_value_clone() {
    let original = LLVMValue::Int(42);
    let cloned = original.clone();
    assert_eq!(original, cloned);
}

#[test]
fn test_llvm_eval_error_new() {
    let error = LLVMEvalError::new("test error");
    assert_eq!(error.message, "test error");
}

#[test]
fn test_llvm_eval_error_display() {
    let error = LLVMEvalError::new("display test");
    assert_eq!(format!("{error}"), "display test");
}

#[test]
fn test_llvm_eval_error_from_string() {
    let error = LLVMEvalError::new(String::from("from string"));
    assert_eq!(error.message, "from string");
}

// `test_compile_module_with_tests_empty` exercises evaluator construction and
// drop while asserting the compiled result.

fn empty_executable(symbols: &SharedInterner) -> ExecutableProgram {
    let main = symbols.intern("artifact_root");
    let function = ArcFunction {
        name: main,
        params: Vec::new(),
        return_type: Idx::UNIT,
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![ArcInstr::Let {
                dst: ArcVarId::new(0),
                ty: Idx::UNIT,
                value: ArcValue::Literal(LitValue::Unit),
            }],
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
        primitive_facts: ori_arc::ir::PrimitiveFacts::default(),
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
    };
    let pool = Pool::new();
    let contract = MemoryContract::conservative(0);
    let functions = vec![function.clone()];
    let param_disjointness = prove_param_disjointness(&[], &pool);
    let callable_facts = ori_arc::freeze_function_callable_facts(&functions, &pool);
    ExecutableProgram::validate(ExecutableProgramParts {
        version: EXECUTABLE_PROGRAM_VERSION,
        symbols: symbols.clone(),
        pool,
        functions: functions.clone(),
        function_families: vec![ori_repr::executable::FunctionFamilyTopology::new(
            main,
            Vec::new(),
        )],
        contracts: [(main, contract.clone())].into_iter().collect(),
        function_effects: [(main, contract.function_effect_facts(&function))]
            .into_iter()
            .collect(),
        fresh_return_facts: [(main, contract.fresh_self_allocation_facts())]
            .into_iter()
            .collect(),
        param_disjointness: [(main, param_disjointness)].into_iter().collect(),
        callable_facts,
        closure_adapters: Default::default(),
        retain_plans: Default::default(),
        roots: vec![main],
        cli_entry: None,
        externals: ori_repr::executable::ValidatedExternalCallables::empty(),
        method_targets: Default::default(),
        user_drop_bindings: Vec::new(),
        repr_plan: ReprPlan::new(NarrowingPolicy::Disabled),
        type_registry: TypeRegistry::new(),
    })
    .unwrap_or_else(|error| panic!("fixture executable should validate: {error}"))
}

#[test]
fn test_compile_module_with_tests_empty() {
    let symbols = SharedInterner::new();
    let interner = &*symbols;
    let executable = empty_executable(&symbols);
    let evaluator = OwnedLLVMEvaluator::new();

    let module = ori_ir::ast::Module {
        file_attr: None,
        imports: vec![],
        consts: vec![],
        functions: vec![],
        tests: vec![],
        types: vec![],
        traits: vec![],
        impls: vec![],
        extends: vec![],
        def_impls: vec![],
        extension_imports: vec![],
        extern_blocks: vec![],
    };

    let canon = ori_ir::canon::CanonResult::empty();
    let result = evaluator.compile_module_with_tests(
        &module,
        &[],
        &canon,
        interner,
        &[],
        &[],
        &[],
        &[],
        &[],
        &executable,
        &[],
    );

    assert!(
        result.is_ok(),
        "empty module should compile: {}",
        result.err().map(|e| e.message).unwrap_or_default()
    );
}

/// Regression: ordinal-qualified names are registered as unconstrained.
///
/// When two trait impls on the same type share a method name (e.g., two `Index`
/// impls with `@index`), `collect_unconstrained_fn_names()` must register both
/// the base name (`__impl_42_index`) and ordinal-suffixed names
/// (`__impl_42_index_1`).
#[test]
fn collect_unconstrained_fn_names_registers_ordinal_variants() {
    let interner = StringInterner::new();

    // Simulate type checker output: same type (Idx 42) defines `index` twice
    // (e.g., `impl Index<int>` and `impl Index<str>` on the same type).
    let self_type = ori_types::Idx::from_raw(42);
    let index_name = interner.intern("index");
    let trait_impl_fn_names = vec![(self_type, index_name), (self_type, index_name)];

    let result =
        ori_repr::collect_unconstrained_fn_names(&[], &trait_impl_fn_names, Some(&interner));

    let base_qualified = interner.intern("__impl_42_index");
    let ordinal_qualified = interner.intern("__impl_42_index_1");

    assert!(
        result.contains(&(None, base_qualified)),
        "Base qualified name __impl_42_index must be registered"
    );
    assert!(
        result.contains(&(None, ordinal_qualified)),
        "Ordinal-qualified name __impl_42_index_1 must be registered"
    );

    // Negative: ordinal 0 must NOT produce a suffixed name
    let wrong_base = interner.intern("__impl_42_index_0");
    assert!(
        !result.contains(&(None, wrong_base)),
        "Ordinal 0 must use the unsuffixed base name, not __impl_42_index_0"
    );
}
