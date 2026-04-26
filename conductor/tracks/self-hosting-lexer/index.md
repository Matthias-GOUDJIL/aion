# TRACK-010: Self-Hosting Lexer (Phase 2)

## Status: 🚧 In Progress
**Owner:** Architect

## Goals
Initiate **Phase 2: The Great Port** by writing the Lexer of the Aion language in pure Aion code. This will serve as the ultimate stress test for the standard library (`String`, `Result`, `Vector`, `Enum`).

## Scope
1. **Tokens Definition**: Translate `src/token.rs` into `compiler/token.ai`.
2. **Lexer Logic**: Translate `src/lexer.rs` into `compiler/lexer.ai`.
3. **Integration**: Create a small `compiler/main.ai` that takes an Aion source file, tokenizes it with our new Aion lexer, and prints the tokens to prove it matches the Rust output.

## Technical Challenges
- Proper string slicing and char iteration in Aion (`std.string.substr`).
- Deep `match` exhaustion on `TokenKind` variants.
- Safely handling pointers and memory if string manipulations get complex.

## Plan
See [plan.md](plan.md) for detailed steps.