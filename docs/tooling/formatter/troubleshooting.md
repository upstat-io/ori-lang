# Troubleshooting Guide: ori fmt

This guide covers common issues and their solutions when using the Ori formatter.

## Common Issues

### "Parse Error" — File Cannot Be Formatted

**Symptom**: The formatter reports a parse error and skips the file.

```
error[E1001]: expected `=`, found integer
  --> src/broken.ori:1:19
   |
 1 | @broken () -> int 42
   |                   ^^
  = help: function definitions require `=` before the body

note: fix the syntax error to enable formatting
```

**Cause**: The formatter requires valid Ori syntax. Files with parse errors cannot be formatted.

**Solution**: Fix the syntax error first. The error message includes:
- Location (file:line:column)
- Source snippet with underline
- Helpful suggestion

In this case, add `=` before the function body:

```ori
@broken () -> int = 42
```

### File Not Changed When Expected

**Symptom**: Running `ori fmt` reports "0 formatted, 1 unchanged" but you expected changes.

**Possible causes**:

1. **File is already formatted**: The formatter is idempotent. If a file is already in canonical format, no changes are made.

2. **File has parse errors**: Check for error messages. Files with syntax errors are skipped.

3. **File is ignored**: Check `.orifmtignore` or default ignores.

**Solution**: Run `ori fmt --diff file.ori` to see what (if anything) would change.

### Changes Don't Match Expectations

**Symptom**: The formatter makes changes you didn't expect.

**Cause**: The formatter enforces a specific style with no configuration options. Some changes you might not expect:

| You wrote | Formatter outputs | Why |
|-----------|-------------------|-----|
| `@f()->int` | `@f () -> int` | Spaces around arrows and after function name |
| `{a:1,b:2}` | `{ a: 1, b: 2 }` | Spaces inside braces and after colons |
| `[1,2,3]` | `[1, 2, 3]` | Spaces after commas |
| Tabs | 4 spaces | Tabs converted to spaces |
| Multiple blank lines | Single blank line | Consecutive blank lines collapsed |

**Solution**: Accept the canonical style. The formatter intentionally has no options. See the [Style Guide](style-guide.md) for complete formatting rules.

### Exit Code 1 in CI

**Symptom**: CI fails with exit code 1 when running `ori fmt --check`.

**Cause**: One or more files would be formatted (i.e., are not in canonical format).

**Solution**:

1. Run locally to see which files need formatting:
   ```bash
   ori fmt --check
   ```

2. Format them:
   ```bash
   ori fmt
   ```

3. Commit the changes and push.

**Prevention**: Set up format-on-save in your editor. See [Integration Guide](integration.md).

### Large Repository Takes Too Long

**Symptom**: Formatting a large repository is slow.

**Cause**: Many files need processing.

**Solutions**:

1. **Use `.orifmtignore`** to exclude non-essential paths:
   ```gitignore
   # Ignore generated code
   generated/
   **/*.generated.ori

   # Ignore vendored code
   vendor/
   ```

2. **Format incrementally** during development:
   ```bash
   # Only format changed files
   git diff --name-only --diff-filter=d | grep '\.ori$' | xargs ori fmt
   ```

3. **CI optimization**: Only check modified files:
   ```bash
   git diff --name-only origin/main...HEAD | grep '\.ori$' | xargs ori fmt --check
   ```

### stdin Mode Issues

**Symptom**: `ori fmt --stdin` hangs or produces unexpected output.

**Solutions**:

1. **Check for parse errors**: Errors go to stderr, not stdout:
   ```bash
   echo 'broken' | ori fmt --stdin 2>&1
   ```

2. **Ensure input is complete**: stdin must contain valid, complete Ori code.

3. **Don't combine with path arguments**:
   ```bash
   # Wrong - will fail
   ori fmt --stdin file.ori

   # Correct
   cat file.ori | ori fmt --stdin
   ```

### Diff Output Hard to Read

**Symptom**: `ori fmt --diff` output is difficult to interpret.

**Solution**: Use external diff tools for better visualization:

```bash
# Using diff with color
diff -u <(cat file.ori) <(ori fmt --stdin < file.ori) | less -R

# Using delta (if installed)
diff -u <(cat file.ori) <(ori fmt --stdin < file.ori) | delta

# Using git diff
ori fmt --stdin < file.ori > /tmp/formatted.ori
git diff --no-index file.ori /tmp/formatted.ori
```

## Error Messages Reference

### Lexer Errors (E0xxx)

| Code | Message | Solution |
|------|---------|----------|
| E0001 | Unterminated string | Add closing `"` |
| E0004 | Unterminated character literal | Use single quotes: `'a'` |
| E0005 | Invalid escape sequence | Valid: `\n`, `\t`, `\r`, `\\`, `\"`, `\'` |

### Parser Errors (E1xxx)

| Code | Message | Solution |
|------|---------|----------|
| E1001 | Unexpected token | Check for missing punctuation |
| E1002 | Expected expression | Add an expression where expected |
| E1003 | Unclosed delimiter | Add missing `)`, `}`, or `]` |
| E1004 | Expected identifier | Use valid identifier (letter/underscore start) |
| E1005 | Expected type | Add type annotation: `name: Type` |
| E1006 | Invalid function definition | Use: `@name (params) -> Type = body` |
| E1007 | Missing function body | Add `= expression` after signature |
| E1011 | Named arguments required | Use: `func(arg1: val, arg2: val)` |

## Platform-Specific Issues

### Windows Line Endings

**Symptom**: Files show as changed after formatting due to line endings.

**Cause**: Git or editor configured for Windows line endings (CRLF).

**Solution**: Configure Git for LF line endings in Ori files:

```gitattributes
# .gitattributes
*.ori text eol=lf
```

### PATH Issues

**Symptom**: `ori: command not found`

**Solutions**:

```bash
# Check if ori is installed
which ori

# If installed via cargo, add to PATH
export PATH="$HOME/.cargo/bin:$PATH"

# Or use full path
~/.cargo/bin/ori fmt
```

### Permission Denied

**Symptom**: Cannot write formatted output back to file.

**Cause**: File is read-only or in a protected directory.

**Solution**:
```bash
# Check file permissions
ls -la file.ori

# Fix if needed
chmod u+w file.ori
```

## Getting Help

If you encounter an issue not covered here:

1. Check the [User Guide](user-guide.md) for correct usage
2. Check the [Style Guide](style-guide.md) for expected output
3. Run with verbose output to diagnose:
   ```bash
   ori fmt --diff file.ori 2>&1 | less
   ```
4. Report bugs at: https://github.com/ori-lang/ori/issues

## See Also

- [User Guide](user-guide.md) — Command-line usage
- [Style Guide](style-guide.md) — What the formatter enforces
- [Integration Guide](integration.md) — Editor setup
