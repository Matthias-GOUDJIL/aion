# Aion Specification v0.6 (Current Implementation)

This document describes the **currently implemented** behavior of the Aion
compiler. It is the source of truth for what the compiler actually does —
`ROADMAP.md` describes what is planned. When code changes behavior, update
this file in the same commit.

## 1. Architecture

Aion is a direct-to-LLVM compiler with a safety-first pipeline.

- **Frontend**: hand-written Lexer (`src/lexer/`) and recursive-descent
  Parser (`src/parser/`) producing an AST tagged with `Span` (line, col).
- **Middle**: Type Checker (`src/analysis/`) with scoped environments,
  fuzzy name resolution, and strict generic validation.
- **Backend**: direct LLVM IR generation (`src/codegen/`) via `inkwell`
  (LLVM 15, opaque pointers). A SQL transpiler lives in
  `src/codegen/transpiler/sql.rs`.
- **Runtime**: minimal C runtime (`src/runtime.c`) for I/O, string ops,
  threading, and AI tensor primitives. Links against **Boehm GC**
  (`libgc`) and `pthread`.
- **Orchestration**: Docker-first execution via the `./aion` wrapper.
  Host `cargo` commands are forbidden for compilation — use the
  `aion-compiler` Docker image.

```
source → Lexer → Parser → TypeChecker → LLVM Codegen → FPM+MPM → .ll
                                                                ↓
                                              llc-15 (PIC) → .o
                                                                ↓
                                            gcc links runtime.c → binary
```

`compile_file()` in `src/lib.rs` orchestrates imports → TypeChecker →
LLVM Compiler → optimization passes → writes the `.ll` file.

## 2. Type System

### Primitives
- `i64` (default integer), `f64` (float), `bool`, `String` (pointer to
  C-string), `Duration` (i64 millis), `Date` (i64 millis timestamp).
- Char literals: `'a'` → integer char code.
- Type system is **monomorphic at codegen**: only `i64`, `f64`, `bool`,
  `String`, `Duration`, `Date` exist at the LLVM level. All composite
  types lower to opaque pointers (see §3).

### Composite
- `Struct`, `Enum` (tagged unions), `Interface`, `Impl` blocks.
- **Enums** compile as `{ i64, [64 x i8] }` — index 0 = Tag, index 1 =
  Payload (64 bytes). Tags: `Some/Ok = 0`, `None/Err = 1`.
- **Generics**: monomorphization at compile time. Full type substitution
  before LLVM generation. Generic params are substituted via
  `<Param>`-pattern matching to avoid corrupting nested type names.
- **Pointers**: `*T` for explicit pointer types. All complex types are
  passed by reference.
- **Fuzzy resolution**: if a simple name is not found, the compiler
  searches for qualified names ending with that suffix (e.g. `args` →
  `std.env.args`). Avoids complex AST rewriting for imports.

## 3. Memory & Strings

- **Memory model**: automatic management via **Boehm GC** (`libgc`).
  `GC_init()` is called in the synthesized `main`. No RAII, no
  destructors, no linear types yet (tracked in ROADMAP).
- **All structs/enums/strings lower to opaque `ptr`** at the LLVM level.
  Field access uses GEP with the LLVM struct type for offset calculation,
  but every field slot is pointer-sized (8 bytes). Known limitation:
  `@intrinsic("mem_zero")` writes only 8 bytes — it cannot zero a
  multi-field struct (issue #62).
- **Strings**: C-style `char*`. Escape sequences in regular strings:
  `\n`, `\t`, `\r`, `\\`, `\"`, `\0`.
- **f-strings** (v0.6): `f"Hello {name}!"` is syntax sugar for
  left-associative string concatenation. Each `{expr}` segment is parsed
  as a full Aion expression and joined with `+`. Non-String expressions
  must be explicitly converted with `string.from_int`,
  `string.from_float`, etc. — Aion does not yet auto-stringify at
  interpolation sites.
- **Concat**: `String + String` lowers to `aion_str_concat` (allocates a
  new buffer). `String + i64` treats the integer as a single char code
  via `aion_char_to_str` — do not use this for integer formatting.
- **Comparison**: `==` and `!=` compare string content via `aion_str_eq`
  when both operands are `String`; otherwise pointer comparison.
- **`mem_zero`**: `@intrinsic("mem_zero")` returns a null pointer (8 bytes). `@intrinsic("mem_zero", Type)` returns a properly-sized zero constant for struct/enum types by looking up the actual LLVM struct type. For primitives, returns the zero value of that type.
- **`mem_zero_ptr`**: `@intrinsic("mem_zero_ptr", ptr, size)` calls `aion_memzero` to zero `size` bytes of existing memory. Used for in-place zeroing of bucket arrays and reusable buffers.
- **`mem_is_null`**: `@intrinsic("mem_is_null", ptr)` returns `true` if the pointer is null.
- **`sizeof`**: `@intrinsic("sizeof", Type)` returns the LLVM size of the type in bytes. Works on both variable instances and type names.
- **Safety**: `unsafe` blocks required for pointer dereferences, FFI, and specific intrinsics (see `docs/SPEC_SANDBOX.md`).

## 4. Temporal Primitives

First-class `Duration` and `Date` literals (see `docs/SPEC_TIME.md`).

- **Duration literals**: `5s`, `500ms`, `10us`, `2ns`, `2h`, `30m`.
  Internally stored as `i64` milliseconds.
- **Date literals**: `D2024-01-01` → `i64` millisecond timestamp.
- **Arithmetic**: `Date + Duration → Date`, `Date - Duration → Date`,
  `Duration + Duration → Duration`, `Duration / Duration → float`.

## 5. Compiler Robustness

- **Error handling**: zero `unwrap()` in production paths. All compiler
  stages return `Result<T, CompileError>` with typed variants: `Type`,
  `Unsafe`, `NotFound`, `NotFunction`, `InvalidOperator`, `Inkwell`,
  `Io`, `Import`, `Internal`.
- **Block termination**: every LLVM basic block is guaranteed to be
  terminated. The compiler injects `ret` or `unreachable` when
  `get_terminator().is_none()`.
- **Type safety**: `Type::Unknown` is never returned silently. Fallbacks
  are explicit and validated.
- **Debug hygiene**: no `println!`/`eprintln!` in production builds.
- **Span tracking**: all AST nodes carry `Span` (line, col) for precise
  error reporting. Quality of error messages is tracked by issue #40.
- **Recursion**: full self- and mutual-recursion. Two-pass compilation
  (register prototypes, then compile bodies) enables forward references.
  Tested up to 1B+ recursion depth (limited only by the OS 8MB stack).

## 6. Concurrency & AI-Native Features

- **Threading**: 1:1 via `pthread`. `spawn { ... }` creates detached
  threads. `aion_spawn` is currently a C runtime builtin — rewriting it
  in Aion is a ROADMAP item (Self-Runtime).
- **AI Tensors**: first-class `std.ai.tensor` with `zeros`, `ones`,
  `rand`, `matmul`, `backward` (autograd), `to(device)`.
- **Intents**: `::intent "..."` syntax for AI-guided compilation hints.
  Handled as a `NoOp` statement (not an expression).
- **Short-circuiting**: `&&` and `||` use lazy evaluation via
  conditional branching.
- **Pipeline operator**: `data |> filter |> map` for fluid data
  transforms.
- **Method chaining**: returns from method calls chain directly.

## 7. Transpilation

- **SQL transpiler** (`src/codegen/transpiler/sql.rs`, `./aion transpile`):
  compiles a subset of Aion to PostgreSQL functions. Supports `if`
  blocks and complex expressions. Status: **IN PROGRESS** (ROADMAP
  Phase 1.5).

## 8. Environment & Build

- **Docker-first**: LLVM 15 dependencies are isolated in the
  `aion-compiler` Docker image (Ubuntu 22.04 base).
- **Wrapper**: `./aion` is the primary entry point for `build`, `run`,
  `doc`, `transpile` subcommands.
- **Testing**: `cargo test` with `insta` snapshots + `assert_cmd` CLI
  tests, run inside Docker. See `docs/testing.md`.
- **Import resolution**: `compiler.*` → project root, everything else →
  `stdlib/`. Recursive imports with automatic namespace prefixing to
  prevent symbol collisions. Local declarations are renamed **before**
  recursing to avoid exponential prefix concatenation.

## 9. Self-Hosting Progress

The Aion-written compiler lives in `compiler/`:

- `lexer.ai`, `token.ai` — Aion lexer (v0.7, validated against the
  Rust bootstrap compiler).
- `parser.ai`, `ast.ai` — Aion parser (v0.8, AST manipulation via Aion
  `struct` and `match`).
- **v0.9 (open)**: direct generation of `.ll` text files from the AST,
  in Aion. Tracked by issue #9.

## 10. Known Limitations

- No RAII / destructors (GC-only memory model).
- LLVM opaque pointers require explicit type casting in edge cases.
- Cross-compilation targets are not yet supported (x86_64-linux only).
- `instantiate_function` and `substitute_types_in_expr` still use naive
  string-based type resolution (#67).
- f-string interpolation requires explicit `string.from_*` wrapping for
  non-String values.
- `std.json` cannot parse arrays or objects (type checker limitation on
  non-generic `Vector<Value>` — see `stdlib/std/json/SPEC.md`).
- Integer widths are not distinguished (only `i64`); `i8/i32/u64` are
  not yet supported (#52).

## 11. Backwards-Incompatible Notes

- The `%` operator is **unsigned remainder** (`urem`), not signed.
  `hash % cap` always yields `[0, cap-1]` regardless of hash sign.
- Static methods (e.g. `Vector::new()`) do **not** receive a `self`
  argument. The receiver is only pushed for instance method calls.
