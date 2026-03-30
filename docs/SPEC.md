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
-   **Current**: Automatic memory management via Boehm Garbage Collector (GC).
-   **Strings**: C-style strings (`char*`). 
-   **Safe Concat**: String concatenation (`+` or `concat`) uses a specialized `aion_str_concat` runtime function that allocates a new buffer.
-   **Comparison**: The `==` and `!=` operators compare string content (via `strcmp`) rather than just pointers when both operands are `String`.
-   **Safety**: Basic checks in `checker.rs`. Unsafe blocks are required for pointer dereferences and specific intrinsics.

## 4. Environment & Build
-   **Docker-First**: The compiler requires LLVM 15, which is managed via the `aion-compiler` Docker image.
-   **Wrapper**: The `./aion` script is the primary entry point for building and running code.
-   **Fuzzy Resolution**: The compiler supports fuzzy name resolution for types and methods (e.g., `HashMap` can resolve to `std.collections.map.HashMap`).

## 5. Concurrency
-   **Model**: 1:1 Threading via `pthread`.
-   **Keyword**: `spawn { ... }` creates a detached thread.

## 6. Language Features
-   **Short-circuiting**: Logical operators `&&` and `||` support short-circuiting (lazy evaluation).
-   **Generics**: Monomorphization for structs and functions.
-   **Pattern Matching**: Deep pattern matching on enums with payload extraction.

## 7. Known Limitations (Phase 1.7)
-   LLVM 15 Opaque Pointers must be used strictly.
-   No RAII/Destructors (GC only).
