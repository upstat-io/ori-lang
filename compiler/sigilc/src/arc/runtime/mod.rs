// Runtime module for ARC Memory Management
//
// This module generates the C runtime library for ARC:
// - sigil_arc.h header file
// - sigil_arc.c implementation file
// - Inline versions for embedding

pub mod header;
pub mod impl_gen;
pub mod templates;

use crate::ir::Type;

use super::traits::{ArcConfig, ArcEmitter, ReleasePoint};

// Re-export
pub use header::{generate_header, generate_inline_header};
pub use impl_gen::{generate_impl, generate_inline_impl, generate_string_impl};

/// Default implementation of ArcEmitter that generates C code
pub struct DefaultArcEmitter {
    config: ArcConfig,
}

impl Default for DefaultArcEmitter {
    fn default() -> Self {
        Self::new()
    }
}

impl DefaultArcEmitter {
    /// Create a new emitter with default config
    pub fn new() -> Self {
        DefaultArcEmitter {
            config: ArcConfig::default(),
        }
    }

    /// Create an emitter with custom config
    pub fn with_config(config: ArcConfig) -> Self {
        DefaultArcEmitter { config }
    }

    /// Get the configuration
    pub fn config(&self) -> &ArcConfig {
        &self.config
    }

    /// Get the C type name for a Sigil type
    fn c_type_name(&self, ty: &Type) -> String {
        match ty {
            Type::Int => "int64_t".to_string(),
            Type::Float => "double".to_string(),
            Type::Bool => "bool".to_string(),
            Type::Str => "SigilString".to_string(),
            Type::Void => "void".to_string(),
            Type::List(_) => "SigilList".to_string(),
            Type::Map(_, _) => "SigilMap".to_string(),
            Type::Tuple(elems) => format!("SigilTuple{}", elems.len()),
            Type::Struct { name, .. } => format!("Sigil_{}", name),
            Type::Enum { name, .. } => format!("Sigil_{}", name),
            Type::Named(name) => format!("Sigil_{}", name),
            Type::Function { .. } => "SigilClosure".to_string(),
            Type::Result(_, _) => "SigilResult".to_string(),
            Type::Option(_) => "SigilOption".to_string(),
            Type::Record(_) => "SigilRecord".to_string(),
            Type::Range => "SigilRange".to_string(),
            Type::Any => "SigilAny".to_string(),
            Type::DynTrait(name) => format!("SigilDyn_{}", name),
        }
    }

    /// Get the retain function name for a type
    fn retain_fn(&self, ty: &Type) -> String {
        match ty {
            Type::Str => "sigil_string_retain".to_string(),
            Type::List(_) => "sigil_list_retain".to_string(),
            Type::Map(_, _) => "sigil_map_retain".to_string(),
            Type::Function { .. } => "sigil_closure_retain".to_string(),
            Type::Struct { name, .. } | Type::Enum { name, .. } | Type::Named(name) => {
                format!("sigil_{}_retain", name.to_lowercase())
            }
            _ => "sigil_arc_retain".to_string(),
        }
    }

    /// Get the release function name for a type
    fn release_fn(&self, ty: &Type) -> String {
        match ty {
            Type::Str => "sigil_string_release".to_string(),
            Type::List(_) => "sigil_list_release".to_string(),
            Type::Map(_, _) => "sigil_map_release".to_string(),
            Type::Function { .. } => "sigil_closure_release".to_string(),
            Type::Struct { name, .. } | Type::Enum { name, .. } | Type::Named(name) => {
                format!("sigil_{}_release", name.to_lowercase())
            }
            _ => "sigil_arc_release".to_string(),
        }
    }
}

impl ArcEmitter for DefaultArcEmitter {
    fn emit_runtime_header(&self, config: &ArcConfig) -> String {
        generate_header(config)
    }

    fn emit_runtime_impl(&self, config: &ArcConfig) -> String {
        generate_impl(config)
    }

    fn emit_retain(&self, ty: &Type, var: &str) -> String {
        let fn_name = self.retain_fn(ty);
        format!("{}(&{});", fn_name, var)
    }

    fn emit_release(&self, ty: &Type, var: &str) -> String {
        let fn_name = self.release_fn(ty);
        format!("{}(&{});", fn_name, var)
    }

    fn emit_scope_cleanup(&self, releases: &[ReleasePoint]) -> String {
        if releases.is_empty() {
            return String::new();
        }

        let mut code = String::new();
        code.push_str("    // Scope cleanup (LIFO order)\n");

        for release in releases {
            let release_code = self.emit_release(&release.ty, &format!("local_{}", release.local_id.0));
            code.push_str(&format!("    {}\n", release_code));
        }

        code
    }

    fn emit_alloc(&self, ty: &Type, var: &str) -> String {
        let type_name = self.c_type_name(ty);
        match ty {
            Type::Str => format!("{} {} = sigil_string_new(\"\", 0);", type_name, var),
            Type::List(elem) => {
                let elem_size = self.elem_size(elem);
                format!("{} {} = sigil_list_new({});", type_name, var, elem_size)
            }
            Type::Map(k, v) => {
                let key_size = self.elem_size(k);
                let value_size = self.elem_size(v);
                format!(
                    "{} {} = sigil_map_new({}, {});",
                    type_name, var, key_size, value_size
                )
            }
            Type::Struct { name, .. } => {
                format!("{} {} = sigil_{}_new();", type_name, var, name.to_lowercase())
            }
            _ => format!(
                "{} {} = ({})sigil_arc_alloc(sizeof({}));",
                type_name, var, type_name, type_name
            ),
        }
    }

    fn emit_dealloc(&self, ty: &Type, var: &str) -> String {
        match ty {
            Type::Str => format!("sigil_string_release(&{});", var),
            Type::List(_) => format!("sigil_list_release(&{});", var),
            Type::Map(_, _) => format!("sigil_map_release(&{});", var),
            Type::Function { .. } => format!("sigil_closure_release(&{});", var),
            Type::Struct { name, .. } => format!("sigil_{}_release(&{});", name.to_lowercase(), var),
            _ => format!("sigil_arc_release(&{});", var),
        }
    }
}

impl DefaultArcEmitter {
    /// Get the size expression for an element type
    fn elem_size(&self, ty: &Type) -> String {
        match ty {
            Type::Int => "sizeof(int64_t)".to_string(),
            Type::Float => "sizeof(double)".to_string(),
            Type::Bool => "sizeof(bool)".to_string(),
            Type::Str => "sizeof(SigilString)".to_string(),
            _ => format!("sizeof({})", self.c_type_name(ty)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_emitter_creation() {
        let emitter = DefaultArcEmitter::new();
        assert!(!emitter.config().thread_safe);
    }

    #[test]
    fn test_emit_retain() {
        let emitter = DefaultArcEmitter::new();

        let retain = emitter.emit_retain(&Type::Str, "my_string");
        assert_eq!(retain, "sigil_string_retain(&my_string);");

        let retain = emitter.emit_retain(&Type::List(Box::new(Type::Int)), "my_list");
        assert_eq!(retain, "sigil_list_retain(&my_list);");
    }

    #[test]
    fn test_emit_release() {
        let emitter = DefaultArcEmitter::new();

        let release = emitter.emit_release(&Type::Str, "s");
        assert_eq!(release, "sigil_string_release(&s);");
    }

    #[test]
    fn test_emit_alloc() {
        let emitter = DefaultArcEmitter::new();

        let alloc = emitter.emit_alloc(&Type::Str, "new_str");
        assert!(alloc.contains("SigilString"));
        assert!(alloc.contains("sigil_string_new"));

        let alloc = emitter.emit_alloc(&Type::List(Box::new(Type::Int)), "my_list");
        assert!(alloc.contains("sigil_list_new"));
    }

    #[test]
    fn test_runtime_generation() {
        let emitter = DefaultArcEmitter::new();
        let config = ArcConfig::default();

        let header = emitter.emit_runtime_header(&config);
        assert!(header.contains("SIGIL_ARC_H"));

        let impl_code = emitter.emit_runtime_impl(&config);
        assert!(impl_code.contains("sigil_arc_alloc"));
    }
}
