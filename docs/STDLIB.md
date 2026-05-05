# Aion Standard Library

This document provides a comprehensive overview of the Aion Standard Library (`stdlib`), organized by domain.

## 1. Core & Primitives

Fundamental building blocks of the language.

### `core.heap`
Low-level memory management intrinsics.
- `alloc`, `dealloc`, `realloc`, `null`.

### `std.mem`
Memory utilities.
- `size_of`, `align_of`, `swap`, `replace`.

### `std.prelude`
Common imports (`Option`, `Result`, `String`, `Vector`, `print`, `println`).

---

## 2. Data Structures & Algorithms

### `std.collections`
- **`vector`**: `Vector<T>` (dynamic array).
- **`list`**: `LinkedList<T>` (singly linked).
- **`map`**: `HashMap<V>` (generic V, string key).
- **`set`**: `HashSet`.

### `std.string` & `std.char`
Full string and character manipulation suite.

### `std.iter`
`Iterator` interface and `Range`.

### `std.hash`
`Hashable` interface and FNV-1a implementation.

### `std.bits`
Bitwise operations on integers.

---

## 3. System, OS & I/O

### `std.io` & `std.io.buf`
Standard and Buffered I/O (`BufReader`).

### `std.fs` & `std.path`
File system access and cross-platform path manipulation.

### `std.process` & `std.signal`
Process spawning (`Command`, `Child`) and signal handling (`Signal`, `handle`).

### `std.env` & `std.os`
Environment variables (`args`, `var`) and OS primitives (`exit`, `cpu_count`).

### `std.flags`
Command-line argument parsing (CLI builder).

### `std.error`
Error handling types and utilities.

---

## 4. Network & Web

### `std.net`
TCP Sockets (`bind`, `accept`).

### `std.net.http`
HTTP Client/Server structures (`Request`, `Response`, `Client`).

### `std.net.tls`
Secure networking (`TlsContext`, `TlsStream`) for HTTPS support.

### `std.net.websocket`
Real-time web communication.
- **Struct** `WebSocket`: `connect`, `send`, `receive`.
- **Enum** `Message`: `Text`, `Binary`, `Ping`, `Close`.

### `std.uri`
URI parsing and construction.

### `web.dom`
DOM manipulation primitives.

---

## 5. Concurrency & Distribution

### `std.thread`
Basic threading (`yield_now`).

### `std.sync`
- **`mutex`**: Mutual exclusion.
- **`channel`**: MPSC Channels for message passing.
- **`atomic`**: Atomic integers and booleans.

### `std.task`
Async runtime primitives (`Poll`, `Context`).

### `std.distrib`
Native Clustering.
- `node` (gossip), `spawn_remote` (remote actors).

---

## 6. Artificial Intelligence (Native)

### `std.ai.tensor`
The core of Aion.
- `Tensor`: N-dimensional array.
- `matmul`, `add`: Accelerated math.
- `backward()`: Automatic differentiation (Autograd).
- `to(device)`: GPU/TPU support.

### `std.media`
- **`image`**: Load/Save images, convert to Tensor.
- **`audio`**: Load audio, compute spectrograms for AI.

---

## 7. Data Science & Formats

### `std.data`
- **`dataframe`**: Native Dataframes (`read_csv`, `select`, `filter`).
- **`series`**: Typed column arrays.

### `std.json`
JSON parsing and stringification.

### `std.encoding`
Hex and Base64 encoding.

### `std.text.template`
Text templating engine (Jinja-like).

### `std.archive`
- **`zip`**: ZIP archive creation and extraction.

### `std.compress`
- **`gzip`**: Gzip compression and decompression.

### `std.sql`
- **`driver`**: SQL database driver interface.

---

## 8. Math & Cryptography

### `std.math`
Basic math, `complex` numbers, and `big` integers (arbitrary precision).

### `std.crypto`
- **`sha256`**: Hashing.
- **`aes`**: Symmetric encryption (CBC/GCM).

### `std.random`
Pseudo-random number generation.

---

## 9. Observability & Global

### `std.telemetry`
Tracing (`Span`) and Metrics (`Counter`, `Gauge`).

### `std.log`
Structured logging (`debug`, `info`, `warn`, `error`).

### `std.i18n`
Internationalization (`Locale`, `Catalog`).

### `std.test`
Unit testing assertions.

### `std.ffi`
C Interop (`CString`, `c_malloc`).

### `std.reflect`
Runtime Introspection (`type_of`, `type_name`, `has_field`).

### `std.regex`
Regular expression matching.

### `std.convert`
Type conversion utilities.

### `std.fmt`
Formatting utilities.

---

## 10. User Interface & Real-time

### `std.ui.core`
Native Declarative UI.
- **Widgets**: `Window`, `Text`, `Button`, `Column`, `Row`.
- `Window::run()`: Starts the UI event loop.

### `std.uuid`
Universally Unique Identifiers.
- `new_v4()`: Random based.
- `new_v7()`: Time-ordered.

---

## 11. Time & Date

### `std.time`
Duration and Date types with arithmetic operations.

### `std.date`
Date utilities and formatting.
