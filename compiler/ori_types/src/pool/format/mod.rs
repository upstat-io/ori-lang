//! Type formatting for debugging and error messages.

#![allow(
    clippy::format_push_string,
    reason = "debug formatting prioritizes clarity over allocation"
)]

mod hash_debug;
mod resolved;

use crate::{GeneralizedVarState, Idx, Pool, Tag, UnboundVarState, VarState};

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
            Tag::Int
            | Tag::Float
            | Tag::Bool
            | Tag::Str
            | Tag::Char
            | Tag::Byte
            | Tag::Unit
            | Tag::Never
            | Tag::Error
            | Tag::Duration
            | Tag::Size
            | Tag::Ordering
            | Tag::Borrowed
            | Tag::Alias
            | Tag::Projection
            | Tag::ModuleNs
            | Tag::Infer
            | Tag::SelfType => self.format_atomic_type(idx, buf),
            Tag::List
            | Tag::Option
            | Tag::Set
            | Tag::Channel
            | Tag::Range
            | Tag::Iterator
            | Tag::DoubleEndedIterator => self.format_single_child_type(idx, buf),
            Tag::Map | Tag::Result => self.format_pair_type(idx, buf),
            Tag::Function | Tag::Tuple | Tag::Scheme => self.format_sequence_type(idx, buf),
            Tag::Var | Tag::BoundVar | Tag::RigidVar => self.format_variable_type(idx, buf),
            Tag::Named | Tag::Applied | Tag::Struct | Tag::Enum => {
                self.format_named_type(idx, buf);
            }
        }
    }

    fn format_atomic_type(&self, idx: Idx, buf: &mut String) {
        let text = match self.tag(idx) {
            Tag::Int => "int",
            Tag::Float => "float",
            Tag::Bool => "bool",
            Tag::Str => "str",
            Tag::Char => "char",
            Tag::Byte => "byte",
            Tag::Unit => "()",
            Tag::Never => "Never",
            Tag::Error => "<error>",
            Tag::Duration => "Duration",
            Tag::Size => "Size",
            Tag::Ordering => "Ordering",
            Tag::Borrowed => "<borrowed>",
            Tag::Alias => "<alias>",
            Tag::Projection => "<projection>",
            Tag::ModuleNs => "<module>",
            Tag::Infer => "<infer>",
            Tag::SelfType => "Self",
            tag => unreachable!("non-atomic type {tag:?}"),
        };
        buf.push_str(text);
    }

    fn format_single_child_type(&self, idx: Idx, buf: &mut String) {
        let child = Idx::from_raw(self.data(idx));
        let (prefix, suffix) = match self.tag(idx) {
            Tag::List => ("[", "]"),
            Tag::Option => ("", "?"),
            Tag::Set => ("{", "}"),
            Tag::Channel => ("chan<", ">"),
            Tag::Range => ("range<", ">"),
            Tag::Iterator => ("Iterator<", ">"),
            Tag::DoubleEndedIterator => ("DoubleEndedIterator<", ">"),
            tag => unreachable!("non-single-child type {tag:?}"),
        };
        buf.push_str(prefix);
        self.format_type_into(child, buf);
        buf.push_str(suffix);
    }

    fn format_pair_type(&self, idx: Idx, buf: &mut String) {
        let (prefix, first, separator, second, suffix) = match self.tag(idx) {
            Tag::Map => ("{", self.map_key(idx), ": ", self.map_value(idx), "}"),
            Tag::Result => (
                "result<",
                self.result_ok(idx),
                ", ",
                self.result_err(idx),
                ">",
            ),
            tag => unreachable!("non-pair type {tag:?}"),
        };
        buf.push_str(prefix);
        self.format_type_into(first, buf);
        buf.push_str(separator);
        self.format_type_into(second, buf);
        buf.push_str(suffix);
    }

    fn format_sequence_type(&self, idx: Idx, buf: &mut String) {
        match self.tag(idx) {
            Tag::Function => {
                self.format_parenthesized(&self.function_params(idx), buf);
                buf.push_str(" -> ");
                self.format_type_into(self.function_return(idx), buf);
            }
            Tag::Tuple => self.format_parenthesized(&self.tuple_elems(idx), buf),
            Tag::Scheme => {
                buf.push_str("forall ");
                for (position, var) in self.scheme_vars(idx).iter().enumerate() {
                    if position > 0 {
                        buf.push_str(", ");
                    }
                    buf.push_str(&format!("t{var}"));
                }
                buf.push_str(". ");
                self.format_type_into(self.scheme_body(idx), buf);
            }
            tag => unreachable!("non-sequence type {tag:?}"),
        }
    }

    fn format_parenthesized(&self, elements: &[Idx], buf: &mut String) {
        buf.push('(');
        for (position, &element) in elements.iter().enumerate() {
            if position > 0 {
                buf.push_str(", ");
            }
            self.format_type_into(element, buf);
        }
        buf.push(')');
    }

    fn format_variable_type(&self, idx: Idx, buf: &mut String) {
        let var_id = self.data(idx);
        match self.tag(idx) {
            Tag::Var => match self.var_state(var_id) {
                VarState::Unbound(UnboundVarState {
                    name: Some(name),
                    id,
                    ..
                }) => buf.push_str(&format!("${}#{id}", name.raw())),
                VarState::Unbound(UnboundVarState { id, .. }) => {
                    buf.push_str(&format!("$t{id}"));
                }
                VarState::Link { target } => self.format_type_into(*target, buf),
                VarState::Rigid { name } => buf.push_str(&format!("'{}", name.raw())),
                VarState::Generalized(GeneralizedVarState { id, name }) => match name {
                    Some(name) => buf.push_str(&format!("forall {}", name.raw())),
                    None => buf.push_str(&format!("forall t{id}")),
                },
            },
            Tag::BoundVar => buf.push_str(&format!("$b{var_id}")),
            Tag::RigidVar => match self.var_state(var_id) {
                VarState::Rigid { name } => buf.push_str(&format!("'{}", name.raw())),
                _ => buf.push_str(&format!("'r{var_id}")),
            },
            tag => unreachable!("non-variable type {tag:?}"),
        }
    }

    fn format_named_type(&self, idx: Idx, buf: &mut String) {
        let extra_idx = self.data(idx) as usize;
        let name_bits =
            u64::from(self.extra[extra_idx]) | (u64::from(self.extra[extra_idx + 1]) << 32);
        match self.tag(idx) {
            Tag::Named => buf.push_str(&format!("Named#{name_bits}")),
            Tag::Struct => buf.push_str(&format!("Struct#{name_bits}")),
            Tag::Enum => buf.push_str(&format!("Enum#{name_bits}")),
            Tag::Applied => {
                let arg_count = self.extra[extra_idx + 2] as usize;
                buf.push_str(&format!("Applied#{name_bits}<"));
                for position in 0..arg_count {
                    if position > 0 {
                        buf.push_str(", ");
                    }
                    self.format_type_into(Idx::from_raw(self.extra[extra_idx + 3 + position]), buf);
                }
                buf.push('>');
            }
            tag => unreachable!("non-named type {tag:?}"),
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
