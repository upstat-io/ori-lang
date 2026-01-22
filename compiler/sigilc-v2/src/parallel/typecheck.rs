//! Parallel type checking integration.
//!
//! This module wires up the type checker to the parallel infrastructure,
//! enabling level-based parallel type checking of modules.

use std::sync::Arc;
use rayon::prelude::*;
use parking_lot::RwLock;

use crate::intern::{StringInterner, TypeInterner, TypeId};
use crate::syntax::{Item, ItemKind, ExprArena, TypeExprId, TypeExprKind};
use crate::hir::{DefinitionRegistry, FunctionSig, ConfigDef, ParamSig};
use crate::check::TypeContext;
use crate::errors::Diagnostic;
use super::{ParsedFile, DependencyGraph};

/// Result of type checking a single module.
#[derive(Debug)]
pub struct TypeCheckedModule {
    /// Module name.
    pub module_name: String,
    /// Function signatures extracted from this module.
    pub signatures: Vec<FunctionSig>,
    /// Config definitions.
    pub configs: Vec<ConfigDef>,
    /// Type checking errors.
    pub errors: Vec<Diagnostic>,
    /// Whether type checking succeeded (no errors).
    pub success: bool,
}

/// Parallel type checker that processes modules level-by-level.
pub struct ParallelTypeChecker<'a> {
    /// Shared string interner.
    interner: &'a StringInterner,
    /// Shared type interner (concurrent via DashMap).
    types: &'a TypeInterner,
    /// Global registry built incrementally as levels complete.
    global_registry: Arc<RwLock<DefinitionRegistry>>,
}

impl<'a> ParallelTypeChecker<'a> {
    /// Create a new parallel type checker.
    pub fn new(interner: &'a StringInterner, types: &'a TypeInterner) -> Self {
        ParallelTypeChecker {
            interner,
            types,
            global_registry: Arc::new(RwLock::new(DefinitionRegistry::new())),
        }
    }

    /// Type check modules according to the dependency graph.
    ///
    /// Modules at the same level are checked in parallel.
    /// Levels are processed sequentially to respect dependencies.
    pub fn check_modules(
        &self,
        modules: &[ParsedFile],
        graph: &DependencyGraph,
    ) -> Vec<TypeCheckedModule> {
        let mut all_results = Vec::with_capacity(modules.len());

        // Process each level sequentially
        for level in graph.iter_levels() {
            // Get modules at this level
            let level_modules: Vec<_> = level
                .modules
                .iter()
                .filter_map(|name| modules.iter().find(|m| &m.module_name == name))
                .collect();

            // Type check modules at this level in parallel
            let level_results: Vec<TypeCheckedModule> = level_modules
                .par_iter()
                .map(|module| self.check_module(module))
                .collect();

            // Merge signatures into global registry for next level
            {
                let mut registry = self.global_registry.write();
                for result in &level_results {
                    for sig in &result.signatures {
                        registry.register_function(sig.clone());
                    }
                    for config in &result.configs {
                        registry.register_config(config.clone());
                    }
                }
            }

            all_results.extend(level_results);
        }

        all_results
    }

    /// Type check a single module.
    fn check_module(&self, module: &ParsedFile) -> TypeCheckedModule {
        // First pass: extract signatures without checking bodies
        let signatures = self.extract_signatures(&module.items, &module.arena);
        let configs = self.extract_configs(&module.items, &module.arena);

        // Create local registry with this module's definitions
        let mut local_registry = DefinitionRegistry::new();
        for sig in &signatures {
            local_registry.register_function(sig.clone());
        }
        for config in &configs {
            local_registry.register_config(config.clone());
        }

        // Get imported definitions from global registry
        let imported_registry = self.global_registry.read().clone();

        // Second pass: type check function bodies
        let mut ctx = TypeContext::new(
            self.interner,
            self.types,
            &module.arena,
            &local_registry,
            &imported_registry,
        );

        for item in &module.items {
            if let ItemKind::Function(func) = &item.kind {
                // Resolve return type
                let return_type = func.return_type
                    .map(|id| self.resolve_type_expr(id, &module.arena))
                    .unwrap_or(TypeId::VOID);
                ctx.scopes.push_function(return_type);

                // Bind parameters with resolved types
                let params = module.arena.get_params(func.params);
                for (i, param) in params.iter().enumerate() {
                    let param_ty = param.ty
                        .map(|id| self.resolve_type_expr(id, &module.arena))
                        .unwrap_or(TypeId::INFER);
                    ctx.scopes.define_param(param.name, param_ty, i);
                }

                // Check body
                let _body_ty = ctx.infer(func.body);

                ctx.scopes.pop();
            }
        }

        let errors = ctx.into_diagnostics();
        let success = errors.is_empty();

        TypeCheckedModule {
            module_name: module.module_name.clone(),
            signatures,
            configs,
            errors,
            success,
        }
    }

    /// Extract function signatures from items (first pass).
    fn extract_signatures(&self, items: &[Item], arena: &ExprArena) -> Vec<FunctionSig> {
        items
            .iter()
            .filter_map(|item| {
                if let ItemKind::Function(func) = &item.kind {
                    let params = arena.get_params(func.params);
                    let param_sigs: Vec<_> = params
                        .iter()
                        .map(|p| ParamSig {
                            name: p.name,
                            ty: p.ty.map(|id| self.resolve_type_expr(id, arena))
                                .unwrap_or(TypeId::INFER),
                            has_default: false,
                        })
                        .collect();

                    let return_type = func.return_type
                        .map(|id| self.resolve_type_expr(id, arena))
                        .unwrap_or(TypeId::VOID);

                    Some(FunctionSig {
                        name: func.name,
                        params: param_sigs,
                        return_type,
                        type_params: Vec::new(),
                        capabilities: Vec::new(),
                        is_async: false,
                        span: item.span,
                    })
                } else {
                    None
                }
            })
            .collect()
    }

    /// Extract config definitions from items.
    fn extract_configs(&self, items: &[Item], _arena: &ExprArena) -> Vec<ConfigDef> {
        items
            .iter()
            .filter_map(|item| {
                if let ItemKind::Config(config) = &item.kind {
                    Some(ConfigDef {
                        name: config.name,
                        // config.ty is Option<TypeExprId>, use INFER as placeholder
                        ty: crate::intern::TypeId::INFER,
                        span: item.span,
                    })
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get type checking statistics.
    pub fn stats(&self) -> TypeCheckStats {
        TypeCheckStats {
            modules_checked: 0, // Would track during check_modules
            errors_found: 0,
            signatures_extracted: 0,
        }
    }

    /// Resolve a type expression to a TypeId.
    /// Handles basic named types and some built-in generics.
    fn resolve_type_expr(&self, type_expr_id: TypeExprId, arena: &ExprArena) -> TypeId {
        let type_expr = arena.get_type_expr(type_expr_id);
        self.resolve_type_expr_inner(&type_expr.kind, arena)
    }

    /// Inner type resolution.
    fn resolve_type_expr_inner(&self, kind: &TypeExprKind, arena: &ExprArena) -> TypeId {
        match kind {
            TypeExprKind::Named { name, type_args } => {
                let name_str = self.interner.lookup(*name);
                match name_str {
                    "int" => TypeId::INT,
                    "float" => TypeId::FLOAT,
                    "bool" => TypeId::BOOL,
                    "str" => TypeId::STR,
                    "char" => TypeId::CHAR,
                    "byte" => TypeId::BYTE,
                    "void" => TypeId::VOID,
                    "Never" => TypeId::NEVER,
                    "Option" if type_args.len() == 1 => {
                        let inner = self.resolve_type_expr_inner(&type_args[0].kind, arena);
                        self.types.intern_option(inner)
                    }
                    "Result" if type_args.len() == 2 => {
                        let ok = self.resolve_type_expr_inner(&type_args[0].kind, arena);
                        let err = self.resolve_type_expr_inner(&type_args[1].kind, arena);
                        self.types.intern_result(ok, err)
                    }
                    _ => {
                        // Unknown type - return named type placeholder
                        self.types.intern(crate::intern::TypeKind::Named {
                            name: *name,
                            type_args: crate::intern::TypeRange::EMPTY,
                        })
                    }
                }
            }
            TypeExprKind::List(inner) => {
                let elem = self.resolve_type_expr_inner(&inner.kind, arena);
                self.types.intern_list(elem)
            }
            TypeExprKind::Tuple(elems) => {
                let elem_types: Vec<_> = elems
                    .iter()
                    .map(|e| self.resolve_type_expr_inner(&e.kind, arena))
                    .collect();
                self.types.intern_tuple(&elem_types)
            }
            TypeExprKind::Function { params, ret } => {
                let param_types: Vec<_> = params
                    .iter()
                    .map(|p| self.resolve_type_expr_inner(&p.kind, arena))
                    .collect();
                let ret_type = self.resolve_type_expr_inner(&ret.kind, arena);
                self.types.intern_function(&param_types, ret_type)
            }
            TypeExprKind::Map { key, value } => {
                let key_type = self.resolve_type_expr_inner(&key.kind, arena);
                let value_type = self.resolve_type_expr_inner(&value.kind, arena);
                self.types.intern_map(key_type, value_type)
            }
            TypeExprKind::Infer => TypeId::INFER,
            TypeExprKind::Ref { .. } => TypeId::INFER, // References not fully supported yet
            TypeExprKind::Error => self.types.intern(crate::intern::TypeKind::Error),
        }
    }
}

/// Statistics from parallel type checking.
#[derive(Clone, Debug, Default)]
pub struct TypeCheckStats {
    /// Number of modules type checked.
    pub modules_checked: usize,
    /// Total errors found.
    pub errors_found: usize,
    /// Total function signatures extracted.
    pub signatures_extracted: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn make_parsed_file(name: &str, source: &str, interner: &StringInterner) -> ParsedFile {
        use crate::syntax::{Lexer, Parser};

        let lexer = Lexer::new(source, interner);
        let tokens = lexer.lex_all();
        let parser = Parser::new(&tokens, interner);
        let result = parser.parse_module();

        ParsedFile {
            path: PathBuf::from(format!("{}.si", name)),
            items: result.items,
            arena: result.arena,
            errors: result.diagnostics,
            success: true,
            module_name: name.to_string(),
            imports: Vec::new(),
        }
    }

    #[test]
    fn test_parallel_typecheck_single_module() {
        let interner = StringInterner::new();
        let types = TypeInterner::new();

        let module = make_parsed_file(
            "main",
            "@add (a: int, b: int) -> int = a + b",
            &interner,
        );

        // Create modules slice without cloning
        let modules = [module];
        let graph = DependencyGraph::from_modules(&modules);
        let checker = ParallelTypeChecker::new(&interner, &types);
        let results = checker.check_modules(&modules, &graph);

        assert_eq!(results.len(), 1);
        assert!(results[0].success, "Errors: {:?}", results[0].errors);
        assert_eq!(results[0].signatures.len(), 1);
    }

    #[test]
    fn test_parallel_typecheck_multiple_modules() {
        let interner = StringInterner::new();
        let types = TypeInterner::new();

        let module_a = make_parsed_file("a", "@helper () -> int = 42", &interner);
        let module_b = make_parsed_file("b", "@other () -> int = 100", &interner);

        let modules = vec![module_a, module_b];
        let graph = DependencyGraph::from_modules(&modules);
        let checker = ParallelTypeChecker::new(&interner, &types);
        let results = checker.check_modules(&modules, &graph);

        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|r| r.success));
    }

    #[test]
    fn test_parallel_typecheck_type_error() {
        let interner = StringInterner::new();
        let types = TypeInterner::new();

        // Type error: adding int and bool
        let module = make_parsed_file("bad", "@bad () -> int = 1 + true", &interner);

        let modules = [module];
        let graph = DependencyGraph::from_modules(&modules);
        let checker = ParallelTypeChecker::new(&interner, &types);
        let results = checker.check_modules(&modules, &graph);

        assert_eq!(results.len(), 1);
        assert!(!results[0].success);
        assert!(!results[0].errors.is_empty());
    }

    #[test]
    fn test_parallel_typecheck_with_dependencies() {
        let interner = StringInterner::new();
        let types = TypeInterner::new();

        // a has no deps, b depends on a
        let module_a = make_parsed_file("a", "@base () -> int = 1", &interner);
        let mut module_b = make_parsed_file("b", "@derived () -> int = 2", &interner);
        module_b.imports = vec!["./a".to_string()];

        let modules = vec![module_a, module_b];
        let graph = DependencyGraph::from_modules(&modules);

        // Should have 2 levels
        assert_eq!(graph.level_count(), 2);

        let checker = ParallelTypeChecker::new(&interner, &types);
        let results = checker.check_modules(&modules, &graph);

        assert_eq!(results.len(), 2);
    }
}
