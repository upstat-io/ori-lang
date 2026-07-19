//! `Interpreter` method dispatch for `map` and `set` values.
//!
//! Map and set dispatch are interpreter methods (not free functions) because
//! non-primitive (user-`Hashable`) keys require `@hash` + `@eq` calls, which
//! need interpreter access.

use ori_patterns::{no_such_method, ControlAction, EvalResult, IteratorValue, MapData, Value};

use super::super::arguments::require_args;
use super::super::compare::{equals_values, hash_value};
use super::super::debug_format::debug_value;
use super::super::length::len_to_value;
use crate::interpreter::Interpreter;

/// A key's resolved bucket placement.
///
/// Primitive keys carry their `to_map_key()` string, which IS the key identity —
/// a single-entry bucket, no `@eq` needed. Non-primitive keys carry the `@hash`
/// bucket string; `@eq` resolves collisions within the bucket.
pub(crate) enum BucketKey {
    Primitive(String),
    Hashed(String),
}

impl Interpreter<'_> {
    /// Resolve a key `Value` to its bucket placement.
    ///
    /// Primitive keys use the `to_map_key()` string. Non-primitive (user-`Hashable`)
    /// keys invoke the user `@hash` via method dispatch and bucket by `h:{hash}`.
    pub(crate) fn resolve_bucket_key(
        &mut self,
        key: &Value,
        method: &str,
    ) -> Result<BucketKey, ControlAction> {
        if let Ok(s) = key.to_map_key() {
            return Ok(BucketKey::Primitive(s));
        }
        // Non-primitive (user-Hashable) key: bucket by the user `@hash`.
        let hash_val = self.eval_method_call(key.clone(), self.op_names.hash, vec![])?;
        let Value::Int(h) = hash_val else {
            return Err(ori_patterns::wrong_arg_type(method, "Hashable key (@hash -> int)").into());
        };
        Ok(BucketKey::Hashed(MapData::hash_bucket_key(h.raw())))
    }

    /// Find the index in a hash bucket whose stored key `@eq`-matches `lookup`.
    ///
    /// Clones candidate keys, drops the `MapData` borrow, then runs the `@eq`
    /// comparison loop (needs `&mut self`), avoiding a borrow conflict.
    pub(crate) fn find_eq_index(
        &mut self,
        data: &MapData,
        bucket_key: &str,
        lookup: &Value,
    ) -> Result<Option<usize>, ControlAction> {
        let candidates: Vec<Value> = match data.bucket_keys(bucket_key) {
            Some(entries) => entries.iter().map(|(k, _)| k.clone()).collect(),
            None => return Ok(None),
        };
        for (idx, stored) in candidates.into_iter().enumerate() {
            let eq = self.eval_method_call(stored, self.op_names.eq, vec![lookup.clone()])?;
            if matches!(eq, Value::Bool(true)) {
                return Ok(Some(idx));
            }
        }
        Ok(None)
    }

    /// Look up a value by key in bucketed map/set storage.
    pub(crate) fn bucket_get<'d>(
        &mut self,
        data: &'d MapData,
        key: &Value,
        method: &str,
    ) -> Result<Option<&'d Value>, ControlAction> {
        match self.resolve_bucket_key(key, method)? {
            BucketKey::Primitive(s) => Ok(data.get_primitive(&s)),
            BucketKey::Hashed(bk) => {
                let idx = self.find_eq_index(data, &bk, key)?;
                Ok(idx.and_then(|i| data.bucket_value_at(&bk, i)))
            }
        }
    }

    /// Test key membership in bucketed map/set storage.
    pub(crate) fn bucket_contains(
        &mut self,
        data: &MapData,
        key: &Value,
        method: &str,
    ) -> Result<bool, ControlAction> {
        match self.resolve_bucket_key(key, method)? {
            BucketKey::Primitive(s) => Ok(data.contains_primitive(&s)),
            BucketKey::Hashed(bk) => Ok(self.find_eq_index(data, &bk, key)?.is_some()),
        }
    }

    /// Insert `(key, value)` into bucketed map/set storage in place.
    pub(crate) fn bucket_insert(
        &mut self,
        data: &mut MapData,
        key: Value,
        value: Value,
        method: &str,
    ) -> Result<(), ControlAction> {
        match self.resolve_bucket_key(&key, method)? {
            BucketKey::Primitive(s) => data.insert_primitive(s, value),
            BucketKey::Hashed(bk) => {
                let at = self.find_eq_index(data, &bk, &key)?;
                data.bucket_put(bk, at, key, value);
            }
        }
        Ok(())
    }

    /// Remove a key from bucketed map/set storage in place.
    pub(crate) fn bucket_remove(
        &mut self,
        data: &mut MapData,
        key: &Value,
        method: &str,
    ) -> Result<(), ControlAction> {
        match self.resolve_bucket_key(key, method)? {
            BucketKey::Primitive(s) => data.remove_primitive(&s),
            BucketKey::Hashed(bk) => {
                if let Some(idx) = self.find_eq_index(data, &bk, key)? {
                    data.bucket_remove_at(&bk, idx);
                }
            }
        }
        Ok(())
    }

    /// Dispatch methods on map values.
    pub(crate) fn dispatch_map_method(
        &mut self,
        receiver: Value,
        method: ori_ir::Name,
        args: Vec<Value>,
    ) -> EvalResult {
        let Value::Map(mut map) = receiver else {
            unreachable!("dispatch_map_method called with non-map receiver")
        };

        let n = self.builtin_method_names;
        let interner = self.interner;

        if method == n.len || method == n.length {
            len_to_value(map.len(), "map")
        } else if method == n.is_empty {
            Ok(Value::Bool(map.is_empty()))
        } else if method == n.contains_key || method == n.contains {
            require_args("contains_key", 1, args.len())?;
            Ok(Value::Bool(self.bucket_contains(
                &map,
                &args[0],
                "contains_key",
            )?))
        } else if method == n.get {
            require_args("get", 1, args.len())?;
            match self.bucket_get(&map, &args[0], "get")? {
                Some(v) => Ok(Value::some(v.clone())),
                None => Ok(Value::None),
            }
        } else if method == n.insert || method == n.updated {
            // `IndexSet.updated` is insert-or-replace, identical to `insert`;
            // map keys never go out of bounds, so neither form panics.
            let name = if method == n.insert {
                "insert"
            } else {
                "updated"
            };
            require_args(name, 2, args.len())?;
            let mut args = args;
            let value = args.swap_remove(1);
            let key = args.swap_remove(0);
            self.bucket_insert(map.make_mut(), key, value, name)?;
            Ok(Value::Map(map))
        } else if method == n.remove {
            require_args("remove", 1, args.len())?;
            self.bucket_remove(map.make_mut(), &args[0], "remove")?;
            Ok(Value::Map(map))
        } else if method == n.keys {
            let keys: Vec<Value> = map.keys().cloned().collect();
            Ok(Value::list(keys))
        } else if method == n.values {
            let values: Vec<Value> = map.values().cloned().collect();
            Ok(Value::list(values))
        } else if method == n.entries {
            require_args("entries", 0, args.len())?;
            let pairs: Vec<Value> = map
                .iter()
                .map(|(k, v)| Value::tuple(vec![k.clone(), v.clone()]))
                .collect();
            Ok(Value::list(pairs))
        } else if method == n.iter {
            require_args("iter", 0, args.len())?;
            Ok(Value::iterator(IteratorValue::from_map(&map)))
        // Eq trait - deep value equality (order-independent)
        } else if method == n.equals {
            require_args("equals", 1, args.len())?;
            let receiver = Value::Map(map);
            let eq = equals_values(&receiver, &args[0], interner)?;
            Ok(Value::Bool(eq))
        // Hashable trait - order-independent XOR of entry hashes
        } else if method == n.hash {
            require_args("hash", 0, args.len())?;
            Ok(Value::int(hash_value(&Value::Map(map), interner)?))
        } else if method == n.clone_ {
            require_args("clone", 0, args.len())?;
            Ok(Value::Map(map))
        // Debug trait - shows map structure
        } else if method == n.debug {
            require_args("debug", 0, args.len())?;
            Ok(Value::string(debug_value(&Value::Map(map), interner)))
        // Printable to_str — compound rendering via display_value, mirroring the
        // dispatch_builtin_method compound-printable blanket. Map routes here
        // (not through that blanket), so it must carry its own to_str arm; the
        // non-primitive {x:spec} desugar routes Printable-only receivers through
        // to_str().
        } else if method == n.to_str {
            require_args("to_str", 0, args.len())?;
            Ok(Value::string(Value::Map(map).display_value()))
        // Additional map methods (cold path — string-based dispatch)
        } else {
            let method_str = interner.lookup(method);
            self.dispatch_map_method_str(map, method_str, &args)
        }
    }

    /// String-based dispatch for map methods not covered by `Name`-based dispatch.
    fn dispatch_map_method_str(
        &mut self,
        mut map: ori_patterns::Heap<MapData>,
        method: &str,
        args: &[Value],
    ) -> EvalResult {
        match method {
            "merge" => {
                require_args("merge", 1, args.len())?;
                let Value::Map(other) = args[0].clone() else {
                    return Err(ori_patterns::wrong_arg_type("merge", "Map").into());
                };
                let entries: Vec<(Value, Value)> =
                    other.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
                for (k, v) in entries {
                    self.bucket_insert(map.make_mut(), k, v, "merge")?;
                }
                Ok(Value::Map(map))
            }
            "update" => {
                // update(key, f) requires a closure — arg count check
                require_args("update", 2, args.len())?;
                Err(ori_patterns::wrong_arg_type("update", "function").into())
            }
            // Higher-order methods requiring closures (dispatched by CollectionMethodResolver
            // in production; recognized here so dispatch coverage test sees non-UndefinedMethod)
            "filter" | "map" => {
                require_args(method, 1, args.len())?;
                Err(ori_patterns::wrong_arg_type(method, "function").into())
            }
            _ => Err(no_such_method(method, "map").into()),
        }
    }

    /// Build a set by retaining elements of `data` that pass `keep`.
    ///
    /// `keep(self, elem)` decides membership; used by intersection / difference.
    /// Element bucket placement is recomputed via `@hash` so the result is a
    /// well-formed `MapData` (the elements may be non-primitive).
    fn set_filter(
        &mut self,
        data: &MapData,
        method: &str,
        mut keep: impl FnMut(&mut Self, &Value) -> Result<bool, ControlAction>,
    ) -> Result<MapData, ControlAction> {
        let elems: Vec<Value> = data.values().cloned().collect();
        let mut result = MapData::new();
        for elem in elems {
            if keep(self, &elem)? {
                self.bucket_insert(&mut result, elem.clone(), elem, method)?;
            }
        }
        Ok(result)
    }

    /// Dispatch methods on set values.
    pub(crate) fn dispatch_set_method(
        &mut self,
        receiver: Value,
        method: ori_ir::Name,
        args: Vec<Value>,
    ) -> EvalResult {
        let Value::Set(mut items) = receiver else {
            unreachable!("dispatch_set_method called with non-set receiver")
        };

        let n = self.builtin_method_names;
        let interner = self.interner;

        if method == n.iter {
            require_args("iter", 0, args.len())?;
            let receiver = Value::Set(items);
            match IteratorValue::from_value(&receiver) {
                Some(iter) => Ok(Value::iterator(iter)),
                None => unreachable!("Set is always iterable"),
            }
        } else if method == n.len || method == n.length {
            require_args("len", 0, args.len())?;
            len_to_value(items.len(), "set")
        } else if method == n.is_empty {
            require_args("is_empty", 0, args.len())?;
            Ok(Value::Bool(items.is_empty()))
        } else if method == n.contains {
            require_args("contains", 1, args.len())?;
            Ok(Value::Bool(
                self.bucket_contains(&items, &args[0], "contains")?,
            ))
        } else if method == n.insert {
            require_args("insert", 1, args.len())?;
            let mut args = args;
            let elem = args.swap_remove(0);
            self.bucket_insert(items.make_mut(), elem.clone(), elem, "insert")?;
            Ok(Value::Set(items))
        } else if method == n.remove {
            require_args("remove", 1, args.len())?;
            self.bucket_remove(items.make_mut(), &args[0], "remove")?;
            Ok(Value::Set(items))
        } else if method == n.union {
            require_args("union", 1, args.len())?;
            let Value::Set(other) = args[0].clone() else {
                return Err(ori_patterns::wrong_arg_type("union", "Set").into());
            };
            let other_elems: Vec<Value> = other.values().cloned().collect();
            let result = items.make_mut();
            for elem in other_elems {
                if !self.bucket_contains(result, &elem, "union")? {
                    self.bucket_insert(result, elem.clone(), elem, "union")?;
                }
            }
            Ok(Value::Set(items))
        } else if method == n.intersection {
            require_args("intersection", 1, args.len())?;
            let Value::Set(other) = args[0].clone() else {
                return Err(ori_patterns::wrong_arg_type("intersection", "Set").into());
            };
            let result = self.set_filter(&items, "intersection", |slf, elem| {
                slf.bucket_contains(&other, elem, "intersection")
            })?;
            Ok(Value::set_data(result))
        } else if method == n.difference {
            require_args("difference", 1, args.len())?;
            let Value::Set(other) = args[0].clone() else {
                return Err(ori_patterns::wrong_arg_type("difference", "Set").into());
            };
            let result = self.set_filter(&items, "difference", |slf, elem| {
                Ok(!slf.bucket_contains(&other, elem, "difference")?)
            })?;
            Ok(Value::set_data(result))
        } else if method == n.to_list || method == n.into {
            require_args("to_list", 0, args.len())?;
            Ok(Value::list(items.values().cloned().collect()))
        // Clone trait - deep clone of set
        } else if method == n.clone_ {
            require_args("clone", 0, args.len())?;
            Ok(Value::Set(items))
        // Eq trait - deep value equality
        } else if method == n.equals {
            require_args("equals", 1, args.len())?;
            let receiver = Value::Set(items);
            let eq = equals_values(&receiver, &args[0], interner)?;
            Ok(Value::Bool(eq))
        // Hashable trait - order-independent XOR of element hashes
        } else if method == n.hash {
            require_args("hash", 0, args.len())?;
            Ok(Value::int(hash_value(&Value::Set(items), interner)?))
        // Debug trait - shows set structure
        } else if method == n.debug {
            require_args("debug", 0, args.len())?;
            Ok(Value::string(debug_value(&Value::Set(items), interner)))
        // Printable to_str — compound rendering via display_value, mirroring the
        // dispatch_builtin_method compound-printable blanket. Set routes here
        // (not through that blanket), so it must carry its own to_str arm.
        } else if method == n.to_str {
            require_args("to_str", 0, args.len())?;
            Ok(Value::string(Value::Set(items).display_value()))
        } else {
            Err(no_such_method(interner.lookup(method), "Set").into())
        }
    }
}
