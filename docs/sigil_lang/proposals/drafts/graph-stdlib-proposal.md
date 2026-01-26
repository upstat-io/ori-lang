# Proposal: Graph Data Structure in Standard Library

**Status:** Draft
**Author:** Eric (with Claude)
**Created:** 2026-01-26

---

## Summary

Add `Graph<N, E>` to `std.collections` for representing directed and undirected graphs without reference cycles. Uses dual adjacency maps for O(1) bidirectional traversal.

```sigil
use std.collections.graph { Graph, NodeId }

let g = Graph.empty()
let (g, alice) = g.add_node(data: User { name: "Alice" })
let (g, bob) = g.add_node(data: User { name: "Bob" })
let g = g.add_edge_undirected(a: alice, b: bob, data: Friendship { since: "2020" })

let friends = g.neighbors(id: alice)  // [bob]
```

---

## Motivation

### The Problem

Graphs with bidirectional relationships (social networks, dependency graphs, navigation meshes) traditionally use object references:

```typescript
class User {
  friends: Set<User> = new Set();
  addFriend(other: User) {
    this.friends.add(other);
    other.friends.add(this);  // Cycle: alice ↔ bob
  }
}
```

This creates reference cycles that break ARC.

### The Solution

Represent relationships as data, not references:

```
Nodes don't reference nodes.
The graph structure maps IDs to IDs.

alice.friends.contains(bob)  // Object reference (cycle!)
graph.has_edge(from: alice_id, to: bob_id)  // ID lookup (no cycle)
```

### Why Dual Adjacency Maps?

| Approach | Add Edge | Query Out | Query In | Memory |
|----------|----------|-----------|----------|--------|
| Single adjacency map | O(1) | O(1) | O(n) scan | 1x |
| Edge list | O(1) | O(e) | O(e) | Compact |
| **Dual adjacency maps** | O(1) | O(1) | O(1) | 2x |

Social network queries go both ways ("who do I follow" and "who follows me"). Dual maps make both O(1).

---

## Design

### Core Types

```sigil
// Node identifier (opaque, prevents direct node-to-node references)
type NodeId = int

// Graph with node data N and edge data E
type Graph<N, E> = {
    nodes: {NodeId: N},
    outbound: {NodeId: [(NodeId, E)]},  // from → [(to, edge_data)]
    inbound: {NodeId: [(NodeId, E)]},   // to → [(from, edge_data)]
    next_id: NodeId,
}
```

### Construction

```sigil
@empty<N, E> () -> Graph<N, E>

@from_edges<N, E> (
    nodes: [(NodeId, N)],
    edges: [(NodeId, NodeId, E)],
) -> Graph<N, E>
```

### Node Operations

```sigil
// Add node, returns updated graph and assigned ID
@add_node<N, E> (self, data: N) -> (Graph<N, E>, NodeId)

// Get node data by ID
@get_node<N, E> (self, id: NodeId) -> Option<N>

// Update node data
@update_node<N, E> (self, id: NodeId, data: N) -> Graph<N, E>

// Remove node and all its edges
@remove_node<N, E> (self, id: NodeId) -> Graph<N, E>

// Check if node exists
@has_node<N, E> (self, id: NodeId) -> bool

// Get all node IDs
@node_ids<N, E> (self) -> [NodeId]

// Get node count
@node_count<N, E> (self) -> int
```

### Edge Operations

```sigil
// Add directed edge from → to
@add_edge<N, E> (self, from: NodeId, to: NodeId, data: E) -> Graph<N, E>

// Add undirected edge (adds both directions)
@add_edge_undirected<N, E: Clone> (self, a: NodeId, b: NodeId, data: E) -> Graph<N, E>

// Remove directed edge
@remove_edge<N, E> (self, from: NodeId, to: NodeId) -> Graph<N, E>

// Remove undirected edge (removes both directions)
@remove_edge_undirected<N, E> (self, a: NodeId, b: NodeId) -> Graph<N, E>

// Check if edge exists
@has_edge<N, E> (self, from: NodeId, to: NodeId) -> bool

// Get edge data
@get_edge<N, E> (self, from: NodeId, to: NodeId) -> Option<E>

// Update edge data
@update_edge<N, E> (self, from: NodeId, to: NodeId, data: E) -> Graph<N, E>

// Get edge count
@edge_count<N, E> (self) -> int
```

### Traversal Queries

```sigil
// Nodes this node points to (outgoing edges)
@successors<N, E> (self, id: NodeId) -> [NodeId]

// Nodes that point to this node (incoming edges)
@predecessors<N, E> (self, id: NodeId) -> [NodeId]

// For undirected graphs: same as successors
@neighbors<N, E> (self, id: NodeId) -> [NodeId]

// Outgoing edges with data
@outgoing_edges<N, E> (self, id: NodeId) -> [(NodeId, E)]

// Incoming edges with data
@incoming_edges<N, E> (self, id: NodeId) -> [(NodeId, E)]

// Degree (number of edges)
@out_degree<N, E> (self, id: NodeId) -> int
@in_degree<N, E> (self, id: NodeId) -> int
@degree<N, E> (self, id: NodeId) -> int  // out + in for directed, neighbors for undirected
```

### Graph Algorithms

```sigil
// Breadth-first traversal
@bfs<N, E> (self, start: NodeId, visit: (NodeId, N) -> void) -> void

// Depth-first traversal
@dfs<N, E> (self, start: NodeId, visit: (NodeId, N) -> void) -> void

// Shortest path (unweighted)
@shortest_path<N, E> (self, from: NodeId, to: NodeId) -> Option<[NodeId]>

// All paths between two nodes (with max depth)
@all_paths<N, E> (self, from: NodeId, to: NodeId, max_depth: int) -> [[NodeId]]

// Check if path exists
@is_reachable<N, E> (self, from: NodeId, to: NodeId) -> bool

// Topological sort (for DAGs)
@topological_sort<N, E> (self) -> Option<[NodeId]>  // None if cycle exists

// Detect cycle
@has_cycle<N, E> (self) -> bool

// Connected components
@connected_components<N, E> (self) -> [[NodeId]]

// Strongly connected components (directed)
@strongly_connected_components<N, E> (self) -> [[NodeId]]
```

### Transformation

```sigil
// Map over node data
@map_nodes<N, E, M> (self, transform: (NodeId, N) -> M) -> Graph<M, E>

// Map over edge data
@map_edges<N, E, F> (self, transform: (NodeId, NodeId, E) -> F) -> Graph<N, F>

// Filter nodes (removes edges to/from removed nodes)
@filter_nodes<N, E> (self, predicate: (NodeId, N) -> bool) -> Graph<N, E>

// Filter edges
@filter_edges<N, E> (self, predicate: (NodeId, NodeId, E) -> bool) -> Graph<N, E>

// Subgraph induced by node set
@subgraph<N, E> (self, nodes: Set<NodeId>) -> Graph<N, E>

// Reverse all edges
@reverse<N, E> (self) -> Graph<N, E>
```

---

## Examples

### Social Network

```sigil
use std.collections.graph { Graph, NodeId }

type User = { name: str, email: str }
type Friendship = { since: str, strength: int }

type SocialGraph = Graph<User, Friendship>

@create_network () -> SocialGraph = run(
    let g = Graph.empty(),
    let (g, alice) = g.add_node(data: User { name: "Alice", email: "alice@test.com" }),
    let (g, bob) = g.add_node(data: User { name: "Bob", email: "bob@test.com" }),
    let (g, carol) = g.add_node(data: User { name: "Carol", email: "carol@test.com" }),

    let g = g.add_edge_undirected(a: alice, b: bob, data: Friendship { since: "2020", strength: 5 }),
    let g = g.add_edge_undirected(a: bob, b: carol, data: Friendship { since: "2021", strength: 3 }),
    let g = g.add_edge_undirected(a: alice, b: carol, data: Friendship { since: "2022", strength: 4 }),

    g,
)

@mutual_friends (g: SocialGraph, a: NodeId, b: NodeId) -> [NodeId] = run(
    let a_friends = g.neighbors(id: a).to_set(),
    let b_friends = g.neighbors(id: b).to_set(),
    a_friends.intersect(other: b_friends).to_list(),
)

@friends_of_friends (g: SocialGraph, user: NodeId) -> [NodeId] = run(
    let direct = g.neighbors(id: user).to_set(),
    let fof = direct
        .to_list()
        .flat_map(transform: f -> g.neighbors(id: f))
        .to_set(),
    fof.remove(item: user).difference(other: direct).to_list(),
)

@suggest_friends (g: SocialGraph, user: NodeId) -> [(NodeId, int)] = run(
    // Score by number of mutual friends
    let candidates = friends_of_friends(g: g, user: user),
    candidates
        .map(transform: c -> (c, len(collection: mutual_friends(g: g, a: user, b: c))))
        .sort_by(key: (_, score) -> -score),
)
```

### Dependency Graph

```sigil
use std.collections.graph { Graph, NodeId }

type Package = { name: str, version: str }
type Dependency = { version_req: str, optional: bool }

type DepGraph = Graph<Package, Dependency>

@build_order (g: DepGraph) -> Result<[NodeId], Error> =
    g.topological_sort()
        .ok_or(error: Error { message: "Circular dependency detected" })

@all_dependencies (g: DepGraph, pkg: NodeId) -> Set<NodeId> = run(
    let deps = Set.empty(),
    g.bfs(
        start: pkg,
        visit: (id, _) -> deps = deps.insert(item: id),
    ),
    deps.remove(item: pkg),
)

@dependents (g: DepGraph, pkg: NodeId) -> [NodeId] =
    // Who depends on this package?
    g.predecessors(id: pkg)
```

### Navigation Mesh

```sigil
use std.collections.graph { Graph, NodeId }

type Waypoint = { x: float, y: float, name: str }
type Path = { distance: float, terrain: str }

type NavMesh = Graph<Waypoint, Path>

@find_route (mesh: NavMesh, from: NodeId, to: NodeId) -> Option<[NodeId]> =
    mesh.shortest_path(from: from, to: to)

@nearby_waypoints (mesh: NavMesh, pos: Waypoint, max_dist: float) -> [NodeId] =
    mesh.node_ids()
        .filter(predicate: id -> match(
            mesh.get_node(id: id),
            Some(wp) -> distance(a: pos, b: wp) <= max_dist,
            None -> false,
        ))
```

---

## ARC Safety

The graph structure is ARC-safe because:

1. **Nodes don't reference nodes** — The `nodes` map holds data, not references to other nodes.

2. **Edges are ID pairs** — `outbound` and `inbound` store `NodeId` (integers), not node values.

3. **No closure captures** — All operations are pure functions on the graph structure.

4. **Reference structure:**
   ```
   Graph
     ├── nodes: {NodeId: N}      (no cycles)
     ├── outbound: {NodeId: [...]}  (IDs only)
     └── inbound: {NodeId: [...]}   (IDs only)
   ```

The "bidirectional" relationship exists as data (two map entries), not as object references.

---

## Implementation Notes

### Memory Layout

```
Graph<N, E>:
  nodes:    HashMap<int, N>     // Node storage
  outbound: HashMap<int, Vec<(int, E)>>  // Forward edges
  inbound:  HashMap<int, Vec<(int, E)>>  // Reverse edges
  next_id:  int                 // ID counter
```

### Edge Storage Trade-offs

Edges are stored twice (in `outbound` and `inbound`). This doubles edge memory but provides O(1) bidirectional queries.

For memory-constrained scenarios, a single-map variant could be provided:

```sigil
type SparseGraph<N, E> = {
    nodes: {NodeId: N},
    edges: {NodeId: [(NodeId, E)]},  // outbound only
    next_id: NodeId,
}
// predecessors() is O(n) scan
```

### Multigraph Support

The current design allows multiple edges between the same pair of nodes (stored as separate list entries). For single-edge graphs:

```sigil
@add_edge_unique<N, E> (self, from: NodeId, to: NodeId, data: E) -> Graph<N, E> =
    if self.has_edge(from: from, to: to)
    then self.update_edge(from: from, to: to, data: data)
    else self.add_edge(from: from, to: to, data: data)
```

---

## Comparison to Other Libraries

| Library | Language | Approach |
|---------|----------|----------|
| petgraph | Rust | Index-based, multiple graph types |
| NetworkX | Python | Dict of dicts |
| JGraphT | Java | Object references + interfaces |
| **std.collections.graph** | Sigil | Dual adjacency maps, ID-based |

Sigil's approach is closest to NetworkX but with static typing and ARC safety.

---

## Future Extensions

### Weighted Shortest Path

```sigil
@dijkstra<N, E> (self, from: NodeId, to: NodeId, weight: (E) -> float) -> Option<([NodeId], float)>
@a_star<N, E> (self, from: NodeId, to: NodeId, heuristic: (NodeId) -> float) -> Option<[NodeId]>
```

### Graph I/O

```sigil
@to_dot<N: Printable, E: Printable> (self) -> str  // GraphViz DOT format
@to_adjacency_matrix<N, E> (self) -> [[bool]]
```

### Specialized Graph Types

```sigil
type DiGraph<N, E> = Graph<N, E>  // Alias for clarity
type UnGraph<N, E> = ...  // Enforces undirected invariant
type DAG<N, E> = ...  // Enforces acyclic invariant
```

---

## Summary

| Aspect | Decision |
|--------|----------|
| Storage | Dual adjacency maps |
| Node identity | Opaque `NodeId = int` |
| Edge direction | Directed by default, undirected helpers |
| Multigraph | Supported (multiple edges allowed) |
| ARC safety | ID-based, no object references |
| Language changes | None — stdlib only |

Provides a complete graph API while maintaining ARC safety through ID-based relationships.
