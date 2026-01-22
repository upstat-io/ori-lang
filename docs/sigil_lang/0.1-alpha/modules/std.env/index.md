# std.env

Environment variables and command-line arguments.

```sigil
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

```sigil
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

```sigil
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

```sigil
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

```sigil
@test_database_url tests @get_database_url () -> void =
    with Env = MockEnv {
        vars: {"DATABASE_URL": "postgres://test/testdb"},
        arguments: [],
        cwd: "/tmp",
    } in
    run(
        let url = get_database_url(),
        assert_eq(url, "postgres://test/testdb"),
    )

@test_database_url_default tests @get_database_url () -> void =
    with Env = MockEnv { vars: {}, arguments: [], cwd: "/tmp" } in
    run(
        let url = get_database_url(),
        assert_eq(url, "postgres://localhost/dev"),
    )
```

---

## Environment Variables

### @get_var

```sigil
@get_var (name: str) -> Option<str>
```

Gets an environment variable.

```sigil
use std.env { get_var }

let home = get_var("HOME") ?? "/tmp"
let debug = get_var("DEBUG").is_some()
```

---

### @set_var

```sigil
@set_var (name: str, value: str) -> void
```

Sets an environment variable for the current process.

```sigil
use std.env { set_var }

set_var("MY_CONFIG", "value")
```

---

### @remove_var

```sigil
@remove_var (name: str) -> void
```

Removes an environment variable.

---

### @vars

```sigil
@vars () -> {str: str}
```

Returns all environment variables.

```sigil
use std.env { vars }

let env = vars()
for (key, value) in env.entries() do
    print(key + "=" + value)
```

---

## Command-Line Arguments

### @args

```sigil
@args () -> [str]
```

Returns command-line arguments (including program name).

```sigil
use std.env { args }

let arguments = args()
// ["./myprogram", "--config", "app.json"]

let program = arguments[0]
let flags = arguments[1..]
```

---

### @args_os

```sigil
@args_os () -> [str]
```

Returns arguments without program name.

```sigil
use std.env { args_os }

let arguments = args_os()
// ["--config", "app.json"]
```

---

## Current Directory

### @current_dir

```sigil
@current_dir () -> Result<str, EnvError>
```

Returns current working directory.

```sigil
use std.env { current_dir }

let cwd = current_dir()?
print("Working in: " + cwd)
```

---

### @set_current_dir

```sigil
@set_current_dir (path: str) -> Result<void, EnvError>
```

Changes current working directory.

---

## Types

### EnvError

```sigil
type EnvError =
    | NotFound(name: str)
    | InvalidPath(path: str)
    | PermissionDenied
```

---

## Examples

### Config from environment

```sigil
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

```sigil
use std.env { args_os }

@parse_args () uses Env -> Result<Options, str> = run(
    let args = args_os(),
    let options = Options.default(),

    for arg in args do match(arg,
        "--help" -> return Ok(options.with_help(true)),
        "--verbose" -> options = options.with_verbose(true),
        s if s.starts_with("--") -> return Err("Unknown flag: " + s),
        s -> options = options.with_file(s),
    ),

    Ok(options),
)
```

---

## See Also

- [std.process](../std.process/) — Process management
- [Capabilities](../../spec/14-capabilities.md)
