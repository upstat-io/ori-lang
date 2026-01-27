# std.compress

Compression and decompression.

```ori
use std.compress.gzip { compress, decompress }
use std.compress.zip { ZipArchive }
```

**No capability required** (in-memory operations)

---

## Overview

The `std.compress` module provides:

- Gzip compression/decompression
- Zlib compression/decompression
- Zip archive reading/writing

---

## Submodules

| Module | Description |
|--------|-------------|
| [gzip](gzip.md) | Gzip compression |
| [zlib](zlib.md) | Zlib compression |
| [zip](zip.md) | Zip archives |

---

## std.compress.gzip

### @compress

```ori
@compress (data: [byte]) -> [byte]
@compress (data: [byte], level: int) -> [byte]
```

Compresses data using gzip. Level 1-9 (default 6).

```ori
use std.compress.gzip

let compressed = gzip.compress(data)
let fast = gzip.compress(data, level: 1)
let best = gzip.compress(data, level: 9)
```

---

### @decompress

```ori
@decompress (data: [byte]) -> Result<[byte], CompressError>
```

Decompresses gzip data.

```ori
use std.compress.gzip

let original = gzip.decompress(compressed)?
```

---

## std.compress.zlib

### @compress / @decompress

Same interface as gzip, but uses zlib format.

```ori
use std.compress.zlib

let compressed = zlib.compress(data)
let original = zlib.decompress(compressed)?
```

---

## std.compress.zip

### ZipArchive

```ori
type ZipArchive
```

A zip archive for reading or writing.

**Reading:**

```ori
use std.compress.zip { ZipArchive }
use std.fs { read_bytes }

let data = read_bytes("archive.zip")?
let archive = ZipArchive.from_bytes(data)?

for entry in archive.entries() do
    print(entry.name + " (" + str(entry.size) + " bytes)")

let content = archive.read("file.txt")?
```

**Writing:**

```ori
use std.compress.zip { ZipArchive }

let archive = ZipArchive.new()
archive.add("hello.txt", "Hello, world!".as_bytes())
archive.add("data/config.json", json_bytes)

let zip_bytes = archive.to_bytes()
```

**Methods:**
- `from_bytes(data: [byte]) -> Result<ZipArchive, CompressError>` — Open archive
- `new() -> ZipArchive` — Create new archive
- `entries() -> [ZipEntry]` — List entries
- `read(name: str) -> Result<[byte], CompressError>` — Read entry
- `add(name: str, data: [byte])` — Add entry
- `to_bytes() -> [byte]` — Serialize archive

---

### ZipEntry

```ori
type ZipEntry = {
    name: str,
    size: Size,
    compressed_size: Size,
    is_dir: bool,
}
```

---

## Types

### CompressError

```ori
type CompressError =
    | InvalidData(str)
    | EntryNotFound(name: str)
    | IoError(str)
```

---

## Examples

### Compress file

```ori
use std.compress.gzip { compress }
use std.fs { read_bytes, write_bytes }

@compress_file (src: str, dst: str) uses FileSystem -> Result<void, Error> = run(
    let data = read_bytes(src)?,
    let compressed = compress(data),
    write_bytes(dst, compressed),
)
```

### Extract zip

```ori
use std.compress.zip { ZipArchive }
use std.fs { read_bytes, write_bytes, create_dir_all }

@extract_zip (zip_path: str, dest: str) uses FileSystem -> Result<void, Error> = run(
    let data = read_bytes(zip_path)?,
    let archive = ZipArchive.from_bytes(data)?,

    for entry in archive.entries() do
        if !entry.is_dir then run(
            let content = archive.read(entry.name)?,
            let path = dest + "/" + entry.name,
            create_dir_all(parent(path))?,
            write_bytes(path, content)?,
        ),
    Ok(()),
)
```

---

## See Also

- [std.fs](../std.fs/) — File operations
- [std.io](../std.io/) — Stream compression
