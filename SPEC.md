# Aion Specification v0.1

## 1. The Type System
Aion uses a **Linear Type System** ensuring resources are used exactly once unless explicitly borrowed.

-   `u8`, `i64`, `f32`, `bool`: Primitives.
-   `str`: UTF-8 immutable string slice.
-   `String`: Heap allocated string.
-   `Tensor<T, N>`: First-class N-dimensional array for AI workloads.

## 2. Memory Model
-   **Ownership**: Automatic. Variables own their data.
-   **Borrowing**: `&` (immutable) and `&mut` (mutable). 
-   **Lifetimes**: Inferred in 99% of cases. Explicit notation `'a` exists but is rarely needed.

## 3. AI-Friendly Features
Aion is designed to be parsed by LLMs with 100% accuracy.
-   **Explicit Context**: 
    ```aion
    @desc("Calculates fast inverse square root")
    fn fast_inv_sqrt(x: f32) -> f32 { ... }
    ```
-   **Predictable Grammar**: Context-free grammar designed to minimize lookahead, making token generation for Transformers cheaper and more accurate.

## 4. The Toolchain (`aionc`)
The compiler is built in Rust for bootstrapping.
-   **Stage 1**: Lexer/Parser (generates Aion AST).
-   **Stage 2**: AIR (Aion Intermediate Representation) - A high-level IR for optimization.
-   **Stage 3**: CodeGen (LLVM / Cranelift).
