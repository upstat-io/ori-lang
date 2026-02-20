---
title: "Capabilities"
description: "Dependency injection, custom capabilities, and testing."
order: 13
part: "Effects and Concurrency"
---

# Capabilities

Most programming languages let any function do anything — read files, make network requests, print output. When effects are hidden, code becomes hard to test, hard to understand, and hard to reason about.

Ori takes a different approach: **effects are tracked explicitly**. Functions declare their dependencies, and callers provide implementations. This isn't a security boundary — it's a **dependency injection system** that makes effects visible in type signatures and trivially mockable in tests.

## The Problem with Hidden Dependencies

Consider this function signature in a typical language:

```python
def calculate_price(item_id):
    item = fetch_from_database(item_id)  # Hidden database dependency
    log.info(f"Calculating price for {item}")  # Hidden logging dependency
    price = item.base_price * 1.2
    cache.set(item_id, price)  # Hidden cache dependency
    return price
```

Looking at `calculate_price(item_id)`, you'd expect a simple calculation. But it actually depends on:
- A database connection
- A logging system
- A cache service

Testing this requires setting up or mocking three different systems. You can't tell from the signature what this function needs to work.

## Capabilities: Dependencies Made Explicit

In Ori, that same function declares its dependencies:

```ori
@calculate_price (item_id: int) -> Result<float, Error>
    uses Database, Logger, Cache = {
    let item = Database.get(table: "items", id: item_id)?
    Logger.info(msg: `Calculating price for {item.name}`)
    let price = item.base_price * 1.2
    Cache.set(key: item_id as str, value: price)
    Ok(price)
}
```

The `uses Database, Logger, Cache` clause tells you exactly what dependencies this function requires. No hidden effects.

## What Capabilities Are (and Aren't)

**Capabilities are a dependency tracking system**, not a security sandbox. The compiler doesn't prevent you from making syscalls or accessing the network. What it does:

1. **Makes dependencies visible** — The type signature shows what a function needs
2. **Enables dependency injection** — Callers provide implementations
3. **Tracks propagation** — If A calls B which uses Http, A must declare or provide Http
4. **Enables mocking** — Tests inject mock implementations

What capabilities are NOT:
- Not a security boundary (you can still call external C libraries)
- Not capability-based security (this is dependency injection, not object-capability model)
- Not runtime enforcement (it's compile-time type checking)

The value is **visibility and testability**, not prevention.

## How Capabilities Connect to the Real World

A natural question: if capabilities are just traits, what actually performs the I/O?

**Standard capabilities have runtime-provided implementations.** You don't implement `RealHttp` yourself — the Ori runtime provides it. When you write:

```ori
with Http = RealHttp { base_url: "https://api.example.com" } in
    Http.get(url: "/users/1")
```

`RealHttp` is a type provided by the runtime with a built-in implementation that actually opens sockets and sends HTTP requests. The "real" implementations are native code (Rust, syscalls) that the Ori runtime includes.

**You don't write the low-level I/O code.** The standard implementations are:

| Capability | Runtime Implementation | What It Does |
|------------|----------------------|--------------|
| `Http` | `RealHttp` | Actual HTTP over TCP |
| `FileSystem` | `RealFileSystem` | Actual file syscalls |
| `Clock` | `RealClock` | System time |
| `Random` | `RealRandom` | OS random number generator |
| `Print` | `StdoutPrint` | Writes to stdout |

**Custom capabilities compose standard ones.** When you create your own capability like `PaymentProcessor`, you don't implement raw network I/O — you use `Http`:

```ori
impl PaymentProcessor for StripeProcessor {
    @charge (customer_id: str, amount: float) -> Result<Receipt, PaymentError> uses Http = {
        // Uses the Http capability — doesn't do raw socket I/O
        let response = Http.post(
            url: `{self.base_url}/charges`
            body: `{"customer": "{customer_id}", "amount": {amount}}`
        )?
        parse_receipt(json: response.body)
    }
}
```

Your custom capability needs `Http`, so when you provide `PaymentProcessor`, you also need to provide `Http`:

```ori
with Http = RealHttp { base_url: "https://api.stripe.com" },
     PaymentProcessor = StripeProcessor { base_url: "https://api.stripe.com" } in
    process_purchase(customer_id: "cust_123", cart: cart)
```

**The capability chain always bottoms out at runtime-provided implementations** for actual I/O.

## Capabilities Are Traits

When you write `uses Http`, you're saying "this function requires something that implements the `Http` trait."

The standard capabilities like `Http`, `FileSystem`, and `Logger` are all just trait definitions:

```ori
trait Http {
    @get (url: str) -> Result<Response, Error>
    @post (url: str, body: str) -> Result<Response, Error>
    @put (url: str, body: str) -> Result<Response, Error>
    @delete (url: str) -> Result<Response, Error>
}

trait FileSystem {
    @read (path: str) -> Result<str, Error>
    @write (path: str, content: str) -> Result<void, Error>
    @exists (path: str) -> bool
    @delete (path: str) -> Result<void, Error>
}

trait Logger {
    @debug (msg: str) -> void
    @info (msg: str) -> void
    @warn (msg: str) -> void
    @error (msg: str) -> void
}
```

This means you can create your own capabilities by defining your own traits.

## Declaring Capabilities

### Basic Syntax

```ori
@fetch_user (id: int) -> Result<User, Error> uses Http =
    Http.get(url: `/api/users/{id}`)
```

Breaking this down:
- `@fetch_user` — the function name
- `(id: int)` — takes a user ID
- `-> Result<User, Error>` — returns a user or an error
- `uses Http` — requires the HTTP capability
- `Http.get(...)` — uses the capability to make a request

### Multiple Capabilities

```ori
@process_order (order_id: int) -> Result<Receipt, Error>
    uses Database, Logger, Email = {
    Logger.info(msg: `Processing order {order_id}`)
    let order = Database.get(table: "orders", id: order_id)?
    let receipt = calculate_receipt(order: order)
    Email.send(to: order.customer_email, subject: "Receipt", body: receipt.to_str())
    Ok(receipt)
}
```

## Providing Capabilities

### The `with...in` Expression

When calling a function that uses capabilities, provide them with `with...in`:

```ori
@main () -> void = {
    let user = with Http = RealHttp { base_url: "https://api.example.com" } in
        fetch_user(id: 42)

    match user {
        Ok(u) -> print(msg: `Found user: {u.name}`)
        Err(e) -> print(msg: `Error: {e}`)
    }
}
```

### Multiple Capabilities

```ori
with Http = RealHttp { base_url: "https://api.example.com" },
     Logger = FileLogger { path: "/var/log/app.log" },
     Database = PostgresDatabase { connection_string: "..." } in
    process_order(order_id: 123)
```

### Capability Propagation

If your function calls another function that uses a capability, you have two choices:

**Option 1: Propagate the capability**

```ori
@get_user_name (id: int) -> Result<str, Error> uses Http = {
    let user = fetch_user(id: id)?
    Ok(user.name)
}
```

Now `get_user_name` also requires `Http`. The caller must provide it.

**Option 2: Provide the capability internally**

```ori
@get_user_name_hardcoded (id: int) -> Result<str, Error> = {
    let user = with Http = RealHttp { base_url: "https://api.example.com" } in
        fetch_user(id: id)?
    Ok(user.name)
}
```

This function provides its own `Http` implementation, so it doesn't require the capability.

## Defining Custom Capabilities

Since capabilities are just traits, you create custom capabilities by defining traits:

### Step 1: Define the Trait

```ori
trait PaymentProcessor {
    @charge (customer_id: str, amount: float) -> Result<Receipt, PaymentError>
    @refund (transaction_id: str) -> Result<void, PaymentError>
    @get_balance (customer_id: str) -> Result<float, PaymentError>
}
```

### Step 2: Use It in Functions

```ori
@process_purchase (customer_id: str, cart: Cart) -> Result<Receipt, PaymentError>
    uses PaymentProcessor, Logger = {
    let total = calculate_total(cart: cart)
    Logger.info(msg: `Charging {customer_id} for {total}`)
    PaymentProcessor.charge(customer_id: customer_id, amount: total)
}
```

### Step 3: Create Implementations

Create a real implementation for production. Note that it uses `Http` internally — real implementations compose standard capabilities:

```ori
type StripeProcessor = {
    api_key: str,
    base_url: str,
}

impl PaymentProcessor for StripeProcessor {
    // Note: this impl uses Http — it doesn't do raw I/O itself
    @charge (customer_id: str, amount: float) -> Result<Receipt, PaymentError> uses Http = {
        let response = Http.post(
            url: `{self.base_url}/v1/charges`
            body: `{"customer": "{customer_id}", "amount": {amount}}`
        ).map_err(transform: e -> PaymentError { message: e.to_str() })?

        // Parse the JSON response
        let data = parse_stripe_response(json: response.body)?
        Ok(Receipt {
            transaction_id: data.id
            amount: amount
            timestamp: data.created
        })
    }

    @refund (transaction_id: str) -> Result<void, PaymentError> uses Http = {
        Http.post(
            url: `{self.base_url}/v1/refunds`
            body: `{"charge": "{transaction_id}"}`
        ).map_err(transform: e -> PaymentError { message: e.to_str() })?
        Ok(())
    }

    @get_balance (customer_id: str) -> Result<float, PaymentError> uses Http = {
        let response = Http.get(
            url: `{self.base_url}/v1/customers/{customer_id}/balance`
        ).map_err(transform: e -> PaymentError { message: e.to_str() })?
        parse_balance(json: response.body)
    }
}
```

Create a mock for testing:

```ori
type MockPaymentProcessor = {
    responses: {str: Result<Receipt, PaymentError>},
    balance: float,
}

impl PaymentProcessor for MockPaymentProcessor {
    // No "uses Http" — mock doesn't do real I/O, just returns canned data
    @charge (customer_id: str, amount: float) -> Result<Receipt, PaymentError> =
        self.responses[customer_id].unwrap_or(default: Ok(Receipt {
            transaction_id: "mock-txn-123",
            amount: amount,
            timestamp: 0,
        }))

    @refund (transaction_id: str) -> Result<void, PaymentError> = Ok(())

    @get_balance (customer_id: str) -> Result<float, PaymentError> =
        Ok(self.balance)
}
```

Notice the mock doesn't declare `uses Http` — it returns canned responses without doing any real I/O. This is why tests don't need to provide `Http` when using mocks.

### Step 4: Provide in Production and Tests

Production — note you must provide both `Http` (for the real I/O) and `PaymentProcessor`:

```ori
@main () -> void uses Env = {
    let processor = StripeProcessor {
        api_key: Env.get(name: "STRIPE_API_KEY").unwrap_or(default: "")
        base_url: "https://api.stripe.com"
    }

    // Must provide Http because StripeProcessor uses it internally
    with Http = RealHttp {}
         PaymentProcessor = processor in {
        let result = process_purchase(customer_id: "cust_123", cart: shopping_cart)
        match result {
            Ok(receipt) -> print(msg: `Payment successful: {receipt.transaction_id}`)
            Err(e) -> print(msg: `Payment failed: {e.message}`)
        }
    }
}
```

Tests:

```ori
@test_process_purchase tests @process_purchase () -> void =
    with PaymentProcessor = MockPaymentProcessor {
        responses: {},
        balance: 100.0,
    },
    Logger = MockLogger {} in {
        let cart = Cart { items: [Item { price: 29.99 }] }
        let result = process_purchase(customer_id: "test-customer", cart: cart)
        assert_ok(result: result)
    }

@test_payment_failure tests @process_purchase () -> void =
    with PaymentProcessor = MockPaymentProcessor {
        responses: {
            "test-customer": Err(PaymentError { message: "Card declined" }),
        },
        balance: 0.0,
    },
    Logger = MockLogger {} in {
        let cart = Cart { items: [Item { price: 29.99 }] }
        let result = process_purchase(customer_id: "test-customer", cart: cart)
        assert_err(result: result)
    }
```

## Capability Design Patterns

### Domain-Specific Capabilities

Create capabilities that match your domain:

```ori
// E-commerce domain
trait Inventory {
    @check_stock (sku: str) -> Result<int, Error>
    @reserve (sku: str, quantity: int) -> Result<Reservation, Error>
    @release (reservation_id: str) -> Result<void, Error>
}

trait Shipping {
    @calculate_cost (destination: Address, weight: float) -> Result<float, Error>
    @create_label (order: Order) -> Result<ShippingLabel, Error>
    @track (tracking_number: str) -> Result<TrackingInfo, Error>
}

// Healthcare domain
trait PatientRecords {
    @get_patient (id: str) -> Result<Patient, Error>
    @update_record (id: str, record: MedicalRecord) -> Result<void, Error>
    @get_history (id: str) -> Result<[MedicalRecord], Error>
}

trait Prescriptions {
    @create (patient_id: str, medication: Medication) -> Result<Prescription, Error>
    @verify (prescription_id: str) -> Result<bool, Error>
    @fill (prescription_id: str) -> Result<void, Error>
}
```

### Layered Capabilities

Build higher-level capabilities from lower-level ones:

```ori
// Low-level: raw HTTP
trait Http {
    @get (url: str) -> Result<Response, Error>
    @post (url: str, body: str) -> Result<Response, Error>
}

// Mid-level: typed API client
trait UserApi {
    @get_user (id: int) -> Result<User, ApiError>
    @create_user (user: CreateUserRequest) -> Result<User, ApiError>
    @update_user (id: int, updates: UserUpdates) -> Result<User, ApiError>
}

// Implementation bridges the levels
type RealUserApi = { base_url: str }

impl UserApi for RealUserApi {
    @get_user (id: int) -> Result<User, ApiError> uses Http = {
        let response = Http.get(url: `{self.base_url}/users/{id}`)?
        parse_user(json: response.body)
    }
    // ... other methods
}
```

### Capability Composition

Combine capabilities to build complex operations:

```ori
@complete_order (order_id: int) -> Result<OrderConfirmation, Error>
    uses Database, Inventory, PaymentProcessor, Shipping, Email, Logger = {

    Logger.info(msg: `Processing order {order_id}`)

    // Fetch order
    let order = Database.get(table: "orders", id: order_id)?

    // Check and reserve inventory
    for item in order.items do {
        let stock = Inventory.check_stock(sku: item.sku)?
        if stock < item.quantity then
            Err(Error { message: `Insufficient stock for {item.sku}` })?
        Inventory.reserve(sku: item.sku, quantity: item.quantity)?
    }

    // Process payment
    let receipt = PaymentProcessor.charge(
        customer_id: order.customer_id
        amount: order.total
    )?

    // Create shipping label
    let label = Shipping.create_label(order: order)?

    // Update order status
    Database.update(
        table: "orders"
        id: order_id
        data: { status: "shipped", tracking: label.tracking_number }
    )?

    // Send confirmation email
    Email.send(
        to: order.customer_email
        subject: "Order Shipped!"
        body: `Your order {order_id} is on its way. Track it: {label.tracking_number}`
    )?

    Logger.info(msg: `Order {order_id} completed successfully`)

    Ok(OrderConfirmation {
        order_id: order_id
        receipt: receipt
        tracking_number: label.tracking_number
    })
}
```

### Stateful vs Stateless Capabilities

**Stateless** — each call is independent:

```ori
trait Random {
    @rand_int (min: int, max: int) -> int
    @rand_float () -> float
}
```

**Stateful** — maintains state across calls:

```ori
trait Counter {
    @increment () -> int
    @decrement () -> int
    @get () -> int
    @reset () -> void
}

type InMemoryCounter = { value: int }

impl Counter for InMemoryCounter {
    @increment () -> int = {
        self.value = self.value + 1
        self.value
    }

    @decrement () -> int = {
        self.value = self.value - 1
        self.value
    }

    @get () -> int = self.value

    @reset () -> void = {
        self.value = 0
    }
}
```

## Standard Capabilities

Ori provides these built-in capabilities:

### Http — Network Requests

```ori
trait Http {
    @get (url: str) -> Result<Response, Error>
    @post (url: str, body: str) -> Result<Response, Error>
    @put (url: str, body: str) -> Result<Response, Error>
    @delete (url: str) -> Result<Response, Error>
    @patch (url: str, body: str) -> Result<Response, Error>
    @head (url: str) -> Result<Response, Error>
}
```

```ori
@fetch_data (url: str) -> Result<str, Error> uses Http = {
    let response = Http.get(url: url)?
    Ok(response.body)
}
```

### FileSystem — File Operations

```ori
trait FileSystem {
    @read (path: str) -> Result<str, Error>
    @write (path: str, content: str) -> Result<void, Error>
    @exists (path: str) -> bool
    @delete (path: str) -> Result<void, Error>
    @list_dir (path: str) -> Result<[str], Error>
    @create_dir (path: str) -> Result<void, Error>
}
```

```ori
@read_config () -> Result<Config, Error> uses FileSystem = {
    let contents = FileSystem.read(path: "config.json")?
    parse_config(json: contents)
}
```

### Clock — Time Operations

```ori
trait Clock {
    @now () -> DateTime
    @today () -> Date
    @elapsed_since (start: DateTime) -> Duration
}
```

```ori
@log_with_timestamp (msg: str) -> void uses Clock, Print = {
    let now = Clock.now()
    Print.println(msg: `[{now}] {msg}`)
}
```

### Random — Random Numbers

```ori
trait Random {
    @rand_int (min: int, max: int) -> int
    @rand_float () -> float
    @rand_bool () -> bool
}
```

```ori
@roll_dice () -> int uses Random =
    Random.rand_int(min: 1, max: 6)
```

### Logger — Structured Logging

```ori
trait Logger {
    @debug (msg: str) -> void
    @info (msg: str) -> void
    @warn (msg: str) -> void
    @error (msg: str) -> void
}
```

### Cache — Caching

```ori
trait Cache {
    @get (key: str) -> Option<str>
    @set (key: str, value: str, ttl: Duration) -> void
    @delete (key: str) -> void
    @exists (key: str) -> bool
}
```

### Env — Environment Variables

```ori
trait Env {
    @get (name: str) -> Option<str>
}
```

### Print — Console Output

```ori
trait Print {
    @print (msg: str) -> void
    @println (msg: str) -> void
    @output () -> str
    @clear () -> void
}
```

`Print` is special — it has a default implementation. You can use `print` without declaring it:

```ori
// This works without "uses Print"
@main () -> void = print(msg: "Hello, World!")
```

## The Async Capability

`Async` is a marker capability that indicates a function may suspend (perform non-blocking I/O):

```ori
@fetch_many (urls: [str]) -> [Result<str, Error>] uses Http, Async =
    parallel(
        tasks: for url in urls yield () -> Http.get(url: url),
        max_concurrent: 10,
        timeout: 30s,
    )
```

Unlike other languages:
- There's no `async/await` syntax
- You don't mark individual expressions as async
- The capability declaration is sufficient

Functions with `Async` can call functions without it, but not vice versa (unless you provide a blocking implementation).

## Pure Functions

Functions without `uses` are **pure** — they have no side effects:

```ori
@double (x: int) -> int = x * 2

@greet (name: str) -> str = `Hello, {name}!`

@sum (numbers: [int]) -> int =
    numbers.iter().fold(initial: 0, op: (acc, n) -> acc + n)
```

Pure functions are powerful:
- **Always return the same output for the same input** — no hidden state
- **Can be safely parallelized** — no race conditions possible
- **Trivially testable** — no mocking needed
- **Cacheable** — results can be memoized

Prefer pure functions when possible. Push effects to the edges of your program.

## Testing with Capabilities

One of the biggest benefits of capabilities is easy testing:

```ori
@fetch_user_profile (id: int) -> Result<UserProfile, Error> uses Http = {
    let user = Http.get(url: `/api/users/{id}`)?
    let posts = Http.get(url: `/api/users/{id}/posts`)?
    Ok(UserProfile { user, posts })
}

@test_fetch_profile tests @fetch_user_profile () -> void =
    with Http = MockHttp {
        responses: {
            "/api/users/1": `{"id": 1, "name": "Alice"}`,
            "/api/users/1/posts": `[{"title": "Hello"}]`,
        },
    } in {
        let result = fetch_user_profile(id: 1)
        assert_ok(result: result)
        match result {
            Ok(profile) -> {
                assert_eq(actual: profile.user.name, expected: "Alice")
                assert_eq(actual: len(collection: profile.posts), expected: 1)
            }
            Err(_) -> panic(msg: "Expected Ok")
        }
    }
```

No network calls, no test databases, no flaky tests.

### Testing Custom Capabilities

When you create a custom capability, also create a mock:

```ori
// Production implementation
type RealEmailService = { smtp_host: str, api_key: str }

impl Email for RealEmailService {
    @send (to: str, subject: str, body: str) -> Result<void, Error> = {
        // Actual SMTP/API call
        smtp_send(host: self.smtp_host, key: self.api_key, to: to, subject: subject, body: body)
    }
}

// Test mock
type MockEmail = {
    sent: [{to: str, subject: str, body: str}],
    should_fail: bool,
}

impl Email for MockEmail {
    @send (to: str, subject: str, body: str) -> Result<void, Error> = {
        if self.should_fail then
            Err(Error { message: "Mock email failure" })
        else {
            self.sent = [...self.sent, {to, subject, body}]
            Ok(())
        }
    }
}

// Test verifies emails were "sent"
@test_sends_confirmation tests @process_order () -> void = {
    let mock_email = MockEmail { sent: [], should_fail: false }

    with Email = mock_email
         Database = MockDatabase { ... }
         Logger = MockLogger {} in {
        let result = process_order(order_id: 123)
        assert_ok(result: result)
        assert_eq(actual: len(collection: mock_email.sent), expected: 1)
        assert(condition: mock_email.sent[0].subject.contains(substring: "Confirmation"))
    }
}
```

## Best Practices

### Push Effects to the Edges

Keep most of your code pure. Handle effects at the boundaries:

```ori
// BAD: Effects scattered throughout
@process_order (id: int) -> Result<void, Error> uses Database, Logger, Email = {
    Logger.info(msg: "Starting"),           // Effect in middle
    let order = Database.get(id: id)?,      // Effect
    let total = calculate_total(order: order),  // Pure
    Database.update(order: order)?,         // Effect
    Email.send(to: order.customer)?,        // Effect
    Ok(())
}

// GOOD: Effects at boundaries, pure core
@calculate_total (order: Order) -> float = ...  // Pure
@validate_order (order: Order) -> Result<Order, str> = ...  // Pure

@process_order (id: int) -> Result<void, Error> uses Database, Logger, Email = {
    Logger.info(msg: "Starting")

    // Fetch data (effect boundary)
    let order = Database.get(id: id)?

    // Pure processing
    let validated = validate_order(order: order)
        .map_err(transform: e -> Error { message: e })?
    let total = calculate_total(order: validated)

    // Save data (effect boundary)
    Database.update(order: validated)?
    Email.send(to: order.customer)?

    Ok(())
}
```

### Use Specific Capabilities

Declare only the capabilities you need:

```ori
// BAD: Too broad
@process () -> void uses Http, FileSystem, Database, Cache, Logger = ...

// GOOD: Specific to actual needs
@process () -> void uses Database, Logger = ...
```

### Name Capabilities by Domain, Not Implementation

```ori
// BAD: Implementation-specific
trait StripePayments { ... }
trait PostgresDatabase { ... }

// GOOD: Domain-focused
trait PaymentProcessor { ... }
trait Database { ... }
```

This makes it easy to swap implementations without changing your function signatures.

### Keep Capability Interfaces Minimal

```ori
// BAD: Too many methods, some rarely used
trait Database {
    @get (table: str, id: int) -> Result<Row, Error>
    @query (sql: str) -> Result<[Row], Error>
    @insert (table: str, data: Row) -> Result<int, Error>
    @update (table: str, id: int, data: Row) -> Result<void, Error>
    @delete (table: str, id: int) -> Result<void, Error>
    @begin_transaction () -> Result<Transaction, Error>
    @commit (tx: Transaction) -> Result<void, Error>
    @rollback (tx: Transaction) -> Result<void, Error>
    @vacuum () -> Result<void, Error>
    @analyze () -> Result<void, Error>
    // ... 20 more methods
}

// GOOD: Split into focused capabilities
trait Database {
    @get (table: str, id: int) -> Result<Row, Error>
    @query (sql: str) -> Result<[Row], Error>
    @insert (table: str, data: Row) -> Result<int, Error>
    @update (table: str, id: int, data: Row) -> Result<void, Error>
    @delete (table: str, id: int) -> Result<void, Error>
}

trait Transactional {
    @begin () -> Result<Transaction, Error>
    @commit (tx: Transaction) -> Result<void, Error>
    @rollback (tx: Transaction) -> Result<void, Error>
}

trait DatabaseAdmin {
    @vacuum () -> Result<void, Error>
    @analyze () -> Result<void, Error>
}
```

## Complete Example

```ori
// Define domain-specific capabilities
type Notification = { user_id: int, message: str, channel: str }

trait NotificationService {
    @send (notification: Notification) -> Result<void, Error>
    @send_bulk (notifications: [Notification]) -> Result<int, Error>
    @get_preferences (user_id: int) -> Result<NotificationPrefs, Error>
}

type NotificationPrefs = {
    email_enabled: bool,
    sms_enabled: bool,
    push_enabled: bool,
}

// Real implementation
type MultiChannelNotifier = {
    email_service: EmailClient,
    sms_service: SmsClient,
    push_service: PushClient,
}

impl NotificationService for MultiChannelNotifier {
    @send (notification: Notification) -> Result<void, Error> = {
        let prefs = self.get_preferences(user_id: notification.user_id)?
        match notification.channel {
            "email" -> if prefs.email_enabled then
                self.email_service.send(user_id: notification.user_id, msg: notification.message)
            else
                Ok(())
            "sms" -> if prefs.sms_enabled then
                self.sms_service.send(user_id: notification.user_id, msg: notification.message)
            else
                Ok(())
            "push" -> if prefs.push_enabled then
                self.push_service.send(user_id: notification.user_id, msg: notification.message)
            else
                Ok(())
            _ -> Err(Error { message: `Unknown channel: {notification.channel}` })
        }
    }

    @send_bulk (notifications: [Notification]) -> Result<int, Error> = {
        let sent = 0
        for notification in notifications do {
            let result = self.send(notification: notification)
            if is_ok(result: result) then
                sent = sent + 1
        }
        Ok(sent)
    }

    @get_preferences (user_id: int) -> Result<NotificationPrefs, Error> = {
        // Fetch from database in real implementation
        Ok(NotificationPrefs { email_enabled: true, sms_enabled: false, push_enabled: true })
    }
}

// Mock for testing
type MockNotificationService = {
    sent_notifications: [Notification],
    preferences: {int: NotificationPrefs},
    should_fail: bool,
}

impl NotificationService for MockNotificationService {
    @send (notification: Notification) -> Result<void, Error> =
        if self.should_fail then
            Err(Error { message: "Mock failure" })
        else {
            self.sent_notifications = [...self.sent_notifications, notification]
            Ok(())
        }

    @send_bulk (notifications: [Notification]) -> Result<int, Error> = {
        let count = 0
        for n in notifications do {
            self.send(notification: n)?
            count = count + 1
        }
        Ok(count)
    }

    @get_preferences (user_id: int) -> Result<NotificationPrefs, Error> =
        self.preferences[user_id]
            .ok_or(error: Error { message: "User not found" })
}

// Business logic using the capability
@notify_user (user_id: int, event: str) -> Result<void, Error>
    uses NotificationService, Logger = {
    Logger.info(msg: `Notifying user {user_id} about {event}`)
    let notification = Notification {
        user_id: user_id
        message: `Event occurred: {event}`
        channel: "email"
    }
    NotificationService.send(notification: notification)
}

// Pure function — no capabilities needed
@format_notification (event: str, details: str) -> str =
    `Event: {event}\n\nDetails: {details}`

@test_format tests @format_notification () -> void = {
    let result = format_notification(event: "Order Shipped", details: "Tracking: 123")
    assert(condition: result.contains(substring: "Order Shipped"))
    assert(condition: result.contains(substring: "123"))
}

// Test with mock
@test_notify_user tests @notify_user () -> void = {
    let mock = MockNotificationService {
        sent_notifications: []
        preferences: {
            42: NotificationPrefs { email_enabled: true, sms_enabled: false, push_enabled: true }
        }
        should_fail: false
    }

    with NotificationService = mock
         Logger = MockLogger {} in {
        let result = notify_user(user_id: 42, event: "Test Event")
        assert_ok(result: result)
        assert_eq(actual: len(collection: mock.sent_notifications), expected: 1)
        assert_eq(actual: mock.sent_notifications[0].user_id, expected: 42)
    }
}

@test_notify_user_failure tests @notify_user () -> void = {
    let mock = MockNotificationService {
        sent_notifications: []
        preferences: {}
        should_fail: true
    }

    with NotificationService = mock
         Logger = MockLogger {} in {
        let result = notify_user(user_id: 42, event: "Test Event")
        assert_err(result: result)
    }
}
```

## Quick Reference

### Defining a Capability

```ori
trait CapabilityName {
    @method1 (param: Type) -> ReturnType
    @method2 (param: Type) -> ReturnType
}
```

### Implementing a Capability

```ori
type RealImplementation = { config: Config }

impl CapabilityName for RealImplementation {
    @method1 (param: Type) -> ReturnType = ...
    @method2 (param: Type) -> ReturnType = ...
}
```

### Declaring Capabilities

```ori
// Single capability
@fn () -> T uses Cap = ...

// Multiple capabilities
@fn () -> T uses Cap1, Cap2 = ...

// Pure function (no capabilities)
@fn () -> T = ...
```

### Providing Capabilities

```ori
// Single capability
with Cap = Implementation in expression

// Multiple capabilities
with Cap1 = Impl1, Cap2 = Impl2 in expression
```

### Standard Capabilities

| Capability | Purpose | Key Methods |
|------------|---------|-------------|
| `Http` | Network requests | `get`, `post`, `put`, `delete` |
| `FileSystem` | File I/O | `read`, `write`, `exists`, `delete` |
| `Clock` | Time | `now`, `today`, `elapsed_since` |
| `Random` | Random numbers | `rand_int`, `rand_float` |
| `Cache` | Caching | `get`, `set`, `delete` |
| `Logger` | Logging | `debug`, `info`, `warn`, `error` |
| `Env` | Environment | `get` |
| `Print` | Console output | `print`, `println` (has default) |
| `Async` | Non-blocking I/O | (marker capability) |

## What's Next

Now that you understand capabilities:

- **[Concurrency](/guide/14-concurrency)** — Parallel execution patterns
- **[Channels](/guide/15-channels)** — Communication between tasks
