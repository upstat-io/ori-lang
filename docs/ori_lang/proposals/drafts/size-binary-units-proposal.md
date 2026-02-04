# Proposal: Binary Size Units (IEC Standard)

**Status:** Draft
**Author:** Eric (with AI assistance)
**Created:** 2026-02-03
**Depends On:** decimal-duration-size-literals-proposal.md
**Amends:** decimal-duration-size-literals-proposal.md, duration-size-types-proposal.md

---

## Summary

Add binary (1024-based) size units alongside the existing SI (1000-based) units, following the IEC standard naming convention (`kib`, `mib`, `gib`, `tib`).

---

## Motivation

The current SI-only approach creates a **domain mismatch** between Size literals and the systems programmers interact with:

1. **File systems report in binary** — When you read a file's size, you get bytes in powers of 1024
2. **Memory is binary** — RAM, page sizes (4096 bytes), buffer allocations are all powers of 2
3. **Programmer expectations** — Most developers think of "4kb" as 4096 bytes, not 4000
4. **Subtle bugs** — Comparing `file.size() < 1mb` checks against 1,000,000 bytes when the programmer likely means 1,048,576

### The Problem in Practice

```ori
// Programmer wants a 4KB page-aligned buffer
let page_size = 4kb   // Gets 4000 bytes, NOT 4096!
                      // This will cause alignment bugs

// Checking if file fits in "1MB" limit
if file.size() < 1mb then  // Checks against 1,000,000
    process(file)          // But OS reports 1,048,576 byte files as "1MB"
```

### When SI Units ARE Correct

SI units are appropriate for:
- **Storage marketing** — "500GB hard drive" means 500,000,000,000 bytes
- **Network bandwidth** — "100 Mbps" uses decimal
- **Human-readable display** — Storage vendors use SI

Both use cases are valid. Programmers need access to both.

---

## Proposed Solution

Add IEC binary units alongside SI units:

### SI Units (Existing, Unchanged)

| Suffix | Name | Bytes |
|--------|------|-------|
| `b` | bytes | 1 |
| `kb` | kilobytes | 1,000 |
| `mb` | megabytes | 1,000,000 |
| `gb` | gigabytes | 1,000,000,000 |
| `tb` | terabytes | 1,000,000,000,000 |

### Binary Units (New)

| Suffix | Name | Bytes |
|--------|------|-------|
| `kib` | kibibytes | 1,024 |
| `mib` | mebibytes | 1,048,576 |
| `gib` | gibibytes | 1,073,741,824 |
| `tib` | tebibytes | 1,099,511,627,776 |

### Examples

```ori
// Binary - for memory, buffers, file operations
let page_size = 4kib        // 4,096 bytes (correct page size)
let buffer = 64kib          // 65,536 bytes
let chunk_size = 1mib       // 1,048,576 bytes
let memory_limit = 16gib    // 17,179,869,184 bytes

// SI - for storage capacity, network, display
let disk_quota = 500gb      // 500,000,000,000 bytes
let upload_limit = 10mb     // 10,000,000 bytes
let bandwidth = 100mb       // for display purposes
```

### Comparison

```ori
1kb == 1000b      // true (SI)
1kib == 1024b     // true (binary)
1kb != 1kib       // true (different values)

1mib == 1024kib   // true
1mb == 1000kb     // true
```

---

## Decimal Syntax Support

Binary units support the same decimal syntax as SI units, with the same rule: the result must be a whole number of bytes.

### Valid Binary Decimals

```ori
0.5kib      // 512 bytes ✓
0.25mib     // 256 kibibytes = 262,144 bytes ✓
0.5mib      // 512 kibibytes = 524,288 bytes ✓
1.5kib      // 1,536 bytes ✓
0.125gib    // 128 mibibytes = 134,217,728 bytes ✓
```

### Invalid Binary Decimals

```ori
0.1kib      // Error: 102.4 bytes not whole
0.3mib      // Error: 314,572.8 bytes not whole
1.7kib      // Error: 1,740.8 bytes not whole
```

The restricted valid decimals for binary units (powers of 2 fractions) naturally guide programmers toward binary-aligned thinking.

### Valid SI Decimals (Unchanged)

```ori
0.5kb       // 500 bytes ✓
1.5mb       // 1,500,000 bytes ✓
0.1gb       // 100,000,000 bytes ✓
```

---

## Conversion Methods

### New Binary Methods

```ori
impl Size {
    // Existing SI methods (unchanged)
    @bytes (self) -> int
    @kilobytes (self) -> int      // truncates: 1536b.kilobytes() = 1
    @megabytes (self) -> int
    @gigabytes (self) -> int
    @terabytes (self) -> int

    // New binary methods
    @kibibytes (self) -> int      // truncates: 1536b.kibibytes() = 1
    @mebibytes (self) -> int
    @gibibytes (self) -> int
    @tebibytes (self) -> int

    // Existing SI factories (unchanged)
    @from_bytes (b: int) -> Size
    @from_kilobytes (kb: int) -> Size
    @from_megabytes (mb: int) -> Size
    @from_gigabytes (gb: int) -> Size
    @from_terabytes (tb: int) -> Size

    // New binary factories
    @from_kibibytes (kib: int) -> Size
    @from_mebibytes (mib: int) -> Size
    @from_gibibytes (gib: int) -> Size
    @from_tebibytes (tib: int) -> Size
}
```

### Usage

```ori
let file_size = 1536kib
file_size.bytes()       // 1,572,864
file_size.kibibytes()   // 1536
file_size.mebibytes()   // 1 (truncated)
file_size.kilobytes()   // 1572 (truncated, SI interpretation)

let s = Size.from_mebibytes(mib: 4)  // 4,194,304 bytes
```

---

## Use Case Guidelines

### Use Binary (`kib`, `mib`, `gib`, `tib`) For:

- **Buffer sizes** — `let buf = [byte, max 64kib]`
- **Memory limits** — `let heap_max = 2gib`
- **File operations** — `if file.size() > 100mib then compress(file)`
- **Page alignment** — `let page = 4kib`
- **Cache sizes** — `let l1_cache = 32kib`
- **Chunk sizes** — `let read_chunk = 1mib`

### Use SI (`kb`, `mb`, `gb`, `tb`) For:

- **Storage quotas** — `let disk_limit = 500gb` (matches what storage shows)
- **Network transfer** — `let downloaded = 150mb`
- **Human display** — When showing sizes to users in "familiar" format
- **Marketing/specs** — When matching vendor specifications

---

## Printable Format

### Display Behavior

The `to_str()` method uses the most appropriate unit based on value:

```ori
1024b.to_str()      // "1 KiB" (exact binary boundary)
1000b.to_str()      // "1000 bytes" or "1 KB" (implementation choice)
1536kib.to_str()    // "1.5 MiB"
1500kb.to_str()     // "1.5 MB"
```

Implementation may choose to:
1. Detect if value is a clean binary boundary → use binary unit
2. Otherwise use SI or bytes
3. Or always use the unit family that was used to construct the value (if trackable)

---

## Alternatives Considered

### 1. Binary Only (Remove SI)

**Rejected:** SI units have legitimate use cases (storage capacity, network bandwidth). Removing them would force awkward conversions for those scenarios.

### 2. Make `kb` Mean Binary, Add `kb_si`

**Rejected:** Breaks existing code expectations. The SI meaning of `kb` is well-established in the current spec.

### 3. Use `K`/`Ki` Capitalization

Example: `4K` (SI) vs `4Ki` (binary)

**Rejected:** Case-sensitivity for units is error-prone and inconsistent with Ori's case conventions.

### 4. Provide Only Factory Methods for Binary

Example: `Size.from_kibibytes(kib: 4)` without literal syntax.

**Rejected:** Literals are more ergonomic for the common case of hardcoded sizes. Factory methods are still useful for computed values.

---

## Migration / Compatibility

This proposal is **purely additive**:
- All existing SI literals (`kb`, `mb`, `gb`, `tb`) retain their meaning
- No existing code is affected
- New binary literals are opt-in

Programmers who want binary semantics simply use the new suffixes.

---

## Documentation Updates

### Spec Changes

1. **`spec/06-types.md` § Size:**
   - Add binary unit table
   - Add binary literal examples
   - Add binary conversion methods
   - Add usage guidelines

2. **`spec/03-lexical-elements.md` § Literals:**
   - Add `kib`, `mib`, `gib`, `tib` to size suffix grammar

3. **`grammar.ebnf`:**
   - Update: `size_unit = "b" | "kb" | "mb" | "gb" | "tb" | "kib" | "mib" | "gib" | "tib" .`

### CLAUDE.md Changes

Update Size section:
```
**Size**: 64-bit bytes (non-negative); SI suffixes `b`/`kb`/`mb`/`gb`/`tb` (1000-based);
binary suffixes `kib`/`mib`/`gib`/`tib` (1024-based); decimal syntax (`0.5kib`=512 bytes)
```

---

## Implementation Notes

### Lexer Changes

Add new token variants or extend existing Size token to handle binary suffixes:

```rust
enum SizeUnit {
    Bytes,
    // SI (1000-based)
    Kilobytes,
    Megabytes,
    Gigabytes,
    Terabytes,
    // Binary (1024-based)
    Kibibytes,
    Mebibytes,
    Gibibytes,
    Tebibytes,
}

impl SizeUnit {
    fn multiplier(&self) -> u64 {
        match self {
            Bytes => 1,
            Kilobytes => 1_000,
            Megabytes => 1_000_000,
            Gigabytes => 1_000_000_000,
            Terabytes => 1_000_000_000_000,
            Kibibytes => 1_024,
            Mebibytes => 1_048_576,
            Gibibytes => 1_073_741_824,
            Tebibytes => 1_099_511_627_776,
        }
    }
}
```

### Decimal Parsing

The existing decimal parsing algorithm works unchanged — it simply uses the appropriate multiplier for the unit. The "whole number" validation naturally restricts binary units to valid fractions.

---

## Summary

| Aspect | SI Units | Binary Units |
|--------|----------|--------------|
| Suffixes | `kb`, `mb`, `gb`, `tb` | `kib`, `mib`, `gib`, `tib` |
| Base | 1000 | 1024 |
| Use case | Storage, network, display | Memory, buffers, files |
| Decimal | Any decimal → whole bytes | Power-of-2 fractions only |
| Standard | SI | IEC 60027-2 |

This proposal gives Ori programmers the **right tool for each job** while maintaining backward compatibility with existing code.

---

## References

- IEC 60027-2: Binary prefixes for units of information
- IEEE 1541-2002: Prefixes for Binary Multiples
- Original proposal: `proposals/approved/duration-size-types-proposal.md`
- Decimal literals: `proposals/approved/decimal-duration-size-literals-proposal.md`
