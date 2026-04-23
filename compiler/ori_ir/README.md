# ori_ir

> **`ori_ir` is the interned, flat, Salsa-compatible data foundation every other crate depends on.** Three design principles: **Intern Everything**, **Flatten Everything**, **Interface Segregation**.

## Role in the pipeline

`ori_ir` is the leaf of the compiler's dependency graph — it depends only on external utility crates and is depended on by every other compiler crate. It owns the shared data shapes: `Span`, `Name`, `TokenList`, AST nodes, `ExprArena`, `ExprId`, `TypeId`, `BuiltinType`, `builtin_constants`, and the canonical `DerivedTrait` enum.

Every type derives `Clone + Eq + PartialEq + Hash + Debug` for Salsa compatibility: Salsa requires `Clone` for storage, `Eq`/`Hash` for memoization keys, and `Debug` for error messages.

## Design principles (verbatim from `src/lib.rs:21-26`)

1. **Intern Everything**: strings → `Name(u32)`, types → `TypeId(u32)`.
2. **Flatten Everything**: no `Box<Expr>`, use `ExprId(u32)` indices.
3. **Interface Segregation**: focused traits (`Spanned`, `Named`).

Types containing floats store them as `u64` bits for `Hash` compatibility. Types containing strings use interned `Name` for O(1) equality.

## Architecture

- `arena/` — `ExprArena`, `ExprRange`, `define_range!` macro
- `type_id/` — `TypeId` (parser-level type index, aligned with `ori_types::Idx`)
- `name/` — `Name` interning with sharded layout
- `ast/` — AST node definitions
- `visitor/` — `Visitor<'ast>` trait and `walk_*()` functions
- `derives/` — `DerivedTrait` enum (SSOT for derivable traits)
- `builtin_type/` — `BuiltinType` (IR-level type identity)
- `format_spec.rs` — format-string parser types

## Dependencies

| Direction | Crates |
|---|---|
| Upstream | external utility deps only (leaf) |
| Downstream | every other compiler crate |

## Invariants

- **Leaf status**: if `ori_ir` grew an upstream compiler dep, every crate would rebuild on unrelated IR changes. Do not add compiler-internal deps.
- **`DerivedTrait` is the SSOT for derivable traits**: adding a variant requires updating four sync points in `ori_types/check/registration/`, `ori_eval/interpreter/derived_methods.rs`, `ori_eval/derives/mod.rs`, and `ori_llvm/codegen/derive_codegen/`. Missing any sync point is a DRIFT finding that blocks merge.
- **Interning is mandatory**: identifiers are `Name`, types are `TypeId` / `Idx`, expressions are `ExprId`. Raw strings and `Box<T>` are banned at boundaries.

## Testing

```bash
cargo test -p ori_ir
```

## Where to look

- Arena / ranges: `src/arena/`
- Type IDs: `src/type_id/`
- Name interning: `src/name/`
- AST: `src/ast/`
- Derived traits SSOT: `src/derives/mod.rs`
