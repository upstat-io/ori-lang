# Proposal: std.fs Windows FFI Implementation

**Status:** Draft
**Created:** 2026-01-30
**Affects:** Standard library
**Depends on:** std.fs API proposal, Platform FFI proposal, Fixed-capacity lists proposal

---

## Summary

This proposal adds Windows FFI implementation details to the approved `std.fs` API. File system operations on Windows are backed by Win32 APIs from kernel32.dll, with UTF-16 string handling and handle-based I/O.

---

## Motivation

Windows file system APIs differ significantly from POSIX:

| Aspect | POSIX | Windows |
|--------|-------|---------|
| String encoding | UTF-8 | UTF-16 (WCHAR) |
| File handles | int (fd) | HANDLE (void*) |
| Error handling | errno | GetLastError() |
| Path separators | `/` | `\` (also accepts `/`) |
| Path length | ~4096 | 260 (MAX_PATH) or extended |
| Permissions | Mode bits | ACLs |

A dedicated Windows implementation ensures:
1. Correct UTF-16 string marshalling
2. Proper handle management
3. Windows-specific error mapping
4. Extended path support (>260 chars)

---

## FFI Implementation

### External Declarations

```ori
// std/fs/ffi_windows.ori (internal)
#target(os: "windows")

// Windows type aliases
type HANDLE = CPtr
type DWORD = int      // 32-bit unsigned, but int for simplicity
type BOOL = int       // Windows BOOL is int, not bool
type WCHAR = int      // 16-bit, stored as int for manipulation

// Invalid handle sentinel
let $INVALID_HANDLE_VALUE: HANDLE = CPtr.from_int(i: -1)

#repr("c")
type FILETIME = {
    dwLowDateTime: int,
    dwHighDateTime: int
}

#repr("c")
type WIN32_FIND_DATAW = {
    dwFileAttributes: DWORD,
    ftCreationTime: FILETIME,
    ftLastAccessTime: FILETIME,
    ftLastWriteTime: FILETIME,
    nFileSizeHigh: DWORD,
    nFileSizeLow: DWORD,
    dwReserved0: DWORD,
    dwReserved1: DWORD,
    cFileName: [WCHAR, max 260],
    cAlternateFileName: [WCHAR, max 14]
}

#repr("c")
type BY_HANDLE_FILE_INFORMATION = {
    dwFileAttributes: DWORD,
    ftCreationTime: FILETIME,
    ftLastAccessTime: FILETIME,
    ftLastWriteTime: FILETIME,
    dwVolumeSerialNumber: DWORD,
    nFileSizeHigh: DWORD,
    nFileSizeLow: DWORD,
    nNumberOfLinks: DWORD,
    nFileIndexHigh: DWORD,
    nFileIndexLow: DWORD
}

#repr("c")
type SECURITY_ATTRIBUTES = {
    nLength: DWORD,
    lpSecurityDescriptor: CPtr,
    bInheritHandle: BOOL
}

extern "c" from "kernel32" {
    // File operations
    @_CreateFileW (
        lpFileName: CPtr,           // LPCWSTR - pointer to UTF-16 string
        dwDesiredAccess: DWORD,
        dwShareMode: DWORD,
        lpSecurityAttributes: CPtr, // nullable
        dwCreationDisposition: DWORD,
        dwFlagsAndAttributes: DWORD,
        hTemplateFile: HANDLE       // nullable
    ) -> HANDLE as "CreateFileW"

    @_ReadFile (
        hFile: HANDLE,
        lpBuffer: [byte],
        nNumberOfBytesToRead: DWORD,
        lpNumberOfBytesRead: CPtr,  // out: DWORD*
        lpOverlapped: CPtr          // nullable
    ) -> BOOL as "ReadFile"

    @_WriteFile (
        hFile: HANDLE,
        lpBuffer: [byte],
        nNumberOfBytesToWrite: DWORD,
        lpNumberOfBytesWritten: CPtr,  // out: DWORD*
        lpOverlapped: CPtr              // nullable
    ) -> BOOL as "WriteFile"

    @_CloseHandle (hObject: HANDLE) -> BOOL as "CloseHandle"

    @_FlushFileBuffers (hFile: HANDLE) -> BOOL as "FlushFileBuffers"

    @_SetFilePointerEx (
        hFile: HANDLE,
        liDistanceToMove: int,      // LARGE_INTEGER (64-bit)
        lpNewFilePointer: CPtr,     // out: nullable
        dwMoveMethod: DWORD
    ) -> BOOL as "SetFilePointerEx"

    @_SetEndOfFile (hFile: HANDLE) -> BOOL as "SetEndOfFile"

    @_GetFileSizeEx (
        hFile: HANDLE,
        lpFileSize: CPtr            // out: LARGE_INTEGER*
    ) -> BOOL as "GetFileSizeEx"

    // File info
    @_GetFileAttributesW (lpFileName: CPtr) -> DWORD as "GetFileAttributesW"

    @_GetFileAttributesExW (
        lpFileName: CPtr,
        fInfoLevelId: int,          // GET_FILEEX_INFO_LEVELS
        lpFileInformation: CPtr     // out: WIN32_FILE_ATTRIBUTE_DATA*
    ) -> BOOL as "GetFileAttributesExW"

    @_GetFileInformationByHandle (
        hFile: HANDLE,
        lpFileInformation: BY_HANDLE_FILE_INFORMATION
    ) -> BOOL as "GetFileInformationByHandle"

    @_SetFileAttributesW (
        lpFileName: CPtr,
        dwFileAttributes: DWORD
    ) -> BOOL as "SetFileAttributesW"

    // Directory operations
    @_CreateDirectoryW (
        lpPathName: CPtr,
        lpSecurityAttributes: CPtr  // nullable
    ) -> BOOL as "CreateDirectoryW"

    @_RemoveDirectoryW (lpPathName: CPtr) -> BOOL as "RemoveDirectoryW"

    @_FindFirstFileW (
        lpFileName: CPtr,
        lpFindFileData: WIN32_FIND_DATAW
    ) -> HANDLE as "FindFirstFileW"

    @_FindNextFileW (
        hFindFile: HANDLE,
        lpFindFileData: WIN32_FIND_DATAW
    ) -> BOOL as "FindNextFileW"

    @_FindClose (hFindFile: HANDLE) -> BOOL as "FindClose"

    // File manipulation
    @_DeleteFileW (lpFileName: CPtr) -> BOOL as "DeleteFileW"

    @_MoveFileExW (
        lpExistingFileName: CPtr,
        lpNewFileName: CPtr,
        dwFlags: DWORD
    ) -> BOOL as "MoveFileExW"

    @_CopyFileExW (
        lpExistingFileName: CPtr,
        lpNewFileName: CPtr,
        lpProgressRoutine: CPtr,    // nullable
        lpData: CPtr,               // nullable
        pbCancel: CPtr,             // nullable
        dwCopyFlags: DWORD
    ) -> BOOL as "CopyFileExW"

    @_CreateHardLinkW (
        lpFileName: CPtr,
        lpExistingFileName: CPtr,
        lpSecurityAttributes: CPtr  // nullable
    ) -> BOOL as "CreateHardLinkW"

    @_CreateSymbolicLinkW (
        lpSymlinkFileName: CPtr,
        lpTargetFileName: CPtr,
        dwFlags: DWORD
    ) -> BOOL as "CreateSymbolicLinkW"

    // Path operations
    @_GetCurrentDirectoryW (
        nBufferLength: DWORD,
        lpBuffer: CPtr              // out: WCHAR*
    ) -> DWORD as "GetCurrentDirectoryW"

    @_SetCurrentDirectoryW (lpPathName: CPtr) -> BOOL as "SetCurrentDirectoryW"

    @_GetFullPathNameW (
        lpFileName: CPtr,
        nBufferLength: DWORD,
        lpBuffer: CPtr,             // out: WCHAR*
        lpFilePart: CPtr            // out: nullable WCHAR**
    ) -> DWORD as "GetFullPathNameW"

    @_GetTempPathW (
        nBufferLength: DWORD,
        lpBuffer: CPtr              // out: WCHAR*
    ) -> DWORD as "GetTempPathW"

    @_GetLongPathNameW (
        lpszShortPath: CPtr,
        lpszLongPath: CPtr,         // out: WCHAR*
        cchBuffer: DWORD
    ) -> DWORD as "GetLongPathNameW"

    // Temp files
    @_GetTempFileNameW (
        lpPathName: CPtr,
        lpPrefixString: CPtr,
        uUnique: int,
        lpTempFileName: CPtr        // out: WCHAR[MAX_PATH]
    ) -> int as "GetTempFileNameW"

    // Error handling
    @_GetLastError () -> DWORD as "GetLastError"

    @_FormatMessageW (
        dwFlags: DWORD,
        lpSource: CPtr,             // nullable
        dwMessageId: DWORD,
        dwLanguageId: DWORD,
        lpBuffer: CPtr,             // out: WCHAR*
        nSize: DWORD,
        Arguments: CPtr             // nullable
    ) -> DWORD as "FormatMessageW"

    @_LocalFree (hMem: CPtr) -> CPtr as "LocalFree"
}

// Access rights
let $GENERIC_READ: DWORD = 0x80000000
let $GENERIC_WRITE: DWORD = 0x40000000
let $GENERIC_EXECUTE: DWORD = 0x20000000
let $GENERIC_ALL: DWORD = 0x10000000

// Share modes
let $FILE_SHARE_READ: DWORD = 0x00000001
let $FILE_SHARE_WRITE: DWORD = 0x00000002
let $FILE_SHARE_DELETE: DWORD = 0x00000004

// Creation dispositions
let $CREATE_NEW: DWORD = 1
let $CREATE_ALWAYS: DWORD = 2
let $OPEN_EXISTING: DWORD = 3
let $OPEN_ALWAYS: DWORD = 4
let $TRUNCATE_EXISTING: DWORD = 5

// File attributes
let $FILE_ATTRIBUTE_READONLY: DWORD = 0x00000001
let $FILE_ATTRIBUTE_HIDDEN: DWORD = 0x00000002
let $FILE_ATTRIBUTE_SYSTEM: DWORD = 0x00000004
let $FILE_ATTRIBUTE_DIRECTORY: DWORD = 0x00000010
let $FILE_ATTRIBUTE_ARCHIVE: DWORD = 0x00000020
let $FILE_ATTRIBUTE_NORMAL: DWORD = 0x00000080
let $FILE_ATTRIBUTE_TEMPORARY: DWORD = 0x00000100
let $FILE_ATTRIBUTE_REPARSE_POINT: DWORD = 0x00000400

// Invalid file attributes (error indicator)
let $INVALID_FILE_ATTRIBUTES: DWORD = 0xFFFFFFFF

// Move flags
let $MOVEFILE_REPLACE_EXISTING: DWORD = 0x00000001
let $MOVEFILE_COPY_ALLOWED: DWORD = 0x00000002

// Copy flags
let $COPY_FILE_FAIL_IF_EXISTS: DWORD = 0x00000001

// Symbolic link flags
let $SYMBOLIC_LINK_FLAG_FILE: DWORD = 0x0
let $SYMBOLIC_LINK_FLAG_DIRECTORY: DWORD = 0x1

// Seek methods
let $FILE_BEGIN: DWORD = 0
let $FILE_CURRENT: DWORD = 1
let $FILE_END: DWORD = 2

// FormatMessage flags
let $FORMAT_MESSAGE_ALLOCATE_BUFFER: DWORD = 0x00000100
let $FORMAT_MESSAGE_FROM_SYSTEM: DWORD = 0x00001000
let $FORMAT_MESSAGE_IGNORE_INSERTS: DWORD = 0x00000200
```

---

## UTF-16 String Handling

Windows APIs use UTF-16 (wide strings). Ori strings are UTF-8.

### Conversion Functions

```ori
// std/fs/ffi_windows_string.ori (internal)
#target(os: "windows")

// Convert Ori UTF-8 string to Windows UTF-16
// Returns pointer to null-terminated WCHAR array (allocated)
@utf8_to_utf16 (s: str) -> CPtr uses FFI =
    run(
        // Calculate required buffer size
        let utf8_bytes = s.as_bytes(),
        let max_wchars = len(collection: utf8_bytes) + 1,  // Worst case + null
        let buffer: [WCHAR, max 32768] = [],  // 32KB should handle most paths

        // Convert UTF-8 to UTF-16
        let wchar_count = utf8_to_utf16_impl(
            utf8: utf8_bytes,
            utf16: buffer,
            max_len: max_wchars
        ),

        // Return pointer to buffer
        buffer.as_ptr()
    )

// Low-level conversion (implemented in Ori or via FFI)
@utf8_to_utf16_impl (utf8: [byte], utf16: [WCHAR, max 32768], max_len: int) -> int =
    run(
        let i = 0,
        let j = 0,
        loop(
            if i >= len(collection: utf8) then break,

            let byte0 = utf8[i],

            // Decode UTF-8 codepoint
            let (codepoint, bytes_consumed) =
                if (byte0 & 0x80) == 0 then
                    // ASCII
                    (byte0 as int, 1)
                else if (byte0 & 0xE0) == 0xC0 then
                    // 2-byte sequence
                    let byte1 = utf8[i + 1],
                    (((byte0 & 0x1F) as int) << 6 | ((byte1 & 0x3F) as int), 2)
                else if (byte0 & 0xF0) == 0xE0 then
                    // 3-byte sequence
                    let byte1 = utf8[i + 1],
                    let byte2 = utf8[i + 2],
                    (((byte0 & 0x0F) as int) << 12 | ((byte1 & 0x3F) as int) << 6 | ((byte2 & 0x3F) as int), 3)
                else
                    // 4-byte sequence (surrogate pair needed)
                    let byte1 = utf8[i + 1],
                    let byte2 = utf8[i + 2],
                    let byte3 = utf8[i + 3],
                    (((byte0 & 0x07) as int) << 18 | ((byte1 & 0x3F) as int) << 12 | ((byte2 & 0x3F) as int) << 6 | ((byte3 & 0x3F) as int), 4),

            // Encode to UTF-16
            if codepoint <= 0xFFFF then
                run(
                    utf16[j] = codepoint,
                    j = j + 1
                )
            else
                // Surrogate pair for codepoints > 0xFFFF
                run(
                    let adjusted = codepoint - 0x10000,
                    let high_surrogate = 0xD800 + (adjusted >> 10),
                    let low_surrogate = 0xDC00 + (adjusted & 0x3FF),
                    utf16[j] = high_surrogate,
                    utf16[j + 1] = low_surrogate,
                    j = j + 2
                ),

            i = i + bytes_consumed,
            continue
        ),

        // Null terminate
        utf16[j] = 0,
        j
    )

// Convert Windows UTF-16 to Ori UTF-8 string
@utf16_to_utf8 (wstr: CPtr, len: int) -> str uses FFI =
    run(
        let buffer: [byte, max 65536] = [],
        let byte_count = utf16_to_utf8_impl(
            utf16: wstr,
            utf16_len: len,
            utf8: buffer,
            max_len: 65536
        ),
        str.from_utf8(bytes: buffer[0..byte_count]).unwrap_or(default: "")
    )

// Read null-terminated UTF-16 string from pointer
@utf16_to_utf8_null_terminated (wstr: CPtr) -> str uses FFI =
    run(
        // Find null terminator
        let len = 0,
        loop(
            let wchar = unsafe(ptr_read_u16(ptr: wstr, offset: len)),
            if wchar == 0 then break,
            len = len + 1,
            continue
        ),
        utf16_to_utf8(wstr: wstr, len: len)
    )
```

### Extended Path Support

Windows has a 260 character path limit (MAX_PATH). For longer paths, use the `\\?\` prefix:

```ori
// std/fs/ffi_windows_path.ori (internal)
#target(os: "windows")

// Convert path to extended-length format for paths > 260 chars
@to_extended_path (path: str) -> str =
    if len(collection: path) >= 260 && !path.starts_with(prefix: "\\\\?\\") then
        if path.starts_with(prefix: "\\\\") then
            // UNC path: \\server\share -> \\?\UNC\server\share
            "\\\\?\\UNC\\" + path[2..]
        else
            // Regular path: C:\foo -> \\?\C:\foo
            "\\\\?\\" + path
    else
        path

// Normalize path separators (/ -> \)
@normalize_path_separators (path: str) -> str =
    path.replace(from: "/", to: "\\")

// Prepare path for Windows API call
@prepare_win_path (path: str) -> CPtr uses FFI =
    run(
        let normalized = normalize_path_separators(path: path),
        let extended = to_extended_path(path: normalized),
        utf8_to_utf16(s: extended)
    )
```

---

## Implementation Mapping

### Reading Files

```ori
// std/fs/read_windows.ori
#target(os: "windows")
use "./ffi_windows" { ... }
use "./ffi_windows_string" { prepare_win_path, utf16_to_utf8 }
use "./error_windows" { last_error_to_file_error }

pub @read (path: str) -> Result<str, FileError> uses FileSystem =
    run(
        let wpath = prepare_win_path(path: path),
        let handle = _CreateFileW(
            lpFileName: wpath,
            dwDesiredAccess: $GENERIC_READ,
            dwShareMode: $FILE_SHARE_READ,
            lpSecurityAttributes: CPtr.null(),
            dwCreationDisposition: $OPEN_EXISTING,
            dwFlagsAndAttributes: $FILE_ATTRIBUTE_NORMAL,
            hTemplateFile: CPtr.null()
        ),

        if handle == $INVALID_HANDLE_VALUE then
            Err(last_error_to_file_error(path: Path.from_str(s: path)))
        else
            run(
                // Get file size
                let size_out: [int, max 1] = [0],
                let size_ok = _GetFileSizeEx(hFile: handle, lpFileSize: size_out.as_ptr()),
                if size_ok == 0 then
                    run(
                        _CloseHandle(hObject: handle),
                        Err(last_error_to_file_error(path: Path.from_str(s: path)))
                    )
                else
                    run(
                        let size = size_out[0],
                        let buf: [byte, max 1048576] = [],  // 1MB max inline read
                        let bytes_read_out: [int, max 1] = [0],

                        let read_ok = _ReadFile(
                            hFile: handle,
                            lpBuffer: buf,
                            nNumberOfBytesToRead: min(left: size, right: 1048576),
                            lpNumberOfBytesRead: bytes_read_out.as_ptr(),
                            lpOverlapped: CPtr.null()
                        ),

                        _CloseHandle(hObject: handle),

                        if read_ok == 0 then
                            Err(last_error_to_file_error(path: Path.from_str(s: path)))
                        else
                            str.from_utf8(bytes: buf[0..bytes_read_out[0]])
                                .map_err(transform: e -> FileError {
                                    kind: IoError,
                                    path: Path.from_str(s: path),
                                    message: "Invalid UTF-8"
                                })
                    )
            )
    )

pub @read_bytes (path: str) -> Result<[byte], FileError> uses FileSystem =
    run(
        let wpath = prepare_win_path(path: path),
        let handle = _CreateFileW(
            lpFileName: wpath,
            dwDesiredAccess: $GENERIC_READ,
            dwShareMode: $FILE_SHARE_READ,
            lpSecurityAttributes: CPtr.null(),
            dwCreationDisposition: $OPEN_EXISTING,
            dwFlagsAndAttributes: $FILE_ATTRIBUTE_NORMAL,
            hTemplateFile: CPtr.null()
        ),

        if handle == $INVALID_HANDLE_VALUE then
            Err(last_error_to_file_error(path: Path.from_str(s: path)))
        else
            run(
                let size_out: [int, max 1] = [0],
                _GetFileSizeEx(hFile: handle, lpFileSize: size_out.as_ptr()),
                let size = size_out[0],
                let buf: [byte, max 1048576] = [],
                let bytes_read_out: [int, max 1] = [0],

                let read_ok = _ReadFile(
                    hFile: handle,
                    lpBuffer: buf,
                    nNumberOfBytesToRead: min(left: size, right: 1048576),
                    lpNumberOfBytesRead: bytes_read_out.as_ptr(),
                    lpOverlapped: CPtr.null()
                ),

                _CloseHandle(hObject: handle),

                if read_ok == 0 then
                    Err(last_error_to_file_error(path: Path.from_str(s: path)))
                else
                    Ok(buf[0..bytes_read_out[0]].to_dynamic())
            )
    )
```

### Writing Files

```ori
// std/fs/write_windows.ori
#target(os: "windows")
use "./ffi_windows" { ... }
use "./ffi_windows_string" { prepare_win_path }
use "./error_windows" { last_error_to_file_error }

pub @write (path: str, content: str) -> Result<void, FileError> uses FileSystem =
    write_bytes(path: path, content: content.as_bytes())

pub @write_bytes (path: str, content: [byte]) -> Result<void, FileError> uses FileSystem =
    run(
        let wpath = prepare_win_path(path: path),
        let handle = _CreateFileW(
            lpFileName: wpath,
            dwDesiredAccess: $GENERIC_WRITE,
            dwShareMode: 0,  // Exclusive access for writing
            lpSecurityAttributes: CPtr.null(),
            dwCreationDisposition: $CREATE_ALWAYS,
            dwFlagsAndAttributes: $FILE_ATTRIBUTE_NORMAL,
            hTemplateFile: CPtr.null()
        ),

        if handle == $INVALID_HANDLE_VALUE then
            Err(last_error_to_file_error(path: Path.from_str(s: path)))
        else
            run(
                let bytes_written_out: [int, max 1] = [0],
                let write_ok = _WriteFile(
                    hFile: handle,
                    lpBuffer: content,
                    nNumberOfBytesToWrite: len(collection: content),
                    lpNumberOfBytesWritten: bytes_written_out.as_ptr(),
                    lpOverlapped: CPtr.null()
                ),

                _CloseHandle(hObject: handle),

                if write_ok == 0 then
                    Err(last_error_to_file_error(path: Path.from_str(s: path)))
                else
                    Ok(())
            )
    )

pub @write_with (
    path: str,
    content: str,
    mode: WriteMode = Truncate,
    create_dirs: bool = false
) -> Result<void, FileError> uses FileSystem =
    run(
        if create_dirs then
            run(
                let parent = Path.from_str(s: path).parent(),
                match(parent,
                    Some(p) -> create_dir_all(path: p.to_str())?,
                    None -> ()
                )
            ),

        let disposition = match(mode,
            Create -> $CREATE_NEW,
            Append -> $OPEN_ALWAYS,
            Truncate -> $CREATE_ALWAYS
        ),

        let wpath = prepare_win_path(path: path),
        let handle = _CreateFileW(
            lpFileName: wpath,
            dwDesiredAccess: $GENERIC_WRITE,
            dwShareMode: 0,
            lpSecurityAttributes: CPtr.null(),
            dwCreationDisposition: disposition,
            dwFlagsAndAttributes: $FILE_ATTRIBUTE_NORMAL,
            hTemplateFile: CPtr.null()
        ),

        if handle == $INVALID_HANDLE_VALUE then
            Err(last_error_to_file_error(path: Path.from_str(s: path)))
        else
            run(
                // For append mode, seek to end
                if mode == Append then
                    _SetFilePointerEx(
                        hFile: handle,
                        liDistanceToMove: 0,
                        lpNewFilePointer: CPtr.null(),
                        dwMoveMethod: $FILE_END
                    ),

                let bytes = content.as_bytes(),
                let bytes_written_out: [int, max 1] = [0],
                let write_ok = _WriteFile(
                    hFile: handle,
                    lpBuffer: bytes,
                    nNumberOfBytesToWrite: len(collection: bytes),
                    lpNumberOfBytesWritten: bytes_written_out.as_ptr(),
                    lpOverlapped: CPtr.null()
                ),

                _CloseHandle(hObject: handle),

                if write_ok == 0 then
                    Err(last_error_to_file_error(path: Path.from_str(s: path)))
                else
                    Ok(())
            )
    )
```

### Directory Operations

```ori
// std/fs/dir_windows.ori
#target(os: "windows")
use "./ffi_windows" { ... }
use "./ffi_windows_string" { prepare_win_path, utf16_to_utf8_null_terminated }
use "./error_windows" { last_error_to_file_error }

pub @list_dir (path: str) -> Result<[str], FileError> uses FileSystem =
    run(
        // Append \* for FindFirstFile pattern
        let pattern = path + "\\*",
        let wpattern = prepare_win_path(path: pattern),

        let find_data = WIN32_FIND_DATAW.zeroed(),
        let handle = _FindFirstFileW(lpFileName: wpattern, lpFindFileData: find_data),

        if handle == $INVALID_HANDLE_VALUE then
            Err(last_error_to_file_error(path: Path.from_str(s: path)))
        else
            run(
                let entries: [str, max 4096] = [],

                // Process first result
                let name = utf16_to_utf8_null_terminated(wstr: find_data.cFileName.as_ptr()),
                if name != "." && name != ".." then
                    entries.push(name),

                // Process remaining results
                loop(
                    let find_data_next = WIN32_FIND_DATAW.zeroed(),
                    let found = _FindNextFileW(hFindFile: handle, lpFindFileData: find_data_next),
                    if found == 0 then break,

                    let name_next = utf16_to_utf8_null_terminated(wstr: find_data_next.cFileName.as_ptr()),
                    if name_next != "." && name_next != ".." then
                        entries.push(name_next),
                    continue
                ),

                _FindClose(hFindFile: handle),
                Ok(entries.to_dynamic())
            )
    )

pub @create_dir (path: str) -> Result<void, FileError> uses FileSystem =
    run(
        let wpath = prepare_win_path(path: path),
        let result = _CreateDirectoryW(lpPathName: wpath, lpSecurityAttributes: CPtr.null()),
        if result == 0 then
            Err(last_error_to_file_error(path: Path.from_str(s: path)))
        else
            Ok(())
    )

pub @create_dir_all (path: str) -> Result<void, FileError> uses FileSystem =
    run(
        let parts = Path.from_str(s: path).segments,
        let current = if Path.from_str(s: path).is_absolute() then
            // Windows absolute path starts with drive letter
            parts[0]
        else
            "",

        let start_idx = if Path.from_str(s: path).is_absolute() then 1 else 0,

        for i in start_idx..len(collection: parts) do
            run(
                current = current + "\\" + parts[i],
                if !exists(path: current) then
                    create_dir(path: current)?
            ),
        Ok(())
    )

pub @remove_dir (path: str) -> Result<void, FileError> uses FileSystem =
    run(
        let wpath = prepare_win_path(path: path),
        let result = _RemoveDirectoryW(lpPathName: wpath),
        if result == 0 then
            Err(last_error_to_file_error(path: Path.from_str(s: path)))
        else
            Ok(())
    )
```

### File Info

```ori
// std/fs/info_windows.ori
#target(os: "windows")
use "./ffi_windows" { ... }
use "./ffi_windows_string" { prepare_win_path }
use "./error_windows" { last_error_to_file_error }
use std.time { Instant }

pub @info (path: str) -> Result<FileInfo, FileError> uses FileSystem =
    run(
        let wpath = prepare_win_path(path: path),

        // Use GetFileAttributesExW for basic info
        let attrs = _GetFileAttributesW(lpFileName: wpath),
        if attrs == $INVALID_FILE_ATTRIBUTES then
            Err(last_error_to_file_error(path: Path.from_str(s: path)))
        else
            // Open file to get detailed info
            run(
                let handle = _CreateFileW(
                    lpFileName: wpath,
                    dwDesiredAccess: 0,  // No access needed for metadata
                    dwShareMode: $FILE_SHARE_READ | $FILE_SHARE_WRITE | $FILE_SHARE_DELETE,
                    lpSecurityAttributes: CPtr.null(),
                    dwCreationDisposition: $OPEN_EXISTING,
                    dwFlagsAndAttributes: $FILE_ATTRIBUTE_NORMAL,
                    hTemplateFile: CPtr.null()
                ),

                if handle == $INVALID_HANDLE_VALUE then
                    Err(last_error_to_file_error(path: Path.from_str(s: path)))
                else
                    run(
                        let file_info = BY_HANDLE_FILE_INFORMATION.zeroed(),
                        let info_ok = _GetFileInformationByHandle(
                            hFile: handle,
                            lpFileInformation: file_info
                        ),

                        _CloseHandle(hObject: handle),

                        if info_ok == 0 then
                            Err(last_error_to_file_error(path: Path.from_str(s: path)))
                        else
                            Ok(win32_to_file_info(path: path, attrs: attrs, info: file_info))
                    )
            )
    )

@win32_to_file_info (path: str, attrs: DWORD, info: BY_HANDLE_FILE_INFORMATION) -> FileInfo =
    FileInfo {
        path: Path.from_str(s: path),
        size: (info.nFileSizeHigh as int) << 32 | (info.nFileSizeLow as int),
        is_file: (attrs & $FILE_ATTRIBUTE_DIRECTORY) == 0,
        is_dir: (attrs & $FILE_ATTRIBUTE_DIRECTORY) != 0,
        is_symlink: (attrs & $FILE_ATTRIBUTE_REPARSE_POINT) != 0,
        modified: filetime_to_instant(ft: info.ftLastWriteTime),
        created: Some(filetime_to_instant(ft: info.ftCreationTime)),
        readonly: (attrs & $FILE_ATTRIBUTE_READONLY) != 0
    }

// Convert Windows FILETIME (100-nanosecond intervals since Jan 1, 1601) to Instant
@filetime_to_instant (ft: FILETIME) -> Instant =
    run(
        // FILETIME is 64-bit value split into two 32-bit parts
        let ticks = (ft.dwHighDateTime as int) << 32 | (ft.dwLowDateTime as int),
        // Difference between Windows epoch (1601) and Unix epoch (1970) in 100-ns intervals
        let windows_to_unix_ticks: int = 116444736000000000,
        let unix_ticks = ticks - windows_to_unix_ticks,
        // Convert 100-ns intervals to seconds
        let secs = unix_ticks / 10000000,
        Instant.from_unix_secs(secs: secs)
    )

pub @exists (path: str) -> bool uses FileSystem =
    run(
        let wpath = prepare_win_path(path: path),
        let attrs = _GetFileAttributesW(lpFileName: wpath),
        attrs != $INVALID_FILE_ATTRIBUTES
    )

pub @is_file (path: str) -> bool uses FileSystem =
    run(
        let wpath = prepare_win_path(path: path),
        let attrs = _GetFileAttributesW(lpFileName: wpath),
        attrs != $INVALID_FILE_ATTRIBUTES && (attrs & $FILE_ATTRIBUTE_DIRECTORY) == 0
    )

pub @is_dir (path: str) -> bool uses FileSystem =
    run(
        let wpath = prepare_win_path(path: path),
        let attrs = _GetFileAttributesW(lpFileName: wpath),
        attrs != $INVALID_FILE_ATTRIBUTES && (attrs & $FILE_ATTRIBUTE_DIRECTORY) != 0
    )
```

### Error Mapping

```ori
// std/fs/error_windows.ori
#target(os: "windows")
use "./ffi_windows" { _GetLastError, _FormatMessageW, _LocalFree, $FORMAT_MESSAGE_ALLOCATE_BUFFER, $FORMAT_MESSAGE_FROM_SYSTEM, $FORMAT_MESSAGE_IGNORE_INSERTS }
use "./ffi_windows_string" { utf16_to_utf8_null_terminated }

// Windows error codes
let $ERROR_FILE_NOT_FOUND: DWORD = 2
let $ERROR_PATH_NOT_FOUND: DWORD = 3
let $ERROR_ACCESS_DENIED: DWORD = 5
let $ERROR_INVALID_HANDLE: DWORD = 6
let $ERROR_NOT_ENOUGH_MEMORY: DWORD = 8
let $ERROR_INVALID_ACCESS: DWORD = 12
let $ERROR_INVALID_DATA: DWORD = 13
let $ERROR_OUTOFMEMORY: DWORD = 14
let $ERROR_FILE_EXISTS: DWORD = 80
let $ERROR_CANNOT_MAKE: DWORD = 82
let $ERROR_INVALID_PARAMETER: DWORD = 87
let $ERROR_BROKEN_PIPE: DWORD = 109
let $ERROR_DISK_FULL: DWORD = 112
let $ERROR_INVALID_NAME: DWORD = 123
let $ERROR_DIR_NOT_EMPTY: DWORD = 145
let $ERROR_ALREADY_EXISTS: DWORD = 183
let $ERROR_FILENAME_EXCED_RANGE: DWORD = 206
let $ERROR_DIRECTORY: DWORD = 267

@last_error_to_file_error (path: Path) -> FileError uses FFI =
    run(
        let err = _GetLastError(),
        let kind = match(err,
            $ERROR_FILE_NOT_FOUND -> NotFound,
            $ERROR_PATH_NOT_FOUND -> NotFound,
            $ERROR_ACCESS_DENIED -> PermissionDenied,
            $ERROR_INVALID_ACCESS -> PermissionDenied,
            $ERROR_FILE_EXISTS -> AlreadyExists,
            $ERROR_ALREADY_EXISTS -> AlreadyExists,
            $ERROR_DIR_NOT_EMPTY -> DirectoryNotEmpty,
            $ERROR_DIRECTORY -> NotAFile,
            $ERROR_INVALID_NAME -> InvalidPath,
            $ERROR_FILENAME_EXCED_RANGE -> InvalidPath,
            _ -> IoError
        ),
        FileError {
            kind: kind,
            path: path,
            message: format_win_error(err: err)
        }
    )

@format_win_error (err: DWORD) -> str uses FFI =
    run(
        let buffer_ptr: [CPtr, max 1] = [CPtr.null()],
        let len = _FormatMessageW(
            dwFlags: $FORMAT_MESSAGE_ALLOCATE_BUFFER | $FORMAT_MESSAGE_FROM_SYSTEM | $FORMAT_MESSAGE_IGNORE_INSERTS,
            lpSource: CPtr.null(),
            dwMessageId: err,
            dwLanguageId: 0,  // Default language
            lpBuffer: buffer_ptr.as_ptr(),
            nSize: 0,
            Arguments: CPtr.null()
        ),

        if len == 0 || buffer_ptr[0].is_null() then
            `Windows error {err}`
        else
            run(
                let msg = utf16_to_utf8_null_terminated(wstr: buffer_ptr[0]),
                _LocalFree(hMem: buffer_ptr[0]),
                // Remove trailing newlines
                msg.trim_end()
            )
    )
```

---

## FFI Struct Zeroed Methods

```ori
// std/fs/ffi_windows_zeroed.ori (internal)
#target(os: "windows")

impl FILETIME {
    @zeroed () -> FILETIME =
        FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 }
}

impl WIN32_FIND_DATAW {
    @zeroed () -> WIN32_FIND_DATAW =
        WIN32_FIND_DATAW {
            dwFileAttributes: 0,
            ftCreationTime: FILETIME.zeroed(),
            ftLastAccessTime: FILETIME.zeroed(),
            ftLastWriteTime: FILETIME.zeroed(),
            nFileSizeHigh: 0,
            nFileSizeLow: 0,
            dwReserved0: 0,
            dwReserved1: 0,
            cFileName: [0; 260],
            cAlternateFileName: [0; 14]
        }
}

impl BY_HANDLE_FILE_INFORMATION {
    @zeroed () -> BY_HANDLE_FILE_INFORMATION =
        BY_HANDLE_FILE_INFORMATION {
            dwFileAttributes: 0,
            ftCreationTime: FILETIME.zeroed(),
            ftLastAccessTime: FILETIME.zeroed(),
            ftLastWriteTime: FILETIME.zeroed(),
            dwVolumeSerialNumber: 0,
            nFileSizeHigh: 0,
            nFileSizeLow: 0,
            nNumberOfLinks: 0,
            nFileIndexHigh: 0,
            nFileIndexLow: 0
        }
}
```

---

## Build Configuration

```toml
# ori.toml
[native.windows]
libraries = ["kernel32"]
```

---

## Platform Selection

The std.fs module selects implementation based on target platform:

```ori
// std/fs/mod.ori

// POSIX implementation
#target(family: "unix")
use "./read_posix" { read, read_bytes }
use "./write_posix" { write, write_bytes, write_with }
use "./dir_posix" { list_dir, create_dir, create_dir_all, remove_dir }
use "./info_posix" { info, exists, is_file, is_dir }

// Windows implementation
#target(os: "windows")
use "./read_windows" { read, read_bytes }
use "./write_windows" { write, write_bytes, write_with }
use "./dir_windows" { list_dir, create_dir, create_dir_all, remove_dir }
use "./info_windows" { info, exists, is_file, is_dir }

// Common types (pure Ori, no FFI)
pub use "./types" { Path, FileInfo, FileError, FileErrorKind, WriteMode, Permissions }
pub use "./path" { Path }
```

---

## Differences from POSIX Implementation

| Aspect | POSIX | Windows |
|--------|-------|---------|
| String encoding | UTF-8 native | UTF-8 ↔ UTF-16 conversion |
| File handles | int (fd) | HANDLE (CPtr) |
| Error handling | errno + strerror | GetLastError + FormatMessage |
| Path separators | `/` only | `\` (also accepts `/`) |
| Extended paths | N/A | `\\?\` prefix for >260 chars |
| Creation time | Often unavailable | Always available |
| Permissions | Mode bits | Simplified (readonly flag only) |
| Case sensitivity | Usually sensitive | Usually insensitive |

---

## Future Considerations

### ACL Support

Windows Access Control Lists provide fine-grained permissions beyond the simple readonly flag. A future proposal could add:

```ori
type WinPermissions = {
    owner: Principal,
    acl: [AclEntry]
}

@get_acl (path: str) -> Result<WinPermissions, FileError> uses FileSystem
@set_acl (path: str, permissions: WinPermissions) -> Result<void, FileError> uses FileSystem
```

### Alternate Data Streams

NTFS supports alternate data streams (ADS):

```ori
// Read from alternate stream
read(path: "file.txt:stream_name")
```

### Junction Points and Reparse Points

Windows has multiple symlink-like features that could be exposed:

```ori
type ReparseType = Symlink | Junction | MountPoint

@create_junction (target: str, link: str) -> Result<void, FileError> uses FileSystem
```

---

## Summary

| Component | Implementation |
|-----------|----------------|
| File I/O | CreateFileW, ReadFile, WriteFile, CloseHandle |
| Directory ops | CreateDirectoryW, RemoveDirectoryW, FindFirstFileW/FindNextFileW |
| File info | GetFileAttributesW, GetFileInformationByHandle |
| Error handling | GetLastError, FormatMessageW |
| String encoding | Manual UTF-8 ↔ UTF-16 conversion |
| Extended paths | Automatic `\\?\` prefix for long paths |
| Permissions | Simplified to readonly flag (ACLs deferred) |
