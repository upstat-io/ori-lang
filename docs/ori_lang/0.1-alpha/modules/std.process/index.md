# std.process

Process spawning and management.

```ori
use std.process { Command, spawn, exit }
```

**Capability required:** `Process`

---

## Overview

The `std.process` module provides:

- Spawning child processes
- Capturing output
- Process control (exit, signals)

---

## Types

### Command

```ori
type Command = {
    program: str,
    args: [str],
    env: {str: str},
    cwd: Option<str>,
    stdin: Stdio,
    stdout: Stdio,
    stderr: Stdio,
}

type Stdio = Inherit | Piped | Null
```

Builder for spawning processes.

```ori
use std.process { Command }

let cmd = Command.new("ls")
    .arg("-la")
    .arg("/tmp")
    .env("LANG", "C")
    .cwd("/home/user")

let output = cmd.output()?
```

**Methods:**
- `new(program: str) -> Command` — Create command
- `arg(arg: str) -> Command` — Add argument
- `args(args: [str]) -> Command` — Add multiple arguments
- `env(key: str, value: str) -> Command` — Set env var
- `cwd(dir: str) -> Command` — Set working directory
- `stdin(stdio: Stdio) -> Command` — Configure stdin
- `stdout(stdio: Stdio) -> Command` — Configure stdout
- `stderr(stdio: Stdio) -> Command` — Configure stderr
- `output() -> Result<Output, ProcessError>` — Run and capture
- `status() -> Result<int, ProcessError>` — Run and get exit code
- `spawn() -> Result<Child, ProcessError>` — Start process

---

### Output

```ori
type Output = {
    status: int,
    stdout: str,
    stderr: str,
}
```

Captured output from a completed process.

---

### Child

```ori
type Child
```

A handle to a running child process.

**Methods:**
- `wait() -> Result<int, ProcessError>` — Wait for exit
- `kill() -> Result<void, ProcessError>` — Terminate process
- `id() -> int` — Process ID

---

### ProcessError

```ori
type ProcessError =
    | NotFound(program: str)
    | PermissionDenied
    | IoError(str)
```

---

## Functions

### @spawn

```ori
@spawn (program: str, args: [str]) -> Result<Child, ProcessError>
```

Spawns a process.

```ori
use std.process { spawn }

let child = spawn("sleep", ["10"])?
// ... do other work ...
let status = child.wait()?
```

---

### @run

```ori
@run (program: str, args: [str]) -> Result<Output, ProcessError>
```

Runs a command and captures output.

```ori
use std.process { run }

let output = run("git", ["status", "--short"])?
print(output.stdout)
```

---

### @exit

```ori
@exit (code: int) -> Never
```

Exits the current process.

```ori
use std.process { exit }

if fatal_error then exit(1)
```

---

### @id

```ori
@id () -> int
```

Returns current process ID.

---

## Examples

### Running shell commands

```ori
use std.process { Command }

@shell (cmd: str) uses Process -> Result<str, ProcessError> = run(
    let output = Command.new("sh")
        .arg("-c")
        .arg(cmd)
        .output()?,
    if output.status == 0 then Ok(output.stdout)
    else Err(ProcessError.IoError("Command failed: " + output.stderr)),
)
```

### Pipeline

```ori
use std.process { Command, Stdio }

@git_log_oneline () uses Process -> Result<str, ProcessError> = run(
    let git = Command.new("git")
        .args(["log", "--oneline", "-10"])
        .stdout(Stdio.Piped)
        .output()?,
    Ok(git.stdout),
)
```

### Background process

```ori
use std.process { Command }

@start_server () uses Process -> Result<Child, ProcessError> =
    Command.new("./server")
        .arg("--port")
        .arg("8080")
        .spawn()

@main () uses Process -> Result<void, Error> = run(
    let server = start_server()?,
    print("Server started with PID: " + str(server.id())),
    // ... do work ...
    server.kill()?,
    Ok(()),
)
```

---

## See Also

- [std.env](../std.env/) — Environment variables
- [Capabilities](../../spec/14-capabilities.md)
