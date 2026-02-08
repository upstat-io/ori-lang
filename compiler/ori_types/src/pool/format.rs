//! Type formatting for debugging and error messages.

#![allow(clippy::format_push_string)] // Debug formatting prioritizes clarity over allocation

use crate::{Idx, Pool, Tag, VarState};
use ori_ir::StringInterner;

impl Pool {
    /// Format a type as a human-readable string.
    ///
    /// This is used for error messages and debugging output.
    pub fn format_type(&self, idx: Idx) -> String {
        let mut buf = String::new();
        self.format_type_into(idx, &mut buf);
        buf
    }

    /// Format a type into an existing string buffer.
    pub fn format_type_into(&self, idx: Idx, buf: &mut String) {
        match self.tag(idx) {
            // Primitives
            Tag::Int => buf.push_str("int"),
            Tag::Float => buf.push_str("float"),
            Tag::Bool => buf.push_str("bool"),
            Tag::Str => buf.push_str("str"),
            Tag::Char => buf.push_str("char"),
            Tag::Byte => buf.push_str("byte"),
            Tag::Unit => buf.push_str("()"),
            Tag::Never => buf.push_str("never"),
            Tag::Error => buf.push_str("<error>"),
            Tag::Duration => buf.push_str("duration"),
            Tag::Size => buf.push_str("size"),
            Tag::Ordering => buf.push_str("ordering"),

            // Simple containers
            Tag::List => {
                buf.push('[');
                let child = Idx::from_raw(self.data(idx));
                self.format_type_into(child, buf);
                buf.push(']');
            }
            Tag::Option => {
                let inner = Idx::from_raw(self.data(idx));
                self.format_type_into(inner, buf);
                buf.push('?');
            }
            Tag::Set => {
                buf.push('{');
                let elem = Idx::from_raw(self.data(idx));
                self.format_type_into(elem, buf);
                buf.push('}');
            }
            Tag::Channel => {
                buf.push_str("chan<");
                let elem = Idx::from_raw(self.data(idx));
                self.format_type_into(elem, buf);
                buf.push('>');
            }
            Tag::Range => {
                buf.push_str("range<");
                let elem = Idx::from_raw(self.data(idx));
                self.format_type_into(elem, buf);
                buf.push('>');
            }

            // Two-child containers
            Tag::Map => {
                buf.push('{');
                self.format_type_into(self.map_key(idx), buf);
                buf.push_str(": ");
                self.format_type_into(self.map_value(idx), buf);
                buf.push('}');
            }
            Tag::Result => {
                buf.push_str("result<");
                self.format_type_into(self.result_ok(idx), buf);
                buf.push_str(", ");
                self.format_type_into(self.result_err(idx), buf);
                buf.push('>');
            }

            // Function
            Tag::Function => {
                let params = self.function_params(idx);
                let ret = self.function_return(idx);

                buf.push('(');
                for (i, &param) in params.iter().enumerate() {
                    if i > 0 {
                        buf.push_str(", ");
                    }
                    self.format_type_into(param, buf);
                }
                buf.push_str(") -> ");
                self.format_type_into(ret, buf);
            }

            // Tuple
            Tag::Tuple => {
                let elems = self.tuple_elems(idx);
                buf.push('(');
                for (i, &elem) in elems.iter().enumerate() {
                    if i > 0 {
                        buf.push_str(", ");
                    }
                    self.format_type_into(elem, buf);
                }
                buf.push(')');
            }

            // Type variables
            Tag::Var => {
                let var_id = self.data(idx);
                match self.var_state(var_id) {
                    VarState::Unbound {
                        name: Some(name),
                        id,
                        ..
                    } => {
                        buf.push_str(&format!("${}", name.raw()));
                        buf.push_str(&format!("#{id}"));
                    }
                    VarState::Unbound { id, .. } => {
                        buf.push_str(&format!("$t{id}"));
                    }
                    VarState::Link { target } => {
                        // Follow the link
                        self.format_type_into(*target, buf);
                    }
                    VarState::Rigid { name } => {
                        buf.push_str(&format!("'{}", name.raw()));
                    }
                    VarState::Generalized { id, name } => {
                        if let Some(n) = name {
                            buf.push_str(&format!("forall {}", n.raw()));
                        } else {
                            buf.push_str(&format!("forall t{id}"));
                        }
                    }
                }
            }

            Tag::BoundVar => {
                let var_id = self.data(idx);
                buf.push_str(&format!("$b{var_id}"));
            }

            Tag::RigidVar => {
                let var_id = self.data(idx);
                match self.var_state(var_id) {
                    VarState::Rigid { name } => {
                        buf.push_str(&format!("'{}", name.raw()));
                    }
                    _ => {
                        buf.push_str(&format!("'r{var_id}"));
                    }
                }
            }

            // Scheme
            Tag::Scheme => {
                let vars = self.scheme_vars(idx);
                let body = self.scheme_body(idx);

                buf.push_str("forall ");
                for (i, &var) in vars.iter().enumerate() {
                    if i > 0 {
                        buf.push_str(", ");
                    }
                    buf.push_str(&format!("t{var}"));
                }
                buf.push_str(". ");
                self.format_type_into(body, buf);
            }

            // Named types (simplified - would need string interner for real names)
            Tag::Named => {
                let extra_idx = self.data(idx) as usize;
                let name_lo = self.extra[extra_idx];
                let name_hi = self.extra[extra_idx + 1];
                let name_bits = u64::from(name_lo) | (u64::from(name_hi) << 32);
                buf.push_str(&format!("Named#{name_bits}"));
            }

            Tag::Applied => {
                let extra_idx = self.data(idx) as usize;
                let name_lo = self.extra[extra_idx];
                let name_hi = self.extra[extra_idx + 1];
                let name_bits = u64::from(name_lo) | (u64::from(name_hi) << 32);
                let arg_count = self.extra[extra_idx + 2] as usize;

                buf.push_str(&format!("Applied#{name_bits}<"));
                for i in 0..arg_count {
                    if i > 0 {
                        buf.push_str(", ");
                    }
                    let arg_idx = Idx::from_raw(self.extra[extra_idx + 3 + i]);
                    self.format_type_into(arg_idx, buf);
                }
                buf.push('>');
            }

            Tag::Alias => buf.push_str("<alias>"),
            Tag::Struct => {
                let extra_idx = self.data(idx) as usize;
                let name_lo = self.extra[extra_idx];
                let name_hi = self.extra[extra_idx + 1];
                let name_bits = u64::from(name_lo) | (u64::from(name_hi) << 32);
                buf.push_str(&format!("Struct#{name_bits}"));
            }
            Tag::Enum => {
                let extra_idx = self.data(idx) as usize;
                let name_lo = self.extra[extra_idx];
                let name_hi = self.extra[extra_idx + 1];
                let name_bits = u64::from(name_lo) | (u64::from(name_hi) << 32);
                buf.push_str(&format!("Enum#{name_bits}"));
            }
            Tag::Projection => buf.push_str("<projection>"),
            Tag::ModuleNs => buf.push_str("<module>"),
            Tag::Infer => buf.push_str("<infer>"),
            Tag::SelfType => buf.push_str("Self"),
        }
    }

    /// Format a type as a human-readable string, resolving named types via the interner.
    pub fn format_type_resolved(&self, idx: Idx, interner: &StringInterner) -> String {
        let mut buf = String::new();
        self.format_type_into_resolved(idx, interner, &mut buf);
        buf
    }

    /// Format a type into an existing buffer, resolving named types via the interner.
    fn format_type_into_resolved(&self, idx: Idx, interner: &StringInterner, buf: &mut String) {
        match self.tag(idx) {
            Tag::Named => {
                let name = self.named_name(idx);
                buf.push_str(interner.lookup(name));
            }
            Tag::Applied => {
                let name = self.applied_name(idx);
                buf.push_str(interner.lookup(name));
                let args = self.applied_args(idx);
                buf.push('<');
                for (i, &arg) in args.iter().enumerate() {
                    if i > 0 {
                        buf.push_str(", ");
                    }
                    self.format_type_into_resolved(arg, interner, buf);
                }
                buf.push('>');
            }
            // For all other tags, delegate to the base formatter.
            // Re-dispatch only types that can contain Named/Applied children.
            Tag::List | Tag::Option | Tag::Set | Tag::Channel | Tag::Range => {
                self.format_type_into_resolved_container(idx, interner, buf);
            }
            Tag::Map | Tag::Result => {
                self.format_type_into_resolved_two_child(idx, interner, buf);
            }
            Tag::Function => {
                let params = self.function_params(idx);
                let ret = self.function_return(idx);
                buf.push('(');
                for (i, &param) in params.iter().enumerate() {
                    if i > 0 {
                        buf.push_str(", ");
                    }
                    self.format_type_into_resolved(param, interner, buf);
                }
                buf.push_str(") -> ");
                self.format_type_into_resolved(ret, interner, buf);
            }
            Tag::Tuple => {
                let elems = self.tuple_elems(idx);
                buf.push('(');
                for (i, &elem) in elems.iter().enumerate() {
                    if i > 0 {
                        buf.push_str(", ");
                    }
                    self.format_type_into_resolved(elem, interner, buf);
                }
                buf.push(')');
            }
            Tag::Var => {
                let var_id = self.data(idx);
                match self.var_state(var_id) {
                    VarState::Link { target } => {
                        self.format_type_into_resolved(*target, interner, buf);
                    }
                    _ => self.format_type_into(idx, buf),
                }
            }
            Tag::Scheme => {
                let body = self.scheme_body(idx);
                // For display, show scheme vars then recurse into body
                let vars = self.scheme_vars(idx);
                buf.push_str("forall ");
                for (i, &var) in vars.iter().enumerate() {
                    if i > 0 {
                        buf.push_str(", ");
                    }
                    buf.push_str(&format!("t{var}"));
                }
                buf.push_str(". ");
                self.format_type_into_resolved(body, interner, buf);
            }
            Tag::Struct => {
                let name = self.struct_name(idx);
                buf.push_str(interner.lookup(name));
            }
            Tag::Enum => {
                let name = self.enum_name(idx);
                buf.push_str(interner.lookup(name));
            }
            // Leaf types — no children to recurse into
            _ => self.format_type_into(idx, buf),
        }
    }

    /// Helper: format single-child containers with interner resolution.
    fn format_type_into_resolved_container(
        &self,
        idx: Idx,
        interner: &StringInterner,
        buf: &mut String,
    ) {
        let child = Idx::from_raw(self.data(idx));
        match self.tag(idx) {
            Tag::List => {
                buf.push('[');
                self.format_type_into_resolved(child, interner, buf);
                buf.push(']');
            }
            Tag::Option => {
                self.format_type_into_resolved(child, interner, buf);
                buf.push('?');
            }
            Tag::Set => {
                buf.push('{');
                self.format_type_into_resolved(child, interner, buf);
                buf.push('}');
            }
            Tag::Channel => {
                buf.push_str("chan<");
                self.format_type_into_resolved(child, interner, buf);
                buf.push('>');
            }
            Tag::Range => {
                buf.push_str("range<");
                self.format_type_into_resolved(child, interner, buf);
                buf.push('>');
            }
            _ => unreachable!(),
        }
    }

    /// Helper: format two-child containers with interner resolution.
    fn format_type_into_resolved_two_child(
        &self,
        idx: Idx,
        interner: &StringInterner,
        buf: &mut String,
    ) {
        match self.tag(idx) {
            Tag::Map => {
                buf.push('{');
                self.format_type_into_resolved(self.map_key(idx), interner, buf);
                buf.push_str(": ");
                self.format_type_into_resolved(self.map_value(idx), interner, buf);
                buf.push('}');
            }
            Tag::Result => {
                buf.push_str("result<");
                self.format_type_into_resolved(self.result_ok(idx), interner, buf);
                buf.push_str(", ");
                self.format_type_into_resolved(self.result_err(idx), interner, buf);
                buf.push('>');
            }
            _ => unreachable!(),
        }
    }

    /// Get a short description of the type category.
    pub fn type_category(&self, idx: Idx) -> &'static str {
        match self.tag(idx) {
            Tag::Int | Tag::Float | Tag::Bool | Tag::Str | Tag::Char | Tag::Byte => "primitive",
            Tag::Unit => "unit type",
            Tag::Never => "never type",
            Tag::Error => "error type",
            Tag::Duration | Tag::Size | Tag::Ordering => "built-in type",
            Tag::List => "list",
            Tag::Option => "option",
            Tag::Set => "set",
            Tag::Channel => "channel",
            Tag::Range => "range",
            Tag::Map => "map",
            Tag::Result => "result",
            Tag::Function => "function",
            Tag::Tuple => "tuple",
            Tag::Var | Tag::BoundVar | Tag::RigidVar => "type variable",
            Tag::Scheme => "type scheme",
            Tag::Named | Tag::Applied | Tag::Alias => "named type",
            Tag::Struct => "struct",
            Tag::Enum => "enum",
            Tag::Projection => "type projection",
            Tag::ModuleNs => "module",
            Tag::Infer => "inference variable",
            Tag::SelfType => "Self type",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_primitives() {
        let pool = Pool::new();

        assert_eq!(pool.format_type(Idx::INT), "int");
        assert_eq!(pool.format_type(Idx::FLOAT), "float");
        assert_eq!(pool.format_type(Idx::BOOL), "bool");
        assert_eq!(pool.format_type(Idx::STR), "str");
        assert_eq!(pool.format_type(Idx::CHAR), "char");
        assert_eq!(pool.format_type(Idx::UNIT), "()");
        assert_eq!(pool.format_type(Idx::NEVER), "never");
        assert_eq!(pool.format_type(Idx::ERROR), "<error>");
    }

    #[test]
    fn format_containers() {
        let mut pool = Pool::new();

        let list_int = pool.list(Idx::INT);
        assert_eq!(pool.format_type(list_int), "[int]");

        let opt_str = pool.option(Idx::STR);
        assert_eq!(pool.format_type(opt_str), "str?");

        let set_bool = pool.set(Idx::BOOL);
        assert_eq!(pool.format_type(set_bool), "{bool}");
    }

    #[test]
    fn format_two_child() {
        let mut pool = Pool::new();

        let map_ty = pool.map(Idx::STR, Idx::INT);
        assert_eq!(pool.format_type(map_ty), "{str: int}");

        let result_ty = pool.result(Idx::INT, Idx::STR);
        assert_eq!(pool.format_type(result_ty), "result<int, str>");
    }

    #[test]
    fn format_function() {
        let mut pool = Pool::new();

        let fn_ty = pool.function(&[Idx::INT, Idx::STR], Idx::BOOL);
        assert_eq!(pool.format_type(fn_ty), "(int, str) -> bool");

        let nullary = pool.function0(Idx::UNIT);
        assert_eq!(pool.format_type(nullary), "() -> ()");
    }

    #[test]
    fn format_tuple() {
        let mut pool = Pool::new();

        let tuple = pool.tuple(&[Idx::INT, Idx::STR, Idx::BOOL]);
        assert_eq!(pool.format_type(tuple), "(int, str, bool)");
    }

    #[test]
    fn format_nested() {
        let mut pool = Pool::new();

        // [[int]]
        let inner = pool.list(Idx::INT);
        let outer = pool.list(inner);
        assert_eq!(pool.format_type(outer), "[[int]]");

        // (int, [str])?
        let list_str = pool.list(Idx::STR);
        let tuple = pool.tuple(&[Idx::INT, list_str]);
        let opt = pool.option(tuple);
        assert_eq!(pool.format_type(opt), "(int, [str])?");
    }

    #[test]
    fn format_fresh_var() {
        let mut pool = Pool::new();

        let var = pool.fresh_var();
        let formatted = pool.format_type(var);
        assert!(formatted.starts_with("$t"));
    }

    #[test]
    fn format_named_resolved() {
        let mut pool = Pool::new();
        let interner = ori_ir::StringInterner::new();

        let name = interner.intern("Point");
        let named = pool.named(name);
        assert_eq!(pool.format_type_resolved(named, &interner), "Point");
    }

    #[test]
    fn format_named_in_container_resolved() {
        let mut pool = Pool::new();
        let interner = ori_ir::StringInterner::new();

        let name = interner.intern("Point");
        let named = pool.named(name);
        let list = pool.list(named);
        assert_eq!(pool.format_type_resolved(list, &interner), "[Point]");

        let opt = pool.option(named);
        assert_eq!(pool.format_type_resolved(opt, &interner), "Point?");
    }

    #[test]
    fn format_named_without_interner_shows_raw() {
        let mut pool = Pool::new();
        let interner = ori_ir::StringInterner::new();

        let name = interner.intern("Point");
        let named = pool.named(name);
        // Without interner, shows raw index
        assert!(pool.format_type(named).starts_with("Named#"));
    }
}
