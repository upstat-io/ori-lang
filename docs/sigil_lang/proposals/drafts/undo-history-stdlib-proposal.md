# Proposal: Undo History in Standard Library

**Status:** Draft
**Author:** Eric (with Claude)
**Created:** 2026-01-26

---

## Summary

Add `History<T>` to `std.collections` for undo/redo functionality using persistent data structures instead of the command pattern. Keep old states rather than computing inverses.

```sigil
use std.collections { History }

let h = History.new(initial: Document { content: "" })
let h = h.push(state: Document { content: "Hello" })
let h = h.push(state: Document { content: "Hello World" })
let h = h.undo()   // Back to "Hello"
let h = h.redo()   // Forward to "Hello World"
```

---

## Motivation

### The Problem

The traditional command pattern stores operations with `execute()` and `undo()` methods:

```typescript
interface Command {
  execute(): void;
  undo(): void;
}

class Editor {
  history: Command[] = [];

  insert(text: string) {
    const cmd = {
      execute: () => { this.content += text; },
      undo: () => { this.content = this.content.slice(0, -text.length); },
    };
    cmd.execute();
    this.history.push(cmd);
  }
}
```

Problems:
1. **Cycles** — Commands capture `this`, creating `Editor → history → Command → this → Editor`
2. **Inverse bugs** — Computing `undo()` correctly is error-prone
3. **State coupling** — Commands depend on execution order

### The Solution

Don't compute undo — keep the old state:

```sigil
type History<T> = {
    past: [T],      // Previous states
    present: T,     // Current state
    future: [T],    // Undone states (for redo)
}
```

Undo just swaps `present` with the previous state. No inverse computation needed.

### Why This Works in Sigil

1. **Structural sharing** — Sigil's immutable data structures share unchanged parts. "Hello World" and "Hello" share the "Hello" bytes.

2. **Value semantics** — States are values, not references. No cycles possible.

3. **Pure operations** — `insert_text(doc, text)` returns a new document. No `this` capture.

---

## Design

### Core Type

```sigil
type History<T> = {
    past: [T],
    present: T,
    future: [T],
}
```

Visual model:
```
past:    [S2, S1, S0]  (most recent first)
present: S3
future:  [S4, S5]      (most recent first, for redo)

Timeline: S0 → S1 → S2 → [S3] → S4 → S5
                         ↑ current
```

### Construction

```sigil
@new<T> (initial: T) -> History<T> =
    History { past: [], present: initial, future: [] }

@from_state<T> (state: T) -> History<T> =
    History.new(initial: state)
```

### Core Operations

```sigil
// Add new state (clears redo stack)
@push<T> (self, state: T) -> History<T> =
    History {
        past: [self.present] + self.past,
        present: state,
        future: [],  // New edit clears redo
    }

// Undo: move back one state
@undo<T> (self) -> History<T> =
    match(
        self.past,
        [] -> self,  // Nothing to undo
        [prev, ..rest] -> History {
            past: rest,
            present: prev,
            future: [self.present] + self.future,
        },
    )

// Redo: move forward one state
@redo<T> (self) -> History<T> =
    match(
        self.future,
        [] -> self,  // Nothing to redo
        [next, ..rest] -> History {
            past: [self.present] + self.past,
            present: next,
            future: rest,
        },
    )

// Get current state
@current<T> (self) -> T =
    self.present

// Modify current state (convenience for push)
@modify<T> (self, f: (T) -> T) -> History<T> =
    self.push(state: f(self.present))
```

### Query Operations

```sigil
@can_undo<T> (self) -> bool =
    !self.past.is_empty()

@can_redo<T> (self) -> bool =
    !self.future.is_empty()

@undo_count<T> (self) -> int =
    len(collection: self.past)

@redo_count<T> (self) -> int =
    len(collection: self.future)

// Total states in history
@history_size<T> (self) -> int =
    len(collection: self.past) + 1 + len(collection: self.future)
```

### Bulk Operations

```sigil
// Undo multiple steps
@undo_n<T> (self, n: int) -> History<T> =
    (0..n).fold(initial: self, op: (h, _) -> h.undo())

// Redo multiple steps
@redo_n<T> (self, n: int) -> History<T> =
    (0..n).fold(initial: self, op: (h, _) -> h.redo())

// Go to specific point in history
@go_to<T> (self, index: int) -> History<T> = run(
    let current_index = len(collection: self.past),
    if index < current_index
    then self.undo_n(n: current_index - index)
    else self.redo_n(n: index - current_index),
)

// Undo all
@undo_all<T> (self) -> History<T> =
    self.undo_n(n: len(collection: self.past))

// Redo all
@redo_all<T> (self) -> History<T> =
    self.redo_n(n: len(collection: self.future))
```

### Memory Management

```sigil
// Limit history depth (drop oldest)
@limit<T> (self, max_undo: int) -> History<T> =
    History {
        past: self.past.take(n: max_undo),
        present: self.present,
        future: self.future,
    }

// Clear all history, keep current
@clear_history<T> (self) -> History<T> =
    History { past: [], present: self.present, future: [] }

// Checkpoint: collapse history to single state
@checkpoint<T> (self) -> History<T> =
    History.new(initial: self.present)
```

---

## Examples

### Text Editor

```sigil
use std.collections { History }

type Document = {
    content: str,
    cursor: int,
}

type Editor = History<Document>

@new_editor () -> Editor =
    History.new(initial: Document { content: "", cursor: 0 })

// Operations are pure functions
@insert_text (doc: Document, text: str) -> Document =
    Document {
        content: doc.content.slice(end: doc.cursor)
            + text
            + doc.content.slice(start: doc.cursor),
        cursor: doc.cursor + len(collection: text),
    }

@delete_char (doc: Document) -> Document =
    if doc.cursor == 0
    then doc
    else Document {
        content: doc.content.slice(end: doc.cursor - 1)
            + doc.content.slice(start: doc.cursor),
        cursor: doc.cursor - 1,
    }

@move_cursor (doc: Document, offset: int) -> Document =
    Document {
        content: doc.content,
        cursor: max(left: 0, right: min(left: doc.cursor + offset, right: len(collection: doc.content))),
    }

// Editor operations push new states
@editor_insert (ed: Editor, text: str) -> Editor =
    ed.modify(f: doc -> insert_text(doc: doc, text: text))

@editor_delete (ed: Editor) -> Editor =
    ed.modify(f: delete_char)

@editor_move (ed: Editor, offset: int) -> Editor =
    ed.modify(f: doc -> move_cursor(doc: doc, offset: offset))

// Usage
@example_editor () -> void = run(
    let ed = new_editor(),
    let ed = editor_insert(ed: ed, text: "Hello"),
    let ed = editor_insert(ed: ed, text: " World"),

    assert_eq(actual: ed.current().content, expected: "Hello World"),

    let ed = ed.undo(),
    assert_eq(actual: ed.current().content, expected: "Hello"),

    let ed = ed.undo(),
    assert_eq(actual: ed.current().content, expected: ""),

    let ed = ed.redo(),
    assert_eq(actual: ed.current().content, expected: "Hello"),

    // New edit clears redo
    let ed = editor_insert(ed: ed, text: "!!!"),
    assert_eq(actual: ed.current().content, expected: "Hello!!!"),
    assert(condition: !ed.can_redo()),
)
```

### Drawing Application

```sigil
use std.collections { History }

type Point = { x: float, y: float }
type Shape = Circle { center: Point, radius: float }
         | Rectangle { top_left: Point, width: float, height: float }
         | Line { start: Point, end: Point }

type Canvas = { shapes: [Shape], selected: Option<int> }

type DrawingApp = History<Canvas>

@add_shape (canvas: Canvas, shape: Shape) -> Canvas =
    Canvas { shapes: canvas.shapes + [shape], selected: Some(len(collection: canvas.shapes)) }

@delete_selected (canvas: Canvas) -> Canvas =
    match(
        canvas.selected,
        None -> canvas,
        Some(idx) -> Canvas {
            shapes: canvas.shapes.remove_at(index: idx),
            selected: None,
        },
    )

@move_selected (canvas: Canvas, dx: float, dy: float) -> Canvas =
    match(
        canvas.selected,
        None -> canvas,
        Some(idx) -> Canvas {
            shapes: canvas.shapes.update_at(
                index: idx,
                f: shape -> translate_shape(shape: shape, dx: dx, dy: dy),
            ),
            selected: canvas.selected,
        },
    )

// App operations
@app_add_circle (app: DrawingApp, center: Point, radius: float) -> DrawingApp =
    app.modify(f: c -> add_shape(canvas: c, shape: Circle { center: center, radius: radius }))

@app_delete (app: DrawingApp) -> DrawingApp =
    app.modify(f: delete_selected)

@app_move (app: DrawingApp, dx: float, dy: float) -> DrawingApp =
    app.modify(f: c -> move_selected(canvas: c, dx: dx, dy: dy))
```

### Game State

```sigil
use std.collections { History }

type GameState = {
    player_pos: Point,
    health: int,
    inventory: [Item],
    level: int,
}

type Game = History<GameState>

@move_player (state: GameState, dir: Direction) -> GameState =
    GameState { ..state, player_pos: apply_direction(pos: state.player_pos, dir: dir) }

@take_damage (state: GameState, amount: int) -> GameState =
    GameState { ..state, health: max(left: 0, right: state.health - amount) }

@pickup_item (state: GameState, item: Item) -> GameState =
    GameState { ..state, inventory: state.inventory + [item] }

// Undo last move (for puzzle games, turn-based games)
@game_undo (game: Game) -> Game =
    game.undo()

// Checkpoint at level start
@start_level (game: Game, level: int) -> Game =
    game.checkpoint().modify(f: s -> GameState { ..s, level: level })
```

### Form with Validation

```sigil
use std.collections { History }

type FormData = {
    name: str,
    email: str,
    age: Option<int>,
}

type FormState = {
    data: FormData,
    errors: {str: str},
}

type Form = History<FormState>

@update_field (state: FormState, field: str, value: str) -> FormState = run(
    let new_data = match(
        field,
        "name" -> FormData { ..state.data, name: value },
        "email" -> FormData { ..state.data, email: value },
        "age" -> FormData { ..state.data, age: parse_int(s: value).ok() },
        _ -> state.data,
    ),
    let errors = validate_form(data: new_data),
    FormState { data: new_data, errors: errors },
)

// User can undo form changes
@form_set (form: Form, field: str, value: str) -> Form =
    form.modify(f: s -> update_field(state: s, field: field, value: value))

@form_reset (form: Form) -> Form =
    form.undo_all()
```

---

## Grouped Undo

For operations that should undo together:

```sigil
type GroupedHistory<T> = {
    history: History<T>,
    pending: [T],  // Group being built
}

@begin_group<T> (self) -> GroupedHistory<T> =
    GroupedHistory { history: self.history, pending: [] }

@group_push<T> (self, state: T) -> GroupedHistory<T> =
    GroupedHistory { history: self.history, pending: self.pending + [state] }

@end_group<T> (self) -> History<T> =
    match(
        self.pending,
        [] -> self.history,
        [final, ..] -> self.history.push(state: final),  // Only keep final state
    )

// Usage: drag operation should undo as one step
@drag_shape (app: DrawingApp, moves: [Point]) -> DrawingApp = run(
    let grouped = app.begin_group(),
    let grouped = moves.fold(
        initial: grouped,
        op: (g, pos) -> g.group_push(state: move_to(canvas: g.current(), pos: pos)),
    ),
    grouped.end_group(),
)
```

---

## Delta Compression (Optional)

For large states, store deltas instead of full snapshots:

```sigil
type Delta<T> = {
    apply: (T) -> T,
    reverse: (T) -> T,
}

type CompactHistory<T> = {
    base: T,
    past_deltas: [Delta<T>],
    future_deltas: [Delta<T>],
    current_index: int,
}

@apply_to<T> (self, index: int) -> T =
    if index <= self.current_index
    then self.past_deltas
        .take(n: self.current_index - index)
        .fold(initial: self.current(), op: (state, delta) -> delta.reverse(state))
    else self.future_deltas
        .take(n: index - self.current_index)
        .fold(initial: self.current(), op: (state, delta) -> delta.apply(state))
```

This trades complexity for memory. Only use when states are large and deltas are small.

---

## ARC Safety

The history structure is ARC-safe because:

1. **No closures stored** — Operations are pure functions, not captured methods.

2. **States are values** — Each state in `past`/`future` is an independent value.

3. **No back-references** — States don't reference the History that contains them.

4. **Reference structure:**
   ```
   History<T>
     ├── past: [T, T, T]      (independent values)
     ├── present: T
     └── future: [T, T]       (independent values)
   ```

5. **Structural sharing** — Sigil's immutable collections share unchanged parts, making storage efficient despite keeping full states.

---

## Comparison to Command Pattern

| Aspect | Command Pattern | History<T> |
|--------|-----------------|------------|
| Undo mechanism | Compute inverse | Restore old state |
| Redo mechanism | Re-execute | Restore newer state |
| Bug potential | High (inverse errors) | Low (just swap values) |
| Memory | O(operations) | O(states) |
| Cycles | Yes (captures `this`) | No |
| ARC safe | No | Yes |

With structural sharing, `O(states)` memory is often comparable to `O(operations)` for typical edit patterns.

---

## Summary

| Aspect | Decision |
|--------|----------|
| Storage | Full states with structural sharing |
| Undo | Swap present with previous |
| Redo | Swap present with next |
| Grouping | Collapse to single final state |
| Compression | Optional delta mode |
| ARC safety | Yes — no closures, no cycles |
| Language changes | None — stdlib only |

Simple, correct, and impossible to get wrong. The "just keep old states" approach eliminates entire categories of bugs.
