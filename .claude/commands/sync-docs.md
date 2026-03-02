# Update Design Docs

Update the design documentation to accurately describe the current implementation. This covers three areas:

- **Compiler Design** (`docs/compiler/design/`) — Compiler internals
- **Formatter** (`docs/tooling/formatter/design/`) — Code formatter design
- **LSP** (`docs/tooling/lsp/design/`) — Language server design

## Paths

**IMPORTANT:** Design docs are located at:

| Area | Path |
|------|------|
| Compiler | `docs/compiler/design/` |
| Formatter | `docs/tooling/formatter/design/` |
| LSP | `docs/tooling/lsp/design/` |

NOT at `docs/ori_lang/` (that's the language spec, not design docs).

## Directory Structure

### Compiler Design

```
docs/compiler/design/
├── index.md                    # Overview and navigation
├── 01-architecture/            # Compiler architecture
├── 02-intermediate-representation/  # IR design
├── 03-lexer/                   # Lexer design
├── 04-parser/                  # Parser design
├── 05-type-system/             # Type system design
├── 06-pattern-system/          # Pattern matching design
├── 07-evaluator/               # Interpreter/evaluator design
├── 08-diagnostics/             # Error reporting design
├── 09-testing/                 # Test infrastructure design
├── 10-llvm-backend/            # LLVM code generation
└── appendices/                 # Reference materials
```

### Formatter

```
docs/tooling/formatter/design/
├── index.md                    # Overview
├── 01-algorithm/               # Core formatting algorithm
├── 02-constructs/              # Per-construct formatting rules
├── 03-comments/                # Comment handling
├── 04-implementation/          # Implementation approach
└── appendices/                 # Edge cases
```

### LSP

```
docs/tooling/lsp/design/
├── index.md                    # Overview
├── 01-protocol/                # LSP protocol handling
├── 02-architecture/            # Crate structure, WASM
├── 03-features/                # Feature implementations
└── 04-integration/             # Editor/playground integration
```

## When to Update

Update design docs when:

| Change Type | Affected Docs |
|-------------|---------------|
| AST nodes, parser changes | Compiler: `04-parser/`, `06-pattern-system/` |
| Type system changes | Compiler: `05-type-system/` |
| Evaluator changes | Compiler: `07-evaluator/` |
| New patterns | Compiler: `06-pattern-system/` |
| Error handling | Compiler: `08-diagnostics/` |
| Formatting rules | Formatter: `01-algorithm/`, `02-constructs/` |
| New construct formatting | Formatter: `02-constructs/` |
| LSP features | LSP: `03-features/` |
| Editor integration | LSP: `04-integration/` |

## Update Process

1. **Identify affected docs** based on what changed (see table above)

2. **Read the relevant source files** to understand current implementation:
   - Compiler: `compiler/ori_*/src/`
   - Formatter: `compiler/ori_fmt/src/`
   - LSP: `compiler/ori_lsp/src/`

3. **Update the design docs** to accurately describe:
   - Data structures and their purpose
   - Algorithms and approaches
   - Interfaces and APIs
   - Examples showing current syntax/behavior

4. **Cross-reference with roadmap** (`plans/roadmap/priority-and-tracking.md`)

## Writing Style

### Document the Current Design, Not Changes

**CRITICAL:** Design docs describe **what IS**, not what changed or was fixed.

**DO NOT write:**
- "This was changed to..."
- "Previously X, now Y..."
- "The problem was... the solution is..."
- "This fix enables..."
- Progress updates or completion notes

**DO write:**
- "The lexer produces X tokens"
- "The parser synthesizes Y from Z"
- "This design enables..."
- Present tense, factual descriptions

Write as if documenting a fresh codebase for someone who has never seen previous versions. The reader doesn't care what it used to be — they need to understand what it is now.

### Content Guidelines

Design docs explain **WHY** and **HOW**, not just what:
- Explain design decisions and trade-offs
- Document the reasoning behind choices
- Include diagrams or ASCII art where helpful
- Reference the spec (`docs/ori_lang/v2026/spec/`) for normative definitions

### What to Exclude

**DO NOT include:**
- Unit test counts or numbers (e.g., "tested by 47 unit tests")
- Test coverage percentages
- Specific test file counts
- Any metrics that change as tests are added/removed

These are volatile implementation details, not design documentation. Design docs describe architecture and design decisions that remain stable — test counts belong in CI reports, not design documents.

## Example Updates

### Compiler: Multi-field variant patterns

1. Update `docs/compiler/design/06-pattern-system/index.md`:
   - Document `MatchPattern::Variant { inner: Vec<MatchPattern> }` structure

2. Update `docs/compiler/design/04-parser/grammar-modules.md`:
   - Document `parse_variant_inner_patterns()` helper

3. Update `docs/compiler/design/07-evaluator/tree-walking.md`:
   - Document how `try_match()` handles multi-field variants

### Formatter: New construct formatting

1. Update `docs/tooling/formatter/design/02-constructs/patterns.md`:
   - Document formatting rules for the new construct
   - Include inline and broken examples

### LSP: New feature

1. Update `docs/tooling/lsp/design/03-features/index.md`:
   - Add feature to the list with LSP method

2. Create or update specific feature doc:
   - Document request/response handling
   - Explain compiler integration

## Output

Report what was updated:
- Which design doc files were modified
- Summary of changes made
- Any new sections added
- Cross-references updated
