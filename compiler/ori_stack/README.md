# ori_stack

> **`ori_stack` exists to let recursive compiler code handle arbitrarily deep input without host stack overflow.**

## Role in the pipeline

Infrastructure utility used by any compiler phase that recurses into potentially deeply-nested source structures: `ori_parse` (recursive descent), `ori_types` (type substitution, inference traversal), `ori_eval` (recursive interpretation).

Wrap recursive calls with `ensure_sufficient_stack`:

```rust
fn parse_expr(&mut self) -> Result<ExprId, ParseError> {
    ensure_sufficient_stack(|| {
        // recursive parsing logic
    })
}
```

## Platform support

- **Native**: uses the `stacker` crate to grow the stack on demand.
- **WASM**: no-op passthrough (WASM has its own stack management).

## Configuration

- **Red zone**: 100 KB — if less than this remains, the stack grows.
- **Growth size**: 1 MB — each growth allocates this much additional space.

These values are chosen to handle deeply nested code (100k+ recursion depth) while keeping memory usage reasonable.

## Dependencies

| Direction | Crates |
|---|---|
| Upstream | `stacker` (on native targets only) |
| Downstream | `ori_parse`, `ori_types`, `ori_eval`, `ori_llvm`, `ori_compiler`, `oric` |

## Invariants

- **Stack overflow in the compiler is always a bug.** Either recursion depth exceeded its documented limit or `ensure_sufficient_stack` is missing at a recursion point.
- **WASM no-op is safe**: WASM's own stack manager handles growth; the abstraction leaks nothing.
- **Never rewrite a recursion as iteration just to avoid `ori_stack`**: iterative rewrites are valid for performance but `ori_stack` is still the floor for any remaining recursion.

## Testing

```bash
cargo test -p ori_stack
```
