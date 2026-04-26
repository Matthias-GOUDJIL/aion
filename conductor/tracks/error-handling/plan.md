# Plan for TRACK-007: Error Handling

## 1. Initial Test Setup
- [x] Create `tests/fixtures/039_result_basic.ai`.
- [x] Create `tests/fixtures/040_result_methods.ai`.

## 2. Stdlib Implementation
- [x] Verify or create `stdlib/std/result.ai`.
- [x] Implement `Result<T, E> { Ok(T), Err(E) }`.

## 3. Execution & Debugging
- [x] Run `python3 runner.py` to see how the compiler handles the new tests.
- [x] If generic resolution fails for multiple parameters (e.g., `Result<String, String>`), patch `src/compiler.rs` (specifically `get_expr_type_name` and `substitute_types_in_expr`).
- [x] If LLVM PHI nodes crash for `Match` arms of `Result`, patch `src/compiler.rs`.

## 4. Final Polish
- [x] Ensure all 38+ tests pass.