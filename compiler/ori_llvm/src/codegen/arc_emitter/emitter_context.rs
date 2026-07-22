use core::fmt;

use crate::aot::debug::DebugContext;
use crate::codegen::value_id::FunctionId;

use super::CodegenContext;

/// Function-scoped inputs that vary between emitter instances.
#[derive(Clone, Copy)]
pub(crate) struct ArcEmitterFunctionContext<'a, 'ctx> {
    /// LLVM function receiving emitted blocks.
    pub(super) current_function: FunctionId,
    /// Function-level code generation state.
    pub(super) codegen: &'a CodegenContext,
    /// Optional source-debug metadata sink.
    pub(super) debug: Option<&'a DebugContext<'ctx>>,
}

impl<'a, 'ctx> ArcEmitterFunctionContext<'a, 'ctx> {
    /// Groups the function-local inputs for one emitter instance.
    pub(crate) const fn new(
        current_function: FunctionId,
        codegen: &'a CodegenContext,
        debug: Option<&'a DebugContext<'ctx>>,
    ) -> Self {
        Self {
            current_function,
            codegen,
            debug,
        }
    }
}

// Why: Function contexts retain large and opaque LLVM state; report only local identity.
impl fmt::Debug for ArcEmitterFunctionContext<'_, '_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ArcEmitterFunctionContext")
            .field("current_function", &self.current_function)
            .field("has_debug", &self.debug.is_some())
            .finish()
    }
}
