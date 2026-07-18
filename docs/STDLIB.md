# Aion Standard Library

This document provides an overview of the Aion Standard Library (`stdlib`),
organized by domain.

> **Implementation status legend**
> - **[stable]** — implemented, tested via fixtures, used by examples.
> - **[partial]** — implemented but missing features or with known bugs.
> - **[stub]** — module skeleton only; declarations present, bodies incomplete.
> - **[skeleton]** — placeholder file, no usable implementation yet.
>
> Status reflects what the current compiler can actually compile and run.
> Modules marked **[stub]** or **[skeleton]** should not be relied on.

## 1. Core & Primitives

Fundamental building blocks of the language.

### `core.heap` **[stable]**
Low-level memory management intrinsics.
- `alloc`, `dealloc`, `realloc`, `null`.

### `std.mem` **[stub]**
Memory utilities.
- `size_of`, `align_of`, `swap`, `replace`.

### `std.prelude` **[stable]**
Common imports (`Option`, `Result`, `String`, `Vector`, `print`, `println`).

---

## 2. Data Structures & Algorithms

### `std.collections`
- **`vector`** **[stable]** — `Vector<T>` (dynamic array). Includes `new`, `push`, `pop`, `get`, `set`, `len`, `is_empty`, `first`, `last`, `contains`, `clear`, `swap`, `insert`, `remove`. Missing `iter`/`map`/`filter`/`fold` (blocked: no closures in Aion yet).
- **`list`** **[stub]** — `LinkedList<T>` (singly linked).
- **`map`** **[stable]** — `HashMap<V>` (generic V, string key). Includes
  `contains_key`, `keys`, `values`, `clear`, `remove`.
- **`ordered_map`** **[stable]** — `OrderedMap<V>` (insertion-ordered,
  generic V, string key). Backed by two parallel `Vector`s so iteration
  order is deterministic across runs. Includes `new`, `insert`, `get`,
  `contains_key`, `remove`, `len`, `is_empty`, `keys_in_order`,
  `values_in_order`, `clear`. O(n) lookup — intended for small maps
  (<1000 entries) where snapshot/dump ordering matters (e.g. codegen #9).
  Issue #136.
- **`set`** **[stub]** — `HashSet`.

### `std.string` **[stable]** & `std.char` **[stable]**
Full string and character manipulation suite (`len`, `concat`, `from_int`,
`from_float`, `to_float`, `at`, `substr`, `contains`, `starts_with`,
`ends_with`, `find`, `trim`, `to_upper`, `to_lower`, `replace`).

### `std.iter` **[stub]**
`Iterator` interface and `Range`.

### `std.hash` **[stable]**
`Hashable` interface and FNV-1a implementation.

### `std.bits` **[stub]**
Bitwise operations on integers.

---

## 3. System, OS & I/O

### `std.io` **[stable]** & `std.io.buf` **[stub]**
Standard and Buffered I/O (`BufReader`).

### `std.fs` **[stable]** & `std.path` **[stable]**
File system access and cross-platform path manipulation.

### `std.process` **[stub]** & `std.signal` **[stub]**
Process spawning (`Command`, `Child`) and signal handling (`Signal`,
`handle`).

### `std.env` **[stable]** & `std.os` **[stub]**
Environment variables (`args`, `var`) and OS primitives (`exit`,
`cpu_count`).

### `std.flags` **[stub]**
Command-line argument parsing (CLI builder).

### `std.error` **[stub]**
Error handling types and utilities.

---

## 4. Network & Web

### `std.net` **[skeleton]**
TCP Sockets (`bind`, `accept`).

### `std.net.http` **[skeleton]**
HTTP Client/Server structures (`Request`, `Response`, `Client`).

### `std.net.tls` **[skeleton]**
Secure networking (`TlsContext`, `TlsStream`) for HTTPS support.

### `std.net.websocket` **[skeleton]**
Real-time web communication.
- **Struct** `WebSocket`: `connect`, `send`, `receive`.
- **Enum** `Message`: `Text`, `Binary`, `Ping`, `Close`.

### `std.uri` **[skeleton]**
URI parsing and construction.

### `web.dom` **[skeleton]**
DOM manipulation primitives.

---

## 5. Concurrency & Distribution

### `std.thread` **[stub]**
Basic threading (`yield_now`). `spawn { ... }` is a language keyword, not
this module.

### `std.sync`
- **`mutex`** **[stub]** — Mutual exclusion.
- **`channel`** **[stub]** — MPSC Channels for message passing.
- **`atomic`** **[stub]** — Atomic integers and booleans.

### `std.task` **[skeleton]**
Async runtime primitives (`Poll`, `Context`).

### `std.distrib` **[skeleton]**
Native Clustering.
- `node` (gossip), `spawn_remote` (remote actors).

---

## 6. Artificial Intelligence (Native)

### `std.ai.tensor` **[partial]**
The core of Aion. Only the constructors are functional today; the math ops
and autograd are runtime placeholders (see `src/runtime.c:220-235`).
- `tensor_zeros`, `tensor_ones`, `tensor_rand`: Working constructors.
- `tensor_move`: Device transfer stub (currently a no-op string swap, not real GPU/TPU).
- `matmul`, `add`: **Not implemented** — `aion_ai_tensor_matmul` / `aion_ai_tensor_add` return a zero tensor regardless of input (`// Placeholder: return zeros for now`).
- `backward()`: **Not implemented** — `aion_ai_tensor_backward` only prints to stdout, no gradient is computed.
- `to(device)`: See `tensor_move` above; no real device support yet.

### `std.media`
- **`image`** **[skeleton]** — Load/Save images, convert to Tensor.
- **`audio`** **[skeleton]** — Load audio, compute spectrograms for AI.

---

## 7. Data Science & Formats

### `std.data`
- **`dataframe`** **[stable]** — Native Dataframes (`read_csv`, `select`,
  `filter`).
- **`series`** **[partial]** — Typed column arrays.

### `std.json` **[partial]**
JSON parsing and stringification. Primitives only — array/object parsing
blocked by type checker (see `stdlib/std/json_SPEC.md`).

### `std.encoding`
- **`hex`** **[stub]** — Hex encoding.
- **`base64`** **[skeleton]** — Base64 encoding.

### `std.text.template` **[skeleton]**
Text templating engine (Jinja-like).

### `std.archive`
- **`zip`** **[skeleton]** — ZIP archive creation and extraction.

### `std.compress`
- **`gzip`** **[skeleton]** — Gzip compression and decompression.

### `std.sql`
- **`driver`** **[skeleton]** — SQL database driver interface.

---

## 8. Math & Cryptography

### `std.math` **[stub]**
Basic math.
- **`complex`** **[stub]** — Complex numbers.
- **`big`** **[stub]** — Big integers (arbitrary precision).

### `std.crypto`
- **`sha256`** **[skeleton]** — Hashing.
- **`aes`** **[skeleton]** — Symmetric encryption (CBC/GCM).

### `std.random` **[stub]**
Pseudo-random number generation.

---

## 9. Observability & Global

### `std.telemetry` **[skeleton]**
Tracing (`Span`) and Metrics (`Counter`, `Gauge`).

### `std.log` **[stub]**
Structured logging (`debug`, `info`, `warn`, `error`).

### `std.i18n` **[skeleton]**
Internationalization (`Locale`, `Catalog`).

### `std.test` **[stub]**
Unit testing assertions.

### `std.ffi` **[stub]**
C Interop (`CString`, `c_malloc`).

### `std.reflect` **[skeleton]**
Runtime Introspection (`type_of`, `type_name`, `has_field`).

### `std.regex` **[skeleton]**
Regular expression matching.

### `std.convert` **[stub]**
Type conversion utilities.

### `std.fmt` **[stable]**
Formatting utilities. `Display`/`Debug` interfaces and `format(template,
args)` for `'{}'`-placeholder interpolation.

---

## 10. User Interface & Real-time

### `std.ui.core` **[skeleton]**
Native Declarative UI.
- **Widgets**: `Window`, `Text`, `Button`, `Column`, `Row`.
- `Window::run()`: Starts the UI event loop.

### `std.uuid` **[skeleton]**
Universally Unique Identifiers.
- `new_v4()`: Random based.
- `new_v7()`: Time-ordered.

---

## 11. Time & Date

### `std.time` **[partial]**
Duration works (constructors, `as_secs`/`as_millis`, arithmetic — exercised by
`duration_literal`). `now()` and `sleep()` are **not implemented**: their
`@intrinsic("time_now")` / `@intrinsic("time_sleep")` resolve to
`aion_time_now` / `aion_time_sleep`, which are absent from `src/runtime.c`.
Add the runtime symbols before re-promoting to [stable].

### `std.date` **[partial]**
The `Date` struct and date-literal arithmetic work (exercised by
`date_arithmetic`). `DateTime::now()` is **not implemented**: its
`date_year`/`date_month`/`date_day`/`date_hour`/`date_min`/`date_sec`
intrinsics resolve to `aion_date_*` symbols absent from `src/runtime.c`.
Add the runtime symbols before re-promoting to [stable].
