# Aion - Tracks Registry

## Active Tracks

| ID | Name | Status | Owner | Description |
| :--- | :--- | :--- | :--- | :--- |
| `TRACK-001` | [Parser Robustness](tracks/parser-robustness/index.md) | ✅ Done | Architect | Fix infinite loops and ambiguity in `parse_primary`. |
| `TRACK-004` | [Parser Refinement](tracks/parser-refinement/spec.md) | ✅ Done | Architect | Implement Lookahead (LL(k)) to disambiguate StructInst from If/Match. |
| `TRACK-002` | [Standard Library Expansion](tracks/stdlib-expansion/index.md) | 🚧 In Progress | Librarian | Implement `Result`, `Option`, and basic IO. |
| `TRACK-003` | [Generics Implementation](tracks/generics/index.md) | 📅 Planned | Architect | Monomorphization for Functions and Structs. |

## Backlog

-   `TRACK-004`: Error Handling (Result<T, E> enum support).
-   `TRACK-005`: Memory Safety (Borrow Checker prototype).
-   `TRACK-006`: Networking (TCP/HTTP stack).
