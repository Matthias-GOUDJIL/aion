# Aion Programming Language

Aion is a system programming language designed for AI-native applications, prioritizing **performance (Rust/LLVM)**, **expressiveness (Python/Elo)**, and **intelligence (First-class Intents)**.

## Core Pillars

1. **AI-Native**: First-class support for Tensors, Time Series, and Probabilistic reasoning.
2. **Safety & Speed**: Compiles to optimized native code via LLVM, with a linear type system for memory safety without GC.
3. **Parsability**: Designed with a simple, context-free grammar to be easily read and written by LLMs.
4. **Interop**: Zero-cost FFI with C and Python ecosystems.

## Target Audience

- AI Researchers needing performance without C++ complexity.
- System Engineers building high-performance inference engines.
- Autonomous Agent developers requiring strict safety guarantees.

## Quick Start

```bash
./aion run examples/hello.ai
```

## Project Structure

```
src/           — Rust compiler (lexer → parser → type checker → LLVM codegen)
stdlib/        — Aion standard library (written in Aion)
tests/         — Test fixtures and expected outputs
examples/      — Sample Aion programs
docs/          — Language specification and API docs
compiler/      — Self-hosting compiler (lexer.ai, parser.ai, ast.ai, token.ai)
```

## Tech Stack

| Component | Technology |
|-----------|------------|
| Compiler | Rust (Edition 2024) |
| Backend | LLVM 15 (via `inkwell` crate) |
| Runtime | C (C11) |
| Memory | Boehm GC (`libgc`) |
| Threading | Pthreads |
| Testing | Python 3 + Docker |
| CI | GitHub Actions |

## Contributing

1. Read [docs/SPEC.md](docs/SPEC.md)
2. Create a feature branch (`git checkout -b feat/my-feature`)
3. Commit with conventional commits (`feat:`, `fix:`, `refactor:`)
4. Open a PR against `main`

See [AGENTS.md](AGENTS.md) for AI-assistant guidelines.

## License

MIT
