# Aion Specification v0.2 (Current Implementation)

## 1. Architecture
Aion is currently a direct-to-LLVM compiler.
-   **Frontend**: Hand-written Lexer (`lexer.rs`) and Recursive Descent Parser (`parser.rs`).
-   **Middle**: Type Checker (`checker.rs`) with basic type inference.
-   **Backend**: Direct LLVM IR generation (`compiler.rs`) using `inkwell` (LLVM 15).
-   **Runtime**: Minimal C runtime (`runtime.c`) for I/O and threading.

## 2. Type System (Implemented)
-   **Primitives**: `i64` (integer), `f64` (float), `bool`, `String` (pointer to C-string).
-   **Composite**: `Struct`, `Enum` (tagged unions).
-   **Enum Layout**: Enums are compiled as `{ i64, [64 x i8] }`. Index 0 is the Tag, Index 1 is the Payload.
-   **Pointers**: All complex types are passed by reference (pointers). Explicit pointer types use `*T`.
-   **Generics**: Monomorphization at compile time (like C++ templates or Rust).

## 3. Memory & Strings
-   **Current**: Unsafe manual memory management (mostly leaked or stack-allocated).
-   **Strings**: C-style strings (`char*`). 
-   **Safe Concat**: String concatenation (`+` or `concat`) uses a specialized `aion_str_concat` runtime function that allocates a new buffer to avoid memory corruption.
-   **Safety**: Basic checks in `checker.rs`. Unsafe blocks are required for pointer dereferences and specific intrinsics.

## 4. Environment & Build
-   **Docker-First**: The compiler requires LLVM 15, which is managed via the `aion-compiler` Docker image.
-   **Wrapper**: The `./aion` script is the primary entry point for building and running code.
-   **Fuzzy Resolution**: The compiler supports fuzzy name resolution for types and methods (e.g., `HashMap` can resolve to `std.collections.map.HashMap`).

## 5. Concurrency
-   **Model**: 1:1 Threading via `pthread`.
-   **Keyword**: `spawn { ... }` creates a detached thread.

## 5. Known Limitations (Phase 1.6)
-   No Garbage Collection (GC) or RAII yet.
-   Operators `&&` and `||` are eager (no short-circuiting yet).
-   LLVM 15 Opaque Pointers must be used strictly.
