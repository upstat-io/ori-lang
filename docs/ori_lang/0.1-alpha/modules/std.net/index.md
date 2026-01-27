# std.net

Networking primitives for TCP, UDP, and HTTP.

```ori
use std.net { TcpListener, TcpStream }
use std.net.http { Client, get, post }
```

**Capability required:** `Network`

---

## Overview

The `std.net` module provides:

- TCP client and server
- UDP sockets
- HTTP client and server (in `std.net.http`)
- URL parsing

---

## Submodules

| Module | Description |
|--------|-------------|
| [std.net.http](http.md) | HTTP client and server |
| [std.net.url](url.md) | URL parsing and building |

---

## Types

### TcpListener

```ori
type TcpListener
```

A TCP socket server, listening for connections.

```ori
use std.net { TcpListener }

let listener = TcpListener.bind("127.0.0.1:8080")?
for conn in listener.incoming() do
    handle(conn)
```

**Methods:**
- `bind(addr: str) -> Result<TcpListener, NetError>` — Bind to address
- `incoming() -> Iterator<Result<TcpStream, NetError>>` — Accept connections
- `accept() -> Result<TcpStream, NetError>` — Accept single connection
- `local_addr() -> Result<str, NetError>` — Local address

---

### TcpStream

```ori
type TcpStream: Reader + Writer
```

A TCP connection.

```ori
use std.net { TcpStream }

let conn = TcpStream.connect("example.com:80")?
conn.write_str("GET / HTTP/1.1\r\n\r\n")?
let response = conn.read_to_string()?
```

**Methods:**
- `connect(addr: str) -> Result<TcpStream, NetError>` — Connect to address
- `peer_addr() -> Result<str, NetError>` — Remote address
- `local_addr() -> Result<str, NetError>` — Local address
- `shutdown() -> Result<void, NetError>` — Close connection
- `set_timeout(timeout: Duration) -> Result<void, NetError>` — Set read/write timeout

---

### UdpSocket

```ori
type UdpSocket
```

A UDP socket.

```ori
use std.net { UdpSocket }

let socket = UdpSocket.bind("0.0.0.0:0")?
socket.send_to("hello".as_bytes(), "127.0.0.1:8080")?
let (data, addr) = socket.recv_from()?
```

**Methods:**
- `bind(addr: str) -> Result<UdpSocket, NetError>` — Bind to address
- `send_to(data: [byte], addr: str) -> Result<int, NetError>` — Send datagram
- `recv_from() -> Result<([byte], str), NetError>` — Receive datagram

---

### NetError

```ori
type NetError =
    | ConnectionRefused(addr: str)
    | ConnectionReset
    | ConnectionTimeout
    | AddrInUse(addr: str)
    | AddrNotAvailable(addr: str)
    | HostUnreachable(host: str)
    | NetworkUnreachable
    | InvalidAddr(addr: str)
    | IoError(str)
```

---

## Functions

### @resolve

```ori
@resolve (host: str) -> Result<[str], NetError>
```

Resolves hostname to IP addresses.

```ori
use std.net { resolve }

let addrs = resolve("example.com")?
// ["93.184.216.34", "2606:2800:220:1:248:1893:25c8:1946"]
```

---

## Examples

### Simple TCP echo server

```ori
use std.net { TcpListener }
use std.io { copy }

@echo_server (addr: str) uses Network -> Result<Never, NetError> = run(
    let listener = TcpListener.bind(addr)?,
    print("Listening on " + addr),
    for conn in listener.incoming() do run(
        let stream = conn?,
        copy(stream, stream)?,  // echo back
    ),
)
```

### TCP client

```ori
use std.net { TcpStream }

@fetch (host: str, port: int) uses Network -> Result<str, NetError> = run(
    let conn = TcpStream.connect(host + ":" + str(port))?,
    conn.write_str("GET / HTTP/1.0\r\nHost: " + host + "\r\n\r\n")?,
    conn.read_to_string(),
)
```

---

## See Also

- [std.net.http](http.md) — HTTP client/server
- [std.io](../std.io/) — I/O traits
- [std.async](../std.async/) — Async networking
