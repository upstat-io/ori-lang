# Proposal: With Pattern (Resource Acquisition)

**Status:** Draft
**Author:** Eric (with AI assistance)
**Created:** 2026-01-30
**Affects:** Compiler, patterns, resource management

---

## Summary

This proposal formalizes the `with` pattern semantics for resource acquisition, use, and guaranteed release.

---

## Problem Statement

The spec documents the `with` pattern syntax but leaves unclear:

1. **Release guarantee**: Under what conditions does release run?
2. **Exception handling**: What happens if acquire, use, or release fails?
3. **Resource lifetime**: How is the resource scoped?
4. **Nesting behavior**: How do nested `with` patterns behave?
5. **Async context**: How does `with` interact with concurrency?

---

## Syntax

```ori
with(
    acquire: expression,
    use: resource -> expression,
    release: resource -> expression,
)
```

All three parameters are required.

---

## Semantics

### Execution Order

1. Evaluate `acquire` expression
2. If `acquire` succeeds, bind result to `resource`
3. Evaluate `use` lambda with `resource`
4. **Always** evaluate `release` lambda with `resource`
5. Return value of `use` expression

### Guarantee

The `release` expression **always executes** if `acquire` succeeds, regardless of:
- Normal completion of `use`
- Panic during `use`
- Error propagation (`?`) in `use`
- Early return from `use` via `break`/`continue`

### Type Signature

```ori
with<R, T>(
    acquire: R,
    use: (R) -> T,
    release: (R) -> void,
) -> T
```

Where:
- `R` is the resource type
- `T` is the return type
- `release` must return `void`

---

## Acquisition Semantics

### Successful Acquisition

```ori
with(
    acquire: open_file(path),  // Returns FileHandle
    use: f -> read_all(f),
    release: f -> close(f),
)
```

### Fallible Acquisition

If `acquire` evaluates to `Result<R, E>`:

```ori
with(
    acquire: open_file(path)?,  // Propagate error if Err
    use: f -> read_all(f),
    release: f -> close(f),
)
```

The `?` propagates before `with` binds the resource. If acquisition fails, `release` does not run.

### Acquisition Panic

If `acquire` panics, `release` does not run. The panic propagates normally.

---

## Release Semantics

### Normal Release

```ori
with(
    acquire: open_file(path),
    use: f -> process(f),
    release: f -> run(
        flush(f),
        close(f),
    ),
)
```

### Release Panic

If `release` panics:
- During normal unwinding: panic propagates
- During panic unwinding (double fault): program aborts

```ori
with(
    acquire: get_resource(),
    use: r -> panic(msg: "use failed"),  // First panic
    release: r -> panic(msg: "release failed"),  // Abort: double fault
)
```

### Release Errors

The `release` lambda must return `void`. If cleanup can fail, handle within release:

```ori
with(
    acquire: open_file(path),
    use: f -> read_all(f),
    release: f -> match(
        close(f),
        Ok(_) -> (),
        Err(e) -> log(msg: `close failed: {e}`),
    ),
)
```

Do not use `?` in release — errors should be logged, not propagated.

---

## Use Phase Semantics

### Normal Completion

```ori
let content = with(
    acquire: open_file(path),
    use: f -> read_all(f),  // Returns str
    release: f -> close(f),
)
// content: str
```

### Error Propagation

```ori
@load (path: str) -> Result<Data, Error> = with(
    acquire: open_file(path),
    use: f -> run(
        let content = read_all(f)?,  // May return early
        let data = parse(content)?,
        Ok(data),
    ),
    release: f -> close(f),  // Still runs on early return
)
```

### Panic in Use

```ori
with(
    acquire: open_file(path),
    use: f -> run(
        let data = process(f),
        if !valid(data) then panic(msg: "invalid"),  // Panic
        data,
    ),
    release: f -> close(f),  // Still runs during unwinding
)
```

---

## Nesting

### Sequential Resources

```ori
with(
    acquire: connect(db_url),
    use: conn -> with(
        acquire: begin_transaction(conn),
        use: tx -> run(
            execute(tx, query),
            commit(tx),
        ),
        release: tx -> if !tx.committed then rollback(tx),
    ),
    release: conn -> disconnect(conn),
)
```

Release order is inside-out: inner resources release before outer.

### Parallel Resources

For independent resources, use `run` with multiple `with`:

```ori
run(
    let result_a = with(acquire: a, use: ..., release: ...),
    let result_b = with(acquire: b, use: ..., release: ...),
    combine(result_a, result_b),
)
```

---

## Async Context

### With in Async

The `with` pattern works in async contexts:

```ori
@fetch_data (url: str) -> Result<Data, Error> uses Async = with(
    acquire: connect(url),
    use: conn -> run(
        let response = conn.request(method: "GET"),  // Async operation
        parse(response),
    ),
    release: conn -> conn.close(),  // Release runs after async use completes
)
```

### Release During Cancellation

If a task is cancelled during `use`:
1. Cancellation is deferred until a checkpoint
2. At the checkpoint, unwinding begins
3. `release` runs during unwinding
4. Task terminates after release completes

```ori
nursery(
    body: n -> n.spawn(task: () -> with(
        acquire: expensive_resource(),
        use: r -> long_operation(r),  // Cancelled here
        release: r -> cleanup(r),  // Still runs
    )),
    on_error: FailFast,
    timeout: 1s,  // Timeout triggers cancellation
)
```

---

## Capability Interaction

### Resource Capabilities

If the resource requires capabilities, declare them on the enclosing function:

```ori
@process_file (path: str) -> Data uses FileSystem = with(
    acquire: open_file(path),  // Requires FileSystem
    use: f -> transform(read_all(f)),
    release: f -> close(f),
)
```

### Capability in Release

If release requires capabilities, they must also be declared:

```ori
@with_logging (path: str) -> Data uses FileSystem, Logger = with(
    acquire: open_file(path),
    use: f -> read_all(f),
    release: f -> run(
        log(msg: "closing file"),  // Requires Logger
        close(f),
    ),
)
```

---

## Common Patterns

### File I/O

```ori
@read_file (path: str) -> Result<str, Error> uses FileSystem = with(
    acquire: open_file(path)?,
    use: f -> Ok(read_all(f)),
    release: f -> close(f),
)
```

### Database Transaction

```ori
@in_transaction<T> (conn: Connection, op: (Transaction) -> T) -> Result<T, Error> = with(
    acquire: conn.begin()?,
    use: tx -> run(
        let result = op(tx),
        tx.commit()?,
        Ok(result),
    ),
    release: tx -> if !tx.committed then tx.rollback() else (),
)
```

### Lock Acquisition

```ori
@with_lock<T> (lock: Lock, op: () -> T) -> T = with(
    acquire: lock.acquire(),
    use: _ -> op(),
    release: _ -> lock.release(),
)
```

---

## Error Messages

### Missing Release

```
error[E0860]: `with` pattern missing required parameter
  --> src/main.ori:5:1
   |
 5 | with(acquire: open_file(p), use: f -> read_all(f))
   | ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
   |
   = note: `release` parameter is required
   = help: add `release: f -> close(f)`
```

### Non-Void Release

```
error[E0861]: `release` must return `void`
  --> src/main.ori:7:14
   |
 7 |     release: f -> close(f)?,
   |              ^^^^^^^^^^^^^^ returns `Result<void, Error>`
   |
   = help: handle errors within release: `release: f -> match(close(f), ...)`
```

---

## Spec Changes Required

### Update `10-patterns.md`

Expand the `with` section with:
1. Complete execution order
2. Guarantee conditions
3. Panic/error handling
4. Nesting semantics
5. Async interaction

---

## Summary

| Aspect | Behavior |
|--------|----------|
| Release guarantee | Always runs if acquire succeeds |
| Acquire failure | Release does not run |
| Use panic | Release runs during unwinding |
| Use error (`?`) | Release runs before propagation |
| Release panic | Abort if double fault |
| Release return | Must be `void` |
| Nesting | Inside-out release order |
| Cancellation | Release runs during task unwinding |
