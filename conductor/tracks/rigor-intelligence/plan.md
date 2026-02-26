# Implementation Plan: Rigor & Intelligence (Phase 1.7)

Detailed steps to complete the stabilization and AI core integration.

## 🛠️ Task 1: Method Chaining (Compiler Core)
Recursive receiver resolution for `MethodCall`.

1.  **Understand**: `Expression::MethodCall` in `src/compiler.rs` currently only handles identifiers as receivers.
2.  **Implementation**:
    - Modify `compile_expr` for `Expression::MethodCall`.
    - If the receiver is not an `Identifier`, compile it to a `BasicValueEnum`.
    - Recursively resolve the type name of the receiver.
    - If the result is a struct/enum value, store it in an `alloca` to get a pointer if needed (for `self` methods).
    - Call `resolve_fuzzy_name` to find the method on that type.
3.  **Test**: `tests/fixtures/031_method_chaining.ai` (e.g. `Vector<i64>.new().push(1).len()`).

## 🛠️ Task 2: Short-circuiting Logical Operators (`&&`, `||`)
Lazy evaluation for logical AND and OR.

1.  **Understand**: `compiler.rs` currently compiles `Token::And` and `Token::Or` as eager instructions.
2.  **Implementation**:
    - Modify `compile_expr` for `Expression::Infix` with `And`/`Or`.
    - Use `br` (branching) to skip the second operand evaluation if the first one determines the result.
    - Implement a `phi` node in the merge block.
3.  **Test**: `tests/fixtures/032_short_circuit.ai` (e.g. `ptr != null && *ptr == 1`).

## 🛠️ Task 3: AI Tensor Core (Initial @intrinsic)
Introduction of native AI tensor capabilities.

1.  **Understand**: `stdlib/std/ai/tensor.ai` uses non-existent intrinsics.
2.  **Implementation**:
    - Add `ai_tensor_zeros`, `ai_tensor_ones`, etc., to `compiler.rs`.
    - Link to a basic C implementation in `src/runtime.c` for these intrinsics.
    - Support simple tensor creation and addition.
3.  **Test**: `tests/fixtures/033_tensor_basic.ai`.

## 🛠️ Task 4: Memory Management (Boehm GC)
Initial GC integration to prevent memory leaks.

1.  **Understand**: Aion currently relies on `malloc` and leaks strings/objects.
2.  **Implementation**:
    - Install `libgc-dev` in the `Dockerfile`.
    - Link `gcc` against `-lgc`.
    - Update `src/runtime.c` to use `GC_malloc` instead of `malloc`.
3.  **Test**: `tests/fixtures/034_gc_leak.ai` (intensive allocation loop).
