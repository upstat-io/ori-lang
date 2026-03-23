//! Core machine representation types.
//!
//! `MachineRepr` captures the physical representation chosen for each type.
//! It is rich enough to express all optimizations in §02–§11 but simple
//! enough that codegen can pattern-match exhaustively.

use crate::enum_repr::EnumRepr;
use crate::struct_repr::{ClosureRepr, FatRepr, RcRepr, StructRepr, TupleRepr};

/// Fixed-width integer variant (narrowed from semantic i64).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IntWidth {
    /// 8-bit integer
    I8,
    /// 16-bit integer
    I16,
    /// 32-bit integer
    I32,
    /// 64-bit integer (canonical for `int`)
    I64,
}

/// Fixed-width float variant (narrowed from semantic f64).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FloatWidth {
    /// 32-bit IEEE 754 single precision
    F32,
    /// 64-bit IEEE 754 double precision (canonical for `float`)
    F64,
}

/// The physical representation of a type in generated code.
///
/// Every `Idx` in the `Pool` maps to exactly one `MachineRepr`.
/// Optimization passes (§02–§11) refine canonical representations
/// into narrower ones; codegen reads the final `MachineRepr`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MachineRepr {
    /// Fixed-width integer (narrowed from semantic i64).
    Int {
        /// Bit width: I8, I16, I32, or I64.
        width: IntWidth,
        /// Whether this integer is signed.
        signed: bool,
    },
    /// Fixed-width float (narrowed from semantic f64).
    Float {
        /// Bit width: F32 or F64.
        width: FloatWidth,
    },
    /// Boolean (always i1).
    Bool,
    /// Unicode scalar value (always i32 — 0..=0x10FFFF).
    Char,
    /// 8-bit unsigned byte (always i8).
    Byte,
    /// Duration in nanoseconds (always i64).
    Duration,
    /// Memory size in bytes (always i64).
    Size,
    /// Comparison ordering (always i8: Less=0, Equal=1, Greater=2).
    Ordering,
    /// Unit (zero-sized in memory, i64(0) as value).
    Unit,
    /// Never (uninhabited).
    Never,
    /// Struct with optimized field layout.
    Struct(StructRepr),
    /// Enum with optimized discriminant and payload.
    Enum(EnumRepr),
    /// Tuple (treated as anonymous struct).
    Tuple(TupleRepr),
    /// Heap-allocated reference-counted value.
    RcPointer(RcRepr),
    /// Fat pointer (ptr + metadata) — used for str, [T], {K:V}, Set<T>.
    FatPointer(FatRepr),
    /// Function pointer (fn ptr + optional env ptr).
    Closure(ClosureRepr),
    /// Range (always {i64 start, i64 end, i64 step, i64 inclusive}).
    Range,
    /// Stack-promoted value (was heap, promoted by escape analysis).
    ///
    /// `had_rc` records whether the original heap allocation used RC
    /// headers — needed so drop codegen knows whether to emit a
    /// stack-local destructor.
    StackPromoted {
        /// The representation of the promoted value.
        inner: Box<MachineRepr>,
        /// Whether the original allocation had an RC header.
        had_rc: bool,
    },
    /// Opaque pointer (iterator, channel — runtime-managed).
    OpaquePtr,
}
