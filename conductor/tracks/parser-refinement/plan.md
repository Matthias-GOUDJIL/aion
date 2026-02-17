# Plan for TRACK-004: Parser Refinement

## 1. Implement Lookahead (LL(k))
Currently, `Lexer` provides `peek_char()` but `Parser` only peeks one token. We need to peek deeper.

### Task 1.1: `peek_token_2`
Implement a method `peek_token_2(&mut self) -> Token` in `Parser`.
-   [x] Implemented `peek_at(n)` using a token buffer.
-   [x] `peek_at(0)` is next token, `peek_at(1)` is token after.

## 2. Refactor `parse_primary` Logic for `Identifier {`

When encountering `Identifier` then `{`:
1.  Consume `{`.
2.  Peek at the *next* token (after `{`).
3.  **Check**: Is it `Identifier` followed by (`:` or `=`)? Or `}` (empty struct)?
    -   Yes -> Proceed with `StructInst`.
    -   No -> **Backtrack** (Problem: Backtracking is hard in naive parser) OR **Reinterpret**.

### Alternative: Non-Backtracking Solution
Since `parse_primary` returns `Expression`, we can implement logic to return `Expression::StructInst` ONLY if the contents match a struct field pattern.
If it doesn't match, we return `Expression::Identifier` but we have already consumed `{`!

**Better Approach:**
Modify `parse_primary` to peek at the token *after* `{` BEFORE consuming `{`.
-   If `next_token` is `{`, peek *again*.
-   If `token_after_brace` is `Identifier` AND `token_after_identifier` is (`:` or `=`), then it IS a struct.
-   Else, it is just an Identifier expression followed by a Block (which `If` statement will handle).

## 3. Implementation Steps

1.  [x] Add `peek_token_at(n)` capability to `Lexer` or `Parser`.
    -   Implemented via `VecDeque` buffer in `Parser`.
2.  [x] Update `parse_primary` to check for `StructInst` validity *before* consuming `{`.
    -   Logic implemented: checks for `Identifier` followed by `Colon` or `Eq` inside braces.
3.  [x] Remove parentheses from `tests/fixtures/*.ai`.
    -   Verified that existing tests are clean.
    -   Added `tests/fixtures/018_struct_ambiguity.ai` to prevent regression.
4.  [x] Verify all tests pass.
    -   All tests passed including new test case.
    -   Also fixed infinite loop in `match` parser for unexpected tokens.
