# TRACK-002: Standard Library Expansion

## Status: 🚧 In Progress
**Owner:** Librarian

## Goals
Expand the Aion Standard Library (`stdlib/`) to support essential system programming features.

## Scope

### 1. File System (`std.fs`)
- [ ] `read_file(path: String) -> String`
- [ ] `write_file(path: String, content: String) -> Result<void, String>`
- [ ] `exists(path: String) -> bool`

### 2. Collections (`std.collections`)
- [ ] `Vector<T>` implementation (native or wrapper).
- [ ] `HashMap<K, V>` implementation.

### 3. Environment (`std.env`)
- [ ] `args() -> Vector<String>`
- [ ] `var(key: String) -> Option<String>`

## Implementation Plan
See `plan.md` (to be created).
