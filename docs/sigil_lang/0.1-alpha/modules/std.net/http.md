# std.net.http

HTTP client and server.

```sigil
use std.net.http { Client, get, post, Server, Request, Response }
```

**Capability required:** `Network`

---

## Overview

The `std.net.http` module provides:

- HTTP client for making requests
- HTTP server for handling requests
- Request and response types
- Convenience functions (`get`, `post`)

---

## Client

### Client Type

```sigil
type Client = {
    timeout: Duration,
    headers: {str: str},
}
```

Reusable HTTP client with configuration.

```sigil
let client = Client.new()
    .timeout(30s)
    .header("User-Agent", "sigil/0.1")

let response = client.get("https://api.example.com/users")?
```

**Methods:**
- `new() -> Client` — Create client with defaults
- `timeout(d: Duration) -> Client` — Set request timeout
- `header(key: str, value: str) -> Client` — Add default header
- `get(url: str) -> async Result<Response, HttpError>` — GET request
- `post(url: str, body: str) -> async Result<Response, HttpError>` — POST request
- `put(url: str, body: str) -> async Result<Response, HttpError>` — PUT request
- `delete(url: str) -> async Result<Response, HttpError>` — DELETE request
- `request(req: Request) -> async Result<Response, HttpError>` — Custom request

---

### Convenience Functions

```sigil
@get (url: str) -> async Result<Response, HttpError>
@post (url: str, body: str) -> async Result<Response, HttpError>
```

Simple one-off requests.

```sigil
use std.net.http { get, post }

let resp = get("https://api.example.com/status").await?
let resp = post("https://api.example.com/data", json_body).await?
```

---

## Request & Response

### Request

```sigil
type Request = {
    method: Method,
    url: str,
    headers: {str: str},
    body: Option<str>,
}

type Method = GET | POST | PUT | DELETE | PATCH | HEAD | OPTIONS
```

```sigil
let req = Request {
    method: POST,
    url: "https://api.example.com/users",
    headers: {"Content-Type": "application/json"},
    body: Some(json_string),
}
```

---

### Response

```sigil
type Response = {
    status: int,
    headers: {str: str},
    body: str,
}
```

```sigil
let resp = get(url).await?

if resp.status == 200 then
    let data = parse<Data>(resp.body)?
else
    Err(HttpError.StatusError(resp.status))
```

**Methods:**
- `is_success() -> bool` — Status 200-299
- `is_redirect() -> bool` — Status 300-399
- `is_client_error() -> bool` — Status 400-499
- `is_server_error() -> bool` — Status 500-599
- `json<T>() -> Result<T, Error>` — Parse body as JSON

---

## Server

### Server Type

```sigil
type Server
```

HTTP server that handles requests.

```sigil
use std.net.http { Server, Request, Response }

@main () uses Network -> async Result<void, Error> = run(
    let server = Server.bind("0.0.0.0:8080")?,
    print("Server running on :8080"),
    server.serve(handle_request).await,
)

@handle_request (req: Request) -> Response = match(req.method,
    GET -> Response { status: 200, headers: {}, body: "Hello!" },
    _ -> Response { status: 405, headers: {}, body: "Method not allowed" },
)
```

**Methods:**
- `bind(addr: str) -> Result<Server, HttpError>` — Bind to address
- `serve(handler: Request -> Response) -> async Result<void, HttpError>` — Start serving

---

### HttpError

```sigil
type HttpError =
    | ConnectionFailed(str)
    | Timeout
    | InvalidUrl(str)
    | StatusError(int)
    | ParseError(str)
    | IoError(str)
```

---

## Examples

### Fetching JSON

```sigil
use std.net.http { get }
use std.json { parse }

type User = { id: int, name: str, email: str }

@fetch_user (id: int) uses Network -> async Result<User, Error> = run(
    let resp = get("https://api.example.com/users/" + str(id)).await?,
    if resp.status != 200 then
        Err(Error { message: "User not found", source: None })
    else
        parse<User>(resp.body),
)
```

### REST API server

```sigil
use std.net.http { Server, Request, Response }
use std.json { stringify }

@main () uses Network -> async Result<void, Error> = run(
    let server = Server.bind(":8080")?,
    server.serve(router).await,
)

@router (req: Request) -> Response = match((req.method, req.url),
    (GET, "/health") -> ok("healthy"),
    (GET, "/users") -> ok(stringify(get_users())),
    (POST, "/users") -> create_user(req.body),
    _ -> not_found(),
)

@ok (body: str) -> Response = Response {
    status: 200,
    headers: {"Content-Type": "application/json"},
    body: body
}

@not_found () -> Response = Response {
    status: 404,
    headers: {},
    body: "Not found"
}
```

---

## See Also

- [std.json](../std.json/) — JSON encoding/decoding
- [std.net](index.md) — Low-level networking
- [std.async](../std.async/) — Async patterns
