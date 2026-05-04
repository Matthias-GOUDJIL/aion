# TRACK-013 Plan: Compiler Error Handling

## Phase 1: Identify all panic! calls
- [ ] 1. Search for all `panic!` in `src/`
- [ ] 2. Categorize by severity (crashes vs internal errors)
- [ ] 3. Focus on parser first (user-facing)

## Phase 2: Replace parser panics
- [ ] 4. Replace `src/parser.rs:910` - "Expected right to be an infix expression"
- [ ] 5. Replace `src/parser.rs:911` - "Expected infix expression"
- [ ] 6. Replace `src/parser.rs:925` - "Expected method call"
- [ ] 7. Replace `src/parser.rs:940` - "Expected function declaration"

## Phase 3: Testing
- [ ] 8. Write test cases that trigger these errors
- [ ] 9. Verify graceful error messages instead of crashes

## Phase 4: Expand to other modules
- [ ] 10. Handle panics in `src/compiler.rs` (if any)
- [ ] 11. Handle panics in `src/checker.rs` (if any)