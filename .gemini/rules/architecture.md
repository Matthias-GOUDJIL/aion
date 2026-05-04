## Architecture

```
src/              — Rust compiler (lexer → parser → type checker → LLVM codegen)
  main.rs         — CLI entrypoint: build / run / doc / transpile subcommands
  lib.rs          — compile_file(): orchestrates lexer → parser → checker → compiler
  compiler.rs     — LLVM IR generation via inkwell (Rust bindings for LLVM)
  runtime.c       — C runtime: I/O, string ops, GC init, FFI builtins
  lexer.rs        — Tokenizer (char → token stream)
  parser.rs       — Token stream → AST
  ast.rs          — AST node definitions (Program, Declaration, Expression, Statement)
  token.rs        — Token and TokenKind definitions
  types.rs        — Type system (Type enum: Integer, Float, String, GenericInstance, etc.)
  checker.rs      — TypeChecker: safety and type verification pass
  environment.rs  — Scoped symbol table for type checker
  transpiler/     — Transpilation backends
    mod.rs        — Module root
    sql.rs        — SQL transpiler (Aion → PostgreSQL functions)
stdlib/           — Aion standard library (written in Aion, .ai files)
  core/           — Core memory primitives (heap.ai, memory.ai)
  std/            — Standard library modules (io, fs, collections, math, etc.)
  web/            — Web primitives (dom.ai)
tests/
  fixtures/       — .ai test files (001_hello.ai … 045_self_parser.ai)
  expected/       — .out files with expected program output
compiler/         — Self-hosting compiler (lexer.ai, token.ai, ast.ai, parser.ai)
conductor/        — Project planning and roadmap docs
examples/         — Example Aion programs
editors/          — Editor integrations (vscode/)
```

**Compiler pipeline**: `compile_file()` in `src/lib.rs` runs: imports resolution → TypeChecker → LLVM Compiler → optimization passes (FPM + MPM) → writes `.ll` file.

**`./aion run` flow**: compiles to temp `.ll` → `llc-15` to `.o` (PIC) → `gcc` links with `src/runtime.c` → executes binary → cleans up temp files.

## Architectural Invariants

- **Enum layout**: compiled as `{ i64, [64 x i8] }`. Index 0 = Tag, Index 1 = Payload. Tags: `Some/Ok = 0`, `None/Err = 1`
- **Global args**: `argc`/`argv` stored in global LLVM variables `aion_argc`/`aion_argv`, set in `main`
- **`io.println`** only accepts `String`. Convert non-string types with `string.from_int`, `string.from_float`, etc.
- **Block termination**: every LLVM basic block in `compiler.rs` must be terminated. Check `builder.get_insert_block().unwrap().get_terminator().is_none()` before adding default returns
- **Inkwell safety**: use `ValueKind` matching for call results instead of `inkwell::values::Either`

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