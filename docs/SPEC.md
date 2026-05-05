# Aion Specification v0.3 (Current Implementation)

## 1. Architecture
Aion is a direct-to-LLVM compiler with a strict safety-first pipeline.
-   **Frontend**: Hand-written Lexer (`lexer/`) and Recursive Descent Parser (`parser/`).
-   **Middle**: Type Checker (`analysis/`) with scoped environments, fuzzy resolution, and strict generic validation.
-   **Backend**: Direct LLVM IR generation (`codegen/`) using `inkwell` (LLVM 15+ Opaque Pointers).
-   **Runtime**: Minimal C runtime (`runtime.c`) for I/O, threading, and AI tensor primitives.
-   **Orchestration**: Docker-first execution via `./aion` wrapper. Host `cargo` commands are strictly forbidden for compilation.

## 2. Type System (Implemented)
-   **Primitives**: `i64` (integer), `f64` (float), `bool`, `String` (pointer to C-string).
-   **Composite**: `Struct`, `Enum` (tagged unions), `Interface`, `Impl` blocks.
-   **Enum Layout**: Enums are compiled as `{ i64, [64 x i8] }`. Index 0 is the Tag, Index 1 is the Payload.
    - Tags: `Some/Ok = 0`, `None/Err = 1`.
-   **Pointers**: All complex types are passed by reference. Explicit pointer types use `*T`.
-   **Generics**: Monomorphization at compile time. Full type substitution before LLVM generation.
-   **Fuzzy Resolution**: Supports suffix/prefix matching for imports and method calls (e.g., `HashMap` → `std.collections.map.HashMap`).

## 3. Memory & Strings
-   **Current**: Automatic memory management via Boehm Garbage Collector (`libgc`).
-   **Strings**: C-style strings (`char*`). Escape sequences supported: `\n`, `\t`, `\r`, `\\`, `\"`, `\0`.
-   **Safe Concat**: String concatenation (`+` or `concat`) uses `aion_str_concat` (allocates new buffer).
-   **Comparison**: `==` and `!=` compare string content via `aion_str_eq` when both operands are `String`.
-   **Safety**: `unsafe` blocks required for pointer dereferences, FFI, and specific intrinsics.

## 4. Compiler Robustness (v0.3 Additions)
-   **Error Handling**: Zero `unwrap()` in production code. All compiler stages return `Result<T, CompileError>` with typed error variants (Type, Unsafe, NotFound, NotFunction, InvalidOperator, Inkwell, Io, Import, Internal).
-   **Block Termination**: Every LLVM basic block is guaranteed to be terminated. The compiler injects `ret` or `unreachable` if `get_terminator().is_none()`.
-   **Type Safety**: `Type::Unknown` is never returned silently. Fallbacks are explicit and validated.
-   **Debug Hygiene**: No `println!`/`eprintln!` in production builds. Conditional logging only.
-   **Span Tracking**: All AST nodes carry `Span` (line, col) for precise error reporting.
-   **Recursion**: Full support for self-recursion and mutual recursion. Two-pass compilation (prototypes first, then bodies) enables forward references. Tested up to 1B+ recursion depth. Stack depth is limited only by the OS stack size (default 8MB on Linux).

## 5. Environment & Build
-   **Docker-First**: LLVM 15+ dependencies are isolated in `aion-compiler` Docker image.
-   **Wrapper**: `./aion` is the primary entry point for compilation and execution.
-   **Testing**: `cargo test` with insta snapshot tests, run inside Docker.
-   **Import Resolution**: Recursive import processing with automatic namespace prefixing to prevent symbol collisions.

## 6. Concurrency & AI-Native Features
-   **Model**: 1:1 Threading via `pthread`. `spawn { ... }` creates detached threads.
-   **AI Tensors**: First-class `std.ai.tensor` support (`zeros`, `ones`, `rand`, `matmul`, `backward`).
-   **Intents**: `::intent "..."` syntax for AI-guided compilation hints.
-   **Short-circuiting**: `&&` and `||` use lazy evaluation via conditional branching.

## 7. Known Limitations (Phase 1.7)
-   No RAII/Destructors (GC-only memory model).
-   LLVM opaque pointers require explicit type casting in some edge cases.
-   Cross-compilation targets are not yet supported (x86_64-linux only).
