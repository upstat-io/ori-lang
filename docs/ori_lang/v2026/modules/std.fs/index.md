# std.fs

Filesystem operations.

```ori
use std.fs { read_file, write_file, Path, exists }
```

**Capability required:** `FileSystem`

---

## Overview

The `std.fs` module provides:

- File reading and writing
- Path manipulation
- Directory operations
- File metadata

---

## The FileSystem Capability

```ori
trait FileSystem {
    @read (path: str) -> Result<str, FileError>
    @read_bytes (path: str) -> Result<[byte], FileError>
    @write (path: str, content: str) -> Result<void, FileError>
    @write_bytes (path: str, data: [byte]) -> Result<void, FileError>
    @exists (path: str) -> bool
    @delete (path: str) -> Result<void, FileError>
    @list (dir: str) -> Result<[DirEntry], FileError>
    @metadata (path: str) -> Result<Metadata, FileError>
}
```

The `FileSystem` capability represents the ability to perform file system operations. Functions that read, write, or query files must declare `uses FileSystem` in their signature.

```ori
@load_config (path: str) -> Result<Config, Error> uses FileSystem =
    FileSystem.read(path)?.parse()
```

**Implementations:**

| Type | Description |
|------|-------------|
| `LocalFileSystem` | Real file system (default) |
| `MockFileSystem` | In-memory mock for testing |

### MockFileSystem

For testing, create an in-memory mock:

```ori
type MockFileSystem = {
    files: {str: str},
}

impl FileSystem for MockFileSystem {
    @read (path: str) -> Result<str, FileError> =
        match self.files.get(path) {
            Some(content) -> Ok(content)
            None -> Err(FileError.NotFound(path))
        }

    @write (path: str, content: str) -> Result<void, FileError> = {
        self.files = self.files.insert(path, content)
        Ok(())
    }

    @exists (path: str) -> bool = self.files.contains_key(path)

    // ... other methods
}
```

```ori
@test_load_config tests @load_config () -> void =
    with FileSystem = MockFileSystem {
        files: {"/config.json": "{\"debug\": true}"}
    } in
    {
        let config = load_config("/config.json")?
        assert(config.debug)
    }
```

---

## Types

### Path

```ori
type Path = {
    raw: str,
}
```

A filesystem path. Provides cross-platform path handling.

```ori
let p = Path.from("src/main.ori")
p.parent()      // Path("src")
p.file_name()   // "main.ori"
p.extension()   // "si"
p.join("lib")   // Path("src/main.ori/lib")
```

**Methods:**
- `parent() -> Option<Path>` — Parent directory
- `file_name() -> Option<str>` — Final component
- `extension() -> Option<str>` — File extension
- `stem() -> Option<str>` — File name without extension
- `join(other: str) -> Path` — Append path component
- `is_absolute() -> bool` — Starts from root
- `is_relative() -> bool` — Relative path
- `exists() -> bool` — Path exists (requires capability)
- `to_str() -> str` — Convert to string

---

### FileError

```ori
type FileError =
    | NotFound(path: str)
    | PermissionDenied(path: str)
    | IsDirectory(path: str)
    | NotDirectory(path: str)
    | AlreadyExists(path: str)
    | IoError(str)
```

---

### Metadata

```ori
type Metadata = {
    size: Size,
    is_file: bool,
    is_dir: bool,
    is_symlink: bool,
    modified: DateTime,
    created: Option<DateTime>,
}
```

---

## File Operations

### @read_file

```ori
@read_file (path: str) -> Result<str, FileError>
```

Reads entire file as UTF-8 string.

```ori
use std.fs { read_file }

let content = read_file("config.json")?
```

---

### @read_bytes

```ori
@read_bytes (path: str) -> Result<[byte], FileError>
```

Reads entire file as bytes.

```ori
use std.fs { read_bytes }

let data = read_bytes("image.png")?
```

---

### @write_file

```ori
@write_file (path: str, content: str) -> Result<void, FileError>
```

Writes string to file, creating or overwriting.

```ori
use std.fs { write_file }

write_file("output.txt", "Hello, world!")?
```

---

### @write_bytes

```ori
@write_bytes (path: str, data: [byte]) -> Result<void, FileError>
```

Writes bytes to file.

```ori
use std.fs { write_bytes }

write_bytes("data.bin", bytes)?
```

---

### @append_file

```ori
@append_file (path: str, content: str) -> Result<void, FileError>
```

Appends string to file.

```ori
use std.fs { append_file }

append_file("log.txt", timestamp + " Event occurred\n")?
```

---

## File Handles

### @open_read

```ori
@open_read (path: str) -> Result<File, FileError>
```

Opens file for reading. Returns a `Reader`.

```ori
use std.fs { open_read }

let file = open_read("data.txt")?
let content = file.read_to_string()?
```

---

### @create

```ori
@create (path: str) -> Result<File, FileError>
```

Creates file for writing. Truncates if exists.

```ori
use std.fs { create }

let file = create("output.txt")?
file.write_str("content")?
```

---

### @open_append

```ori
@open_append (path: str) -> Result<File, FileError>
```

Opens file for appending.

---

## Directory Operations

### @read_dir

```ori
@read_dir (path: str) -> Result<[DirEntry], FileError>
```

Lists directory contents.

```ori
use std.fs { read_dir }

let entries = read_dir("src")?
for entry in entries do
    print(entry.name)
```

---

### @create_dir

```ori
@create_dir (path: str) -> Result<void, FileError>
```

Creates a directory.

---

### @create_dir_all

```ori
@create_dir_all (path: str) -> Result<void, FileError>
```

Creates directory and all parent directories.

```ori
use std.fs { create_dir_all }

create_dir_all("data/cache/images")?
```

---

### @remove_file

```ori
@remove_file (path: str) -> Result<void, FileError>
```

Deletes a file.

---

### @remove_dir

```ori
@remove_dir (path: str) -> Result<void, FileError>
```

Deletes an empty directory.

---

### @remove_dir_all

```ori
@remove_dir_all (path: str) -> Result<void, FileError>
```

Deletes directory and all contents. **Use with caution.**

---

## Path Queries

### @exists

```ori
@exists (path: str) -> bool
```

Returns true if path exists.

```ori
use std.fs { exists }

if exists("config.json") then load_config()
else use_defaults()
```

---

### @is_file

```ori
@is_file (path: str) -> bool
```

Returns true if path is a file.

---

### @is_dir

```ori
@is_dir (path: str) -> bool
```

Returns true if path is a directory.

---

### @metadata

```ori
@metadata (path: str) -> Result<Metadata, FileError>
```

Gets file metadata.

```ori
use std.fs { metadata }

let meta = metadata("data.txt")?
print("Size: " + str(meta.orize))
print("Modified: " + str(meta.modified))
```

---

## Examples

### Reading and processing a file

```ori
use std.fs { read_file }
use std.json { parse }

@load_config (path: str) uses FileSystem -> Result<Config, Error> = try {
    let content = read_file(path)?
    let config = parse<Config>(content)?
    Ok(config)
}
```

### Walking a directory tree

```ori
use std.fs { read_dir, is_dir, Path }

@walk (dir: str, action: str -> void) uses FileSystem -> Result<void, FileError> = {
    for entry in read_dir(dir)? do
        let path = Path.from(dir).join(entry.name).to_str()
        if is_dir(path) then walk(path, action)?
        else action(path)

    Ok(())
}
```

---

## See Also

- [std.io](../std.io/) — I/O traits
- [Capabilities](../../spec/14-capabilities.md)
