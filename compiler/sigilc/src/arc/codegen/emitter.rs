// ARC Code Emitter
//
// Generates C code for retain/release operations and scope cleanup.
// This module is responsible for emitting the actual ARC operations
// that get inserted into the generated code.

use crate::ir::Type;

use super::super::traits::{ArcConfig, ArcEmitter, ReleasePoint};

/// Default implementation of ArcEmitter
pub struct DefaultCodeEmitter {
    /// Configuration for code generation
    #[allow(dead_code)]
    config: ArcConfig,
}

impl Default for DefaultCodeEmitter {
    fn default() -> Self {
        Self::new()
    }
}

impl DefaultCodeEmitter {
    /// Create a new emitter with default config
    pub fn new() -> Self {
        DefaultCodeEmitter {
            config: ArcConfig::default(),
        }
    }

    /// Create an emitter with custom config
    pub fn with_config(config: ArcConfig) -> Self {
        DefaultCodeEmitter { config }
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

    /// Check if a type needs generic ARC or type-specific operations
    fn needs_generic_arc(&self, ty: &Type) -> bool {
        matches!(
            ty,
            Type::Any | Type::DynTrait(_) | Type::Record(_) | Type::Tuple(_)
        )
    }
}

impl ArcEmitter for DefaultCodeEmitter {
    fn emit_runtime_header(&self, _config: &ArcConfig) -> String {
        // This is delegated to the runtime module
        // Here we just provide the interface
        String::new()
    }

    fn emit_runtime_impl(&self, _config: &ArcConfig) -> String {
        // This is delegated to the runtime module
        String::new()
    }

    fn emit_retain(&self, ty: &Type, var: &str) -> String {
        let fn_name = self.retain_fn(ty);

        if self.needs_generic_arc(ty) {
            format!("{}((void*)&{});", fn_name, var)
        } else {
            format!("{}(&{});", fn_name, var)
        }
    }

    fn emit_release(&self, ty: &Type, var: &str) -> String {
        let fn_name = self.release_fn(ty);

        if self.needs_generic_arc(ty) {
            format!("{}((void*)&{});", fn_name, var)
        } else {
            format!("{}(&{});", fn_name, var)
        }
    }

    fn emit_scope_cleanup(&self, releases: &[ReleasePoint]) -> String {
        if releases.is_empty() {
            return String::new();
        }

        let mut code = String::new();
        code.push_str("// Scope cleanup\n");

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
            Type::List(_) => format!("{} {} = sigil_list_new();", type_name, var),
            Type::Map(_, _) => format!("{} {} = sigil_map_new();", type_name, var),
            Type::Struct { name, .. } => {
                format!("{} {} = sigil_{}_new();", type_name, var, name.to_lowercase())
            }
            _ => format!("{} {} = sigil_arc_alloc(sizeof({}));", type_name, var, type_name),
        }
    }

    fn emit_dealloc(&self, ty: &Type, var: &str) -> String {
        match ty {
            Type::Str => format!("sigil_string_free(&{});", var),
            Type::List(_) => format!("sigil_list_free(&{});", var),
            Type::Map(_, _) => format!("sigil_map_free(&{});", var),
            Type::Struct { name, .. } => format!("sigil_{}_free(&{});", name.to_lowercase(), var),
            _ => format!("sigil_arc_free((void*)&{});", var),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_c_type_names() {
        let emitter = DefaultCodeEmitter::new();

        assert_eq!(emitter.c_type_name(&Type::Int), "int64_t");
        assert_eq!(emitter.c_type_name(&Type::Str), "SigilString");
        assert_eq!(emitter.c_type_name(&Type::List(Box::new(Type::Int))), "SigilList");
    }

    #[test]
    fn test_retain_emission() {
        let emitter = DefaultCodeEmitter::new();

        let retain = emitter.emit_retain(&Type::Str, "my_string");
        assert!(retain.contains("sigil_string_retain"));
        assert!(retain.contains("my_string"));
    }

    #[test]
    fn test_release_emission() {
        let emitter = DefaultCodeEmitter::new();

        let release = emitter.emit_release(&Type::List(Box::new(Type::Int)), "my_list");
        assert!(release.contains("sigil_list_release"));
        assert!(release.contains("my_list"));
    }

    #[test]
    fn test_scope_cleanup() {
        let emitter = DefaultCodeEmitter::new();
        use super::super::super::ids::{LocalId, ScopeId};
        use super::super::super::traits::ReleaseReason;

        let releases = vec![
            ReleasePoint {
                scope_id: ScopeId::new(0),
                local_id: LocalId::new(0),
                ty: Type::Str,
                reason: ReleaseReason::ScopeExit,
                order: 0,
            },
            ReleasePoint {
                scope_id: ScopeId::new(0),
                local_id: LocalId::new(1),
                ty: Type::List(Box::new(Type::Int)),
                reason: ReleaseReason::ScopeExit,
                order: 1,
            },
        ];

        let cleanup = emitter.emit_scope_cleanup(&releases);
        assert!(cleanup.contains("sigil_string_release"));
        assert!(cleanup.contains("sigil_list_release"));
        assert!(cleanup.contains("local_0"));
        assert!(cleanup.contains("local_1"));
    }

    #[test]
    fn test_alloc_emission() {
        let emitter = DefaultCodeEmitter::new();

        let alloc = emitter.emit_alloc(&Type::Str, "new_string");
        assert!(alloc.contains("SigilString"));
        assert!(alloc.contains("sigil_string_new"));
    }

    #[test]
    fn test_custom_type_functions() {
        let emitter = DefaultCodeEmitter::new();

        let user_type = Type::Struct {
            name: "User".to_string(),
            fields: vec![],
        };

        let retain = emitter.emit_retain(&user_type, "user");
        assert!(retain.contains("sigil_user_retain"));

        let release = emitter.emit_release(&user_type, "user");
        assert!(release.contains("sigil_user_release"));
    }
}
