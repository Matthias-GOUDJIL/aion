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
- Integer types: `i8`, `u8`, `i32`, `u32`, `i64` (default), `u64`. All are
  distinct in the type system and codegen emits the matching LLVM width
  (`i8_type()`, `i32_type()`, ...). Integer literals default to `i64` and
  coerce to any annotated integer type at `let`/`return` (`let x: i32 = 0`
  stores an `i32`). Arithmetic requires both operands to share the same
  bit width; mixing `i32` and `i64` is a type error (#52). Same-width
  signed/unsigned mixing (`i64 ^ u64`) is allowed, result takes the LHS.
- `f64` (float), `bool`, `String` (pointer to C-string), `Duration` (i64
  millis), `Date` (i64 millis timestamp).
- Char literals: `'a'` → integer char code.
- Type system is **monomorphic at codegen**: each integer keeps its width,
  `f64`, `bool`,
  `String`, `Duration`, `Date` exist at the LLVM level. All composite
  types lower to opaque pointers (see §3).

### Composite
- `Struct`, `Enum` (tagged unions), `Interface`, `Impl` blocks.
- **Enums** compile as `{ i64, [64 x i8] }` — index 0 = Tag, index 1 =
  Payload (64 bytes). Tags: `Some/Ok = 0`, `None/Err = 1`.
- **Generics**: monomorphization at compile time. Full type substitution
  before LLVM generation. Generic params are substituted via token-aware
  identifier matching (`substitute_type_string`) to avoid corrupting
  nested type names (param `T` no longer clobbers `Tensor`). Explicit
  instantiation supports single- and multi-arg calls (`foo<A, B>(...)`)
  and nested arg lists (`Vector<HashMap<K, V>>`); the comma separating
  args is recognized inside the angle list.
- **Pointers**: `*T` for explicit pointer types. All complex types are
  passed by reference. Dereferencing `*T` yields `T` in the type checker,
  so `(*p).field` works for `p: *Struct` (member access also resolves when
  the pointee is a struct placeholder, e.g. `*Triple`). `*i64` derefs to
  `i64`; member access on a non-struct deref is a type error.
- **Tuples**: `(T, U, ...)` — heterogeneous fixed-size sequences (#53).
  Tuple literals `(1, "x", true)`, field access by index `pair.0`/`pair.1`,
  and destructuring `let (a, b) = pair` are supported. Functions may
  return tuples (`fn f() -> (i64, String)`) and callers destructure the
  result. Tuples lower to anonymous LLVM struct types, heap-allocated and
  passed by pointer (same convention as `StructInst`). Nested tuples
  `(i64, (String, bool))` are supported; the element type is recovered
  via depth-aware type-name parsing.
- **Arrays**: `[T; N]` — homogeneous fixed-size sequences (#54). Array
  literals `[1, 2, 3]`, index reads `arr[i]`, index writes `arr[i] = v`,
  and passing arrays to functions are supported. All elements must share
  the same type. Arrays are stack-allocated as LLVM `[N x T]` types and
  passed by pointer. Indexing is bounds-checked at runtime; an
  out-of-bounds access calls the `aion_array_oob(idx, len)` trap
  (stderr message + `exit(1)`) instead of producing undefined behavior.
  `@sizeof([T; N])` returns `N * sizeof(T)`.
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
- **`mem_zero`**: `@intrinsic("mem_zero")` (no argument) returns a null pointer (8 bytes) — used to null-init pointer slots. `@intrinsic("mem_zero", Type)` returns the zero value of `Type` in the language's native representation: for struct/enum types it returns a **boxed pointer** to a freshly heap-allocated, zeroed struct/enum (same convention as `StructInst`/`EnumInst`), so `*p = @intrinsic("mem_zero", T)` then `(*p).field` is consistent; for pointer types it returns a null pointer; for primitives it returns the zero value of that type.
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
  `Io`, `Import`, `Internal`, `Warning`. Every variant carries `line`,
  `col`, and `snippet` (snippets are populated when the location is
  known; `Inkwell`/`Io`/`Import` location-default to `0`/`None` when the
  error has no source position — e.g. a file-read failure). #40.
- **`internal compiler error:` prefix**: the `Internal` variant renders
  its message with a leading `internal compiler error: ` so user-facing
  type errors are visually distinct from compiler bugs. #40.
- **"Did you mean X?" suggestions**: undefined-function, -field, and
  -method errors (via the `NotFound` variant) carry a Levenshtein-closest
  suggestion from the candidate symbols in scope ("Not Found: function
  'greet_word' is not defined (did you mean 'greet_world'?)"). Suggestion
  is only surfaced when the closest candidate is within 3 edits AND within
  `max(1, len/3)` of the typed name, so short typos do not match unrelated
  short names. #40.
- **`Warning` variant**: a non-fatal variant (renders `warning: ...`) for
  future passes (unused-var, dead-code, ...). `CompileError::is_warning()`
  lets the driver print to stderr without halting compilation. #40.
- **Block termination**: every LLVM basic block is guaranteed to be
  terminated. The compiler injects `ret` or `unreachable` when
  `get_terminator().is_none()`.
- **Type safety**: `Type::Unknown` is never returned silently. Fallbacks
  are explicit and validated.
- **Debug hygiene**: no `println!`/`eprintln!` in production builds.
- **Span tracking**: all AST nodes carry `Span` (line, col) for precise
  error reporting.
- **Recursion**: full self- and mutual-recursion. Two-pass compilation
  (register prototypes, then compile bodies) enables forward references.
  Tested up to 1B+ recursion depth (limited only by the OS 8MB stack).

### Variable Bindings

- `let name = expr` — inferred-type binding (default).
- `let name: Type = expr` — explicit type annotation. The annotation
  wins over inference for the variable's stored type and the LLVM alloca
  type, disambiguating cases where inference is ambiguous (e.g.
  `let p: *Triple = unsafe { core.heap.alloc(24) as *Triple }`,
  `let m: HashMap<String> = ...`, intrinsics that lower to `i64` but
  carry a struct/pointer meaning). The value is coerced to the annotated
  LLVM type when needed (int↔pointer). #78.
- `let mut name = expr` / `let mut name: Type = expr` — mutable binding.
- Assignment is a statement (`name = expr`), not an expression.

## 6. Concurrency & AI-Native Features

- **Threading**: 1:1 via `pthread`. `spawn { ... }` creates detached
  threads. `aion_spawn` is currently a C runtime builtin — rewriting it
  in Aion is a ROADMAP item (Self-Runtime).
- **AI Tensors**: first-class `std.ai.tensor`. Constructors `zeros`,
  `ones`, `rand` are functional; `matmul`, `add` and `backward` (autograd)
  are runtime placeholders, not yet implemented (see `docs/STDLIB.md` and
  `src/runtime.c:220-235`).
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
- f-string interpolation requires explicit `string.from_*` wrapping for
  non-String values.
- `std.json` cannot parse arrays or objects (type checker limitation on
  non-generic `Vector<Value>` — see `stdlib/std/json_SPEC.md`).
- Integer arithmetic between **different bit widths** (e.g. `i32 + i64`)
  is a type error. Same-width signed/unsigned mixing (`i64 ^ u64`, used
  by the stdlib hash) is allowed; the result takes the LHS type. #52.

## 11. Backwards-Incompatible Notes

- The `%` operator is **unsigned remainder** (`urem`), not signed.
  `hash % cap` always yields `[0, cap-1]` regardless of hash sign.
- Static methods (e.g. `Vector::new()`) do **not** receive a `self`
  argument. The receiver is only pushed for instance method calls.
