---
title: "Drop Descriptors"
description: "Ori Compiler Design — Per-Type Drop Generation"
order: 908
section: "ARC System"
---

# Drop Descriptors

## Overview

When a reference count reaches zero, the value must be cleaned up: its reference-counted
children must be decremented, and its memory must be freed. Drop descriptors declaratively
specify what cleanup is needed for each type. Rather than embedding drop logic inline at
every RcDec site, the compiler generates a single drop function per type and passes its
address to the runtime's `ori_rc_dec` function.

Drop descriptors are computed in `ori_arc` and consumed by the LLVM backend's
`drop_gen.rs` to generate specialized drop functions.

## DropKind Variants

Each type that requires cleanup gets a `DropInfo` containing a `DropKind` that describes
the structure of its drop:

| DropKind | Applies To | Cleanup Strategy |
|---|---|---|
| `Trivial` | `str`, `[int]`, bare function pointers | Free the allocation. No children to decrement. |
| `Fields(Vec<(u32, Idx)>)` | Structs, tuples | RcDec each listed field (by index and type), then free. |
| `Enum(Vec<Vec<(u32, Idx)>>)` | Sum types | Switch on the tag discriminant. Per-variant: RcDec variant-specific fields, then free. |
| `Collection { element_type }` | `[T]`, `Set<T>` | Iterate over elements, RcDec each one, then free the buffer and the struct. |
| `Map { key_type, value_type, dec_keys, dec_values }` | `{K: V}` | Iterate over entries. Conditionally RcDec keys (if `dec_keys`) and values (if `dec_values`), then free the buffer and the struct. |
| `ClosureEnv(Vec<(u32, Idx)>)` | Closure environments | RcDec each captured RC field, then free the environment allocation. |

The `(u32, Idx)` pairs in `Fields`, `Enum`, and `ClosureEnv` encode the field index
(for GEP offset) and the type pool index (to look up the child's drop function).

### Trivial Drops

A trivial drop means the type has no reference-counted children. The allocation can be
freed directly without visiting any fields. Strings and lists of primitives fall into
this category — their contents are inline data (bytes, integers) with no pointers to
manage.

### Enum Drops

Sum types require a tag-based dispatch. The outer vector is indexed by variant tag.
Each inner vector lists the RC fields for that variant. Variants with no RC fields
(e.g., a variant containing only `int` fields) have an empty inner vector, and the
drop function skips straight to freeing.

### Collection and Map Drops

Collections and maps have a two-level structure: the container struct (with metadata
like length and capacity) and a heap-allocated buffer holding the elements or entries.
The drop function iterates the buffer, decrements each RC element, then frees both the
buffer and the container struct.

For maps, the `dec_keys` and `dec_values` booleans are determined by whether the key
and value types are reference-counted. A `{str: int}` map has `dec_keys: true` and
`dec_values: false`, avoiding a no-op decrement on each integer value.

### Closure Environment Drops

Closure environments are heap-allocated structs containing captured variables. Only
captures whose types are reference-counted appear in the `ClosureEnv` field list.
Primitive captures (integers, booleans) are skipped.

## API

### compute_drop_info

```
compute_drop_info(ty: Idx, classifier: &TypeClassifier, pool: &TypePool) -> Option<DropInfo>
```

Computes the drop descriptor for a single type. Returns `None` if the type does not
need RC (stack-only, no heap allocation). The classifier determines whether a type
needs RC and which of its fields are reference-counted.

### compute_closure_env_drop

```
compute_closure_env_drop(capture_types: &[Idx], classifier: &TypeClassifier) -> DropKind
```

Builds a `ClosureEnv` drop kind from the list of capture types in a `PartialApply`
instruction. Filters to only RC-needing captures and records their indices.

### collect_drop_infos

```
collect_drop_infos(functions: &[Function], classifier: &TypeClassifier, pool: &TypePool) -> Vec<DropInfo>
```

Scans all functions in a compilation unit, collects every type that appears in an RcDec
or Construct instruction, computes drop info for each, and returns a deduplicated list.
This is the main entry point used by the LLVM backend to gather all needed drop
functions before code generation.

## LLVM Consumption

The LLVM backend's `DropFunctionGenerator` reads each `DropInfo` and generates a
corresponding drop function.

### Naming

Drop functions are named `_ori_drop$<idx_raw>` where `idx_raw` is the raw value of the
type pool index. This naming is deterministic and unique per type, allowing the linker
to deduplicate across compilation units.

### Caching

Drop functions are cached by their mangled name. If a drop function has already been
generated for a given type, the cached function pointer is returned. This is essential
for recursive types.

### Recursive Types

For recursive types (e.g., a linked list where a node contains an `Option<Node>`), the
function ID is inserted into the cache BEFORE the function body is generated. This
breaks the recursion: when generating the body of `_ori_drop$Node`, the RcDec for the
`next` field looks up `_ori_drop$Node` and finds the already-registered (but not yet
complete) function. LLVM handles forward references to functions natively.

### Drop Function Structure

The generated function has the signature `extern "C" fn(*mut u8)` and follows this
structure:

1. **Cast**: Bitcast the opaque `*mut u8` data pointer to a pointer to the concrete
   struct type.

2. **Field cleanup**: For each reference-counted field listed in the `DropKind`:
   - GEP to the field offset.
   - Load the field pointer.
   - Call `ori_rc_dec(field_ptr, child_drop_fn)` where `child_drop_fn` is the drop
     function for the field's type (looked up or generated recursively).

3. **Free**: Call `ori_rc_free(data_ptr, size, align)` to release the allocation.

For enum types, step 2 is wrapped in a switch on the tag value, with each case
handling the fields specific to that variant.

For collection types, step 2 is replaced by a loop over the buffer, calling
`ori_rc_dec` on each element, followed by freeing the buffer allocation before
freeing the container struct.
