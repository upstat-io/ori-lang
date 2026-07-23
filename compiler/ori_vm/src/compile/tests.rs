//! Compile, verification, and execution tests for the VM bytecode projection.

mod closure_programs;
mod entry_closure_cases;
mod rc_programs;
mod rc_semantics_cases;
mod rc_values;
mod runtime_dispatch_cases;
mod runtime_programs;

use super::{compile, compile_with_options, CompileOptions};
use crate::bytecode::{
    BytecodeProgram, CallArgument, Op, Register, VmClosureAdapterAction, VmRetainPlanId,
};
use crate::{
    execute_report, verify, CompileError, ExecutionConfig, ExitValue, IndexKind, VerifyError,
};
use closure_programs::*;
use ori_arc::ir::{compute_var_rc_strategies, PrimitiveFacts};
use ori_arc::uniqueness::{CowAnnotations, DropHints};
use ori_arc::{
    prove_param_disjointness, realize_closed_program, ArcBlock, ArcBlockId, ArcClassifier,
    ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, ArgOwnership, BuiltinOwnershipSets,
    CtorKind, FrozenClosureAdapters, LitValue, MemoryContract, Ownership, PrimOp, RcAtomicity,
    RcStrategy, RetainPlanTable,
};
use ori_ir::{BinaryOp, Name, SharedInterner};
use ori_registry::RuntimeOperator;
use ori_repr::executable::{
    validate_external_callables, CallableTarget, CompilerOperation, ExecutableProgram,
    ExecutableProgramParts, ExternalCallable, ExternalUnwind, FunctionFamilyTopology, RuntimeCall,
    EXECUTABLE_PROGRAM_VERSION,
};
use ori_repr::{NarrowingPolicy, ReprPlan};
use ori_types::{Idx, Pool, TypeRegistry};
use rc_programs::*;
use rc_values::*;
use runtime_programs::*;
use rustc_hash::FxHashMap;

struct RcValueFixture {
    body: Vec<ArcInstr>,
    rc_var: ArcVarId,
    var_types: Vec<Idx>,
    method_call_facts: Vec<ori_arc::MethodCallFact>,
}

const ADMITTED_STRATEGIES: [RcStrategy; 5] = [
    RcStrategy::HeapPointer,
    RcStrategy::FatPointer,
    RcStrategy::AggregateFields,
    RcStrategy::InlineEnum,
    RcStrategy::Iterator,
];
