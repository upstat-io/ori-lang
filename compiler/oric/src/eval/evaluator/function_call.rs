//! Function call evaluation methods for the Evaluator.

use crate::ir::CallArgRange;
use ori_eval::not_callable;
use super::{Evaluator, EvalResult};
use super::super::value::Value;
use super::super::exec::call::{
    check_arg_count, bind_captures, bind_parameters, bind_self,
    eval_function_val_call, extract_named_args,
};

impl Evaluator<'_> {
    /// Evaluate a function call.
    pub(super) fn eval_call(&mut self, func: Value, args: &[Value]) -> EvalResult {
        match func.clone() {
            Value::Function(f) => {
                check_arg_count(&f, args)?;

                // Create new environment with captures, then push a local scope
                let mut call_env = self.env.child();
                call_env.push_scope();

                // Bind captured variables
                bind_captures(&mut call_env, &f);

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
                bind_parameters(&mut call_env, &f, args);

                // Bind 'self' to the current function for recursive patterns
                bind_self(&mut call_env, func, self.interner);

                // Evaluate body using the function's arena (arena threading pattern).
                let func_arena = f.arena();
                let mut call_evaluator = self.create_function_evaluator(func_arena, call_env);
                let result = call_evaluator.eval(f.body);
                call_evaluator.env.pop_scope();
                result
            }
            Value::FunctionVal(func_ptr, _name) => {
                eval_function_val_call(func_ptr, args)
            }
            _ => Err(not_callable(func.type_name())),
        }
    }

    /// Evaluate a function call with named arguments.
    pub(super) fn eval_call_named(&mut self, func: Value, args: CallArgRange) -> EvalResult {
        let arg_values = extract_named_args(args, self.arena, |expr| self.eval(expr))?;
        self.eval_call(func, &arg_values)
    }

    /// Call a function value with the given arguments.
    ///
    /// This is a public wrapper around `eval_call` for use in queries.
    pub fn eval_call_value(&mut self, func: Value, args: &[Value]) -> EvalResult {
        self.eval_call(func, args)
    }
}
