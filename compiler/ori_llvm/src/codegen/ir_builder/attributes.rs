//! Function and parameter attribute operations for `IrBuilder`.
//!
//! Provides methods for adding LLVM attributes to functions, parameters,
//! and call sites. These attributes drive optimization (nounwind, cold,
//! noalias) and ABI correctness (uwtable, sret, byval).

use inkwell::attributes::{Attribute, AttributeLoc};
use inkwell::types::AnyType;

use super::IrBuilder;
use crate::codegen::value_id::{FunctionId, LLVMTypeId};

impl IrBuilder<'_, '_> {
    // -- Function attributes --

    /// Add the `uwtable` attribute to a function.
    ///
    /// Required on Windows x86-64 for all functions so the SEH unwinder can
    /// walk the stack. Without this, functions that are unwound through (e.g.,
    /// a caller of `ori_panic`) will corrupt the stack because
    /// `RtlVirtualUnwind` cannot find their frame info in `.pdata`/`.xdata`.
    pub fn add_uwtable_attribute(&mut self, func: FunctionId) {
        let f = self.arena.get_function(func);
        let kind = Attribute::get_named_enum_kind_id("uwtable");
        // uwtable value: 2 = UWTableKind::Async (full async unwind tables)
        let attr = self.scx.llcx.create_enum_attribute(kind, 2);
        f.add_attribute(AttributeLoc::Function, attr);
    }

    /// Add the `noreturn` attribute to a function.
    ///
    /// Declares the function never returns to its caller (e.g., `ori_panic`).
    /// Enables LLVM to insert `unreachable` after calls to this function,
    /// eliminating dead code on panic paths.
    pub fn add_noreturn_attribute(&mut self, func: FunctionId) {
        let f = self.arena.get_function(func);
        let kind = Attribute::get_named_enum_kind_id("noreturn");
        let attr = self.scx.llcx.create_enum_attribute(kind, 0);
        f.add_attribute(AttributeLoc::Function, attr);
    }

    /// Add the `nounwind` attribute to a function.
    ///
    /// Declares the function will not unwind (no exceptions). Enables LLVM
    /// to optimize exception handling paths around calls to this function.
    pub fn add_nounwind_attribute(&mut self, func: FunctionId) {
        let f = self.arena.get_function(func);
        let kind = Attribute::get_named_enum_kind_id("nounwind");
        let attr = self.scx.llcx.create_enum_attribute(kind, 0);
        f.add_attribute(AttributeLoc::Function, attr);
    }

    /// Add the `noinline` attribute to a function.
    ///
    /// Prevents LLVM from inlining this function. Used for cold paths like
    /// specialized drop functions and panic handlers.
    pub fn add_noinline_attribute(&mut self, func: FunctionId) {
        let f = self.arena.get_function(func);
        let kind = Attribute::get_named_enum_kind_id("noinline");
        let attr = self.scx.llcx.create_enum_attribute(kind, 0);
        f.add_attribute(AttributeLoc::Function, attr);
    }

    /// Add the `cold` attribute to a function.
    ///
    /// Hints that this function is rarely called. LLVM uses this to:
    /// - Move cold code out of hot code layout
    /// - Reduce inlining priority
    /// - Optimize branch prediction away from cold paths
    pub fn add_cold_attribute(&mut self, func: FunctionId) {
        let f = self.arena.get_function(func);
        let kind = Attribute::get_named_enum_kind_id("cold");
        let attr = self.scx.llcx.create_enum_attribute(kind, 0);
        f.add_attribute(AttributeLoc::Function, attr);
    }

    /// Add the `noredzone` attribute to a function.
    ///
    /// Prevents the function from using the 128-byte red zone below `%rsp`
    /// (`x86_64` `SysV` ABI). Required for JIT-compiled code where `fastcc`
    /// functions call into host runtime functions: the host functions create
    /// their own stack frames by pushing below `%rsp`, which can clobber
    /// red-zone data if the JIT code was using it.
    pub fn add_noredzone_attribute(&mut self, func: FunctionId) {
        let f = self.arena.get_function(func);
        let kind = Attribute::get_named_enum_kind_id("noredzone");
        let attr = self.scx.llcx.create_enum_attribute(kind, 0);
        f.add_attribute(AttributeLoc::Function, attr);
    }

    /// Add the `noalias` attribute to a function's return value.
    ///
    /// Guarantees the returned pointer does not alias any other pointer
    /// visible to the caller. Used for allocation functions like `ori_rc_alloc`
    /// where the returned pointer is a fresh heap allocation.
    pub fn add_noalias_return_attribute(&mut self, func: FunctionId) {
        let f = self.arena.get_function(func);
        let kind = Attribute::get_named_enum_kind_id("noalias");
        let attr = self.scx.llcx.create_enum_attribute(kind, 0);
        f.add_attribute(AttributeLoc::Return, attr);
    }

    /// Add the `memory(argmem: readwrite)` attribute to a function.
    ///
    /// Declares the function only reads/writes memory reachable from its pointer
    /// arguments (no global memory access, no inaccessible memory). This is
    /// critical for ARC runtime functions (`ori_rc_inc`, `ori_rc_dec`) which
    /// modify refcount at `ptr - 8` but don't access any other memory.
    ///
    /// # LLVM `MemoryEffects` Encoding
    ///
    /// The `memory` attribute uses a bitfield encoding from `ModRef.h`:
    /// - Bits \[1:0\]: `DefaultMem` access (None=0, Ref=1, Mod=2, ModRef=3)
    /// - Bits \[3:2\]: `ArgMem` access
    /// - Bits \[5:4\]: `InaccessibleMem` access
    ///
    /// `memory(argmem: readwrite)` = DefaultMem:None | ArgMem:ModRef | InaccessibleMem:None
    /// = 0 | (3 << 2) | (0 << 4) = 12
    pub fn add_memory_argmem_readwrite_attribute(&mut self, func: FunctionId) {
        let f = self.arena.get_function(func);
        let kind = Attribute::get_named_enum_kind_id("memory");
        // MemoryEffects encoding: argmem: readwrite (ModRef=3 at ArgMem position bits [3:2])
        let attr = self.scx.llcx.create_enum_attribute(kind, 12);
        f.add_attribute(AttributeLoc::Function, attr);
    }

    // -- Parameter attributes --

    /// Add the `sret(T)` attribute to a function parameter.
    ///
    /// Marks the parameter as a hidden struct return pointer. LLVM uses
    /// this to optimize the return path and generate correct ABI code.
    pub fn add_sret_attribute(
        &mut self,
        func: FunctionId,
        param_index: u32,
        pointee_type: LLVMTypeId,
    ) {
        let f = self.arena.get_function(func);
        let ty = self.arena.get_type(pointee_type);
        let sret_kind = Attribute::get_named_enum_kind_id("sret");
        let sret_attr = self
            .scx
            .llcx
            .create_type_attribute(sret_kind, ty.as_any_type_enum());
        f.add_attribute(AttributeLoc::Param(param_index), sret_attr);
    }

    /// Add the `noalias` attribute to a function parameter.
    ///
    /// Guarantees the parameter pointer does not alias any other pointer
    /// visible to the callee. Required on sret parameters by the x86-64 ABI.
    pub fn add_noalias_attribute(&mut self, func: FunctionId, param_index: u32) {
        let f = self.arena.get_function(func);
        let noalias_kind = Attribute::get_named_enum_kind_id("noalias");
        let noalias_attr = self.scx.llcx.create_enum_attribute(noalias_kind, 0);
        f.add_attribute(AttributeLoc::Param(param_index), noalias_attr);
    }

    /// Add the `byval(T)` attribute to a function parameter.
    ///
    /// Indicates the parameter is passed by value on the stack. The callee
    /// receives a copy; modifications don't affect the caller's data.
    pub fn add_byval_attribute(
        &mut self,
        func: FunctionId,
        param_index: u32,
        pointee_type: LLVMTypeId,
    ) {
        let f = self.arena.get_function(func);
        let ty = self.arena.get_type(pointee_type);
        let byval_kind = Attribute::get_named_enum_kind_id("byval");
        let byval_attr = self
            .scx
            .llcx
            .create_type_attribute(byval_kind, ty.as_any_type_enum());
        f.add_attribute(AttributeLoc::Param(param_index), byval_attr);
    }

    /// Add the `noundef` attribute to a function parameter.
    ///
    /// Declares the parameter value is never `undef` or `poison`. Ori's type
    /// system guarantees all scalar values are initialized, so this is always
    /// safe for scalar types (`i64`, `f64`, `i1`, `i32`, `i8`).
    pub fn add_noundef_param_attribute(&mut self, func: FunctionId, param_index: u32) {
        let f = self.arena.get_function(func);
        let kind = Attribute::get_named_enum_kind_id("noundef");
        let attr = self.scx.llcx.create_enum_attribute(kind, 0);
        f.add_attribute(AttributeLoc::Param(param_index), attr);
    }

    /// Add the `noundef` attribute to a function's return value.
    ///
    /// Declares the return value is never `undef` or `poison`. Safe for
    /// scalar types where Ori guarantees the value is always initialized.
    pub fn add_noundef_return_attribute(&mut self, func: FunctionId) {
        let f = self.arena.get_function(func);
        let kind = Attribute::get_named_enum_kind_id("noundef");
        let attr = self.scx.llcx.create_enum_attribute(kind, 0);
        f.add_attribute(AttributeLoc::Return, attr);
    }

    // -- Per-call-site attributes --

    /// Add `noalias` to a parameter of the most recently emitted call.
    ///
    /// Used for COW operations where static uniqueness analysis proves the
    /// data pointer is uniquely owned (refcount == 1). Unlike function-level
    /// `noalias` (which applies to all call sites), this is per-call-site —
    /// only the specific call where uniqueness is proven gets the attribute.
    ///
    /// Does nothing if no call has been emitted yet.
    pub fn mark_last_call_param_noalias(&mut self, param_index: u32) {
        if let Some(call_site) = self.last_call_site {
            let noalias_kind = Attribute::get_named_enum_kind_id("noalias");
            let noalias_attr = self.scx.llcx.create_enum_attribute(noalias_kind, 0);
            call_site.add_attribute(AttributeLoc::Param(param_index), noalias_attr);
        }
    }
}
