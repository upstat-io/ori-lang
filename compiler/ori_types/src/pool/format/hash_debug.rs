//! Merkle-hash formatting and recursive hash-tree diagnostics.

use crate::{Idx, Pool, Tag};

impl Pool {
    // Merkle Hash Debug Tooling

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
}
