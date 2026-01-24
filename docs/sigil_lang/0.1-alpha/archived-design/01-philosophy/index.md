# Philosophy

This section describes Sigil's design philosophy and the principles that guide language decisions.

---

## Documents

| Document | Description |
|----------|-------------|
| [AI-First Design](01-ai-first-design.md) | Why Sigil optimizes for AI-authored code |
| [Core Principles](02-core-principles.md) | Explicitness, consistency, minimalism, pragmatism |

---

## Overview

Sigil's core thesis: **AI will be the primary author of code in the future.** Sigil is designed for AI as a first-class citizen, while remaining human-readable and writable.

### What This Means

| Concern | Human-First Languages | AI-First (Sigil) |
|---------|----------------------|------------------|
| Verbosity | Minimize typing | Doesn't matter (AI types fast) |
| Consistency | Nice to have | Critical (AI learns patterns) |
| Explicitness | Can rely on context | Essential (no ambiguity) |
| Error messages | Help human debug | Help AI self-correct |
| "Magic" features | Convenient shortcuts | Avoid (unpredictable) |
| Multiple ways to do X | Flexibility | Bad (AI might pick wrong one) |
| Testing | Often skipped | Mandatory (validates AI output) |

### Key Insight

Traditional languages optimize for typing speed and expression brevity. AI doesn't care about typing — it generates tokens instantly. AI cares about:

- **Correctness** - Will the code work?
- **Predictability** - Can I reason about what it does?
- **Verifiability** - Can I check if it's right?

Sigil optimizes for these concerns.

---

## See Also

- [Main Index](../00-index.md)
- [Basic Syntax](../02-syntax/01-basic-syntax.md)
