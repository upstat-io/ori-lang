//! Type formatting with resolved names and pool-index annotations.

use ori_ir::StringInterner;

use crate::{Idx, Pool, Tag, VarState};

impl Pool {
    /// Format a type with its resolved pool `Idx` annotations, for debugging the
    /// monomorphization / interning surface. Each type renders as `<name>#<idx>`;
    /// a composite additionally shows its resolved body idx (`=>#<body>` when the
    /// body interns at a different idx) and recursively each field/payload as
    /// `<field>: <name>#<idx>`. Reveals nested-generic instantiation DUPLICATION
    /// (two distinct bodies for one surface type, e.g. `Wrap<int>#217` vs
    /// `#231`) and an UN-SUBSTITUTED generic field (a field resolving to a bare
    /// param `T#<idx>` instead of the concrete arg). Consumed by the
    /// `ORI_DUMP_AFTER_TYPECK` dump under `ORI_DUMP_TYPE_IDX=1`.
    pub fn format_type_with_idx(&self, idx: Idx, interner: &StringInterner) -> String {
        let mut buf = String::new();
        self.format_type_idx_into(idx, interner, &mut buf, 0);
        buf
    }

    fn format_type_idx_into(
        &self,
        idx: Idx,
        interner: &StringInterner,
        buf: &mut String,
        depth: usize,
    ) {
        use std::fmt::Write;
        let name = self.format_type_resolved(idx, interner);
        let _ = write!(buf, "{name}#{}", idx.raw());
        // Bound recursion: deep / cyclic composites stop at a fixed depth.
        if depth >= 4 {
            return;
        }
        let resolved = self.resolve_fully(idx);
        match self.tag(resolved) {
            Tag::Struct => {
                if resolved != idx {
                    let _ = write!(buf, "=>#{}", resolved.raw());
                }
                buf.push('{');
                for (i, (fname, fty)) in self.struct_fields(resolved).into_iter().enumerate() {
                    if i > 0 {
                        buf.push_str(", ");
                    }
                    let _ = write!(buf, "{}: ", interner.lookup(fname));
                    self.format_type_idx_into(fty, interner, buf, depth + 1);
                }
                buf.push('}');
            }
            Tag::Enum => {
                if resolved != idx {
                    let _ = write!(buf, "=>#{}", resolved.raw());
                }
                buf.push('[');
                for (i, (vname, payloads)) in self.enum_variants(resolved).into_iter().enumerate() {
                    if i > 0 {
                        buf.push_str(" | ");
                    }
                    let _ = write!(buf, "{}", interner.lookup(vname));
                    if !payloads.is_empty() {
                        buf.push('(');
                        for (j, pty) in payloads.into_iter().enumerate() {
                            if j > 0 {
                                buf.push_str(", ");
                            }
                            self.format_type_idx_into(pty, interner, buf, depth + 1);
                        }
                        buf.push(')');
                    }
                }
                buf.push(']');
            }
            _ => {}
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
}
