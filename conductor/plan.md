# Project Audit & Gemini Config Optimization Plan

## Objective
Perform a final audit of the Aion project, fix any remaining Rust compiler warnings, and ensure the `.gemini` configuration is optimized and accurate.

## Steps

### 1. Fix Rust Compiler Warnings
The previous test run highlighted a few warnings:
- `unused import: Token` in `src/transpiler/sql.rs`
- `unused import: Token` in `src/compiler.rs`
- `unused variable: generic_args` in `src/checker.rs`
- `variable does not need to be mutable` in `src/compiler.rs`
- `unused variable: mn` in `src/compiler.rs`

I will edit these files to remove or correctly prefix these unused elements.

### 2. Audit & Optimise `.gemini` configuration
- Review `.gemini/GEMINI.md`: Ensure the invariants reflect the current Phase 1.7 state accurately. The file is currently in good shape but I will check if any redundant `...` sections can be improved or removed for better context efficiency.
- Review `.gemini/skills.md`: Ensure it contains useful guidance for the AI.

### 3. Final Verification
- Run `cargo check` to confirm there are zero warnings in the Rust compiler.
- Run `python3 runner.py test` to ensure the 38 tests still pass.

## Outcome
A clean, warning-free compiler core, fully documented and optimized for future AI workflows.
