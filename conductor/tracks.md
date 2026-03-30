# Aion - Tracks Registry

## Active Tracks

| ID | Name | Status | Owner | Description |
| :--- | :--- | :--- | :--- | :--- |
| `TRACK-001` | [Parser Robustness](tracks/parser-robustness/index.md) | ✅ Done | Architect | Fix infinite loops and ambiguity in `parse_primary`. |
| `TRACK-004` | [Parser Refinement](tracks/parser-refinement/spec.md) | ✅ Done | Architect | Implement Lookahead (LL(k)) to disambiguate StructInst from If/Match. |
| `TRACK-002` | [Standard Library Expansion](tracks/stdlib-expansion/index.md) | 🚧 In Progress | Librarian | Vector, Option, Env, and IO implemented. |
| `TRACK-005` | [Runtime & Entry Point Fixes](tracks/runtime-fixes/index.md) | ✅ Done | Architect | Fix argc/argv initialization and struct pass-by-pointer. |
| `TRACK-003` | [Generics Implementation](tracks/generics/index.md) | ✅ Done | Architect | Monomorphization for Functions and Structs. |
| `TRACK-006` | [Rigor & Intelligence](tracks/rigor-intelligence/index.md) | ✅ Done | Architect | Stabilize core (Method Chaining, Short-circuiting) and start AI Tensor. |

## Backlog

-   `TRACK-004`: Error Handling (Result<T, E> enum support).
-   `TRACK-005`: Memory Safety (Borrow Checker prototype).
-   `TRACK-006`: Networking (TCP/HTTP stack).
