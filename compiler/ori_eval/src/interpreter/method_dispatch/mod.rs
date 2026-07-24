mod collection_ops;
mod hash;
mod iterator;

use ori_ir::canon::MonoConstBinding;
use ori_ir::Name;

use crate::errors::wrong_function_args;
use crate::exec::call::bind_captures_iter;
use crate::methods::{dispatch_builtin_method, DispatchCtx};
use crate::{EvalResult, Mutability, UserMethod, Value};

use super::resolvers::MethodResolution;
use super::Interpreter;

impl Interpreter<'_> {
    /// Evaluate a method call through print, associated, user, derived,
    /// collection, and builtin dispatch in that priority order.
    #[tracing::instrument(level = "debug", skip(self, receiver, args))]
    pub fn eval_method_call(
        &mut self,
        receiver: Value,
        method: Name,
        args: Vec<Value>,
    ) -> EvalResult {
        self.eval_method_call_with_const_bindings(receiver, method, args, &[])
    }

    /// Evaluate a method call under one exact mono instance's const bindings.
    pub(super) fn eval_method_call_with_const_bindings(
        &mut self,
        receiver: Value,
        method: Name,
        args: Vec<Value>,
        const_bindings: &[MonoConstBinding],
    ) -> EvalResult {
        self.mode_state.count_method_call();

        let pn = self.print_names;
        if method == pn.println || method == pn.builtin_println {
            self.handle_println(&args);
            return Ok(Value::Void);
        }
        if method == pn.print || method == pn.builtin_print {
            self.handle_print(&args);
            return Ok(Value::Void);
        }

        if let Value::TypeRef { type_name } = &receiver {
            let user_method = self
                .user_method_registry
                .read()
                .lookup(*type_name, method)
                .cloned();

            if let Some(ref method_def) = user_method {
                return self.eval_associated_function_with_const_bindings(
                    method_def,
                    &args,
                    method,
                    const_bindings,
                );
            }

            let derived_info = self
                .user_method_registry
                .read()
                .lookup_derived(*type_name, method)
                .cloned();
            if let Some(ref info) = derived_info {
                return self.eval_derived_method(
                    Value::TypeRef {
                        type_name: *type_name,
                    },
                    info,
                    &args,
                );
            }

            let ctx = DispatchCtx {
                names: &self.builtin_method_names,
                interner: self.interner,
            };
            return crate::methods::dispatch_associated_function(*type_name, method, args, &ctx);
        }

        if let Value::Struct(s) = &receiver {
            if let Some(field_value) = s.get_field(method) {
                match &field_value {
                    Value::Function(_) | Value::MemoizedFunction(_) | Value::FunctionVal(_, _) => {
                        return self.eval_call_with_const_bindings(
                            field_value,
                            &args,
                            const_bindings,
                        );
                    }
                    _ => {}
                }
            }
        }

        let type_name = self.get_value_type_name(&receiver);

        let resolution = self.resolve_method(&receiver, type_name, method);

        match resolution {
            MethodResolution::User(user_method) => self.eval_user_method_with_const_bindings(
                receiver,
                &user_method,
                &args,
                method,
                const_bindings,
            ),
            MethodResolution::Derived(derived_info) => {
                self.eval_derived_method(receiver, &derived_info, &args)
            }
            MethodResolution::Collection(collection_method) => {
                self.eval_collection_method(receiver, collection_method, &args)
            }
            MethodResolution::Builtin => self.eval_resolved_builtin(receiver, method, args),
            MethodResolution::NotFound => {
                let method_str = self.interner.lookup(method);
                let type_str = self.interner.lookup(type_name);
                Err(crate::errors::no_such_method(method_str, type_str).into())
            }
        }
    }

    fn eval_resolved_builtin(
        &mut self,
        receiver: Value,
        method: Name,
        args: Vec<Value>,
    ) -> EvalResult {
        if method == self.builtin_method_names.hash {
            return self.eval_builtin_hash(receiver, &args);
        }

        match receiver {
            // Why: User-defined key equality and hashing require interpreter state.
            Value::Map(_) => self.dispatch_map_method(receiver, method, args),
            Value::Set(_) => self.dispatch_set_method(receiver, method, args),
            _ => {
                let ctx = DispatchCtx {
                    names: &self.builtin_method_names,
                    interner: self.interner,
                };
                dispatch_builtin_method(receiver, method, args, &ctx)
            }
        }
    }

    fn resolve_method(
        &self,
        receiver: &Value,
        type_name: Name,
        method_name: Name,
    ) -> MethodResolution {
        self.method_dispatcher
            .resolve(receiver, type_name, method_name)
    }

    fn handle_println(&self, args: &[Value]) {
        if let Some(msg) = args.first() {
            match msg {
                Value::Str(s) => self.print_handler.println(s),
                other => self.print_handler.println(&other.display_value()),
            }
        }
    }

    fn handle_print(&self, args: &[Value]) {
        if let Some(msg) = args.first() {
            match msg {
                Value::Str(s) => self.print_handler.print(s),
                other => self.print_handler.print(&other.display_value()),
            }
        }
    }

    /// Return the value's pre-interned concrete type name without interner writes.
    pub(super) fn get_value_type_name(&self, value: &Value) -> Name {
        let names = &self.type_names;
        match value {
            Value::Struct(s) => s.type_name,
            Value::Range(_) => names.range,
            Value::Iterator(_) => names.iterator,
            Value::Int(_) => names.int,
            Value::Float(_) => names.float,
            Value::Bool(_) => names.bool_,
            Value::Str(_) => names.str_,
            Value::Char(_) => names.char_,
            Value::Byte(_) => names.byte,
            Value::Void => names.void,
            Value::Duration(_) => names.duration,
            Value::Size(_) => names.size,
            Value::Ordering(_) => names.ordering,
            Value::List(_) => names.list,
            Value::Map(_) => names.map,
            Value::Set(_) => names.set,
            Value::Tuple(_) => names.tuple,
            Value::Some(_) | Value::None => names.option,
            Value::Ok(_) | Value::Err(_) => names.result,
            Value::Variant { type_name, .. }
            | Value::VariantConstructor { type_name, .. }
            | Value::Newtype { type_name, .. }
            | Value::NewtypeConstructor { type_name }
            | Value::TypeRef { type_name } => *type_name,
            Value::Function(_) | Value::MemoizedFunction(_) => names.function,
            Value::FunctionVal(_, _) => names.function_val,
            Value::ModuleNamespace(_) => names.module,
            Value::Error(_) => names.error,
        }
    }

    pub(super) fn eval_user_method(
        &mut self,
        receiver: Value,
        method: &UserMethod,
        args: &[Value],
        method_name: Name,
    ) -> EvalResult {
        self.eval_user_method_with_const_bindings(receiver, method, args, method_name, &[])
    }

    fn eval_user_method_with_const_bindings(
        &mut self,
        receiver: Value,
        method: &UserMethod,
        args: &[Value],
        method_name: Name,
        const_bindings: &[MonoConstBinding],
    ) -> EvalResult {
        let Some((_, explicit_params)) = method.params.split_first() else {
            return Err(wrong_function_args(0, args.len()).into());
        };
        if explicit_params.len() != args.len() {
            return Err(wrong_function_args(explicit_params.len(), args.len()).into());
        }
        self.eval_method_body(Some(receiver), method, args, method_name, const_bindings)
    }

    /// Dispatch a user-defined index method by the runtime key type when needed.
    pub(super) fn eval_index_user_type(&mut self, receiver: Value, idx_val: Value) -> EvalResult {
        let type_name = self.get_value_type_name(&receiver);
        let index_name = self.op_names.index;

        let matched_method = {
            let registry = self.user_method_registry.read();
            match registry.lookup_all(type_name, index_name) {
                None => None,
                Some([single]) => Some(single.clone()),
                Some(methods) => {
                    let key_type = self.get_value_type_name(&idx_val);
                    tracing::debug!(
                        method_count = methods.len(),
                        ?key_type,
                        hints = ?methods.iter().map(|m| m.key_type_hint).collect::<Vec<_>>(),
                        "index multi-dispatch"
                    );
                    methods
                        .iter()
                        .find(|m| m.key_type_hint == Some(key_type))
                        .cloned()
                }
            }
        };

        match matched_method {
            Some(method) => self.eval_user_method(receiver, &method, &[idx_val], index_name),
            None => self.eval_method_call(receiver, index_name, vec![idx_val]),
        }
    }

    fn eval_associated_function_with_const_bindings(
        &mut self,
        method: &UserMethod,
        args: &[Value],
        method_name: Name,
        const_bindings: &[MonoConstBinding],
    ) -> EvalResult {
        if method.params.len() != args.len() {
            return Err(wrong_function_args(method.params.len(), args.len()).into());
        }
        self.eval_method_body(None, method, args, method_name, const_bindings)
    }

    fn eval_method_body(
        &mut self,
        receiver: Option<Value>,
        method: &UserMethod,
        args: &[Value],
        method_name: Name,
        const_bindings: &[MonoConstBinding],
    ) -> EvalResult {
        self.check_recursion_limit()?;

        let mut call_env = self.env.child();
        call_env.push_scope();

        bind_captures_iter(&mut call_env, method.captures.iter());

        for binding in const_bindings {
            call_env.define(
                binding.name,
                super::mono_const_value_to_value(&binding.value),
                Mutability::Immutable,
            );
        }

        let param_args: &[Name] = if let Some(recv) = receiver {
            if let Some(&self_param) = method.params.first() {
                call_env.define(self_param, recv, Mutability::Immutable);
            }
            &method.params[1..]
        } else {
            &method.params
        };

        for (param, arg) in param_args.iter().zip(args.iter()) {
            call_env.define(*param, arg.clone(), Mutability::Immutable);
        }

        // INVARIANT: The child environment owns the call scope for its lifetime.
        let mut call_interpreter = self.create_function_interpreter(
            &method.arena,
            call_env,
            method_name,
            method.canon.clone(),
        );

        let result = call_interpreter.eval_can(method.can_body);
        self.mode_state
            .merge_child_counters(&call_interpreter.mode_state);
        result
    }
}

#[cfg(test)]
mod tests;
