# SPEC-004: Parser Refinement & Lookahead

## Context
The current LL(1) parser is naive when encountering an `Identifier` followed by `{`. It immediately assumes a `StructInst` (Struct Instantiation).

This creates a conflict in control flow statements (`if`, `match`) where a condition can be a simple variable:
```aion
if condition { ... }
```
Currently, the parser sees `condition {` and attempts to parse `StructInst { name: condition, ... }`, consuming the block as invalid struct fields.

## Workaround (Current Debt)
We force users to parenthesize conditions to break the `Identifier {` sequence:
```aion
if (condition) { ... }
```
This is not idiomatic Aion (which aims for Rust/Go-like syntax) and is a technical debt.

## Proposed Solution
Implement **Lookahead (LL(k))** or specific disambiguation logic in `parse_primary`.

When encountering `Identifier {`, the parser should check the token *after* `{`.
-   If it is `Identifier :` or `Identifier =` or `}`, it is likely a `StructInst`.
-   Otherwise, it is a Block, and `StructInst` parsing should be aborted in favor of returning the Identifier expression.

## Acceptance Criteria
1.  `if condition { ... }` parses correctly without parentheses.
2.  `match variable { ... }` parses correctly without parentheses.
3.  `MyStruct { field: value }` still parses correctly.
4.  Tests `008`, `014`, `015`, `017` are updated to remove the temporary parentheses.
