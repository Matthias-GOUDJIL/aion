# Aion - Product Definition

Aion is a system programming language designed for AI-native applications, prioritizing **performance (Rust/LLVM)**, **expressiveness (Python/Elo)**, and **intelligence (First-class Intents)**.

## Core Pillars

1.  **AI-Native**: First-class support for Tensors, Time Series, and Probabilistic reasoning.
2.  **Safety & Speed**: Compiles to optimized native code via LLVM, with a linear type system for memory safety without GC.
3.  **Parsability**: Designed with a simple, context-free grammar to be easily read and written by LLMs.
4.  **Interop**: Zero-cost FFI with C and Python ecosystems.

## Target Audience

-   AI Researchers needing performance without C++ complexity.
-   System Engineers building high-performance inference engines.
-   Autonomous Agent developers requiring strict safety guarantees.

## Roadmap Phase 1.6: Rigor & Completeness

-   [x] Fix Parser Infinite Loops.
-   [x] Fix LLVM 15+ Deprecated Pointer Types.
-   [ ] Implement Generics (Templates).
-   [ ] Complete Standard Library (FS, Net, Time).
-   [ ] Add basic Error Handling (Result<T, E>).
