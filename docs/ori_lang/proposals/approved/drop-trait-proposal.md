# Proposal: Drop Trait

**Status:** Approved
**Approved:** 2026-01-30
**Author:** Eric (with AI assistance)
**Created:** 2026-01-30
**Affects:** Compiler, memory model, traits

---

## Summary

This proposal formalizes the `Drop` trait for custom destructors, including execution timing, constraints, and interaction with ARC.

---

## Problem Statement

The spec mentions `Drop` cannot be async but leaves unclear:

1. **Definition**: What is the exact trait signature?
2. **Timing**: When is `drop` called?
3. **Order**: What order are nested drops called?
4. **Constraints**: What can/cannot be done in drop?
5. **Panics**: What happens if drop panics?

---

## Definition

```ori
trait Drop {
    @drop (self) -> void
}
```

The `drop` method is called when a value's reference count reaches zero.

---

## Execution Timing

### Reference Count Zero

Drop is called when ARC refcount reaches zero:

```ori
{
    let resource = acquire_resource(),  // refcount: 1
    use_resource(resource),             // refcount may increase
}                                       // refcount: 0, drop called
```

### Scope Exit

For values not shared, drop occurs at scope exit:

```ori
@process () -> void = {
    let file = open_file(path),  // Created
    read_all(file)
    // drop(file) called here — end of scope
}
```

### Early Return

Drop is called on early returns:

```ori
@process () -> Result<Data, Error> = {
    let file = open_file(path),  // Created
    let content = read(file)?,   // If Err, file is dropped before return
    Ok(parse(content))
    // If Ok, file is dropped here
}
```

---

## Drop Order

### LIFO (Last In, First Out)

Values are dropped in reverse declaration order:

```ori
{
    let a = Resource { name: "a" }
    let b = Resource { name: "b" }
    let c = Resource { name: "c" }
}
// Drop order: c, b, a
```

### Nested Structs

Fields are dropped in reverse declaration order, then the struct:

```ori
type Container = { first: Resource, second: Resource }

{
    let c = Container { first: r1, second: r2 }
}
// Drop order: r2, r1 (reverse field order)
```

### Collections

Collection elements are dropped in reverse order (back-to-front):

```ori
{
    let list = [r1, r2, r3]
}
// Drop order: r3, r2, r1 (back-to-front)
```

---

## Constraints

### No Async Operations

Drop cannot perform async operations:

```ori
impl Drop for Connection {
    @drop (self) -> void = {
        self.send_goodbye(),  // ERROR if async
        self.close()
    }
}
```

### Rationale

- Drop runs synchronously during stack unwinding
- Async operations could deadlock
- Runtime may not be available during cleanup

### No Self Return

Drop must return `void`:

```ori
impl Drop for Resource {
    @drop (self) -> void = cleanup(self)  // OK
    @drop (self) -> bool = ...            // ERROR: must return void
}
```

### Must Not Panic During Unwind

If drop panics during panic unwinding (double panic), the program aborts:

```ori
impl Drop for Bad {
    @drop (self) -> void = panic(msg: "drop failed")  // Dangerous
}

{
    let bad = Bad {}
    panic(msg: "first panic"),  // During unwind, Bad.drop panics -> ABORT
}
```

---

## Standard Implementations

### Types Without Drop

Most types don't need `Drop`:
- Primitives: `int`, `float`, `bool`, `str`, `char`, `byte`
- Simple collections: `[T]`, `{K: V}`, `Set<T>` (elements dropped automatically)
- Options and Results: `Option<T>`, `Result<T, E>` (values dropped automatically)

### Types With Drop

Types wrapping external resources typically implement Drop:

```ori
impl Drop for FileHandle {
    @drop (self) -> void = close_file_descriptor(self.fd)
}

impl Drop for Connection {
    @drop (self) -> void = close_socket(self.socket)
}

impl Drop for Lock {
    @drop (self) -> void = release_lock(self.handle)
}
```

---

## Derivation

### Not Derivable

`Drop` cannot be derived:

```ori
#derive(Drop)  // ERROR: Drop is not derivable
type Resource = { ... }
```

### Rationale

Drop behavior is highly specific to each type. Automatic derivation would be either:
- No-op (useless)
- Wrong (incorrect cleanup)

---

## Interaction with Clone

### Clone Creates Independent Value

Cloning does NOT share drop responsibility:

```ori
let a = Resource { ... }
let b = a.clone()
// a and b drop independently
```

### Reference Sharing

When values share references via ARC:

```ori
let a = acquire_resource()
let b = a  // ARC: refcount 2
drop_early(a)  // refcount 1
// b still valid
// When b goes out of scope: refcount 0, drop called
```

---

## Explicit Drop

### drop_early Function

Force drop before scope exit:

```ori
@drop_early<T> (value: T) -> void = ()  // Takes ownership, value is dropped

{
    let file = open_file(path)
    let content = read_all(file)
    drop_early(file),  // Close immediately
    // ... continue processing content
}
```

### Rationale

Allows releasing resources early when no longer needed.

---

## Custom Drop Examples

### Resource Cleanup

```ori
type TempFile = { path: str }

impl Drop for TempFile {
    @drop (self) -> void = delete_file(self.path)
}

@with_temp_file<T> (f: (TempFile) -> T) -> T = {
    let temp = TempFile { path: create_temp() }
    f(temp)
    // temp.drop() automatically deletes file
}
```

### Logging

```ori
type TimedOperation = { name: str, start: Duration }

impl Drop for TimedOperation {
    @drop (self) -> void = {
        let elapsed = now() - self.start
        log(msg: `{self.name} took {elapsed}`)
    }
}

@measure<T> (name: str, f: () -> T) -> T = {
    let op = TimedOperation { name: name, start: now() }
    f()
    // Logs duration when op is dropped
}
```

### Reference Counting Debug

```ori
type RefCounted<T> = { value: T, id: int }

impl<T> Drop for RefCounted<T> {
    @drop (self) -> void = log(msg: `RefCounted {self.id} dropped`)
}
```

---

## Error Handling in Drop

### Swallow Errors

Drop should handle its own errors:

```ori
impl Drop for Connection {
    @drop (self) -> void = match self.close() {
        Ok(_) -> ()
        Err(e) -> log(msg: `close failed: {e}`),  // Log, don't propagate
    }
}
```

### Rationale

Drop cannot return errors. Propagating would require panic, which is dangerous during unwinding.

---

## Error Messages

### Async in Drop

```
error[E0980]: `Drop::drop` cannot be async
  --> src/types.ori:5:5
   |
 5 |     @drop (self) -> void uses Suspend = self.async_cleanup()
   |                          ^^^^^^^^^^ `uses Suspend` not allowed
   |
   = note: drop runs synchronously during stack unwinding
   = help: perform async cleanup before dropping
```

### Wrong Return Type

```
error[E0981]: `Drop::drop` must return `void`
  --> src/types.ori:5:25
   |
 5 |     @drop (self) -> bool = ...
   |                     ^^^^ expected `void`
```

---

## Spec Changes Required

### Update `07-properties-of-types.md`

Add Drop trait section with:
1. Trait definition
2. Execution timing
3. Order guarantees
4. Constraints

### Update `15-memory-model.md`

Document Drop's role in ARC.

---

## Summary

| Aspect | Details |
|--------|---------|
| Trait | `trait Drop { @drop (self) -> void }` |
| Called when | Reference count reaches zero |
| Order | LIFO (reverse declaration order) |
| Field order | Reverse declaration order |
| Collection order | Back-to-front (reverse index order) |
| Async | Not allowed |
| Panic in drop | Abort if during unwind |
| Derivable | No |
| Explicit | Use `drop_early(value)` |
