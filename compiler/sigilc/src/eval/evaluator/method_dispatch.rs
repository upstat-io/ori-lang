//! Method dispatch methods for the Evaluator.

use crate::ir::{ExprArena, Name, SharedArena};
use sigil_eval::{DerivedMethodInfo, DerivedTrait, UserMethod, wrong_function_args, wrong_arg_count, EvalError};
use super::{Evaluator, EvalResult};
use super::super::value::Value;
use super::resolvers::{
    MethodResolution, CollectionMethod, MethodDispatcher,
    UserMethodResolver, DerivedMethodResolver, CollectionMethodResolver, BuiltinMethodResolver,
};

impl Evaluator<'_> {
    /// Evaluate a method call using the Chain of Responsibility pattern.
    ///
    /// Methods are resolved in priority order:
    /// 1. User-defined methods from impl blocks (priority 0)
    /// 2. Derived methods from `#[derive(...)]` (priority 1)
    /// 3. Collection methods requiring evaluator (priority 2)
    /// 4. Built-in methods in `MethodRegistry` (priority 3)
    pub(super) fn eval_method_call(&mut self, receiver: Value, method: Name, args: Vec<Value>) -> EvalResult {
        let method_name = self.interner.lookup(method);
        let type_name = self.get_value_type_name(&receiver);

        // Resolve the method using the resolver chain
        let resolution = self.resolve_method(&receiver, &type_name, method_name);

        // Execute based on resolution type
        match resolution {
            MethodResolution::User(user_method) => {
                self.eval_user_method(receiver, &user_method, &args)
            }
            MethodResolution::Derived(derived_info) => {
                self.eval_derived_method(receiver, &derived_info, &args)
            }
            MethodResolution::Collection(collection_method) => {
                self.eval_collection_method(receiver, collection_method, &args)
            }
            MethodResolution::Builtin => {
                self.method_registry.dispatch(receiver, method_name, args)
            }
            MethodResolution::NotFound => {
                // This shouldn't happen as BuiltinResolver always returns Builtin,
                // but if it does, fall back to the method registry which will
                // produce an appropriate error
                self.method_registry.dispatch(receiver, method_name, args)
            }
        }
    }

    /// Resolve a method using the resolver chain.
    ///
    /// Creates a dispatcher with all resolvers and tries them in priority order.
    fn resolve_method(&self, receiver: &Value, type_name: &str, method_name: &str) -> MethodResolution {
        // Build the dispatcher with all resolvers
        let dispatcher = MethodDispatcher::new(vec![
            Box::new(UserMethodResolver::new(self.user_method_registry.clone())),
            Box::new(DerivedMethodResolver::new(self.user_method_registry.clone())),
            Box::new(CollectionMethodResolver::new()),
            Box::new(BuiltinMethodResolver::new()),
        ]);

        dispatcher.resolve(receiver, type_name, method_name)
    }

    /// Evaluate a collection method that requires evaluator access.
    fn eval_collection_method(
        &mut self,
        receiver: Value,
        method: CollectionMethod,
        args: &[Value],
    ) -> EvalResult {
        match method {
            CollectionMethod::Map => match receiver {
                Value::List(items) => self.eval_list_map(items.as_ref(), args),
                Value::Range(range) => self.eval_range_map(&range, args),
                _ => Err(EvalError::new("map requires a collection")),
            },
            CollectionMethod::Filter => match receiver {
                Value::List(items) => self.eval_list_filter(items.as_ref(), args),
                Value::Range(range) => self.eval_range_filter(&range, args),
                _ => Err(EvalError::new("filter requires a collection")),
            },
            CollectionMethod::Fold => match receiver {
                Value::List(items) => self.eval_list_fold(items.as_ref(), args),
                Value::Range(range) => self.eval_range_fold(&range, args),
                _ => Err(EvalError::new("fold requires a collection")),
            },
            CollectionMethod::Find => match receiver {
                Value::List(items) => self.eval_list_find(items.as_ref(), args),
                _ => Err(EvalError::new("find requires a list")),
            },
            CollectionMethod::Collect => match receiver {
                Value::Range(range) => self.eval_range_collect(&range, args),
                _ => Err(EvalError::new("collect requires a range")),
            },
            CollectionMethod::Any => match receiver {
                Value::List(items) => self.eval_list_any(items.as_ref(), args),
                _ => Err(EvalError::new("any requires a list")),
            },
            CollectionMethod::All => match receiver {
                Value::List(items) => self.eval_list_all(items.as_ref(), args),
                _ => Err(EvalError::new("all requires a list")),
            },
            CollectionMethod::MapEntries => match receiver {
                Value::Map(_) => Err(EvalError::new("map entries not yet implemented")),
                _ => Err(EvalError::new("map entries requires a map")),
            },
            CollectionMethod::FilterEntries => match receiver {
                Value::Map(_) => Err(EvalError::new("filter entries not yet implemented")),
                _ => Err(EvalError::new("filter entries requires a map")),
            },
        }
    }

    // Individual collection method implementations

    fn eval_list_map(&mut self, items: &[Value], args: &[Value]) -> EvalResult {
        if args.len() != 1 {
            return Err(wrong_arg_count("map", 1, args.len()));
        }
        let transform = args[0].clone();
        let mut result = Vec::with_capacity(items.len());
        for item in items {
            let mapped = self.eval_call(transform.clone(), &[item.clone()])?;
            result.push(mapped);
        }
        Ok(Value::list(result))
    }

    fn eval_list_filter(&mut self, items: &[Value], args: &[Value]) -> EvalResult {
        if args.len() != 1 {
            return Err(wrong_arg_count("filter", 1, args.len()));
        }
        let predicate = args[0].clone();
        let mut result = Vec::new();
        for item in items {
            let keep = self.eval_call(predicate.clone(), &[item.clone()])?;
            if keep.is_truthy() {
                result.push(item.clone());
            }
        }
        Ok(Value::list(result))
    }

    fn eval_list_fold(&mut self, items: &[Value], args: &[Value]) -> EvalResult {
        if args.len() != 2 {
            return Err(wrong_arg_count("fold", 2, args.len()));
        }
        let mut acc = args[0].clone();
        let op = args[1].clone();
        for item in items {
            acc = self.eval_call(op.clone(), &[acc, item.clone()])?;
        }
        Ok(acc)
    }

    fn eval_list_find(&mut self, items: &[Value], args: &[Value]) -> EvalResult {
        if args.len() != 1 {
            return Err(wrong_arg_count("find", 1, args.len()));
        }
        let predicate = args[0].clone();
        for item in items {
            let found = self.eval_call(predicate.clone(), &[item.clone()])?;
            if found.is_truthy() {
                return Ok(Value::some(item.clone()));
            }
        }
        Ok(Value::None)
    }

    fn eval_list_any(&mut self, items: &[Value], args: &[Value]) -> EvalResult {
        if args.len() != 1 {
            return Err(wrong_arg_count("any", 1, args.len()));
        }
        let predicate = args[0].clone();
        for item in items {
            let result = self.eval_call(predicate.clone(), &[item.clone()])?;
            if result.is_truthy() {
                return Ok(Value::Bool(true));
            }
        }
        Ok(Value::Bool(false))
    }

    fn eval_list_all(&mut self, items: &[Value], args: &[Value]) -> EvalResult {
        if args.len() != 1 {
            return Err(wrong_arg_count("all", 1, args.len()));
        }
        let predicate = args[0].clone();
        for item in items {
            let result = self.eval_call(predicate.clone(), &[item.clone()])?;
            if !result.is_truthy() {
                return Ok(Value::Bool(false));
            }
        }
        Ok(Value::Bool(true))
    }

    fn eval_range_collect(&mut self, range: &sigil_patterns::RangeValue, args: &[Value]) -> EvalResult {
        if !args.is_empty() {
            return Err(wrong_arg_count("collect", 0, args.len()));
        }
        let result: Vec<Value> = range.iter().map(Value::Int).collect();
        Ok(Value::list(result))
    }

    fn eval_range_map(&mut self, range: &sigil_patterns::RangeValue, args: &[Value]) -> EvalResult {
        if args.len() != 1 {
            return Err(wrong_arg_count("map", 1, args.len()));
        }
        let transform = args[0].clone();
        let mut result = Vec::new();
        for i in range.iter() {
            let mapped = self.eval_call(transform.clone(), &[Value::Int(i)])?;
            result.push(mapped);
        }
        Ok(Value::list(result))
    }

    fn eval_range_filter(&mut self, range: &sigil_patterns::RangeValue, args: &[Value]) -> EvalResult {
        if args.len() != 1 {
            return Err(wrong_arg_count("filter", 1, args.len()));
        }
        let predicate = args[0].clone();
        let mut result = Vec::new();
        for i in range.iter() {
            let keep = self.eval_call(predicate.clone(), &[Value::Int(i)])?;
            if keep.is_truthy() {
                result.push(Value::Int(i));
            }
        }
        Ok(Value::list(result))
    }

    fn eval_range_fold(&mut self, range: &sigil_patterns::RangeValue, args: &[Value]) -> EvalResult {
        if args.len() != 2 {
            return Err(wrong_arg_count("fold", 2, args.len()));
        }
        let mut acc = args[0].clone();
        let op = args[1].clone();
        for i in range.iter() {
            acc = self.eval_call(op.clone(), &[acc, Value::Int(i)])?;
        }
        Ok(acc)
    }

    /// Get the concrete type name for a value (for method lookup).
    ///
    /// For struct values, returns the actual struct name.
    /// For other values, returns the basic type name.
    pub(super) fn get_value_type_name(&self, value: &Value) -> String {
        match value {
            Value::Struct(s) => self.interner.lookup(s.type_name).to_string(),
            Value::List(_) => "list".to_string(),
            Value::Str(_) => "str".to_string(),
            Value::Int(_) => "int".to_string(),
            Value::Float(_) => "float".to_string(),
            Value::Bool(_) => "bool".to_string(),
            Value::Char(_) => "char".to_string(),
            Value::Byte(_) => "byte".to_string(),
            Value::Map(_) => "map".to_string(),
            Value::Tuple(_) => "tuple".to_string(),
            Value::Some(_) | Value::None => "Option".to_string(),
            Value::Ok(_) | Value::Err(_) => "Result".to_string(),
            Value::Range(_) => "range".to_string(),
            _ => value.type_name().to_string(),
        }
    }

    /// Evaluate a user-defined method from an impl block.
    pub(super) fn eval_user_method(&mut self, receiver: Value, method: &UserMethod, args: &[Value]) -> EvalResult {
        // Method params include 'self' as first parameter
        if method.params.len() != args.len() + 1 {
            return Err(wrong_function_args(method.params.len() - 1, args.len()));
        }

        // Create new environment with captures
        let mut call_env = self.env.child();
        call_env.push_scope();

        // Bind captured variables
        for (name, value) in &method.captures {
            call_env.define(*name, value.clone(), false);
        }

        // Bind 'self' to receiver (first parameter)
        if let Some(&self_param) = method.params.first() {
            call_env.define(self_param, receiver, false);
        }

        // Bind remaining parameters
        for (param, arg) in method.params.iter().skip(1).zip(args.iter()) {
            call_env.define(*param, arg.clone(), false);
        }

        // Evaluate method body using the method's arena.
        // Every method carries its own arena for thread safety.
        let func_arena: &ExprArena = &method.arena;
        let imported_arena = SharedArena::new(func_arena.clone());
        let mut call_evaluator = Evaluator::with_imported_arena(
            self.interner, func_arena, call_env, imported_arena, self.user_method_registry.clone()
        );
        let result = call_evaluator.eval(method.body);
        call_evaluator.env.pop_scope();
        result
    }

    /// Evaluate a derived method (from `#[derive(...)]`).
    ///
    /// These methods operate directly on struct field values rather than
    /// having an expression body.
    pub(super) fn eval_derived_method(
        &mut self,
        receiver: Value,
        info: &DerivedMethodInfo,
        args: &[Value],
    ) -> EvalResult {
        match info.trait_kind {
            DerivedTrait::Eq => self.eval_derived_eq(receiver, info, args),
            DerivedTrait::Clone => self.eval_derived_clone(receiver, info),
            DerivedTrait::Hashable => self.eval_derived_hash(receiver, info),
            DerivedTrait::Printable => self.eval_derived_to_string(receiver, info),
            DerivedTrait::Default => self.eval_derived_default(info),
        }
    }

    /// Evaluate derived `eq` method for structs.
    ///
    /// Compares each field recursively.
    fn eval_derived_eq(
        &self,
        receiver: Value,
        info: &DerivedMethodInfo,
        args: &[Value],
    ) -> EvalResult {
        // eq takes one argument: the other value to compare
        if args.len() != 1 {
            return Err(wrong_function_args(1, args.len()));
        }

        let other = &args[0];

        // Both must be structs
        let (self_struct, other_struct) = match (&receiver, other) {
            (Value::Struct(s), Value::Struct(o)) => (s, o),
            _ => return Ok(Value::Bool(false)), // Different types are not equal
        };

        // Must be the same type
        if self_struct.type_name != other_struct.type_name {
            return Ok(Value::Bool(false));
        }

        // Compare each field
        for field_name in &info.field_names {
            let self_val = self_struct.get_field(*field_name);
            let other_val = other_struct.get_field(*field_name);

            match (self_val, other_val) {
                (Some(sv), Some(ov)) => {
                    if sv != ov {
                        return Ok(Value::Bool(false));
                    }
                }
                _ => return Ok(Value::Bool(false)), // Missing field
            }
        }

        Ok(Value::Bool(true))
    }

    /// Evaluate derived `clone` method for structs.
    ///
    /// Creates a deep copy of the struct.
    fn eval_derived_clone(&self, receiver: Value, _info: &DerivedMethodInfo) -> EvalResult {
        let struct_val = match receiver {
            Value::Struct(s) => s,
            _ => return Ok(receiver.clone()), // Non-structs just clone directly
        };

        // Clone the struct (Value::Struct already uses Arc for cheap cloning)
        // For a true deep clone, we'd need to recursively clone nested values,
        // but for now we rely on the structural clone behavior of Value.
        Ok(Value::Struct(struct_val.clone()))
    }

    /// Evaluate derived `hash` method for structs.
    ///
    /// Combines hashes of all fields.
    fn eval_derived_hash(&self, receiver: Value, info: &DerivedMethodInfo) -> EvalResult {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let struct_val = match &receiver {
            Value::Struct(s) => s,
            _ => {
                // For non-structs, use a simple hash
                let mut hasher = DefaultHasher::new();
                receiver.type_name().hash(&mut hasher);
                return Ok(Value::Int(hasher.finish() as i64));
            }
        };

        let mut hasher = DefaultHasher::new();

        // Hash the type name
        self.interner.lookup(struct_val.type_name).hash(&mut hasher);

        // Hash each field value
        for field_name in &info.field_names {
            if let Some(val) = struct_val.get_field(*field_name) {
                val.hash(&mut hasher);
            }
        }

        Ok(Value::Int(hasher.finish() as i64))
    }

    /// Evaluate derived `to_string` method for structs.
    ///
    /// Produces a string representation like "Point { x: 10, y: 20 }".
    fn eval_derived_to_string(&self, receiver: Value, info: &DerivedMethodInfo) -> EvalResult {
        let struct_val = match &receiver {
            Value::Struct(s) => s,
            _ => return Ok(Value::string(format!("{receiver}"))),
        };

        let type_name = self.interner.lookup(struct_val.type_name);
        let mut fields = Vec::new();

        for field_name in &info.field_names {
            let field_str = self.interner.lookup(*field_name);
            if let Some(val) = struct_val.get_field(*field_name) {
                fields.push(format!("{field_str}: {val}"));
            }
        }

        let result = format!("{type_name} {{ {} }}", fields.join(", "));
        Ok(Value::string(result))
    }

    /// Evaluate derived `default` method for structs.
    ///
    /// Returns the default value for the type.
    /// Note: This is currently a stub - a proper implementation would need
    /// to recursively default-construct each field.
    fn eval_derived_default(&self, _info: &DerivedMethodInfo) -> EvalResult {
        // Default is a static method that doesn't take self.
        // For now, return an error since we'd need type information
        // to construct the default struct.
        Err(sigil_eval::EvalError::new(
            "default() requires type context; use explicit construction instead",
        ))
    }
}
