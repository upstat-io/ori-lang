# Proposal: std.json Native Parser

**Status:** Draft
**Author:** Eric (with AI assistance)
**Created:** 2026-03-26
**Affects:** Standard library, spec (stdlib modules)
**Supersedes:** `approved/stdlib-json-api-proposal.md`, `approved/stdlib-json-api-ffi-revision.md`
**Prerequisites:** `drafts/compile-time-reflection-proposal.md`, `approved/intrinsics-v2-byte-simd-proposal.md`, `approved/const-generics-proposal.md`

---

## Part I: Vision & Motivation

### 1.1 Summary

This proposal defines `std.json` — a **pure Ori** JSON parser and serializer that achieves competitive performance with the fastest C/C++ JSON libraries without using FFI. The library combines three Ori language features:

1. **Compile-time reflection** (`$for` + `fields_of`) — zero-cost struct ↔ JSON mapping
2. **SIMD intrinsics** (`uses Intrinsics`) — vectorized structural scanning at 64 bytes/cycle
3. **Fixed-capacity lists** (`[byte, max 64]`) — SIMD chunk types with no heap allocation

The goal is to prove that Ori can compete at the lowest level of systems performance while remaining safe, ergonomic, and dependency-free.

### 1.2 Why Pure Ori

The previous `stdlib-json-api-ffi-revision.md` proposal recommended yyjson (a C library) via FFI. This proposal takes the opposite approach: **everything in Ori**, no C, no FFI.

**Why this matters:**

1. **Proof of language** — if Ori's JSON parser can match C, the language has earned its systems-programming credentials. FFI to C is an admission that Ori cannot do the work itself.

2. **No dependency chain** — pure Ori means no C compiler needed, no linking issues, no platform-specific build scripts, no security audit of third-party C code.

3. **Full capability integration** — a pure Ori parser uses `uses Intrinsics` (a standard capability), enabling mocking (`with Intrinsics = EmulatedIntrinsics in { ... }`), testing with scalar fallbacks, and reasoning about effects. A C FFI call is an opaque black box.

4. **Debuggability** — when the JSON parser has a bug, users debug Ori code with Ori tools. Not C code with gdb.

5. **Cross-platform by default** — SIMD intrinsics compile to native instructions on x86_64/AVX2, aarch64/NEON, and wasm/SIMD128 via the Intrinsics capability. No platform-specific C code.

### 1.3 Prior Art

#### simdjson (C++)

The state-of-the-art JSON parser. Processes 2-4 GB/s on x86_64 AVX2. Two-stage architecture:

- **Stage 1**: SIMD structural scanning — loads 64 bytes at a time, uses vector comparisons to find `"`, `{`, `}`, `[`, `]`, `:`, `,` characters, tracks string boundaries via prefix-XOR carry, produces a "structural index" of positions.
- **Stage 2**: Tape building or on-demand navigation — walks the structural index to build a DOM tape or lazily navigate to requested fields.

C++26 reflection (P2996) enables zero-cost struct mapping on top.

#### Zig std.json

Pure Zig, no C dependencies. Uses `@typeInfo` + `inline for` for compile-time struct mapping. Competitive with hand-written C parsers. Scalar (non-SIMD) but fast due to comptime specialization. Proves that a language's own JSON parser can be fast enough.

#### Rust serde_json

Uses proc macros (`#[derive(Serialize, Deserialize)]`) for struct mapping. simd-json crate provides SIMD acceleration as a separate dependency. The split between serde (framework) and simd-json (acceleration) means the fast path requires extra dependencies.

### 1.4 Performance Targets

| Metric | Target | Comparison |
|--------|--------|------------|
| Parsing throughput (SIMD path) | >1 GB/s on x86_64 AVX2 | simdjson: 2-4 GB/s |
| Parsing throughput (scalar fallback) | >200 MB/s | Zig std.json: ~300 MB/s |
| Serialization throughput | >500 MB/s | Direct buffer writes, no intermediate alloc |
| Typed deserialization overhead | Zero | Compile-time reflection, no runtime dispatch |
| Memory per parse | Proportional to input size | No DOM tree for on-demand path |

The SIMD path targets 50%+ of simdjson. This is ambitious for a v1 but achievable — simdjson's core SIMD algorithms are published and well-documented.

---

## Part II: Core Types

### 2.1 JsonValue (Untyped JSON)

For working with arbitrary JSON whose structure is not known at compile time:

```ori
pub type JsonValue =
    | Null
    | Bool(value: bool)
    | Number(value: float)
    | Integer(value: int)
    | String(value: str)
    | Array(items: [JsonValue])
    | Object(entries: {str: JsonValue})
```

**Design note**: `Integer` is separate from `Number` to preserve exact integer values. JSON numbers that are integral (no decimal point, no exponent) parse as `Integer`. This avoids precision loss for large IDs or timestamps.

```ori
impl JsonValue {
    // Type testing
    @is_null (self) -> bool
    @is_bool (self) -> bool
    @is_number (self) -> bool
    @is_integer (self) -> bool
    @is_string (self) -> bool
    @is_array (self) -> bool
    @is_object (self) -> bool

    // Extraction (return Option)
    @as_bool (self) -> Option<bool>
    @as_float (self) -> Option<float>
    @as_int (self) -> Option<int>
    @as_str (self) -> Option<str>
    @as_array (self) -> Option<[JsonValue]>
    @as_object (self) -> Option<{str: JsonValue}>

    // Navigation
    @get (self, key: str) -> Option<JsonValue>       // object field
    @at (self, index: int) -> Option<JsonValue>      // array element
}
```

### 2.2 JsonError

```ori
pub type JsonError = {
    kind: JsonErrorKind,
    message: str,
    path: str,         // JSON path to error location (e.g., ".users[2].name")
    position: int,     // byte offset in input
    line: int,
    column: int,
}

pub type JsonErrorKind =
    | SyntaxError
    | UnexpectedToken
    | UnexpectedEof
    | InvalidNumber
    | InvalidEscape
    | InvalidUtf8
    | TrailingComma
    | DuplicateKey
    | TypeMismatch
    | MissingField(name: str)
    | UnknownField(name: str)
    | DepthExceeded
    | SizeExceeded
```

### 2.3 ParseOptions

```ori
pub type ParseOptions = {
    max_depth: int = 128,            // max nesting depth
    max_size: int = 100_000_000,     // max input size in bytes (100 MB)
    allow_trailing_commas: bool = false,
    allow_comments: bool = false,
    allow_duplicate_keys: bool = true,    // last wins
    reject_unknown_fields: bool = false,  // for typed parsing
}
```

### 2.4 JsonWriter (Buffer for Serialization)

A pre-allocated, growable byte buffer for efficient JSON output. Avoids string concatenation overhead.

```ori
pub type JsonWriter = {
    ::buffer: [byte],
    ::len: int,
}

impl JsonWriter {
    @new () -> JsonWriter
    @with_capacity (capacity: int) -> JsonWriter

    // Low-level writes
    @write_byte (self, value: byte) -> void
    @write_bytes (self, value: [byte]) -> void
    @write_str (self, value: str) -> void

    // JSON-aware writes
    @write_null (self) -> void
    @write_bool (self, value: bool) -> void
    @write_int (self, value: int) -> void
    @write_float (self, value: float) -> void
    @write_escaped (self, value: str) -> void    // escape special chars + quotes
    @write_json_string (self, value: str) -> void // write_escaped wrapped in quotes

    // Structure
    @begin_object (self) -> void       // writes {
    @end_object (self) -> void         // writes }
    @begin_array (self) -> void        // writes [
    @end_array (self) -> void          // writes ]
    @write_key (self, key: str) -> void // writes "key":
    @write_comma (self) -> void        // writes ,

    // Output
    @to_str (self) -> str
    @to_bytes (self) -> [byte]
    @len (self) -> int
}
```

---

## Part III: Traits

### 3.1 ToJson (Serialization)

```ori
pub trait ToJson {
    // Structured: produce a JsonValue (convenient, allocates)
    @to_json (self) -> JsonValue

    // Fast path: write directly to buffer (zero intermediate allocation)
    @write_json (self, writer: JsonWriter) -> void
}
```

Two methods serve different use cases:
- `to_json()` — convenient for manipulation, testing, small payloads
- `write_json()` — fast path for production serialization, writes directly to buffer

#### Primitive Implementations

```ori
impl int: ToJson {
    @to_json (self) -> JsonValue = JsonValue.Integer(value: self)
    @write_json (self, writer: JsonWriter) -> void = writer.write_int(value: self)
}

impl float: ToJson {
    @to_json (self) -> JsonValue = JsonValue.Number(value: self)
    @write_json (self, writer: JsonWriter) -> void = writer.write_float(value: self)
}

impl bool: ToJson {
    @to_json (self) -> JsonValue = JsonValue.Bool(value: self)
    @write_json (self, writer: JsonWriter) -> void = writer.write_bool(value: self)
}

impl str: ToJson {
    @to_json (self) -> JsonValue = JsonValue.String(value: self)
    @write_json (self, writer: JsonWriter) -> void = writer.write_json_string(value: self)
}

impl<T: ToJson> Option<T>: ToJson {
    @to_json (self) -> JsonValue = match self {
        Some(v) -> v.to_json(),
        None -> JsonValue.Null,
    }
    @write_json (self, writer: JsonWriter) -> void = match self {
        Some(v) -> v.write_json(writer: writer),
        None -> writer.write_null(),
    }
}

impl<T: ToJson> [T]: ToJson {
    @to_json (self) -> JsonValue = {
        JsonValue.Array(items: self.map(transform: t -> t.to_json()))
    }
    @write_json (self, writer: JsonWriter) -> void = {
        writer.begin_array()
        for entry in self.enumerate() do {
            if entry.0 > 0 then writer.write_comma()
            entry.1.write_json(writer: writer)
        }
        writer.end_array()
    }
}

impl<V: ToJson> {str: V}: ToJson {
    @to_json (self) -> JsonValue = {
        let entries = for entry in self.iter() yield {
            (entry.0, entry.1.to_json())
        }
        JsonValue.Object(entries: {str: JsonValue}.from_iter(iter: entries.iter()))
    }
    @write_json (self, writer: JsonWriter) -> void = {
        writer.begin_object()
        let first = true
        for entry in self.iter() do {
            if !first then writer.write_comma()
            first = false
            writer.write_key(key: entry.0)
            entry.1.write_json(writer: writer)
        }
        writer.end_object()
    }
}
```

#### Default Impl (Compile-Time Reflection)

Any struct whose fields implement `ToJson` automatically gets serialization — no attributes, no derive:

```ori
pub def impl ToJson {
    @to_json (self) -> JsonValue = {
        let entries = $for field in fields_of(Self) yield {
            (field.name, self.[field].to_json())
        }
        JsonValue.Object(entries: {str: JsonValue}.from_iter(iter: entries.iter()))
    }

    @write_json (self, writer: JsonWriter) -> void = {
        writer.begin_object()
        $for field in fields_of(Self) do {
            $if field.index > 0 then writer.write_comma()
            writer.write_key(key: field.name)        // compile-time string literal
            self.[field].write_json(writer: writer)   // monomorphized per field type
        }
        writer.end_object()
    }
}
```

The `write_json` default impl generates code identical to hand-written serialization. Each field name is a compile-time string literal. Each `.write_json()` call is monomorphized per field type. The `$for` is fully unrolled. Zero overhead from reflection.

### 3.2 FromJson (Deserialization)

```ori
pub trait FromJson {
    @from_json (json: JsonValue) -> Result<Self, JsonError>
}
```

#### Primitive Implementations

```ori
impl int: FromJson {
    @from_json (json: JsonValue) -> Result<int, JsonError> = match json {
        JsonValue.Integer(value: n) -> Ok(n),
        JsonValue.Number(value: n) -> Ok(n as int),
        _ -> Err(JsonError {
            kind: TypeMismatch,
            message: `expected integer, got {json.type_name()}`,
            path: "", position: 0, line: 0, column: 0,
        }),
    }
}

impl str: FromJson {
    @from_json (json: JsonValue) -> Result<str, JsonError> = match json {
        JsonValue.String(value: s) -> Ok(s),
        _ -> Err(JsonError {
            kind: TypeMismatch,
            message: `expected string, got {json.type_name()}`,
            path: "", position: 0, line: 0, column: 0,
        }),
    }
}

// ... float, bool, Option<T>, [T], {str: V}
```

#### Default Impl (Compile-Time Reflection)

```ori
pub def impl FromJson {
    @from_json (json: JsonValue) -> Result<Self, JsonError> = match json {
        JsonValue.Object(entries: obj) -> {
            // Parse each field from the JSON object
            // (depends on struct construction from $for — Open Question Q1
            //  in compile-time-reflection-proposal.md)
            $for field in fields_of(Self) do {
                let field_json = obj[field.name] ?? JsonValue.Null
                let parsed = FromJson.from_json(json: field_json)?
                // ... accumulate into struct construction
            }
            todo()  // blocked on Q1: struct construction from $for
        },
        _ -> Err(JsonError {
            kind: TypeMismatch,
            message: `expected object for {name_of(Self)}`,
            path: "", position: 0, line: 0, column: 0,
        }),
    }
}
```

---

## Part IV: Parser Architecture

### 4.1 Two-Stage Architecture

Following simdjson's proven design, the parser operates in two stages:

```
Input: [byte]
    │
    ▼
┌──────────────────────────────────┐
│  Stage 1: Structural Scanning    │
│  SIMD: 64 bytes at a time        │
│  Find: " { } [ ] : ,            │
│  Track: string boundaries         │
│  Output: StructuralIndex          │
│  uses Intrinsics                  │
└──────────┬───────────────────────┘
           │
           ▼
┌──────────────────────────────────┐
│  Stage 2: Parse / Navigate       │
│  Walk structural index            │
│  Two modes:                       │
│    DOM: build JsonValue tree      │
│    On-demand: lazy field access   │
│  Validate: UTF-8, numbers         │
└──────────────────────────────────┘
```

### 4.2 Stage 1: SIMD Structural Scanning

The core innovation. Loads 64 bytes at a time and uses SIMD comparisons to find all structural characters simultaneously.

```ori
// Conceptual implementation — actual code will be more optimized

@scan_structural (input: [byte]) -> StructuralIndex uses Intrinsics = {
    let index = StructuralIndex.with_capacity(capacity: input.len() / 4)
    let in_string = false
    let offset = 0

    while offset + 64 <= input.byte_len() do {
        // Load 64 bytes into a SIMD register
        let chunk = Intrinsics.simd_load(data: input, offset: offset)

        // Find all quote characters
        let quotes = Intrinsics.simd_cmpeq(a: chunk, b: Intrinsics.simd_splat(value: b'"'))

        // Find backslashes (for escape handling)
        let backslashes = Intrinsics.simd_cmpeq(a: chunk, b: Intrinsics.simd_splat(value: b'\\'))

        // Compute string boundaries via prefix-XOR on quote mask
        // (XOR carry propagates "inside string" state)
        let quote_bits = quotes.bits()
        let escaped = compute_escaped_quotes(backslash_bits: backslashes.bits(), quote_bits: quote_bits)
        let real_quotes = quote_bits & ~escaped
        let string_mask = prefix_xor(bits: real_quotes) ^ ($if in_string then -1 else 0)

        // Find structural characters: { } [ ] : ,
        let structural = find_structural_chars(chunk: chunk)

        // Mask out structural characters that are inside strings
        let real_structural = structural.bits() & ~string_mask

        // Also include quote positions (they are structural for the parser)
        let all_positions = real_structural | real_quotes

        // Extract positions and add to index
        extract_positions(bits: all_positions, base_offset: offset, index: index)

        // Update string state for next chunk
        in_string = (string_mask >> 63) & 1 == 1
        offset = offset + 64
    }

    // Handle remaining bytes (scalar fallback for < 64 bytes)
    scan_scalar_tail(input: input, offset: offset, in_string: in_string, index: index)

    index
}

// Find { } [ ] : , using SIMD comparisons
@find_structural_chars (chunk: [byte, max 64]) -> Mask<64> uses Intrinsics = {
    let open_brace = Intrinsics.simd_cmpeq(a: chunk, b: Intrinsics.simd_splat(value: b'{'))
    let close_brace = Intrinsics.simd_cmpeq(a: chunk, b: Intrinsics.simd_splat(value: b'}'))
    let open_bracket = Intrinsics.simd_cmpeq(a: chunk, b: Intrinsics.simd_splat(value: b'['))
    let close_bracket = Intrinsics.simd_cmpeq(a: chunk, b: Intrinsics.simd_splat(value: b']'))
    let colon = Intrinsics.simd_cmpeq(a: chunk, b: Intrinsics.simd_splat(value: b':'))
    let comma = Intrinsics.simd_cmpeq(a: chunk, b: Intrinsics.simd_splat(value: b','))

    open_brace | close_brace | open_bracket | close_bracket | colon | comma
}

// Prefix XOR: propagates string boundary state through the bitmask
// bit i is 1 if there's an odd number of quotes before position i
@prefix_xor (bits: int) -> int = {
    let x = bits
    x = x ^ (x << 1)
    x = x ^ (x << 2)
    x = x ^ (x << 4)
    x = x ^ (x << 8)
    x = x ^ (x << 16)
    x = x ^ (x << 32)
    x
}
```

### 4.3 Stage 2: DOM Parsing

Walk the structural index to build a `JsonValue` tree:

```ori
@parse (input: str) -> Result<JsonValue, JsonError> = {
    parse_with_options(input: input, options: ParseOptions {})
}

@parse_with_options (input: str, options: ParseOptions) -> Result<JsonValue, JsonError> = {
    let bytes = input.as_bytes()
    if bytes.len() > options.max_size then {
        Err(JsonError { kind: SizeExceeded, message: "input exceeds max size", ... })
    } else {
        let index = scan_structural(input: bytes)
        let parser = Parser { input: bytes, index: index, pos: 0, depth: 0, options: options }
        parser.parse_value()
    }
}
```

### 4.4 Stage 2 (Alternate): On-Demand Typed Parsing

The high-performance path — parse JSON text directly into typed structs without building a `JsonValue` tree. Uses compile-time reflection to generate per-type parsers:

```ori
@parse_into<T: FromJsonDirect>(input: str) -> Result<T, JsonError> uses Intrinsics = {
    let bytes = input.as_bytes()
    let index = scan_structural(input: bytes)
    let reader = OnDemandReader { input: bytes, index: index, pos: 0 }
    T.from_json_direct(reader: reader)
}

trait FromJsonDirect {
    @from_json_direct (reader: OnDemandReader) -> Result<Self, JsonError>
}

// Default impl: compile-time reflection generates a per-type parser
pub def impl FromJsonDirect {
    @from_json_direct (reader: OnDemandReader) -> Result<Self, JsonError> = {
        reader.expect_object_start()?

        // Compile-time known field names enable optimized key matching
        $for field in fields_of(Self) do {
            // For each field, scan for its key in the JSON object
            // The compiler can generate a perfect hash or decision tree
            // because all field names are compile-time string literals
            let key = reader.read_key()?
            // ... match key to field.name, parse value
        }

        reader.expect_object_end()?
        // ... construct Self (depends on Q1 from reflection proposal)
        todo()
    }
}
```

The on-demand path avoids:
- Allocating a `JsonValue` tree (zero intermediate allocation)
- Copying field values through `JsonValue` extraction
- Runtime string matching for field names (compile-time known)

---

## Part V: Public API

### 5.1 Module Exports

```ori
// std.json public API

// Types
pub type JsonValue       // sum type for any JSON value
pub type JsonError        // detailed parse/serialize errors
pub type JsonErrorKind    // error classification
pub type ParseOptions     // parser configuration
pub type JsonWriter       // buffer for serialization

// Traits
pub trait ToJson           // serialization (to_json + write_json)
pub trait FromJson         // deserialization from JsonValue
pub trait FromJsonDirect   // on-demand deserialization from raw bytes

// Functions
pub @parse (input: str) -> Result<JsonValue, JsonError>
pub @parse_with_options (input: str, options: ParseOptions) -> Result<JsonValue, JsonError>
pub @parse_into<T: FromJsonDirect> (input: str) -> Result<T, JsonError> uses Intrinsics
pub @stringify (value: JsonValue) -> str
pub @stringify_pretty (value: JsonValue, indent: int = 2) -> str
pub @to_json_str<T: ToJson> (value: T) -> str               // convenience
pub @to_json_str_pretty<T: ToJson> (value: T, indent: int = 2) -> str
```

### 5.2 Usage Examples

#### Parse untyped JSON

```ori
use std.json { parse, JsonValue }

let json = parse(input: '{"name": "Alice", "age": 30}')?

match json {
    JsonValue.Object(entries: obj) -> {
        let name = obj["name"]?.as_str()?
        let age = obj["age"]?.as_int()?
        print(msg: `{name} is {age}`)
    },
    _ -> panic(msg: "expected object"),
}
```

#### Serialize a struct (zero boilerplate)

```ori
use std.json { to_json_str }

type User = { name: str, age: int, active: bool }

let user = User { name: "Alice", age: 30, active: true }
let json = to_json_str(value: user)
// {"name":"Alice","age":30,"active":true}
```

#### Deserialize into a struct

```ori
use std.json { parse_into }

type Config = { host: str, port: int, debug: bool }

let config = parse_into<Config>(input: config_text)?
print(msg: `Connecting to {config.host}:{config.port}`)
```

#### Pretty printing

```ori
use std.json { to_json_str_pretty }

let json = to_json_str_pretty(value: user, indent: 4)
// {
//     "name": "Alice",
//     "age": 30,
//     "active": true
// }
```

---

## Part VI: Implementation Strategy

### Phase 1: Foundation Types

**Scope**: `JsonValue`, `JsonError`, `JsonWriter`, basic scalar parse/stringify.

**No SIMD, no reflection** — pure scalar implementation. This establishes the API surface and type definitions.

**Deliverables:**
- `JsonValue` sum type with all methods
- `JsonError` with error kinds
- `JsonWriter` buffer type
- Scalar JSON parser (recursive descent, no SIMD)
- Scalar JSON serializer via `JsonWriter`
- `parse()`, `stringify()`, `stringify_pretty()`
- Spec tests for all JSON types, edge cases, error handling

**Dependencies:** None beyond current compiler.

### Phase 2: ToJson / FromJson Traits

**Scope**: Trait definitions, primitive impls, collection impls.

**Deliverables:**
- `ToJson` trait with `to_json()` + `write_json()`
- `FromJson` trait with `from_json()`
- Impls for: int, float, bool, str, Option, List, Map
- `to_json_str()` convenience function
- Spec tests for all primitive serialization/deserialization

**Dependencies:** None beyond current compiler.

### Phase 3: Compile-Time Reflection Integration

**Scope**: Default impls using `$for` + `fields_of` for automatic struct serialization/deserialization.

**Deliverables:**
- `pub def impl ToJson` with compile-time reflection
- `pub def impl FromJson` with compile-time reflection (depends on struct construction Q1)
- Zero-boilerplate struct serialization
- Performance tests comparing reflection path vs hand-written

**Dependencies:** Compile-time reflection proposal (Phases 2-4).

### Phase 4: SIMD Structural Scanner

**Scope**: Stage 1 SIMD scanning, replacing scalar structural character search.

**Deliverables:**
- `scan_structural()` with SIMD path
- `prefix_xor()` for string boundary tracking
- Scalar fallback for tail bytes and non-SIMD platforms
- `StructuralIndex` type
- Performance benchmarks: throughput in GB/s

**Dependencies:** SIMD intrinsics implementation (§06/§21A), const generics (§18.1), fixed-capacity lists (§18.2).

### Phase 5: On-Demand Parser

**Scope**: `FromJsonDirect` trait and `parse_into<T>()` for zero-allocation typed parsing.

**Deliverables:**
- `OnDemandReader` type
- `FromJsonDirect` trait with default impl using compile-time reflection
- `parse_into<T>()` function
- Performance benchmarks vs DOM path

**Dependencies:** Phase 3 (reflection) + Phase 4 (SIMD scanner) + struct construction Q1.

### Phase 6: Optimization

**Scope**: Performance tuning to approach simdjson throughput.

**Deliverables:**
- Branch-free number parsing
- SIMD string validation (UTF-8)
- SIMD string escape handling
- Compile-time perfect hash for field name matching
- Benchmark suite with comparison targets

**Dependencies:** All prior phases.

---

## Part VII: Dependency Map

```
Currently Available
├── Monomorphization (complete)
├── Const functions (parsed, typed)
└── Generic traits + default impls

Phase 1-2: No New Dependencies
├── JsonValue, JsonError, JsonWriter
├── Scalar parser (recursive descent)
├── ToJson / FromJson traits
└── Primitive + collection impls

Phase 3: Compile-Time Reflection
├── $for, $if, fields_of(T), value.[field]
├── (from compile-time-reflection-proposal)
└── Struct construction from $for (Q1)

Phase 4: SIMD
├── Const generics basics (§18.1) — in progress
├── Fixed-capacity lists (§18.2) — not started
├── SIMD intrinsics impl (§06/§21A) — spec'd, not impl'd
│   ├── simd_cmpeq, simd_splat, simd_load
│   ├── Mask<$N> type with .bits()
│   └── count_trailing_zeros, prefix_xor
└── Deep safety (§06 capability propagation) — draft

Phase 5-6: Integration
├── All of the above
└── Performance benchmarks
```

**Key insight**: Phases 1-2 have **zero dependencies** on new language features. They can ship immediately with the current compiler. SIMD and reflection are additive performance improvements layered on top of a working library.

---

## Part VIII: Comparison with Superseded Proposals

### `stdlib-json-api-proposal.md` (Approved 2026-01-30)

| Aspect | Old Proposal | This Proposal |
|--------|-------------|---------------|
| Parser backend | Unspecified | Pure Ori, two-stage SIMD |
| Struct mapping | Runtime `Json` trait | Compile-time reflection (`$for` + `fields_of`) |
| Serialization | `to_json()` returns `JsonValue` | `write_json()` fast path + `to_json()` convenience |
| Deserialization | `from_json(JsonValue)` only | + `parse_into<T>()` on-demand path |
| Dependencies | None specified | None (pure Ori) |

**Kept from old proposal**: `JsonValue` sum type (same structure), `JsonError` (expanded), error path design.

**Changed**: Everything about implementation strategy, performance model, and trait design.

### `stdlib-json-api-ffi-revision.md` (Approved 2026-01-30)

| Aspect | FFI Revision | This Proposal |
|--------|-------------|---------------|
| Parser | yyjson (C, via FFI) | Pure Ori SIMD |
| WASM | JavaScript JSON API | Pure Ori (wasm SIMD128) |
| Fallback | Pure Ori (slow) | Scalar Ori (competitive) |
| Dependencies | C compiler, linking, platform builds | None |
| Debuggability | C code with gdb | Ori code with Ori tools |
| Capability model | Opaque FFI call | `uses Intrinsics` (mockable, testable) |

**Why superseded**: Pure Ori with SIMD intrinsics achieves the performance goals without the FFI complexity. The Intrinsics capability provides the same vector instructions that yyjson uses, but within Ori's type system and capability model.

---

## Part IX: Open Questions

### Q1: Relationship to `std.bytes`

The SIMD intrinsics V2 proposal defines `std.bytes` with functions like `find_byte`, `find_any`, `count_byte`. Should `std.json` use `std.bytes` or call intrinsics directly?

**Current thinking**: `std.json` calls intrinsics directly for maximum control. `std.bytes` is a convenience layer for general byte processing. The JSON parser's SIMD patterns (prefix-XOR, multi-character comparison) are specialized beyond what `std.bytes` provides.

### Q2: Streaming Parser

Should `std.json` support streaming parsing (processing chunks as they arrive from network)?

**Current thinking**: Not in v1. The two-stage architecture requires the full input. Streaming would need a different Stage 1 design. Add in v2 if demand materializes.

### Q3: JSON5 / JSONC Support

Should the parser support JSON5 (comments, trailing commas, unquoted keys)?

**Current thinking**: `ParseOptions` has `allow_trailing_commas` and `allow_comments` flags. These are opt-in extensions, not default behavior. Full JSON5 support is a future extension.

### Q4: Custom Serialization (Field Renaming, Skipping)

How does a user customize serialization (rename fields, skip fields, custom formats)?

**Current thinking**: Depends on the field annotations design in the compile-time reflection proposal (§2.9). For v1, users who need custom serialization implement `ToJson` manually. The default impl handles the common case.

### Q5: Number Precision

JSON numbers are IEEE 754 doubles. Ori's `int` is i64. How to handle integers > 2^53?

**Current thinking**: `JsonValue.Integer` preserves exact `int` values. Serialization emits integers without decimal point. Deserialization of numbers > 2^53 with decimal point may lose precision (documented behavior, matching JavaScript semantics).

---

## Part X: Summary

| Component | Description | Phase |
|-----------|-------------|-------|
| `JsonValue` | Untyped JSON sum type | 1 |
| `JsonError` | Detailed error reporting | 1 |
| `JsonWriter` | Pre-allocated output buffer | 1 |
| `parse()` | Scalar JSON parser | 1 |
| `stringify()` | JSON text output | 1 |
| `ToJson` | Serialization trait | 2 |
| `FromJson` | Deserialization trait | 2 |
| Default impls | `$for` + `fields_of` auto-impl | 3 |
| SIMD scanner | `uses Intrinsics`, 64 bytes/cycle | 4 |
| `parse_into<T>()` | On-demand typed parsing | 5 |
| `FromJsonDirect` | Zero-allocation deserialization | 5 |

**Key properties:**
- Pure Ori — no C, no FFI, no external dependencies
- SIMD-accelerated via `uses Intrinsics` capability (mockable, testable, cross-platform)
- Zero-cost struct mapping via compile-time reflection
- Two serialization paths: convenience (`to_json()`) and performance (`write_json()`)
- Two parsing paths: DOM (`parse()`) and on-demand (`parse_into<T>()`)
- Phased delivery: Phases 1-2 ship immediately, SIMD and reflection layer on top

---

## Related Proposals

- **Supersedes**: `approved/stdlib-json-api-proposal.md` (2026-01-30)
- **Supersedes**: `approved/stdlib-json-api-ffi-revision.md` (2026-01-30)
- **Depends on**: `drafts/compile-time-reflection-proposal.md` (for Phase 3+)
- **Depends on**: `approved/intrinsics-v2-byte-simd-proposal.md` (for Phase 4+)
- **Depends on**: `approved/const-generics-proposal.md` (for Phase 4+)
- **Interacts with**: `approved/const-evaluation-termination-proposal.md` (shared prerequisite)
