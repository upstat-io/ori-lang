# Proposal: Ori Scripts and Developer Tooling

**Status:** Draft
**Author:** Eric (with Claude)
**Created:** 2026-01-27

---

## Summary

Add a simple, npm-style scripts system to Ori with built-in cross-platform file operations. Keep it dead simple - just string commands - while fixing the pain points that plague npm scripts.

---

## Motivation

### What npm Scripts Gets Right

```json
{
  "scripts": {
    "build": "tsc",
    "test": "jest",
    "dev": "./scripts/dev.sh"
  }
}
```

- Dead simple: just `"name": "command"`
- Run anything: shell commands, scripts, other tools
- Flexible: complex stuff goes in script files
- Familiar: everyone knows how it works

### What npm Scripts Gets Wrong

**1. Cross-platform is broken**

```json
{
  "scripts": {
    "clean": "rm -rf dist",
    "setup": "mkdir -p build"
  }
}
```

This fails on Windows. Teams end up installing `rimraf`, `mkdirp`, `cross-env`, `shx` just to do basic operations.

**2. Slow startup**

`npm run` takes ~176ms. Bun proved it can be done in 7ms.

**3. No parallelism**

Need to install `concurrently` or `npm-run-all` to run scripts in parallel.

### What Cargo Gets Wrong

No scripts at all. The Rust community is fragmented across:
- `cargo-make` (TOML config)
- `just` (Makefile-like)
- `xtask` pattern (write Rust code)
- Plain `Makefile`

"There is no standard in the Rust community."

---

## Design

### Project File

Scripts live in `project.ori` (or `Ori.toml` - format TBD):

```toml
[package]
name = "my-app"
version = "0.1.0"

[scripts]
build = "ori compile --release"
test = "ori test"
clean = "ori rm dist"
setup = "ori mkdir build/cache"
dev = "./scripts/dev.sh"
lint = "ori lint && ori fmt --check"
ci = "ori run lint && ori run test && ori run build"
```

That's it. Strings. Simple.

### Built-in Cross-Platform Commands

Ori provides subcommands for common file operations that work on all platforms:

| Command | Description | Unix Equivalent |
|---------|-------------|-----------------|
| `ori rm <path>` | Remove file or directory | `rm -rf` |
| `ori mkdir <path>` | Create directory (recursive) | `mkdir -p` |
| `ori cp <src> <dest>` | Copy file or directory | `cp -r` |
| `ori mv <src> <dest>` | Move/rename | `mv` |
| `ori cat <file>` | Print file contents | `cat` |
| `ori env KEY=val -- <cmd>` | Run with env vars | `KEY=val cmd` |

These work identically on Windows, Mac, and Linux. No packages needed.

### Running Scripts

```bash
ori run build              # Run a script
ori run test -- --verbose  # Pass args through to the script
ori run lint test          # Run multiple in parallel
```

### Multi-line Scripts

For longer scripts, use TOML multi-line strings:

```toml
[scripts]
deploy = """
ori compile --release
ori rm dist/old
ori cp target/release/app dist/
./scripts/upload.sh
"""
```

Each line runs sequentially. If any fails, it stops.

### Complex Scripts

When it gets complex, use a script file:

```toml
[scripts]
deploy = "./scripts/deploy.sh"
setup-dev = "./scripts/setup-dev.sh"
```

This is the npm pattern and it works. Don't fight it.

---

## CLI

### `ori run <script>`

Run a script by name.

```bash
ori run build
ori run test
```

### `ori run <script> -- <args>`

Pass arguments through to the script.

```bash
ori run test -- --filter integration
# Runs: ori test --filter integration
```

### `ori run <script1> <script2> ...`

Run multiple scripts in parallel.

```bash
ori run lint test    # Both run at same time
```

### `ori scripts`

List available scripts.

```bash
$ ori scripts

Available scripts:
  build    ori compile --release
  test     ori test
  clean    ori rm dist
  dev      ./scripts/dev.sh
  lint     ori lint && ori fmt --check
  ci       ori run lint && ori run test && ori run build
```

---

## Cross-Platform Commands Detail

### `ori rm <path>`

Removes files or directories recursively. No error if doesn't exist.

```bash
ori rm dist
ori rm build/cache
ori rm "path with spaces"
```

### `ori mkdir <path>`

Creates directory and all parent directories.

```bash
ori mkdir dist
ori mkdir build/cache/temp
```

### `ori cp <src> <dest>`

Copies files or directories.

```bash
ori cp config.template config.local
ori cp assets dist/assets
```

### `ori mv <src> <dest>`

Moves or renames files/directories.

```bash
ori mv old-name new-name
ori mv temp/output dist/
```

### `ori env <VAR>=<val> [...] -- <command>`

Runs a command with environment variables set.

```bash
ori env DEBUG=true PORT=3000 -- ori run server
ori env NODE_ENV=production -- npm run build
```

Cross-platform. No `cross-env` package needed.

---

## What We're NOT Doing (For Now)

### Script Dependencies

```toml
# NOT doing this
[scripts.test]
run = "ori test"
depends = ["build"]
```

Just use `&&`:
```toml
[scripts]
test = "ori run build && ori test"
```

Or write a script file. Keep it simple.

### Pre/Post Hooks

```toml
# NOT doing this
pretest = "ori lint"
test = "ori test"
posttest = "echo done"
```

Adds complexity, questionable value. Just be explicit:
```toml
[scripts]
test = "ori lint && ori test && echo done"
```

### Structured Environment Variables

```toml
# NOT doing this
[scripts.dev]
run = "ori run server"
env = { PORT = "3000" }
```

Use `ori env` inline:
```toml
[scripts]
dev = "ori env PORT=3000 -- ori run server"
```

### Watch Mode

```toml
# NOT doing this
[scripts]
dev = { cmd = "ori run server", watch = true }
```

That's a separate feature. For now:
```toml
[scripts]
dev = "ori watch --exec 'ori run server'"
```

---

## Implementation Notes

### Speed Target

- **Goal:** <10ms startup for `ori run`
- **How:** No JavaScript runtime, no heavy parsing. Bun does 7ms, we can match it.

### Shell Execution

- On Unix: Use `sh -c` by default
- On Windows: Use `cmd /c` or detect PowerShell
- The `ori rm/mkdir/cp/mv/env` commands bypass the shell entirely - they're native Ori

### Argument Parsing

Everything after `--` passes through:
```bash
ori run build -- --release --target x86_64
# Script receives: --release --target x86_64
```

### Exit Codes

- Script exit code becomes `ori run` exit code
- For chained commands (`&&`), first failure stops execution

---

## Examples

### Simple Project

```toml
[package]
name = "hello"
version = "0.1.0"

[scripts]
build = "ori compile"
test = "ori test"
clean = "ori rm target"
```

### Web Project

```toml
[package]
name = "web-app"
version = "1.0.0"

[scripts]
dev = "ori env PORT=3000 -- ori run server --watch"
build = "ori compile --release && ori run bundle"
bundle = "ori mkdir dist && ori cp static dist/ && ori run compile-assets"
compile-assets = "./scripts/compile-assets.sh"
test = "ori test"
lint = "ori lint && ori fmt --check"
ci = "ori run lint test && ori run build"
clean = "ori rm target dist"
```

### Monorepo

```toml
[package]
name = "monorepo"
version = "0.0.0"

[scripts]
build-all = "./scripts/build-all.sh"
test-all = "./scripts/test-all.sh"
clean = "ori rm packages/*/target"
```

---

## Comparison

| Feature | npm scripts | Cargo | **Ori** |
|---------|-------------|-------|---------|
| Simple string scripts | ✓ | ✗ | ✓ |
| Cross-platform file ops | ✗ (need packages) | N/A | ✓ (built-in) |
| Fast startup | ✗ (176ms) | N/A | ✓ (<10ms) |
| Parallel execution | ✗ (need packages) | N/A | ✓ (built-in) |
| Pass-through args | ✓ | N/A | ✓ |

---

## Open Questions

1. **File format:** `project.ori` with Ori syntax? Or `Ori.toml` with TOML? Or both supported?

2. **Command name:** `ori run` or just `ori <script>`? (npm uses `npm run`, but `npm test` works without `run`)

3. **Glob support in ori rm/cp:** Should `ori rm *.tmp` work? Or keep it simple with explicit paths?

4. **Script naming:** Any reserved names? (`build`, `test`, `start`?)

---

## Future Iterations

Things we might add later based on feedback:

- **Watch mode:** `ori run --watch build`
- **Script dependencies:** if `&&` chaining proves too limiting
- **Caching:** skip scripts if inputs haven't changed
- **Pre/post hooks:** if there's demand
- **Workspaces:** run scripts across monorepo packages

But start simple. Add complexity only when needed.

---

## References

- [npm scripts](https://docs.npmjs.com/cli/v10/using-npm/scripts) - The baseline
- [Bun's script runner](https://bun.sh/docs/cli/run) - Proof that 7ms is possible
- [Just](https://github.com/casey/just) - Good ideas but separate file
- [cargo-make](https://github.com/sagiegurari/cargo-make) - Shows demand in Rust ecosystem
