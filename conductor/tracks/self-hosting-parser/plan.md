# TRACK-011 Implementation Plan: Self-Hosting Parser

## Phase 1: Preparation & AST Definition
- [x] **Step 1.1:** Analyze the existing Rust AST (`src/ast.rs`).
- [x] **Step 1.2:** Create `compiler/ast.ai`. Define the core AST Enums and Structs in pure Aion.
- [x] **Step 1.3:** Verify `ast.ai` compiles with the current Rust compiler.

## Phase 2: Core Parsing Logic Implementation
- [x] **Step 2.1:** Implement `Parser` struct and base methods (`peek`, `advance`, `is_at_end`).
- [x] **Step 2.2:** Implement expression parsing (infix, primary, calls).
- [x] **Step 2.3:** Implement statement parsing (let, return, if, while, match).
- [x] **Step 2.4:** Implement block and function parsing.
- [x] **Step 2.5:** Implement struct and module (program) parsing.

## Phase 3: Integration & Bug Squashing
- [x] **Step 3.1:** Integrate with the Aion Lexer (`compiler/lexer.ai`).
- [x] **Step 3.2:** Fix compiler bugs discovered during parser implementation:
    - [x] Fix `MemberAccess` load logic (must load heap pointer from alloca).
    - [x] Implement `Expression::If` in the Rust compiler.
    - [x] Fix `@sizeof` to return 8 for pointer/generic types (preventing `Vector` corruption).
    - [x] Fix `resolve_fuzzy_name` to prevent collisions between generic instances and structs.
- [x] **Step 3.3:** Increase Enum allocation size (256 -> 512) to accommodate large AST enums.

## Phase 4: Validation & Testing
- [x] **Step 4.1:** Compile `parser.ai` into a test binary.
- [x] **Step 4.2:** Create a fixture (`046_self_parser.ai`) that lexes and parses a simple Aion function and prints the resulting AST.
- [x] **Step 4.3:** Verify the output matches the expected AST structure.

## ✅ Final Success
The self-hosting parser is fully functional and can parse basic Aion programs into a valid AST representation.
