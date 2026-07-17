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
            Tag::Never => buf.push_str("Never"),
            Tag::Error => buf.push_str("<error>"),
            Tag::Duration => buf.push_str("Duration"),
            Tag::Size => buf.push_str("Size"),
            Tag::Ordering => buf.push_str("Ordering"),

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
                    VarState::Unbound(UnboundVarState {
                        name: Some(name),
                        id,
                        ..
                    }) => {
                        buf.push_str(&format!("${}", name.raw()));
                        buf.push_str(&format!("#{id}"));
                    }
                    VarState::Unbound(UnboundVarState { id, .. }) => {
                        buf.push_str(&format!("$t{id}"));
                    }
                    VarState::Link { target } => {
                        // Follow the link
                        self.format_type_into(*target, buf);
                    }
                    VarState::Rigid { name } => {
                        buf.push_str(&format!("'{}", name.raw()));
                    }
                    VarState::Generalized(GeneralizedVarState { id, name }) => {
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
