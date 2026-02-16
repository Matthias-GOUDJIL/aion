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
-   **Pointers**: All complex types are passed by reference (pointers).
-   **Generics**: Monomorphization at compile time (like C++ templates or Rust).

## 3. Memory Model
-   **Current**: Unsafe manual memory management (mostly leaked or stack-allocated).
-   **Strings**: C-style strings (`char*`), currently leaked on modification.
-   **Safety**: Basic checks in `checker.rs`, but runtime safety is minimal.

## 4. Concurrency
-   **Model**: 1:1 Threading via `pthread`.
-   **Keyword**: `spawn { ... }` creates a detached thread.

## 5. Known Limitations (Phase 1.6)
-   No Garbage Collection (GC) or RAII yet.
-   Operators `&&` and `||` are eager (no short-circuiting yet).
-   LLVM 15 Opaque Pointers must be used strictly.
