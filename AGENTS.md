# Aion — Agent Instructions

## Project

Aion is a system programming language for AI-native apps. The compiler (`aionc`) is written in **Rust** and targets **LLVM 15** IR. The runtime links against **Boehm GC** (`libgc`) and `pthread` via a C runtime (`src/runtime.c`).

## Developer Commands

| Action | Command |
|--------|---------|
| Build + run a file | `./aion build <file.ai> && ./output` |
| Compile + run (all-in-one) | `./aion run <file.ai>` |
| Run test suite | `python3 runner.py` |
| Transpile to SQL | `./aion transpile <file.ai>` |

**CRITICAL**: Never run `cargo build`, `cargo run`, or `cargo test` directly on the host. The `./aion` wrapper runs everything inside a Docker container (`aion-compiler` image based on Ubuntu 22.04 with LLVM 15). The wrapper caches `target/` and `cargo/registry` in Docker volumes to avoid full rebuilds.

## Architecture

```
src/           — Rust compiler (lexer → parser → type checker → LLVM codegen)
  main.rs      — CLI entrypoint: build / run / doc / transpile subcommands
  lib.rs       — compile_file(): orchestrates lexer → parser → checker → compiler
  compiler.rs  — LLVM IR generation via inkwell (Rust bindings for LLVM)
  runtime.c    — C runtime: I/O, string ops, GC init, FFI builtins
stdlib/        — Aion standard library (written in Aion, .ai files)
tests/
  fixtures/    — .ai test files (001_hello.ai … 042_self_lexer.ai)
  expected/    — .out files with expected program output
compiler/      — Early self-hosting attempt (lexer.ai, token.ai)
conductor/     — Project planning and roadmap docs
```

**Compiler pipeline**: `compile_file()` in `src/lib.rs` runs: imports resolution → TypeChecker → LLVM Compiler → writes `.ll` file.

**`./aion run` flow**: compiles to temp `.ll` → `llc-15` to `.o` (PIC) → `gcc` links with `src/runtime.c` → executes binary → cleans up temp files.

## Testing

- Test runner: `python3 runner.py` at project root
- Tests are `tests/fixtures/*.ai` files; expected outputs in `tests/expected/*.out`
- Output is parsed between `-------------------------------` delimiter lines
- `005_unsafe_check` is an **expected failure** (tests unsafe block enforcement)
- New expected output files are auto-created on first run if missing

## Toolchain Quirks

- **LLVM 15** is hardcoded (inkwell feature `llvm15-0`, Docker installs `llvm-15-dev`)
- **Boehm GC** is required (`-lgc` in link step, `GC_init()` called in main)
- **PIC relocation** is mandatory (`-relocation-model=pic` in llc-15 call)
- **Rust edition 2024** (see `Cargo.toml`)
- Import resolution: `compiler.*` → project root, everything else → `stdlib/`

## Conventions

- All code, comments, variable names, and commit messages in **English only**
- Read `docs/SPEC.md` before contributing to the compiler
- The `.cursorrules` file contains AI interaction rules (ask before acting, no auto-commits)
