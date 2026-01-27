# Proposal: Zipper Data Structures in Standard Library

**Status:** Draft
**Author:** Claude (with Eric)
**Created:** 2026-01-26

---

## Summary

Add `Zipper<T>` and `TreeZipper<T>` to `std.collections` for efficient cursor-based traversal and editing of sequences and trees without reference cycles.

```ori
use std.collections { Zipper, TreeZipper }

// List zipper - O(1) cursor movement and editing
let z = Zipper.from_list(items: [1, 2, 3, 4, 5])?
let z = z.next()?           // focus: 2
let z = z.insert_before(value: 10)  // [1, 10, 2, 3, 4, 5], focus: 2

// Tree zipper - O(1) parent access without back-references
let tz = TreeZipper.from_tree(tree: my_tree)
let tz = tz.child(index: 0)?   // descend to first child
let tz = tz.parent()?          // back to root - no cycle needed
```

---

## Motivation

### The Problem

Ori's ARC memory model forbids bidirectional references to prevent cycles. This makes certain common patterns awkward:

1. **Doubly-linked list traversal** — Can't have `prev`/`next` pointers
2. **Tree parent access** — Can't store parent reference in child nodes
3. **Cursor-based editing** — Need efficient insert/delete at current position

Traditional solutions (array + indices) work but require manual index management and lose the elegance of direct traversal.

### The Solution

Zippers are a functional programming technique that represent a "focus" within a data structure by storing:
- The focused element
- The context needed to reconstruct the whole structure

This provides O(1) bidirectional traversal without any references between elements.

### Why This Fits Ori

1. **No language changes** — Zippers are just structs with methods
2. **No cycles** — Context is stored as lists, not back-references
3. **Immutable-friendly** — Operations return new zippers, old versions remain valid
4. **Matches sequential model** — Data flows through transformations

---

## Design

### List Zipper

```ori
// std.collections.zipper

type Zipper<T> = {
    before: [T],  // Elements before focus, reversed (head = immediate predecessor)
    focus: T,
    after: [T],   // Elements after focus (head = immediate successor)
}
```

**Visual representation:**

```
List:    [A, B, C, D, E]  with focus on C

Zipper:  before: [B, A]   (reversed!)
         focus:  C
         after:  [D, E]

Moving next:
         before: [C, B, A]
         focus:  D
         after:  [E]
```

### Core Operations

```ori
// Construction
@from_list (items: [T]) -> Option<Zipper<T>> =
    match(
        items,
        [] -> None,
        [first, ..rest] -> Some(Zipper { before: [], focus: first, after: rest }),
    )

@singleton (value: T) -> Zipper<T> =
    Zipper { before: [], focus: value, after: [] }

// Conversion back
@to_list (z: Zipper<T>) -> [T] =
    z.before.reverse() + [z.focus] + z.after

// Navigation
@next (z: Zipper<T>) -> Option<Zipper<T>> =
    match(
        z.after,
        [] -> None,
        [x, ..rest] -> Some(Zipper {
            before: [z.focus] + z.before,
            focus: x,
            after: rest,
        }),
    )

@prev (z: Zipper<T>) -> Option<Zipper<T>> =
    match(
        z.before,
        [] -> None,
        [x, ..rest] -> Some(Zipper {
            before: rest,
            focus: x,
            after: [z.focus] + z.after,
        }),
    )

@first (z: Zipper<T>) -> Zipper<T> =
    match(
        z.before,
        [] -> z,
        _ -> first(z: prev(z: z).unwrap()),
    )

@last (z: Zipper<T>) -> Zipper<T> =
    match(
        z.after,
        [] -> z,
        _ -> last(z: next(z: z).unwrap()),
    )

// Editing at focus
@replace (z: Zipper<T>, value: T) -> Zipper<T> =
    Zipper { before: z.before, focus: value, after: z.after }

@insert_before (z: Zipper<T>, value: T) -> Zipper<T> =
    Zipper { before: [value] + z.before, focus: z.focus, after: z.after }

@insert_after (z: Zipper<T>, value: T) -> Zipper<T> =
    Zipper { before: z.before, focus: z.focus, after: [value] + z.after }

@delete (z: Zipper<T>) -> Option<Zipper<T>> =
    match(
        (z.after, z.before),
        ([x, ..rest], _) -> Some(Zipper { before: z.before, focus: x, after: rest }),
        ([], [x, ..rest]) -> Some(Zipper { before: rest, focus: x, after: [] }),
        ([], []) -> None,
    )

// Query
@position (z: Zipper<T>) -> int =
    len(collection: z.before)

@length (z: Zipper<T>) -> int =
    len(collection: z.before) + 1 + len(collection: z.after)

@is_first (z: Zipper<T>) -> bool =
    is_empty(collection: z.before)

@is_last (z: Zipper<T>) -> bool =
    is_empty(collection: z.after)
```

### Tree Zipper

```ori
// std.collections.tree_zipper

type Tree<T> = { value: T, children: [Tree<T>] }

type TreeZipper<T> = {
    focus: Tree<T>,
    context: [Crumb<T>],
}

type Crumb<T> = {
    value: T,
    left: [Tree<T>],   // Siblings to the left
    right: [Tree<T>],  // Siblings to the right
}
```

**Visual representation:**

```
Tree:       A
          / | \
         B  C  D
           /|\
          E F G

Focus on F:

TreeZipper:
  focus: Tree { value: F, children: [] }
  context: [
    Crumb { value: C, left: [E], right: [G] },
    Crumb { value: A, left: [B], right: [D] },
  ]
```

### Tree Operations

```ori
@from_tree (tree: Tree<T>) -> TreeZipper<T> =
    TreeZipper { focus: tree, context: [] }

@to_tree (z: TreeZipper<T>) -> Tree<T> =
    root(z: z).focus

@parent (z: TreeZipper<T>) -> Option<TreeZipper<T>> =
    match(
        z.context,
        [] -> None,
        [crumb, ..rest] -> Some(TreeZipper {
            focus: Tree {
                value: crumb.value,
                children: crumb.left + [z.focus] + crumb.right,
            },
            context: rest,
        }),
    )

@root (z: TreeZipper<T>) -> TreeZipper<T> =
    match(
        parent(z: z),
        None -> z,
        Some(p) -> root(z: p),
    )

@child (z: TreeZipper<T>, index: int) -> Option<TreeZipper<T>> =
    run(
        let children = z.focus.children,
        if index < 0 || index >= len(collection: children)
        then None
        else Some(TreeZipper {
            focus: children[index],
            context: [Crumb {
                value: z.focus.value,
                left: children.take(n: index),
                right: children.drop(n: index + 1),
            }] + z.context,
        }),
    )

@first_child (z: TreeZipper<T>) -> Option<TreeZipper<T>> =
    child(z: z, index: 0)

@last_child (z: TreeZipper<T>) -> Option<TreeZipper<T>> =
    run(
        let n = len(collection: z.focus.children),
        if n == 0 then None else child(z: z, index: n - 1),
    )

@next_sibling (z: TreeZipper<T>) -> Option<TreeZipper<T>> =
    match(
        z.context,
        [] -> None,
        [crumb, ..rest] -> match(
            crumb.right,
            [] -> None,
            [sib, ..sibs] -> Some(TreeZipper {
                focus: sib,
                context: [Crumb {
                    value: crumb.value,
                    left: crumb.left + [z.focus],
                    right: sibs,
                }] + rest,
            }),
        ),
    )

@prev_sibling (z: TreeZipper<T>) -> Option<TreeZipper<T>> =
    match(
        z.context,
        [] -> None,
        [crumb, ..rest] -> match(
            crumb.left,
            [] -> None,
            _ -> run(
                let sib = crumb.left[# - 1],
                Some(TreeZipper {
                    focus: sib,
                    context: [Crumb {
                        value: crumb.value,
                        left: crumb.left.take(n: len(collection: crumb.left) - 1),
                        right: [z.focus] + crumb.right,
                    }] + rest,
                }),
            ),
        ),
    )

@path_to_root (z: TreeZipper<T>) -> [T] =
    [z.focus.value] + z.context.map(transform: c -> c.value)

@depth (z: TreeZipper<T>) -> int =
    len(collection: z.context)

// Editing
@replace_value (z: TreeZipper<T>, value: T) -> TreeZipper<T> =
    TreeZipper {
        focus: Tree { value: value, children: z.focus.children },
        context: z.context,
    }

@insert_child (z: TreeZipper<T>, index: int, child: Tree<T>) -> TreeZipper<T> =
    run(
        let children = z.focus.children,
        let new_children = children.take(n: index) + [child] + children.drop(n: index),
        TreeZipper {
            focus: Tree { value: z.focus.value, children: new_children },
            context: z.context,
        },
    )

@remove_child (z: TreeZipper<T>, index: int) -> TreeZipper<T> =
    run(
        let children = z.focus.children,
        let new_children = children.take(n: index) + children.drop(n: index + 1),
        TreeZipper {
            focus: Tree { value: z.focus.value, children: new_children },
            context: z.context,
        },
    )
```

---

## Examples

### Text Editor Cursor

```ori
use std.collections { Zipper }

type Editor = {
    lines: Zipper<str>,
}

@move_up (ed: Editor) -> Editor =
    Editor { lines: ed.lines.prev().unwrap_or(default: ed.lines) }

@move_down (ed: Editor) -> Editor =
    Editor { lines: ed.lines.next().unwrap_or(default: ed.lines) }

@insert_line (ed: Editor, text: str) -> Editor =
    Editor { lines: ed.lines.insert_after(value: text).next().unwrap() }

@delete_line (ed: Editor) -> Editor =
    Editor { lines: ed.lines.delete().unwrap_or(default: Zipper.singleton(value: "")) }

@current_line (ed: Editor) -> str =
    ed.lines.focus
```

### Playlist Navigation

```ori
use std.collections { Zipper }

type Playlist = Zipper<Song>

@now_playing (pl: Playlist) -> Song = pl.focus

@skip (pl: Playlist) -> Playlist =
    pl.next().unwrap_or(default: pl.first())  // Loop to start

@previous (pl: Playlist) -> Playlist =
    pl.prev().unwrap_or(default: pl.last())   // Loop to end

@remove_current (pl: Playlist) -> Option<Playlist> =
    pl.delete()

@queue_next (pl: Playlist, song: Song) -> Playlist =
    pl.insert_after(value: song)
```

### DOM-like Tree Manipulation

```ori
use std.collections { TreeZipper, Tree }

type Element = { tag: str, attrs: {str: str} }

@find_by_id (z: TreeZipper<Element>, id: str) -> Option<TreeZipper<Element>> =
    if z.focus.value.attrs["id"] == Some(id)
    then Some(z)
    else run(
        let child_count = len(collection: z.focus.children),
        recurse(
            condition: self.i >= child_count,
            base: None,
            step: match(
                find_by_id(z: z.child(index: self.i)?, id: id),
                Some(found) -> found,
                None -> self(i: self.i + 1),
            ),
        )(i: 0),
    )

@get_path (z: TreeZipper<Element>) -> str =
    "/" + z.path_to_root().reverse().map(transform: e -> e.tag).join(sep: "/")

@append_child (z: TreeZipper<Element>, tag: str) -> TreeZipper<Element> =
    run(
        let new_child = Tree { value: Element { tag: tag, attrs: {} }, children: [] },
        z.insert_child(index: len(collection: z.focus.children), child: new_child),
    )
```

### Undo with Zipper History

```ori
use std.collections { Zipper }

type Document = { content: str }

type UndoableDoc = {
    history: Zipper<Document>,
}

@edit (doc: UndoableDoc, new_content: str) -> UndoableDoc =
    run(
        // Clear any redo history, add new state
        let new_doc = Document { content: new_content },
        let updated = Zipper {
            before: [doc.history.focus] + doc.history.before,
            focus: new_doc,
            after: [],  // Clear redo stack
        },
        UndoableDoc { history: updated },
    )

@undo (doc: UndoableDoc) -> UndoableDoc =
    UndoableDoc { history: doc.history.prev().unwrap_or(default: doc.history) }

@redo (doc: UndoableDoc) -> UndoableDoc =
    UndoableDoc { history: doc.history.next().unwrap_or(default: doc.history) }

@current (doc: UndoableDoc) -> Document =
    doc.history.focus

@can_undo (doc: UndoableDoc) -> bool =
    !doc.history.is_first()

@can_redo (doc: UndoableDoc) -> bool =
    !doc.history.is_last()
```

---

## Complexity Analysis

### List Zipper

| Operation | Time | Space |
|-----------|------|-------|
| `next` / `prev` | O(1) | O(1) |
| `first` / `last` | O(n) | O(1) |
| `insert_before` / `insert_after` | O(1) | O(1) |
| `delete` | O(1) | O(1) |
| `to_list` | O(n) | O(n) |
| `position` | O(n)* | O(1) |

*Position requires counting `before` length. Can be cached if needed frequently.

### Tree Zipper

| Operation | Time | Space |
|-----------|------|-------|
| `parent` | O(k) | O(1) |
| `child` | O(k) | O(1) |
| `root` | O(d) | O(1) |
| `next_sibling` / `prev_sibling` | O(1) | O(1) |
| `to_tree` | O(n) | O(n) |

Where k = number of children at current level, d = depth, n = total nodes.

---

## Design Rationale

### Why Reversed `before` List?

The `before` list is stored reversed so that the most recently passed element is at the head. This makes `next` and `prev` both O(1) — just cons/uncons operations.

If `before` were in natural order, `prev` would need to access the last element, which is O(n) for a list.

### Why Not Array + Index?

Arrays with indices work but have drawbacks:

| Aspect | Zipper | Array + Index |
|--------|--------|---------------|
| Insert at cursor | O(1) | O(n) shift |
| Delete at cursor | O(1) | O(n) shift |
| Random access | O(n) | O(1) |
| Immutable-friendly | Yes | Requires copy |
| Index invalidation | N/A | Must track |

Zippers are better for cursor-centric use cases (editors, navigation). Arrays are better for random access.

### Why Separate Tree Type?

`TreeZipper<T>` requires a specific tree structure (`Tree<T> = { value, children }`). This is intentional:

1. The zipper needs to know how to navigate children
2. User-defined trees can implement `ToTree` / `FromTree` traits for conversion
3. Keeps the core implementation simple

---

## Future Extensions

### Generic Zipper Trait

```ori
trait Zippable<Z> {
    type Focus
    @focus (self) -> Self.Focus
    @replace (self, value: Self.Focus) -> Self
}
```

### Bidirectional Zipper (Infinite)

```ori
type BiZipper<T> = {
    focus: T,
    left: () -> (T, BiZipper<T>),   // Lazy infinite left
    right: () -> (T, BiZipper<T>),  // Lazy infinite right
}
```

### Rose Tree Zipper

Support for trees with variable-width nodes (forests).

---

## Summary

**What's added:**
- `Zipper<T>` — cursor-based list traversal and editing
- `TreeZipper<T>` — cursor-based tree traversal with parent access

**Why it matters:**
- Enables doubly-linked list semantics without cycles
- Enables parent access in trees without back-references
- Pure functional, immutable-friendly
- O(1) cursor operations

**No language changes required** — this is purely a standard library addition using existing Ori features.
