# Plan for TRACK-010: Self-Hosting Lexer

## 1. Token Definitions (`compiler/token.ai`)
- [x] Create an `enum TokenKind` reflecting Aion keywords and symbols.
- [x] Create a `struct Token` holding the kind, literal value, line, and column.

## 2. Lexer Core (`compiler/lexer.ai`)
- [x] Define the `Lexer` struct holding source string, position, current char, line, and col.
- [x] Implement `read_char()`, `peek_char()`, and `skip_whitespace()`.
- [x] Implement `next_token() -> Token`.
- [x] Implement robust keyword detection and string literal parsing.

## 3. Testing & Validation
- [x] Create `tests/fixtures/042_self_lexer.ai` which imports `compiler.lexer` and tokenizes a small script.
- [x] Validate that the tokens match exactly what the Rust lexer produces for the same script.