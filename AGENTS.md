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
- **Always run `python3 runner.py` after any compiler change before committing**
- **Create a test for every new feature or bug fix** — if behavior changes, add or update a fixture

## Architectural Invariants

- **Enum layout**: compiled as `{ i64, [64 x i8] }`. Index 0 = Tag, Index 1 = Payload. Tags: `Some/Ok = 0`, `None/Err = 1`
- **Global args**: `argc`/`argv` stored in global LLVM variables `aion_argc`/`aion_argv`, set in `main`
- **`io.println`** only accepts `String`. Convert non-string types with `string.from_int`, `string.from_float`, etc.
- **Block termination**: every LLVM basic block in `compiler.rs` must be terminated. Check `builder.get_insert_block().unwrap().get_terminator().is_none()` before adding default returns
- **Inkwell safety**: use `ValueKind` matching for call results instead of `inkwell::values::Either`

## Coding Standards

- **Completion first**: never start a new feature if existing ones are incomplete or untested
- **No dead code**: remove commented-out blocks and unused code immediately
- **No debug prints**: use `eprintln!` with feature flags, not `println!` for debugging
- **SPEC alignment**: code behavior must match `docs/SPEC.md`. If code changes, update SPEC first
- **Zero-cost abstractions**: verify generated IR is optimal, isolate `unsafe` blocks, use `Result<T, E>` extensively

## Toolchain Quirks

- **LLVM 15** is hardcoded (inkwell feature `llvm15-0`, Docker installs `llvm-15-dev`)
- **Boehm GC** is required (`-lgc` in link step, `GC_init()` called in main)
- **PIC relocation** is mandatory (`-relocation-model=pic` in llc-15 call)
- **Rust edition 2024** (see `Cargo.toml`)
- Import resolution: `compiler.*` → project root, everything else → `stdlib/`

## Debugging

- **LLVM IR inspection**: run `./aion build file.ai` and check `output.ll` for `undef`, `null`, or misaligned pointers
- **Verify IR**: `opt-15 -verify output.ll`
- **GDB**: `gdb --args ./aion build file.ai`
- **AddressSanitizer**: `RUSTFLAGS="-Z sanitizer=address" cargo build` (requires nightly)

## Conventions

- All code, comments, variable names, and commit messages in **English only**
- Read `docs/SPEC.md` before contributing to the compiler
- **Never run git add, commit, or push without explicit user approval**

## OpenCode Workflow

- **`/task`** — Delegate complex multi-step investigations. Use for broad searches or refactors (e.g., "Find all `unwrap()` in `src/` and replace with proper error handling"). Runs a dedicated agent in parallel.
- **`/grep "pattern"`** — Fast content search across the codebase. Prefer over asking the agent to search manually.
- **`/glob "pattern"`** — Find files by name pattern (e.g., `/glob "*.ai" stdlib/`).
- **`/read <file>`** — Inspect files directly without waiting for the agent.
- **`/todo`** — Track multi-step work. Use when changes span multiple files or require sequential steps.
- **`/edit`** — Make targeted edits to files. Prefer over asking the agent to rewrite entire files.
- **`/bash`** — Run shell commands. The agent uses this for builds, tests, and git operations.
- **Docker cache gotcha**: if tests fail with linker errors (`undefined reference`, `file in wrong format`, `cannot find temp_*.o`), the Docker image or volumes are stale. Run: `docker rmi aion-compiler && docker volume rm aion-target-cache aion-cargo-cache && docker build -t aion-compiler .`
