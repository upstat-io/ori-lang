# Proposal: std.fs API Design

**Status:** Approved
**Approved:** 2026-01-30
**Author:** Eric (with AI assistance)
**Created:** 2026-01-29
**Affects:** Standard library
**Depends on:** `std.time` (for `Instant` type)

---

## Summary

This proposal defines the API for `std.fs`, providing file system operations including reading, writing, directory manipulation, and file metadata.

---

## Motivation

File system operations are fundamental for:
- Configuration file handling
- Data persistence
- Log file management
- Build tools and scripts
- Asset processing

The API should be:
1. **Safe** — Proper error handling, no silent failures
2. **Capability-tracked** — `uses FileSystem` for all operations
3. **Cross-platform** — Abstract over OS differences
4. **Efficient** — Streaming for large files

---

## Core Types

### Path

Represents file system paths:

```ori
type Path = { segments: [str], absolute: bool }

impl Path {
    @from_str (s: str) -> Path
    @join (self, other: Path) -> Path
    @join_str (self, s: str) -> Path
    @parent (self) -> Option<Path>
    @file_name (self) -> Option<str>
    @extension (self) -> Option<str>
    @with_extension (self, ext: str) -> Path
    @is_absolute (self) -> bool
    @to_str (self) -> str
    @relative_to (self, base: Path) -> Option<Path>  // Returns relative path from base to self
}
```

Usage:
```ori
use std.fs { Path }

let path = Path.from_str("/home/user/documents")
let file = path.join_str("report.txt")
// file.to_str() == "/home/user/documents/report.txt"

file.extension()  // Some("txt")
file.file_name()  // Some("report.txt")
file.parent()     // Some(Path "/home/user/documents")
```

### FileInfo

File metadata:

```ori
type FileInfo = {
    path: Path,
    size: int,
    is_file: bool,
    is_dir: bool,
    is_symlink: bool,
    modified: Instant,
    created: Option<Instant>,  // Not available on all platforms
    readonly: bool,
}
```

### FileError

```ori
type FileError = {
    kind: FileErrorKind,
    path: Path,
    message: str,
}

type FileErrorKind =
    | NotFound
    | PermissionDenied
    | AlreadyExists
    | NotAFile
    | NotADirectory
    | DirectoryNotEmpty
    | IoError
    | InvalidPath
```

---

## Reading Files

### Read Entire File

```ori
@read (path: str) -> Result<str, FileError> uses FileSystem
@read_bytes (path: str) -> Result<[byte], FileError> uses FileSystem
```

Usage:
```ori
use std.fs { read, read_bytes }

let content = read(path: "config.json")?
let binary = read_bytes(path: "image.png")?
```

### Read Lines

```ori
@read_lines (path: str) -> Result<[str], FileError> uses FileSystem
```

Usage:
```ori
use std.fs { read_lines }

let lines = read_lines(path: "data.txt")?
for line in lines do process(line)
```

### Streaming Read

For large files:

```ori
type FileReader = { ... }

@open_read (path: str) -> Result<FileReader, FileError> uses FileSystem

impl FileReader {
    @read_chunk (self, size: int) -> Result<([byte], FileReader), FileError> uses FileSystem
    @read_line (self) -> Result<(Option<str>, FileReader), FileError> uses FileSystem
    @close (self) -> void uses FileSystem
}

impl Iterable for FileReader {
    type Item = str
    @iter (self) -> FileLineIterator
}
```

Usage:
```ori
use std.fs { open_read }

let reader = open_read(path: "large_file.log")?
for line in reader do
    if line.contains("ERROR") then report(line)
```

---

## Writing Files

### Write Entire File

```ori
@write (path: str, content: str) -> Result<void, FileError> uses FileSystem
@write_bytes (path: str, content: [byte]) -> Result<void, FileError> uses FileSystem
```

Usage:
```ori
use std.fs { write }

write(path: "output.txt", content: result)?
```

### Write Options

```ori
type WriteMode =
    | Create    // Create new file, error if exists
    | Append    // Open for append, create if not exists
    | Truncate  // Create or overwrite, truncate if exists

@write_with (
    path: str,
    content: str,
    mode: WriteMode = Truncate,
    create_dirs: bool = false,
) -> Result<void, FileError> uses FileSystem
```

Usage:
```ori
use std.fs { write_with, WriteMode }

// Append to log
write_with(path: "app.log", content: log_line, mode: Append)?

// Create with parent directories
write_with(path: "deep/nested/file.txt", content: data, create_dirs: true)?
```

### Streaming Write

```ori
type FileWriter = { ... }

@open_write (path: str, mode: WriteMode = Truncate) -> Result<FileWriter, FileError> uses FileSystem

impl FileWriter {
    @write_chunk (self, data: [byte]) -> Result<FileWriter, FileError> uses FileSystem
    @write_str (self, s: str) -> Result<FileWriter, FileError> uses FileSystem
    @write_line (self, s: str) -> Result<FileWriter, FileError> uses FileSystem
    @flush (self) -> Result<FileWriter, FileError> uses FileSystem
    @close (self) -> Result<void, FileError> uses FileSystem
}
```

Usage:
```ori
use std.fs { open_write }

let writer = open_write(path: "output.csv")?
let writer = writer.write_line(s: "name,age,city")?
for row in data do
    writer = writer.write_line(s: format_row(row))?
writer.close()?
```

---

## Directory Operations

### Read Directory

```ori
@list_dir (path: str) -> Result<[str], FileError> uses FileSystem
@list_dir_info (path: str) -> Result<[FileInfo], FileError> uses FileSystem
```

Usage:
```ori
use std.fs { list_dir, list_dir_info }

let entries = list_dir(path: ".")?
let detailed = list_dir_info(path: ".")?
```

### Recursive Listing

```ori
@walk_dir (path: str) -> Result<[FileInfo], FileError> uses FileSystem
@walk_dir_with (
    path: str,
    max_depth: int = -1,  // -1 = unlimited
    follow_symlinks: bool = false,
) -> Result<[FileInfo], FileError> uses FileSystem
```

Usage:
```ori
use std.fs { walk_dir }

let all_files = walk_dir(path: "src")?
for info in all_files do
    if info.path.extension() == Some("ori") then process(info.path)
```

### Create/Remove Directories

```ori
@create_dir (path: str) -> Result<void, FileError> uses FileSystem
@create_dir_all (path: str) -> Result<void, FileError> uses FileSystem
@remove_dir (path: str) -> Result<void, FileError> uses FileSystem
@remove_dir_all (path: str) -> Result<void, FileError> uses FileSystem
```

Usage:
```ori
use std.fs { create_dir_all, remove_dir_all }

create_dir_all(path: "output/reports/2024")?
remove_dir_all(path: "temp")?  // Removes recursively
```

---

## File Operations

### Copy, Move, Remove

```ori
@copy (from: str, to: str) -> Result<void, FileError> uses FileSystem
@copy_with (
    from: str,
    to: str,
    overwrite: bool = false,
) -> Result<void, FileError> uses FileSystem

@move (from: str, to: str) -> Result<void, FileError> uses FileSystem
@rename (from: str, to: str) -> Result<void, FileError> uses FileSystem  // Alias for move

@remove (path: str) -> Result<void, FileError> uses FileSystem
```

Usage:
```ori
use std.fs { copy, move, remove }

copy(from: "template.txt", to: "output.txt")?
move(from: "temp.txt", to: "final.txt")?
remove(path: "old_file.txt")?
```

### File Info

```ori
@info (path: str) -> Result<FileInfo, FileError> uses FileSystem
@exists (path: str) -> bool uses FileSystem
@is_file (path: str) -> bool uses FileSystem
@is_dir (path: str) -> bool uses FileSystem
```

Usage:
```ori
use std.fs { info, exists, is_file }

if exists(path: "config.json") then
    let metadata = info(path: "config.json")?
    print(msg: `Size: {metadata.size} bytes`)
```

---

## Glob Patterns

### Pattern Matching

```ori
@glob (pattern: str) -> Result<[str], FileError> uses FileSystem
```

Usage:
```ori
use std.fs { glob }

let ori_files = glob(pattern: "src/**/*.ori")?
let configs = glob(pattern: "config/*.{json,toml}")?
```

Supported patterns:
- `*` — matches any characters except `/`
- `**` — matches any characters including `/` (recursive)
- `?` — matches single character
- `[abc]` — matches any of a, b, c
- `{a,b}` — matches a or b

---

## Temporary Files

```ori
@temp_dir () -> Path uses FileSystem
@create_temp_file (prefix: str = "tmp") -> Result<Path, FileError> uses FileSystem
@create_temp_dir (prefix: str = "tmp") -> Result<Path, FileError> uses FileSystem
```

Usage:
```ori
use std.fs { create_temp_file, create_temp_dir }

let temp = create_temp_file(prefix: "download")?
write(path: temp.to_str(), content: data)?
// Use temp file...
remove(path: temp.to_str())?
```

### Scoped Temp

Auto-cleanup with `with` pattern:

```ori
@with_temp_file<T> (prefix: str, action: (Path) -> T) -> Result<T, FileError> uses FileSystem
@with_temp_dir<T> (prefix: str, action: (Path) -> T) -> Result<T, FileError> uses FileSystem
```

Usage:
```ori
use std.fs { with_temp_file }

let result = with_temp_file(prefix: "work", action: temp -> {
    write(path: temp.to_str(), content: data)?
    process_file(temp)
    // temp file automatically deleted after this block
})?
```

---

## Permissions

### Read/Modify Permissions

```ori
type Permissions = { readable: bool, writable: bool, executable: bool }

@get_permissions (path: str) -> Result<Permissions, FileError> uses FileSystem
@set_permissions (path: str, permissions: Permissions) -> Result<void, FileError> uses FileSystem
@set_readonly (path: str, readonly: bool) -> Result<void, FileError> uses FileSystem
```

Usage:
```ori
use std.fs { set_readonly, get_permissions }

set_readonly(path: "important.dat", readonly: true)?

let perms = get_permissions(path: "script.sh")?
if !perms.executable then
    set_permissions(path: "script.sh", permissions: Permissions { ...perms, executable: true })?
```

---

## Path Utilities

### Current Directory

```ori
@cwd () -> Result<Path, FileError> uses FileSystem
@set_cwd (path: str) -> Result<void, FileError> uses FileSystem
```

### Path Resolution

```ori
@canonicalize (path: str) -> Result<Path, FileError> uses FileSystem
@resolve (path: str) -> Result<Path, FileError> uses FileSystem
@relative (from: str, to: str) -> Result<Path, FileError> uses FileSystem
```

Usage:
```ori
use std.fs { canonicalize, relative }

let abs = canonicalize(path: "../data/file.txt")?
// "/home/user/project/data/file.txt"

let rel = relative(from: "/home/user", to: "/home/user/project/src")?
// "project/src"
```

---

## Examples

### Config File Loading

```ori
use std.fs { read, exists }
use std.json { parse_as }

@load_config () -> Result<Config, Error> uses FileSystem = {
    let paths = ["config.json", "~/.config/myapp/config.json", "/etc/myapp/config.json"]
    let config_path = paths.find(p -> exists(path: p))

    match config_path {
        Some(path) -> {
            let content = read(path: path)?
            parse_as<Config>(source: content)
        }
        None -> Ok(Config.default())
    }
}
```

### Directory Backup

```ori
use std.fs { walk_dir, copy, create_dir_all, Path }

@backup_dir (source: str, dest: str) -> Result<int, Error> uses FileSystem = {
    let files = walk_dir(path: source)?
    let source_path = Path.from_str(source)
    let count = 0

    for info in files do
        if info.is_file then {
            let rel = info.path.relative_to(base: source_path).unwrap()
            let target = Path.from_str(dest).join(other: rel)
            create_dir_all(path: target.parent().unwrap().to_str())?
            copy(from: info.path.to_str(), to: target.to_str())?
            count = count + 1
        }

    Ok(count)
}
```

### Log File Rotation

```ori
use std.fs { exists, move, remove, info }

@rotate_logs (base_path: str, max_files: int) -> Result<void, Error> uses FileSystem = {
    // Rotate existing logs
    for i in (max_files - 1)..0 by -1 do {
        let current = `{base_path}.{i}`
        let next = `{base_path}.{i + 1}`
        if exists(path: current) then
            if i == max_files - 1 then
                remove(path: current)?
            else
                move(from: current, to: next)?
    }

    // Rotate current to .1
    if exists(path: base_path) then
        move(from: base_path, to: `{base_path}.1`)?

    Ok(())
}
```

---

## Module Structure

```ori
// std/fs/mod.ori
pub use "./path" { Path }
pub use "./types" { FileInfo, FileError, FileErrorKind, Permissions, WriteMode }
pub use "./read" { read, read_bytes, read_lines, open_read, FileReader }
pub use "./write" { write, write_bytes, write_with, open_write, FileWriter }
pub use "./dir" { list_dir, list_dir_info, walk_dir, walk_dir_with, create_dir, create_dir_all, remove_dir, remove_dir_all }
pub use "./ops" { copy, copy_with, move, rename, remove }
pub use "./info" { info, exists, is_file, is_dir, get_permissions, set_permissions, set_readonly }
pub use "./glob" { glob }
pub use "./temp" { temp_dir, create_temp_file, create_temp_dir, with_temp_file, with_temp_dir }
pub use "./path_utils" { cwd, set_cwd, canonicalize, resolve, relative }
```

---

## Summary

| Category | Functions |
|----------|-----------|
| Reading | `read`, `read_bytes`, `read_lines`, `open_read` |
| Writing | `write`, `write_bytes`, `write_with`, `open_write` |
| Directories | `list_dir`, `walk_dir`, `create_dir`, `remove_dir` |
| Operations | `copy`, `move`, `remove` |
| Info | `info`, `exists`, `is_file`, `is_dir` |
| Glob | `glob` |
| Temp | `create_temp_file`, `with_temp_file` |
| Paths | `cwd`, `canonicalize`, `resolve`, `relative` |
