# Aion - Tech Stack

## Compiler (Bootstrapped)

- **Language**: Rust (Edition 2024)
- **Backend**: LLVM 15 (via `inkwell` crate)
- **Build System**: Cargo

## Runtime

- **Language**: C (C11)
- **Threading**: Pthreads (1:1 model)
- **Memory**: Boehm GC (`libgc`)
- **IO**: Standard Libc

## Testing

- **Runner**: Python 3.10+
- **Containerization**: Docker (for reproducible build environments)

## Development Environment

- **Editor**: VS Code (with custom syntax highlighting in `editors/`)
- **AI Assistants**: AI-agnostic — all instructions maintained in `AGENTS.md`
