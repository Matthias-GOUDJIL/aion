# Plan for TRACK-002: Standard Library Expansion

## 1. File System (`std.fs`)
- [x] `read_to_string(path: String) -> String`
- [x] `write(path: String, content: String) -> i64`
- [x] `exists(path: String) -> bool`

## 2. Collections (`std.collections`)
- [x] `VectorString` implementation (Manual monomorphization).
- [x] `Option<T>` enum support.
- [x] `HashMap<K, V>` implementation.
- [x] Transition `VectorString` to generic `Vector<T>`.

## 3. Environment (`std.env`)
- [x] `var(key: String) -> Option<String>`
- [x] `args() -> VectorString`

## 4. IO (`std.io`)
- [x] `print(msg: String)` (via intrinsic).
- [x] `println(msg: String)` (via intrinsic).
- [x] `read_line() -> String`.

