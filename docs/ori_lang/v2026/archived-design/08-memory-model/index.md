# Memory Model

This section covers Ori's memory management: automatic reference counting, value semantics, and efficient data structures.

---

## Documents

| Document | Description |
|----------|-------------|
| [ARC Overview](01-arc-overview.md) | Automatic Reference Counting strategy |
| [Value Semantics](02-value-semantics.md) | Immutability, shadowing, bindings |
| [Strings and Lists](03-strings-and-lists.md) | SSO, structural sharing |

---

## Overview

Ori uses Automatic Reference Counting (ARC) for memory management:

| Strategy | AI Complexity | Runtime Cost | Determinism |
|----------|---------------|--------------|-------------|
| GC | Low | Medium (pauses) | No |
| **ARC** | **Low** | **Low** | **Yes** |
| Ownership | High | None | Yes |

### Why ARC?

1. **Simple mental model** - Values live until nothing references them
2. **Deterministic** - Destruction happens at predictable points
3. **No lifetime complexity** - AI doesn't need to reason about borrows

### Key Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Core strategy | ARC | Simple, deterministic |
| Cycle handling | Compile-time prevention | No runtime overhead, fully deterministic |
| Value vs reference | Implicit, size-based | Compiler optimizes |
| Strings | Ref-counted + SSO | Efficient for all sizes |
| Lists | Structural sharing | Makes immutability cheap |

---

## See Also

- [Main Index](../00-index.md)
- [Value Semantics](02-value-semantics.md)
- [Type System](../03-type-system/index.md)
