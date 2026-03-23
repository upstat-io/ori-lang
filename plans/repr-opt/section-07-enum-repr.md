---
section: "07"
title: "Enum Representation Optimization"
status: not-started
reviewed: false
goal: "Optimize enum layout with niche filling, discriminant narrowing, tagged pointers, and payload compression — matching Rust's enum layout optimizations"
inspired_by:
  - "Rust niche optimization (compiler/rustc_abi/src/layout.rs, Niche struct)"
  - "Rust Option<&T> = pointer with null niche (compiler/rustc_abi/src/lib.rs)"
  - "Swift enum layout (lib/IRGen/GenEnum.cpp)"
  - "Zig optional representation (src/Type.zig)"
depends_on: ["04", "05"]
sections:
  - id: "07.1"
    title: "Niche Filling"
    status: not-started
  - id: "07.2"
    title: "Discriminant Narrowing"
    status: not-started
  - id: "07.3"
    title: "Tagged Pointers"
    status: not-started
  - id: "07.4"
    title: "Payload Compression"
    status: not-started
  - id: "07.5"
    title: "Completion Checklist"
    status: not-started
---

# Section 07: Enum Representation Optimization

**Context:** Today, Ori enums use `{i64 tag, [M x i64] payload}` — every enum has a full i64 tag plus the maximum variant payload (padded to i64 word size). This wastes memory:
- `Option<int>`: 16 bytes (i64 tag + i64 value) → could be 8 bytes via niche or tagged pointer
- `Option<bool>`: 16 bytes (i64 tag + padded i1 value) → could be 1 byte (value 2 = None)
- `Option<str>`: 32 bytes (i64 tag + 24-byte str payload) → could be 24 bytes (null ptr = None)
- All-unit enum with N variants: 8 bytes (i64 tag only, no payload) → could be 1 byte (i8 tag)

Rust's niche optimization is the gold standard. We study and match it.

**Reference implementations:**
- **Rust** `compiler/rustc_abi/src/layout.rs`: `Niche` struct with `available()`, `reserve()` — tracks invalid bit patterns per type
- **Rust** `compiler/rustc_abi/src/lib.rs`: `NaiveLayout`, `LayoutData`, `Variants::Multiple`
- **Swift** `lib/IRGen/GenEnum.cpp`: Multi-payload enum layout with spare bits analysis

**Depends on:** §04 (narrowed integer types create new niches — e.g., a narrowed `i8` field with range `[0, 2]` has 253 unused values as niches), §05 (float-narrowed fields may have niche patterns — e.g., `f32` has NaN spare bits usable for Option optimization, though the value must be checked carefully against IEEE 754 NaN semantics before use).

---

## 07.1 Niche Filling

**File(s):** `compiler/ori_repr/src/layout/enum_repr.rs`, `compiler/ori_repr/src/layout/niche.rs`

A "niche" is an invalid bit pattern in a type. If an enum variant's payload has a niche, we can use it to encode a different variant, eliminating the explicit tag.

- [ ] Define `Niche`:
  ```rust
  pub struct Niche {
      /// Which field contains the niche
      pub field_index: u32,
      /// Byte offset within the field
      pub offset: u32,
      /// Number of available niche values
      pub available: u128,
      /// Starting value of the niche range
      pub start: u128,
  }
  ```

- [ ] Identify niches for each type:
  ```rust
  pub fn find_niches(repr: &MachineRepr) -> Vec<Niche> {
      match repr {
          // bool: values 0 and 1 → niche at value 2..=255 (254 niches)
          MachineRepr::Bool => vec![Niche {
              field_index: 0, offset: 0, available: 254, start: 2,
          }],

          // Ordering: values 0,1,2 → niche at 3..=255 (253 niches)
          MachineRepr::Int { width: IntWidth::I8, .. }
              if /* is ordering */ => vec![Niche { available: 253, start: 3, .. }],

          // Narrowed int i8 with range [0, N] → niche at N+1..=255
          MachineRepr::Int { width: IntWidth::I8, signed: false }
              => /* depends on range */,

          // Reference/pointer: null (0) is never a valid heap pointer
          MachineRepr::RcPointer(_) => vec![Niche {
              field_index: 0, offset: 0, available: 1, start: 0, // null = niche
          }],

          // Fat pointer (str, [T], {K:V}): layout is {i64 len, i64 cap, ptr data}
          // Niche is at field_index 2 (the data pointer, field 0=len, 1=cap, 2=data)
          // null data pointer (0) indicates None — heap data is never null
          MachineRepr::FatPointer(_) => vec![Niche {
              field_index: 2, offset: 0, available: 1, start: 0,
          }],

          // Nested enum: if it has unused discriminant values
          MachineRepr::Enum(e) => find_enum_niches(e),

          // Char: 0x110000..=0xFFFFFFFF are invalid Unicode (huge niche space)
          MachineRepr::Int { width: IntWidth::I32, .. }
              if /* is char */ => vec![Niche { available: 0xFFFFFFFF - 0x10FFFF, start: 0x110000, .. }],

          _ => vec![],
      }
  }
  ```

- [ ] Apply niche to `Option<T>`:
  ```rust
  pub fn optimize_option_repr(inner: &MachineRepr) -> EnumRepr {
      let niches = find_niches(inner);
      if let Some(niche) = niches.first() {
          if niche.available >= 1 {
              // None variant uses the niche value — no tag needed!
              return EnumRepr {
                  tag: EnumTag::Niche {
                      field_index: niche.field_index,
                      niche_value: niche.start as u64,
                  },
                  variants: vec![/* Some payload */, /* None = niche */],
                  size: inner.size(),
                  align: inner.alignment(),
              };
          }
      }
      // Fallback: explicit tag
      default_option_repr(inner)
  }
  ```

- [ ] Apply niche to `Result<T, E>`:
  - If T has a niche, Err variant uses it (or vice versa)
  - If both have niches, prefer the one that eliminates more padding

---

## 07.2 Discriminant Narrowing

**File(s):** `compiler/ori_repr/src/layout/enum_repr.rs`

The discriminant (tag) should use the minimum width needed.

- [ ] Compute minimum tag width:
  ```rust
  pub fn min_tag_width(variant_count: usize) -> IntWidth {
      if variant_count <= 1 { return IntWidth::I8; } // or no tag at all
      let bits_needed = (variant_count as f64).log2().ceil() as u32;
      match bits_needed {
          0..=8 => IntWidth::I8,    // up to 256 variants
          9..=16 => IntWidth::I16,  // up to 65536 variants
          17..=32 => IntWidth::I32, // up to 4 billion variants
          _ => IntWidth::I64,
      }
  }
  ```

- [ ] Current behavior: always `i8` — already handles ≤256 variants
- [ ] Extension: for enums with >256 variants (rare but possible), use `i16`/`i32`
- [ ] For single-variant enums (newtypes), eliminate tag entirely

---

## 07.3 Tagged Pointers

**File(s):** `compiler/ori_repr/src/layout/tagged_ptr.rs`

On 64-bit systems, heap pointers have alignment ≥8, meaning the low 3 bits are always zero. These bits can store a 3-bit tag (up to 8 variants).

- [ ] Implement tagged pointer optimization:
  ```rust
  pub fn can_use_tagged_pointer(enum_repr: &EnumRepr) -> bool {
      // At most 8 variants (3 bits for tag)
      if enum_repr.variants.len() > 8 {
          return false;
      }
      // Every non-unit variant must be a pointer type with alignment ≥ 8.
      // The decode path uses `value & ~0x7` to recover the pointer, which
      // would corrupt non-pointer payloads (e.g., masking int(5) gives 0).
      // Unit variants (size == 0) are fine — they carry no payload, just a tag.
      enum_repr.variants.iter().all(|v| {
          v.size == 0 || (v.is_pointer() && v.alignment >= 8)
      })
      // At least one variant must actually be a pointer (otherwise
      // there's no benefit — use discriminant narrowing instead)
      && enum_repr.variants.iter().any(|v| v.is_pointer())
  }
  ```

- [ ] Tagged pointer layout:
  ```
  [63:3] pointer value  [2:0] tag
  ```
  - Store pointer variant: `ptr | tag` (low 3 bits of ptr are 0 due to alignment)
  - Load tag: `value & 0x7`
  - Load pointer: `value & ~0x7`
  - Unit variants: only the tag value matters, no payload to decode

- [ ] Safety:
  - Only applicable when the runtime guarantees 8-byte aligned allocations (ori_rt already does: alignment is always ≥ 8)
  - Non-pointer scalar payloads (int, bool, float) are **excluded** — their low bits carry data that `& ~0x7` would destroy
  - Future: could support scalar payloads by shifting them left 3 bits during encode and right 3 bits during decode, at the cost of reducing the usable range (61 bits instead of 64)

---

## 07.4 Payload Compression

**File(s):** `compiler/ori_repr/src/layout/enum_repr.rs`

When variant payloads have different sizes, the current approach uses `max(sizeof(variant))` for all. We can do better.

- [ ] Shared prefix optimization:
  - If multiple variants start with the same fields, share that prefix
  - Example: `enum Shape { Circle(float), Rect(float, float) }` — `float` prefix shared

- [ ] Size-class bucketing:
  - Group variants by payload size class
  - Small variants (≤ 8 bytes): inline in the enum
  - Large variants (> threshold): heap-allocate payload, store pointer
  - This prevents one large variant from bloating all others

- [ ] All-unit variant detection (already implemented):
  - Enums where every variant is unit → tag only, no payload
  - Verify this path is still used and correct

---

## 07.5 Completion Checklist

**Test matrix for §07 (write failing tests FIRST, verify they fail, then implement):**

| Type | Expected representation | Semantic pin |
|---|---|---|
| `Option<bool>` | 1 byte `i8`: `Some(false)=0`, `Some(true)=1`, `None=2` | Yes — `sizeof == 1`, no struct wrapper |
| `Option<Ordering>` | 1 byte `i8`: `Some(Less)=0`, `Some(Equal)=1`, `Some(Greater)=2`, `None=3` | Yes — `sizeof == 1` |
| `Option<str>` | 24 bytes (null data ptr niche for None, no tag field) | Yes — `sizeof == sizeof(str)` |
| `Option<[int]>` | 24 bytes (null data ptr niche for None) | Yes — `sizeof == sizeof([int])` |
| `Option<int>` | 16 bytes (no niche available in i64 — must use explicit tag) | Yes — `sizeof == 16` |
| All-unit enum `type Dir = North \| South \| East \| West` | `i8` tag, no payload | Yes — `sizeof == 1` |
| Single-variant enum `type Wrapper(val: int)` | newtype erasure — same as `int` (no tag) | Yes — `sizeof == 8` |
| `Result<bool, Ordering>` | 1 byte (niche from bool payload covers Ordering variants) | Yes — niche across Result arms |
| Narrowed `i8` field with range `[0, 2]` after §04 | 253 niche values available | Yes — §04+§07 interaction |
| `f32`-typed field after §05 | NaN niches conservatively skipped | Yes — no NaN-based niche |
| Pattern match on `Option<bool>` with niche repr | Correct values: `None` = 2, `Some(false)` = 0 | Yes — match produces correct results |

- [ ] Write failing test matrix BEFORE implementation (verify tests fail with current `{ i64 tag, payload }` layout)
- [ ] `Option<bool>` → 1 byte (niche value 2 for None)
- [ ] `Option<Ordering>` → 1 byte (niche value 3+ for None)
- [ ] `Option<str>` → 24 bytes (null ptr niche for None, no tag byte — same size as str itself)
- [ ] `Option<[int]>` → 24 bytes (null ptr niche — same size as [int] itself)
- [ ] All-unit enums → tag-only (no payload) — already working, verify preserved
- [ ] Single-variant enums → newtype erasure (no tag)
- [ ] Discriminant uses minimum width (i8 for ≤256, i16 for ≤65536)
- [ ] Niche analysis queries `ReprPlan` for narrowed field types, not canonical types (§04+§07 interaction)
- [ ] `f32`-typed fields (from §05) use empty niche list (NaN-based niches conservatively skipped)
- [ ] Pattern matching codegen correctly reads niche-encoded variants (the match is the most dangerous codegen path)
- [ ] Add semantic pin test: `Option<bool>` LLVM type is `i8` (not `{ i64, i1 }`), with `None` encoded as integer 2. This test can ONLY pass with niche optimization enabled.
- [ ] `./test-all.sh` green in both debug (`cargo b`) and release (`cargo b --release`) builds
- [ ] `./clippy-all.sh` green
- [ ] `./diagnostics/valgrind-aot.sh` clean

**Exit Criteria:** `Option<bool>` compiles to a single `i8` in LLVM IR (no struct wrapper), with `None = 2`, `Some(false) = 0`, `Some(true) = 1`. Verified by inspecting LLVM IR and running all Option-related spec tests.
