# std.env

Environment variables and command-line arguments.

```ori
use std.env { get_var, set_var, args }
```

**Capability required:** `Env`

---

## Overview

The `std.env` module provides:

- Environment variable access
- Command-line argument parsing
- Current directory operations

---

## The Env Capability

```ori
trait Env {
    @get (name: str) -> Option<str>
    @set (name: str, value: str) -> void
    @remove (name: str) -> void
    @all () -> {str: str}
    @args () -> [str]
    @current_dir () -> Result<str, EnvError>
}
```

The `Env` capability represents access to environment variables and process information. Functions that read or modify the environment must declare `uses Env` in their signature.

```ori
@get_database_url () -> str uses Env =
    Env.get("DATABASE_URL") ?? "postgres://localhost/dev"
```

**Implementations:**

| Type | Description |
|------|-------------|
| `SystemEnv` | Real system environment (default) |
| `MockEnv` | Configurable mock for testing |

### MockEnv

For testing environment-dependent code:

```ori
type MockEnv = {
    vars: {str: str},
    arguments: [str],
    cwd: str,
}

impl Env for MockEnv {
    @get (name: str) -> Option<str> = self.vars.get(name)
    @set (name: str, value: str) -> void = self.vars = self.vars.insert(name, value)
    @remove (name: str) -> void = self.vars = self.vars.remove(name)
    @all () -> {str: str} = self.vars
    @args () -> [str] = self.arguments
    @current_dir () -> Result<str, EnvError> = Ok(self.cwd)
}
```

```ori
@test_database_url tests @get_database_url () -> void =
    with Env = MockEnv {
        vars: {"DATABASE_URL": "postgres://test/testdb"},
        arguments: [],
        cwd: "/tmp",
    } in
    {
        let url = get_database_url()
        assert_eq(
            .actual: url,
            .expected: "postgres://test/testdb",
        )
    }

@test_database_url_default tests @get_database_url () -> void =
    with Env = MockEnv { vars: {}, arguments: [], cwd: "/tmp" } in
    {
        let url = get_database_url()
        assert_eq(
            .actual: url,
            .expected: "postgres://localhost/dev",
        )
    }
```

---

## Environment Variables

### @get_var

```ori
@get_var (name: str) -> Option<str>
```

Gets an environment variable.

```ori
use std.env { get_var }

let home = get_var("HOME") ?? "/tmp"
let debug = get_var("DEBUG").is_some()
```

---

### @set_var

```ori
@set_var (name: str, value: str) -> void
```

Sets an environment variable for the current process.

```ori
use std.env { set_var }

set_var("MY_CONFIG", "value")
```

---

### @remove_var

```ori
@remove_var (name: str) -> void
```

Removes an environment variable.

---

### @vars

```ori
@vars () -> {str: str}
```

Returns all environment variables.

```ori
use std.env { vars }

let env = vars()
for (key, value) in env.entries() do
    print(key + "=" + value)
```

---

## Command-Line Arguments

### @args

```ori
@args () -> [str]
```

Returns command-line arguments (including program name).

```ori
use std.env { args }

let arguments = args()
// ["./myprogram", "--config", "app.json"]

let program = arguments[0]
let flags = arguments[1..]
```

---

### @args_os

```ori
@args_os () -> [str]
```

Returns arguments without program name.

```ori
use std.env { args_os }

let arguments = args_os()
// ["--config", "app.json"]
```

---

## Current Directory

### @current_dir

```ori
@current_dir () -> Result<str, EnvError>
```

Returns current working directory.

```ori
use std.env { current_dir }

let cwd = current_dir()?
print("Working in: " + cwd)
```

---

### @set_current_dir

```ori
@set_current_dir (path: str) -> Result<void, EnvError>
```

Changes current working directory.

---

## Types

### EnvError

```ori
type EnvError =
    | NotFound(name: str)
    | InvalidPath(path: str)
    | PermissionDenied
```

---

## Examples

### Config from environment

```ori
use std.env { get_var }

type Config = {
    host: str,
    port: int,
    debug: bool,
}

@config_from_env () uses Env -> Config = Config {
    host: get_var("HOST") ?? "localhost",
    port: get_var("PORT").and_then(parse_int).unwrap_or(8080),
    debug: get_var("DEBUG").map(v -> v == "true") ?? false,
}
```

### Simple argument parsing

```ori
use std.env { args_os }

@parse_args () uses Env -> Result<Options, str> = {
    let args = args_os()

    // Fold through args, accumulating options or error
    args.fold(
        initial: Ok(Options.default()),
        f: (acc, arg) -> try {
            let options = acc?
            match arg {
                "--help" -> Ok(options.with_help(true))
                "--verbose" -> Ok(options.with_verbose(true))
                s if s.starts_with("--") -> Err("Unknown flag: " + s)
                s -> Ok(options.with_file(s))
            }
        },
    )
}
```

---

## See Also

- [std.process](../std.process/) — Process management
- [Capabilities](../../spec/14-capabilities.md)
