# TRACK-011 Implementation Plan: Self-Hosting Parser

## Phase 1: Preparation & AST Definition
- [x] **Step 1.1:** Analyze the existing Rust AST (`src/ast.rs`).
- [x] **Step 1.2:** Create `compiler/ast.ai`. Define the core AST Enums (`Expr`, `Stmt`, `Decl`, `Type`) and Structs.
- [x] **Step 1.3:** Test the compilation of `ast.ai` using the current Aion compiler to ensure it can handle the complexity of the AST types. Fix any compiler bugs that arise.

## Phase 2: Parser Core & Expressions
- [ ] **Step 2.1:** Create `compiler/parser.ai` and set up the `Parser` struct holding a list of tokens and a current index.
- [ ] **Step 2.2:** Implement helper methods: `peek`, `advance`, `expect`, `is_at_end`.
- [ ] **Step 2.3:** Implement `parse_primary()` for literals and identifiers.
- [ ] **Step 2.4:** Implement binary expression parsing (handling precedence).

## Phase 3: Statements & Declarations
- [ ] **Step 3.1:** Implement statement parsing (`let`, `return`, `if`, expression statements).
- [ ] **Step 3.2:** Implement declaration parsing (`fn`, `struct`, `enum`).
- [ ] **Step 3.3:** Ensure error handling uses `Result` properly and halts gracefully on syntax errors.

## Phase 4: Integration & Validation
- [ ] **Step 4.1:** Connect `lexer.ai` to `parser.ai` in a test fixture.
- [ ] **Step 4.2:** Create a fixture (`046_self_parser.ai`) that lexes and parses a simple Aion function and prints the resulting AST.
- [ ] **Step 4.3:** Verify the output matches the expected AST structure.
