use ori_ir::ast::items::Module;
use ori_ir::canon::{CanonRoot, MethodRoot};
use ori_ir::Name;
use rustc_hash::{FxHashMap, FxHashSet};

use super::Lowerer;

pub(super) fn lower_function_roots(lowerer: &mut Lowerer<'_>, module: &Module) -> Vec<CanonRoot> {
    let mut groups: FxHashMap<Name, Vec<&ori_ir::Function>> = FxHashMap::default();
    for function in &module.functions {
        groups.entry(function.name).or_default().push(function);
    }

    let mut roots = Vec::with_capacity(module.functions.len() + module.tests.len());
    let mut seen = FxHashSet::default();
    for function in &module.functions {
        if !seen.insert(function.name) {
            continue;
        }
        if let Some(root) = lower_function_group(lowerer, function, &groups[&function.name]) {
            roots.push(root);
        }
    }
    roots
}

fn lower_function_group(
    lowerer: &mut Lowerer<'_>,
    function: &ori_ir::Function,
    group: &[&ori_ir::Function],
) -> Option<CanonRoot> {
    if group.len() == 1 {
        if !function.body.is_valid() {
            return None;
        }
        return Some(CanonRoot {
            name: function.name,
            body: lowerer.lower_expr(function.body),
            defaults: lowerer.lower_param_defaults(function.params),
            param_names: Vec::new(),
        });
    }

    let body = lowerer.lower_multi_clause(group);
    let defaults = lowerer.lower_param_defaults(group[0].params);
    let param_names = lowerer
        .typed
        .function(function.name)
        .map(|signature| signature.param_names.clone())
        .unwrap_or_default();
    Some(CanonRoot {
        name: function.name,
        body,
        defaults,
        param_names,
    })
}

pub(super) fn lower_test_roots(
    lowerer: &mut Lowerer<'_>,
    module: &Module,
    roots: &mut Vec<CanonRoot>,
) {
    for test in &module.tests {
        if !test.body.is_valid() {
            continue;
        }
        roots.push(CanonRoot {
            name: test.name,
            body: lowerer.lower_expr(test.body),
            defaults: Vec::new(),
            param_names: Vec::new(),
        });
    }
}

pub(super) fn trait_default_methods(
    module: &Module,
) -> FxHashMap<Name, Vec<&ori_ir::TraitDefaultMethod>> {
    let mut defaults = FxHashMap::default();
    for trait_def in &module.traits {
        for item in &trait_def.items {
            if let ori_ir::TraitItem::DefaultMethod(method) = item {
                defaults
                    .entry(trait_def.name)
                    .or_insert_with(Vec::new)
                    .push(method);
            }
        }
    }
    defaults
}

pub(super) fn lower_impl_method_roots(
    lowerer: &mut Lowerer<'_>,
    module: &Module,
    interner: &ori_ir::StringInterner,
    trait_defaults: &FxHashMap<Name, Vec<&ori_ir::TraitDefaultMethod>>,
) -> Vec<MethodRoot> {
    let mut roots = Vec::new();
    for impl_def in &module.impls {
        let Some(type_name) = impl_def.semantic_type_name(interner) else {
            continue;
        };
        let mut overridden = FxHashSet::default();
        for method in &impl_def.methods {
            overridden.insert(method.name);
            if let Some(root) = lower_method_root(lowerer, type_name, method) {
                roots.push(root);
            }
        }

        let Some(trait_name) = impl_def
            .trait_path
            .as_ref()
            .and_then(|path| path.last())
            .copied()
        else {
            continue;
        };
        let Some(defaults) = trait_defaults.get(&trait_name) else {
            continue;
        };
        for method in defaults {
            if !overridden.contains(&method.name) && method.body.is_valid() {
                roots.push(MethodRoot {
                    type_name,
                    method_name: method.name,
                    source_body: method.body,
                    body: lowerer.lower_expr(method.body),
                });
            }
        }
    }
    roots
}

fn lower_method_root(
    lowerer: &mut Lowerer<'_>,
    type_name: Name,
    method: &ori_ir::ImplMethod,
) -> Option<MethodRoot> {
    method.body.is_valid().then(|| MethodRoot {
        type_name,
        method_name: method.name,
        source_body: method.body,
        body: lowerer.lower_expr(method.body),
    })
}

pub(super) fn lower_named_method_roots<'a>(
    lowerer: &mut Lowerer<'_>,
    groups: impl IntoIterator<Item = (Name, &'a [ori_ir::ImplMethod])>,
    roots: &mut Vec<MethodRoot>,
) {
    for (type_name, methods) in groups {
        for method in methods {
            if let Some(root) = lower_method_root(lowerer, type_name, method) {
                roots.push(root);
            }
        }
    }
}
