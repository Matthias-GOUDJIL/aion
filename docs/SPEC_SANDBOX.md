# Aion Specification: Sandboxing & Safety

## 1. The Safety Model
Aion adopts a safety model similar to Rust.
By default, all code is **Safe**.
Operations that can violate memory safety or system integrity must be wrapped in `unsafe`.

## 2. Unsafe Operations
The following are considered unsafe:
1.  **Foreign Function Interface (FFI)**: Calling C functions directly (except whitelisted stdlib).
2.  **Raw Pointer Dereference**: Accessing memory via `*ptr`.
3.  **Inline Assembly**: Embedding ASM.
4.  **System Calls**: Direct kernel interaction.

## 3. The `unsafe` Keyword
```aion
unsafe fn dangerous_op() {
    // ...
}

fn main() {
    unsafe {
        dangerous_op()
    }
}
```

## 4. Sandbox Enforcement (Compiler Pass)
The compiler enforces isolation rules based on the build profile:
*   **Kernel Mode**: All operations allowed.
*   **User Mode (Default)**:
    *   Cannot define `unsafe` blocks/functions.
    *   Cannot import modules marked `@system`.
    *   Cannot call non-whitelisted external functions.

## 5. Implementation Status
- [x] **Lexer/Parser**: Support for `unsafe` keyword and `unsafe { ... }` blocks.
- [x] **AST**: `Statement::UnsafeBlock` and `Expression::Block` (with `is_unsafe` flag).
- [x] **Type Checker**: Rigorous safety pass that:
    -   Tracks safety context (`in_unsafe_context`).
    -   Flags an error if an `unsafe fn` is called outside an `unsafe` block or an `unsafe fn`.
- [x] **Codegen**: Transparent execution of `unsafe` blocks in LLVM.
