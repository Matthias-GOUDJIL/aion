# TRACK-015 Plan: Better Compiler Error Messages

## Phase 1: Error Type Enhancement
- [ ] 1. Create `ParseError` struct with fields: message, line, column, source snippet
- [ ] 2. Update parser to track line/column during parsing
- [ ] 3. Replace string errors with structured errors

## Phase 2: Error Display
- [ ] 4. Create error formatting function
- [ ] 5. Display: "error: <message>\n  --> file.ai:line:col\n    | \n    | <code>\n    | ^"
- [ ] 6. Integrate with `main.rs` to display errors nicely

## Phase 3: Suggestions
- [ ] 7. Add "did you mean" for unknown identifiers
- [ ] 8. Suggest available methods on types
- [ ] 9. Help with common syntax errors

## Phase 4: Type Checker Integration
- [ ] 10. Add source location to type errors
- [ ] 11. Show expected vs actual types clearly
- [ ] 12. Suggest available trait implementations