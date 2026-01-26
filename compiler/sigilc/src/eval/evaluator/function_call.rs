//! Function call evaluation methods for the Evaluator.

use crate::ir::{SharedArena, CallArgRange};
use sigil_eval::{wrong_function_args, not_callable};
use super::{Evaluator, EvalResult, EvalError};
use super::super::value::Value;

impl Evaluator<'_> {
    /// Evaluate a function call.
    pub(super) fn eval_call(&mut self, func: Value, args: &[Value]) -> EvalResult {
        match func.clone() {
            Value::Function(f) => {
                if args.len() != f.params.len() {
                    return Err(wrong_function_args(f.params.len(), args.len()));
                }

                // Create new environment with captures, then push a local scope
                let mut call_env = self.env.child();
                call_env.push_scope();  // Push a new scope for this call's locals

                // Bind captured variables (immutable captures via iterator)
                for (name, value) in f.captures() {
                    call_env.define(*name, value.clone(), false);
                }

                // Pass capabilities from calling scope to called function
                // This enables capability propagation: functions that `uses` a capability
                // can access it when called from within a `with Capability = ... in` block
                for cap_name in f.capabilities() {
                    if let Some(cap_value) = self.env.lookup(*cap_name) {
                        call_env.define(*cap_name, cap_value, false);
                    }
                    // Note: If capability is not in scope, we don't error here.
                    // The type checker already validated that the capability is required.
                    // Runtime errors will occur if the function tries to use the capability.
                }

                // Bind parameters
                for (param, arg) in f.params.iter().zip(args.iter()) {
                    call_env.define(*param, arg.clone(), false);
                }

                // Bind 'self' to the current function for recursive patterns
                let self_name = self.interner.intern("self");
                call_env.define(self_name, func, false);

                // Evaluate body in new environment using the function's arena.
                // Every function carries its own arena for thread safety.
                let func_arena = f.arena();
                let imported_arena = SharedArena::new(func_arena.clone());
                let mut call_evaluator = Evaluator::with_imported_arena(
                    self.interner, func_arena, call_env, imported_arena, self.user_method_registry.clone()
                );
                let result = call_evaluator.eval(f.body);
                call_evaluator.env.pop_scope();
                result
            }
            Value::FunctionVal(func, _name) => {
                func(args).map_err(EvalError::new)
            }
            _ => Err(not_callable(func.type_name())),
        }
    }

    /// Evaluate a function call with named arguments.
    pub(super) fn eval_call_named(&mut self, func: Value, args: CallArgRange) -> EvalResult {
        let call_args = self.arena.get_call_args(args);
        let arg_values: Result<Vec<_>, _> = call_args.iter()
            .map(|arg| self.eval(arg.value))
            .collect();
        self.eval_call(func, &arg_values?)
    }

    /// Call a function value with the given arguments.
    ///
    /// This is a public wrapper around `eval_call` for use in queries.
    pub fn eval_call_value(&mut self, func: Value, args: &[Value]) -> EvalResult {
        self.eval_call(func, args)
    }
}
