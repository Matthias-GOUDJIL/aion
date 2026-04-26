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

## Backlog

-   `TRACK-008`: Memory Safety (Borrow Checker prototype).
-   `TRACK-009`: Networking (TCP/HTTP stack).
-   `TRACK-011`: Self-Hosting Parser (`parser.ai`).
