# std.net.http

HTTP client and server.

```ori
use std.net.http { Client, get, post, Server, Request, Response }
```

**Capability required:** `Http`, `Async`

---

## Overview

The `std.net.http` module provides:

- HTTP client for making requests
- HTTP server for handling requests
- Request and response types
- Convenience functions (`get`, `post`)

---

## The Http Capability

```ori
trait Http {
    @get (url: str) -> Result<Response, HttpError>
    @post (url: str, body: str) -> Result<Response, HttpError>
    @put (url: str, body: str) -> Result<Response, HttpError>
    @delete (url: str) -> Result<Response, HttpError>
    @request (req: Request) -> Result<Response, HttpError>
}
```

The `Http` capability represents the ability to make HTTP requests. Functions that perform HTTP operations must declare `uses Http` in their signature.

```ori
@fetch_data (url: str) -> Result<Data, Error> uses Http, Async =
    Http.get(url)?.json()
```

**Implementations:**

| Type | Description |
|------|-------------|
| `Client` | Production HTTP client (default) |
| `MockHttp` | Test mock with configurable responses |

### MockHttp

For testing, create a mock implementation:

```ori
type MockHttp = {
    responses: {str: Response},
}

impl Http for MockHttp {
    @get (url: str) -> Result<Response, HttpError> =
        match(self.responses.get(url),
            Some(resp) -> Ok(resp),
            None -> Err(HttpError.ConnectionFailed("No mock for: " + url)),
        )

    @post (url: str, body: str) -> Result<Response, HttpError> = self.get(url)
    @put (url: str, body: str) -> Result<Response, HttpError> = self.get(url)
    @delete (url: str) -> Result<Response, HttpError> = self.get(url)
    @request (req: Request) -> Result<Response, HttpError> = self.get(req.url)
}
```

```ori
@test_fetch tests @fetch_data () -> void =
    with Http = MockHttp {
        responses: {"https://api.example.com/data": Response { status: 200, headers: {}, body: "{}" }}
    } in
    run(
        let result = fetch_data("https://api.example.com/data"),
        assert(is_ok(result)),
    )
```

---

## Client

### Client Type

```ori
type Client = {
    timeout: Duration,
    headers: {str: str},
}
```

Reusable HTTP client with configuration.

```ori
let client = Client.new()
    .timeout(30s)
    .header("User-Agent", "ori/0.1")

let response = client.get("https://api.example.com/users")?
```

**Methods:**
- `new() -> Client` — Create client with defaults
- `timeout(d: Duration) -> Client` — Set request timeout
- `header(key: str, value: str) -> Client` — Add default header
- `get(url: str) -> Result<Response, HttpError> uses Http, Async` — GET request
- `post(url: str, body: str) -> Result<Response, HttpError> uses Http, Async` — POST request
- `put(url: str, body: str) -> Result<Response, HttpError> uses Http, Async` — PUT request
- `delete(url: str) -> Result<Response, HttpError> uses Http, Async` — DELETE request
- `request(req: Request) -> Result<Response, HttpError> uses Http, Async` — Custom request

---

### Convenience Functions

```ori
@get (url: str) -> Result<Response, HttpError> uses Http, Async
@post (url: str, body: str) -> Result<Response, HttpError> uses Http, Async
```

Simple one-off requests.

```ori
use std.net.http { get, post }

let resp = get("https://api.example.com/status")?
let resp = post("https://api.example.com/data", json_body)?
```

---

## Request & Response

### Request

```ori
type Request = {
    method: Method,
    url: str,
    headers: {str: str},
    body: Option<str>,
}

type Method = GET | POST | PUT | DELETE | PATCH | HEAD | OPTIONS
```

```ori
let req = Request {
    method: POST,
    url: "https://api.example.com/users",
    headers: {"Content-Type": "application/json"},
    body: Some(json_string),
}
```

---

### Response

```ori
type Response = {
    status: int,
    headers: {str: str},
    body: str,
}
```

```ori
let resp = get(url)?

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

```ori
type Server
```

HTTP server that handles requests.

```ori
use std.net.http { Server, Request, Response }

@main () -> Result<void, Error> uses Http, Async = run(
    let server = Server.bind("0.0.0.0:8080")?,
    print("Server running on :8080"),
    server.serve(handle_request),
)

@handle_request (req: Request) -> Response = match(req.method,
    GET -> Response { status: 200, headers: {}, body: "Hello!" },
    _ -> Response { status: 405, headers: {}, body: "Method not allowed" },
)
```

**Methods:**
- `bind(addr: str) -> Result<Server, HttpError>` — Bind to address
- `serve(handler: Request -> Response) -> Result<void, HttpError> uses Http, Async` — Start serving

---

### HttpError

```ori
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

```ori
use std.net.http { get }
use std.json { parse }

type User = { id: int, name: str, email: str }

@fetch_user (id: int) -> Result<User, Error> uses Http, Async = run(
    let resp = get("https://api.example.com/users/" + str(id))?,
    if resp.status != 200 then
        Err(Error { message: "User not found", source: None })
    else
        parse<User>(resp.body),
)
```

### REST API server

```ori
use std.net.http { Server, Request, Response }
use std.json { stringify }

@main () -> Result<void, Error> uses Http, Async = run(
    let server = Server.bind(":8080")?,
    server.serve(router),
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
