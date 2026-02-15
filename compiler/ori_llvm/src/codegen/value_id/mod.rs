//! Opaque ID newtypes and `ValueArena` for the V2 `IrBuilder`.
//!
//! These IDs decouple callers from inkwell's `'ctx` lifetime. All LLVM
//! values, types, blocks, and functions are stored in a `ValueArena`
//! and referenced by `Copy` ID handles.
//!
//! Each ID is a `u32` index into the corresponding arena `Vec`.
//! A `NONE` sentinel (`u32::MAX`) marks uninitialized/absent values.
//!
//! # Design
//!
//! Follows the same arena + ID pattern as `ExprArena` + `ExprId` in
//! the parser, and `Idx` in the type pool. The key benefit: callers
//! never see inkwell lifetimes.

use inkwell::basic_block::BasicBlock;
use inkwell::types::BasicTypeEnum;
use inkwell::values::{BasicValueEnum, FunctionValue};

// ---------------------------------------------------------------------------
// ID newtypes
// ---------------------------------------------------------------------------

/// Opaque handle to an LLVM value stored in a `ValueArena`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ValueId(u32);

/// Opaque handle to an LLVM type stored in a `ValueArena`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LLVMTypeId(u32);

/// Opaque handle to an LLVM basic block stored in a `ValueArena`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BlockId(u32);

/// Opaque handle to an LLVM function stored in a `ValueArena`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FunctionId(u32);

// -- Sentinel constants and helpers --

impl ValueId {
    /// Sentinel for "no value".
    pub const NONE: Self = Self(u32::MAX);

    /// True if this is the `NONE` sentinel.
    #[inline]
    pub fn is_none(self) -> bool {
        self.0 == u32::MAX
    }

    /// The raw index.
    #[inline]
    pub fn raw(self) -> u32 {
        self.0
    }
}

impl LLVMTypeId {
    /// Sentinel for "no type".
    pub const NONE: Self = Self(u32::MAX);

    /// True if this is the `NONE` sentinel.
    #[inline]
    pub fn is_none(self) -> bool {
        self.0 == u32::MAX
    }

    /// The raw index.
    #[inline]
    pub fn raw(self) -> u32 {
        self.0
    }
}

impl BlockId {
    /// Sentinel for "no block".
    pub const NONE: Self = Self(u32::MAX);

    /// True if this is the `NONE` sentinel.
    #[inline]
    pub fn is_none(self) -> bool {
        self.0 == u32::MAX
    }

    /// The raw index.
    #[inline]
    pub fn raw(self) -> u32 {
        self.0
    }
}

impl FunctionId {
    /// Sentinel for "no function".
    pub const NONE: Self = Self(u32::MAX);

    /// True if this is the `NONE` sentinel.
    #[inline]
    pub fn is_none(self) -> bool {
        self.0 == u32::MAX
    }

    /// The raw index.
    #[inline]
    pub fn raw(self) -> u32 {
        self.0
    }
}

// ---------------------------------------------------------------------------
// ValueArena
// ---------------------------------------------------------------------------

/// Stores LLVM values behind opaque IDs, hiding the `'ctx` lifetime.
///
/// Internal to `IrBuilder` — callers interact only with ID types.
/// Each `push_*` allocates a slot and returns an ID; each `get_*`
/// retrieves the stored value by ID.
pub(crate) struct ValueArena<'ctx> {
    values: Vec<BasicValueEnum<'ctx>>,
    types: Vec<BasicTypeEnum<'ctx>>,
    blocks: Vec<BasicBlock<'ctx>>,
    functions: Vec<FunctionValue<'ctx>>,
}

impl<'ctx> ValueArena<'ctx> {
    /// Create an empty arena.
    pub(crate) fn new() -> Self {
        Self {
            values: Vec::new(),
            types: Vec::new(),
            blocks: Vec::new(),
            functions: Vec::new(),
        }
    }

    // -- Values --

    /// Store a value, returning its ID.
    #[inline]
    pub(crate) fn push_value(&mut self, val: BasicValueEnum<'ctx>) -> ValueId {
        let id = self.values.len();
        self.values.push(val);
        ValueId(id as u32)
    }

    /// Retrieve a value by ID.
    #[inline]
    pub(crate) fn get_value(&self, id: ValueId) -> BasicValueEnum<'ctx> {
        debug_assert!(
            (id.0 as usize) < self.values.len(),
            "ValueId {} out of bounds (arena has {} values)",
            id.0,
            self.values.len()
        );
        self.values[id.0 as usize]
    }

    // -- Types --

    /// Store a type, returning its ID.
    #[inline]
    pub(crate) fn push_type(&mut self, ty: BasicTypeEnum<'ctx>) -> LLVMTypeId {
        let id = self.types.len();
        self.types.push(ty);
        LLVMTypeId(id as u32)
    }

    /// Retrieve a type by ID.
    #[inline]
    pub(crate) fn get_type(&self, id: LLVMTypeId) -> BasicTypeEnum<'ctx> {
        debug_assert!(
            (id.0 as usize) < self.types.len(),
            "LLVMTypeId {} out of bounds (arena has {} types)",
            id.0,
            self.types.len()
        );
        self.types[id.0 as usize]
    }

    // -- Blocks --

    /// Store a basic block, returning its ID.
    #[inline]
    pub(crate) fn push_block(&mut self, bb: BasicBlock<'ctx>) -> BlockId {
        let id = self.blocks.len();
        self.blocks.push(bb);
        BlockId(id as u32)
    }

    /// Retrieve a basic block by ID.
    #[inline]
    pub(crate) fn get_block(&self, id: BlockId) -> BasicBlock<'ctx> {
        debug_assert!(
            (id.0 as usize) < self.blocks.len(),
            "BlockId {} out of bounds (arena has {} blocks)",
            id.0,
            self.blocks.len()
        );
        self.blocks[id.0 as usize]
    }

    // -- Functions --

    /// Store a function, returning its ID.
    #[inline]
    pub(crate) fn push_function(&mut self, func: FunctionValue<'ctx>) -> FunctionId {
        let id = self.functions.len();
        self.functions.push(func);
        FunctionId(id as u32)
    }

    /// Retrieve a function by ID.
    #[inline]
    pub(crate) fn get_function(&self, id: FunctionId) -> FunctionValue<'ctx> {
        debug_assert!(
            (id.0 as usize) < self.functions.len(),
            "FunctionId {} out of bounds (arena has {} functions)",
            id.0,
            self.functions.len()
        );
        self.functions[id.0 as usize]
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests;
