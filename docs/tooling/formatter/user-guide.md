# User Guide: ori fmt

The Ori formatter produces canonical source code formatting with minimal configuration. Like Go's `gofmt`, the formatter output is the canonical Ori style. The only configurable option is line width.

## Quick Start

```bash
# Format all files in current directory
ori fmt

# Format a single file
ori fmt src/main.ori

# Format a specific directory
ori fmt src/

# Check if files are formatted (for CI)
ori fmt --check

# Preview changes without modifying files
ori fmt --diff src/main.ori
```

## Command Reference

```
ori fmt [options] [paths...]
```

### Arguments

| Argument | Description |
|----------|-------------|
| `paths` | Files or directories to format (default: `.`) |

### Options

| Option | Description |
|--------|-------------|
| `--width=N` | Set maximum line width (default: 100) |
| `--check` | Check if files are formatted, exit 1 if any would change |
| `--diff` | Show unified diff output instead of modifying files |
| `--stdin` | Read from stdin, write to stdout (for editor integration) |
| `--no-ignore` | Ignore `.orifmtignore` files and format everything |
| `--help` | Show help message |

## Usage Patterns

### Format All Code

```bash
# Format entire project
ori fmt

# Format specific directories
ori fmt src/ tests/
```

### CI Integration

Use `--check` in CI pipelines to enforce formatting:

```bash
# Fails with exit code 1 if any files would be formatted
ori fmt --check

# With verbose output showing which files would change
ori fmt --check src/
```

Example GitHub Actions workflow:

```yaml
- name: Check formatting
  run: ori fmt --check
```

### Preview Changes

Use `--diff` to see what would change without modifying files:

```bash
ori fmt --diff src/main.ori
```

Output shows unified diff format:

```diff
--- src/main.ori
+++ src/main.ori
@@ -1,3 +1,3 @@
-@main() -> void = print(msg:"Hello")
+@main () -> void = print(msg: "Hello")
```

### Pipe Through Stdin

For editor integration or scripting:

```bash
# Format content from stdin
cat src/main.ori | ori fmt --stdin

# Use with process substitution
diff <(ori fmt --stdin < src/main.ori) src/main.ori
```

## Ignoring Files

### .orifmtignore

Create a `.orifmtignore` file to exclude paths from formatting:

```gitignore
# Ignore generated code
generated/

# Ignore specific file patterns
**/*.generated.ori
*.tmp

# Ignore vendor code
vendor/
```

### Pattern Syntax

| Pattern | Matches |
|---------|---------|
| `**/*.test.ori` | Any path ending in `.test.ori` |
| `*.tmp` | Files named `*.tmp` in any single directory |
| `generated/` | The entire `generated/` directory |
| `# comment` | Lines starting with `#` are ignored |

### Default Ignores

Unless `--no-ignore` is specified, these are always ignored:

- Hidden files and directories (starting with `.`)
- `target/` directory
- `node_modules/` directory

Use `--no-ignore` to format everything:

```bash
ori fmt --no-ignore
```

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success (all files formatted or already formatted) |
| 1 | Files would be changed (`--check`) or parse errors occurred |

## Error Handling

### Parse Errors

Files with syntax errors cannot be formatted. The formatter shows helpful error messages:

```
error[E1001]: expected `=`, found integer
  --> src/broken.ori:1:19
   |
 1 | @broken () -> int 42
   |                   ^^
  = help: function definitions require `=` before the body

note: fix the syntax error to enable formatting
```

### Partial Formatting

Like `gofmt` and `rustfmt`, `ori fmt` requires valid syntax. If a file has parse errors, it is skipped with an error message. Fix the syntax first, then format.

## Performance

The formatter is optimized for large codebases:

- Uses parallel processing for directories (2.4x speedup)
- Formats 10,000 lines in ~3ms
- Memory-efficient (minimal allocation)

For very large repositories, format incrementally or use `.orifmtignore` to exclude non-essential paths.

## Best Practices

1. **Format before commit**: Run `ori fmt` before committing to ensure consistent style
2. **CI enforcement**: Add `ori fmt --check` to CI to catch unformatted code
3. **Editor integration**: Configure format-on-save (see [Integration Guide](integration.md))
4. **Ignore generated code**: Add generated files to `.orifmtignore`

## See Also

- [Integration Guide](integration.md) — Setting up editor integration
- [Style Guide](style-guide.md) — What the formatter enforces
- [Troubleshooting](troubleshooting.md) — Common issues and solutions
