//! Name resolution for identifiers and imports.
//!
//! The resolver takes parsed names and resolves them to their definitions,
//! handling imports, scoping, and visibility.

use crate::intern::{Name, TypeId, StringInterner};
use crate::syntax::Span;
use crate::errors::Diagnostic;
use super::scope::{Scopes, Binding};
use super::registry::{DefinitionRegistry, FunctionSig, TypeDef, ConfigDef};
use std::sync::Arc;

/// Result of resolving a name.
#[derive(Clone, Debug)]
pub enum ResolvedName {
    /// Local variable or parameter.
    Local(Binding),
    /// Function.
    Function(Arc<FunctionSig>),
    /// Type.
    Type(Arc<TypeDef>),
    /// Config variable.
    Config(Arc<ConfigDef>),
    /// Builtin function (print, len, etc.).
    Builtin(BuiltinKind),
    /// Enum variant constructor.
    Variant {
        enum_type: TypeId,
        variant: Name,
    },
}

/// Built-in functions and values.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum BuiltinKind {
    // Functions
    Print,
    Len,
    Str,
    Int,
    Float,
    Compare,
    Panic,
    Assert,
    AssertEq,
    // Constructors
    Some,
    None,
    Ok,
    Err,
}

/// Error during name resolution.
#[derive(Clone, Debug)]
pub struct ResolutionError {
    pub kind: ResolutionErrorKind,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum ResolutionErrorKind {
    /// Name not found.
    NotFound(Name),
    /// Name is ambiguous (multiple imports).
    Ambiguous(Name),
    /// Name is private and not accessible.
    Private(Name),
    /// Circular import detected.
    CircularImport,
}

impl ResolutionError {
    pub fn not_found(name: Name, span: Span) -> Self {
        ResolutionError {
            kind: ResolutionErrorKind::NotFound(name),
            span,
        }
    }

    pub fn to_diagnostic(&self, interner: &StringInterner) -> Diagnostic {
        match &self.kind {
            ResolutionErrorKind::NotFound(name) => {
                Diagnostic::error(
                    format!("cannot find `{}` in this scope", interner.lookup(*name)),
                    self.span,
                ).with_code("E3002")
            }
            ResolutionErrorKind::Ambiguous(name) => {
                Diagnostic::error(
                    format!("`{}` is ambiguous", interner.lookup(*name)),
                    self.span,
                ).with_code("E3010")
            }
            ResolutionErrorKind::Private(name) => {
                Diagnostic::error(
                    format!("`{}` is private", interner.lookup(*name)),
                    self.span,
                ).with_code("E3011")
            }
            ResolutionErrorKind::CircularImport => {
                Diagnostic::error(
                    "circular import detected".to_string(),
                    self.span,
                ).with_code("E3012")
            }
        }
    }
}

/// Name resolver that combines scopes and registries.
pub struct Resolver<'a> {
    /// String interner for name lookup.
    interner: &'a StringInterner,
    /// Local scopes.
    scopes: &'a Scopes,
    /// Current module's definitions.
    local_registry: &'a DefinitionRegistry,
    /// Imported definitions.
    imported_registry: &'a DefinitionRegistry,
    /// Builtin names (pre-resolved).
    builtins: BuiltinNames,
}

/// Pre-interned names for builtins.
struct BuiltinNames {
    print: Name,
    len: Name,
    str_: Name,
    int: Name,
    float: Name,
    compare: Name,
    panic: Name,
    assert: Name,
    assert_eq: Name,
    some: Name,
    none: Name,
    ok: Name,
    err: Name,
}

impl BuiltinNames {
    fn new(interner: &StringInterner) -> Self {
        BuiltinNames {
            print: interner.intern("print"),
            len: interner.intern("len"),
            str_: interner.intern("str"),
            int: interner.intern("int"),
            float: interner.intern("float"),
            compare: interner.intern("compare"),
            panic: interner.intern("panic"),
            assert: interner.intern("assert"),
            assert_eq: interner.intern("assert_eq"),
            some: interner.intern("Some"),
            none: interner.intern("None"),
            ok: interner.intern("Ok"),
            err: interner.intern("Err"),
        }
    }

    fn lookup(&self, name: Name) -> Option<BuiltinKind> {
        if name == self.print { Some(BuiltinKind::Print) }
        else if name == self.len { Some(BuiltinKind::Len) }
        else if name == self.str_ { Some(BuiltinKind::Str) }
        else if name == self.int { Some(BuiltinKind::Int) }
        else if name == self.float { Some(BuiltinKind::Float) }
        else if name == self.compare { Some(BuiltinKind::Compare) }
        else if name == self.panic { Some(BuiltinKind::Panic) }
        else if name == self.assert { Some(BuiltinKind::Assert) }
        else if name == self.assert_eq { Some(BuiltinKind::AssertEq) }
        else if name == self.some { Some(BuiltinKind::Some) }
        else if name == self.none { Some(BuiltinKind::None) }
        else if name == self.ok { Some(BuiltinKind::Ok) }
        else if name == self.err { Some(BuiltinKind::Err) }
        else { None }
    }
}

impl<'a> Resolver<'a> {
    /// Create a new resolver.
    pub fn new(
        interner: &'a StringInterner,
        scopes: &'a Scopes,
        local_registry: &'a DefinitionRegistry,
        imported_registry: &'a DefinitionRegistry,
    ) -> Self {
        Resolver {
            interner,
            scopes,
            local_registry,
            imported_registry,
            builtins: BuiltinNames::new(interner),
        }
    }

    /// Resolve a name to its definition.
    ///
    /// Search order:
    /// 1. Local scope (variables, parameters)
    /// 2. Builtins
    /// 3. Current module definitions
    /// 4. Imported definitions
    pub fn resolve(&self, name: Name, span: Span) -> Result<ResolvedName, ResolutionError> {
        // 1. Local scope
        if let Some(binding) = self.scopes.lookup(name) {
            return Ok(ResolvedName::Local(binding.clone()));
        }

        // 2. Builtins
        if let Some(builtin) = self.builtins.lookup(name) {
            return Ok(ResolvedName::Builtin(builtin));
        }

        // 3. Current module definitions
        if let Some(func) = self.local_registry.get_function(name) {
            return Ok(ResolvedName::Function(Arc::clone(func)));
        }
        if let Some(ty) = self.local_registry.get_type(name) {
            return Ok(ResolvedName::Type(Arc::clone(ty)));
        }
        if let Some(config) = self.local_registry.get_config(name) {
            return Ok(ResolvedName::Config(Arc::clone(config)));
        }

        // 4. Imported definitions
        if let Some(func) = self.imported_registry.get_function(name) {
            return Ok(ResolvedName::Function(Arc::clone(func)));
        }
        if let Some(ty) = self.imported_registry.get_type(name) {
            return Ok(ResolvedName::Type(Arc::clone(ty)));
        }
        if let Some(config) = self.imported_registry.get_config(name) {
            return Ok(ResolvedName::Config(Arc::clone(config)));
        }

        Err(ResolutionError::not_found(name, span))
    }

    /// Resolve a function reference (@name).
    pub fn resolve_function(&self, name: Name, span: Span) -> Result<Arc<FunctionSig>, ResolutionError> {
        // Check local module first
        if let Some(func) = self.local_registry.get_function(name) {
            return Ok(Arc::clone(func));
        }

        // Then imports
        if let Some(func) = self.imported_registry.get_function(name) {
            return Ok(Arc::clone(func));
        }

        Err(ResolutionError::not_found(name, span))
    }

    /// Resolve a config reference ($name).
    pub fn resolve_config(&self, name: Name, span: Span) -> Result<Arc<ConfigDef>, ResolutionError> {
        // Check local module first
        if let Some(config) = self.local_registry.get_config(name) {
            return Ok(Arc::clone(config));
        }

        // Then imports
        if let Some(config) = self.imported_registry.get_config(name) {
            return Ok(Arc::clone(config));
        }

        Err(ResolutionError::not_found(name, span))
    }

    /// Resolve a type name.
    pub fn resolve_type(&self, name: Name, span: Span) -> Result<Arc<TypeDef>, ResolutionError> {
        // Check local module first
        if let Some(ty) = self.local_registry.get_type(name) {
            return Ok(Arc::clone(ty));
        }

        // Then imports
        if let Some(ty) = self.imported_registry.get_type(name) {
            return Ok(Arc::clone(ty));
        }

        Err(ResolutionError::not_found(name, span))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_local() {
        let interner = StringInterner::new();
        let mut scopes = Scopes::new();
        let local_registry = DefinitionRegistry::new();
        let imported_registry = DefinitionRegistry::new();

        let x = interner.intern("x");
        scopes.define_local(x, TypeId::INT, false);

        let resolver = Resolver::new(&interner, &scopes, &local_registry, &imported_registry);

        match resolver.resolve(x, Span::DUMMY) {
            Ok(ResolvedName::Local(binding)) => {
                assert_eq!(binding.ty(), TypeId::INT);
            }
            _ => panic!("Expected local binding"),
        }
    }

    #[test]
    fn test_resolve_builtin() {
        let interner = StringInterner::new();
        let scopes = Scopes::new();
        let local_registry = DefinitionRegistry::new();
        let imported_registry = DefinitionRegistry::new();

        let resolver = Resolver::new(&interner, &scopes, &local_registry, &imported_registry);

        let print = interner.intern("print");
        match resolver.resolve(print, Span::DUMMY) {
            Ok(ResolvedName::Builtin(BuiltinKind::Print)) => {}
            _ => panic!("Expected builtin Print"),
        }
    }

    #[test]
    fn test_resolve_not_found() {
        let interner = StringInterner::new();
        let scopes = Scopes::new();
        let local_registry = DefinitionRegistry::new();
        let imported_registry = DefinitionRegistry::new();

        let resolver = Resolver::new(&interner, &scopes, &local_registry, &imported_registry);

        let unknown = interner.intern("unknown_name");
        assert!(resolver.resolve(unknown, Span::DUMMY).is_err());
    }
}
