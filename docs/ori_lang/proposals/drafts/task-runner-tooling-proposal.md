# Proposal: Ori Task Runner and Developer Tooling

**Status:** Draft
**Author:** Eric (with Claude)
**Created:** 2026-01-27

---

## Summary

Design a first-class task runner and developer tooling system for Ori that leverages the language's unique features: expression-based syntax, capabilities, type safety, and mandatory testing. The goal is to create the best-in-class developer experience that eliminates the pain points of existing systems while providing features no other language offers.

---

## Motivation

### The Current Landscape is Broken

Every major language ecosystem has task runner pain points:

| Ecosystem | Tool | Problems |
|-----------|------|----------|
| **Rust** | None built-in | Fragmented: cargo-make, just, make, xtask pattern. "No standard in the Rust community" |
| **Node.js** | npm scripts | Cross-platform hell (`rm -rf` fails on Windows), 176ms startup overhead, verbose for complex tasks |
| **Go** | None built-in | Relies on Makefiles or Taskfile.yml. "Build specifications scattered all over the place" |
| **Python** | Various | Poetry scripts limited, tox/nox complex, pyproject.toml still evolving |
| **Java** | Maven/Gradle | XML verbosity (Maven) or steep learning curve (Gradle). "Gradle is complex, hard to learn" |

### What Developers Actually Want

From extensive research (Bun, Just, Taskfile, cargo-make, pnpm, Poetry):

**Speed**: Bun's `bun run` takes 7ms vs npm's 176ms. This matters.

**Cross-Platform by Default**: No `rimraf` wrappers, no `cross-env`, no OS-specific conditionals.

**Single Source of Truth**: One file for configuration, not scattered across package.json, Makefile, justfile, etc.

**Readable Syntax**: No cryptic `$@`, `$<`, `.PHONY` from Make.

**Type Safety for Tasks**: Why should build scripts be stringly-typed?

**Dependencies**: Tasks should know what other tasks they depend on.

**Parallelism**: Independent tasks should run concurrently by default.

**Incremental Execution**: Don't re-run tasks if sources haven't changed.

### The Ori Opportunity

Ori has unique advantages no other language has:

1. **Capabilities System** — Tasks can declare effects (`uses FileSystem, Http`)
2. **Expression-Based Syntax** — Tasks can be expressions, not just shell commands
3. **Type System** — Task parameters can be typed and validated
4. **Mandatory Testing** — Tasks can have tests just like functions
5. **Parallel Pattern** — Built-in concurrency primitives
6. **Dependency-Aware Testing** — Already tracks what depends on what

This proposal leverages these to create something genuinely new.

---

## Design

### Core Principle: Tasks Are Ori Code

Unlike every other task runner, Ori tasks are written in Ori itself. No YAML, no TOML, no custom DSL.

```ori
// project.ori - the one file for all project configuration
$project = Project {
    name: "my-app",
    version: "1.0.0",
}

// Tasks are just functions with the @task attribute
#[task]
@build () -> void uses FileSystem = run(
    shell(cmd: "cargo build --release"),
)

#[task(depends: [@lint, @test])]
@ci () -> void = run(
    print(msg: "CI passed!"),
)
```

**Why This Matters:**
- Syntax highlighting works out of the box
- IDE autocomplete works
- Type checking works
- Refactoring tools work
- No new syntax to learn

### The `project.ori` File

Every Ori project has a `project.ori` at the root. This is the single source of truth.

```ori
// project.ori

// ═══════════════════════════════════════════════════════════════════════════
// Project Metadata
// ═══════════════════════════════════════════════════════════════════════════

$project = Project {
    name: "my-app",
    version: "0.1.0",
    authors: ["Alice <alice@example.com>"],
    license: "MIT",
    repository: "https://github.com/alice/my-app",

    // Minimum Ori version required
    ori: "0.1.0",
}

// ═══════════════════════════════════════════════════════════════════════════
// Dependencies
// ═══════════════════════════════════════════════════════════════════════════

$deps = {
    "std": "0.1.0",
    "json": "1.2.3",
    "http": { version: "2.0.0", features: ["tls"] },
}

$dev_deps = {
    "test-utils": "1.0.0",
}

// ═══════════════════════════════════════════════════════════════════════════
// Tasks
// ═══════════════════════════════════════════════════════════════════════════

#[task(desc: "Build the project")]
@build (release: bool = false) -> void uses FileSystem = run(
    let flags = if release then "--release" else "",
    shell(cmd: f"ori compile {flags}"),
)

#[task(desc: "Run all tests")]
@test (filter: str = "") -> void uses FileSystem = run(
    let args = if is_empty(collection: filter) then "" else f"--filter {filter}",
    shell(cmd: f"ori test {args}"),
)

#[task(desc: "Format code")]
@fmt (check: bool = false) -> void uses FileSystem = run(
    let mode = if check then "--check" else "",
    shell(cmd: f"ori fmt {mode}"),
)

#[task(desc: "Lint code")]
@lint () -> void uses FileSystem = run(
    shell(cmd: "ori lint"),
)

#[task(desc: "Run CI pipeline", depends: [@fmt, @lint, @test, @build])]
@ci () -> void = run(
    print(msg: "All CI checks passed"),
)

#[task(desc: "Clean build artifacts")]
@clean () -> void uses FileSystem = run(
    fs.remove_dir(path: "./target", recursive: true),
)

#[task(desc: "Watch for changes and rebuild")]
@watch () -> void uses FileSystem, Async = run(
    fs.watch(
        paths: ["./src/**/*.ori"],
        on_change: () -> run(
            print(msg: "Change detected, rebuilding..."),
            build(release: false),
        ),
    ),
)

#[task(desc: "Run the dev server")]
@dev (port: int = 3000) -> void uses FileSystem, Http, Async = run(
    parallel(
        tasks: [
            () -> watch(),
            () -> serve(port: port),
        ],
    ),
)
```

### Task Attributes

```ori
#[task]                                    // Basic task
#[task(desc: "Description")]               // With description
#[task(depends: [@build, @test])]          // With dependencies
#[task(alias: "b")]                        // Short alias
#[task(group: "build")]                    // Group for listing
#[task(private: true)]                     // Hidden from `ori tasks`
#[task(incremental: true)]                 // Only run if sources changed
#[task(sources: ["src/**/*.ori"])]         // Files to watch for changes
#[task(generates: ["target/app"])]         // Output files
```

### Task Parameters with Types

Unlike shell scripts, task parameters are typed:

```ori
#[task(desc: "Deploy to environment")]
@deploy (
    env: Environment,      // Typed enum, not stringly-typed
    version: str,
    dry_run: bool = false,
) -> Result<void, Error> uses Http, FileSystem = run(
    if dry_run then (
        print(msg: f"Would deploy {version} to {env}"),
    ) else (
        let url = match(env,
            Environment.Production -> "https://prod.example.com",
            Environment.Staging -> "https://staging.example.com",
        ),
        http.post(url: f"{url}/deploy", body: { version }),
    ),
)

type Environment = Production | Staging
```

Invocation:
```bash
ori deploy env:staging version:"1.2.3"
ori deploy env:production version:"1.2.3" dry_run:true
```

### Cross-Platform File Operations

No more `rimraf`, `mkdirp`, `cross-env`. Ori provides cross-platform primitives:

```ori
#[task]
@setup () -> void uses FileSystem = run(
    // Works on Windows, Mac, Linux
    fs.create_dir(path: "./build", recursive: true),
    fs.copy(from: "./config.template", to: "./config.local"),
    fs.remove(path: "./temp", recursive: true),
)

#[task]
@with_env () -> void uses Env, FileSystem = run(
    // Environment variables work cross-platform
    with Env = Env { vars: { "DEBUG": "true", "PORT": "3000" } } in (
        shell(cmd: "ori run"),
    ),
)
```

### The `shell` Function

For commands that must be shell commands:

```ori
@shell (
    cmd: str,
    cwd: str = ".",
    env: {str: str} = {},
    ignore_errors: bool = false,
    silent: bool = false,
) -> Result<ShellOutput, ShellError> uses FileSystem

type ShellOutput = { stdout: str, stderr: str, exit_code: int }
```

**Cross-Platform Shell Detection:**
- On Unix: Uses `sh -c` by default, respects `$SHELL`
- On Windows: Uses `cmd /c` or PowerShell based on context
- Explicit override: `shell(cmd: "...", shell: Shell.PowerShell)`

### Parallel Tasks

Leverage Ori's `parallel` pattern:

```ori
#[task(desc: "Run all checks in parallel")]
@check () -> void uses FileSystem = run(
    let results = parallel(
        tasks: [
            () -> lint(),
            () -> fmt(check: true),
            () -> test(),
        ],
        max_concurrent: num_cpus(),
    ),
    // All tasks must succeed
    for result in results do (
        result?,
    ),
)
```

### Incremental Tasks

Only re-run if sources changed:

```ori
#[task(
    desc: "Compile assets",
    incremental: true,
    sources: ["assets/**/*"],
    generates: ["dist/assets/**/*"],
)]
@compile_assets () -> void uses FileSystem = run(
    shell(cmd: "ori compile-assets"),
)
```

The runtime tracks file modification times and checksums. If sources haven't changed since generates were created, the task is skipped.

### Task Dependencies and DAG

```ori
#[task(depends: [@clean])]
@build () -> void = ...

#[task(depends: [@build])]
@test () -> void = ...

#[task(depends: [@lint, @test, @build])]
@ci () -> void = ...
```

When you run `ori ci`:
1. Build dependency graph
2. Topologically sort
3. Execute in parallel where possible

```
clean ──→ build ──→ test ──┐
                           ├──→ ci
lint ─────────────────────┘
```

`clean` and `lint` run in parallel. `build` waits for `clean`. `test` waits for `build`. `ci` waits for all.

### Task Groups

Organize tasks for discoverability:

```ori
#[task(group: "development")]
@dev () -> void = ...

#[task(group: "development")]
@watch () -> void = ...

#[task(group: "ci")]
@lint () -> void = ...

#[task(group: "ci")]
@test () -> void = ...

#[task(group: "release")]
@deploy () -> void = ...
```

```bash
$ ori tasks

Development:
  dev    Run the dev server
  watch  Watch for changes and rebuild

CI:
  lint   Lint code
  test   Run all tests

Release:
  deploy Deploy to environment
```

### Task Aliases

```ori
#[task(alias: "t")]
@test () -> void = ...

#[task(alias: "b")]
@build () -> void = ...
```

```bash
ori t           # same as: ori test
ori b release:true  # same as: ori build release:true
```

### Private Tasks

Tasks that are only used as dependencies:

```ori
#[task(private: true)]
@_setup_env () -> void = ...

#[task(depends: [@_setup_env])]
@build () -> void = ...
```

Private tasks don't appear in `ori tasks` listing but can be called explicitly if needed.

### Workspace Support

For monorepos:

```ori
// workspace.ori at root
$workspace = Workspace {
    members: [
        "packages/*",
        "apps/*",
        "!packages/deprecated-*",  // Exclusion pattern
    ],
}

// Shared version catalog (like pnpm catalogs)
$catalog = {
    "json": "1.2.3",
    "http": "2.0.0",
}

#[task(desc: "Build all packages")]
@build_all () -> void uses FileSystem = run(
    for member in workspace.members do (
        print(msg: f"Building {member}..."),
        shell(cmd: "ori build", cwd: member),
    ),
)

#[task(desc: "Test changed packages only")]
@test_changed () -> void uses FileSystem = run(
    let changed = git.changed_files(base: "main"),
    let affected = workspace.affected_by(files: changed),
    for pkg in affected do (
        shell(cmd: "ori test", cwd: pkg),
    ),
)
```

### Pre/Post Hooks

Like npm's `pre` and `post` scripts, but explicit:

```ori
#[task]
@build () -> void = ...

// Runs before build
#[before(@build)]
@_typecheck () -> void = run(
    shell(cmd: "ori check"),
)

// Runs after build
#[after(@build)]
@_notify () -> void uses Http = run(
    http.post(url: "https://hooks.slack.com/...", body: { text: "Build complete" }),
)
```

### Interactive Prompts

For tasks that need user input:

```ori
#[task(desc: "Create a new release")]
@release () -> Result<void, Error> uses FileSystem, Print = run(
    let version = prompt(
        message: "Enter version number:",
        validate: is_valid_semver,
    )?,

    let confirm = confirm(
        message: f"Release v{version}?",
        default: false,
    )?,

    if confirm then (
        shell(cmd: f"git tag v{version}"),
        shell(cmd: "git push --tags"),
        Ok(())
    ) else (
        Err(Error { message: "Release cancelled" })
    ),
)
```

### Conditional Execution

```ori
#[task]
@deploy () -> void uses FileSystem, Env = run(
    let env = env.get(name: "DEPLOY_ENV").unwrap_or(default: "staging"),

    match(env,
        "production" -> run(
            // Production-specific steps
            shell(cmd: "ori build release:true"),
            shell(cmd: "docker push prod-registry/app"),
        ),
        _ -> run(
            // Non-production
            shell(cmd: "ori build"),
            shell(cmd: "docker push staging-registry/app"),
        ),
    ),
)
```

### Platform-Specific Tasks

```ori
#[task(platforms: [Platform.Linux, Platform.MacOS])]
@install_unix () -> void uses FileSystem = run(
    shell(cmd: "brew install tool || apt-get install tool"),
)

#[task(platforms: [Platform.Windows])]
@install_windows () -> void uses FileSystem = run(
    shell(cmd: "choco install tool", shell: Shell.PowerShell),
)

#[task]
@install () -> void uses FileSystem = run(
    match(platform(),
        Platform.Windows -> install_windows(),
        _ -> install_unix(),
    ),
)
```

---

## CLI Interface

### Running Tasks

```bash
ori <task-name> [args...]

# Examples
ori build
ori build release:true
ori test filter:"unit"
ori deploy env:staging version:"1.2.3"
```

### Listing Tasks

```bash
ori tasks                  # List all tasks
ori tasks --all            # Include private tasks
ori tasks --group=ci       # Filter by group
```

### Task Info

```bash
ori task build             # Show task details, parameters, dependencies
```

### Running Multiple Tasks

```bash
ori lint test build        # Run in sequence
ori -p lint test build     # Run in parallel
```

### Dry Run

```bash
ori --dry-run ci           # Show what would run without executing
```

### Watch Mode

```bash
ori --watch test           # Re-run test on file changes
```

---

## Advanced Features

### Task Caching

Like Turborepo, cache task outputs:

```ori
#[task(
    cache: true,
    inputs: ["src/**/*.ori", "project.ori"],
    outputs: ["target/release/app"],
)]
@build () -> void uses FileSystem = run(
    shell(cmd: "ori compile --release"),
)
```

Cache key = hash of inputs. If cache hit, restore outputs instead of running.

### Remote Cache

For CI:

```ori
$cache = Cache {
    provider: CacheProvider.S3 {
        bucket: "my-build-cache",
        region: "us-east-1",
    },
}
```

### Task Timeouts

```ori
#[task(timeout: 5m)]  // 5 minute timeout
@long_running_test () -> void = ...
```

### Retry Logic

```ori
#[task(retry: 3, retry_delay: 10s)]
@flaky_integration_test () -> void = ...
```

### Matrix Execution

```ori
#[task(
    matrix: {
        os: [Platform.Linux, Platform.MacOS, Platform.Windows],
        features: ["default", "full", "minimal"],
    },
)]
@test_matrix (os: Platform, features: str) -> void = run(
    print(msg: f"Testing on {os} with features: {features}"),
    shell(cmd: f"ori test --features {features}"),
)
```

---

## Comparison with Other Systems

| Feature | npm scripts | Just | Taskfile | cargo-make | **Ori** |
|---------|-------------|------|----------|------------|---------|
| Cross-platform | Manual (`rimraf`) | Shell config | Built-in | Conditional | **Native** |
| Type-safe params | No | No | No | No | **Yes** |
| Parallel exec | Manual | Recipe-level | Task-level | Task-level | **Native** |
| Incremental | No | No | Yes (sources) | Yes | **Yes** |
| Dependencies | Manual | Yes | Yes | Yes | **Yes** |
| Testable tasks | No | No | No | No | **Yes** |
| Capabilities | No | No | No | No | **Yes** |
| IDE support | Limited | Plugin | Plugin | Plugin | **Native** |
| Speed | 176ms startup | ~10ms | ~20ms | ~30ms | **<10ms** |

---

## Migration Path

### From npm scripts

```json
{
  "scripts": {
    "build": "tsc && node build.js",
    "test": "jest",
    "lint": "eslint src/"
  }
}
```

Becomes:

```ori
#[task]
@build () -> void uses FileSystem = run(
    shell(cmd: "tsc"),
    shell(cmd: "node build.js"),
)

#[task]
@test () -> void uses FileSystem = run(
    shell(cmd: "jest"),
)

#[task]
@lint () -> void uses FileSystem = run(
    shell(cmd: "eslint src/"),
)
```

### From Justfile

```just
build:
    cargo build --release

test: build
    cargo test

deploy env="staging": test
    ./deploy.sh {{env}}
```

Becomes:

```ori
#[task]
@build () -> void uses FileSystem = run(
    shell(cmd: "cargo build --release"),
)

#[task(depends: [@build])]
@test () -> void uses FileSystem = run(
    shell(cmd: "cargo test"),
)

#[task(depends: [@test])]
@deploy (env: str = "staging") -> void uses FileSystem = run(
    shell(cmd: f"./deploy.sh {env}"),
)
```

---

## Implementation Phases

### Phase 1: Core Task System
- `project.ori` parsing
- `#[task]` attribute
- Basic task execution with `shell()`
- Task listing (`ori tasks`)

### Phase 2: Dependencies and Parallelism
- `depends` attribute
- DAG construction and topological sort
- Parallel execution of independent tasks

### Phase 3: Incremental Execution
- `sources` and `generates` tracking
- File modification time and checksum tracking
- Skip-if-unchanged logic

### Phase 4: Advanced Features
- Task caching
- Workspace support
- Matrix execution
- Remote cache

---

## Rationale

### Why Not YAML/TOML/Custom DSL?

**1. No Context Switching**
Developers already know Ori syntax. No new grammar to learn.

**2. Full Language Power**
Conditionals, loops, functions, types — all available. No artificial limitations.

**3. Tooling Works**
Syntax highlighting, autocomplete, type checking, refactoring — all work immediately.

**4. Testable**
Tasks are functions. Functions can have tests. Tasks can have tests.

**5. Type Safety**
`env: Environment` catches typos at compile time, not deploy time.

### Why `shell()` Instead of Direct Commands?

Explicit is better than implicit. `shell(cmd: "npm install")` makes clear:
- This runs in a shell
- This uses the FileSystem capability
- This can fail

### Why Capabilities on Tasks?

**Security**: A build task shouldn't need Http access.
**Mockability**: Test tasks with injected capabilities.
**Documentation**: Capabilities are self-documenting side effects.

---

## Future Extensions

### Task Generators (Macros)

```ori
// Generate test tasks for each module
#[generate_tasks]
$test_tasks = for module in ["core", "parser", "runtime"] yield (
    Task {
        name: f"test_{module}",
        cmd: f"ori test --filter {module}",
    }
)
```

### Remote Task Execution

```ori
#[task(remote: "build-server.example.com")]
@heavy_build () -> void = ...
```

### Task Metrics

```ori
#[task(metrics: true)]
@build () -> void = ...

// Later: ori metrics build
// Shows: avg duration, success rate, cache hit rate
```

---

## References

- [Just](https://github.com/casey/just) — Command runner with readable syntax
- [Taskfile](https://taskfile.dev/) — YAML-based task runner with go templates
- [Bun](https://bun.sh/) — Fast JavaScript runtime with 7ms script startup
- [cargo-make](https://github.com/sagiegurari/cargo-make) — Rust task runner with TOML config
- [Turborepo](https://turbo.build/) — Monorepo build system with remote caching
- [pnpm](https://pnpm.io/) — Fast package manager with workspace catalogs

---

## Open Questions

1. **File name**: `project.ori` vs `Orifile` vs `ori.toml`?
   - Proposal: `project.ori` — consistent with language, not Yet Another File Format

2. **Task vs Script naming**: Should we call them "tasks" or "scripts"?
   - Proposal: "Tasks" — scripts implies shell-only, tasks are more general

3. **Global tasks**: Should there be a `~/.ori/tasks.ori` for user-level tasks?
   - Proposal: Yes, for things like `ori new`, `ori upgrade`

4. **Shebang support**: Allow tasks in other languages via shebang?
   - Proposal: Not initially — keep focus on Ori-native experience. Can add later.

---

## Conclusion

Ori has the unique opportunity to build a task runner that is:
- **Native**: Written in Ori, not a separate DSL
- **Type-safe**: Catch parameter errors at compile time
- **Capability-aware**: Declare and control side effects
- **Testable**: Tasks are functions, functions have tests
- **Fast**: Sub-10ms startup, parallel execution
- **Cross-platform**: No shell-specific hacks

This isn't just "npm scripts but for Ori." It's a fundamentally better approach to project automation that only Ori can offer.
