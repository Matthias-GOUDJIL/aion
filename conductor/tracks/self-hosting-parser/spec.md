# TRACK-011 Specification: Self-Hosting Parser

## Goal
Implement a recursive descent parser for the Aion language, written in Aion itself (`compiler/parser.ai` and `compiler/ast.ai`).

## Requirements

1. **AST Definition (`ast.ai`)**:
   - Define the Aion AST using Aion's `enum` and `struct` features.
   - Support `Expr` (Literals, Binary Ops, Unary Ops, Identifiers, Calls, Method Calls, etc.).
   - Support `Stmt` (Let, Assign, If, While, Return, ExprStmt).
   - Support `Type` (Basic, Generic, Array, Func).
   - Support `Decl` (Function, Struct, Enum, Impl).

2. **Parser Implementation (`parser.ai`)**:
   - Consume tokens provided by `lexer.ai`.
   - Implement `parse_expr()`, `parse_stmt()`, and `parse_decl()` functions using recursive descent.
   - Properly handle operator precedence (either Pratt parsing or standard recursive descent).
   - Return a `Result<Program, String>` or equivalent error mechanism on parse failure.

3. **Compiler Stress-Test**:
   - The construction of this parser will stress test the current Rust-based compiler's handling of `Result`, recursive `enum` structures, dynamic memory (`Vec<T>`), and pattern matching.
   - Blockers encountered in the Rust compiler (e.g., missing `?` operator) should be reported and fixed dynamically.

4. **Testing**:
   - Implement unit tests within the `tests/` directory (e.g., `tests/fixtures/046_self_parser.ai`) to validate parsing of small code snippets into the expected AST structures.
