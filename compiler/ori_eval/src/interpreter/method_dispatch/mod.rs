//! Method dispatch methods for the Interpreter.

use ori_ir::Name;

mod collection_ops;
mod iterator;

use crate::errors::wrong_function_args;
use crate::exec::call::bind_captures_iter;
use crate::methods::{dispatch_builtin_method, DispatchCtx};
use crate::{EvalResult, Mutability, UserMethod, Value};

use super::resolvers::MethodResolution;
use super::Interpreter;

impl Interpreter<'_> {
    /// Evaluate a method call using the Chain of Responsibility pattern.
    ///
    /// Methods are resolved in priority order:
    /// 0. Print methods (invoked via `PatternExecutor` for the Print capability)
    /// 1. Associated functions on type references (e.g., `Duration.from_seconds`)
    /// 2. User-defined methods from impl blocks (priority 0)
    /// 3. Derived methods from `#[derive(...)]` (priority 1)
    /// 4. Collection methods requiring interpreter (priority 2)
    /// 5. Built-in methods in `MethodRegistry` (priority 3)
    #[tracing::instrument(level = "debug", skip(self, receiver, args))]
    pub fn eval_method_call(
        &mut self,
        receiver: Value,
        method: Name,
        args: Vec<Value>,
    ) -> EvalResult {
        self.mode_state.count_method_call();

        // Handle print methods (invoked via PatternExecutor for the Print capability).
        // Pre-interned Name comparison avoids string lookup on every method call.
        let pn = self.print_names;
        if method == pn.println || method == pn.builtin_println {
            self.handle_println(&args);
            return Ok(Value::Void);
        }
        if method == pn.print || method == pn.builtin_print {
            self.handle_print(&args);
            return Ok(Value::Void);
        }

        // Handle associated function calls on type references
        if let Value::TypeRef { type_name } = &receiver {
            // First check user-defined associated functions in the registry
            // Clone the method to release the lock before calling eval_associated_function
            let user_method = self
                .user_method_registry
                .read()
                .lookup(*type_name, method)
                .cloned();

            if let Some(ref method_def) = user_method {
                return self.eval_associated_function(method_def, &args, method);
            }

            // Check derived methods (e.g., Default.default() is a static method)
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

            // Fall back to built-in associated functions (Duration, Size)
            let ctx = DispatchCtx {
                names: &self.builtin_method_names,
                interner: self.interner,
            };
            return crate::methods::dispatch_associated_function(*type_name, method, args, &ctx);
        }

        // Handle callable struct fields: if a struct has a field with the method name
        // and that field is a function, call it instead of treating as a method.
        // This enables patterns like: `Handler { callback: fn }.callback(arg)`
        if let Value::Struct(s) = &receiver {
            if let Some(field_value) = s.get_field(method) {
                // Check if the field is callable
                match &field_value {
                    Value::Function(_) | Value::MemoizedFunction(_) | Value::FunctionVal(_, _) => {
                        return self.eval_call(field_value, &args);
                    }
                    _ => {
                        // Field exists but isn't callable - fall through to method dispatch
                    }
                }
            }
        }

        let type_name = self.get_value_type_name(&receiver);

        // Resolve the method using the resolver chain
        let resolution = self.resolve_method(&receiver, type_name, method);

        // Execute based on resolution type
        match resolution {
            MethodResolution::User(user_method) => {
                self.eval_user_method(receiver, &user_method, &args, method)
            }
            MethodResolution::Derived(derived_info) => {
                self.eval_derived_method(receiver, &derived_info, &args)
            }
            MethodResolution::Collection(collection_method) => {
                self.eval_collection_method(receiver, collection_method, &args)
            }
            MethodResolution::Builtin => {
                let ctx = DispatchCtx {
                    names: &self.builtin_method_names,
                    interner: self.interner,
                };
                dispatch_builtin_method(receiver, method, args, &ctx)
            }
            MethodResolution::NotFound => {
                let method_str = self.interner.lookup(method);
                let type_str = self.interner.lookup(type_name);
                Err(crate::errors::no_such_method(method_str, type_str).into())
            }
        }
    }

    /// Resolve a method using the cached dispatcher chain.
    ///
    /// Uses the pre-built dispatcher to try resolvers in priority order.
    /// The dispatcher sees method registrations made after construction because
    /// `user_method_registry` uses interior mutability (`SharedMutableRegistry`).
    fn resolve_method(
        &self,
        receiver: &Value,
        type_name: Name,
        method_name: Name,
    ) -> MethodResolution {
        self.method_dispatcher
            .resolve(receiver, type_name, method_name)
    }

    /// Handle a `println` method call via the print handler.
    fn handle_println(&self, args: &[Value]) {
        if let Some(msg) = args.first() {
            match msg {
                Value::Str(s) => self.print_handler.println(s),
                other => self.print_handler.println(&other.display_value()),
            }
        }
    }

    /// Handle a `print` method call via the print handler.
    fn handle_print(&self, args: &[Value]) {
        if let Some(msg) = args.first() {
            match msg {
                Value::Str(s) => self.print_handler.print(s),
                other => self.print_handler.print(&other.display_value()),
            }
        }
    }

    /// Get the concrete type name for a value as an interned Name.
    ///
    /// For struct values, returns the struct's `type_name` directly.
    /// For other values, uses pre-interned type names from `self.type_names`.
    ///
    /// # Performance
    ///
    /// This method is called on every method dispatch (extremely hot path).
    /// Using pre-interned names avoids hash lookups and lock acquisition
    /// that would occur with `interner.intern()` calls.
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

    /// Evaluate a user-defined method from an impl block.
    #[expect(
        clippy::arithmetic_side_effects,
        reason = "method params always include self, so len >= 1"
    )]
    pub(super) fn eval_user_method(
        &mut self,
        receiver: Value,
        method: &UserMethod,
        args: &[Value],
        method_name: Name,
    ) -> EvalResult {
        // Method params include 'self' as first parameter
        if method.params.len() != args.len() + 1 {
            return Err(wrong_function_args(method.params.len() - 1, args.len()).into());
        }
        self.eval_method_body(Some(receiver), method, args, method_name)
    }

    /// Dispatch an Index trait method call on a user-defined type.
    ///
    /// Handles both single-impl and multi-impl cases:
    /// - Single `index` method → call directly
    /// - Multiple `index` methods (e.g., `Index<int, V>` + `Index<str, V>`)
    ///   → match by `key_type_hint` against the runtime type of `idx_val`
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

    /// Evaluate an associated function (no `self` parameter).
    ///
    /// Associated functions are called on types rather than instances:
    /// `Point.origin()` instead of `point.method()`.
    pub(super) fn eval_associated_function(
        &mut self,
        method: &UserMethod,
        args: &[Value],
        method_name: Name,
    ) -> EvalResult {
        // Associated functions don't have 'self', so params == args
        if method.params.len() != args.len() {
            return Err(wrong_function_args(method.params.len(), args.len()).into());
        }
        self.eval_method_body(None, method, args, method_name)
    }

    /// Shared helper for evaluating a method/associated function body.
    ///
    /// When `receiver` is `Some`, binds it as `self` (first param) and zips
    /// remaining params with `args`. When `None`, zips all params with `args`.
    fn eval_method_body(
        &mut self,
        receiver: Option<Value>,
        method: &UserMethod,
        args: &[Value],
        method_name: Name,
    ) -> EvalResult {
        self.check_recursion_limit()?;

        let mut call_env = self.env.child();
        call_env.push_scope();

        bind_captures_iter(&mut call_env, method.captures.iter());

        // Bind self + remaining params, or all params directly
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

        // Evaluate body via canonical IR.
        // The scope is popped automatically via RAII when call_interpreter drops.
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
