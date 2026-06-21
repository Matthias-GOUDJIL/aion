# Aion Programming Language

Aion is a system programming language designed for AI-native applications, prioritizing **performance (Rust/LLVM)**, **expressiveness**, and **intelligence (First-class Intents)**.

## Core Pillars

1. **AI-Native**: First-class support for Tensors and Time primitives (`Duration`, `Date`).
2. **Safety & Speed**: Compiles to optimized native code via LLVM, with a strict `unsafe` boundary and automatic memory management via the Boehm GC.
3. **Parsability**: Designed with a simple, context-free grammar to be easily read and written by LLMs.
4. **Interop**: Zero-cost FFI with C ecosystems.

## Target Audience

- AI Researchers needing performance without C++ complexity.
- System Engineers building high-performance inference engines.
- Autonomous Agent developers requiring strict safety guarantees.

## Quick Start

```bash
./aion run examples/hello.ai
```

> **Note**: Compilation requires the `aion-compiler` Docker image (LLVM 15 + Boehm GC). See `docs/workflow.md` for setup.

## Project Structure

```
src/           — Rust compiler (lexer → parser → type checker → LLVM codegen)
  codegen/       — LLVM IR generation + SQL transpiler
stdlib/         — Aion standard library (written in Aion, .ai files)
compiler/       — Self-hosting compiler (lexer.ai, parser.ai, ast.ai, token.ai)
tests/          — Test fixtures + insta snapshots
examples/      — Sample Aion programs
docs/          — Language specification, architecture, stdlib reference
.github/       — Issue/PR templates, CI workflows
```

## Tech Stack

| Component | Technology |
|-----------|------------|
| Compiler | Rust (Edition 2024) |
| Backend | LLVM 15 (via `inkwell` crate) |
| Runtime | C (C11) |
| Memory | Boehm GC (`libgc`) |
| Threading | Pthreads |
| Testing | Rust (`insta` + `assert_cmd`) + Docker |
| CI | GitHub Actions |

## Contributing

1. Read [AGENTS.md](AGENTS.md) — rules, conventions, triage guide
2. Read [docs/SPEC.md](docs/SPEC.md) — language specification
3. Create a feature branch (`git checkout -b feat/my-feature`)
4. Commit with conventional commits (`feat:`, `fix:`, `refactor:`, `docs:`, `test:`, `chore:`)
5. Open a PR against `main` (uses [PR template](.github/PULL_REQUEST_TEMPLATE.md))

See [docs/workflow.md](docs/workflow.md) for commands, Docker setup, and label taxonomy.

## License

MIT
