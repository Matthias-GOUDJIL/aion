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
    compiler.rs   — Compiler struct, compile_function, module entry + lowerings (split in progress, see #113)
    intrinsics.rs — Builtin/intrinsic registration + token-aware generic substitution (phase 1)
    types.rs      — AionType → LLVM type lowering (phase 2)
    generics.rs   — Generic function instantiation + body/expr substitution (phase 3)
    control_flow.rs — Statement-level codegen: compile_block (let/return/if/while/match/unsafe) (phase 4)
    lvalues.rs      — Assignment-target lowering: compile_lvalue (field/index/deref) (phase 5)
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

**`./aion run` flow**: compiles to temp `.ll` → `llc-15` to `.o` (PIC) → `clang-15 -fuse-ld=lld` links the object with the pre-compiled runtime bitcode (`/opt/aion_runtime.bc` in the Docker image, env `AION_RUNTIME_BC`) + `-lpthread -lgc` → executes binary → cleans up temp files. The link path is gcc-free (lld-15 driven by clang-15, both LLVM tools); a legacy `gcc` fallback is used only when `clang-15` is absent (non-Docker dev without the LLVM toolchain). #73.

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
- **gcc-free link path**: the user binary is linked with `clang-15 -fuse-ld=lld` (lld-15) against the pre-compiled runtime bitcode. `gcc` remains in the Docker image only as the C compiler for `cargo`/`build-optional` deps (inkwell, llvm-sys); it is **not** invoked by `aion run`. #73.
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

- **String-based type resolution in codegen**: `get_expr_type_name` resolves types via string manipulation. Generic-parameter substitution used to use naive `str::replace(p, c)`, which corrupted type names when a generic parameter appeared as a substring of another type name (e.g. param `V` corrupting `Vector` → `Stringector`, fixed in #61 via `<Param>`-pattern matching; param `T` corrupting `Tensor` → `i64ensor`, fixed in #67). All generic substitution now goes through a single tokenizer-based helper, `substitute_type_string` (`src/codegen/intrinsics.rs`), which scans the type string into identifier tokens and replaces a token only when it EXACTLY equals a param name. This is lossless because Aion's type syntax only ever uses a generic param as a standalone identifier (`T`, `*T`, `Vector<T>`, `HashMap<K, V>`). `substitute_generic_params` now delegates to it. A token containing `.` (a qualified name like `std.foo`) is never replaced — generic params are always bare identifiers.

- **`%` operator is unsigned remainder**: Changed from `srem` to `urem` (#66). This ensures `hash % cap` always yields `[0, cap-1]` regardless of hash sign. Code relying on signed remainder semantics for negative numbers would break — no known cases in the current codebase.

- **Static methods receive receiver argument**: The MethodCall codegen always pushed the receiver as the first argument, even for static methods like `new()` that have no `self` parameter. For TypeRef receivers (e.g., `Vector<String>`), `compile_expr` fell through to the catch-all returning `i64 0`, corrupting the calling convention. Fixed in #66 by checking `has_self` before pushing the receiver.

- **HashMap bucket array is an array of pointers (8 bytes/slot)**: `core.heap.alloc(cap * 8)` is correct — each bucket slot stores a single pointer to a heap-allocated Entry (24 bytes). The `@sizeof(Entry<V>)` returns 24 but that's the entry size, not the bucket slot size.

- **Struct representation is uniformly boxed** (#87): `StructInst`, `EnumInst`, and `@intrinsic("mem_zero", Struct/EnumType)` all heap-allocate via `aion_malloc` and yield an 8-byte opaque **box pointer** as their value. `let s = Triple{...}` and `*p = mem_zero(Triple)` therefore both store an 8-byte box pointer; `(*p).field` (Deref + MemberAccess in `src/codegen/compiler.rs`'s `compile_expr`) loads that box pointer, GEPs into the heap struct, and loads the field. This unified convention is what makes `(*p).field` consistent regardless of how `*p` was last written. Earlier, `mem_zero(Type)` returned an inline struct *value* (24 bytes) which broke `(*p).field` after `*p = mem_zero(T)` (#87); the no-arg `mem_zero` (→ null pointer) and primitive `mem_zero(i64)` (→ 0) paths were already consistent and are unchanged.

  **Caveat — storing an inline struct value is still unrepresentable**: Aion has no by-value struct storage; any expression that materializes a struct must do so boxed. Do not return raw `st.const_zero()` struct values from intrinsics — store them into a fresh box and return the box pointer (see the `mem_zero` struct/enum arms in `compile_expr`). The `Deref`+`MemberAccess` path assumes every 8-byte slot it loads is a box pointer.

- **Statement-`if` with all-terminating branches emits `unreachable` in the merge block**: `Statement::If` codegen, when both `then` and `else` already end in a terminator (so `phis` is empty), inserts `build_unreachable()` into the merge block. This is correct for `if cond { return } else { return }`, but becomes a hazard when combined with the aliasing issue above: if a later statement's codegen/optimization makes the branches terminate "early", the merge block (containing subsequent statements) is marked unreachable and dead-code-eliminated. Observe IR with `cargo run -- build <file> -o out.ll` when debugging control-flow that vanishes after a pointer/struct assignment.