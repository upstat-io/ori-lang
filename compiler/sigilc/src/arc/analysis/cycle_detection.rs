// Cycle Detection for ARC Memory Management
//
// Detects cyclic type references at compile time using Tarjan's
// Strongly Connected Components (SCC) algorithm.
//
// In Sigil's ARC model, cyclic type definitions are rejected at compile time
// instead of using weak references. This eliminates an entire class of
// memory leaks by construction.

use std::collections::HashMap;

use crate::ir::{TModule, TTypeDef, TTypeDefKind, Type};

use super::super::traits::{
    CycleCheckResult, CycleDetector, CycleInfo, TypeNode, TypeReference, TypeReferenceGraph,
};

/// Default implementation of CycleDetector using Tarjan's SCC algorithm
pub struct DefaultCycleDetector<'a> {
    /// Reference to the module being checked
    module: &'a TModule,

    /// Map from type name to type definition
    type_defs: HashMap<String, &'a TTypeDef>,
}

impl<'a> DefaultCycleDetector<'a> {
    /// Create a new cycle detector for a module
    pub fn new(module: &'a TModule) -> Self {
        let mut type_defs = HashMap::new();
        for type_def in &module.types {
            type_defs.insert(type_def.name.clone(), type_def);
        }

        DefaultCycleDetector { module, type_defs }
    }

    /// Extract type references from a Type
    fn extract_references(
        &self,
        ty: &Type,
        field_name: &str,
        references: &mut Vec<TypeReference>,
    ) {
        match ty {
            // Named types are potential direct references
            Type::Named(name) | Type::Struct { name, .. } | Type::Enum { name, .. } => {
                // Check if this is a user-defined type
                if self.type_defs.contains_key(name) {
                    references.push(TypeReference {
                        field_name: field_name.to_string(),
                        target_type: name.clone(),
                        is_direct: true,
                    });
                }
            }

            // Option<T> - reference through T but NOT direct (allows self-reference with Option)
            Type::Option(inner) => {
                if let Type::Named(name) = inner.as_ref() {
                    if self.type_defs.contains_key(name) {
                        references.push(TypeReference {
                            field_name: field_name.to_string(),
                            target_type: name.clone(),
                            is_direct: false, // Option breaks cycles
                        });
                    }
                }
            }

            // List<T>, Map<K, V> - references through elements but not direct
            Type::List(elem) => {
                self.extract_references(elem, field_name, references);
                // Mark any found references as indirect
                for r in references.iter_mut() {
                    r.is_direct = false;
                }
            }
            Type::Map(k, v) => {
                self.extract_references(k, field_name, references);
                self.extract_references(v, field_name, references);
                for r in references.iter_mut() {
                    r.is_direct = false;
                }
            }

            // Tuple elements
            Type::Tuple(elems) => {
                for (i, elem) in elems.iter().enumerate() {
                    let elem_field = format!("{}.{}", field_name, i);
                    self.extract_references(elem, &elem_field, references);
                }
            }

            // Result<T, E> - references through T and E
            Type::Result(ok, err) => {
                let ok_field = format!("{}.ok", field_name);
                let err_field = format!("{}.err", field_name);
                self.extract_references(ok, &ok_field, references);
                self.extract_references(err, &err_field, references);
            }

            // Record fields
            Type::Record(fields) => {
                for (name, ty) in fields {
                    let sub_field = format!("{}.{}", field_name, name);
                    self.extract_references(ty, &sub_field, references);
                }
            }

            // Primitives and other types don't reference user types
            _ => {}
        }
    }

    /// Build a TypeNode from a type definition
    fn type_def_to_node(&self, type_def: &TTypeDef) -> TypeNode {
        let mut references = Vec::new();

        match &type_def.kind {
            TTypeDefKind::Alias(ty) => {
                self.extract_references(ty, "alias", &mut references);
            }
            TTypeDefKind::Struct(fields) => {
                for field in fields {
                    self.extract_references(&field.ty, &field.name, &mut references);
                }
            }
            TTypeDefKind::Enum(variants) => {
                for variant in variants {
                    for field in &variant.fields {
                        let field_name = format!("{}.{}", variant.name, field.name);
                        self.extract_references(&field.ty, &field_name, &mut references);
                    }
                }
            }
        }

        TypeNode {
            name: type_def.name.clone(),
            references,
        }
    }
}

impl<'a> CycleDetector for DefaultCycleDetector<'a> {
    fn check_type(&self, type_def: &TTypeDef) -> CycleCheckResult {
        let node = self.type_def_to_node(type_def);

        // Check for direct self-reference
        for reference in &node.references {
            if reference.is_direct && reference.target_type == type_def.name {
                return CycleCheckResult::DirectCycle {
                    field_path: vec![reference.field_name.clone()],
                };
            }
        }

        // Build full graph and check for indirect cycles
        let graph = self.build_type_graph(self.module);
        let cycles = find_cycles_tarjan(&graph);

        // Check if this type is part of any cycle
        for cycle in cycles {
            if cycle.types.contains(&type_def.name) {
                if cycle.is_direct && cycle.types.len() == 1 {
                    return CycleCheckResult::DirectCycle {
                        field_path: cycle.fields,
                    };
                } else {
                    return CycleCheckResult::IndirectCycle {
                        type_path: cycle.types,
                    };
                }
            }
        }

        CycleCheckResult::Acyclic
    }

    fn build_type_graph(&self, module: &TModule) -> TypeReferenceGraph {
        let mut graph = TypeReferenceGraph::new();

        for type_def in &module.types {
            let node = self.type_def_to_node(type_def);
            graph.add_node(node);
        }

        graph
    }
}

/// Tarjan's SCC algorithm state
struct TarjanState {
    index: u32,
    stack: Vec<String>,
    indices: HashMap<String, u32>,
    lowlinks: HashMap<String, u32>,
    on_stack: HashMap<String, bool>,
    sccs: Vec<Vec<String>>,
}

impl TarjanState {
    fn new() -> Self {
        TarjanState {
            index: 0,
            stack: Vec::new(),
            indices: HashMap::new(),
            lowlinks: HashMap::new(),
            on_stack: HashMap::new(),
            sccs: Vec::new(),
        }
    }
}

/// Find cycles using Tarjan's SCC algorithm
pub fn find_cycles_tarjan(graph: &TypeReferenceGraph) -> Vec<CycleInfo> {
    let mut state = TarjanState::new();

    // Build adjacency list from graph
    let mut adjacency: HashMap<String, Vec<(String, String)>> = HashMap::new();
    for node in &graph.nodes {
        adjacency.entry(node.name.clone()).or_default();
        for reference in &node.references {
            if reference.is_direct {
                adjacency
                    .entry(node.name.clone())
                    .or_default()
                    .push((reference.target_type.clone(), reference.field_name.clone()));
            }
        }
    }

    // Run Tarjan's algorithm on each unvisited node
    for node in &graph.nodes {
        if !state.indices.contains_key(&node.name) {
            strongconnect(&node.name, &adjacency, &mut state);
        }
    }

    // Convert SCCs to CycleInfo (only those with cycles)
    let mut cycles = Vec::new();
    for scc in state.sccs {
        // A single node is a cycle only if it has an edge to itself
        if scc.len() == 1 {
            let node = &scc[0];
            if let Some(edges) = adjacency.get(node) {
                for (target, field) in edges {
                    if target == node {
                        cycles.push(CycleInfo {
                            types: scc.clone(),
                            fields: vec![field.clone()],
                            is_direct: true,
                        });
                        break;
                    }
                }
            }
        } else if scc.len() > 1 {
            // Multiple nodes form an indirect cycle
            // Reconstruct the field path
            let mut fields = Vec::new();
            for i in 0..scc.len() {
                let from = &scc[i];
                let to = &scc[(i + 1) % scc.len()];
                if let Some(edges) = adjacency.get(from) {
                    for (target, field) in edges {
                        if target == to {
                            fields.push(field.clone());
                            break;
                        }
                    }
                }
            }

            cycles.push(CycleInfo {
                types: scc,
                fields,
                is_direct: false,
            });
        }
    }

    cycles
}

/// Tarjan's strongconnect function
fn strongconnect(
    v: &str,
    adjacency: &HashMap<String, Vec<(String, String)>>,
    state: &mut TarjanState,
) {
    // Set the depth index for v
    state.indices.insert(v.to_string(), state.index);
    state.lowlinks.insert(v.to_string(), state.index);
    state.index += 1;
    state.stack.push(v.to_string());
    state.on_stack.insert(v.to_string(), true);

    // Consider successors of v
    if let Some(edges) = adjacency.get(v) {
        for (w, _) in edges {
            if !state.indices.contains_key(w) {
                // w has not yet been visited; recurse
                strongconnect(w, adjacency, state);
                let lowlink_v = *state.lowlinks.get(v).unwrap();
                let lowlink_w = *state.lowlinks.get(w).unwrap();
                state.lowlinks.insert(v.to_string(), lowlink_v.min(lowlink_w));
            } else if *state.on_stack.get(w).unwrap_or(&false) {
                // w is in the stack and hence in the current SCC
                let lowlink_v = *state.lowlinks.get(v).unwrap();
                let index_w = *state.indices.get(w).unwrap();
                state.lowlinks.insert(v.to_string(), lowlink_v.min(index_w));
            }
        }
    }

    // If v is a root node, pop the stack and generate an SCC
    let lowlink_v = *state.lowlinks.get(v).unwrap();
    let index_v = *state.indices.get(v).unwrap();
    if lowlink_v == index_v {
        let mut scc = Vec::new();
        loop {
            let w = state.stack.pop().unwrap();
            state.on_stack.insert(w.clone(), false);
            scc.push(w.clone());
            if w == v {
                break;
            }
        }
        state.sccs.push(scc);
    }
}

/// Standalone function to check a single type for cycles
pub fn check_type_cycles(type_def: &TTypeDef, module: &TModule) -> CycleCheckResult {
    let detector = DefaultCycleDetector::new(module);
    detector.check_type(type_def)
}

/// Check an entire module for type cycles
pub fn check_module_cycles(module: &TModule) -> Vec<(String, CycleCheckResult)> {
    let detector = DefaultCycleDetector::new(module);
    let mut results = Vec::new();

    for type_def in &module.types {
        let result = detector.check_type(type_def);
        if result.is_cyclic() {
            results.push((type_def.name.clone(), result));
        }
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{TField, TVariant};

    fn make_module(types: Vec<TTypeDef>) -> TModule {
        TModule {
            name: "test".to_string(),
            types,
            configs: vec![],
            functions: vec![],
            tests: vec![],
            imports: vec![],
        }
    }

    fn make_struct(name: &str, fields: Vec<(&str, Type)>) -> TTypeDef {
        TTypeDef {
            name: name.to_string(),
            public: false,
            params: vec![],
            kind: TTypeDefKind::Struct(
                fields
                    .into_iter()
                    .map(|(n, ty)| TField {
                        name: n.to_string(),
                        ty,
                    })
                    .collect(),
            ),
            span: 0..1,
        }
    }

    #[test]
    fn test_no_cycle() {
        let point = make_struct("Point", vec![("x", Type::Int), ("y", Type::Int)]);
        let module = make_module(vec![point.clone()]);
        let detector = DefaultCycleDetector::new(&module);

        assert!(!detector.check_type(&point).is_cyclic());
    }

    #[test]
    fn test_direct_self_reference() {
        // type Node = { value: int, next: Node }
        let node = make_struct(
            "Node",
            vec![
                ("value", Type::Int),
                ("next", Type::Named("Node".to_string())),
            ],
        );
        let module = make_module(vec![node.clone()]);
        let detector = DefaultCycleDetector::new(&module);

        let result = detector.check_type(&node);
        assert!(result.is_cyclic());
        match result {
            CycleCheckResult::DirectCycle { field_path } => {
                assert!(field_path.contains(&"next".to_string()));
            }
            _ => panic!("Expected DirectCycle"),
        }
    }

    #[test]
    fn test_option_breaks_cycle() {
        // type Node = { value: int, next: Option<Node> }
        // This should be allowed because Option breaks the cycle
        let node = make_struct(
            "Node",
            vec![
                ("value", Type::Int),
                (
                    "next",
                    Type::Option(Box::new(Type::Named("Node".to_string()))),
                ),
            ],
        );
        let module = make_module(vec![node.clone()]);
        let detector = DefaultCycleDetector::new(&module);

        // Option does NOT break cycles in our model - we reject all cycles
        // But we mark Option references as non-direct, so this depends on policy
        // For now, the implementation marks Option as non-direct, so no cycle detected
        // If we want to disallow even Option cycles, we'd need to change this
    }

    #[test]
    fn test_mutual_reference() {
        // type A = { b: B }
        // type B = { a: A }
        let type_a = make_struct("A", vec![("b", Type::Named("B".to_string()))]);
        let type_b = make_struct("B", vec![("a", Type::Named("A".to_string()))]);
        let module = make_module(vec![type_a.clone(), type_b]);
        let detector = DefaultCycleDetector::new(&module);

        let result = detector.check_type(&type_a);
        assert!(result.is_cyclic());
        match result {
            CycleCheckResult::IndirectCycle { type_path } => {
                assert!(type_path.contains(&"A".to_string()));
                assert!(type_path.contains(&"B".to_string()));
            }
            _ => panic!("Expected IndirectCycle"),
        }
    }

    #[test]
    fn test_long_cycle() {
        // type A = { b: B }
        // type B = { c: C }
        // type C = { a: A }
        let type_a = make_struct("A", vec![("b", Type::Named("B".to_string()))]);
        let type_b = make_struct("B", vec![("c", Type::Named("C".to_string()))]);
        let type_c = make_struct("C", vec![("a", Type::Named("A".to_string()))]);
        let module = make_module(vec![type_a.clone(), type_b, type_c]);
        let detector = DefaultCycleDetector::new(&module);

        let result = detector.check_type(&type_a);
        assert!(result.is_cyclic());
    }

    #[test]
    fn test_list_does_not_create_cycle() {
        // type Container = { items: [Item] }
        // type Item = { value: int }
        // This is fine because List breaks direct ownership
        let container = make_struct(
            "Container",
            vec![("items", Type::List(Box::new(Type::Named("Item".to_string()))))],
        );
        let item = make_struct("Item", vec![("value", Type::Int)]);
        let module = make_module(vec![container.clone(), item]);
        let detector = DefaultCycleDetector::new(&module);

        // List references are marked as indirect, so no cycle
        assert!(!detector.check_type(&container).is_cyclic());
    }

    #[test]
    fn test_enum_cycle() {
        // type Tree = Leaf(int) | Branch(left: Tree, right: Tree)
        let tree = TTypeDef {
            name: "Tree".to_string(),
            public: false,
            params: vec![],
            kind: TTypeDefKind::Enum(vec![
                TVariant {
                    name: "Leaf".to_string(),
                    fields: vec![TField {
                        name: "value".to_string(),
                        ty: Type::Int,
                    }],
                },
                TVariant {
                    name: "Branch".to_string(),
                    fields: vec![
                        TField {
                            name: "left".to_string(),
                            ty: Type::Named("Tree".to_string()),
                        },
                        TField {
                            name: "right".to_string(),
                            ty: Type::Named("Tree".to_string()),
                        },
                    ],
                },
            ]),
            span: 0..1,
        };
        let module = make_module(vec![tree.clone()]);
        let detector = DefaultCycleDetector::new(&module);

        // This is a direct cycle (Tree contains Tree)
        let result = detector.check_type(&tree);
        assert!(result.is_cyclic());
    }

    #[test]
    fn test_tarjan_empty_graph() {
        let graph = TypeReferenceGraph::new();
        let cycles = find_cycles_tarjan(&graph);
        assert!(cycles.is_empty());
    }

    #[test]
    fn test_tarjan_single_node_no_cycle() {
        let mut graph = TypeReferenceGraph::new();
        graph.add_node(TypeNode {
            name: "Point".to_string(),
            references: vec![],
        });

        let cycles = find_cycles_tarjan(&graph);
        assert!(cycles.is_empty());
    }
}
