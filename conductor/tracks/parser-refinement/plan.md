# Plan for TRACK-004: Parser Refinement

## 1. Implement Lookahead (LL(k))
Currently, `Lexer` provides `peek_char()` but `Parser` only peeks one token. We need to peek deeper.

### Task 1.1: `peek_token_2`
Implement a method `peek_token_2(&mut self) -> Token` in `Parser`.
-   This requires a minimal tokenizer buffer or `peekable` iterator.
-   Alternatively, implement `lookahead` within `parse_primary` by consuming `{` and then deciding.

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

1.  [ ] Add `peek_token_at(n)` capability to `Lexer` or `Parser`.
    -   Currently `Lexer` uses `Peekable<Chars>`. We need a token buffer.
2.  [ ] Update `parse_primary` to check for `StructInst` validity *before* consuming `{`.
3.  [ ] Remove parentheses from `tests/fixtures/*.ai`.
4.  [ ] Verify all tests pass.
