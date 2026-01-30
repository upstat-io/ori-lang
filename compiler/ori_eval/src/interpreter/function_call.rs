//! Function call evaluation methods for the Interpreter.

use super::Interpreter;
use crate::exec::call::{
    bind_captures, bind_parameters, check_arg_count, eval_function_val_call, extract_named_args,
};
use crate::{not_callable, EvalResult, Mutability, Value};
use ori_ir::CallArgRange;

impl Interpreter<'_> {
    /// Evaluate a function call.
    pub(super) fn eval_call(&mut self, func: &Value, args: &[Value]) -> EvalResult {
        match func {
            Value::Function(f) => {
                check_arg_count(f, args)?;

                // Create new environment with captures, then push a local scope
                let mut call_env = self.env.child();
                call_env.push_scope();

                // Bind captured variables
                bind_captures(&mut call_env, f);

                // Pass capabilities from calling scope to called function
                // This enables capability propagation: functions that `uses` a capability
                // can access it when called from within a `with Capability = ... in` block
                for cap_name in f.capabilities() {
                    if let Some(cap_value) = self.env.lookup(*cap_name) {
                        call_env.define(*cap_name, cap_value, Mutability::Immutable);
                    }
                    // Note: If capability is not in scope, we don't error here.
                    // The type checker already validated that the capability is required.
                    // Runtime errors will occur if the function tries to use the capability.
                }

                // Bind parameters
                bind_parameters(&mut call_env, f, args);

                // Bind 'self' to the current function for recursive patterns
                // Uses pre-computed self_name to avoid repeated interning
                call_env.define(self.self_name, func.clone(), Mutability::Immutable);

                // Evaluate body using the function's arena (arena threading pattern).
                // The scope is popped automatically via RAII when call_interpreter drops.
                let func_arena = f.arena();
                let mut call_interpreter = self.create_function_interpreter(func_arena, call_env);
                call_interpreter.eval(f.body)
            }
            Value::MemoizedFunction(mf) => {
                // Check cache first
                if let Some(cached) = mf.get_cached(args) {
                    return Ok(cached);
                }

                // Not cached - evaluate the underlying function
                let f = &mf.func;
                check_arg_count(f, args)?;

                // Create new environment with captures, then push a local scope
                let mut call_env = self.env.child();
                call_env.push_scope();

                // Bind captured variables
                bind_captures(&mut call_env, f);

                // Pass capabilities from calling scope to called function
                for cap_name in f.capabilities() {
                    if let Some(cap_value) = self.env.lookup(*cap_name) {
                        call_env.define(*cap_name, cap_value, Mutability::Immutable);
                    }
                }

                // Bind parameters
                bind_parameters(&mut call_env, f, args);

                // Bind 'self' to the MEMOIZED function so recursive calls also use the cache
                // Uses pre-computed self_name to avoid repeated interning
                call_env.define(self.self_name, func.clone(), Mutability::Immutable);

                // Evaluate body using the function's arena (arena threading pattern).
                // The scope is popped automatically via RAII when call_interpreter drops.
                let func_arena = f.arena();
                let mut call_interpreter = self.create_function_interpreter(func_arena, call_env);
                let result = call_interpreter.eval(f.body);

                // Cache the result before returning
                if let Ok(ref value) = result {
                    mf.cache_result(args, value.clone());
                }

                result
            }
            Value::FunctionVal(func_ptr, _name) => eval_function_val_call(*func_ptr, args),
            Value::VariantConstructor {
                type_name,
                variant_name,
                field_count,
            } => {
                // Check argument count matches field count
                if args.len() != *field_count {
                    return Err(crate::wrong_arg_count(
                        self.interner.lookup(*variant_name),
                        *field_count,
                        args.len(),
                    ));
                }
                // Construct the variant with the provided arguments
                Ok(Value::variant(*type_name, *variant_name, args.to_vec()))
            }
            Value::NewtypeConstructor { type_name } => {
                // Newtypes take exactly one argument (the underlying value)
                if args.len() != 1 {
                    return Err(crate::wrong_arg_count(
                        self.interner.lookup(*type_name),
                        1,
                        args.len(),
                    ));
                }
                // Construct the newtype wrapping the underlying value
                Ok(Value::newtype(*type_name, args[0].clone()))
            }
            _ => Err(not_callable(func.type_name())),
        }
    }

    /// Evaluate a function call with named arguments.
    pub(super) fn eval_call_named(&mut self, func: &Value, args: CallArgRange) -> EvalResult {
        let arg_values = extract_named_args(args, self.arena, |expr| self.eval(expr))?;
        self.eval_call(func, &arg_values)
    }

    /// Call a function value with the given arguments.
    ///
    /// This is a public wrapper around `eval_call` for use in queries.
    pub fn eval_call_value(&mut self, func: &Value, args: &[Value]) -> EvalResult {
        self.eval_call(func, args)
    }
}
