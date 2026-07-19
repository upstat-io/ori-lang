//! Deterministic dynamic-dispatch profiles for verified bytecode.

use std::ops::Range;

use ori_repr::executable::FunctionId;

use crate::bytecode::{Continuation, Op, OpcodeKind, Pc, VerifiedProgram};

use super::report::ExecutionReport;

/// Dynamic dispatch count for one opcode.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OpcodeCount {
    /// Opcode identity.
    pub opcode: OpcodeKind,
    /// Number of times the opcode was dispatched.
    pub dispatches: u64,
}

/// Dynamic dispatch count for one consecutive opcode pair.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OpcodePairCount {
    /// First dispatched opcode.
    pub first: OpcodeKind,
    /// Immediately following dispatched opcode.
    pub second: OpcodeKind,
    /// Number of observed transitions.
    pub dispatches: u64,
}

/// Dynamic activity within one conservative bytecode region.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegionCount {
    /// Function identity within the bound verified program.
    pub function: ProfileFunctionId,
    /// Inclusive first program counter in the region.
    pub start: ProfilePc,
    /// Exclusive final program counter in the region.
    pub end: ProfilePc,
    /// Number of dispatches at [`Self::start`].
    pub entries: u64,
    /// Total dispatches within the region.
    pub dispatches: u64,
}

/// Function identity scoped to one [`ExecutionProfile`]'s verified program.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProfileFunctionId(usize);

impl ProfileFunctionId {
    /// Return the function's zero-based index in the bound verified program.
    #[must_use]
    pub const fn index(self) -> usize {
        self.0
    }
}

/// Bytecode position scoped to one function in the bound verified program.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProfilePc(usize);

impl ProfilePc {
    /// Return the zero-based bytecode position.
    #[must_use]
    pub const fn index(self) -> usize {
        self.0
    }
}

/// Deterministic dynamic profile from one interpreted session.
#[derive(Clone, Debug)]
pub struct ExecutionProfile<'program> {
    program: &'program VerifiedProgram,
    /// Total number of operations that reached dispatch.
    pub dispatches: u64,
    /// Nonzero opcode totals in stable opcode order.
    pub opcodes: Vec<OpcodeCount>,
    /// Nonzero consecutive dynamic pairs, including control transfers.
    pub all_pairs: Vec<OpcodePairCount>,
    /// Adjacent same-frame pairs reached by linear fallthrough.
    ///
    /// Fusion consumers must retain the second operation as an independent
    /// entry until a separate predecessor proof permits compaction.
    pub linear_fallthrough_pairs: Vec<OpcodePairCount>,
    /// Nonempty conservative regions in function and PC order.
    pub regions: Vec<RegionCount>,
}

impl<'program> ExecutionProfile<'program> {
    /// Return the exact verified program that owns every profile identity.
    #[must_use]
    pub const fn program(&self) -> &'program VerifiedProgram {
        self.program
    }
}

/// Execution report plus dispatch evidence from the same VM session.
#[derive(Debug)]
pub struct ProfiledExecutionReport<'program> {
    /// Result, output, and resource metrics on success or failure.
    pub execution: ExecutionReport,
    /// Dynamic dispatch profile for the executed prefix.
    pub profile: ExecutionProfile<'program>,
}

pub(super) trait DispatchProbe {
    const ENABLED: bool;

    fn dispatched(&mut self, site: DispatchSite);
}

pub(super) struct NoDispatchProbe;

impl DispatchProbe for NoDispatchProbe {
    const ENABLED: bool = false;

    fn dispatched(&mut self, _site: DispatchSite) {}
}

#[derive(Clone, Copy)]
pub(super) struct DispatchSite {
    pub(super) frame: usize,
    pub(super) function: FunctionId,
    pub(super) pc: Pc,
    pub(super) operation: Op,
}

pub(super) struct FrequencyProbe {
    layout: ProfileLayout,
    all_pairs: Box<[u64]>,
    linear_pairs: Box<[u64]>,
    previous: Option<DispatchSite>,
}

impl FrequencyProbe {
    pub(super) fn new(program: &VerifiedProgram) -> Self {
        Self {
            layout: ProfileLayout::from_verified(program),
            all_pairs: vec![0; pair_count()].into_boxed_slice(),
            linear_pairs: vec![0; pair_count()].into_boxed_slice(),
            previous: None,
        }
    }

    pub(super) fn finish(self, program: &VerifiedProgram) -> ExecutionProfile<'_> {
        let mut opcode_totals = [0_u64; OpcodeKind::COUNT];
        let mut regions = Vec::new();
        let mut dispatches = 0_u64;

        for (function_index, function_profile) in self.layout.functions.iter().enumerate() {
            let function = &program.program.functions[function_index];
            for (pc, &count) in function_profile.pc_counts.iter().enumerate() {
                dispatches += count;
                opcode_totals[function.ops[pc].kind().index()] += count;
            }
            append_nonzero_regions(function_index, function_profile, &mut regions);
        }

        ExecutionProfile {
            program,
            dispatches,
            opcodes: opcode_counts(&opcode_totals),
            all_pairs: pair_rows(&self.all_pairs),
            linear_fallthrough_pairs: pair_rows(&self.linear_pairs),
            regions,
        }
    }
}

impl DispatchProbe for FrequencyProbe {
    const ENABLED: bool = true;

    fn dispatched(&mut self, site: DispatchSite) {
        self.layout.functions[site.function.index()].pc_counts[site.pc.index()] += 1;
        if let Some(previous) = self.previous {
            self.all_pairs[pair_index(previous.operation.kind(), site.operation.kind())] += 1;
            if is_linear_fallthrough_pair(previous, site) {
                self.linear_pairs[pair_index(previous.operation.kind(), site.operation.kind())] +=
                    1;
            }
        }
        self.previous = Some(site);
    }
}

struct ProfileLayout {
    functions: Box<[FunctionProfile]>,
}

impl ProfileLayout {
    fn from_verified(program: &VerifiedProgram) -> Self {
        let functions = program
            .program
            .functions
            .iter()
            .map(|function| FunctionProfile {
                pc_counts: vec![0; function.ops.len()].into_boxed_slice(),
                regions: region_ranges(function, &program.program.switches),
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Self { functions }
    }
}

struct FunctionProfile {
    pc_counts: Box<[u64]>,
    regions: Box<[Range<usize>]>,
}

fn region_ranges(
    function: &crate::bytecode::BytecodeFunction,
    switches: &[Box<[(u64, Pc)]>],
) -> Box<[Range<usize>]> {
    if function.ops.is_empty() {
        return Box::default();
    }

    let mut starts = vec![0, function.entry.index()];
    for (pc, &operation) in function.ops.iter().enumerate() {
        match operation {
            Op::Call { normal, unwind, .. } | Op::CallClosure { normal, unwind, .. } => {
                if let Continuation::At(target) = normal {
                    starts.push(target.index());
                }
                if let Some(target) = unwind {
                    starts.push(target.index());
                }
                push_next_start(pc, function.ops.len(), &mut starts);
            }
            Op::Jump { target, .. } => {
                starts.push(target.index());
                push_next_start(pc, function.ops.len(), &mut starts);
            }
            Op::Branch {
                then_pc, else_pc, ..
            } => {
                starts.push(then_pc.index());
                starts.push(else_pc.index());
                push_next_start(pc, function.ops.len(), &mut starts);
            }
            Op::Switch {
                table, default_pc, ..
            } => {
                starts.push(default_pc.index());
                starts.extend(
                    switches[table.index()]
                        .iter()
                        .map(|(_, target)| target.index()),
                );
                push_next_start(pc, function.ops.len(), &mut starts);
            }
            Op::Return { .. } | Op::Resume | Op::Unreachable => {
                push_next_start(pc, function.ops.len(), &mut starts);
            }
            Op::Const { .. }
            | Op::Copy { .. }
            | Op::Binary { .. }
            | Op::IntBinary { .. }
            | Op::StringBinary { .. }
            | Op::RuntimeBinary { .. }
            | Op::Unary { .. }
            | Op::BoolNot { .. }
            | Op::MakeClosure { .. }
            | Op::Construct { .. }
            | Op::Project { .. }
            | Op::RcInc { .. }
            | Op::RcDec { .. }
            | Op::IsShared { .. }
            | Op::Set { .. }
            | Op::SetTag { .. }
            | Op::Select { .. } => {}
        }
    }

    starts.sort_unstable();
    starts.dedup();
    starts
        .iter()
        .copied()
        .zip(
            starts
                .iter()
                .copied()
                .skip(1)
                .chain(std::iter::once(function.ops.len())),
        )
        .map(|(start, end)| start..end)
        .collect::<Vec<_>>()
        .into_boxed_slice()
}

fn push_next_start(pc: usize, operation_count: usize, starts: &mut Vec<usize>) {
    if pc + 1 < operation_count {
        starts.push(pc + 1);
    }
}

fn is_linear_fallthrough_pair(previous: DispatchSite, current: DispatchSite) -> bool {
    previous.frame == current.frame
        && previous.function == current.function
        && previous.pc.index().checked_add(1) == Some(current.pc.index())
        && previous.operation.is_linear_dispatch()
}

fn append_nonzero_regions(
    function_index: usize,
    profile: &FunctionProfile,
    rows: &mut Vec<RegionCount>,
) {
    for region in &profile.regions {
        let dispatches = profile.pc_counts[region.clone()].iter().sum();
        if dispatches != 0 {
            rows.push(RegionCount {
                function: ProfileFunctionId(function_index),
                start: ProfilePc(region.start),
                end: ProfilePc(region.end),
                entries: profile.pc_counts[region.start],
                dispatches,
            });
        }
    }
}

fn opcode_counts(counts: &[u64; OpcodeKind::COUNT]) -> Vec<OpcodeCount> {
    OpcodeKind::ALL
        .iter()
        .copied()
        .zip(counts.iter().copied())
        .filter_map(|(opcode, dispatches)| {
            (dispatches != 0).then_some(OpcodeCount { opcode, dispatches })
        })
        .collect()
}

fn pair_rows(counts: &[u64]) -> Vec<OpcodePairCount> {
    OpcodeKind::ALL
        .iter()
        .copied()
        .flat_map(|first| {
            OpcodeKind::ALL
                .iter()
                .copied()
                .map(move |second| (first, second))
        })
        .filter_map(|(first, second)| {
            let dispatches = counts[pair_index(first, second)];
            (dispatches != 0).then_some(OpcodePairCount {
                first,
                second,
                dispatches,
            })
        })
        .collect()
}

const fn pair_count() -> usize {
    OpcodeKind::COUNT * OpcodeKind::COUNT
}

const fn pair_index(first: OpcodeKind, second: OpcodeKind) -> usize {
    first.index() * OpcodeKind::COUNT + second.index()
}

#[cfg(test)]
mod tests;
