# Aion Specification: SQL Transpilation

`./aion transpile <file.ai>` lowers a subset of Aion to PostgreSQL (plpgsql)
`CREATE TYPE` / `CREATE OR REPLACE FUNCTION` statements. Implementation:
`src/codegen/transpiler/sql.rs` (#74). Only the `sql` target exists.

The transpiler parses the file and emits SQL. It does **not** run the type
checker or resolve imports — semantic guarantees the checker normally
enforces (exhaustiveness, arity, method resolution) are replaced by the
diagnostics described in §6.

## 1. Declarations

| Aion | SQL |
|---|---|
| Non-generic, payload-free `enum` | `CREATE TYPE "E" AS ENUM ('A', 'B');` |
| Non-generic `struct` with fields | `CREATE TYPE "S" AS (f BIGINT, ...);` |
| Non-generic `fn` | `CREATE OR REPLACE FUNCTION ...` (plpgsql) |
| `fn` with generic params | **error** — SQL functions are not parameterized |
| `fn` without a body (`extern`) | **error** |
| Generic struct / enum | comment + JSONB fallback (use sites warn) |
| Payload enum (any variant with data) | comment; **rejected at use sites** |
| Empty struct / enum | comment + JSONB fallback |

Field types use the type map in §3. Nested struct fields reference the
`CREATE TYPE` emitted above (declaration order).

## 2. Statements

| Aion | SQL |
|---|---|
| `let x = e` / `let mut x = e` | `DECLARE` entry (§4) + `x := e;` |
| `x = e` / `p.x = e` | `x := e;` / `p.x := e;` |
| `return e` | `RETURN e;` |
| `if c { } else { }` | `IF c THEN ... ELSE ... END IF;` |
| `while c { }` | `WHILE c LOOP ... END LOOP;` |
| `break` / `continue` | `EXIT;` / `CONTINUE;` |
| `match` (statement) | searched `CASE` (§5) |
| `io.println(e)` / `io.print(e)` | `RAISE NOTICE '%', e;` |
| expression statement | `PERFORM e;` |
| `unsafe { }` | inlined (comment) |
| `let (a, b) = e` | **error** — no destructuring in SQL |
| `spawn { }` | **error** — SQL is single-threaded |
| `for x in range` | **error** — rewrite as `while` |

All variables of a function (including those of nested blocks) are
flattened into the single `DECLARE` section — plpgsql has exactly one
scope for locals. Shadowing a variable with a different type warns; the
variable keeps its declared type.

## 3. Type mapping

| Aion | SQL |
|---|---|
| `i8..i128`, `u8..u128`, `char` | `BIGINT` (`char` = integer code point, as in native codegen) |
| `f32`, `f64` | `DOUBLE PRECISION` |
| `bool` | `BOOLEAN` |
| `String` | `TEXT` |
| `Date` | `TIMESTAMP` |
| `Duration` | `INTERVAL` |
| `void` / `()` / `Unit` | `VOID` |
| `Option<T>` | mapping of `T` (SQL columns/variables are nullable) |
| declared payload-free enum | `"E"` (PG enum type) |
| declared non-generic struct | `"S"` (PG composite type) |
| `ptr` | **error** — raw pointers have no SQL equivalent |
| anything else (`Vector<T>`, `HashMap`, tuples, arrays, payloads) | `JSONB` + warning |

## 4. Variable typing (DECLARE)

Types come from, in order: explicit `let` annotation, else inference over
the initializer (literals, params, calls to functions declared in the file,
field access on declared structs, enum/struct instantiation, casts, and
the first value-yielding arm of `if`/`match` expressions). Unresolvable
initializers declare `JSONB`.

## 5. Expressions

| Aion | SQL |
|---|---|
| integer / float / bool | literal (floats always keep a decimal shape: `2.0`) |
| string | `'…'` with `''` escaping |
| char `'A'` | `65` (code point) |
| duration `5s` / `1500ms` | `INTERVAL '5 seconds'` / `INTERVAL '1500 milliseconds'` (native = i64 ms, sub-ms truncated) |
| date `D2024-01-01` | `'2024-01-01'::timestamp` (native = epoch ms at midnight UTC) |
| `+` on strings (f-strings desugar here) | `\|\|` (text concatenation) |
| `+ - * / %` | same operators |
| `^` (native integer XOR) | `#` (PG `^` is exponentiation) |
| `== != > < >= <=` | `= != > < >= <=` |
| `&&` / `\|\|` | `AND` / `OR` |
| `a inside x..y` | `(a >= x AND a < y)` — **exclusive** upper bound, like native codegen |
| `e as T` | `(e)::T` (`f64`→integer casts wrap in `trunc()` — native fptosi truncates, PG casts round) |
| `p.x` | `"p"."x"` (every dotted segment is quoted, mixed-case safe) |
| `S { x: e, y: e }` | `ROW(e, e)::"S"` — fields reordered to the declared field order, missing fields `NULL` + warning |
| `E::V` | `'V'` (PG coerces the literal to the enum type) |
| `if` / `match` expression | `CASE WHEN c THEN v ... ELSE v END` |
| `if` without `else` | `ELSE NULL` + warning |
| call to fn declared in the file | `"name"(args)` |

### Builtin mappings

| Aion | SQL |
|---|---|
| `string.from_int` / `string.from_float` | `CAST(e AS TEXT)` |
| `string.to_float` | `CAST(e AS DOUBLE PRECISION)` |
| `string.len` / `String.len` / `s.len()` | `length(s)` |
| `i64.abs` / `n.abs()` | `abs(n)` |
| `i64.max` / `n.max(o)` | `GREATEST(n, o)` |
| `i64.min` / `n.min(o)` | `LEAST(n, o)` |
| `aion_str_substr(s, i, l)` | `substr(s, i + 1, l)` (Aion 0-based → PG 1-based) |
| `aion_str_at(s, i)` | `ascii(substr(s, i + 1, 1))` |
| `aion_char_to_str(c)` | `chr(c)` |
| `exit(c)` / `aion_exit(c)` | `RAISE EXCEPTION 'aion exit (%)', c;` + warning |
| other builtins (`fs_*`, `ai.*`, malloc/ptr, …) | **error** — no SQL equivalent |

Method calls lower through the same table: the parser folds simple
`recv.method(args)` receivers into a dotted call name, and the transpiler
splits it at the last dot when the receiver resolves to a local variable
or a declared type (mirroring the type checker's dispatch). Methods not in
the table are **errors** (stdlib `impl`s are not resolvable without import
resolution).

### Match lowering

Arms map to `WHEN` conditions, in order; native semantics preserved
(`src/codegen/match_lowering.rs`):

- integer pattern `5` → `scrut = 5`
- range pattern `1..9` → `(scrut >= 1 AND scrut <= 9)` — **inclusive** both ends
- multi-pattern `10 | 20` → `OR`-joined conditions
- string pattern `"red"` → `scrut = 'red'`
- enum pattern `Color::Red` → `scrut = 'Red'`
- guard `if c` → `(...) AND (c)`
- `_` (no guard) → `ELSE` branch

A match is provably exhaustive when every variant of a known payload-free
enum is covered by guard-free arms — then no `ELSE NULL` fallback is
emitted. Otherwise:

- statement form always emits `ELSE NULL;` (an unmatched scrutinee must
  not raise `CASE_NOT_FOUND`; the checker that would reject a
  non-exhaustive match does not run),
- expression form emits `ELSE NULL` **with a warning**.

## 6. Diagnostics

Unsupported constructs produce **spanned diagnostics** instead of silently
invalid SQL. The driver prints them (`error:`/`warning:`, `--> line, col`,
source snippet with caret) and exits non-zero on any error, writing no
output file. Warnings do not block generation.

Errors: `spawn`, arrays/tuples (literals, indexing, destructuring),
payload enum instantiation and payload match patterns, binding match arms,
generic functions/struct instantiations, raw pointers, `@intrinsic`,
first-class types, ranges outside `inside`, builtins with no equivalent,
methods outside the mapping table.

Warnings: `JSONB` fallbacks, calls to functions not declared in the file
(imports are not resolved — the callee must exist in the target schema),
`exit()` → `RAISE EXCEPTION`, value-returning `return` in a `void`
function, non-exhaustive match expressions, missing struct fields,
unknown patterns.

## 7. Limitations

- PostgreSQL only (plpgsql, PG ≥ 11); no other SQL dialects.
- One file in, one `.sql` out: imports are not resolved; cross-file calls
  rely on the target schema.
- The type checker does not run: exhaustiveness and arity guarantees are
  the diagnostics above, not compile-time rejections.
- Struct-typed values are PG composites and need their `CREATE TYPE` in
  the target schema (the transpiler emits them, but does not run them).
- Statement-position `return` values from `void` functions are passed
  through; PG will reject them at runtime (warning at transpile time).
- `for` loops, `loop`-with-capture semantics beyond `while`-equivalence,
  and the `?` operator (#138) are unsupported.

## 8. Implementation status

- [x] Statements: let/assign/if/while/break/continue/match/unsafe/PERFORM.
- [x] Expressions: literals (int/float/bool/string/char/duration/date),
  operators (incl. `\|\|` concat, `#` xor, `inside` ranges), casts with
  native truncation semantics, field access, struct/enum instantiation.
- [x] Type mapping table (§3) + CREATE TYPE emission.
- [x] Match statement/expression with guards, ranges, multi-patterns,
  provable exhaustiveness.
- [x] Builtin mappings (§5) incl. folded method dispatch.
- [x] Spanned diagnostics with snippets; error exit codes; no output on error.
- [x] Fixtures: `tests/fixtures/stdlib/sql_{transpile,advanced,match_enum,types}.ai`,
  `tests/fixtures/compiler/sql_error_unsupported.ai`; snapshots under
  `tests/snapshots/integration__transpile_*.snap`.
- [x] Self-hosted parser parity: `Range` infix precedence added to
  `compiler/parser.ai` (the new fixtures exposed the gap).
