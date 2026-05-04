# 🎯 Track: Compiler Hardening & Type Safety
**Phase:** 1.6/1.7 - Rigor & Completeness  
**Status:** 🟡 Proposed  
**Owner:** AI Architect + QA Sentinel  
**Target:** `v0.7.1`

## 📋 Objective
Eliminate fragility points in the compilation pipeline (Lexer → Parser → Checker → LLVM) to reach a "Production-Ready" state: zero unjustified `unwrap()`, guaranteed termination of IR blocks, and deterministic type resolution.

## 🔍 Current State Analysis
- `src/compiler.rs`: Heavily relies on `.unwrap()` for LLVM calls and variable lookups. Some `if`/`match` blocks may lack explicit terminators.
- `src/checker.rs`: `resolve_fuzzy_name` and generic substitution are functional but lack safeguards against name collisions or unresolved types.
- `src/lib.rs`: Recursive imports and declaration prefixing are functional but could cause conflicts if two modules export the same name.
- Debug statements: Presence of `println!("DEBUG: ...")` and `eprintln!` in the compilation code.

## 🛠️ Atomic Tasks

### T1: Error Handling Standardization
- [ ] Replace all `.unwrap()` in `compiler.rs` with `.map_err()` or explicit `Err(String)` returns.
- [ ] Create a `CompilerError` enum in `src/lib.rs` or `src/compiler.rs` to type errors (Lexical, Syntax, Type, LLVM, Runtime).
- [ ] Remove debug `println!`/`eprintln!`. Replace with a conditional logging system (`#[cfg(feature = "debug")]`) or move them to a dedicated test file.

### T2: LLVM Block Termination Guarantee
- [ ] Implement a utility function `ensure_block_terminated(builder, default_value)` that checks `get_terminator().is_none()` and injects a `ret` or `unreachable` if necessary.
- [ ] Apply this function at the end of `compile_function`, `compile_block`, and every `if`/`match`/`while` branch.
- [ ] Verify compliance with invariant `GEMINI.md` #4.

### T3: Type Checker Robustness
- [ ] Strengthen `resolve_fuzzy_name` in `checker.rs` to prevent false positives (e.g., `User` matching `std.User` by suffix).
- [ ] Add strict generic validation: ensure all type parameters are fully substituted before LLVM generation.
- [ ] Guarantee that `check_expression` always returns a concrete `Type` or an explicit error, never silently returning `Type::Unknown`.

### T4: Import & Module Isolation
- [ ] In `src/lib.rs`, add an automatic namespace based on the imported file path to prevent global symbol collisions.
- [ ] Validate that `impl` and `struct` declarations are not duplicated during recursive import.

## ✅ Success Criteria
- `cargo check` passes with no warnings.
- `python3 runner.py` executes 100% of fixtures without crashes or memory leaks.
- No residual `unwrap()` in `src/compiler.rs` and `src/checker.rs` (except in unit tests).
- The SQL transpiler and LLVM backend generate valid IR for all `examples/*.ai` files.

## 🔗 Dependencies
- `conductor/tracks/rigor-intelligence/` (alignment phase)
- `conductor/tracks/error-handling/` (reporting standards)
- `GEMINI.md` (v0.7 architectural invariants)

## 📅 Next Steps
1. Validate this plan.
2. Implement T1 (Error Handling) → Commit + Test.
3. Implement T2 (Block Termination) → Commit + Test.
4. Code review & merge.
