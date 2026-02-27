//! Type formatting for debugging and error messages.

#![allow(
    clippy::format_push_string,
    reason = "debug formatting prioritizes clarity over allocation"
)]

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
    #[expect(
        clippy::too_many_lines,
        reason = "exhaustive Tag dispatch for human-readable type formatting"
    )]
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
            Tag::Iterator => {
                buf.push_str("Iterator<");
                let elem = Idx::from_raw(self.data(idx));
                self.format_type_into(elem, buf);
                buf.push('>');
            }
            Tag::DoubleEndedIterator => {
                buf.push_str("DoubleEndedIterator<");
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

            // Borrowed (reserved, never constructed)
            Tag::Borrowed => buf.push_str("<borrowed>"),

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
            Tag::List
            | Tag::Option
            | Tag::Set
            | Tag::Channel
            | Tag::Range
            | Tag::Iterator
            | Tag::DoubleEndedIterator => {
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
            Tag::Iterator => {
                buf.push_str("Iterator<");
                self.format_type_into_resolved(child, interner, buf);
                buf.push('>');
            }
            Tag::DoubleEndedIterator => {
                buf.push_str("DoubleEndedIterator<");
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

    // === Merkle Hash Debug Tooling ===

    /// Format a type's Merkle hash with tag and child hash breakdown.
    ///
    /// Output format:
    /// ```text
    /// List<int> @ Idx(15): hash=0x1a2b3c4d5e6f7890
    ///   tag=List, child_hash=0x0000000000000001 (int)
    /// ```
    pub fn format_hash(&self, idx: Idx) -> String {
        use std::fmt::Write;
        let mut buf = String::new();
        let tag = self.tag(idx);
        let hash = self.hash(idx);

        let _ = write!(
            buf,
            "{} @ Idx({}): hash=0x{:016x}\n  tag={:?}",
            self.format_type(idx),
            idx.raw(),
            hash,
            tag
        );

        if tag.has_child_in_data() {
            let child = Idx::from_raw(self.data(idx));
            let _ = write!(
                buf,
                ", child_hash=0x{:016x} ({})",
                self.hash(child),
                self.format_type(child)
            );
        } else if tag == Tag::Map {
            let k = self.map_key(idx);
            let v = self.map_value(idx);
            let _ = write!(
                buf,
                ", key_hash=0x{:016x} ({}), value_hash=0x{:016x} ({})",
                self.hash(k),
                self.format_type(k),
                self.hash(v),
                self.format_type(v)
            );
        } else if tag == Tag::Result {
            let ok = self.result_ok(idx);
            let err = self.result_err(idx);
            let _ = write!(
                buf,
                ", ok_hash=0x{:016x} ({}), err_hash=0x{:016x} ({})",
                self.hash(ok),
                self.format_type(ok),
                self.hash(err),
                self.format_type(err)
            );
        } else if tag == Tag::Function {
            let params = self.function_params(idx);
            let ret = self.function_return(idx);
            let _ = write!(buf, ", {} params", params.len());
            for (i, &p) in params.iter().enumerate() {
                let _ = write!(buf, ", p{i}_hash=0x{:016x}", self.hash(p));
            }
            let _ = write!(buf, ", ret_hash=0x{:016x}", self.hash(ret));
        }

        buf
    }

    /// Format a recursive Merkle hash tree showing the full breakdown.
    ///
    /// Output format:
    /// ```text
    /// (int, str) -> bool @ hash=0xABCD...
    ///   Function(2 params, 1 ret)
    ///     param[0]: int @ hash=0x0001 (primitive)
    ///     param[1]: str @ hash=0x0003 (primitive)
    ///     return:   bool @ hash=0x0002 (primitive)
    /// ```
    pub fn debug_hash_tree(&self, idx: Idx) -> String {
        let mut buf = String::new();
        self.debug_hash_tree_inner(idx, 0, &mut buf);
        buf
    }

    /// Recursive helper for `debug_hash_tree`.
    fn debug_hash_tree_inner(&self, idx: Idx, depth: usize, buf: &mut String) {
        use std::fmt::Write;
        let indent = "  ".repeat(depth);
        let tag = self.tag(idx);
        let hash = self.hash(idx);

        let _ = writeln!(
            buf,
            "{indent}{} @ hash=0x{:016x}",
            self.format_type(idx),
            hash
        );

        match tag {
            // Leaf types — no children to recurse
            t if t.is_primitive() || t.is_type_variable() => {}

            // Simple containers — one child
            t if t.has_child_in_data() => {
                let child = Idx::from_raw(self.data(idx));
                let _ = write!(buf, "{indent}  elem: ");
                self.debug_hash_tree_inner(child, depth + 1, buf);
            }

            Tag::Map => {
                let _ = write!(buf, "{indent}  key: ");
                self.debug_hash_tree_inner(self.map_key(idx), depth + 1, buf);
                let _ = write!(buf, "{indent}  value: ");
                self.debug_hash_tree_inner(self.map_value(idx), depth + 1, buf);
            }
            Tag::Result => {
                let _ = write!(buf, "{indent}  ok: ");
                self.debug_hash_tree_inner(self.result_ok(idx), depth + 1, buf);
                let _ = write!(buf, "{indent}  err: ");
                self.debug_hash_tree_inner(self.result_err(idx), depth + 1, buf);
            }

            Tag::Function => {
                let params = self.function_params(idx);
                let ret = self.function_return(idx);
                for (i, &p) in params.iter().enumerate() {
                    let _ = write!(buf, "{indent}  param[{i}]: ");
                    self.debug_hash_tree_inner(p, depth + 1, buf);
                }
                let _ = write!(buf, "{indent}  return: ");
                self.debug_hash_tree_inner(ret, depth + 1, buf);
            }

            Tag::Tuple => {
                let elems = self.tuple_elems(idx);
                for (i, &e) in elems.iter().enumerate() {
                    let _ = write!(buf, "{indent}  [{i}]: ");
                    self.debug_hash_tree_inner(e, depth + 1, buf);
                }
            }

            Tag::Struct => {
                let fields = self.struct_fields(idx);
                for (name, ty) in &fields {
                    let _ = write!(buf, "{indent}  field #{}: ", name.raw());
                    self.debug_hash_tree_inner(*ty, depth + 1, buf);
                }
            }

            Tag::Enum => {
                let variants = self.enum_variants(idx);
                for (vname, fields) in &variants {
                    let _ = writeln!(buf, "{indent}  variant #{}:", vname.raw());
                    for (i, &f) in fields.iter().enumerate() {
                        let _ = write!(buf, "{indent}    [{i}]: ");
                        self.debug_hash_tree_inner(f, depth + 2, buf);
                    }
                }
            }

            Tag::Applied => {
                let args = self.applied_args(idx);
                for (i, &a) in args.iter().enumerate() {
                    let _ = write!(buf, "{indent}  arg[{i}]: ");
                    self.debug_hash_tree_inner(a, depth + 1, buf);
                }
            }

            Tag::Scheme => {
                let body = self.scheme_body(idx);
                let _ = write!(buf, "{indent}  body: ");
                self.debug_hash_tree_inner(body, depth + 1, buf);
            }

            // Remaining tags have no children to recurse
            _ => {}
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
            Tag::Iterator | Tag::DoubleEndedIterator => "iterator",
            Tag::Map => "map",
            Tag::Result => "result",
            Tag::Borrowed => "borrowed reference",
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
mod tests;
