# Sigil Language Documentation

This directory contains all documentation for the Sigil programming language, organized by version.

## Structure

```
sigil_lang/
├── README.md       # This file
├── CLAUDE.md       # AI guidance for documentation maintenance
└── {version}/      # Versioned documentation
    ├── spec/       # Language specification (normative)
    ├── design/     # Design rationale (informative)
    ├── guide/      # User guide (tutorial)
    └── modules/    # Standard library documentation
```

## Versions

| Version | Status | Description |
|---------|--------|-------------|
| [0.1-alpha](0.1-alpha/) | Active | Initial development version |

## Document Types

| Type | Purpose | Audience |
|------|---------|----------|
| **spec/** | Formal language definition | Compiler authors, tool developers |
| **design/** | Rationale and philosophy | Language designers, contributors |
| **guide/** | Tutorials and how-tos | Users learning Sigil |
| **modules/** | Standard library reference | All developers |

## Versioning Policy

All documentation within a version is kept in sync:

- **Spec** defines what the language IS
- **Design** explains WHY it works that way
- **Guide** teaches HOW to use it
- **Modules** documents the standard library

When the language changes, ALL parts update together in a new version.

### Version Stages

| Stage | Meaning |
|-------|---------|
| `alpha` | Unstable, breaking changes expected |
| `beta` | Feature-complete, stabilizing |
| `rc` | Release candidate, final review |
| (none) | Stable release |

## Quick Links

### Current Version (0.1-alpha)

- [Language Specification](0.1-alpha/spec/index.md)
- [Design Documentation](0.1-alpha/design/00-index.md)
- [Standard Library](0.1-alpha/modules/std/)

## Contributing

Documentation guidelines and synchronization rules are defined in `.claude/rules/sigil-lang-docs.md`.

**Key rule:** Changes to any document type may require updates to others. Always keep spec, design, guide, and modules in sync.
