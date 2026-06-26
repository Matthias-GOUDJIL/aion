## Architecture

```
src/              — Rust compiler (lexer → parser → type checker → LLVM codegen)
  main.rs         — CLI entrypoint: build / run / doc / transpile subcommands
  lib.rs          — compile_file(): orchestrates lexer → parser → checker → compiler
  error.rs        — CompileError enum (Type, Unsafe, NotFound, NotFunction, InvalidOperator, Inkwell, Io, Import, Internal)
  runtime.c       — C runtime: I/O, string ops, GC init, FFI builtins
  lexer/          — Tokenizer (char → token stream)
    mod.rs        — Re-exports Token, TokenKind, Lexer
    token.rs      — Token and TokenKind definitions
    lexer.rs      — Lexer implementation
  ast/            — AST node definitions
    mod.rs        — Re-exports + Span struct
    expr.rs       — Expression enum
    stmt.rs       — Statement enum + MatchArm
    decl.rs       — Declaration, Function, Struct, Enum, Interface, ImplBlock, Program, Import
  parser/         — Token stream → AST (returns Result<Program, Vec<CompileError>>)
    mod.rs        — Parser struct + parse_program + parse_import
  analysis/       — Type system and verification
    mod.rs        — Re-exports TypeChecker, Environment, Type
    types.rs      — Type enum with from_str/Display
    checker.rs    — TypeChecker: safety and type verification pass
    environment.rs — Scoped symbol table for type checker
  codegen/        — LLVM IR generation via inkwell (Rust bindings for LLVM)
    mod.rs        — Re-exports Compiler
    compiler.rs   — LLVM code generation
    transpiler/   — Transpilation backends
      mod.rs      — Module root
      sql.rs      — SQL transpiler (Aion → PostgreSQL functions)
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

## Known Pitfalls (discovered during bug fixing)

- **All structs → `ptr_type` at LLVM level**: `aion_type_to_llvm` maps every struct/enum/string type to an opaque pointer. Field access uses GEP with the LLVM struct type for offset calculation, but load/store operations are always pointer-sized (8 bytes). `@intrinsic("mem_zero", Type)` now looks up the actual LLVM struct type from `struct_types` and returns a properly-sized zero constant. `@intrinsic("mem_zero_ptr", ptr, size)` calls `aion_memzero` to zero existing memory blocks (fixed #62).

- **String-based type resolution in codegen**: `get_expr_type_name` and `instantiate_function` resolve types via string manipulation (e.g., `replace("V", "String")`). This corrupts type names when a generic parameter appears as a substring of another type name (e.g., `"V"` in `"Vector"` → `"Stringector"`, fixed in #61). `substitute_generic_params()` handles this by replacing only `<Param>` patterns. The same fragility remains in `instantiate_function` and `substitute_types_in_expr` (see #67).

- **`%` operator is unsigned remainder**: Changed from `srem` to `urem` (#66). This ensures `hash % cap` always yields `[0, cap-1]` regardless of hash sign. Code relying on signed remainder semantics for negative numbers would break — no known cases in the current codebase.

- **Static methods receive receiver argument**: The MethodCall codegen always pushed the receiver as the first argument, even for static methods like `new()` that have no `self` parameter. For TypeRef receivers (e.g., `Vector<String>`), `compile_expr` fell through to the catch-all returning `i64 0`, corrupting the calling convention. Fixed in #66 by checking `has_self` before pushing the receiver.

- **HashMap bucket array is an array of pointers (8 bytes/slot)**: `core.heap.alloc(cap * 8)` is correct — each bucket slot stores a single pointer to a heap-allocated Entry (24 bytes). The `@sizeof(Entry<V>)` returns 24 but that's the entry size, not the bucket slot size.

- **Pointer-to-struct deref + member access assumes a boxed-pointee model** (#79 fix): Struct instances are always heap-allocated and stored as 8-byte opaque pointers; `let s = Triple{...}` puts a boxed pointer in `s`. Consequently `(*p).field` for `p: *Triple` compiles to: load 8-byte box pointer from `p`, GEP into the heap struct, load the field. This works when `*p` was assigned a `StructInst` (which stores the box pointer, 8 bytes).

  **Limitation — aliasing with `mem_zero(Type)`**: `@intrinsic("mem_zero", Triple)` returns a 24-byte *struct value* (a zeroed constant of the LLVM struct type), not a boxed pointer. Assigning `*p = @intrinsic("mem_zero", Triple)` stores 24 zero bytes into `p`'s region, overwriting the box pointer that `(*p).field` relies on. A subsequent `(*p).field` then loads a null/stale box pointer, which the LLVM optimizer may forward-substitute to the *original* boxed struct (yielding the stale pre-zero value) or GEP into a null address (crash). This is a codegen model inconsistency: `StructInst` uses the boxed-pointer convention while `mem_zero(Type)` uses an inline-value convention, and the `Deref`+`MemberAccess` path cannot reconcile them. The `memzero_typed.ai` fixture intentionally round-trips through `p as *i64` + `*raw` to avoid hitting this. Fixing this properly requires unifying the struct representation (either make `mem_zero(Type)` return a boxed pointer, or teach `Deref`+`MemberAccess` to handle inline struct values) — tracked separately, not part of #79.

- **Statement-`if` with all-terminating branches emits `unreachable` in the merge block**: `Statement::If` codegen, when both `then` and `else` already end in a terminator (so `phis` is empty), inserts `build_unreachable()` into the merge block. This is correct for `if cond { return } else { return }`, but becomes a hazard when combined with the aliasing issue above: if a later statement's codegen/optimization makes the branches terminate "early", the merge block (containing subsequent statements) is marked unreachable and dead-code-eliminated. Observe IR with `cargo run -- build <file> -o out.ll` when debugging control-flow that vanishes after a pointer/struct assignment.