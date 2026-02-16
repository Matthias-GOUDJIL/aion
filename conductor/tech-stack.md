# Aion - Tech Stack

## Compiler (Bootstrapped)
-   **Language**: Rust (Edition 2021)
-   **Backend**: LLVM 15 (via `inkwell` crate)
-   **Build System**: Cargo

## Runtime
-   **Language**: C (C11)
-   **Threading**: Pthreads (1:1 model)
-   **IO**: Standard Libc

## Testing
-   **Runner**: Python 3.10+
-   **Containerization**: Docker (for reproducible build environments)

## Development Environment
-   **Extension**: Gemini CLI Conductor
-   **Editor**: VS Code (with custom syntax highlighting)
