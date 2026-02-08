# LLVM V2 Index

> **Maintenance Notice:** Update this index when adding/modifying sections.

## How to Use
1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

---

## Keyword Clusters by Section

### Section 01: TypeInfo Enum & Core Type Implementations
**File:** `section-01-type-info.md` | **Status:** Complete

```
TypeInfo, type lowering, LLVM type, type representation
TypeInfo enum, TypeInfoStore, enum dispatch, static dispatch
layout, size, alignment, stride, ABI size
trivial, copyable, reference-counted, scalar
retain, release, copy, destroy, emit_retain, emit_release
inkwell, BasicTypeEnum, StructType, IntType, FloatType
type store, TypeInfoStore, indexed storage, no dyn Trait, Pool only
ArcClassification, ori_arc, no LLVM dependency
Channel, Function, function pointer, closure pointer
unit i64, never i64, void not BasicTypeEnum
Roc-style RC, 8-byte header, strong_count at ptr minus 8, heap layout, C FFI
Idx::NONE guard, unreachable tags, Var, BoundVar, Scheme, Infer
newtype transparent, alias resolved, no Newtype variant, no Alias variant
Pool flattening, struct field data, enum variant data, prerequisite refactor
Swift TypeInfo, HeapTypeInfo, LoadableTypeInfo, FixedTypeInfo
Roc basic_type_from_layout, LayoutRepr
Zig lowerType, TypeMap
```

### Section 02: IrBuilder & ID-Based Value System
**File:** `section-02-ir-builder.md` | **Status:** Complete

```
IrBuilder, builder, instruction emission, inkwell wrapper
RAII, lifetime, BasicBlock, position_at_end
ID-based, ValueId, TypeId, BlockId, FunctionId
context scoping, ValueId per-context, no cross-context IDs
alloca, load, store, gep, call, ret, br, cond_br, select
create_entry_alloca, struct_gep, mem2reg, entry block alloca
udiv, urem, lshr, fneg, unsigned ops, float negation
uitofp, fptoui, ptr_to_int, int_to_ptr, conversions
build_global_string_ptr, string constant
save_position, BuilderPositionGuard, RAII position
debug_assert, type checking, ValueId type safety
phi node, phi_from_incoming, merge block
BuilderExt, Roc pattern, unwrap Result
Rust GenericBuilder, two-context (SimpleCx, FullCx)
Zig Builder.zig, pure IR before LLVM
scope, locals, variable binding, alloca management
ScopeBinding, Immutable, Mutable, im crate, persistent HashMap
```

### Section 03: Expression Lowering Modules
**File:** `section-03-expr-lowering.md` | **Status:** Complete

```
expression lowering, compile_expr, ExprKind dispatch
exhaustive match, no catch-all, compiler error on new variant
ExprLowerer, lower(), dispatcher, central dispatch
ExprId::INVALID, sentinel, absent optional, is_valid()
lower_literals, int, float, string, bool, char, unit, duration, size
TemplateFull, TemplateLiteral, template, string interpolation
FunctionRef, HashLength, SelfRef, Const, Ident
lower_operators, binary_op, unary_op, Cast, type cast, as, as?
FloorDiv, floor division, div keyword, negative correction
Range, RangeInclusive, binary op desugar
short-circuit, And, Or, Coalesce, conditional branch, phi merge
Option tag semantics, Result tag semantics, inverted tags
lower_control_flow, if, match, loop, for, break, continue, assign
LoopContext, exit_block, continue_block, break_values
StmtKind, block lowering, sequential statements, Let binding
For guard, is_yield, list comprehension, filter
lower_calls, call, CallNamed, MethodCall, MethodCallNamed, lambda
sret, Section 04, calling convention, large-struct return
lower_collections, list, map, tuple, struct, range, field, index
ListWithSpread, MapWithSpread, StructWithSpread, spread
lower_error_handling, Ok, Err, Some, None, Try, tagged union
lower_constructs, FunctionSeq, FunctionExp, Await, WithCapability
ScopeBinding, Immutable, Mutable, alloca, SSA value
modular codegen, focused modules, single responsibility
Gleam struct-per-backend, Go pass pipeline
```

### Section 04: Function Declaration & Calling Conventions
**File:** `section-04-functions-abi.md` | **Status:** Complete (04.1–04.7 done)

```
function declaration, function definition, FunctionSig, ParamSig, ReturnSig
calling convention, sret, by-value, by-reference, byval
ABI, parameter passing, return value, stack vs register
declare phase, define phase, two-pass compilation, batch declare-then-define
declare_all, define_all, FunctionCompiler, forward references
compile_function_with_sig, per-function declare+define, current model
declare_runtime_functions, runtime pre-declaration, ori_* functions
ParamPassing, Direct, Indirect, Void, Reference
ReturnPassing, Sret, alignment, TypeInfo-driven threshold
needs_sret, >2 fields, >16 bytes, x86-64 SysV ABI
compute_param_passing, compute_return_passing, TypeInfo::size()
CallConv, Fast, C, fastcc, ccc
fastcc internal functions, ccc for @main @panic FFI runtime
tail call optimization, TCO, musttail
Roc FAST_CALL_CONV, argument_type_from_layout
Rust FnAbi, ArgAbi, PassMode, Direct, Indirect, Pair
Swift NativeConventionSchema, Explosion
closure, lambda, fat pointer, env_ptr, fn_ptr
tagged i64, coerce_to_i64, bit 0 tag, ori_closure_box, LAMBDA_COUNTER
capture by value, environment struct, ARC-managed captures, ptr-8 closure env
TypeInfoStore, no TypeInfoRegistry
no-capture optimization, null env_ptr, direct call
__lambda_N, max 8 captures, capture count, boxed closure
hidden first parameter, env_ptr as hidden param
mangling, symbol name, Mangler, demangling
_ori_ prefix, $ module separator, $$ trait separator, $A$ associated
mangle_function, mangle_trait_impl, mangle_extension, mangle_associated_function
JIT unmangled, AOT mangled, V2 all paths mangled
method dispatch, builtin methods, receiver type, unmangled lookup
method name collision, mangled method names, _ori_<module>$<type>$<method>
entry point, @main, @panic, C main wrapper, ori_args_from_argv
@main () -> void, @main () -> int, @main (args: [str]) -> void
@panic handler registration, ori_user_panic_handler
test wrapper, _ori_test_ prefix, void signature
```

### Section 05: Type Classification for ARC
**File:** `section-05-type-classification.md` | **Status:** Complete

```
type classification, scalar, reference, ARC, RC
isScalar, isPossibleRef, isDefiniteRef
monomorphized classification, concrete types only, post-substitution
trivial type, no RC needed, skip retain/release
option[int] Scalar, option[str] DefiniteRef, monomorphized examples
Channel DefiniteRef, Function DefiniteRef
PossibleRef only for unresolved type variables
classify_compound, transitive classification
Lean 4 IRType, isScalar, isObj, isPossibleRef
Koka ValueRepr, scan fields, raw size
Swift ownership lattice, None, Owned, Guaranteed
type flags, TypeFlags, refcounted bit
Pool integration, Idx properties, type queries, ori_arc depends on ori_types
```

### Section 06: ARC IR & Borrow Inference
**File:** `section-06-borrow-inference.md` | **Status:** Complete (06.0-06.3 all complete, including LLVM wiring)

```
ARC IR, basic blocks, explicit control flow, terminators
ArcFunction, ArcBlock, ArcInstr, ArcTerminator
ArcVarId, ArcBlockId, ArcParam, ArcValue, CtorKind
block parameters, join semantics, phi-like
lowering, typed AST to ARC IR, expression flattening
Apply, ApplyIndirect, indirect call, closure call
PartialApply, partial application, closure creation
Invoke, invoke terminator, panic cleanup, unwind destination, Resume
Reset, Reuse, intermediate ARC IR, expanded by Section 09
IsShared, Set, field mutation, refcount test
borrow inference, borrow analysis, owned vs borrowed
parameter ownership, escape analysis, consumption
projection ownership, bidirectional propagation, use-after-free
tail call preservation, preserveTailCall, TCO borrow promotion
iterative refinement, fixed point, monotonic Borrowed to Owned
closure capture, env struct, RC as unit, drop closure
Lean 4 Borrow.lean, LCNF, inferParamInfo, updateParamOwnership
Koka Borrowed, ParamInfo, borrowedExtend, Core IR
Swift OwnershipKind, Guaranteed, Unowned, SIL
function signature annotation, borrow bit
no RC for borrowed parameters, eliminate inc/dec
ArcIrBuilder, builder API, AST-to-ARC-IR lowering
var_types, var_type(), spans side table, Span preservation
PrimOp, LitValue, TBD during implementation
infer_borrows, apply_borrows, initialize_all_borrowed, update_ownership
ArcClassifier, is_scalar, needs_rc, scalar skipped
mark_owned, try_mark_param_owned, param_index, is_owned_var
check_tail_call, tail position detection, last Apply in block
unknown callee, external function, conservative all args Owned
```

### Section 07: RC Insertion via Liveness
**File:** `section-07-rc-insertion.md` | **Status:** Complete (07.1-07.6 all done)

```
RC insertion, reference counting, inc, dec, drop
liveness analysis on ARC IR, live variables, dead variables
backward dataflow, basic block liveness, BlockLiveness
gen set, kill set, upward-exposed uses, forward scan
postorder traversal, CFG edges, not Vec storage order
block parameters as definitions, parameter substitution
ArcInstr::RcInc, ArcInstr::RcDec, in-place mutation
backward pass ordering, Dec instruction Inc, push then reverse
Perceus, precise RC, last use detection
derived value, borrows set, projection borrow optimization
specialized drop functions, compile-time drop, per-type drop
DropKind, DropInfo, drop descriptor, compute_drop_info, collect_drop_infos
DropKind::Trivial, DropKind::Fields, DropKind::Enum, DropKind::Collection, DropKind::Map
drop_MyStruct, drop_List_Str, _ori_drop$ naming
DropKind::ClosureEnv, compute_closure_env_drop, capture types
closure env drop, Dec each capture, env struct RC
ori_rt redesign, 8-byte header, ptr-8 strong_count
ori_rc_alloc, ori_rc_inc, ori_rc_dec, ori_rc_free, drop_fn
early exit cleanup, edge cleanup, edge gap, cross-block Dec
insert_edge_cleanup, compute_predecessors, redirect_edges
trampoline block, edge splitting, multi-predecessor gap
stranded variable, live_out minus live_in, global borrows
panic cleanup, full cleanup blocks, Invoke terminator
invoke vs call, landing pad, __ori_personality, resume
reset/reuse detection, dec+Construct pattern, constructor-identity
ArcInstr::Reset, ArcInstr::Reuse, intermediate operations
same-type reuse, expanded by Section 09
Koka Parc.hs, backward traversal, dup/drop
Lean 4 RC.lean, addInc, addDec, VarInfo, LiveVars.borrows
Roc ModifyRc, Inc, Dec, Free, per-layout refcount helpers
```

### Section 08: RC Elimination via Dataflow
**File:** `section-08-rc-elimination.md` | **Status:** Complete (08.1–08.3 done)

```
RC elimination, retain/release pairing, optimization
dataflow analysis, top-down, bottom-up, bidirectional
ArcVarId, InstrPos, ArcBlockId, ARC IR types
RcStateMap, per-variable RC state, ArcFunction
lattice state, Decremented, MightBeUsed, MightBeDecremented
TopDownRcState, Incremented, MightBeDecremented, aliased Dec
Swift ARCSequenceOpts, ARCBBState, ARCMatchingSet
GlobalARCSequenceDataflow, BottomUpRefCountState
RC identity, same ArcVarId, alias tracking
loop handling, conservative at boundaries
dead RC operations, redundant retain/release
EliminationCandidate, InstrPos, remove_instr
pipeline order: runs AFTER Section 09, input from both 07 and 09
eliminate_rc_ops, rc_elim.rs, bidirectional intra-block
TopDownState, BottomUpState, EliminationCandidate
cascading elimination, fixed-point iteration
```

### Section 09: Constructor Reuse (FBIP)
**File:** `section-09-constructor-reuse.md` | **Status:** Complete (09.1-09.5 all implemented in expand_reuse.rs)

```
constructor reuse, FBIP, functional but in-place
reset, reuse, memory reuse, allocation avoidance
two-path expansion, isShared, fast path, slow path
fast path in-place mutation, slow path fresh allocation
uniqueness test, refcount > 1, shared vs unique
then branch slow path, else branch fast path, fall-through common case
projection-increment erasure, eraseProjIncFor, claimed fields bitmask
backward scan, Project/RcInc pattern, erased increments
self-set elimination, removeSelfSet, no-op write detection
constructor-identity rule, same-type reuse, not size-based
reuse-eligible patterns, match arm reconstruct, spread struct
recursive data transformation, list map, tree map
ArcInstr::Reset, ArcInstr::Reuse, intermediate IR operations
ArcInstr::IsShared, ArcInstr::Set, expanded operations
IsShared inline, load ptr-8, icmp sgt 1, not a runtime call
Lean 4 ExpandResetReuse, reset/reuse expansion algorithm
Koka ParcReuse, genAllocAt, genReuseAddress, size-based pool (not adopted)
Roc HelperOp Reset, ResetRef, Reuse
pipeline order: runs BEFORE Section 08, after Section 07
```

### Section 10: Pattern Match Decision Trees
**File:** `section-10-decision-trees.md` | **Status:** Not Started

```
decision tree, pattern compilation, match lowering
AST to ARC IR lowering, ori_arc, not ori_llvm
Maranget algorithm, pattern matrix, column selection heuristic
switch instruction, tag dispatch, nested patterns
Switch terminator, Branch terminator, LLVM switch, LLVM br
scrutinee path tracking, PathInstruction, ScrutineePath
TagPayload, TupleIndex, StructField, ListElement
TestKind, EnumTag, IntEq, StrEq, BoolEq, FloatEq, IntRange, ListLen
TestValue, Tag variant_index variant_name, Int, Str, Bool, Float
tag type from TypeInfo, not hardcoded i8, i8 i16 i32
exhaustiveness in ori_types, Fail node, LLVM unreachable
guard fall-through, next compatible arm, on_fail chain
Guard node, guard_pass, guard_fail, compatible arms
or-pattern expansion, shared leaf label, single body block
payload extraction, struct_gep, field projection after tag
Roc decision_tree, DecisionTree, Decision, Leaf, Chain
Elm PatternMatches, Maranget implementation
current: sequential if-else, future: decision tree based
```

### Section 11: LLVM Optimization Pass Configuration
**File:** `section-11-llvm-passes.md` | **Status:** Not Started

```
optimization passes, LLVM pass manager, opt level, new pass manager
O0, O1, O2, O3, Os, Oz, debug, release
Debug, Release, ReleaseFast, ReleaseSmall, ReleaseMinSize, profile presets
Zig-style profiles, --release, --opt=, CLI mapping
OptimizationConfig, OptimizationLevel, OptimizationError
PassBuilderOptionsGuard, RAII, LLVMRunPasses, pipeline string
builder pattern, loop vectorization, SLP vectorization, loop unrolling
loop interleaving, function merging, inliner threshold, verify_each
LTO, thin LTO, full LTO, LtoMode, link-time optimization
thinlto-pre-link, lto-pre-link, prelink_pipeline_string, lto_pipeline_string
is_lto_phase, pre-link pipeline, merge bitcode, LTO gap, never set to true
run_optimization_passes, run_custom_pipeline, extra_passes
module verification, verify_module, unconditional, AOT verification bug
JIT verifies, AOT skips, LLVM segfault, CodegenError::VerificationFailed
optimize_module, verify before optimize, mandatory pre-verification
ARC safety, LLVM attributes, RC runtime functions, opaque calls
ori_rc_inc, ori_rc_dec, ori_rc_alloc, ori_rc_free, drop_fn
nounwind, argmemonly, memory(argmem: readwrite), NOT readonly, NOT readnone
noalias return, fresh allocation, alias analysis
DSE dead store elimination, LICM loop-invariant code motion, GVN
Swift strategy, SIL level ARC, LLVM sees opaque runtime calls
specialized drop functions, _ori_drop$, noinline, cold path
per-function attributes, declaration time, not pass time
fastcc, ccc, calling convention, Section 04 integration
noinline drop, cold panic, sret noalias, alwaysinline future
ModuleEmitter, verify-optimize-emit pipeline, OptPipeline
compile_to_llvm, verify_module, optimize, emit_object
run_optimization_passes internal, ModuleEmitter::emit
--emit=llvm-ir, already exists, ObjectEmitter::emit_llvm_ir
Rust ModuleConfig, pass pipeline, rustc_codegen_ssa
Zig opt_level, CodeGenOptLevel, target machine
Swift IRGenModule, ARC runtime attributes
```

### Section 12: Incremental & Parallel Codegen
**File:** `section-12-incremental-parallel.md` | **Status:** Not Started

```
incremental compilation, caching, content hash, function-level
codegen unit, CGU, partitioning, parallel, per-function compilation
FunctionContentHash, body_hash, signature_hash, callees_hash, globals_hash
FunctionDeps, FunctionDependencyGraph, signature-aware invalidation
signature change recompiles callers, body change does NOT recompile callers
two-layer cache, ARC IR cache, object code cache, Layer 1, Layer 2
ArcIrCacheKey, CachedArcIr, ObjectCacheKey, bincode serialization
Serialize, Deserialize, serde, ArcFunction, ArcBlock, ArcInstr
cache directory, functions/, arc_ir/, objects/, deps.json
Salsa hybrid, front-end Salsa, back-end ArtifactCache
codegen NOT Salsa query, LLVM types not Clone/Eq/Hash
Salsa early cutoff, tokens, parsed, typed, TypeCheckResult
SourceFile, #[salsa::input], set_text, file_cache, CompilerDb
Durability::HIGH, build configuration, stable inputs
std::thread, no rayon, dependency-respecting parallel
execute_parallel, CompilationPlan, SharedPlanState, Condvar
compile_parallel discrepancy, round-robin vs dependency-respecting
one LLVM Context per thread, no cross-context ValueId
ARC IR computed once, distributed to threads, read-only
ArtifactCache thread-safe, atomic file operations
two-level scheduling, module topological order, function parallelism
cross-module references, link time, mangled names, Section 04.5
incremental linking, object file replacement, ld -r, partial linking
fallback file-level, module-level initialization, dependency cycles
Zig updateFunc, in-place function patching, per-function .o file
Rust CguReuse, PreLto, PostLto, work product fingerprinting
Lean 4 RC.lean, serializable ARC IR
existing: aot/incremental/, hash.rs, cache.rs, deps.rs, parallel.rs
existing: multi_file.rs, build_dependency_graph, derive_module_name
SourceHasher, ContentHash, CacheKey, CacheConfig, DependencyGraph
DependencyTracker, CompilationPlan, ParallelCompiler, WorkItem
compile_module_functions, extract_function_hashes, FunctionCache
```

### Section 13: Debug Info Generation
**File:** `section-13-debug-info.md` | **Status:** Not Started

```
debug info, DWARF, DWARF 4, DWARF 5, CodeView, source maps, debuginfo
DIBuilder, DICompileUnit, DIFile, DIScope, DILocation, DISubprogram
DILocalVariable, DIType, DIBasicType, DICompositeType, DILexicalBlock
llvm.dbg.declare, llvm.dbg.value, alloca debug, SSA debug
create_auto_variable, create_parameter_variable, parameter debug info
DebugInfoBuilder, DebugInfoConfig, DebugLevel, DebugFormat, DebugContext
LineMap, offset_to_line_col, span-to-location, binary search
TypeCache, primitive deduplication, composite type cache
DebugInfoError, #[cold] factory, BasicTypeCreation
debug_type(), TypeInfo debug dispatch, per-variant debug type
set_location, set_location_from_offset, clear_location
push_scope, pop_scope, current_scope, scope stack
ScopeBinding, Immutable dbg.value, Mutable dbg.declare
RC heap layout, RC<T>, refcount debug, raw layout, LLDB formatters
ARC IR span preservation, synthetic instruction span inheritance
DWARFSourceLanguage::C, custom language ID future
split debug info, dSYM, .dwo, PDB
pipeline wiring, CodegenCx debug_context, Option<DebugContext>
DISubprogram attachment, function declaration debug info
expression lowering source location, per-expression set_location
variable binding DILocalVariable, Let debug info
module finalization, finalize(), forward reference resolution
existing: aot/debug.rs, 1265 lines, well-structured
Rust rustc_codegen_llvm/debuginfo, source_loc, variable descriptors
Zig getDebugFile, lowerDebugType
Swift IRGenDebugInfo, SIL-to-LLVM debug mapping
```

### Section 14: Codegen Test Harness
**File:** `section-14-test-harness.md` | **Status:** Not Started

```
codegen testing, test harness, three-level testing
unit tests, per-module tests, lowering module tests
FileCheck, LLVM FileCheck, IR verification, pattern matching
CHECK, CHECK-NEXT, CHECK-NOT, named capture, register normalization
filecheck_test, compile_to_llvm_ir, write_temp_file
IR patterns, function declaration IR, if/else IR, match IR, RC ops IR
JIT testing, TestCodegen, setup_test!, jit_execute_i64, jit_execute_bool
AOT execution tests, compile-run-assert, exit code, stdout
ASAN, AddressSanitizer, Valgrind, memory safety, leak detection
use-after-free, double-free, no leaks
ori_arc tests, ARC IR lowering tests, CFG structure
borrow inference tests, borrowed vs owned, parameter classification
RC insertion tests, RcInc placement, RcDec placement, liveness
RC elimination tests, paired retain/release removal
constructor reuse tests, Reset, Reuse, eligible patterns
decision tree tests, Switch structure, pattern compilation
@test annotation, test function compilation, _ori_test_ prefix
test runner binary, test discovery, TestDescriptor
JIT vs AOT test execution, ori test, ori test --aot
--only-attached filtering, attached tests, target function
test runner generation, synthetic main, ori_test_summary
existing: ori_llvm/src/tests/ 17 files 6836 lines
existing: oric/tests/phases/codegen/ 17 files 5451 lines
existing: oric/src/testing/ harness.rs mocks.rs
Rust rustc_codegen_llvm tests, FileCheck IR tests
Zig test/behavior, test/compile_errors
Roc test_mono, gen_llvm execution tests
```

### Section 15: Diagnostics & Error Reporting
**File:** `section-15-diagnostics.md` | **Status:** Not Started

```
codegen errors, diagnostic, error reporting, error codes
E4xxx, ori_arc errors, ARC IR errors
E4001 ArcIrLoweringFailure, E4002 BorrowInferenceFailure
E4003 RcInsertionError, E4004 TypeClassificationFailure
E4005 DecisionTreeFailure
E5xxx, ori_llvm errors, LLVM backend errors
E5001 ModuleVerificationFailed, E5002 PassPipelineError
E5003 ObjectEmissionFailed, E5004 TargetNotSupported
E5005 RuntimeLibraryNotFound, E5006 LinkerFailed
E5007 DebugInfoCreationFailed
ArcProblem, LlvmProblem, CodegenProblem
Problem enum, Codegen variant, is_codegen
HasSpan, Option<Span>, Span::DUMMY, span-less errors
Render trait, rendering, Problem/Reporting 1:1 coupling
codegen.rs problem, codegen.rs reporting
impl_has_span!, impl_from_problem!, impl_problem_predicates!
error accumulation, Vec<CodegenProblem>, error pipeline
recoverable, non-recoverable, semi-recoverable
module verification recovery, skip failed module
debug info fallback, degrade gracefully
optimization fallback, lower optimization level
LLVM fatal error handler, install_fatal_error_handler, E9001
linker failure, stderr, exit code, command
imperative suggestions, verb phrase fixes, Ori diagnostic guidelines
CodegenResult, object_files, problems
existing: ori_diagnostic ErrorCode, E0-E3 E9 ranges
existing: oric/src/problem/ ParseProblem SemanticProblem LexProblem
existing: oric/src/reporting/ Render trait Report struct
Rust rustc_codegen_ssa errors, derive Diagnostic
Zig Compilation error, CodegenFail
Gleam compiler-core error, structured error types
```

---

## Quick Reference

| ID | Title | File | Tier |
|----|-------|------|------|
| 01 | TypeInfo Enum & Core Types | `section-01-type-info.md` | 1 |
| 02 | IrBuilder & ID-Based Values | `section-02-ir-builder.md` | 1 |
| 03 | Expression Lowering Modules | `section-03-expr-lowering.md` | 1 |
| 04 | Function Declaration & ABI (Done) | `section-04-functions-abi.md` | 1 |
| 05 | Type Classification for ARC | `section-05-type-classification.md` | 2 |
| 06 | ARC IR & Borrow Inference | `section-06-borrow-inference.md` | 2 |
| 07 | RC Insertion via Liveness (Complete) | `section-07-rc-insertion.md` | 2 |
| 09 | Constructor Reuse (FBIP) (Complete) | `section-09-constructor-reuse.md` | 2 |
| 08 | RC Elimination via Dataflow (Complete) | `section-08-rc-elimination.md` | 2 |
| 10 | Pattern Match Decision Trees | `section-10-decision-trees.md` | 3 |
| 11 | LLVM Optimization Passes | `section-11-llvm-passes.md` | 3 |
| 12 | Incremental & Parallel Codegen | `section-12-incremental-parallel.md` | 3 |
| 13 | Debug Info Generation | `section-13-debug-info.md` | 4 |
| 14 | Codegen Test Harness | `section-14-test-harness.md` | 4 |
| 15 | Diagnostics & Error Reporting | `section-15-diagnostics.md` | 4 |
