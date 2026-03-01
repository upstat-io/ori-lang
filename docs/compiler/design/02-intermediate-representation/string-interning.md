---
title: "String Interning"
description: "Ori Compiler Design — String Interning"
order: 203
section: "Intermediate Representation"
---

# String Interning

## What Is String Interning?

**String interning** is the practice of storing each unique string value exactly once in a canonical table, then representing occurrences of that string throughout the program as small integer indices into the table. When two variables in source code happen to have the same name, they share the same integer ID rather than each carrying a separate copy of the string.

```text
Source:   let foo = 1; let bar = foo + foo;

Without interning:  "foo" "bar" "foo" "foo"     (4 string allocations, 12 bytes of characters)
With interning:     Name(0) Name(1) Name(0) Name(0)  (4 integers, string table: {"foo", "bar"})
```

The transformation seems simple, but its consequences cascade through the entire compiler. String comparison becomes integer comparison (O(1) instead of O(n) in string length). Hash table lookups hash 4 bytes instead of arbitrary-length strings. Memory usage drops because each unique identifier is stored once, regardless of how many times it appears.

Interning is nearly universal in production compilers and language runtimes. The technique dates to LISP's symbol tables in the 1960s — every LISP symbol was interned, which is why LISP programmers call them "symbols" rather than "strings." The same idea appears in Java's string pool, Lua's interned strings, Python's small-integer and string caching, Ruby's `Symbol` type, and nearly every compiler's identifier table.

## Why Compilers Intern Strings

### Identifier Comparison Is the Hot Path

A type checker compares identifier names constantly. Looking up a variable in scope? Compare its name against every name in every enclosing scope. Resolving a method call? Compare the method name against every method in the type's implementation. Checking trait implementations? Compare trait method names against implementation method names.

In a typical type-checking pass, identifier comparison accounts for a significant fraction of total CPU time. String comparison is O(n) in the length of the string — and while most identifiers are short, the comparison must also compute a hash for hash table lookups, which touches every byte. Interning reduces both operations to integer comparison and integer hashing — effectively O(1) regardless of identifier length.

### Memory Deduplication

Source programs use the same identifiers repeatedly. A variable named `result` might appear dozens of times across a file. A type name like `Option` might appear hundreds of times across a project. Without interning, each occurrence stores its own copy of the string. With interning, all occurrences share a single stored copy and carry only a 4-byte index.

The savings are proportional to repetition. In a typical Ori source file, the ratio of identifier *occurrences* to *unique identifiers* ranges from 3:1 to 10:1. For a file with 5,000 identifier occurrences but only 500 unique identifiers, interning saves roughly `4500 × 24 bytes` (assuming `String` overhead on 64-bit) — about 100 KB per file.

### Salsa Compatibility

Salsa requires query inputs and outputs to implement `Clone + Eq + Hash`. Interned `Name(u32)` satisfies all three trivially: cloning copies 4 bytes, equality compares 4 bytes, hashing hashes 4 bytes. A `String` satisfies all three too, but at much higher cost: cloning allocates a new heap buffer, equality compares character by character, and hashing reads the entire string.

Because `Name` is `Copy`, it can be freely embedded in Salsa-tracked structs, passed through query boundaries, and stored in cached results without any performance concern.

## Classical Approaches

### Global String Table

The simplest interning design is a single hash table mapping strings to integers:

```rust
struct SimpleInterner {
    map: HashMap<String, u32>,
    strings: Vec<String>,
}
```

This works well for single-threaded compilation. The map provides O(1) amortized interning, and the vector provides O(1) reverse lookup. The limitation is concurrency: accessing the table from multiple threads requires a lock around the entire table, serializing all interning operations.

### Sharded Tables

To reduce lock contention, the table can be split into multiple **shards**, each with its own lock. The string's hash determines which shard it maps to. Two threads interning strings that hash to different shards never contend — they acquire different locks and proceed in parallel.

The number of shards is a tuning parameter. Too few shards and contention remains high. Too many shards and the overhead of shard selection (hashing, modular arithmetic) dominates. In practice, 16-64 shards are sufficient for most workloads. Go's `sync.Map` and Java's `ConcurrentHashMap` use similar sharding strategies.

### Arena-Backed Interning

Some interners store strings in an arena rather than individual heap allocations. This improves locality (interned strings are contiguous in memory) and simplifies deallocation (the arena frees everything at once). Rust's `string-interner` crate uses this approach, storing strings in a contiguous byte buffer and returning indices into it.

The tradeoff is that arena-backed strings can't be individually freed. For compilers, this is usually fine — the interner lives for the entire compilation session.

## Ori's Interning Design

Ori uses a 16-shard concurrent interner with leaked string storage, designed for the specific constraints of a Salsa-based compiler.

### Name — The Interned Handle

```rust
#[derive(Clone, Copy, Eq, PartialEq, Hash, Debug)]
pub struct Name(u32);

impl Name {
    pub const EMPTY: Name = Name(0);       // Pre-interned ""
    pub const NUM_SHARDS: usize = 16;
    pub const MAX_LOCAL: u32 = 0x0FFF_FFFF; // 28-bit local index

    pub const fn new(shard: u32, local: u32) -> Self {
        Name((shard << 28) | local)
    }

    pub const fn shard(self) -> usize {
        (self.0 >> 28) as usize
    }

    pub const fn local(self) -> usize {
        (self.0 & Self::MAX_LOCAL) as usize
    }
}
```

The 32-bit `Name` packs two pieces of information: the high 4 bits identify which of the 16 shards owns the string, and the low 28 bits give the index within that shard. This encoding means that reverse lookup (`Name` → `&str`) requires no hashing — extract the shard from the high bits, index into that shard's string array with the low bits. The 28-bit local index supports up to 268 million unique strings per shard, far more than any practical compilation needs.

### StringInterner — 16-Shard Concurrent Design

```rust
pub struct StringInterner {
    shards: [RwLock<InternShard>; 16],
    total_count: AtomicUsize,
}

struct InternShard {
    map: FxHashMap<&'static str, u32>,  // string → local index
    strings: Vec<&'static str>,         // local index → string
}
```

Each shard is independently locked with an `RwLock`. The interning path uses a double-check pattern: first acquire a read lock and check if the string already exists (the common case for repeated identifiers), then only if it's new, upgrade to a write lock and insert. This means that looking up an already-interned string — by far the most common operation after the first few files — never blocks other threads.

**String storage uses `Box::leak()`** to convert owned strings into `&'static str`. This is deliberate: leaked strings have a `'static` lifetime, so they can be stored as both hash map keys and vector elements without lifetime concerns. The memory is never freed — the interner only grows. This is acceptable because the interner lives for the entire compilation session, and the total memory used by unique identifier strings is typically small (tens of KB even for large projects).

### Shard Selection

The string's hash determines its shard: `hash(string) >> 28` extracts 4 bits, giving a shard index from 0-15. Because the hash distributes uniformly, strings are approximately evenly distributed across shards. This means 16 threads interning simultaneously will, on average, each target a different shard — no contention.

The shard index is baked into the `Name` value, so reverse lookup knows exactly which shard to query without recomputing the hash. This is O(1): extract the shard from bits 31-28, index into the shard's `strings` vector with bits 27-0.

### Pre-Interned Keywords

The `StringInterner::new()` constructor pre-interns approximately 60 keywords and common identifiers:

```text
Keywords:    if, else, let, fn, match, impl, trait, use, pub, self, type, ...
Types:       int, float, bool, str, char, byte, Never, Option, Result, ...
Identifiers: main, print, len, compare, panic, assert, assert_eq, ...
```

Pre-interning serves two purposes. First, it guarantees that keywords have predictable `Name` values, enabling fast keyword identification during lexing without string comparison — the lexer can check `name == self.known.kw_if` (an integer comparison) rather than comparing the string `"if"`. Second, it populates the interner's read-path caches, so the first file to use `let` or `int` finds them already interned rather than taking the write-lock path.

### SharedInterner — Thread-Safe Sharing

```rust
pub struct SharedInterner(Arc<StringInterner>);

impl Deref for SharedInterner {
    type Target = StringInterner;
    fn deref(&self) -> &Self::Target { &self.0 }
}
```

`SharedInterner` wraps the interner in `Arc` for cross-thread sharing. The test runner shares a single `SharedInterner` across all parallel test threads, avoiding per-file re-interning of common identifiers. The `Deref` implementation makes all `StringInterner` methods available transparently.

### The StringLookup Trait

The `StringLookup` trait provides a minimal interface for `Name` resolution, enabling crates that need to display names without depending on the full interner implementation:

```rust
pub trait StringLookup {
    fn lookup(&self, name: Name) -> &str;
}

impl StringLookup for StringInterner {
    fn lookup(&self, name: Name) -> &str {
        StringInterner::lookup(self, name)
    }
}
```

This trait is used by `ori_patterns`'s `Value` type to display struct type names without depending on `ori_ir`'s interner. The pattern avoids circular dependencies: `ori_patterns` depends only on the trait (defined in `ori_ir`), not on the concrete interner.

### Fallible Interning

The `try_intern()` method returns `Result<Name, InternError>` instead of panicking when a shard overflows its 28-bit capacity. The infallible `intern()` method unwraps internally — appropriate for normal compilation where overflow is not expected. The `intern_owned()` variant accepts a `String` directly, avoiding a re-allocation when the caller already has an owned string (common in literal processing).

## Usage Across the Compiler

Interned names appear at every level of the compiler:

```rust
// AST nodes store Names, not Strings
struct Function {
    name: Name,
    params: Vec<Param>,
    body: ExprId,
}

// Type checker uses Names for type resolution
// Evaluator uses Names for variable lookup
// LLVM backend uses Names for symbol generation

// Environment lookups are HashMap<Name, Value>
// — hashing 4 bytes, not variable-length strings
```

The `Name` type threads through the entire pipeline: lexer → parser → type checker → canonicalizer → evaluator / LLVM backend. At no point does the compiler store or compare raw identifier strings — every occurrence is an interned `Name(u32)`.

## Design Tradeoffs

### Memory Is Never Reclaimed

Interned strings are leaked via `Box::leak()` and never freed. The interner only grows during a compilation session. This is acceptable because the total memory for unique identifiers is small (the number of unique identifiers in a project is far less than the total source size), and the interner's lifetime matches the compilation session. If Ori were to support long-running IDE sessions where files are repeatedly edited, a generational interning scheme (where old generations can be collected) might be worth investigating — but for batch compilation, the current design is simpler and sufficient.

### Displaying Names Requires Interner Access

`Name(42)` is meaningless without the interner to resolve it back to a string. Every function that formats a name for error messages, debug output, or LLVM symbol names must have access to the interner. The `StringLookup` trait and `db.interner()` pattern make this ergonomic across crate boundaries, but the threading is inherent to any interning design — the indirection that makes comparison fast makes display indirect.

### Hash Collisions Across Shards

Two different strings can hash to the same shard but produce different `Name` values (because they have different local indices within the shard). This is correct — the shard is part of the `Name` encoding, so names from different shards are naturally distinct. However, it means that the same string interned at different times will always produce the same `Name`, because the shard selection is deterministic (based on the string's hash, not on timing). This determinism is required for Salsa compatibility.

## Prior Art

### rustc — Symbol and Interner

[Rust's compiler](https://github.com/rust-lang/rust) (`rustc`) uses a similar interning design. `Symbol` is a 32-bit index into a global interner, pre-populated with keywords and common identifiers. rustc's interner uses a single-threaded design with thread-local access (compilation in rustc is single-threaded per-session), while Ori uses a sharded design to support parallel test execution. The `Symbol` type and its usage patterns (stored in AST nodes, used for fast comparison, resolved for display) are essentially the same as Ori's `Name`.

### Go — Unique String Deduplication

[Go's compiler](https://github.com/golang/go) interns strings during parsing to avoid duplicate allocations. Go's approach is simpler than Ori's — strings are deduplicated but not replaced with integer indices, so comparison is still string comparison. The benefit is purely memory savings, not comparison speed. This is a reasonable choice for Go, where the compiler is fast enough that O(n) string comparison isn't a bottleneck.

### Java — String.intern() and Constant Pool

Java's [`String.intern()`](https://docs.oracle.com/en/java/javase/21/docs/api/java.base/java/lang/String.html#intern()) method adds a string to the JVM's internal string pool. The JVM also uses constant pools in `.class` files, where string constants are stored once and referenced by index — structurally identical to compiler interning. Java's design demonstrates that interning is valuable beyond compilers: any system with repeated string comparisons benefits from deduplication.

### Lua — Automatic Interning

[Lua](https://github.com/lua/lua) interns all strings shorter than 40 bytes automatically. Every short string in a Lua program is stored exactly once, and string equality is pointer comparison. Lua's cutoff at 40 bytes reflects a tradeoff: short strings (identifiers, keys) are compared frequently and benefit from interning, while long strings (file contents, generated output) are compared rarely and don't justify the interning overhead.

### V8 — Internalized Strings

Google's [V8 JavaScript engine](https://v8.dev/) "internalizes" strings used as property names. Internalized strings have a pre-computed hash and enable identity comparison for property lookups — the most frequent string operation in JavaScript. V8's design is selective (only property names, not all strings), while Ori's is universal (all identifiers), reflecting the different workloads: JavaScript property access is dynamic and frequent, while Ori's identifier access is static and resolved at compile time.

## Related Documents

- [Arena Allocation](arena-allocation.md) — The complementary arena strategy for expression nodes
- [Type Representation](type-representation.md) — How the type pool extends interning to type structures
- [Flat AST](flat-ast.md) — How interned `Name` values are stored in AST nodes
- [IR Overview](index.md) — How string interning fits into the overall IR architecture
