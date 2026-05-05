## Architecture

```
src/              — Rust compiler (lexer → parser → type checker → LLVM codegen)
  main.rs         — CLI entrypoint: build / run / doc / transpile subcommands
  lib.rs          — compile_file(): orchestrates lexer → parser → checker → compiler
  compiler.rs     — LLVM IR generation via inkwell (Rust bindings for LLVM)
  runtime.c       — C runtime: I/O, string ops, GC init, FFI builtins
  lexer.rs        — Tokenizer (char → token stream)
  parser.rs       — Token stream → AST (returns Result<Program, Vec<CompileError>>)
  ast.rs          — AST node definitions with Span (Program, Declaration, Expression, Statement)
  token.rs        — Token and TokenKind definitions
  types.rs        — Type system (Type enum with from_str/Display)
  error.rs        — CompileError enum (Type, Unsafe, NotFound, NotFunction, InvalidOperator, Inkwell, Io, Import, Internal)
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
  fixtures/
    language/     — Language features (if, while, match, generics, etc.)
    stdlib/       — Standard library tests (io, fs, collections, etc.)
    compiler/     — Compiler tests (self-hosting, optimization, FFI, etc.)
  snapshots/      — insta snapshot files (committed to git)
  tmp/            — Temporary test artifacts (gitignored)
compiler/         — Self-hosting compiler (lexer.ai, token.ai, ast.ai, parser.ai)
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

## Lessons Learned

- **Pass-by-pointer for `self`**: Methods that modify `self` must receive it as a pointer, not by value. Otherwise changes are local-only.
- **Two-pass compilation**: Forward references require registering all function prototypes before compiling bodies (Pass 1: register, Pass 2: compile).
- **Fuzzy name resolution**: If a simple name isn't found, the compiler searches for qualified names ending with that suffix (e.g., `args` → `std.env.args`). This avoids complex AST rewriting for imports.
- **`::intent` as NoOp**: Metadata like `::intent` must be handled as a `NoOp` statement, not an expression, to avoid "undefined variable" errors.
- **Opaque pointers**: LLVM 15+ uses opaque pointers. `build_load` requires the element type explicitly.
- **Recursive imports**: `process_imports` must rename local declarations *before* recursing to avoid exponential prefix concatenation.
- **Assignment is a statement**: `a = b` is a statement in Aion, not an expression.