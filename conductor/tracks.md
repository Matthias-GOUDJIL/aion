# Aion - Tracks Registry

## Active Tracks

| ID | Name | Status | Owner | Description |
| :--- | :--- | :--- | :--- | :--- |
| `TRACK-001` | [Parser Robustness](tracks/parser-robustness/index.md) | ✅ Done | Architect | Fix infinite loops and ambiguity in `parse_primary`. |
| `TRACK-004` | [Parser Refinement](tracks/parser-refinement/spec.md) | ✅ Done | Architect | Implement Lookahead (LL(k)) to disambiguate StructInst from If/Match. |
| `TRACK-002` | [Standard Library Expansion](tracks/stdlib-expansion/index.md) | ✅ Done | Librarian | Vector, Option, Env, and IO implemented. |
| `TRACK-005` | [Runtime & Entry Point Fixes](tracks/runtime-fixes/index.md) | ✅ Done | Architect | Fix argc/argv initialization and struct pass-by-pointer. |
| `TRACK-003` | [Generics Implementation](tracks/generics/index.md) | ✅ Done | Architect | Monomorphization for Functions and Structs. |
| `TRACK-006` | [Rigor & Intelligence](tracks/rigor-intelligence/index.md) | ✅ Done | Architect | Stabilize core (Method Chaining, Short-circuiting) and start AI Tensor. |
| `TRACK-007` | [Error Handling](tracks/error-handling/index.md) | ✅ Done | Librarian | Implement `Result<T, E>` enum support and robust error management. |
| `TRACK-010` | [Self-Hosting Lexer](tracks/self-hosting-lexer/index.md) | ✅ Done | Architect | Start Phase 2 by writing the Aion Lexer in pure Aion (`lexer.ai`). |
| `TRACK-011` | [Self-Hosting Parser](tracks/self-hosting-parser/index.md) | ✅ Done | Architect | Phase 2.1: Implement Aion's AST and Parser (`parser.ai`) in pure Aion. |
| `TRACK-012` | [Fix Broken Tests](tracks/fix-broken-tests/index.md) | 🔄 In Progress | - | Fix failing tests 021_env_args and 022_vector_repro. |
| `TRACK-013` | [Compiler Error Handling](tracks/compiler-error-handling/index.md) | ⏳ Pending | - | Replace `panic!` calls with proper error handling. |
| `TRACK-014` | [Stdlib JSON Implementation](tracks/stdlib-json-implementation/index.md) | ⏳ Pending | - | Implement working JSON parser in std.json. |
| `TRACK-015` | [Better Compiler Messages](tracks/better-compiler-messages/index.md) | ⏳ Pending | - | Add line numbers, code snippets, and suggestions to errors. |

## Backlog

-   `TRACK-008`: Memory Safety (Borrow Checker prototype).
-   `TRACK-009`: Networking (TCP/HTTP stack).
-   `TRACK-012`: IDE & Tooling (Auto-completion from Stdlib, Code Formatter, LSP, Debugging support).
