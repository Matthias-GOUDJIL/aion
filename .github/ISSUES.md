# Aion — GitHub Issues

Create these issues on https://github.com/Matthias-GOUDJIL/aion/issues

## Labels to create first

```
priority:high
priority:medium
priority:low
phase:1
phase:2
area:compiler
area:parser
area:type-system
area:testing
area:ci
area:stdlib
type:refactor
type:feature
type:bug
```

---

## Issue 1: [Refactor] Replace String errors with CompileError enum

**Labels**: `priority:high`, `phase:1`, `area:compiler`, `type:refactor`

### Problem

All error handling uses `Result<T, String>`. This makes it impossible to:
- Programmatically handle different error types
- Add structured metadata (error codes, suggestions)
- Recover from errors in LSP/IDE integration

### Proposed solution

```rust
#[derive(Error, Debug)]
pub enum CompileError {
    #[error("undefined variable '{name}'")]
    UndefinedVariable { name: String, span: Span },
    #[error("type mismatch: expected {expected}, got {actual}")]
    TypeMismatch { expected: Type, actual: Type, span: Span },
    // ... 20+ variants
}
```

### Scope
- `src/checker.rs` (13 error points)
- `src/compiler.rs` (17+ error points)
- `src/parser.rs` (3 eprintln + sentinel values)
- `src/lib.rs` (3 error points)

---

## Issue 2: [Refactor] Add Span to all AST nodes

**Labels**: `priority:high`, `phase:1`, `area:parser`, `type:refactor`

### Problem

Most AST nodes don't carry source position (line/col). Only `Expression::Infix` (via operator Token) and `Expression::Call`/`MethodCall` (recently added) have positions. This makes error reporting incomplete.

### Proposed solution

```rust
#[derive(Debug, Clone)]
pub struct Span {
    pub line: usize,
    pub col: usize,
}

#[derive(Debug, Clone)]
pub struct Spanned<T> {
    pub node: T,
    pub span: Span,
}

// Then: type Expression = Spanned<ExprKind>;
```

### Scope
- `src/ast.rs` — wrap all Expression/Statement variants
- `src/parser.rs` — populate spans during parsing
- `src/checker.rs` — use spans in error messages
- `src/compiler.rs` — use spans in error messages

---

## Issue 3: [Refactor] Replace String types with proper Type enum

**Labels**: `priority:high`, `phase:1`, `area:type-system`, `type:refactor`

### Problem

Types are represented as strings everywhere (`"i64"`, `"String"`, `"std.collections.vector.Vector"`). No compile-time checking, no autocomplete, unsafe refactoring.

### Proposed solution

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Unit,
    Int,
    Float,
    Bool,
    String,
    Duration,
    Date,
    Pointer(Box<Type>),
    Struct(Path),
    Enum(Path),
    Function { params: Vec<Type>, ret: Box<Type> },
    Generic(Box<Type>, Vec<Type>),
    Placeholder(String),
}
```

With a `Path` type for qualified names instead of dotted strings.

### Scope
- `src/types.rs`
- `src/checker.rs`
- `src/compiler.rs`
- `src/environment.rs`

---

## Issue 4: [Refactor] Parser returns Result with error collection

**Labels**: `priority:high`, `phase:1`, `area:parser`, `type:refactor`

### Problem

The parser:
- Returns `Program` directly, never errors
- Produces sentinel values (`Identifier("invalid_token_...")`) silently
- Only reports 3 errors via `eprintln!`, swallows the rest
- Users get confusing errors later in checker/compiler

### Proposed solution

```rust
pub fn parse(source: &str) -> Result<Program, Vec<CompileError>> {
    let mut errors = Vec::new();
    // ... parse, collect errors ...
    if errors.is_empty() { Ok(program) } else { Err(errors) }
}
```

Key changes:
- `parse_program()` returns `Result<Program, Vec<CompileError>>`
- Sentinel expressions eliminated — real errors reported
- Multiple errors collected before returning (for IDE use)

### Scope
- `src/parser.rs` (major refactor)
- `src/lib.rs` (handle new Result type)

---

## Issue 5: [Test] Replace Python test runner with Rust integration tests

**Labels**: `priority:medium`, `phase:1`, `area:testing`, `type:refactor`

### Problem

The Python `runner.py` parses stdout between `---` delimiters. Fragile — changing output format breaks all tests (we saw this happen).

### Proposed solution

Option A: Rust integration tests with `assert_cmd`
```rust
#[test]
fn test_hello() {
    let output = Command::new("./aion").args(["run", "tests/fixtures/001_hello.ai"]).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "Hello from Test Harness!");
}
```

Option B: Snapshot testing with `insta` crate
```rust
#[test]
fn test_hello() {
    let output = compile_and_run("tests/fixtures/001_hello.ai");
    assert_snapshot!(output);
}
```

### Scope
- Delete `runner.py`
- Create `tests/integration.rs` or `tests/snapshots/`

---

## Issue 6: [CI] Set up GitHub Actions

**Labels**: `priority:medium`, `phase:1`, `area:ci`, `type:feature`

### Problem

No CI/CD. Tests must be run manually. No validation on push/PR.

### Proposed solution

GitHub Actions workflow that:
1. Builds the Docker image
2. Runs the full test suite
3. Runs on every push and PR

### Scope
- `.github/workflows/ci.yml`

---

## Issue 7: [Feature] Stdlib JSON parser (TRACK-014)

**Labels**: `priority:medium`, `phase:1`, `area:stdlib`, `type:feature`

### Problem

`std.json` exists but doesn't have a working JSON parser.

### Proposed solution

Implement JSON parsing in pure Aion using `std.string` and `std.fs`.

### Scope
- `stdlib/std/json.ai`

---

## Issue 8: [Refactor] Module organization

**Labels**: `priority:low`, `phase:2`, `area:compiler`, `type:refactor`

### Problem

All compiler files are flat in `src/`. No clear module boundaries. Hard to navigate as the codebase grows.

### Proposed solution

```
src/
  lib.rs
  main.rs
  error.rs
  ast/
    mod.rs
    expr.rs
    stmt.rs
    decl.rs
  parser/
    mod.rs
    expr.rs
    stmt.rs
  checker/
    mod.rs
    expr.rs
    stmt.rs
  compiler/
    mod.rs
    expr.rs
    stmt.rs
    builtins.rs
  runtime.c
```

### Scope
- All `src/` files

---

## Issue 9: [Feature] Phase 2 v0.9 — LLVM Backend in Aion

**Labels**: `priority:high`, `phase:2`, `area:compiler`, `type:feature`

### Problem

The LLVM backend is still in Rust. Self-hosting requires rewriting it in Aion.

### Proposed solution

Write `compiler/codegen.ai` that:
1. Takes an AST (from `compiler/parser.ai`)
2. Generates LLVM IR text files (`.ll`)
3. Supports the same features as `src/compiler.rs`

This is the last major milestone before full self-hosting.

### Dependencies
- Depends on: #2 (Spans), #3 (Type enum), #4 (Parser errors)
- Blocks: Phase 3 (Full self-hosting)

---

## Issue 10: [Feature] LSP server for IDE support

**Labels**: `priority:low`, `phase:3`, `area:compiler`, `type:feature`

### Problem

No IDE support. No autocomplete, no diagnostics, no go-to-definition.

### Proposed solution

Implement a Language Server Protocol (LSP) server that reuses the parser and checker.

### Dependencies
- Depends on: #1 (CompileError), #2 (Spans), #3 (Type enum)

---

## Issue 11: [Bug] generate_docs() is a placeholder

**Labels**: `priority:low`, `phase:2`, `area:compiler`, `type:bug`

### Problem

`src/lib.rs:116`: `pub fn generate_docs(_: &str) -> Result<String, String> { Ok("Documentation placeholder".to_string()) }`

The `./aion doc` command exists but does nothing.

### Proposed solution

Either implement documentation generation or remove the `doc` subcommand.
