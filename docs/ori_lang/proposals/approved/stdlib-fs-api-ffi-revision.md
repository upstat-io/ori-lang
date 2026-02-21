# Proposal: std.fs API Design (FFI Revision)

**Status:** Approved
**Approved:** 2026-01-30
**Created:** 2026-01-30
**Affects:** Standard library
**Depends on:** C FFI proposal, Fixed-capacity lists proposal

---

## Summary

This revision adds FFI implementation details to the approved `std.fs` proposal. File system operations are backed by POSIX APIs on Unix platforms. Windows support is deferred to a separate proposal.

---

## FFI Implementation

### Backend Selection

| Platform | Primary API | Library |
|----------|-------------|---------|
| Linux | POSIX | libc |
| macOS | POSIX | libc |
| BSD | POSIX | libc |

### Windows Support

Windows file system FFI is deferred to a separate proposal due to:
- WCHAR/UTF-16 encoding complexity
- Different API semantics (handles vs file descriptors)
- Different error handling patterns (GetLastError vs errno)

See: `stdlib-fs-windows-ffi-proposal.md` (future)

### External Declarations (POSIX)

```ori
// std/fs/ffi_posix.ori (internal)
#target(family: "unix")

#repr("c")
type CStat = {
    st_dev: int,
    st_ino: int,
    st_mode: int,
    st_nlink: int,
    st_uid: int,
    st_gid: int,
    st_rdev: int,
    st_size: int,
    st_blksize: int,
    st_blocks: int,
    st_atime: int,
    st_mtime: int,
    st_ctime: int
}

#repr("c")
type CDirent = {
    d_ino: int,
    d_off: int,
    d_reclen: int,
    d_type: byte,
    d_name: [byte, max 256]
}

extern "c" from "libc" {
    // File operations
    @_open (path: str, flags: int, mode: int) -> int as "open"
    @_close (fd: int) -> int as "close"
    @_read (fd: int, buf: [byte], count: int) -> int as "read"
    @_write (fd: int, buf: [byte], count: int) -> int as "write"
    @_lseek (fd: int, offset: int, whence: int) -> int as "lseek"
    @_fsync (fd: int) -> int as "fsync"
    @_ftruncate (fd: int, length: int) -> int as "ftruncate"

    // File info
    @_stat (path: str, buf: CStat) -> int as "stat"
    @_lstat (path: str, buf: CStat) -> int as "lstat"
    @_fstat (fd: int, buf: CStat) -> int as "fstat"
    @_access (path: str, mode: int) -> int as "access"

    // Directory operations
    @_mkdir (path: str, mode: int) -> int as "mkdir"
    @_rmdir (path: str) -> int as "rmdir"
    @_opendir (path: str) -> CPtr as "opendir"
    @_readdir (dir: CPtr) -> CPtr as "readdir"
    @_closedir (dir: CPtr) -> int as "closedir"

    // File manipulation
    @_unlink (path: str) -> int as "unlink"
    @_rename (old: str, new: str) -> int as "rename"
    @_link (old: str, new: str) -> int as "link"
    @_symlink (target: str, linkpath: str) -> int as "symlink"
    @_readlink (path: str, buf: [byte], bufsiz: int) -> int as "readlink"

    // Permissions
    @_chmod (path: str, mode: int) -> int as "chmod"
    @_chown (path: str, owner: int, group: int) -> int as "chown"

    // Path operations
    @_getcwd (buf: [byte], size: int) -> CPtr as "getcwd"
    @_chdir (path: str) -> int as "chdir"
    @_realpath (path: str, resolved: [byte]) -> CPtr as "realpath"

    // Temp files
    @_mkstemp (template: [byte]) -> int as "mkstemp"
    @_mkdtemp (template: [byte]) -> CPtr as "mkdtemp"

    // Error string
    @_strerror (errnum: int) -> str as "strerror"
}

// Platform-specific errno access
#target(os: "linux")
extern "c" from "libc" {
    @_errno_location () -> CPtr as "__errno_location"
}

#target(os: "macos")
extern "c" from "libc" {
    @_errno_location () -> CPtr as "__error"
}

#target(any_os: ["freebsd", "openbsd", "netbsd"])
extern "c" from "libc" {
    @_errno_location () -> CPtr as "__error"
}

// Helper to get errno value
@get_errno () -> int uses FFI =
    unsafe(ptr_read_int(ptr: _errno_location()))

// Open flags
let $O_RDONLY: int = 0
let $O_WRONLY: int = 1
let $O_RDWR: int = 2
let $O_CREAT: int = 64
let $O_EXCL: int = 128
let $O_TRUNC: int = 512
let $O_APPEND: int = 1024

// File mode bits (owner)
let $S_IRUSR: int = 256   // 0400
let $S_IWUSR: int = 128   // 0200
let $S_IXUSR: int = 64    // 0100
let $S_IRWXU: int = 448   // 0700 = S_IRUSR | S_IWUSR | S_IXUSR

// File mode bits (group)
let $S_IRGRP: int = 32    // 0040
let $S_IWGRP: int = 16    // 0020
let $S_IXGRP: int = 8     // 0010
let $S_IRWXG: int = 56    // 0070 = S_IRGRP | S_IWGRP | S_IXGRP

// File mode bits (other)
let $S_IROTH: int = 4     // 0004
let $S_IWOTH: int = 2     // 0002
let $S_IXOTH: int = 1     // 0001
let $S_IRWXO: int = 7     // 0007 = S_IROTH | S_IWOTH | S_IXOTH

// Stat mode masks
let $S_IFMT: int = 61440
let $S_IFREG: int = 32768
let $S_IFDIR: int = 16384
let $S_IFLNK: int = 40960

// Access modes
let $F_OK: int = 0
let $R_OK: int = 4
let $W_OK: int = 2
let $X_OK: int = 1

// Seek whence
let $SEEK_SET: int = 0
let $SEEK_CUR: int = 1
let $SEEK_END: int = 2

// Platform-specific dirent name offset
#target(os: "linux")
let $DIRENT_NAME_OFFSET: int = 19  // After d_ino(8) + d_off(8) + d_reclen(2) + d_type(1)

#target(os: "macos")
let $DIRENT_NAME_OFFSET: int = 21  // macOS dirent has different layout

#target(any_os: ["freebsd", "openbsd", "netbsd"])
let $DIRENT_NAME_OFFSET: int = 8   // BSD dirent layout
```

### FFI Helper Functions

```ori
// std/fs/ffi_helpers.ori (internal)
#target(family: "unix")

// Zero-initialize a CStat struct for use with stat/fstat/lstat
impl CStat {
    @zeroed () -> CStat =
        CStat {
            st_dev: 0, st_ino: 0, st_mode: 0, st_nlink: 0,
            st_uid: 0, st_gid: 0, st_rdev: 0, st_size: 0,
            st_blksize: 0, st_blocks: 0,
            st_atime: 0, st_mtime: 0, st_ctime: 0
        }
}

// Extract name from dirent pointer
@dirent_name (entry: CPtr) -> str uses FFI =
    unsafe(ptr_read_cstr(ptr: entry, offset: $DIRENT_NAME_OFFSET))
```

---

## Implementation Mapping

### Reading Files

```ori
// std/fs/read.ori
#target(family: "unix")
use "./ffi_posix" { _open, _close, _read, _fstat, CStat, $O_RDONLY }
use "./ffi_helpers" { CStat.zeroed }
use "./error" { errno_to_file_error }

pub @read (path: str) -> Result<str, FileError> uses FileSystem =
    {
        let fd = _open(path: path, flags: $O_RDONLY, mode: 0)
        if fd < 0 then
            Err(errno_to_file_error(path: Path.from_str(s: path)))
        else
            {
                let stat_buf = CStat.zeroed()
                _fstat(fd: fd, buf: stat_buf)
                let size = stat_buf.st_size
                let buf: [byte, max 1048576] = [],  // 1MB max inline read
                let bytes_read = _read(fd: fd, buf: buf, count: size)
                _close(fd: fd)
                if bytes_read < 0 then
                    Err(errno_to_file_error(path: Path.from_str(s: path)))
                else
                    str.from_utf8(bytes: buf[0..bytes_read])
                        .map_err(transform: e -> FileError {
                            kind: IoError
                            path: Path.from_str(s: path)
                            message: "Invalid UTF-8"
                        })
            }
    }

pub @read_bytes (path: str) -> Result<[byte], FileError> uses FileSystem =
    {
        let fd = _open(path: path, flags: $O_RDONLY, mode: 0)
        if fd < 0 then
            Err(errno_to_file_error(path: Path.from_str(s: path)))
        else
            {
                let stat_buf = CStat.zeroed()
                _fstat(fd: fd, buf: stat_buf)
                let size = stat_buf.st_size
                let buf: [byte, max 1048576] = []
                let bytes_read = _read(fd: fd, buf: buf, count: size)
                _close(fd: fd)
                if bytes_read < 0 then
                    Err(errno_to_file_error(path: Path.from_str(s: path)))
                else
                    Ok(buf[0..bytes_read].to_dynamic())
            }
    }
```

### Writing Files

```ori
// std/fs/write.ori
#target(family: "unix")
use "./ffi_posix" { _open, _close, _write, $O_WRONLY, $O_CREAT, $O_TRUNC, $O_APPEND, $O_EXCL, $S_IRUSR, $S_IWUSR, $S_IRGRP, $S_IROTH }
use "./error" { errno_to_file_error }

pub @write (path: str, content: str) -> Result<void, FileError> uses FileSystem =
    write_bytes(path: path, content: content.as_bytes())

pub @write_bytes (path: str, content: [byte]) -> Result<void, FileError> uses FileSystem =
    {
        let flags = $O_WRONLY | $O_CREAT | $O_TRUNC
        let mode = $S_IRUSR | $S_IWUSR | $S_IRGRP | $S_IROTH,  // 0644
        let fd = _open(path: path, flags: flags, mode: mode)
        if fd < 0 then
            Err(errno_to_file_error(path: Path.from_str(s: path)))
        else
            {
                let written = _write(fd: fd, buf: content, count: len(collection: content))
                _close(fd: fd)
                if written < 0 then
                    Err(errno_to_file_error(path: Path.from_str(s: path)))
                else
                    Ok(())
            }
    }

pub @write_with (
    path: str,
    content: str,
    mode: WriteMode = Truncate,
    create_dirs: bool = false
) -> Result<void, FileError> uses FileSystem =
    {
        if create_dirs then
            {
                let parent = Path.from_str(s: path).parent()
                match parent {
                    Some(p) -> create_dir_all(path: p.to_str())?
                    None -> ()
                }
            }

        let flags = match mode {
            Create -> $O_WRONLY | $O_CREAT | $O_EXCL
            Append -> $O_WRONLY | $O_CREAT | $O_APPEND
            Truncate -> $O_WRONLY | $O_CREAT | $O_TRUNC
        }
        let file_mode = $S_IRUSR | $S_IWUSR | $S_IRGRP | $S_IROTH
        let fd = _open(path: path, flags: flags, mode: file_mode)
        if fd < 0 then
            Err(errno_to_file_error(path: Path.from_str(s: path)))
        else
            {
                let bytes = content.as_bytes()
                let written = _write(fd: fd, buf: bytes, count: len(collection: bytes))
                _close(fd: fd)
                if written < 0 then
                    Err(errno_to_file_error(path: Path.from_str(s: path)))
                else
                    Ok(())
            }
    }
```

### Directory Operations

```ori
// std/fs/dir.ori
#target(family: "unix")
use "./ffi_posix" { _opendir, _readdir, _closedir, _mkdir, $S_IRWXU, $S_IRGRP, $S_IXGRP, $S_IROTH, $S_IXOTH }
use "./ffi_helpers" { dirent_name }
use "./error" { errno_to_file_error }

pub @list_dir (path: str) -> Result<[str], FileError> uses FileSystem =
    {
        let dir = _opendir(path: path)
        if dir.is_null() then
            Err(errno_to_file_error(path: Path.from_str(s: path)))
        else
            {
                let entries: [str, max 4096] = []
                loop {
                    let entry = _readdir(dir: dir)
                    if entry.is_null() then break
                    let name = dirent_name(entry: entry)
                    if name != "." && name != ".." then
                        entries.push(name)
                    continue
                }
                _closedir(dir: dir)
                Ok(entries.to_dynamic())
            }
    }

pub @create_dir (path: str) -> Result<void, FileError> uses FileSystem =
    {
        let mode = $S_IRWXU | $S_IRGRP | $S_IXGRP | $S_IROTH | $S_IXOTH,  // 0755
        let result = _mkdir(path: path, mode: mode)
        if result < 0 then
            Err(errno_to_file_error(path: Path.from_str(s: path)))
        else
            Ok(())
    }

pub @create_dir_all (path: str) -> Result<void, FileError> uses FileSystem =
    {
        let parts = Path.from_str(s: path).segments
        let current = if Path.from_str(s: path).is_absolute() then "/" else ""
        for part in parts do
            {
                current = current + "/" + part
                if !exists(path: current) then
                    create_dir(path: current)?
            }
        Ok(())
    }

pub @walk_dir (path: str) -> Result<[FileInfo], FileError> uses FileSystem =
    walk_dir_with(path: path, max_depth: -1, follow_symlinks: false)

pub @walk_dir_with (
    path: str,
    max_depth: int = -1,
    follow_symlinks: bool = false
) -> Result<[FileInfo], FileError> uses FileSystem =
    walk_recursive(path: path, depth: 0, max_depth: max_depth, follow_symlinks: follow_symlinks)

@walk_recursive (
    path: str,
    depth: int,
    max_depth: int,
    follow_symlinks: bool
) -> Result<[FileInfo], FileError> uses FileSystem =
    {
        let entries = list_dir_info(path: path)?
        let results: [FileInfo, max 10000] = []
        for entry in entries do
            {
                results.push(entry)
                if entry.is_dir && (max_depth < 0 || depth < max_depth) then
                    if !entry.is_symlink || follow_symlinks then
                        {
                            let sub = walk_recursive(
                                path: entry.path.to_str()
                                depth: depth + 1
                                max_depth: max_depth
                                follow_symlinks: follow_symlinks
                            )?
                            for s in sub do results.push(s)
                        }
            }
        Ok(results.to_dynamic())
    }
```

### File Info

```ori
// std/fs/info.ori
#target(family: "unix")
use "./ffi_posix" { _stat, _lstat, _access, CStat, $S_IFMT, $S_IFREG, $S_IFDIR, $S_IFLNK, $S_IWUSR, $F_OK }
use "./ffi_helpers" { CStat.zeroed }
use "./error" { errno_to_file_error }
use std.time { Instant }

pub @info (path: str) -> Result<FileInfo, FileError> uses FileSystem =
    {
        let stat_buf = CStat.zeroed()
        let result = _lstat(path: path, buf: stat_buf)
        if result < 0 then
            Err(errno_to_file_error(path: Path.from_str(s: path)))
        else
            Ok(stat_to_file_info(path: path, stat: stat_buf))
    }

@stat_to_file_info (path: str, stat: CStat) -> FileInfo =
    FileInfo {
        path: Path.from_str(s: path),
        size: stat.st_size,
        is_file: (stat.st_mode & $S_IFMT) == $S_IFREG,
        is_dir: (stat.st_mode & $S_IFMT) == $S_IFDIR,
        is_symlink: (stat.st_mode & $S_IFMT) == $S_IFLNK,
        modified: Instant.from_unix_secs(secs: stat.st_mtime),
        created: None,  // Not reliably available on all Unix
        readonly: (stat.st_mode & $S_IWUSR) == 0
    }

pub @exists (path: str) -> bool uses FileSystem =
    _access(path: path, mode: $F_OK) == 0

pub @is_file (path: str) -> bool uses FileSystem =
    {
        let stat_buf = CStat.zeroed()
        let result = _stat(path: path, buf: stat_buf)
        result == 0 && (stat_buf.st_mode & $S_IFMT) == $S_IFREG
    }

pub @is_dir (path: str) -> bool uses FileSystem =
    {
        let stat_buf = CStat.zeroed()
        let result = _stat(path: path, buf: stat_buf)
        result == 0 && (stat_buf.st_mode & $S_IFMT) == $S_IFDIR
    }
```

### Error Mapping

```ori
// std/fs/error.ori
#target(family: "unix")
use "./ffi_posix" { get_errno, _strerror }

// POSIX errno values
let $ENOENT: int = 2
let $EACCES: int = 13
let $EEXIST: int = 17
let $ENOTDIR: int = 20
let $EISDIR: int = 21
let $ENOTEMPTY: int = 39

@errno_to_file_error (path: Path) -> FileError =
    {
        let err = get_errno()
        let kind = match err {
            $ENOENT -> NotFound
            $EACCES -> PermissionDenied
            $EEXIST -> AlreadyExists
            $ENOTDIR -> NotADirectory
            $EISDIR -> NotAFile
            $ENOTEMPTY -> DirectoryNotEmpty
            _ -> IoError
        }
        FileError {
            kind: kind
            path: path
            message: _strerror(errnum: err)
        }
    }
```

### Glob Pattern Matching

```ori
// std/fs/glob.ori
#target(family: "unix")

// Use libc glob()
#repr("c")
type CGlob = {
    gl_pathc: int,
    gl_pathv: CPtr,
    gl_offs: int
}

extern "c" from "libc" {
    @_glob (pattern: str, flags: int, errfunc: CPtr, pglob: CGlob) -> int as "glob"
    @_globfree (pglob: CGlob) -> void as "globfree"
}

let $GLOB_ERR: int = 1
let $GLOB_MARK: int = 2

impl CGlob {
    @zeroed () -> CGlob =
        CGlob { gl_pathc: 0, gl_pathv: CPtr.null(), gl_offs: 0 }
}

// Read path from glob result at index
@read_glob_path (pglob: CGlob, index: int) -> str uses FFI =
    unsafe {
        let path_ptr = ptr_array_index(ptr: pglob.gl_pathv, index: index)
        ptr_read_cstr(ptr: path_ptr, offset: 0)
    }

pub @glob (pattern: str) -> Result<[str], FileError> uses FileSystem =
    {
        let pglob = CGlob.zeroed()
        let result = _glob(pattern: pattern, flags: 0, errfunc: CPtr.null(), pglob: pglob)
        if result != 0 then
            {
                _globfree(pglob: pglob)
                Err(FileError { kind: IoError, path: Path.from_str(s: pattern), message: "glob failed" })
            }
        else
            {
                let paths: [str, max 10000] = []
                for i in 0..pglob.gl_pathc do
                    paths.push(read_glob_path(pglob: pglob, index: i))
                _globfree(pglob: pglob)
                Ok(paths.to_dynamic())
            }
    }
```

---

## Pure Ori Components

These don't need FFI:

| Component | Implementation |
|-----------|----------------|
| `Path` type | Pure Ori string manipulation |
| `Path.join()` | Pure Ori string concatenation |
| `Path.parent()` | Pure Ori string parsing |
| `Path.extension()` | Pure Ori string parsing |
| `WriteMode` enum | Pure Ori type |
| `FileError` type | Pure Ori type |
| `FileInfo` type | Pure Ori type |
| Glob pattern parsing | Pure Ori (pattern to regex/matcher) |

```ori
// std/fs/path.ori - Pure Ori
impl Path {
    pub @from_str (s: str) -> Path =
        {
            let normalized = normalize_separators(s: s)
            let absolute = normalized.starts_with(prefix: "/")
            let segments = normalized.split(sep: "/").filter(predicate: s -> !is_empty(collection: s)).collect()
            Path { segments: segments, absolute: absolute }
        }

    pub @join (self, other: Path) -> Path =
        if other.absolute then
            other
        else
            Path { segments: [...self.segments, ...other.segments], absolute: self.absolute }

    pub @parent (self) -> Option<Path> =
        if is_empty(collection: self.segments) then
            None
        else
            Some(Path { segments: self.segments[0..# - 1], absolute: self.absolute })

    pub @extension (self) -> Option<str> =
        {
            let name = self.file_name()?
            let dot_idx = name.rfind(s: ".")
            match dot_idx {
                Some(i) if i > 0 -> Some(name[(i + 1)..])
                _ -> None
            }
        }

    pub @to_str (self) -> str =
        {
            let prefix = if self.absolute then "/" else ""
            prefix + self.segments.join(sep: "/")
        }
}
```

---

## Streaming File I/O

```ori
// std/fs/stream.ori
#target(family: "unix")
use "./ffi_posix" { _open, _close, _read, $O_RDONLY }
use "./error" { errno_to_file_error }

type FileReader = { fd: int, path: Path }

pub @open_read (path: str) -> Result<FileReader, FileError> uses FileSystem =
    {
        let fd = _open(path: path, flags: $O_RDONLY, mode: 0)
        if fd < 0 then
            Err(errno_to_file_error(path: Path.from_str(s: path)))
        else
            Ok(FileReader { fd: fd, path: Path.from_str(s: path) })
    }

impl FileReader {
    pub @read_chunk (self, size: int) -> Result<([byte], FileReader), FileError> uses FileSystem =
        {
            let buf: [byte, max 65536] = [],  // 64KB chunks
            let read_size = min(left: size, right: 65536)
            let bytes_read = _read(fd: self.fd, buf: buf, count: read_size)
            if bytes_read < 0 then
                Err(errno_to_file_error(path: self.path))
            else
                Ok((buf[0..bytes_read].to_dynamic(), self))
        }

    pub @close (self) -> void uses FileSystem =
        _close(fd: self.fd)
}

// Line iterator wraps FileReader
type FileLineIterator = { reader: FileReader, buffer: str, eof: bool }

impl Iterator for FileLineIterator {
    type Item = str

    @next (self) -> (Option<str>, FileLineIterator) =
        {
            if self.eof && is_empty(collection: self.buffer) then
                (None, self)
            else
                {
                    // Check for newline in buffer
                    let newline_idx = self.buffer.find(s: "\n")
                    match newline_idx {
                        Some(idx) ->
                            {
                                let line = self.buffer[0..idx]
                                let rest = self.buffer[(idx + 1)..]
                                (Some(line), FileLineIterator { reader: self.reader, buffer: rest, eof: self.eof })
                            }
                        None ->
                            if self.eof then
                                // Return remaining buffer as final line
                                (Some(self.buffer), FileLineIterator { reader: self.reader, buffer: "", eof: true })
                            else
                                // Need to read more
                                {
                                    let result = self.reader.read_chunk(size: 8192)
                                    match result {
                                        Ok((chunk, reader)) ->
                                            {
                                                let new_buffer = self.buffer + str.from_utf8(bytes: chunk).unwrap_or(default: "")
                                                let new_eof = is_empty(collection: chunk)
                                                FileLineIterator { reader: reader, buffer: new_buffer, eof: new_eof }.next()
                                            }
                                        Err(_) ->
                                            (None, FileLineIterator { reader: self.reader, buffer: "", eof: true })
                                    }
                                }
                    }
                }
        }
}
```

---

## Build Configuration

```toml
# ori.toml
[native]
libraries = []  # libc is implicit on Unix

[native.linux]
libraries = []

[native.macos]
libraries = []
```

---

## Performance Note

The implementation examples in this proposal use clear, immutable-style patterns for readability:

```ori
// Clear but creates new list each iteration
let results: [FileInfo, max 10000] = []
for entry in entries do
    results.push(entry)
```

Production implementations may use more efficient patterns depending on the compiler's optimization capabilities. This proposal focuses on correctness and API design; performance optimizations are implementation details.

---

## Summary of Changes from Original

| Aspect | Original | This Revision |
|--------|----------|---------------|
| Public API | Defined | **Unchanged** |
| POSIX implementation | Not specified | **Detailed FFI bindings** |
| Windows implementation | Not specified | **Deferred to separate proposal** |
| Path type | Defined | **Pure Ori implementation** |
| Glob | Pattern described | **libc glob() implementation** |
| Error mapping | Error types defined | **errno mapping with platform-specific access** |
| Fixed arrays | Used `[T; N]` | **Uses `[T, max N]` (depends on fixed-capacity lists)** |
| Struct initialization | Used `...` | **Uses `.zeroed()` method** |

---

## Design Decisions

### Why platform-specific errno?

`__errno_location` (Linux) and `__error` (macOS/BSD) are the only portable ways to access errno in a thread-safe manner. A unified wrapper function `get_errno()` hides this platform difference from implementation code.

### Why defer Windows?

Windows file system APIs have fundamentally different patterns:
- Wide character strings (UTF-16) vs UTF-8
- Handles vs file descriptors
- Different error handling (GetLastError vs errno)
- Different permission model (ACLs vs Unix mode bits)

A proper Windows implementation deserves its own focused proposal.

### Why use fixed-capacity lists?

C structs often contain fixed-size arrays (e.g., `char name[256]`). The fixed-capacity list proposal's `[T, max N]` syntax maps directly to this need, avoiding the need for FFI-specific array syntax.

### Why `.zeroed()` method?

FFI structs passed to C functions that populate them (like `stat()`) need to be zero-initialized. A `.zeroed()` method is cleaner than manually initializing every field and makes intent clear.

## Errata (added 2026-02-20)

> **Superseded by [unsafe-semantics-proposal](unsafe-semantics-proposal.md)**: Examples in this proposal use the `unsafe(expr)` parenthesized form, which has been removed. The approved syntax is `unsafe { expr }` (block-only form). See the unsafe semantics proposal for the full specification.
