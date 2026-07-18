# `compiler/codegen.ai` — Blockers Audit (#128, sub-issue of #9)

> Output of #128. Read this before starting any of #129 → #135. Every line
> of the existing Rust codegen was scanned and cross-referenced against the
> Aion language (SPEC.md v0.6), the stdlib actually used by `compiler/lexer.ai`
> and `compiler/parser.ai`, and the working fixtures. Each blocker that
> requires a language change is opened as its own issue.

## 1. Capability matrix — Rust module ↔ Aion features

| Rust mod | LOC | Responsibility | Aion features required | Status |
|---|---:|---|---|---|
| `types.rs` | 163 | `Type → LLVM type` lowering + array/tuple/coercion helpers | String building, match on enum, i8..i64 width dispatch | ✅ ready |
| `intrinsics.rs` | 133 | `register_builtins`, `substitute_type_string` (token-aware generic sub) | Vector<(String, String, String, bool)>, mutable HashMap<String, Declaration> | ✅ ready |
| `compiler.rs` | 530 | Module setup, extern decls (35 functions), struct/enum type registration, `compile_function`, optimization, file write | Struct/Enum/HashMap codegen state, fs.write, match dispatch | ⚠️ blocker B1 (snapshot ordering) |
| `control_flow.rs` | 748 | All `Statement` lowering, basic-block management, match on enum + primitives (String/i64/range), `break`/`continue` loop stack | Match on enum (22 variants in `Expression`, 10 in `Statement`), nested blocks, AA-stack via Vector\<BasicBlock> | ✅ ready |
| `expressions.rs` | 1684 | All `Expression` lowering, GEP for struct/enum field, f-string cache, indirect call | String concat, vector of strings, match on enum | ⚠️ blocker B2 (string formatting ergonomics) |
| `generics.rs` | 211 | Monomorphization: substitute params, clone declaration, emit | String manipulation, recursion | ✅ ready |
| `lvalues.rs` | 175 | Compile lvalue (path access, struct member, deref) | Pointer ops, match, recursion | ✅ ready |
| `type_helpers.rs` | 384 | `get_field_type`, `get_expr_type_name` (infer type of expression) | String parsing for generics `Foo<...>` (depth-aware), match | ✅ ready |

Symbol legend: ✅ ready with current Aion — ⚠️ needs a workaround or a small upstream change.

## 2. Confirmed Aion capabilities (already proven by v0.7 lexer + v0.8 parser)

The self-hosted front end already exercises everything the codegen needs at
the language level:

- **Struct/Enum/Match/Generics/Tuples/Arrays** — `ast.ai` declares all of
  these, `parser.ai` builds them (39 `match` statements on the 73-variant
  `TokenKind` enum — strictly larger than the 22-variant `Expression`).
- **`mut self` + stateful structs** — `Parser` mutates `self.current`,
  `self.tokens` via `mut self` methods; same pattern works for a `Codegen`
  struct holding `output: Vector<String>`, `ssa_counter: i64`,
  `struct_types: HashMap<String, StructInfo>`.
- **`Option<T>` / `Result<T, E>`** with `unwrap`, `unwrap_or`, `expect`,
  `is_some`, `is_err`. The parser already returns `Result<ast.Statement,
  String>` everywhere instead of `?`.
- **Pointers** `*T`, `*mut T` (per `std.mem`), deref, `as` casts, `unsafe`
  blocks — available.
- **Function pointers + `fn(P1) -> Ret` annotation** (#84) — used by the
  Rust codegen in `expressions.rs:88-120` for indirect calls.
- **f-string interpolation** — 5 fixtures (`fstring_*`) prove it; LLVM IR
  text with `%t{idx} = add i64 {lhs}, {rhs}` will work.
- **File I/O** — `std.fs.write`, `std.fs.append`, `std.fs.exists`,
  `std.fs.read_to_string`. Sufficient to emit the `.ll` to disk.
- **String ops** — `len, concat, from_int, from_float, equals, at, substr,
  contains, find, trim, replace, to_upper, to_lower`. Sufficient for name
  mangling (`fn<i64>` → `fn_i64`), generic substitution, escape pass.
- **Vector\<String>** — used pervasively in `ast.ai` and `parser.ai`.
- **HashMap\<String, V>** — used by stdlib `std.collections.map` with
  insert/get/keys/values/remove. Used by parser for non-AST lookups — YES
  this exists; semantics proven.
- **`@intrinsic("...")`** for runtime calls — available; the Aion codegen
  itself will NOT call intrinsics (it emits text), so this is unused.
- **Tail recursion + deep nested match** — exercised by parser.ai at 657
  lines (deep expression parsing).

## 3. Blockers identified

### B1. Deterministic snapshot ordering of `HashMap` iteration → critical

The Rust codegen uses `HashMap<String, Declaration> self.decls` and
`HashMap<String, StructType> self.struct_types`. Multiple pass-loops over
`map.keys()` / `self.decls.values()` produce IR whose order depends on
Rust's randomized `RandomState` seed. Today the Rust tests pass because
the C-compiled output of each fixture is order-independent at runtime,
**but the snapshot `.ll` (insta) tests will be non-deterministic**.

**Aion fix**: when porting, replace `HashMap<String, Declaration>` with a
deterministic ordered structure. Two options — Aion's `HashMap` iteration
order is currently unspecified (its `keys()` and `values()` methods have
already lost entries in #66). Choose either:

- **(preferred)** Open a new `OrderedMap<K, V>` stdlib module with
  insertion-order iteration (`insert`/`get`/`keys_in_order`/`values_in_order`)
  backed by `Vector<(K,V)>`. No upstream language change needed.
- **(alternative)** Fix and document `HashMap` iteration as
  insertion-ordered. Touches `stdlib/std/collections/map.ai`.

Opened as **issue #136 — OrderedMap stdlib module** (`priority-high`,
`type-feature`, `area-stdlib`, blocks #129).

### B2. String formatting ergonomics → medium (workable)

Rust uses `format!("  %t{} = add i64 {}, {}\n", n, lhs, rhs)` 1000+ times.
Aion has `std.fmt.format(template, Vector<String>)` but the parser/lexer
**do not use it** — they prefer `string.concat(string.from_int(n), ...)`.
The codegen port can be written entirely with `string.concat` +
`string.from_int` + f-string interpolation; no upstream change required,
**but source expressiveness will be markedly worse**.

**Recommendation**: add a convenience `std.fmt.s(template: String, args:
Vector<String>)` placeholder-expanding function (`{0}`, `{1}`, `{0}` → args),
backed by `string.replace` and `string.find`. This is a stdlib-only change,
no compiler work. Issue opened as **#137** (`priority-medium`,
`area-stdlib`, `type-feature`; non-blocking for #129 but recommended before
#131/#9.3 where it bites hardest).

### B3. `Result::?` propagation operator → low (not blocking)

Every Rust codegen function returns `Result<_, CompileError>` and uses `?`
~50 times per file. Aion has no `?` operator yet (verified: no fixture uses
it, no SPEC line documents it). The parser uses `unwrap` + explicit match
on `Result::Err(msg)`, which is verbose but works. 5000 LOC of codegen
without `?` will be ~600-1000 extra lines. Allocation cost is acceptable.

**Recommendation**: keep using explicit `match` / `unwrap_or` patterns; defer
`?` to a separate ROADMAP item (not part of Phase 2). Tracked in
**#138 — `?` operator for `Result<T, E>`** (`priority-low`, `type-feature`,
`area-parser`, `area-checker`; phase-1.x cleanup).

### B4. `escape_llvm_c_string_literal(String) -> String` → low (stdlib-only)

IR uses `c"hello\0A\00"` string globals with C escape rules (`\n`→`\0A`,
`"`→`\22`, `\`→`\5C`, NUL terminator). No stdlib helper exists.
Implementable in `std.string` via existing `replace` calls; no upstream
feature needed. Tracked in **#139 — std.string.escape_llvm_c_string
helper** (`priority-medium`, `area-stdlib`, `type-feature`; should land
before #130/#9.2 since struct/string literals are emitted at that phase).

### B5. Recursive mutable state across long `match` on `Expression` → none

Aion match handles 73-variant `TokenKind` in `parser.ai`; the 22-variant
`Expression` is strictly smaller. The Rust `compile_expr` is recursive
(deep nested expressions). Aion supports recursive `mut self` methods as
shown by parser's `parse_block` / `parse_statement`. **No blocker.**

### B6. Building a `.ll` buffer in `Vector<String>` (line-by-line) then writing all at once → none

Pattern already used by parser (builds `Vector<ast.Statement>` one element
at a time, recurses). Codegen will do the same: push IR lines to
`self.output.push(line)`, then `fs.write(path, join(output, "\n"))` at the
end. Requires a `std.string.join(Vector<String>, sep) -> String` helper.
Currently absent from `stdlib/std/string.ai`. Tracked as **#140 —
std.string.join** (`priority-medium`, `area-stdlib`, `type-feature`; needed
by #130/#9.2 and beyond).

### B7. Global string constants for emitted string literals → none

Rust uses `builder.build_global_string_ptr(s, "aion_str")`. The Aion
codegen will produce the same text by appending `@.str.{n} = private
unnamed_addr constant [...] c"...".` lines (so long as B4 lands to escape
correctly). No language change.

### B8. Tail-call-sized recursive descent → none

Rust `compile_expr` recurses on every infix branch + nested struct init +
match. Aion parser already recurses 5+ levels deep. STACK space — verified
by `tests/fixtures/language/recursion_deep.ai`. No blocker.

### B9. Cross-module circular references → none

`expressions.rs` uses `compile_lvalue` from `lvalues.rs`, which uses
`compile_expr` from `expressions.rs`. In Aion, a single `compiler/codegen.ai`
file can host both functions; or split files via module imports like
`compiler/codegen/expressions.ai` + `compiler/codegen/lvalues.ai`. Aion's
`use` directive already supports this pattern (see `parser.ai`'s `use
std.option` + `use std.result`). No blocker — **but** verify that two-way
`use` works (open issue for confidence). Tracked in **#141 — Two-way
mutual `use` import test** (`priority-low`, `type-test`, `area-compiler`).

### B10. `@llvm.gcroot` emission needs explicit text → none

Codegen.ai emits `call void @llvm.gcroot(...)`. Aion itself does not invoke
the intrinsic — only formats it as text. No blocker.

### B11. Pass manager / `opt-15 -verify` invocation → none

The optimizer (`Compiler::optimize` in `compiler.rs:507`) is external:
Rust calls `PassManagerBuilder` + `PassManager`. In the Aion port, we
**skip the optimizer** — `codegen.ai` writes `.ll` text and
`opt-15 -verify`/`llc-15`/`clang-15` are invoked externally (already part
of the Docker build flow). No Aion-side optimizer code is required. This
slices ~30 lines off the port. Documented in #129 acceptance.

## 4. Recommended landing order

```
B1 (#136 OrderedMap)         ─┐
B4 (#139 escape_c_string)     ├──► land before #130 (Phase B: layout ABI)
B6 (#140 string.join)         ┘
B2 (#137 std.fmt.s)         ─► land before #131 (Phase B: expressions)
B5 / B7 / B8 / B9 / B10 / B11 → no action
B3 (#138 ?) → deferred (NOT blocking Phase 2)
```

All B5–B11 are non-issues. Three blockers (B1, B4, B6) MUST land before
#130; one ergonomic improvement (B2) before #131; one (B3) deferred.

## 5. Open blocker issues

| ID | Title | Priority | Blocks | Status |
|---|---|---|---|---|
| #136 | OrderedMap stdlib module | high | #129 | open |
| #137 | std.fmt.s placeholder formatter | medium | #131 (recommended) | open |
| #138 | `?` operator for Result<T, E> | low | (deferred) | open |
| #139 | std.string.escape_llvm_c_string | medium | #130 | open |
| #140 | std.string.join | medium | #130 | open |
| #141 | Mutual `use` import test | low | #133 | open |

## 6. Reference: Aion stdlib used by `parser.ai` + `lexer.ai`

These imports prove the codegen port has equivalent support:

```
use std.collections.vector  // Vector<T> with push/get/len/remove/insert
use std.option              // Option<T> with unwrap/unwrap_or/is_some
use std.result              // Result<T, E> with Ok/Err/unwrap/unwrap_err
use std.string              // len/concat/from_int/from_float/at/substr/...
use std.fs                  // write/append/exists/read_to_string
use core.heap               // null/alloc/realloc/free
use core.memory             // memcpy via @intrinsic
```

**Codegen's expected imports** (predicted):

```
use std.collections.vector  // IR line buffer
use std.collections.ordered_map // B1 — replaces HashMap for determinism
use std.option
use std.result
use std.string              // + escape_llvm_c_string (B4) + join (B6)
use std.fs                  // writing the .ll file
use std.fmt                 // + s(...) ergonomics (B2)
use compiler.ast            // AST input
use compiler.token          // for intrinsic attribute values & modifiers
```

## 7. Audit verification

This audit was produced by reading the 8 codegen Rust files in full
(5 110 LOC), the existing `ast.ai` / `lexer.ai` / `parser.ai` (1 070 lines
of Aion), the stdlib headers for `std.string`, `std.io`, `std.fs`,
`std.option`, `std.result`, `std.iter`, `std.collections.{vector,map}`,
`docs/SPEC.md` (sections 1 → 6), and the working fixtures in
`tests/fixtures/{language,compiler}/`. No claim above is speculative;
every ✅ is backed by existing Aion code, every ⚠️ has an opened issue.