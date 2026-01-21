# std.io

Input/output traits and operations.

```sigil
use std.io { Reader, Writer, read_line, stdin, stdout }
```

**Capability required:** `IO`

---

## Overview

The `std.io` module provides:

- Core I/O traits (`Reader`, `Writer`)
- Standard streams (`stdin`, `stdout`, `stderr`)
- Buffered I/O utilities
- Stream composition

---

## Traits

### Reader

```sigil
trait Reader {
    @read (self, buffer: [byte]) -> Result<int, IoError>
    @read_all (self) -> Result<[byte], IoError>
    @read_to_string (self) -> Result<str, IoError>
}
```

Reads bytes from a source.

---

### Writer

```sigil
trait Writer {
    @write (self, data: [byte]) -> Result<int, IoError>
    @write_str (self, s: str) -> Result<int, IoError>
    @flush (self) -> Result<void, IoError>
}
```

Writes bytes to a destination.

---

### BufReader

```sigil
trait BufReader: Reader {
    @read_line (self) -> Result<Option<str>, IoError>
    @lines (self) -> LineIterator
}
```

Buffered reading with line support.

---

## Types

### IoError

```sigil
type IoError =
    | UnexpectedEof
    | BrokenPipe
    | InvalidData(str)
    | Other(str)
```

---

## Standard Streams

### stdin

```sigil
@stdin () -> impl BufReader
```

Standard input stream.

```sigil
use std.io { stdin }

let line = stdin().read_line()?
```

---

### stdout

```sigil
@stdout () -> impl Writer
```

Standard output stream.

```sigil
use std.io { stdout }

stdout().write_str("Hello\n")?
stdout().flush()?
```

---

### stderr

```sigil
@stderr () -> impl Writer
```

Standard error stream.

```sigil
use std.io { stderr }

stderr().write_str("Error: something went wrong\n")?
```

---

## Functions

### @read_line

```sigil
@read_line () -> Result<str, IoError>
```

Reads a line from stdin. Convenience function.

```sigil
use std.io { read_line }

let name = read_line()?
print("Hello, " + name)
```

---

### @copy

```sigil
@copy (reader: impl Reader, writer: impl Writer) -> Result<int, IoError>
```

Copies all bytes from reader to writer. Returns bytes copied.

```sigil
use std.io { copy }
use std.fs { open_read, create }

let src = open_read("input.txt")?
let dst = create("output.txt")?
let bytes = copy(src, dst)?
```

---

## Buffering

### @buffered

```sigil
@buffered<R: Reader> (reader: R) -> BufReader<R>
@buffered<W: Writer> (writer: W) -> BufWriter<W>
```

Wraps a reader or writer with buffering.

```sigil
use std.io { buffered }
use std.fs { open_read }

let file = open_read("large.txt")?
let reader = buffered(file)

for line in reader.lines() do
    process(line)
```

---

## Examples

### Reading stdin line by line

```sigil
use std.io { stdin }

@process_input () uses IO -> Result<void, IoError> = run(
    for line in stdin().lines() do
        print("Got: " + line),
    Ok(()),
)
```

### Writing to stdout with buffering

```sigil
use std.io { stdout, buffered }

@write_output (items: [str]) uses IO -> Result<void, IoError> = run(
    let out = buffered(stdout()),
    for item in items do out.write_str(item + "\n")?,
    out.flush(),
)
```

---

## See Also

- [std.fs](../std.fs/) — File I/O
- [std.net](../std.net/) — Network I/O
- [Capabilities](../../spec/14-capabilities.md)
