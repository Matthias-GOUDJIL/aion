# Plan for TRACK-002: Standard Library Expansion

## 1. File System (`std.fs`)
- [x] `read_to_string(path: String) -> String`
    - Implemented as `fs.read_to_string` intrinsic.
- [x] `write(path: String, content: String) -> i64`
    - Implemented as `fs.write` intrinsic (returns bytes written).
- [x] `exists(path: String) -> bool`
    - Implemented as `fs.exists` intrinsic.

## 2. Collections (`std.collections`)
- [ ] `Vector<T>` implementation.
- [ ] `HashMap<K, V>` implementation.

## 3. Environment (`std.env`)
- [ ] `args() -> Vector<String>`
- [ ] `var(key: String) -> Option<String>`
