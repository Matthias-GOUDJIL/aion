# Aion Programming Language

Aion is an experimental systems and application programming language designed from the ground up to be **ergonomic for both humans and AI code generators**, while compiling to fast native binaries via **LLVM**.

> **Status: v0.6 (Alpha / Active Development)** — The bootstrap compiler (Rust → LLVM 15), the PostgreSQL transpiler, and the self-hosted compiler pipeline (`compiler/*.ai`) are functional and tested across 159 integration & unit tests.

## Why Aion?

Modern languages force a trade-off when used with LLMs and autonomous coding agents: Python is easy for models to write but slow and weakly typed, while Rust and C++ offer native performance but burden code generation with lifetime annotations, borrow-checker friction, or manual memory management.

Aion explores a pragmatic middle ground:
1. **Predictable, LLM-Friendly Syntax**: A clean, context-free grammar inspired by Rust and TypeScript—algebraic data types (`enum`), exhaustive `match`, generics, and pipelines (`|>`)—without lifetimes or macro magic.
2. **Native Performance with Automatic Memory**: Compiles directly to optimized native machine code via **LLVM 15**, paired with automatic memory management (Boehm GC) and an explicit `unsafe` boundary for raw pointers and C FFI.
3. **Universal Targets (Native + SQL)**: Write strongly-typed functions once; compile them to native binaries or transpile pure Aion subsets directly into PostgreSQL (`PL/pgSQL`) functions and composite types.
4. **Built-in Temporal & Data Primitives**: Native syntax for durations (`5s`, `500ms`), dates (`D2026-01-01`), pipeline operators (`data |> transform`), and tabular data structures.

## Who is it for? (Current Scope)

- **Language & Compiler Enthusiasts** exploring self-hosting compilers and LLVM IR generation.
- **AI Agent & Tooling Developers** looking for a strongly-typed, garbage-collected native language with clear compiler diagnostics ("did you mean...?" suggestions, spanned snippets) tailored for automated repair loops.
- **Backend & Data Engineers** interested in unified application and database (`PL/pgSQL`) code generation.

*(Note: Native AI tensor operations in `std.ai.tensor` and intent-guided optimizations `::intent` are currently experimental stubs on the roadmap and are not yet ready for production ML workloads.)*

## What Works Today (v0.6)

- **Type System**: `i8`..`i64`, `u8`..`u64`, `f64`, `bool`, `String`, `Duration`, `Date`, `Struct`, tagged-union `Enum` with multi-payloads, `Tuple`, bounds-checked `Array` (`[T; N]`), function pointers, and compile-time monomorphized **Generics** (`Vector<T>`, `HashMap<V>`, `Option<T>`, `Result<T, E>`).
- **Control Flow & Safety**: Expression-based `if`/`match` with exhaustiveness checking, `while`/`loop`/`break`/`continue`, `spawn { ... }` threads, and strict `unsafe` blocks for pointer dereferencing and FFI.
- **Self-Hosting Pipeline (`compiler/`)**: The Aion lexer (`lexer.ai`), parser (`parser.ai` — parses 100% of the language test suite), and LLVM IR backend (`codegen.ai`) are written in Aion and pass end-to-end binary parity tests against the Rust bootstrap compiler.
- **PostgreSQL Transpiler**: `./aion transpile file.ai` lowers Aion structs, enums, pattern matching, and functions to valid PostgreSQL definitions with spanned diagnostics.

## Quick Start

Aion uses a self-contained Docker toolchain (Ubuntu 22.04 + LLVM 15 + `lld` + `libgc`) so you don't need to install LLVM locally:

```bash
# Run an Aion program (compiles to native binary via LLVM 15 and executes)
./aion run examples/hello.ai

# Build to LLVM IR (output.ll)
./aion build examples/generics.ai

# Transpile an Aion file to PostgreSQL (PL/pgSQL)
./aion transpile tests/fixtures/stdlib/sql_match_enum.ai

# Generate Markdown documentation from source comments
./aion doc stdlib/std/collections/vector.ai
```

## Project Structure

```text
src/           — Bootstrap compiler in Rust (lexer → parser → type checker → LLVM 15 codegen)
  codegen/     — LLVM IR emitter (inkwell) + PostgreSQL transpiler (transpiler/sql.rs)
  runtime.c    — Minimal C11 runtime (I/O, strings, pthreads, Boehm GC bridge)
compiler/      — Self-hosted compiler written in Aion (token.ai, lexer.ai, ast.ai, parser.ai, codegen.ai)
stdlib/        — Standard library in Aion (see docs/STDLIB.md for per-module maturity status)
tests/         — 150+ integration fixtures & insta snapshots
docs/          — Specifications (SPEC.md, SPEC_SQL.md, STDLIB.md, architecture.md, workflow.md)
```

## Tech Stack

| Component | Technology |
|-----------|------------|
| Bootstrap Compiler | Rust (Edition 2024) |
| Self-Hosted Compiler | Aion (`compiler/*.ai`) |
| Backend | LLVM 15 (`inkwell` / textual `.ll` + `llc-15` + `lld-15`) |
| Transpiler | PostgreSQL (`PL/pgSQL`) |
| Runtime | C (C11) + Boehm GC (`libgc`) + Pthreads |
| Testing | Rust (`insta` snapshots + `assert_cmd`) in Docker |
| CI | GitHub Actions (`cargo test`, `cargo audit`, CodeQL, `docs-check.sh`) |

## Running the Test Suite

```bash
docker run --rm -v "$(pwd)":/workspace -w /workspace aion-compiler cargo test -- --test-threads=1
```

## Documentation & Contributing

1. [AGENTS.md](AGENTS.md) — rules, conventions, triage guide
2. [docs/SPEC.md](docs/SPEC.md) — core language specification (current implementation)
3. [docs/STDLIB.md](docs/STDLIB.md) — standard library reference & implementation status (`[stable]` vs `[stub]`)
4. [docs/SPEC_SQL.md](docs/SPEC_SQL.md) — PostgreSQL transpiler specification
5. [docs/architecture.md](docs/architecture.md) — compiler internals and invariants
6. [docs/workflow.md](docs/workflow.md) — commands, Docker setup, and label taxonomy

## License

MIT
